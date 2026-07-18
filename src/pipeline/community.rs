//! Pipeline Phase 14: Community Detection
//!
//! Reads: `calls` (resolved caller/callee pairs), `symbols`
//! Writes: `communities` (detected groups with modularity scores), `symbols` (updates `community_id`)
//!
//! Groups symbols into communities using BFS-based connected component analysis
//! on the resolved call graph, computing internal/external edge ratios and modularity.

use crate::db::Database;
use rusqlite::params;
use std::collections::{HashMap, HashSet, VecDeque};

/// Phase 14: Detect module communities using connected component analysis.
///
/// Groups symbols into communities based on call relationships.
/// Uses BFS-based connected component detection on the resolved call graph.
pub fn detect_communities(db: &Database) -> anyhow::Result<()> {
    let conn = db.conn();

    // Clear existing community data
    conn.execute("DELETE FROM communities", [])?;
    conn.execute("UPDATE symbols SET community_id = NULL", [])?;

    // Build an adjacency list from resolved calls
    let mut adjacency: HashMap<i64, HashSet<i64>> = HashMap::new();

    let mut stmt = conn.prepare(
        "SELECT caller_symbol_id, callee_symbol_id FROM calls
         WHERE caller_symbol_id IS NOT NULL AND callee_symbol_id IS NOT NULL",
    )?;

    let call_edges: Vec<(i64, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    for (caller, callee) in &call_edges {
        adjacency.entry(*caller).or_default().insert(*callee);
        adjacency.entry(*callee).or_default().insert(*caller);
    }

    // BFS to find connected components
    let mut visited: HashSet<i64> = HashSet::new();
    let mut community_id = 0i64;

    // Sorted seed order: community numbering must not depend on HashMap
    // iteration order, or IDs differ across runs on identical input.
    let mut all_symbols: Vec<i64> = adjacency.keys().copied().collect();
    all_symbols.sort_unstable();

    for seed in &all_symbols {
        if visited.contains(seed) {
            continue;
        }

        let mut component = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(*seed);

        while let Some(sym_id) = queue.pop_front() {
            if !visited.insert(sym_id) {
                continue;
            }
            component.push(sym_id);

            if let Some(neighbors) = adjacency.get(&sym_id) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        if component.is_empty() {
            continue;
        }

        // Count internal and external edges for this community
        let component_set: HashSet<i64> = component.iter().copied().collect();
        let mut internal_edges = 0i32;
        let mut external_edges = 0i32;

        for &sym_id in &component {
            if let Some(neighbors) = adjacency.get(&sym_id) {
                for &neighbor in neighbors {
                    if component_set.contains(&neighbor) {
                        internal_edges += 1;
                    } else {
                        external_edges += 1;
                    }
                }
            }
        }

        // Internal edges are double-counted (bidirectional adjacency)
        internal_edges /= 2;

        let symbol_count = component.len() as i32;
        let total_edges = (internal_edges + external_edges).max(1) as f64;
        let modularity = internal_edges as f64 / total_edges;

        let comm_id = crate::db::write::insert_community(
            db,
            &format!("community_{}", community_id),
            symbol_count,
            internal_edges,
            external_edges,
            modularity,
        )?;

        // Assign community_id to all symbols in this community
        for &sym_id in &component {
            conn.execute(
                "UPDATE symbols SET community_id = ?1 WHERE id = ?2",
                params![comm_id, sym_id],
            )?;
        }

        community_id += 1;
    }

    Ok(())
}
