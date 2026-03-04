use ariadne_graph::graph::{CallGraph, SymbolNode, CallEdge};
use ariadne_graph::graph::blast_radius::analyze_blast_radius;

#[test]
fn test_empty_blast_radius() {
    let graph = CallGraph::new();
    let result = analyze_blast_radius(&graph, 1, None, false);
    assert_eq!(result.total_affected, 0);
}

#[test]
fn test_blast_radius_with_dependents() {
    let mut graph = CallGraph::new();

    graph.add_symbol(SymbolNode {
        id: 1,
        name: "target".to_string(),
        qualified_name: "mod::target".to_string(),
        kind: "function".to_string(),
        file_path: "src/lib.rs".to_string(),
        is_dead: false,
        is_test: false,
    });

    graph.add_symbol(SymbolNode {
        id: 2,
        name: "caller".to_string(),
        qualified_name: "mod::caller".to_string(),
        kind: "function".to_string(),
        file_path: "src/main.rs".to_string(),
        is_dead: false,
        is_test: false,
    });

    graph.add_call(2, 1, CallEdge {
        confidence: 1.0,
        resolution: "import".to_string(),
        line: 10,
    });

    let result = analyze_blast_radius(&graph, 1, None, false);
    assert_eq!(result.total_affected, 1);
}
