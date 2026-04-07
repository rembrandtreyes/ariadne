use super::{CallGraph, SymbolNode};
use petgraph::graph::NodeIndex;
use petgraph::Direction;
use std::collections::{HashMap, HashSet, VecDeque};

/// Get all symbols that depend on the given symbol (upstream callers).
pub fn get_dependents(
    graph: &CallGraph,
    symbol_id: i64,
    max_depth: Option<u32>,
) -> Vec<&SymbolNode> {
    let Some(&start) = graph.symbol_index.get(&symbol_id) else {
        return Vec::new();
    };

    let mut results = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((start, 0u32));
    visited.insert(start);

    while let Some((node, depth)) = queue.pop_front() {
        if node != start {
            results.push(&graph.graph[node]);
        }
        if max_depth.is_some_and(|max| depth >= max) {
            continue;
        }
        for neighbor in graph.graph.neighbors_directed(node, Direction::Incoming) {
            if visited.insert(neighbor) {
                queue.push_back((neighbor, depth + 1));
            }
        }
    }

    results
}

/// Get all symbols that the given symbol depends on (downstream callees).
pub fn get_dependencies(
    graph: &CallGraph,
    symbol_id: i64,
    max_depth: Option<u32>,
) -> Vec<&SymbolNode> {
    let Some(&start) = graph.symbol_index.get(&symbol_id) else {
        return Vec::new();
    };

    let mut results = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((start, 0u32));
    visited.insert(start);

    while let Some((node, depth)) = queue.pop_front() {
        if node != start {
            results.push(&graph.graph[node]);
        }
        if max_depth.is_some_and(|max| depth >= max) {
            continue;
        }
        for neighbor in graph.graph.neighbors_directed(node, Direction::Outgoing) {
            if visited.insert(neighbor) {
                queue.push_back((neighbor, depth + 1));
            }
        }
    }

    results
}

/// Find the shortest directed path from `from_id` to `to_id` in the call graph.
///
/// Returns an ordered `Vec` of symbol IDs from `from_id` to `to_id` (inclusive),
/// or `None` if either symbol is absent or no directed path exists.
pub fn find_shortest_path(graph: &CallGraph, from_id: i64, to_id: i64) -> Option<Vec<i64>> {
    let &from_idx = graph.symbol_index.get(&from_id)?;
    let &to_idx = graph.symbol_index.get(&to_id)?;

    if from_idx == to_idx {
        return Some(vec![from_id]);
    }

    // BFS with parent-pointer tracking for path reconstruction.
    let mut parent: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut queue: VecDeque<NodeIndex> = VecDeque::new();

    queue.push_back(from_idx);
    visited.insert(from_idx);

    while let Some(node) = queue.pop_front() {
        for neighbor in graph.graph.neighbors_directed(node, Direction::Outgoing) {
            if visited.insert(neighbor) {
                parent.insert(neighbor, node);
                if neighbor == to_idx {
                    // Reconstruct path by walking parent pointers back to source.
                    let mut path = Vec::new();
                    let mut cur = to_idx;
                    while cur != from_idx {
                        path.push(graph.graph[cur].id);
                        cur = parent[&cur];
                    }
                    path.push(from_id);
                    path.reverse();
                    return Some(path);
                }
                queue.push_back(neighbor);
            }
        }
    }

    None
}
