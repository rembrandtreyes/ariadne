use super::CallGraph;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Result of analyzing the blast radius of changing a symbol.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlastRadiusResult {
    /// Total number of symbols affected (direct + transitive).
    pub total_affected: usize,
    /// Direct dependents of the target symbol.
    pub direct_dependents: Vec<AffectedSymbol>,
    /// Transitive dependents (indirect callers).
    pub transitive_dependents: Vec<AffectedSymbol>,
    /// Affected files (unique file paths).
    pub affected_files: Vec<String>,
    /// Affected services (for cross-service analysis).
    pub affected_services: Vec<String>,
}

/// A symbol affected by a change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedSymbol {
    pub id: i64,
    pub name: String,
    pub file_path: String,
    pub kind: String,
    pub depth: u32,
}

/// Analyze the blast radius of changing a given symbol in the call graph.
///
/// Traverses the reverse call graph to find all symbols that directly
/// or transitively depend on the target symbol.
pub fn analyze_blast_radius(
    graph: &CallGraph,
    target_id: u64,
    max_depth: Option<u32>,
    _cross_service: bool,
) -> BlastRadiusResult {
    let effective_max_depth = max_depth.unwrap_or(10);

    // Find the target node index
    let target_idx = match graph.find_node(target_id) {
        Some(idx) => idx,
        None => return BlastRadiusResult::default(),
    };

    let mut visited: HashSet<usize> = HashSet::new();
    let mut direct = Vec::new();
    let mut transitive = Vec::new();
    let mut file_set: HashSet<String> = HashSet::new();

    // BFS traversal of reverse edges (callers of the target)
    let mut frontier: Vec<(usize, u32)> = vec![(target_idx, 0)];
    visited.insert(target_idx);

    while let Some((node_idx, depth)) = frontier.pop() {
        if depth > effective_max_depth {
            continue;
        }

        // Find all callers of this node (reverse edges)
        for caller_idx in graph.callers_of(node_idx) {
            if visited.insert(caller_idx) {
                if let Some(caller_node) = graph.get_symbol(caller_idx) {
                    let affected = AffectedSymbol {
                        id: caller_node.id,
                        name: caller_node.name.clone(),
                        file_path: caller_node.file_path.clone(),
                        kind: caller_node.kind.clone(),
                        depth: depth + 1,
                    };
                    file_set.insert(caller_node.file_path.clone());

                    if depth == 0 {
                        direct.push(affected);
                    } else {
                        transitive.push(affected);
                    }

                    frontier.push((caller_idx, depth + 1));
                }
            }
        }
    }

    let total_affected = direct.len() + transitive.len();
    // Sorted: set iteration order must never reach serialized output.
    let mut affected_files: Vec<String> = file_set.into_iter().collect();
    affected_files.sort_unstable();

    BlastRadiusResult {
        total_affected,
        direct_dependents: direct,
        transitive_dependents: transitive,
        affected_files,
        affected_services: Vec::new(),
    }
}

/// Enhanced blast radius that includes cross-service impact via service topology.
///
/// After computing the local call-graph blast radius, queries the database
/// for service edges to determine which other services might be affected.
pub fn analyze_blast_radius_cross_service(
    graph: &CallGraph,
    db: &crate::db::Database,
    target_id: u64,
    max_depth: Option<u32>,
) -> BlastRadiusResult {
    let mut result = analyze_blast_radius(graph, target_id, max_depth, true);

    // Find the service of the target symbol
    let conn = db.conn();
    let target_service: Option<(i64, String)> = conn
        .query_row(
            "SELECT s2.id, s2.name FROM symbols s
             JOIN files f ON s.file_id = f.id
             JOIN services s2 ON f.service_id = s2.id
             WHERE s.id = ?1",
            rusqlite::params![target_id as i64],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    let Some((service_id, service_name)) = target_service else {
        return result;
    };

    // Find all services that depend on this service (reverse service edges)
    let mut affected_services: Vec<String> = Vec::new();
    let mut visited_services: HashSet<i64> = HashSet::new();
    visited_services.insert(service_id);

    let mut service_frontier = vec![service_id];
    while let Some(svc_id) = service_frontier.pop() {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT se.from_service_id, s.name
             FROM service_edges se
             JOIN services s ON se.from_service_id = s.id
             WHERE se.to_service_id = ?1",
        ) {
            if let Ok(deps) = stmt
                .query_map(rusqlite::params![svc_id], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })
                .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
            {
                for (dep_id, dep_name) in deps {
                    if visited_services.insert(dep_id) {
                        affected_services.push(dep_name);
                        service_frontier.push(dep_id);
                    }
                }
            }
        }
    }

    if !affected_services.is_empty() {
        // Prepend the origin service
        result.affected_services = std::iter::once(service_name)
            .chain(affected_services)
            .collect();
    }

    result
}
