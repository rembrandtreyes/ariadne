use rusqlite::params;

use super::Database;
use crate::parse::types::{ParsedCall, ParsedImport, ParsedSymbol};

/// Insert a new service record, returning the new row ID.
pub fn insert_service(
    db: &Database,
    name: &str,
    repo_path: &str,
    service_type: &str,
    primary_language: &str,
) -> anyhow::Result<i64> {
    db.conn().execute(
        "INSERT OR IGNORE INTO services (name, repo_path, type, primary_language, last_indexed)
         VALUES (?1, ?2, ?3, ?4, strftime('%s','now'))",
        params![name, repo_path, service_type, primary_language],
    )?;
    let id = db.conn().query_row(
        "SELECT id FROM services WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )?;
    Ok(id)
}

/// Insert a file record, returning the new row ID.
pub fn insert_file(
    db: &Database,
    service_id: i64,
    path: &str,
    absolute_path: &str,
    language: &str,
    last_modified: f64,
) -> anyhow::Result<i64> {
    db.conn().execute(
        "INSERT OR REPLACE INTO files (service_id, path, absolute_path, language, last_modified, last_indexed)
         VALUES (?1, ?2, ?3, ?4, ?5, strftime('%s','now'))",
        params![service_id, path, absolute_path, language, last_modified],
    )?;
    Ok(db.conn().last_insert_rowid())
}

/// Record how many syntax-error nodes the parser saw for a file.
pub fn set_file_parse_error_count(db: &Database, file_id: i64, count: usize) -> anyhow::Result<()> {
    db.conn().execute(
        "UPDATE files SET parse_error_count = ?1 WHERE id = ?2",
        params![count as i64, file_id],
    )?;
    Ok(())
}

/// Insert a single symbol record and return its row id.
// TODO(P3): Refactor into a SymbolInsert builder struct to reduce argument count.
#[allow(clippy::too_many_arguments)]
pub fn insert_symbol(
    db: &Database,
    file_id: i64,
    name: &str,
    qualified_name: &str,
    kind: &str,
    line_start: u32,
    line_end: u32,
    is_exported: bool,
    is_test: bool,
    signature: &str,
    decorators: &str,
    parent_symbol_id: Option<i64>,
) -> anyhow::Result<i64> {
    db.conn().execute(
        "INSERT INTO symbols (file_id, name, qualified_name, kind, line_start, line_end, is_exported, is_test, signature, decorators, parent_symbol_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            file_id,
            name,
            qualified_name,
            kind,
            line_start,
            line_end,
            is_exported,
            is_test,
            signature,
            decorators,
            parent_symbol_id,
        ],
    )?;
    let symbol_id = db.conn().last_insert_rowid();

    // Also insert into the FTS5 index
    db.conn().execute(
        "INSERT INTO symbols_fts (rowid, name, qualified_name, signature) VALUES (?1, ?2, ?3, ?4)",
        params![symbol_id, name, qualified_name, signature],
    )?;

    Ok(symbol_id)
}

/// Batch-insert parsed symbols for a file.
pub fn insert_symbols_batch(
    db: &Database,
    file_id: i64,
    symbols: &[ParsedSymbol],
) -> anyhow::Result<()> {
    for sym in symbols {
        let decorators_json = serde_json::to_string(&sym.decorators).unwrap_or_default();
        // A symbol is a test if its decorators include "test" (set by language parsers
        // for #[test] functions and symbols inside #[cfg(test)] modules).
        let is_test = sym.decorators.iter().any(|d| d == "test");
        insert_symbol(
            db,
            file_id,
            &sym.name,
            &sym.qualified_name,
            &sym.kind.to_string(),
            sym.line_start,
            sym.line_end,
            sym.is_exported,
            is_test,
            &sym.signature,
            &decorators_json,
            None,
        )?;
    }
    Ok(())
}

