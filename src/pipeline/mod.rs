pub mod api_resolution;
pub mod call_resolution;
pub mod community;
pub mod coupling;
pub mod dead_code;
pub mod discovery;
pub mod flow;
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
    let result = run_pipeline_phases(db, root, &discovered);
    match result {
        Ok(()) => {
            db.conn().execute_batch("COMMIT")?;
        }
        Err(e) => {
            let _ = db.conn().execute_batch("ROLLBACK");
            return Err(e);
        }
    }

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
    })
}

/// Run all pipeline phases. Separated to allow transactional wrapping.
fn run_pipeline_phases(
    db: &Database,
    root: &Path,
    discovered: &discovery::DiscoveryResult,
) -> anyhow::Result<()> {
    structure::create_structure(db, discovered, root)?;
    parsing::parse_all(db, discovered)?;
    import_resolution::resolve_imports(db)?;
    call_resolution::resolve_calls(db)?;
    heritage::build_heritage(db)?;
    dead_code::detect_dead_code(db)?;
    flow::trace_flows(db)?;
    coupling::analyze_coupling(db, root)?;
    search_index::build_search_index(db)?;
    api_resolution::resolve_api_boundaries(db)?;
    service_topology::build_topology(db)?;
    community::detect_communities(db)?;
    schema_resolution::resolve_schemas(db, root)?;
    Ok(())
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
}
