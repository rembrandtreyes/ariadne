/// Generate a Mermaid flowchart from a list of edges.
pub fn generate_flowchart(title: &str, edges: &[(String, String)]) -> String {
    let mut output = format!("graph LR\n    %% {}\n", title);
    for (from, to) in edges {
        let from_id = sanitize_id(from);
        let to_id = sanitize_id(to);
        output.push_str(&format!(
            "    {}[\"{}\"] --> {}[\"{}\"]\n",
            from_id, from, to_id, to
        ));
    }
    output
}

/// Generate a Mermaid diagram for service topology.
pub fn generate_service_topology(services: &[(String, String, String)]) -> String {
    let mut output = String::from("graph TD\n");
    for (from, to, protocol) in services {
        let from_id = sanitize_id(from);
        let to_id = sanitize_id(to);
        output.push_str(&format!(
            "    {}[\"{}\"] -->|{}| {}[\"{}\"]\n",
            from_id, from, protocol, to_id, to
        ));
    }
    output
}

fn sanitize_id(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_non_alphanum() {
        assert_eq!(sanitize_id("foo.bar::baz"), "foo_bar__baz");
    }

    #[test]
    fn flowchart_basic() {
        let edges = vec![("A".to_string(), "B".to_string())];
        let chart = generate_flowchart("test", &edges);
        assert!(chart.contains("graph LR"));
        assert!(chart.contains("A"));
        assert!(chart.contains("B"));
    }
}
