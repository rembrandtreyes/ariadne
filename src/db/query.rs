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

/// A resolution candidate: the symbol row plus the file path an agent or user
/// needs to tell same-name symbols apart.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolCandidate {
    pub symbol: SymbolRow,
    pub file_path: String,
}

/// Outcome of resolving a symbol name against the index.
#[derive(Debug, Clone)]
pub enum SymbolResolution {
    Unique(SymbolRow),
    Ambiguous(Vec<SymbolCandidate>),
    NotFound,
}

const CANDIDATE_SELECT: &str = "SELECT s.id, s.file_id, s.name, s.qualified_name, s.kind, \
     s.line_start, s.line_end, s.is_dead, s.is_test, f.path \
     FROM symbols s JOIN files f ON f.id = s.file_id";

const CANDIDATE_ORDER: &str = "ORDER BY f.path, s.line_start, s.id";

fn row_to_candidate(row: &rusqlite::Row<'_>) -> rusqlite::Result<SymbolCandidate> {
    Ok(SymbolCandidate {
        symbol: row_to_symbol(row)?,
        file_path: row.get(9)?,
    })
}

/// All candidates for a name, in deterministic (file path, line, id) order.
///
/// Pass 1 matches exact name or qualified_name. On a miss, pass 2 takes the
/// trailing `:`-segment ("module:func" → "func") and matches it as a bare
/// name or as a separator-bounded qualified_name suffix.
fn symbol_candidates(db: &Database, name: &str) -> anyhow::Result<Vec<SymbolCandidate>> {
    let mut stmt = db.conn().prepare(&format!(
        "{CANDIDATE_SELECT} WHERE s.name = ?1 OR s.qualified_name = ?1 {CANDIDATE_ORDER}"
    ))?;
    let rows = stmt
        .query_map(params![name], row_to_candidate)?
        .collect::<Result<Vec<_>, _>>()?;
    if !rows.is_empty() {
        return Ok(rows);
    }

    let suffix = name.split(':').next_back().unwrap_or(name);
    if suffix.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt2 = db.conn().prepare(&format!(
        "{CANDIDATE_SELECT} WHERE s.name = ?1 \
         OR s.qualified_name LIKE '%.' || ?1 \
         OR s.qualified_name LIKE '%:' || ?1 \
         OR s.qualified_name LIKE '%/' || ?1 {CANDIDATE_ORDER}"
    ))?;
    let rows = stmt2
        .query_map(params![suffix], row_to_candidate)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Fold `::`, `:`, and `/` qualifiers into `.` so "orders:process" can be
/// compared against a stored qualified_name like "orders.process".
fn normalize_qualifiers(s: &str) -> String {
    s.replace("::", ".").replace([':', '/'], ".")
}

/// Resolve a symbol name to `Unique`, `Ambiguous` (all candidates, ordered by
/// file path, line, id), or `NotFound`.
///
/// Before declaring ambiguity, two narrowing steps run: a candidate whose
/// qualified_name matches the query (exactly, or qualifier-normalized as a
/// full name or dot-bounded suffix) wins when it is the only such match; then
/// `file_hint` (path suffix match) filters the field — a hint that eliminates
/// every candidate is ignored rather than turning a collision into a miss.
pub fn resolve_symbol_by_name(
    db: &Database,
    name: &str,
    file_hint: Option<&str>,
) -> anyhow::Result<SymbolResolution> {
    let mut candidates = symbol_candidates(db, name)?;
    if candidates.is_empty() {
        return Ok(SymbolResolution::NotFound);
    }
    if candidates.len() == 1 {
        return Ok(SymbolResolution::Unique(candidates.remove(0).symbol));
    }

    let exact: Vec<usize> = candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.symbol.qualified_name == name)
        .map(|(i, _)| i)
        .collect();
    if let [only] = exact[..] {
        return Ok(SymbolResolution::Unique(candidates.remove(only).symbol));
    }

    let normalized = normalize_qualifiers(name);
    let qualified: Vec<usize> = candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            let q = normalize_qualifiers(&c.symbol.qualified_name);
            q == normalized || q.ends_with(&format!(".{normalized}"))
        })
        .map(|(i, _)| i)
        .collect();
    if let [only] = qualified[..] {
        return Ok(SymbolResolution::Unique(candidates.remove(only).symbol));
    }

    if let Some(hint) = file_hint {
        let narrowed: Vec<SymbolCandidate> = candidates
            .iter()
            .filter(|c| c.file_path.ends_with(hint))
            .cloned()
            .collect();
        match narrowed.len() {
            0 => {}
            1 => {
                return Ok(SymbolResolution::Unique(
                    narrowed.into_iter().next().expect("len checked").symbol,
                ))
            }
            _ => return Ok(SymbolResolution::Ambiguous(narrowed)),
        }
    }

    Ok(SymbolResolution::Ambiguous(candidates))
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

