use crate::db::Database;

/// Phase 10: Build the FTS5 search index for fast symbol lookup.
///
/// Rebuilds the symbols_fts virtual table to ensure it is in sync
/// with the symbols table after all inserts and updates.
pub fn build_search_index(db: &Database) -> anyhow::Result<()> {
    let conn = db.conn();

    // Rebuild the FTS5 index from scratch
    // First, delete all existing FTS entries
    conn.execute_batch("DELETE FROM symbols_fts;")?;

    // Re-populate from the symbols table
    conn.execute_batch(
        "INSERT INTO symbols_fts (rowid, name, qualified_name, signature)
         SELECT id, name, qualified_name, signature FROM symbols;",
    )?;

    Ok(())
}
