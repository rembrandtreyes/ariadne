use rusqlite::params;
use serde::Serialize;

use super::Database;

/// A symbol row returned from queries.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolRow {
    pub id: i64,
    pub file_id: i64,
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub line_start: u32,
    pub line_end: u32,
    pub is_dead: bool,
    pub is_test: bool,
}

/// A file row returned from queries.
#[derive(Debug, Clone, Serialize)]
pub struct FileRow {
    pub id: i64,
    pub path: String,
    pub absolute_path: String,
    pub language: String,
}

fn row_to_symbol(row: &rusqlite::Row<'_>) -> rusqlite::Result<SymbolRow> {
    Ok(SymbolRow {
        id: row.get(0)?,
        file_id: row.get(1)?,
        name: row.get(2)?,
        qualified_name: row.get(3)?,
        kind: row.get(4)?,
        line_start: row.get(5)?,
        line_end: row.get(6)?,
        is_dead: row.get(7)?,
        is_test: row.get(8)?,
    })
}

/// Find symbols by exact name.
pub fn get_symbol_by_name(db: &Database, name: &str) -> anyhow::Result<Vec<SymbolRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, file_id, name, qualified_name, kind, line_start, line_end, is_dead, is_test
         FROM symbols WHERE name = ?1",
    )?;
    let rows = stmt
        .query_map(params![name], row_to_symbol)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Returns symbols that CALL `symbol_id` (upstream callers / dependents).
///
/// Example: if `login()` calls `hash_password()`, then `login` is a dependent of `hash_password`.
/// Use this to find what code would break if `symbol_id` changes.
pub fn get_dependents(db: &Database, symbol_id: i64) -> anyhow::Result<Vec<SymbolRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT s.id, s.file_id, s.name, s.qualified_name, s.kind, s.line_start, s.line_end, s.is_dead, s.is_test
         FROM symbols s
         JOIN calls c ON c.caller_symbol_id = s.id
         WHERE c.callee_symbol_id = ?1",
    )?;
    let rows = stmt
        .query_map(params![symbol_id], row_to_symbol)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Returns symbols that `symbol_id` CALLS (downstream dependencies / callees).
///
/// Example: if `login()` calls `hash_password()`, then `hash_password` is a dependency of `login`.
/// Use this to find what `symbol_id` relies on.
pub fn get_dependencies(db: &Database, symbol_id: i64) -> anyhow::Result<Vec<SymbolRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT s.id, s.file_id, s.name, s.qualified_name, s.kind, s.line_start, s.line_end, s.is_dead, s.is_test
         FROM symbols s
         JOIN calls c ON c.callee_symbol_id = s.id
         WHERE c.caller_symbol_id = ?1",
    )?;
    let rows = stmt
        .query_map(params![symbol_id], row_to_symbol)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Full-text search for symbols by name or qualified name.
pub fn search_symbols(db: &Database, query: &str) -> anyhow::Result<Vec<SymbolRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT s.id, s.file_id, s.name, s.qualified_name, s.kind, s.line_start, s.line_end, s.is_dead, s.is_test
         FROM symbols s
         JOIN symbols_fts f ON f.rowid = s.id
         WHERE symbols_fts MATCH ?1
         LIMIT 100",
    )?;
    let rows = stmt
        .query_map(params![query], row_to_symbol)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get all symbols in a file.
pub fn get_file_symbols(db: &Database, file_id: i64) -> anyhow::Result<Vec<SymbolRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, file_id, name, qualified_name, kind, line_start, line_end, is_dead, is_test
         FROM symbols WHERE file_id = ?1 ORDER BY line_start",
    )?;
    let rows = stmt
        .query_map(params![file_id], row_to_symbol)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get all files in a service.
