use rmcp::model::*;

use crate::db::query;

use super::{get_string_param, AriadneService};

impl AriadneService {
    pub(crate) fn tool_get_call_chain(&self, params: &CallToolRequestParam) -> CallToolResult {
        let symbol_name = get_string_param(params, "symbol");
        let sym = match self.with_db(|db| query::find_symbol_by_name(db, &symbol_name)) {
            Ok(Ok(Some(s))) => s,
            Ok(Ok(None)) => {
                return CallToolResult::error(vec![Content::text(format!(
                    "Symbol not found: {symbol_name}"
                ))])
            }
            Ok(Err(e)) | Err(e) => {
                return CallToolResult::error(vec![Content::text(format!("{e}"))])
            }
        };
        match self.with_cached_graph(|graph| {
            Ok(crate::graph::call_chain::extract_call_chain(
                graph, sym.id, false,
            ))
        }) {
            Ok(mermaid) => CallToolResult::success(vec![Content::text(mermaid)]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_blast_radius(&self, params: &CallToolRequestParam) -> CallToolResult {
        let symbol_name = get_string_param(params, "symbol");
        let sym = match self.with_db(|db| query::find_symbol_by_name(db, &symbol_name)) {
            Ok(Ok(Some(s))) => s,
            Ok(Ok(None)) => {
                return CallToolResult::error(vec![Content::text(format!(
                    "Symbol not found: {symbol_name}"
                ))])
            }
            Ok(Err(e)) | Err(e) => {
                return CallToolResult::error(vec![Content::text(format!("{e}"))])
            }
        };
        match self.with_cached_graph(|graph| {
            Ok(crate::graph::blast_radius::analyze_blast_radius(
                graph,
                sym.id as u64,
                Some(10),
                false,
            ))
        }) {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_detect_cycles(&self) -> CallToolResult {
        match self.with_cached_graph(|graph| {
            Ok(crate::graph::circular::detect_circular_dependencies(graph))
        }) {
            Ok(cycles) => {
                let json = serde_json::to_string_pretty(&cycles).unwrap_or_else(|_| "[]".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Err(e) => {
                CallToolResult::error(vec![Content::text(format!("Cycle detection failed: {e}"))])
            }
        }
    }

    pub(crate) fn tool_get_dependency_path(&self, params: &CallToolRequestParam) -> CallToolResult {
        let from_name = get_string_param(params, "from_symbol");
        let to_name = get_string_param(params, "to_symbol");

        let from_sym = match self.with_db(|db| query::find_symbol_by_name(db, &from_name)) {
            Ok(Ok(Some(s))) => s,
            Ok(Ok(None)) => {
                return CallToolResult::error(vec![Content::text(format!(
                    "Symbol not found: {from_name}"
                ))])
            }
            Ok(Err(e)) | Err(e) => {
                return CallToolResult::error(vec![Content::text(format!("{e}"))])
            }
        };
        let to_sym = match self.with_db(|db| query::find_symbol_by_name(db, &to_name)) {
            Ok(Ok(Some(s))) => s,
            Ok(Ok(None)) => {
                return CallToolResult::error(vec![Content::text(format!(
                    "Symbol not found: {to_name}"
                ))])
            }
            Ok(Err(e)) | Err(e) => {
                return CallToolResult::error(vec![Content::text(format!("{e}"))])
            }
        };

        match self.with_cached_graph(|graph| {
            let path_ids =
                crate::graph::traversal::find_shortest_path(graph, from_sym.id, to_sym.id);
            let result = match path_ids {
                None => serde_json::json!({
                    "path": null,
                    "reachable": false,
                    "path_length": 0,
                    "summary": format!("No path from '{}' to '{}'", from_name, to_name),
                }),
                Some(ids) => {
                    let path_nodes: Vec<serde_json::Value> = ids
                        .iter()
                        .filter_map(|&id| {
                            graph.symbol_index.get(&id).map(|&idx| {
                                let node = &graph.graph[idx];
                                serde_json::json!({
                                    "id": node.id,
                                    "name": node.name,
                                    "kind": node.kind,
                                    "file_path": node.file_path,
                                })
                            })
                        })
                        .collect();
                    let hops = path_nodes.len().saturating_sub(1);
                    let summary = if path_nodes.len() <= 6 {
                        let names: Vec<&str> = path_nodes
                            .iter()
                            .filter_map(|n| n["name"].as_str())
                            .collect();
                        format!(
                            "{} ({} hop{})",
                            names.join(" → "),
                            hops,
                            if hops == 1 { "" } else { "s" }
                        )
                    } else {
                        format!("Path found ({hops} hops)")
                    };
                    serde_json::json!({
                        "path": path_nodes,
                        "reachable": true,
                        "path_length": hops,
                        "summary": summary,
                    })
                }
            };
            Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into()))
        }) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_boundaries(&self) -> CallToolResult {
        match self.with_db(crate::analysis::boundaries::analyze_boundaries) {
            Ok(Ok(analysis)) => {
                let json = serde_json::to_string_pretty(&analysis).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => CallToolResult::error(vec![Content::text(format!(
                "Boundary analysis failed: {e}"
            ))]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }
}
