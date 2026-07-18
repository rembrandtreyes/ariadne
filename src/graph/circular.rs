use super::CallGraph;
use petgraph::algo::kosaraju_scc;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CircularDependency {
    pub symbols: Vec<String>,
    pub cycle_length: usize,
}

pub fn detect_circular_dependencies(graph: &CallGraph) -> Vec<CircularDependency> {
    let sccs = kosaraju_scc(&graph.graph);

    // Canonical order: kosaraju emits SCCs in NodeIndex order, which is not
    // stable across graph builds — sort members and cycles for identical output.
    let mut cycles: Vec<CircularDependency> = sccs
        .into_iter()
        .filter(|scc| scc.len() > 1)
        .map(|scc| {
            let mut symbols: Vec<String> = scc
                .iter()
                .map(|&idx| graph.graph[idx].qualified_name.clone())
                .collect();
            symbols.sort_unstable();
            let len = symbols.len();
            CircularDependency {
                symbols,
                cycle_length: len,
            }
        })
        .collect();
    cycles.sort_by(|a, b| a.symbols.cmp(&b.symbols));
    cycles
}
