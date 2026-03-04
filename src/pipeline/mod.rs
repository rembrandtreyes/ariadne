pub mod discovery;
pub mod structure;
pub mod parsing;
pub mod import_resolution;
pub mod call_resolution;
pub mod heritage;
pub mod dead_code;
pub mod flow;
pub mod coupling;
pub mod search_index;
pub mod api_resolution;
pub mod service_topology;
pub mod community;
pub mod schema_resolution;

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

    let discovered = discovery::discover(root, config)?;
    let file_count = discovered.files.len();

    structure::create_structure(db, &discovered, root)?;
    parsing::parse_all(db, &discovered)?;
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

    let (sym_count, _file_count, call_count) =
        crate::db::query::count_symbols(db)
            .map(|s| (s, 0u64, 0u64))
            .unwrap_or((0, 0, 0));

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
