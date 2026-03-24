//! Pipeline Phase 8: Execution Flow Tracing
//!
//! Reads: `symbols` (entry points), `calls` (resolved call graph)
//! Writes: `flows`, `flow_steps` (complete execution paths from entry points)
//!
//! Performs BFS traversal from each entry-point symbol through the resolved
//! call graph, recording ordered execution flow steps with depth tracking.

use crate::db::Database;
use rusqlite::params;

/// Phase 8: Trace execution flows from entry points.
///
/// Follows the call graph from entry point symbols to build
/// complete execution flow paths through the codebase.
pub fn trace_flows(db: &Database) -> anyhow::Result<()> {
    let conn = db.conn();

    // Find all entry point symbols
    let mut entry_stmt = conn.prepare("SELECT id, name FROM symbols WHERE is_entry_point = 1")?;

    let entries: Vec<(i64, String)> = entry_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    for (entry_id, entry_name) in &entries {
        // Insert a flow record for each entry point
        conn.execute(
            "INSERT INTO flows (entry_symbol_id, name) VALUES (?1, ?2)",
            params![entry_id, format!("flow:{}", entry_name)],
        )?;

        let flow_id = conn.last_insert_rowid();

        // BFS to trace the call chain from the entry point
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((*entry_id, 0i32, 0i32));
        let mut step_order = 0i32;

        while let Some((sym_id, depth, _)) = queue.pop_front() {
            if !visited.insert(sym_id) {
                continue;
            }

            conn.execute(
                "INSERT INTO flow_steps (flow_id, symbol_id, step_order, depth) VALUES (?1, ?2, ?3, ?4)",
                params![flow_id, sym_id, step_order, depth],
            )?;
            step_order += 1;

            // Find all callees of this symbol
            let mut callee_stmt = conn.prepare(
                "SELECT DISTINCT callee_symbol_id FROM calls WHERE caller_symbol_id = ?1 AND callee_symbol_id IS NOT NULL",
            )?;
            let callees: Vec<i64> = callee_stmt
                .query_map(params![sym_id], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;

            for callee_id in callees {
                if !visited.contains(&callee_id) {
                    queue.push_back((callee_id, depth + 1, step_order));
                }
            }
        }
    }

    Ok(())
}
