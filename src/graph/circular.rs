use super::CallGraph;
use petgraph::algo::kosaraju_scc;

#[derive(Debug)]
pub struct CircularDependency {
    pub symbols: Vec<String>,
    pub cycle_length: usize,
}

pub fn detect_circular_dependencies(graph: &CallGraph) -> Vec<CircularDependency> {
    let sccs = kosaraju_scc(&graph.graph);

    sccs.into_iter()
        .filter(|scc| scc.len() > 1)
        .map(|scc| {
            let symbols: Vec<String> = scc
                .iter()
                .map(|&idx| graph.graph[idx].qualified_name.clone())
                .collect();
            let len = symbols.len();
            CircularDependency {
                symbols,
                cycle_length: len,
            }
        })
        .collect()
}
