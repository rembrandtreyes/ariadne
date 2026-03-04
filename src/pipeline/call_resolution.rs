use crate::db::Database;

/// Phase 5: Resolve call sites to their target symbols.
///
/// For each unresolved call record, attempts to match the callee_name
/// to a known symbol. Uses a multi-pass approach with decreasing
/// confidence levels.
pub fn resolve_calls(db: &Database) -> anyhow::Result<()> {
    let conn = db.conn();

    // Pass 1: Exact match on name within same file
    conn.execute_batch(
        "UPDATE calls SET callee_symbol_id = (
            SELECT s.id FROM symbols s
            WHERE s.name = calls.callee_name
              AND s.file_id = calls.file_id
            LIMIT 1
         ), confidence = 0.95, resolution = 'same_file'
         WHERE callee_symbol_id IS NULL
           AND EXISTS (
               SELECT 1 FROM symbols s
               WHERE s.name = calls.callee_name
                 AND s.file_id = calls.file_id
           )",
    )?;

    // Pass 2: Exact match on name within same service
    conn.execute_batch(
        "UPDATE calls SET callee_symbol_id = (
            SELECT s.id FROM symbols s
            JOIN files f ON s.file_id = f.id
            JOIN files cf ON calls.file_id = cf.id
            WHERE s.name = calls.callee_name
              AND f.service_id = cf.service_id
            LIMIT 1
         ), confidence = 0.75, resolution = 'same_service'
         WHERE callee_symbol_id IS NULL
           AND EXISTS (
               SELECT 1 FROM symbols s
               JOIN files f ON s.file_id = f.id
               JOIN files cf ON calls.file_id = cf.id
               WHERE s.name = calls.callee_name
                 AND f.service_id = cf.service_id
           )",
    )?;

    // Pass 3: Fuzzy match on name across all symbols
    conn.execute_batch(
        "UPDATE calls SET callee_symbol_id = (
            SELECT s.id FROM symbols s
            WHERE s.name = calls.callee_name
            LIMIT 1
         ), confidence = 0.5, resolution = 'global'
         WHERE callee_symbol_id IS NULL
           AND EXISTS (
               SELECT 1 FROM symbols s
               WHERE s.name = calls.callee_name
           )",
    )?;

    // Count unresolved
    let unresolved: i64 = conn.query_row(
        "SELECT COUNT(*) FROM calls WHERE callee_symbol_id IS NULL",
        [],
        |row| row.get(0),
    )?;

    if unresolved > 0 {
        eprintln!("Call resolution: {} calls remain unresolved", unresolved);
    }

    Ok(())
}
