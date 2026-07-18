use ariadne::graph::blast_radius::analyze_blast_radius;
use ariadne::graph::{CallEdge, CallGraph, SymbolNode};

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

    graph.add_call(
        2,
        1,
        CallEdge {
            confidence: 1.0,
            resolution: "import".to_string(),
            line: 10,
        },
    );

    let result = analyze_blast_radius(&graph, 1, None, false);
    assert_eq!(result.total_affected, 1);
}

/// affected_files must be sorted: HashSet iteration order must never reach output.
#[test]
fn test_blast_radius_affected_files_sorted_deterministic() {
    let mut graph = CallGraph::new();

    graph.add_symbol(SymbolNode {
        id: 1,
        name: "target".to_string(),
        qualified_name: "mod::target".to_string(),
        kind: "function".to_string(),
        file_path: "src/target.rs".to_string(),
        is_dead: false,
        is_test: false,
    });

    // 40 callers in 40 distinct files, inserted in reverse-alphabetical order
    for i in (0..40).rev() {
        graph.add_symbol(SymbolNode {
            id: 100 + i,
            name: format!("caller_{i:02}"),
            qualified_name: format!("mod::caller_{i:02}"),
            kind: "function".to_string(),
            file_path: format!("src/caller_{i:02}.rs"),
            is_dead: false,
            is_test: false,
        });
        graph.add_call(
            100 + i,
            1,
            CallEdge {
                confidence: 1.0,
                resolution: "import".to_string(),
                line: 10,
            },
        );
    }

    let result = analyze_blast_radius(&graph, 1, None, false);

    let expected: Vec<String> = (0..40).map(|i| format!("src/caller_{i:02}.rs")).collect();
    assert_eq!(
        result.affected_files, expected,
        "affected_files must be sorted lexicographically"
    );
}