/// Find a symbol by name. Collisions resolve to the first candidate in
/// deterministic (file path, line, id) order — callers that need to surface
/// ambiguity should use [`resolve_symbol_by_name`] instead.
pub fn find_symbol_by_name(db: &Database, name: &str) -> anyhow::Result<Option<SymbolRow>> {
    Ok(match resolve_symbol_by_name(db, name, None)? {
        SymbolResolution::Unique(sym) => Some(sym),
        SymbolResolution::Ambiguous(mut candidates) => Some(candidates.remove(0).symbol),
        SymbolResolution::NotFound => None,
    })
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

/// Files whose last parse produced syntax errors, worst first.
///
/// Returns `(path, parse_error_count)` pairs. Non-zero counts mean the graph's
/// answers for those files are built from a partial parse.
pub fn get_files_with_parse_errors(db: &Database) -> anyhow::Result<Vec<(String, i64)>> {
    let mut stmt = db.conn().prepare(
        "SELECT path, parse_error_count FROM files
         WHERE parse_error_count > 0
         ORDER BY parse_error_count DESC, path",
    )?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<(String, i64)>, _>>()?;
    Ok(rows)
}

/// Syntax-error count recorded for a single file.
pub fn get_file_parse_error_count(db: &Database, file_id: i64) -> anyhow::Result<i64> {
    let count: i64 = db.conn().query_row(
        "SELECT parse_error_count FROM files WHERE id = ?1",
        params![file_id],
        |row| row.get(0),
    )?;
    Ok(count)
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
///
/// Uses edge-driven loading: loads call edges first (up to `limit`), then loads
/// only the symbols referenced by those edges. This means `symbol_index` contains
/// only symbols that participate in at least one resolved call edge — not every
/// symbol in the database. Memory scales with edge count, not total symbol count.
pub fn build_call_graph(
    db: &Database,
    limit: Option<usize>,
) -> anyhow::Result<crate::graph::CallGraph> {
    use crate::graph::{CallEdge, CallGraph, SymbolNode};
    use std::collections::HashSet;

    let conn = db.conn();

    // Step 1: Load edges first (with limit)
    let edge_limit = limit.unwrap_or(usize::MAX);
    let mut call_stmt = conn.prepare(
        "SELECT caller_symbol_id, callee_symbol_id, confidence, resolution, line
         FROM calls WHERE callee_symbol_id IS NOT NULL LIMIT ?1",
    )?;

    let calls: Vec<(i64, i64, f64, String, u32)> = call_stmt
        .query_map(params![edge_limit as i64], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Step 2: Collect unique symbol IDs referenced by edges
    let mut referenced_ids: HashSet<i64> = HashSet::with_capacity(calls.len());
    for (caller_id, callee_id, _, _, _) in &calls {
        referenced_ids.insert(*caller_id);
        referenced_ids.insert(*callee_id);
    }

    // Step 3: Load only referenced symbols (batch via IN clause, chunked for SQLite limit)
    let mut graph = CallGraph::new();
    // Sorted: NodeIndex assignment must not follow HashSet iteration order,
    // or index-ordered consumers (e.g. kosaraju_scc) vary across runs.
    let mut id_vec: Vec<i64> = referenced_ids.into_iter().collect();
    id_vec.sort_unstable();
    for chunk in id_vec.chunks(500) {
        let placeholders: String = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT s.id, s.name, s.qualified_name, s.kind, f.path, s.is_dead, s.is_test
             FROM symbols s JOIN files f ON s.file_id = f.id
             WHERE s.id IN ({})",
            placeholders
        );
        let mut sym_stmt = conn.prepare(&sql)?;
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = chunk
            .iter()
            .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        let symbols: Vec<SymbolNode> = sym_stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
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
    }

    // Step 4: Add edges (all symbols are guaranteed present from step 3)
    for (caller_id, callee_id, confidence, resolution, line) in calls {
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

/// Resolve multiple file paths to their symbol IDs in a single batched query.
/// Replaces the N+1 pattern of calling find_file_by_path + get_file_symbols per path.
pub fn resolve_paths_to_symbol_ids(db: &Database, paths: &[String]) -> anyhow::Result<Vec<i64>> {
    if paths.is_empty() {
        return Ok(vec![]);
    }
    let conn = db.conn();
    let mut all_ids = Vec::new();

    for chunk in paths.chunks(250) {
        // Build OR conditions: each path matches exact or suffix
        let conditions: Vec<String> = (0..chunk.len())
            .map(|i| {
                format!(
                    "f.path = ?{n} OR f.path LIKE '%/' || ?{n} ESCAPE '\\'",
                    n = i + 1
                )
            })
            .collect();
        let where_clause = conditions.join(" OR ");
        let sql = format!(
            "SELECT s.id FROM files f JOIN symbols s ON s.file_id = f.id WHERE {} ORDER BY s.file_id, s.line_start",
            where_clause
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = chunk
            .iter()
            .map(|p| Box::new(p.clone()) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        let ids: Vec<i64> = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        all_ids.extend(ids);
    }

    Ok(all_ids)
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
         ORDER BY c.strength DESC, c.file_a_id, c.file_b_id",
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
         ORDER BY c.strength DESC, c.file_a_id, c.file_b_id
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

/// A file-level dependency with the symbols that create the connection.
#[derive(Debug, Clone, Serialize)]
pub struct FileDependency {
    pub file_id: i64,
    pub path: String,
    pub language: String,
    pub connections: Vec<SymbolConnection>,
}

/// A pair of symbols that creates a dependency edge between two files.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolConnection {
    pub from_symbol: String,
    pub to_symbol: String,
}

/// Get files that `file_id` depends on (files containing symbols called by symbols in this file).
///
/// Returns each dependent file with the connecting symbol pairs.
/// Only includes resolved calls (callee_symbol_id IS NOT NULL).
pub fn get_file_dependencies(db: &Database, file_id: i64) -> anyhow::Result<Vec<FileDependency>> {
    let mut stmt = db.conn().prepare(
        "SELECT DISTINCT f.id, f.path, f.language, caller_s.name, callee_s.name
         FROM calls c
         JOIN symbols caller_s ON c.caller_symbol_id = caller_s.id
         JOIN symbols callee_s ON c.callee_symbol_id = callee_s.id
         JOIN files f ON callee_s.file_id = f.id
         WHERE caller_s.file_id = ?1
           AND callee_s.file_id != ?1
           AND c.callee_symbol_id IS NOT NULL
         ORDER BY f.path, caller_s.name",
    )?;

    let rows: Vec<(i64, String, String, String, String)> = stmt
        .query_map(params![file_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    group_file_connections(rows)
}

/// Get files that depend on `file_id` (files containing symbols that call symbols in this file).
///
/// Returns each dependent file with the connecting symbol pairs.
/// Only includes resolved calls (callee_symbol_id IS NOT NULL).
pub fn get_file_dependents(db: &Database, file_id: i64) -> anyhow::Result<Vec<FileDependency>> {
    let mut stmt = db.conn().prepare(
        "SELECT DISTINCT f.id, f.path, f.language, caller_s.name, callee_s.name
         FROM calls c
         JOIN symbols caller_s ON c.caller_symbol_id = caller_s.id
         JOIN symbols callee_s ON c.callee_symbol_id = callee_s.id
         JOIN files f ON caller_s.file_id = f.id
         WHERE callee_s.file_id = ?1
           AND caller_s.file_id != ?1
           AND c.callee_symbol_id IS NOT NULL
         ORDER BY f.path, caller_s.name",
    )?;

    let rows: Vec<(i64, String, String, String, String)> = stmt
        .query_map(params![file_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    group_file_connections(rows)
}

/// Group raw (file_id, path, language, from_sym, to_sym) rows into FileDependency structs.
fn group_file_connections(
    rows: Vec<(i64, String, String, String, String)>,
) -> anyhow::Result<Vec<FileDependency>> {
    use std::collections::BTreeMap;

    let mut grouped: BTreeMap<i64, FileDependency> = BTreeMap::new();
    for (fid, path, lang, from_sym, to_sym) in rows {
        let entry = grouped.entry(fid).or_insert_with(|| FileDependency {
            file_id: fid,
            path,
            language: lang,
            connections: Vec::new(),
        });
        entry.connections.push(SymbolConnection {
            from_symbol: from_sym,
            to_symbol: to_sym,
        });
    }

    Ok(grouped.into_values().collect())
}

/// A symbol history row returned from queries.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolHistoryRow {
    pub symbol_id: i64,
    pub symbol_name: String,
    pub qualified_name: String,
    pub kind: String,
    pub file_path: String,
    pub created_at: Option<i64>,
    pub last_modified_at: Option<i64>,
    pub modification_count: i32,
    pub author_count: i32,
    pub is_volatile: bool,
}

/// Get temporal history for a symbol (git blame aggregate).
pub fn get_symbol_history(
    db: &Database,
    symbol_id: i64,
) -> anyhow::Result<Option<SymbolHistoryRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT sh.symbol_id, s.name, s.qualified_name, s.kind, f.path,
                sh.created_at, sh.last_modified_at, sh.modification_count,
                sh.author_count, sh.is_volatile
         FROM symbol_history sh
         JOIN symbols s ON sh.symbol_id = s.id
         JOIN files f ON s.file_id = f.id
         WHERE sh.symbol_id = ?1",
    )?;
    let mut rows = stmt.query(params![symbol_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(SymbolHistoryRow {
            symbol_id: row.get(0)?,
            symbol_name: row.get(1)?,
            qualified_name: row.get(2)?,
            kind: row.get(3)?,
            file_path: row.get(4)?,
            created_at: row.get(5)?,
            last_modified_at: row.get(6)?,
            modification_count: row.get(7)?,
            author_count: row.get(8)?,
            is_volatile: row.get(9)?,
        })),
        None => Ok(None),
    }
}

/// Risk signal data for a single file, aggregated from symbols, history, coupling, and calls.
#[derive(Debug, Clone, Serialize)]
pub struct FileRiskData {
    pub file_id: i64,
    pub path: String,
    pub total_symbols: i64,
    pub dead_symbols: i64,
    pub total_modifications: i64,
    pub max_authors: i64,
    pub volatile_count: i64,
    pub symbols_with_history: i64,
    pub coupled_files: i64,
    pub max_coupling_strength: f64,
    pub external_fan_in: i64,
}

/// Get aggregated risk signal data for a file: churn, coupling, fan-in, dead code.
/// Returns None if the file has no symbols.
pub fn get_file_risk_data(db: &Database, file_id: i64) -> anyhow::Result<Option<FileRiskData>> {
    let path: String = db.conn().query_row(
        "SELECT path FROM files WHERE id = ?1",
        params![file_id],
        |row| row.get(0),
    )?;

    let total_symbols: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM symbols WHERE file_id = ?1",
        params![file_id],
        |row| row.get(0),
    )?;

    if total_symbols == 0 {
        return Ok(None);
    }

    let dead_symbols: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM symbols WHERE file_id = ?1 AND is_dead = 1",
        params![file_id],
        |row| row.get(0),
    )?;

    // Churn signals from symbol_history
    let (total_modifications, max_authors, volatile_count, symbols_with_history): (
        i64,
        i64,
        i64,
        i64,
    ) = db.conn().query_row(
        "SELECT COALESCE(SUM(sh.modification_count), 0),
                COALESCE(MAX(sh.author_count), 0),
                COALESCE(SUM(CASE WHEN sh.is_volatile THEN 1 ELSE 0 END), 0),
                COUNT(sh.id)
         FROM symbols s
         LEFT JOIN symbol_history sh ON sh.symbol_id = s.id
         WHERE s.file_id = ?1",
        params![file_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;

    // Coupling signals
    let (coupled_files, max_coupling_strength): (i64, f64) = db.conn().query_row(
        "SELECT COUNT(*), COALESCE(MAX(strength), 0.0)
         FROM coupling WHERE file_a_id = ?1 OR file_b_id = ?1",
        params![file_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    // External fan-in: distinct callers from OTHER files
    let external_fan_in: i64 = db.conn().query_row(
        "SELECT COUNT(DISTINCT c.caller_symbol_id)
         FROM calls c
         JOIN symbols callee ON c.callee_symbol_id = callee.id
         JOIN symbols caller ON c.caller_symbol_id = caller.id
         WHERE callee.file_id = ?1 AND caller.file_id != ?1",
        params![file_id],
        |row| row.get(0),
    )?;

    Ok(Some(FileRiskData {
        file_id,
        path,
        total_symbols,
        dead_symbols,
        total_modifications,
        max_authors,
        volatile_count,
        symbols_with_history,
        coupled_files,
        max_coupling_strength,
        external_fan_in,
    }))
}

/// Health data for a single symbol, combining call graph metrics and temporal signals.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolHealthData {
    pub id: i64,
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub file_path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub is_dead: bool,
    pub fan_in: i64,
    pub fan_out: i64,
    pub modification_count: i64,
    pub author_count: i64,
    pub is_volatile: bool,
    pub has_history: bool,
}

/// Get health signal data for a symbol by name: fan-in, fan-out, and temporal history.
/// Name collisions resolve to the first candidate in deterministic order; use
/// [`resolve_symbol_by_name`] + [`get_symbol_health_data_for`] to surface ambiguity.
pub fn get_symbol_health_data(
    db: &Database,
    name: &str,
) -> anyhow::Result<Option<SymbolHealthData>> {
    match find_symbol_by_name(db, name)? {
        Some(sym) => Ok(Some(get_symbol_health_data_for(db, &sym)?)),
        None => Ok(None),
    }
}

/// Get health signal data for an already-resolved symbol.
pub fn get_symbol_health_data_for(
    db: &Database,
    sym: &SymbolRow,
) -> anyhow::Result<SymbolHealthData> {
    let file_path = file_path_by_id(db, sym.file_id).unwrap_or_else(|_| "unknown".into());

    let fan_in: i64 = db.conn().query_row(
        "SELECT COUNT(DISTINCT caller_symbol_id) FROM calls
         WHERE callee_symbol_id = ?1",
        params![sym.id],
        |row| row.get(0),
    )?;

    let fan_out: i64 = db.conn().query_row(
        "SELECT COUNT(DISTINCT callee_symbol_id) FROM calls
         WHERE caller_symbol_id = ?1 AND callee_symbol_id IS NOT NULL",
        params![sym.id],
        |row| row.get(0),
    )?;

    let (modification_count, author_count, is_volatile, has_history): (i64, i64, bool, bool) =
        db.conn().query_row(
            "SELECT COALESCE(sh.modification_count, 0),
                    COALESCE(sh.author_count, 0),
                    COALESCE(sh.is_volatile, 0),
                    CASE WHEN sh.id IS NOT NULL THEN 1 ELSE 0 END
             FROM symbols s
             LEFT JOIN symbol_history sh ON sh.symbol_id = s.id
             WHERE s.id = ?1",
            params![sym.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;

    Ok(SymbolHealthData {
        id: sym.id,
        name: sym.name.clone(),
        qualified_name: sym.qualified_name.clone(),
        kind: sym.kind.clone(),
        file_path,
        line_start: sym.line_start,
        line_end: sym.line_end,
        is_dead: sym.is_dead,
        fan_in,
        fan_out,
        modification_count,
        author_count,
        is_volatile,
        has_history,
    })
}

/// Get symbols ranked by combined complexity signals (fan-in + fan-out + volatility).
/// Returns the top `limit` hotspots, excluding test symbols and dead code.
pub fn get_complexity_hotspots(db: &Database, limit: i64) -> anyhow::Result<Vec<SymbolHealthData>> {
    let mut stmt = db.conn().prepare(
        "SELECT s.id, s.name, s.qualified_name, s.kind, f.path,
                s.line_start, s.line_end, s.is_dead,
                (SELECT COUNT(DISTINCT caller_symbol_id) FROM calls WHERE callee_symbol_id = s.id) as fan_in,
                (SELECT COUNT(DISTINCT callee_symbol_id) FROM calls WHERE caller_symbol_id = s.id AND callee_symbol_id IS NOT NULL) as fan_out,
                COALESCE(sh.modification_count, 0),
                COALESCE(sh.author_count, 0),
                COALESCE(sh.is_volatile, 0),
                CASE WHEN sh.id IS NOT NULL THEN 1 ELSE 0 END as has_history
         FROM symbols s
         JOIN files f ON s.file_id = f.id
         LEFT JOIN symbol_history sh ON sh.symbol_id = s.id
         WHERE s.is_test = 0 AND s.is_dead = 0
         ORDER BY (fan_in + fan_out + COALESCE(sh.modification_count, 0) * 0.1
                   + CASE WHEN sh.is_volatile THEN 10 ELSE 0 END) DESC
         LIMIT ?1",
    )?;

    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(SymbolHealthData {
                id: row.get(0)?,
                name: row.get(1)?,
                qualified_name: row.get(2)?,
                kind: row.get(3)?,
                file_path: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
                is_dead: row.get(7)?,
                fan_in: row.get(8)?,
                fan_out: row.get(9)?,
                modification_count: row.get(10)?,
                author_count: row.get(11)?,
                is_volatile: row.get(12)?,
                has_history: row.get(13)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Get symbols whose fan_in meets or exceeds `threshold` — the "god objects" agents
/// should handle carefully. Excludes dead and test symbols. Ordered by fan_in desc.
pub fn get_god_objects(
    db: &Database,
    threshold: i64,
    limit: i64,
) -> anyhow::Result<Vec<SymbolHealthData>> {
    let mut stmt = db.conn().prepare(
        "SELECT s.id, s.name, s.qualified_name, s.kind, f.path,
                s.line_start, s.line_end, s.is_dead,
                (SELECT COUNT(DISTINCT caller_symbol_id) FROM calls WHERE callee_symbol_id = s.id) as fan_in,
                (SELECT COUNT(DISTINCT callee_symbol_id) FROM calls WHERE caller_symbol_id = s.id AND callee_symbol_id IS NOT NULL) as fan_out,
                COALESCE(sh.modification_count, 0),
                COALESCE(sh.author_count, 0),
                COALESCE(sh.is_volatile, 0),
                CASE WHEN sh.id IS NOT NULL THEN 1 ELSE 0 END as has_history
         FROM symbols s
         JOIN files f ON s.file_id = f.id
         LEFT JOIN symbol_history sh ON sh.symbol_id = s.id
         WHERE s.is_test = 0 AND s.is_dead = 0
           AND (SELECT COUNT(DISTINCT caller_symbol_id) FROM calls WHERE callee_symbol_id = s.id) >= ?1
         ORDER BY fan_in DESC, s.name ASC
         LIMIT ?2",
    )?;

    let rows = stmt
        .query_map(params![threshold, limit], |row| {
            Ok(SymbolHealthData {
                id: row.get(0)?,
                name: row.get(1)?,
                qualified_name: row.get(2)?,
                kind: row.get(3)?,
                file_path: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
                is_dead: row.get(7)?,
                fan_in: row.get(8)?,
                fan_out: row.get(9)?,
                modification_count: row.get(10)?,
                author_count: row.get(11)?,
                is_volatile: row.get(12)?,
                has_history: row.get(13)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// A codebase entry point: a symbol where execution originates from outside the project
/// (framework callback, HTTP handler, or `main` function).
#[derive(Debug, Clone, Serialize)]
pub struct EntryPoint {
    pub id: i64,
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub file_path: String,
    pub line_start: u32,
    /// One of: "framework", "http", "main".
    pub category: String,
}

/// List the codebase's entry points: framework-detected callbacks, HTTP/RPC handlers,
/// and `main` functions. Excludes dead code. Use this when onboarding to an unfamiliar
/// codebase to discover where execution begins.
///
/// `category_filter` may be `Some("framework")`, `Some("http")`, `Some("main")`, or `None`
/// (returns all categories). Unknown categories return an empty result set.
pub fn get_entry_points(
    db: &Database,
    category_filter: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<EntryPoint>> {
    // Deduplicate via UNION: the same symbol may be both framework-flagged and an HTTP
    // handler; pick one deterministic category via priority in the outer SELECT.
    let sql = "\
         SELECT id, name, qualified_name, kind, file_path, line_start, category FROM (\
             SELECT s.id, s.name, s.qualified_name, s.kind, f.path AS file_path, \
                    s.line_start, 'http' AS category, 1 AS prio \
               FROM symbols s JOIN files f ON s.file_id = f.id \
               JOIN api_endpoints ae ON ae.handler_symbol_id = s.id \
              WHERE s.is_dead = 0 \
             UNION ALL \
             SELECT s.id, s.name, s.qualified_name, s.kind, f.path AS file_path, \
                    s.line_start, 'framework' AS category, 2 AS prio \
               FROM symbols s JOIN files f ON s.file_id = f.id \
              WHERE s.is_entry_point = 1 AND s.is_dead = 0 \
             UNION ALL \
             SELECT s.id, s.name, s.qualified_name, s.kind, f.path AS file_path, \
                    s.line_start, 'main' AS category, 3 AS prio \
               FROM symbols s JOIN files f ON s.file_id = f.id \
              WHERE s.name = 'main' AND s.kind = 'function' AND s.is_dead = 0\
         ) \
         WHERE (?1 IS NULL OR category = ?1) \
         GROUP BY id \
         HAVING prio = MIN(prio) \
         ORDER BY category ASC, name ASC \
         LIMIT ?2";

    let mut stmt = db.conn().prepare(sql)?;
    let rows = stmt
        .query_map(params![category_filter, limit], |row| {
            Ok(EntryPoint {
                id: row.get(0)?,
                name: row.get(1)?,
                qualified_name: row.get(2)?,
                kind: row.get(3)?,
                file_path: row.get(4)?,
                line_start: row.get(5)?,
                category: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Get symbols that exhibit code smell patterns (high volatility, high fan-in, etc.).
/// Returns all non-test symbols with their health data for smell classification in the tool layer.
pub fn get_code_smell_candidates(db: &Database) -> anyhow::Result<Vec<SymbolHealthData>> {
    let mut stmt = db.conn().prepare(
        "SELECT s.id, s.name, s.qualified_name, s.kind, f.path,
                s.line_start, s.line_end, s.is_dead,
                (SELECT COUNT(DISTINCT caller_symbol_id) FROM calls WHERE callee_symbol_id = s.id) as fan_in,
                (SELECT COUNT(DISTINCT callee_symbol_id) FROM calls WHERE caller_symbol_id = s.id AND callee_symbol_id IS NOT NULL) as fan_out,
                COALESCE(sh.modification_count, 0),
                COALESCE(sh.author_count, 0),
                COALESCE(sh.is_volatile, 0),
                CASE WHEN sh.id IS NOT NULL THEN 1 ELSE 0 END as has_history
         FROM symbols s
         JOIN files f ON s.file_id = f.id
         LEFT JOIN symbol_history sh ON sh.symbol_id = s.id
         WHERE s.is_test = 0
           AND (
             (sh.is_volatile = 1 AND sh.modification_count > 10)
             OR (SELECT COUNT(DISTINCT caller_symbol_id) FROM calls WHERE callee_symbol_id = s.id) > 5
             OR (SELECT COUNT(DISTINCT callee_symbol_id) FROM calls WHERE caller_symbol_id = s.id AND callee_symbol_id IS NOT NULL) > 8
             OR (s.is_dead = 1)
           )
         ORDER BY COALESCE(sh.modification_count, 0) DESC",
    )?;

    let rows = stmt
        .query_map([], |row| {
            Ok(SymbolHealthData {
                id: row.get(0)?,
                name: row.get(1)?,
                qualified_name: row.get(2)?,
                kind: row.get(3)?,
                file_path: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
                is_dead: row.get(7)?,
                fan_in: row.get(8)?,
                fan_out: row.get(9)?,
                modification_count: row.get(10)?,
                author_count: row.get(11)?,
                is_volatile: row.get(12)?,
                has_history: row.get(13)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// File-level summary within a module for the dashboard.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleFileSummary {
    pub name: String,
    pub symbol_count: u64,
    pub dead_count: u64,
    pub risk: f64,
    pub health: f64,
}

/// Module-level aggregation for the dashboard Signal view.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleSummary {
    pub name: String,
    pub path: String,
    pub symbol_count: u64,
    pub file_count: u64,
    pub health: f64,
    pub risk: f64,
    pub dead_count: u64,
    pub cycle_count: u64,
    pub god_objects: u64,
    pub files: Vec<ModuleFileSummary>,
}

/// Build module summaries grouped by top-level source directory.
///
/// Groups files by the first directory component of their path after any "src/" prefix
/// (e.g., "src/pipeline/foo.rs" → "pipeline"). Computes per-module symbol counts,
/// dead code counts, cycle counts (from SCC analysis), god-object counts (fan_in ≥ 20),
/// and per-file breakdowns. Cycles are attributed to every module whose symbols
/// participate in an SCC of size ≥ 2 (a cycle spanning two modules counts once per module).
pub fn get_module_summaries(db: &Database) -> anyhow::Result<Vec<ModuleSummary>> {
    // Build the CallGraph once — reused for cycle detection and kept cheap enough
    // for one dashboard request. get_god_objects is a pure DB query, no graph needed.
    let graph = build_call_graph(db, None)?;
    let cycles_per_module = compute_cycles_per_module(&graph);
    let god_objects_per_module = compute_god_objects_per_module(db, 20, 1000)?;

    let conn = db.conn();

    let mut stmt = conn.prepare(
        "SELECT f.id, f.path,
                COUNT(s.id) as sym_count,
                SUM(CASE WHEN s.is_dead = 1 THEN 1 ELSE 0 END) as dead_count
         FROM files f
         LEFT JOIN symbols s ON s.file_id = f.id
         GROUP BY f.id
         ORDER BY f.path",
    )?;

    struct FileInfo {
        _id: i64,
        path: String,
        sym_count: u64,
        dead_count: u64,
    }

    let file_infos: Vec<FileInfo> = stmt
        .query_map([], |row| {
            Ok(FileInfo {
                _id: row.get(0)?,
                path: row.get(1)?,
                sym_count: row.get::<_, i64>(2)? as u64,
                dead_count: row.get::<_, i64>(3)? as u64,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut module_map: std::collections::BTreeMap<String, Vec<usize>> =
        std::collections::BTreeMap::new();

    for (idx, fi) in file_infos.iter().enumerate() {
        let module_name = extract_module_name(&fi.path);
        module_map.entry(module_name).or_default().push(idx);
    }

    let mut modules = Vec::new();
    for (name, indices) in &module_map {
        let symbol_count: u64 = indices.iter().map(|&i| file_infos[i].sym_count).sum();
        let dead_count: u64 = indices.iter().map(|&i| file_infos[i].dead_count).sum();
        let file_count = indices.len() as u64;

        let file_summaries: Vec<ModuleFileSummary> = indices
            .iter()
            .filter(|&&i| file_infos[i].sym_count > 0)
            .map(|&i| {
                let fi = &file_infos[i];
                let dead_ratio = fi.dead_count as f64 / fi.sym_count as f64;
                let file_risk = dead_ratio.min(1.0);
                ModuleFileSummary {
                    name: fi.path.rsplit('/').next().unwrap_or(&fi.path).to_string(),
                    symbol_count: fi.sym_count,
                    dead_count: fi.dead_count,
                    risk: file_risk,
                    health: (1.0 - file_risk).max(0.0),
                }
            })
            .collect();

        let dead_ratio = if symbol_count > 0 {
            dead_count as f64 / symbol_count as f64
        } else {
            0.0
        };
        let module_risk = dead_ratio.min(1.0);

        modules.push(ModuleSummary {
            name: name.clone(),
            path: format!("src/{}", name),
            symbol_count,
            file_count,
            health: (1.0 - module_risk).max(0.0),
            risk: module_risk,
            dead_count,
            cycle_count: cycles_per_module.get(name).copied().unwrap_or(0),
            god_objects: god_objects_per_module.get(name).copied().unwrap_or(0),
            files: file_summaries,
        });
    }

    modules.sort_by(|a, b| b.symbol_count.cmp(&a.symbol_count));

    Ok(modules)
}

/// A coupling pair with module-level grouping for the dashboard Signal view.
#[derive(Debug, Clone, Serialize)]
pub struct CouplingPairSummary {
    pub from_module: String,
    pub to_module: String,
    pub from_file: String,
    pub to_file: String,
    pub strength: f64,
    pub co_changes: i32,
    pub is_cycle: bool,
}

/// Get top N coupled file pairs, annotated with module names, for the dashboard Signal view.
pub fn get_top_coupling_pairs(
    db: &Database,
    limit: i64,
) -> anyhow::Result<Vec<CouplingPairSummary>> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT fa.path, fb.path, c.co_changes, c.strength
         FROM coupling c
         JOIN files fa ON c.file_a_id = fa.id
         JOIN files fb ON c.file_b_id = fb.id
         ORDER BY c.strength DESC, c.file_a_id, c.file_b_id
         LIMIT ?1",
    )?;

    let pairs = stmt
        .query_map(params![limit], |row| {
            let path_a: String = row.get(0)?;
            let path_b: String = row.get(1)?;
            let co_changes: i32 = row.get(2)?;
            let strength: f64 = row.get(3)?;
            let mod_a = extract_module_name(&path_a);
            let mod_b = extract_module_name(&path_b);
            Ok(CouplingPairSummary {
                from_module: mod_a,
                to_module: mod_b,
                from_file: path_a,
                to_file: path_b,
                strength,
                co_changes,
                is_cycle: false,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(pairs)
}

/// Count SCC-based cycles per module. A cycle spanning multiple modules
/// is counted once against each module whose symbols participate — a vibe-coder
/// seeing `cycles: 2` on a module card expects 2 cycles touch that module.
fn compute_cycles_per_module(
    graph: &crate::graph::CallGraph,
) -> std::collections::HashMap<String, u64> {
    use petgraph::algo::kosaraju_scc;
    let mut per_module: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for scc in kosaraju_scc(&graph.graph) {
        if scc.len() < 2 {
            continue;
        }
        let mut modules_in_cycle: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for node_idx in &scc {
            if let Some(sym) = graph.graph.node_weight(*node_idx) {
                modules_in_cycle.insert(extract_module_name(&sym.file_path));
            }
        }
        for m in modules_in_cycle {
            *per_module.entry(m).or_insert(0) += 1;
        }
    }
    per_module
}

/// Count god-objects (fan_in ≥ threshold) per module by grouping
/// `get_god_objects` results by their file's module.
fn compute_god_objects_per_module(
    db: &Database,
    threshold: i64,
    limit: i64,
) -> anyhow::Result<std::collections::HashMap<String, u64>> {
    let rows = get_god_objects(db, threshold, limit)?;
    let mut per_module: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for row in rows {
        let module = extract_module_name(&row.file_path);
        *per_module.entry(module).or_insert(0) += 1;
    }
    Ok(per_module)
}

/// Extract module name from a file path (e.g., "src/pipeline/foo.rs" → "pipeline").
fn extract_module_name(path: &str) -> String {
    let path = path.strip_prefix("src/").unwrap_or(path);
    match path.split('/').next() {
        Some(first) if path.contains('/') => first.to_string(),
        _ => "root".to_string(),
    }
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
