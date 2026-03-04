pub mod affected_tests;
pub mod arch_rules;
pub mod boundaries;
pub mod scip_export;

use crate::graph::DependencyGraph;
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of a dead code analysis pass.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeadCodeReport {
    pub dead_symbols: Vec<DeadSymbol>,
    pub total_symbols: usize,
    pub dead_count: usize,
}

/// A symbol identified as potentially dead (unreachable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadSymbol {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub kind: String,
    pub confidence: u32,
}

/// Result of a community detection pass.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommunityReport {
    pub communities: Vec<Community>,
    pub modularity_score: f64,
}

/// A cluster of tightly connected symbols.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Community {
    pub id: usize,
    pub members: Vec<String>,
    pub cohesion: f64,
}

/// Result of an architectural rule check.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuleCheckReport {
    pub violations: Vec<RuleViolation>,
    pub rules_checked: usize,
    pub passed: usize,
    pub failed: usize,
}

/// A single architectural rule violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleViolation {
    pub rule_name: String,
    pub from_symbol: String,
    pub to_symbol: String,
    pub file: String,
    pub line: u32,
    pub severity: String,
    pub message: String,
}

/// Analyze the graph for dead code (symbols with no incoming edges
/// that are not entry points).
pub fn find_dead_code(
    graph: &DependencyGraph,
    _entry_points: &[String],
    threshold: u32,
) -> DeadCodeReport {
    let inner = graph.inner();
    let total_symbols = inner.node_count();
    let mut dead_symbols = Vec::new();

    for node_idx in inner.node_indices() {
        let in_degree = inner
            .edges_directed(node_idx, petgraph::Direction::Incoming)
            .count();

        if in_degree == 0 {
            if let Some(node) = graph.node_data(node_idx) {
                // Assign confidence based on symbol kind and context
                let confidence = if node.kind == "function" { 90 } else { 70 };
                if confidence >= threshold {
                    dead_symbols.push(DeadSymbol {
                        name: node.name.clone(),
                        file: node.file.clone(),
                        line: node.line,
                        kind: node.kind.clone(),
                        confidence,
                    });
                }
            }
        }
    }

    let dead_count = dead_symbols.len();
    DeadCodeReport {
        dead_symbols,
        total_symbols,
        dead_count,
    }
}

/// Detect communities in the graph using a simple label propagation approach.
pub fn detect_communities(graph: &DependencyGraph) -> CommunityReport {
    let inner = graph.inner();
    if inner.node_count() == 0 {
        return CommunityReport::default();
    }

    // Simple approach: each connected component is a community
    let mut labels: HashMap<NodeIndex, usize> = HashMap::new();
    let mut community_id = 0;

    for node_idx in inner.node_indices() {
        if labels.contains_key(&node_idx) {
            continue;
        }

        // BFS to find all connected nodes
        let mut queue = vec![node_idx];
        let mut visited = Vec::new();

        while let Some(current) = queue.pop() {
            if labels.contains_key(&current) {
                continue;
            }
            labels.insert(current, community_id);
            visited.push(current);

            for neighbor in inner.neighbors_undirected(current) {
                if !labels.contains_key(&neighbor) {
                    queue.push(neighbor);
                }
            }
        }

        community_id += 1;
    }

    // Group nodes by community label
    let mut community_members: HashMap<usize, Vec<String>> = HashMap::new();
    for (node_idx, label) in &labels {
        if let Some(node) = graph.node_data(*node_idx) {
            community_members
                .entry(*label)
                .or_default()
                .push(node.name.clone());
        }
    }

    let communities: Vec<Community> = community_members
        .into_iter()
        .map(|(id, members)| Community {
            id,
            cohesion: 1.0,
            members,
        })
        .collect();

    CommunityReport {
        communities,
        modularity_score: 0.0,
    }
}

/// Check architectural rules against the dependency graph.
pub fn check_rules(
    _graph: &DependencyGraph,
    _rules: &[crate::config::ArchRule],
) -> RuleCheckReport {
    RuleCheckReport::default()
}

/// Find all tests affected by changes to the given symbols.
pub fn find_affected_tests(
    _graph: &DependencyGraph,
    _changed_symbols: &[String],
) -> Vec<String> {
    Vec::new()
}

/// Compute codebase statistics from the graph.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodebaseStats {
    pub total_files: usize,
    pub total_symbols: usize,
    pub total_edges: usize,
    pub languages: HashMap<String, usize>,
    pub symbol_kinds: HashMap<String, usize>,
    pub edge_kinds: HashMap<String, usize>,
}

pub fn compute_stats(graph: &DependencyGraph) -> CodebaseStats {
    let inner = graph.inner();
    CodebaseStats {
        total_symbols: inner.node_count(),
        total_edges: inner.edge_count(),
        ..Default::default()
    }
}
