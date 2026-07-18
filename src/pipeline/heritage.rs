//! Pipeline Phase 5: Inheritance Heritage Resolution
//!
//! Reads: `heritage` (unresolved parent names), `symbols` (classes, interfaces, traits)
//! Writes: `heritage` (updates `parent_symbol_id` for resolved entries)
//!
//! Resolves parent-child inheritance relationships by matching unresolved
//! parent names in the heritage table to known class/interface/trait symbols.

use crate::db::Database;
use rusqlite::params;

/// Phase 5: Build the class/type inheritance hierarchy.
///
/// Scans symbols for classes and interfaces, then resolves
/// parent-child relationships based on naming conventions.
pub fn build_heritage(db: &Database) -> anyhow::Result<()> {
    let conn = db.conn();

    // Resolve heritage records where parent_symbol_id is NULL
    let mut stmt =
        conn.prepare("SELECT id, parent_name FROM heritage WHERE parent_symbol_id IS NULL")?;

    let unresolved: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    for (heritage_id, parent_name) in &unresolved {
        // Try to find a matching symbol for the parent name
        let parent_sym: Option<i64> = conn
            .query_row(
                "SELECT id FROM symbols WHERE name = ?1 AND kind IN ('class', 'interface', 'trait') LIMIT 1",
                params![parent_name],
                |row| row.get(0),
            )
            .ok();

        if let Some(pid) = parent_sym {
            conn.execute(
                "UPDATE heritage SET parent_symbol_id = ?1 WHERE id = ?2",
                params![pid, heritage_id],
            )?;
        }
    }

    Ok(())
}
