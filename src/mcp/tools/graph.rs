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
