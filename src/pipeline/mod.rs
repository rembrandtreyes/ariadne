pub mod api_resolution;
pub mod call_resolution;
pub mod community;
pub mod coupling;
pub mod dead_code;
pub mod discovery;
pub mod flow;
pub mod framework_entry_points;
pub mod heritage;
pub mod import_resolution;
pub mod parsing;
pub mod schema_resolution;
pub mod search_index;
pub mod service_topology;
pub mod structure;

use crate::config::RepoConfig;
use crate::db::Database;
use std::path::Path;
use std::time::Instant;

/// Run the full 14-phase indexing pipeline.
pub fn run_full_pipeline(
    db: &Database,
    root: &Path,
    config: &RepoConfig,
) -> anyhow::Result<PipelineStats> {
    let start = Instant::now();

    // Clear stale data before re-indexing to avoid FK constraint violations
    crate::db::write::clear_all_data(db)?;

    let discovered = discovery::discover(root, config)?;
    let file_count = discovered.files.len();

    // Wrap all pipeline phases in a single transaction for performance and atomicity
    db.conn().execute_batch("BEGIN")?;
    let result = run_pipeline_phases(db, root, &discovered, config);
    match &result {
        Ok(_) => {
            db.conn().execute_batch("COMMIT")?;
        }
        Err(_) => {
            let _ = db.conn().execute_batch("ROLLBACK");
        }
    }
    let phase_durations = result?;

    let sym_count = crate::db::query::count_symbols(db).unwrap_or(0);
    let call_count = crate::db::query::count_calls(db).unwrap_or(0);

    let dead_count = crate::db::query::count_dead(db).unwrap_or(0);
    let rate = crate::db::query::resolution_rate(db).unwrap_or(0.0);

    Ok(PipelineStats {
        files_scanned: file_count,
        symbols_found: sym_count as usize,
        calls_resolved: call_count as usize,
        dead_functions: dead_count as usize,
        resolution_rate: rate,
        duration_ms: start.elapsed().as_millis() as u64,
        phase_durations,
    })
}

/// Run all pipeline phases with per-phase timing. Separated to allow transactional wrapping.
fn run_pipeline_phases(
    db: &Database,
    root: &Path,
    discovered: &discovery::DiscoveryResult,
    config: &RepoConfig,
) -> anyhow::Result<Vec<PhaseTiming>> {
    let mut timings = Vec::with_capacity(13);

    macro_rules! timed {
        ($name:expr, $body:expr) => {{
            let t = Instant::now();
            let result = $body;
            timings.push(PhaseTiming {
                name: $name,
                duration_ms: t.elapsed().as_millis() as u64,
            });
            result
        }};
    }

    timed!(
        "structure",
        structure::create_structure(db, discovered, root)
    )?;
    timed!("parsing", parsing::parse_all(db, discovered))?;
    timed!("import_resolution", import_resolution::resolve_imports(db))?;
    timed!("call_resolution", call_resolution::resolve_calls(db))?;
    timed!("heritage", heritage::build_heritage(db))?;
    timed!(
        "framework_entry_points",
        framework_entry_points::apply_framework_rules(db, &discovered.frameworks, root)
    )?;
    timed!("dead_code", dead_code::detect_dead_code(db, config))?;
    timed!("flow", flow::trace_flows(db))?;
    timed!("coupling", coupling::analyze_coupling(db, root))?;
    timed!("search_index", search_index::build_search_index(db))?;
    timed!("api_resolution", api_resolution::resolve_api_boundaries(db))?;
    timed!("service_topology", service_topology::build_topology(db))?;
    timed!("community", community::detect_communities(db))?;
    timed!(
        "schema_resolution",
        schema_resolution::resolve_schemas(db, root)
    )?;

    Ok(timings)
}

/// Timing for a single pipeline phase.
#[derive(Debug, Clone)]
pub struct PhaseTiming {
    pub name: &'static str,
    pub duration_ms: u64,
}

/// Represents the result of running the full indexing pipeline.
#[derive(Debug, Default)]
pub struct PipelineStats {
    pub files_scanned: usize,
    pub symbols_found: usize,
    pub calls_resolved: usize,
    pub dead_functions: usize,
    pub resolution_rate: f64,
    pub duration_ms: u64,
    pub phase_durations: Vec<PhaseTiming>,
}
