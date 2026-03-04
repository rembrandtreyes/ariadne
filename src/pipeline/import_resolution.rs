use crate::db::Database;
use rusqlite::params;

/// Phase 4: Resolve import statements to their target files and symbols.
///
/// For each import record, attempts to match the module_path to a known
/// file path or symbol qualified name in the database.
pub fn resolve_imports(db: &Database) -> anyhow::Result<()> {
    let conn = db.conn();

    // Get all unresolved imports
    let mut import_stmt = conn.prepare(
        "SELECT i.id, i.module_path, i.imported_name, i.file_id
         FROM imports i
         WHERE i.resolved_file_id IS NULL AND i.is_external = 0",
    )?;

    let imports: Vec<(i64, String, String, i64)> = import_stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (import_id, module_path, imported_name, _file_id) in &imports {
        // Try to resolve by matching the module path to a file path
        let resolved_file: Option<i64> = conn
            .query_row(
                "SELECT id FROM files WHERE path LIKE '%' || ?1 || '%' LIMIT 1",
                params![module_path],
                |row| row.get(0),
            )
            .ok();

        if let Some(fid) = resolved_file {
            conn.execute(
                "UPDATE imports SET resolved_file_id = ?1 WHERE id = ?2",
                params![fid, import_id],
            )?;

            // Try to resolve the specific symbol within that file
            let resolved_sym: Option<i64> = conn
                .query_row(
                    "SELECT id FROM symbols WHERE file_id = ?1 AND name = ?2 LIMIT 1",
                    params![fid, imported_name],
                    |row| row.get(0),
                )
                .ok();

            if let Some(sid) = resolved_sym {
                conn.execute(
                    "UPDATE imports SET resolved_symbol_id = ?1 WHERE id = ?2",
                    params![sid, import_id],
                )?;
            }
        }
    }

    Ok(())
}
