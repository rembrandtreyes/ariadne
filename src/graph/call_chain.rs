use super::CallGraph;

pub fn extract_call_chain(
    graph: &CallGraph,
    symbol_id: i64,
    _cross_service: bool,
) -> String {
    let deps = super::traversal::get_dependencies(graph, symbol_id, Some(10));

    let mut mermaid = String::from("graph LR\n");

    let start_name = graph
        .symbol_index
        .get(&symbol_id)
        .map(|&idx| graph.graph[idx].name.clone())
        .unwrap_or_else(|| "unknown".to_string());

    for dep in &deps {
        mermaid.push_str(&format!("    {} --> {}\n", start_name, dep.name));
    }

    if deps.is_empty() {
        mermaid.push_str(&format!("    {}[\"No downstream calls\"]\n", start_name));
    }

    mermaid
}
