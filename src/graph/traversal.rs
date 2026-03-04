use super::{CallGraph, SymbolNode};
use petgraph::Direction;
use std::collections::{HashSet, VecDeque};

/// Get all symbols that depend on the given symbol (upstream callers).
pub fn get_dependents(graph: &CallGraph, symbol_id: i64, max_depth: Option<u32>) -> Vec<&SymbolNode> {
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
pub fn get_dependencies(graph: &CallGraph, symbol_id: i64, max_depth: Option<u32>) -> Vec<&SymbolNode> {
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
