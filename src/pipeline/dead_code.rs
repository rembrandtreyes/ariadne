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

    // Mark common web framework handler patterns as entry points.
    // These are registered via router macros (.route(), .get(), etc.) which
    // don't create call edges but ARE reachable at runtime.
    conn.execute(
        "UPDATE symbols SET is_entry_point = 1 WHERE name LIKE '%_handler'
         OR name LIKE 'handle_%' OR name LIKE '%Handler'",
        [],
    )?;

    // Mark JS/TS class constructors as entry points (called via `new`)
    conn.execute(
        "UPDATE symbols SET is_entry_point = 1 WHERE name = 'constructor'",
        [],
    )?;

    // Mark class methods as entry points — they're called via instance
    // references (obj.method()) which static analysis can't trace without
    // type inference. Uses qualified_name prefix matching against known classes.
    conn.execute_batch(
        "UPDATE symbols SET is_entry_point = 1
         WHERE kind = 'method' AND EXISTS (
             SELECT 1 FROM symbols c
             WHERE c.kind = 'class'
               AND symbols.qualified_name LIKE c.name || '.%'
         )",
    )?;

    // Collect all reachable symbol IDs via BFS from entry points and exported symbols
    let mut reachable = HashSet::new();

    // Seed: entry points, exported symbols, test symbols, and trait impl methods.
    // Trait implementations (e.g. Display::fmt, From::from, Default::default) are
    // satisfied through the trait's dispatch contract — they are always "reachable"
    // even when no direct call site appears in the codebase.
    let mut seed_stmt = conn.prepare(
        "SELECT id FROM symbols WHERE is_entry_point = 1 OR is_exported = 1 OR is_test = 1
         OR decorators LIKE '%trait_impl%'",
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
    let mut all_funcs_stmt = conn
        .prepare("SELECT id FROM symbols WHERE kind IN ('function', 'method') AND is_test = 0")?;
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
