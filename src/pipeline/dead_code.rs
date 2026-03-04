use crate::db::Database;
use rusqlite::params;
use std::collections::HashSet;

/// Phase 7: Multi-pass reachability analysis to detect dead code.
///
/// Marks symbols as dead if they are:
/// 1. Not entry points
/// 2. Not called by any other symbol
/// 3. Not exported
/// 4. Not tests
pub fn detect_dead_code(db: &Database) -> anyhow::Result<()> {
    let conn = db.conn();

    // Reset all dead flags
    conn.execute("UPDATE symbols SET is_dead = 0", [])?;

    // Mark entry points (main functions, handlers, etc.)
    conn.execute(
        "UPDATE symbols SET is_entry_point = 1 WHERE name IN ('main', 'Main', 'run', 'start', 'init')",
        [],
    )?;

    // Collect all reachable symbol IDs via BFS from entry points and exported symbols
    let mut reachable = HashSet::new();

    // Seed: entry points and exported symbols
    let mut seed_stmt = conn.prepare(
        "SELECT id FROM symbols WHERE is_entry_point = 1 OR is_exported = 1 OR is_test = 1",
    )?;
    let seeds: Vec<i64> = seed_stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut queue: Vec<i64> = seeds;
    while let Some(sym_id) = queue.pop() {
        if reachable.contains(&sym_id) {
            continue;
        }
        reachable.insert(sym_id);

        // Find all symbols called by this symbol
        let mut callee_stmt = conn.prepare(
            "SELECT DISTINCT callee_symbol_id FROM calls WHERE caller_symbol_id = ?1 AND callee_symbol_id IS NOT NULL",
        )?;
        let callees: Vec<i64> = callee_stmt
            .query_map(params![sym_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        for callee_id in callees {
            if !reachable.contains(&callee_id) {
                queue.push(callee_id);
            }
        }
    }

    // Mark unreachable functions and methods as dead
    let mut all_funcs_stmt = conn.prepare(
        "SELECT id FROM symbols WHERE kind IN ('function', 'method') AND is_test = 0",
    )?;
    let all_funcs: Vec<i64> = all_funcs_stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    for func_id in all_funcs {
        if !reachable.contains(&func_id) {
            conn.execute(
                "UPDATE symbols SET is_dead = 1 WHERE id = ?1",
                params![func_id],
            )?;
        }
    }

    Ok(())
}
