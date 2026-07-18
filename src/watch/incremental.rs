use std::path::Path;
use std::time::SystemTime;

use crate::db::Database;

pub fn reindex_files(db: &Database, root: &Path, changed_files: &[&Path]) -> anyhow::Result<()> {
    for file in changed_files {
        if let Some(ext) = file.extension().and_then(|e| e.to_str()) {
            if crate::parse::types::Language::from_extension(ext).is_some() {
                reindex_single_file(db, root, file)?;
            }
        }
    }
    Ok(())
}

/// Handle file deletions by removing all data for deleted files from the DB.
pub fn handle_deleted_files(db: &Database, deleted_paths: &[&Path]) -> anyhow::Result<()> {
    for path in deleted_paths {
        let abs_path = path.to_string_lossy().to_string();
        let file_id: Option<i64> = db
            .conn()
            .query_row(
                "SELECT id FROM files WHERE absolute_path = ?1",
                rusqlite::params![abs_path],
                |row| row.get(0),
            )
            .ok();

        if let Some(fid) = file_id {
            crate::db::write::delete_file_data(db, fid)?;
            tracing::info!(path = %path.display(), "Cleaned up deleted file data");
        }
    }
    Ok(())
}

/// Re-run downstream resolution phases after incremental reindex.
///
/// Watch-mode freshness contract: imports, calls, heritage, framework
/// entry-point marking, dead code, and execution flows are recomputed on
/// every batch (entry points must precede dead code — a handler added under
/// watch that isn't re-marked as an entry point would be flagged dead, the
/// worst possible advice to hand an agent). The FTS search index stays fresh
/// via insert_symbol/delete_file_data. The heavier analytical layers —
/// coupling, git history, communities, api/schema resolution, service
/// topology — refresh on the next full `ariadne index`.
pub fn run_post_reindex_resolution(
    db: &Database,
    root: &Path,
    config: &crate::config::RepoConfig,
) -> anyhow::Result<()> {
    crate::pipeline::import_resolution::resolve_imports(db)?;
    crate::pipeline::call_resolution::resolve_calls(db)?;
    crate::pipeline::heritage::build_heritage(db)?;
    let frameworks = crate::pipeline::discovery::detect_frameworks(root);
    crate::pipeline::framework_entry_points::apply_framework_rules(db, &frameworks, root)?;
    crate::pipeline::dead_code::detect_dead_code(db, config)?;
    crate::pipeline::flow::trace_flows(db)?;
    Ok(())
}

/// Bump the pipeline generation counter so MCP cache knows to rebuild.
pub fn bump_generation(db: &Database) -> anyhow::Result<()> {
    let current: u64 = db
        .get_metadata("pipeline_generation")
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    db.set_metadata("pipeline_generation", &(current + 1).to_string())?;
    Ok(())
}

/// Resolve which service a (possibly new) file belongs to: keep the
/// assignment of a previously indexed file, else pick the service whose
/// root path is the longest prefix of the file's path, else fall back to
/// the first service. Errors when the index has no services at all —
/// fabricating an ID would insert rows that join to nothing.
fn resolve_service_id(db: &Database, abs_path: &str, previous: Option<i64>) -> anyhow::Result<i64> {
    use rusqlite::OptionalExtension;

    if let Some(sid) = previous {
        return Ok(sid);
    }
    let by_prefix: Option<i64> = db
        .conn()
        .query_row(
            "SELECT id FROM services WHERE ?1 LIKE repo_path || '%'
             ORDER BY LENGTH(repo_path) DESC LIMIT 1",
            rusqlite::params![abs_path],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(sid) = by_prefix {
        return Ok(sid);
    }
    db.conn()
        .query_row("SELECT id FROM services ORDER BY id LIMIT 1", [], |row| {
            row.get(0)
        })
        .map_err(|_| anyhow::anyhow!("No services in index — run `ariadne index` before watching"))
}

fn reindex_single_file(db: &Database, root: &Path, path: &Path) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(path)?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let Some(lang) = crate::parse::types::Language::from_extension(ext) else {
        return Ok(());
    };

    let abs_path = path.to_string_lossy().to_string();

    // Find and delete existing file data, remembering its service assignment
    let existing: Option<(i64, i64)> = db
        .conn()
        .query_row(
            "SELECT id, service_id FROM files WHERE absolute_path = ?1",
            rusqlite::params![abs_path],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    if let Some((fid, _)) = existing {
        crate::db::write::delete_file_data(db, fid)?;
    }

    // Re-insert file record
    let rel_path = path.strip_prefix(root).unwrap_or(path);
    let service_id = resolve_service_id(db, &abs_path, existing.map(|(_, sid)| sid))?;

    let modified = path
        .metadata()?
        .modified()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    let new_file_id = crate::db::write::insert_file(
        db,
        service_id,
        &rel_path.to_string_lossy(),
        &abs_path,
        &lang.to_string(),
        modified,
    )?;

    // Parse and ingest through the same path as the full pipeline, so
    // watch-mode files get identical treatment (test marking, caller
    // mapping, FTS upkeep via insert_symbol).
    let parser = crate::parse::get_parser(lang);
    let mut result = parser.parse_file(&source, &abs_path)?;
    for err in crate::pipeline::parsing::ingest_parse_result(db, new_file_id, path, &mut result)? {
        tracing::warn!(error = %err, "Incremental ingest error");
    }

    tracing::info!(path = %rel_path.display(), "Re-indexed file");

    Ok(())
}