/// Batch-insert parsed imports for a file.
pub fn insert_imports_batch(
    db: &Database,
    file_id: i64,
    imports: &[ParsedImport],
) -> anyhow::Result<()> {
    let mut stmt = db.conn().prepare(
        "INSERT INTO imports (file_id, imported_name, module_path, line, is_external, original_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;

    for imp in imports {
        stmt.execute(params![
            file_id,
            imp.imported_name,
            imp.module_path,
            imp.line,
            imp.is_external,
            imp.original_name,
        ])?;
    }

    Ok(())
}

/// Batch-insert parsed calls for a file.
///
/// `caller_id` is the symbol ID of the enclosing function.
pub fn insert_calls_batch(
    db: &Database,
    file_id: i64,
    caller_id: i64,
    calls: &[ParsedCall],
) -> anyhow::Result<()> {
    let mut stmt = db.conn().prepare(
        "INSERT INTO calls (caller_symbol_id, callee_name, file_id, line, confidence, resolution)
         VALUES (?1, ?2, ?3, ?4, 0.5, 'unresolved')",
    )?;

    for call in calls {
        stmt.execute(params![caller_id, call.callee_name, file_id, call.line,])?;
    }

    Ok(())
}

/// Insert an API endpoint record.
pub fn insert_api_endpoint(
    db: &Database,
    service_id: i64,
    method: &str,
    path_pattern: &str,
    handler_symbol_id: Option<i64>,
    file_id: Option<i64>,
    line: Option<u32>,
) -> anyhow::Result<i64> {
    db.conn().execute(
        "INSERT OR IGNORE INTO api_endpoints (service_id, method, path_pattern, handler_symbol_id, file_id, line)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![service_id, method, path_pattern, handler_symbol_id, file_id, line],
    )?;
    Ok(db.conn().last_insert_rowid())
}

/// Insert an API call site record.
// TODO(P3): Refactor into an ApiCallInsert builder struct to reduce argument count.
#[allow(clippy::too_many_arguments)]
pub fn insert_api_call(
    db: &Database,
    service_id: i64,
    method: &str,
    url_pattern: &str,
    caller_symbol_id: Option<i64>,
    file_id: Option<i64>,
    line: Option<u32>,
    is_dynamic: bool,
) -> anyhow::Result<i64> {
    db.conn().execute(
        "INSERT INTO api_calls (service_id, method, url_pattern, caller_symbol_id, file_id, line, is_dynamic)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![service_id, method, url_pattern, caller_symbol_id, file_id, line, is_dynamic],
    )?;
    Ok(db.conn().last_insert_rowid())
}

/// Insert a coupling record between two files.
pub fn insert_coupling(
    db: &Database,
    file_a_id: i64,
    file_b_id: i64,
    co_changes: i32,
    strength: f64,
) -> anyhow::Result<()> {
    db.conn().execute(
        "INSERT OR REPLACE INTO coupling (file_a_id, file_b_id, co_changes, strength)
         VALUES (?1, ?2, ?3, ?4)",
        params![file_a_id, file_b_id, co_changes, strength],
    )?;
    Ok(())
}

/// Insert a heritage (inheritance/implementation) record.
pub fn insert_heritage(
    db: &Database,
    child_symbol_id: i64,
    parent_name: &str,
    kind: &str,
) -> anyhow::Result<()> {
    db.conn().execute(
        "INSERT INTO heritage (child_symbol_id, parent_name, kind)
         VALUES (?1, ?2, ?3)",
        params![child_symbol_id, parent_name, kind],
    )?;
    Ok(())
}

/// Insert a community record.
pub fn insert_community(
    db: &Database,
    name: &str,
    symbol_count: i32,
    internal_edges: i32,
    external_edges: i32,
    modularity: f64,
) -> anyhow::Result<i64> {
    db.conn().execute(
        "INSERT INTO communities (name, symbol_count, internal_edges, external_edges, modularity)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            name,
            symbol_count,
            internal_edges,
            external_edges,
            modularity
        ],
    )?;
    Ok(db.conn().last_insert_rowid())
}

/// Insert a service edge.
pub fn insert_service_edge(
    db: &Database,
    from_service_id: i64,
    to_service_id: i64,
    protocol: &str,
    call_count: i32,
    confidence: f64,
) -> anyhow::Result<()> {
    db.conn().execute(
        "INSERT OR REPLACE INTO service_edges (from_service_id, to_service_id, protocol, call_count, confidence)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![from_service_id, to_service_id, protocol, call_count, confidence],
    )?;
    Ok(())
}

/// Insert or replace a symbol history record (git blame aggregate).
pub fn insert_symbol_history(
    db: &Database,
    symbol_id: i64,
    created_at: Option<i64>,
    last_modified_at: Option<i64>,
    modification_count: i32,
    author_count: i32,
    is_volatile: bool,
) -> anyhow::Result<()> {
    db.conn().execute(
        "INSERT OR REPLACE INTO symbol_history
         (symbol_id, created_at, last_modified_at, modification_count, author_count, is_volatile)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            symbol_id,
            created_at,
            last_modified_at,
            modification_count,
            author_count,
            is_volatile
        ],
    )?;
    Ok(())
}

/// Clear all data from the database in FK-safe order for re-indexing.
pub fn clear_all_data(db: &Database) -> anyhow::Result<()> {
    // Delete in dependency order: children before parents
    db.conn().execute_batch(
        "DELETE FROM symbol_history;
         DELETE FROM flow_steps;
         DELETE FROM flows;
         DELETE FROM rule_violations;
         DELETE FROM service_edges;
         DELETE FROM api_calls;
         DELETE FROM api_endpoints;
         DELETE FROM coupling;
         DELETE FROM heritage;
         DELETE FROM calls;
         DELETE FROM imports;
         DELETE FROM symbols_fts;
         DELETE FROM symbols;
         DELETE FROM communities;
         DELETE FROM files;
         DELETE FROM services;
         DELETE FROM metadata;",
    )?;
    Ok(())
}

/// Delete all data for a given file (symbols, calls, imports).
pub fn delete_file_data(db: &Database, file_id: i64) -> anyhow::Result<()> {
    db.conn()
        .execute("DELETE FROM calls WHERE file_id = ?1", params![file_id])?;
    db.conn()
        .execute("DELETE FROM imports WHERE file_id = ?1", params![file_id])?;
    db.conn().execute(
        "DELETE FROM symbols_fts WHERE rowid IN (SELECT id FROM symbols WHERE file_id = ?1)",
        params![file_id],
    )?;
    db.conn()
        .execute("DELETE FROM symbols WHERE file_id = ?1", params![file_id])?;
    db.conn()
        .execute("DELETE FROM files WHERE id = ?1", params![file_id])?;
    Ok(())
}
