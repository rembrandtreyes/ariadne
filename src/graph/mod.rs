pub mod traversal;
pub mod blast_radius;
pub mod call_chain;
pub mod circular;

use std::collections::{HashMap, HashSet};

use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CallGraph — used by the traversal / blast-radius / call-chain / circular
// sub-modules that operate on the resolved call graph.
// ---------------------------------------------------------------------------

/// The in-memory call graph built from resolved edges in the database.
pub struct CallGraph {
    pub graph: DiGraph<SymbolNode, CallEdge>,
    pub symbol_index: HashMap<i64, NodeIndex>,
}

#[derive(Debug, Clone)]
pub struct SymbolNode {
    pub id: i64,
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub file_path: String,
    pub is_dead: bool,
    pub is_test: bool,
}

#[derive(Debug, Clone)]
pub struct CallEdge {
    pub confidence: f64,
    pub resolution: String,
    pub line: u32,
}

impl CallGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            symbol_index: HashMap::new(),
        }
    }

    pub fn add_symbol(&mut self, node: SymbolNode) -> NodeIndex {
        let id = node.id;
        let idx = self.graph.add_node(node);
        self.symbol_index.insert(id, idx);
        idx
    }

    pub fn add_call(&mut self, from_id: i64, to_id: i64, edge: CallEdge) {
        if let (Some(&from_idx), Some(&to_idx)) =
            (self.symbol_index.get(&from_id), self.symbol_index.get(&to_id))
        {
            self.graph.add_edge(from_idx, to_idx, edge);
        }
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Find a node index by its symbol id (u64 variant for blast-radius compat).
    pub fn find_node(&self, symbol_id: u64) -> Option<usize> {
        self.symbol_index
            .get(&(symbol_id as i64))
            .map(|idx| idx.index())
    }

    /// Get all caller node indexes (incoming neighbors) for a given node index.
    pub fn callers_of(&self, node_idx: usize) -> Vec<usize> {
        let idx = NodeIndex::new(node_idx);
        self.graph
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .map(|n| n.index())
            .collect()
    }

    /// Get a reference to the symbol node at the given raw index.
    pub fn get_symbol(&self, node_idx: usize) -> Option<&SymbolNode> {
        self.graph.node_weight(NodeIndex::new(node_idx))
    }
}

impl Default for CallGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// DependencyGraph — lightweight petgraph wrapper used for generic graph ops.
// ---------------------------------------------------------------------------

/// A node in the dependency graph, representing a symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    /// Unique identifier (matches the database symbol id)
    pub id: u64,
    /// Fully qualified symbol name
    pub name: String,
    /// Symbol kind (function, class, method, etc.)
    pub kind: String,
    /// File path where the symbol is defined
    pub file: String,
    /// Starting line number
    pub line: u32,
}

/// An edge in the dependency graph, representing a relationship.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    /// The kind of relationship (calls, imports, extends, implements, etc.)
    pub kind: EdgeKind,
    /// Numeric weight for the edge (useful for ranking)
    pub weight: f64,
}

/// Types of edges in the dependency graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Calls,
    Imports,
    Extends,
    Implements,
    Uses,
    Contains,
    References,
}

/// The in-memory dependency graph, backed by petgraph.
pub struct DependencyGraph {
    graph: DiGraph<GraphNode, GraphEdge>,
}

impl DependencyGraph {
    /// Create a new empty dependency graph.
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
        }
    }

    /// Add a node to the graph and return its index.
    pub fn add_node(&mut self, node: GraphNode) -> NodeIndex {
        self.graph.add_node(node)
    }

    /// Add a directed edge between two nodes.
    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, edge: GraphEdge) {
        self.graph.add_edge(from, to, edge);
    }

    /// Return the total number of nodes.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Return the total number of edges.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Compute the blast radius of a node: all transitively reachable nodes.
    pub fn blast_radius(&self, start: NodeIndex, max_depth: u32) -> Vec<NodeIndex> {
        let mut visited = HashSet::new();
        let mut frontier = vec![(start, 0u32)];
        let mut result = Vec::new();

        while let Some((node, depth)) = frontier.pop() {
            if depth > max_depth {
                continue;
            }
            if !visited.insert(node) {
                continue;
            }
            if node != start {
                result.push(node);
            }
            for neighbor in self.graph.neighbors(node) {
                if !visited.contains(&neighbor) {
                    frontier.push((neighbor, depth + 1));
                }
            }
        }

        result
    }

    /// Get a reference to a node's data by index.
    pub fn node_data(&self, index: NodeIndex) -> Option<&GraphNode> {
        self.graph.node_weight(index)
    }

    /// Get a reference to the underlying petgraph.
    pub fn inner(&self) -> &DiGraph<GraphNode, GraphEdge> {
        &self.graph
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: u64, name: &str) -> GraphNode {
        GraphNode {
            id,
            name: name.into(),
            kind: "function".into(),
            file: format!("{name}.rs"),
            line: 1,
        }
    }

    #[test]
    fn empty_graph_has_zero_counts() {
        let g = DependencyGraph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn add_nodes_and_edges() {
        let mut g = DependencyGraph::new();
        let a = g.add_node(make_node(1, "a"));
        let b = g.add_node(make_node(2, "b"));
        g.add_edge(
            a,
            b,
            GraphEdge {
                kind: EdgeKind::Calls,
                weight: 1.0,
            },
        );
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.edge_count(), 1);
    }

    #[test]
    fn blast_radius_respects_depth() {
        let mut g = DependencyGraph::new();
        let a = g.add_node(make_node(1, "a"));
        let b = g.add_node(make_node(2, "b"));
        let c = g.add_node(make_node(3, "c"));
        g.add_edge(
            a,
            b,
            GraphEdge {
                kind: EdgeKind::Calls,
                weight: 1.0,
            },
        );
        g.add_edge(
            b,
            c,
            GraphEdge {
                kind: EdgeKind::Calls,
                weight: 1.0,
            },
        );

        // Depth 1 should only reach b, not c
        let radius = g.blast_radius(a, 1);
        assert_eq!(radius.len(), 1);

        // Depth 2 should reach both b and c
        let radius = g.blast_radius(a, 2);
        assert_eq!(radius.len(), 2);
    }

    #[test]
    fn node_data_returns_correct_info() {
        let mut g = DependencyGraph::new();
        let idx = g.add_node(make_node(42, "my_func"));
        let data = g.node_data(idx).expect("node should exist");
        assert_eq!(data.id, 42);
        assert_eq!(data.name, "my_func");
    }

    #[test]
    fn call_graph_add_and_lookup() {
        let mut cg = CallGraph::new();
        let node = SymbolNode {
            id: 1,
            name: "foo".into(),
            qualified_name: "mod::foo".into(),
            kind: "function".into(),
            file_path: "src/lib.rs".into(),
            is_dead: false,
            is_test: false,
        };
        let idx = cg.add_symbol(node);
        assert_eq!(cg.node_count(), 1);
        assert_eq!(cg.graph[idx].name, "foo");
    }
}