pub fn get_service_files(db: &Database, service_id: i64) -> anyhow::Result<Vec<FileRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, path, absolute_path, language FROM files WHERE service_id = ?1 ORDER BY path",
    )?;
    let rows = stmt
        .query_map(params![service_id], |row| {
            Ok(FileRow {
                id: row.get(0)?,
                path: row.get(1)?,
                absolute_path: row.get(2)?,
                language: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get all files in the database.
pub fn all_files(db: &Database) -> anyhow::Result<Vec<FileRow>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, path, absolute_path, language FROM files ORDER BY path")?;
    let rows = stmt
        .query_map([], |row| {
            Ok(FileRow {
                id: row.get(0)?,
                path: row.get(1)?,
                absolute_path: row.get(2)?,
                language: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get dead code symbols.
pub fn get_dead_symbols(db: &Database) -> anyhow::Result<Vec<SymbolRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, file_id, name, qualified_name, kind, line_start, line_end, is_dead, is_test
         FROM symbols WHERE is_dead = 1 ORDER BY file_id, line_start
         LIMIT 500",
    )?;
    let rows = stmt
        .query_map([], row_to_symbol)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Count files in the database.
pub fn count_files(db: &Database) -> anyhow::Result<u64> {
    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
    Ok(count as u64)
}

/// Count symbols in the database.
pub fn count_symbols(db: &Database) -> anyhow::Result<u64> {
    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?;
    Ok(count as u64)
}

/// Count resolved calls in the database.
pub fn count_resolved_calls(db: &Database) -> anyhow::Result<u64> {
    let count: i64 = db.conn().query_row(
        &format!(
            "SELECT COUNT(*) FROM calls WHERE resolution != '{}'",
            super::RESOLUTION_UNRESOLVED
        ),
        [],
        |row| row.get(0),
    )?;
    Ok(count as u64)
}

/// Count dead symbols.
pub fn count_dead(db: &Database) -> anyhow::Result<u64> {
    let count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM symbols WHERE is_dead = 1",
        [],
        |row| row.get(0),
    )?;
    Ok(count as u64)
}

/// Get the resolution rate (fraction of calls that were resolved).
pub fn resolution_rate(db: &Database) -> anyhow::Result<f64> {
    let total_calls: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM calls", [], |row| row.get(0))?;
    let resolved: i64 = db.conn().query_row(
        &format!(
            "SELECT COUNT(*) FROM calls WHERE resolution != '{}'",
            super::RESOLUTION_UNRESOLVED
        ),
        [],
        |row| row.get(0),
    )?;

    if total_calls == 0 {
        return Ok(0.0);
    }

    Ok(resolved as f64 / total_calls as f64)
}

/// Get a symbol by its database id.
pub fn symbol_by_id(db: &Database, symbol_id: i64) -> anyhow::Result<Option<SymbolRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, file_id, name, qualified_name, kind, line_start, line_end, is_dead, is_test
         FROM symbols WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![symbol_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(row_to_symbol(row)?)),
        None => Ok(None),
    }
}

/// Find a symbol by name (first match).
pub fn find_symbol_by_name(db: &Database, name: &str) -> anyhow::Result<Option<SymbolRow>> {
    // Try exact match first
    let mut stmt = db.conn().prepare(
        "SELECT id, file_id, name, qualified_name, kind, line_start, line_end, is_dead, is_test
         FROM symbols WHERE name = ?1 OR qualified_name = ?1 LIMIT 1",
    )?;
    let mut rows = stmt.query(params![name])?;
    match rows.next()? {
        Some(row) => Ok(Some(row_to_symbol(row)?)),
        None => {
            // Try suffix match (e.g., "module:func" -> qualified_name LIKE "%func")
            let suffix = name.split(':').next_back().unwrap_or(name);
            let mut stmt2 = db.conn().prepare(
                "SELECT id, file_id, name, qualified_name, kind, line_start, line_end, is_dead, is_test
                 FROM symbols WHERE name = ?1 LIMIT 1",
            )?;
            let mut rows2 = stmt2.query(params![suffix])?;
            match rows2.next()? {
                Some(row) => Ok(Some(row_to_symbol(row)?)),
                None => Ok(None),
            }
        }
    }
}

/// Get the file path for a given file_id.
pub fn file_path_by_id(db: &Database, file_id: i64) -> anyhow::Result<String> {
    let path: String = db.conn().query_row(
        "SELECT path FROM files WHERE id = ?1",
        params![file_id],
        |row| row.get(0),
    )?;
    Ok(path)
}

/// Get all imports for a file.
pub fn get_file_imports(db: &Database, file_id: i64) -> anyhow::Result<Vec<ImportRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT i.id, i.imported_name, i.module_path, i.line, i.is_external, i.resolved_file_id, i.resolved_symbol_id
         FROM imports i WHERE i.file_id = ?1 ORDER BY i.line",
    )?;
    let rows = stmt
        .query_map(params![file_id], |row| {
            Ok(ImportRow {
                id: row.get(0)?,
                imported_name: row.get(1)?,
                module_path: row.get(2)?,
                line: row.get(3)?,
                is_external: row.get(4)?,
                resolved_file_id: row.get(5)?,
                resolved_symbol_id: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// An import row returned from queries.
#[derive(Debug, Clone, Serialize)]
pub struct ImportRow {
    pub id: i64,
    pub imported_name: String,
    pub module_path: String,
    pub line: u32,
    pub is_external: bool,
    pub resolved_file_id: Option<i64>,
    pub resolved_symbol_id: Option<i64>,
}

/// Count total calls.
pub fn count_calls(db: &Database) -> anyhow::Result<u64> {
    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM calls", [], |row| row.get(0))?;
    Ok(count as u64)
}

/// Get distinct languages from indexed files.
pub fn get_languages(db: &Database) -> anyhow::Result<Vec<String>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT DISTINCT language FROM files ORDER BY language")?;
    let langs = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(langs)
}

/// Build an in-memory CallGraph from the database.
pub fn build_call_graph(
    db: &Database,
    limit: Option<usize>,
) -> anyhow::Result<crate::graph::CallGraph> {
    use crate::graph::{CallEdge, CallGraph, SymbolNode};

    let mut graph = CallGraph::new();
    let conn = db.conn();

    // Load all symbols with their file paths
    let mut sym_stmt = conn.prepare(
        "SELECT s.id, s.name, s.qualified_name, s.kind, f.path, s.is_dead, s.is_test
         FROM symbols s JOIN files f ON s.file_id = f.id",
    )?;

    let symbols: Vec<SymbolNode> = sym_stmt
        .query_map([], |row| {
            Ok(SymbolNode {
                id: row.get(0)?,
                name: row.get(1)?,
                qualified_name: row.get(2)?,
                kind: row.get(3)?,
                file_path: row.get(4)?,
                is_dead: row.get(5)?,
                is_test: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for sym in symbols {
        graph.add_symbol(sym);
    }

    // Load all resolved calls
    let mut call_stmt = conn.prepare(
        "SELECT caller_symbol_id, callee_symbol_id, confidence, resolution, line
         FROM calls WHERE callee_symbol_id IS NOT NULL",
    )?;

    let calls: Vec<(i64, i64, f64, String, u32)> = call_stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let edge_limit = limit.unwrap_or(usize::MAX);
    for (i, (caller_id, callee_id, confidence, resolution, line)) in calls.into_iter().enumerate() {
        if i >= edge_limit {
            break;
        }
        graph.add_call(
            caller_id,
            callee_id,
            CallEdge {
                confidence,
                resolution,
                line,
            },
        );
    }

    Ok(graph)
}

/// Find a file by its relative path.
pub fn find_file_by_path(db: &Database, path: &str) -> anyhow::Result<Option<FileRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, path, absolute_path, language FROM files WHERE path = ?1 OR path LIKE '%/' || ?1 ESCAPE '\\' LIMIT 1",
    )?;
    let mut rows = stmt.query(params![path])?;
    match rows.next()? {
        Some(row) => Ok(Some(FileRow {
            id: row.get(0)?,
            path: row.get(1)?,
            absolute_path: row.get(2)?,
            language: row.get(3)?,
        })),
        None => Ok(None),
    }
}

/// Get coupling data for a file.
pub fn get_file_couplings(db: &Database, file_id: i64) -> anyhow::Result<Vec<CouplingRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT c.file_a_id, c.file_b_id, c.co_changes, c.strength,
                CASE WHEN c.file_a_id = ?1 THEN fb.path ELSE fa.path END as coupled_path
         FROM coupling c
         JOIN files fa ON c.file_a_id = fa.id
         JOIN files fb ON c.file_b_id = fb.id
         WHERE c.file_a_id = ?1 OR c.file_b_id = ?1
         ORDER BY c.strength DESC",
    )?;
    let rows = stmt
        .query_map(params![file_id], |row| {
            Ok(CouplingRow {
                file_a_id: row.get(0)?,
                file_b_id: row.get(1)?,
                co_changes: row.get(2)?,
                strength: row.get(3)?,
                coupled_path: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// A coupling row returned from queries.
#[derive(Debug, Clone, Serialize)]
pub struct CouplingRow {
    pub file_a_id: i64,
    pub file_b_id: i64,
    pub co_changes: i32,
    pub strength: f64,
    pub coupled_path: String,
}

/// A heritage (inheritance) row returned from queries.
#[derive(Debug, Clone, Serialize)]
pub struct HeritageRow {
    pub id: i64,
    pub child_symbol_id: i64,
    pub child_name: String,
    pub parent_name: String,
    pub parent_symbol_id: Option<i64>,
    pub parent_qualified_name: Option<String>,
    pub kind: String,
}

/// Get heritage (inheritance) relationships for a symbol — both as parent and child.
pub fn get_heritage(db: &Database, symbol_id: i64) -> anyhow::Result<Vec<HeritageRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT h.id, h.child_symbol_id, cs.name, h.parent_name, h.parent_symbol_id,
                ps.qualified_name, h.kind
         FROM heritage h
         JOIN symbols cs ON h.child_symbol_id = cs.id
         LEFT JOIN symbols ps ON h.parent_symbol_id = ps.id
         WHERE h.child_symbol_id = ?1 OR h.parent_symbol_id = ?1
         ORDER BY h.kind, cs.name",
    )?;
    let rows = stmt
        .query_map(params![symbol_id], |row| {
            Ok(HeritageRow {
                id: row.get(0)?,
                child_symbol_id: row.get(1)?,
                child_name: row.get(2)?,
                parent_name: row.get(3)?,
                parent_symbol_id: row.get(4)?,
                parent_qualified_name: row.get(5)?,
                kind: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// An execution flow row returned from queries.
#[derive(Debug, Clone, Serialize)]
pub struct FlowStepRow {
    pub flow_name: String,
    pub step_order: i32,
    pub depth: i32,
    pub symbol_name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub kind: String,
}

/// Get execution flows that pass through a given symbol.
pub fn get_execution_flows(db: &Database, symbol_id: i64) -> anyhow::Result<Vec<FlowStepRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT f.name, fs.step_order, fs.depth, s.name, s.qualified_name, fi.path, s.kind
         FROM flow_steps fs
         JOIN flows f ON fs.flow_id = f.id
         JOIN symbols s ON fs.symbol_id = s.id
         JOIN files fi ON s.file_id = fi.id
         WHERE f.id IN (
             SELECT flow_id FROM flow_steps WHERE symbol_id = ?1
         )
         ORDER BY f.id, fs.step_order
         LIMIT 200",
    )?;
    let rows = stmt
        .query_map(params![symbol_id], |row| {
            Ok(FlowStepRow {
                flow_name: row.get(0)?,
                step_order: row.get(1)?,
                depth: row.get(2)?,
                symbol_name: row.get(3)?,
                qualified_name: row.get(4)?,
                file_path: row.get(5)?,
                kind: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get the top coupled file pairs by co-change strength.
pub fn get_top_couplings(db: &Database, limit: usize) -> anyhow::Result<Vec<CouplingRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT c.file_a_id, c.file_b_id, c.co_changes, c.strength, fa.path || ' <-> ' || fb.path
         FROM coupling c
         JOIN files fa ON c.file_a_id = fa.id
         JOIN files fb ON c.file_b_id = fb.id
         ORDER BY c.strength DESC
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(CouplingRow {
                file_a_id: row.get(0)?,
                file_b_id: row.get(1)?,
                co_changes: row.get(2)?,
                strength: row.get(3)?,
                coupled_path: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// A community row returned from queries.
#[derive(Debug, Clone, Serialize)]
pub struct CommunityRow {
    pub id: i64,
    pub name: String,
    pub symbol_count: i32,
    pub internal_edges: i32,
    pub external_edges: i32,
    pub modularity: f64,
}

/// Get all detected communities with their statistics.
pub fn get_communities(db: &Database) -> anyhow::Result<Vec<CommunityRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, symbol_count, internal_edges, external_edges, modularity
         FROM communities
         ORDER BY symbol_count DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(CommunityRow {
                id: row.get(0)?,
                name: row.get(1)?,
                symbol_count: row.get(2)?,
                internal_edges: row.get(3)?,
                external_edges: row.get(4)?,
                modularity: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// An API endpoint row returned from queries.
#[derive(Debug, Clone, Serialize)]
pub struct ApiEndpointRow {
    pub id: i64,
    pub method: String,
    pub path_pattern: String,
    pub protocol: String,
    pub handler_name: Option<String>,
    pub handler_qualified_name: Option<String>,
    pub file_path: Option<String>,
    pub line: Option<u32>,
}

/// Get all API endpoints with handler information.
pub fn get_api_endpoints(db: &Database) -> anyhow::Result<Vec<ApiEndpointRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT ae.id, ae.method, ae.path_pattern, ae.protocol,
                s.name, s.qualified_name, f.path, ae.line
         FROM api_endpoints ae
         LEFT JOIN symbols s ON ae.handler_symbol_id = s.id
         LEFT JOIN files f ON ae.file_id = f.id
         ORDER BY ae.path_pattern, ae.method",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ApiEndpointRow {
                id: row.get(0)?,
                method: row.get(1)?,
                path_pattern: row.get(2)?,
                protocol: row.get(3)?,
                handler_name: row.get(4)?,
                handler_qualified_name: row.get(5)?,
                file_path: row.get(6)?,
                line: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
