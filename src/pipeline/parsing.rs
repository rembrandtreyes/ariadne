use crate::db::write;
use crate::db::Database;
use crate::parse;
use crate::parse::types::ParseResult;
use rayon::prelude::*;
use std::path::Path;

use super::discovery::DiscoveryResult;

/// Detect integration test files (tests/test_*.rs) and mark all their
/// functions as test symbols so dead code analysis seeds them.
fn mark_test_file_symbols(path: &Path, result: &mut ParseResult) {
    let is_test_file = path
        .file_name()
        .and_then(|f| f.to_str())
        .is_some_and(|name| name.starts_with("test_") && name.ends_with(".rs"))
        && path.to_string_lossy().contains("/tests/");
    if is_test_file {
        for sym in &mut result.symbols {
            if !sym.decorators.contains(&"test".to_string()) {
                sym.decorators.push("test".to_string());
            }
        }
    }
}

/// Ingest one parsed file: symbols, imports, parse-error count, and calls.
///
/// Shared by the full pipeline and watch-mode incremental reindex so the two
/// ingestion paths cannot drift — test-file marking, caller mapping, and
/// error reporting happen exactly once, here. Returns soft per-item errors;
/// the caller decides whether to log or aggregate them.
pub fn ingest_parse_result(
    db: &Database,
    file_id: i64,
    path: &Path,
    result: &mut ParseResult,
) -> anyhow::Result<Vec<String>> {
    let abs_path = path.to_string_lossy();
    let mut errors = Vec::new();

    mark_test_file_symbols(path, result);

    if let Err(e) = write::insert_symbols_batch(db, file_id, &result.symbols) {
        errors.push(format!("Symbol insert error for {}: {}", abs_path, e));
    }

    if let Err(e) = write::insert_imports_batch(db, file_id, &result.imports) {
        errors.push(format!("Import insert error for {}: {}", abs_path, e));
    }

    if let Err(e) = write::set_file_parse_error_count(db, file_id, result.syntax_error_count) {
        errors.push(format!("Parse-error-count update for {}: {}", abs_path, e));
    }

    // Insert calls: look up caller symbol IDs by name within this file
    if !result.calls.is_empty() {
        let mut name_to_id = std::collections::HashMap::new();
        match crate::db::query::get_file_symbols(db, file_id) {
            Ok(syms) => {
                for s in &syms {
                    name_to_id.insert(s.name.clone(), s.id);
                    name_to_id.insert(s.qualified_name.clone(), s.id);
                }
            }
            Err(e) => {
                // Without the map every call in this file would be dropped
                // on the floor — say so instead of silently degrading.
                errors.push(format!(
                    "Caller-symbol lookup error for {}: {}",
                    abs_path, e
                ));
            }
        }

        match db.conn().prepare(&format!(
            "INSERT INTO calls (caller_symbol_id, callee_name, file_id, line, confidence, resolution)
             VALUES (?1, ?2, ?3, ?4, 0.5, '{}')",
            crate::db::RESOLUTION_UNRESOLVED,
        )) {
            Ok(mut stmt) => {
                for call in &result.calls {
                    if let Some(&cid) = name_to_id.get(&call.caller_name) {
                        if let Err(e) =
                            stmt.execute(rusqlite::params![cid, call.callee_name, file_id, call.line])
                        {
                            errors.push(format!("Call insert error for {}: {}", abs_path, e));
                        }
                    }
                }
            }
            Err(e) => {
                errors.push(format!("Call statement prepare error for {}: {}", abs_path, e));
            }
        }
    }

    Ok(errors)
}

/// Phase 2: Parse all discovered files in parallel using rayon.
///
/// Reads each file, runs the appropriate language parser, and stores
/// the extracted symbols, imports, and calls in the database.
pub fn parse_all(db: &Database, discovery: &DiscoveryResult) -> anyhow::Result<()> {
    // Collect parse results in parallel
    let results: Vec<_> = discovery
        .files
        .par_iter()
        .filter_map(|file| {
            let source = match std::fs::read_to_string(&file.path) {
                Ok(s) => s,
                Err(e) => {
                    // Unreadable or non-UTF8 files must not vanish silently —
                    // a missing file quietly degrades every downstream answer.
                    tracing::warn!(path = %file.path.display(), error = %e, "Unreadable file; skipped");
                    return None;
                }
            };
            let parser = parse::get_parser(file.language);
            let file_path = file.path.to_string_lossy().to_string();
            match parser.parse_file(&source, &file_path) {
                Ok(result) => Some((file.path.clone(), result)),
                Err(e) => {
                    // I/O-level parser failure (tree-sitter is error-tolerant, so
                    // syntax problems come back Ok with a syntax_error_count).
                    // Skip the file but say so — a silent drop leaves the graph
                    // missing a file with no tell.
                    tracing::warn!(path = %file_path, error = %e, "Parser failed; file skipped");
                    None
                }
            }
        })
        .collect();

    // Insert results sequentially (SQLite is single-writer)
    let mut errors = Vec::<String>::new();

    for (path, mut result) in results {
        let abs_path = path.to_string_lossy().to_string();

        // Look up file_id by absolute_path
        let file_id: Option<i64> = db
            .conn()
            .query_row(
                "SELECT id FROM files WHERE absolute_path = ?1",
                rusqlite::params![abs_path],
                |row| row.get(0),
            )
            .ok();

        let file_id = match file_id {
            Some(id) => id,
            None => {
                errors.push(format!("No file record for: {}", abs_path));
                continue;
            }
        };

        errors.extend(ingest_parse_result(db, file_id, &path, &mut result)?);
    }

    if !errors.is_empty() {
        tracing::warn!(count = errors.len(), "Parse phase encountered errors");
        for err in &errors {
            tracing::warn!(error = %err, "Parse error");
        }
    }

    Ok(())
}
