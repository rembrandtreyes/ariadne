use rmcp::model::*;

use crate::db::query;

use super::AriadneService;

impl AriadneService {
    pub(crate) fn tool_find_dead_code(&self) -> CallToolResult {
        match self.with_db(|db| {
            let dead = query::get_dead_symbols(db)?;
            let output: Vec<serde_json::Value> = dead
                .iter()
                .map(|s| {
                    let file_path =
                        query::file_path_by_id(db, s.file_id).unwrap_or_else(|_| "unknown".into());
                    serde_json::json!({
                        "name": s.name, "qualified_name": s.qualified_name,
                        "kind": s.kind, "file": file_path, "line": s.line_start,
                    })
                })
                .collect();
            Ok::<_, anyhow::Error>(output)
        }) {
            Ok(Ok(output)) => {
                let json = serde_json::to_string_pretty(&output).unwrap_or_else(|_| "[]".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_coupling(&self) -> CallToolResult {
        match self.with_db(|db| query::get_top_couplings(db, 50)) {
            Ok(Ok(rows)) => {
                let pairs: Vec<_> = rows
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "files": r.coupled_path,
                            "co_changes": r.co_changes,
                            "strength": r.strength,
                        })
                    })
                    .collect();
                let result = serde_json::json!({
                    "coupled_pairs": pairs,
                    "count": pairs.len(),
                });
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => {
                CallToolResult::error(vec![Content::text(format!("Coupling query failed: {e}"))])
            }
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_communities(&self) -> CallToolResult {
        match self.with_db(query::get_communities) {
            Ok(Ok(rows)) => {
                let communities: Vec<_> = rows
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "id": c.id,
                            "name": c.name,
                            "symbol_count": c.symbol_count,
                            "internal_edges": c.internal_edges,
                            "external_edges": c.external_edges,
                            "modularity": c.modularity,
                        })
                    })
                    .collect();
                let result = serde_json::json!({
                    "communities": communities,
                    "count": communities.len(),
                });
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => {
                CallToolResult::error(vec![Content::text(format!("Community query failed: {e}"))])
            }
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_api_endpoints(&self) -> CallToolResult {
        match self.with_db(query::get_api_endpoints) {
            Ok(Ok(rows)) => {
                let endpoints: Vec<_> = rows
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "method": e.method,
                            "path": e.path_pattern,
                            "protocol": e.protocol,
                            "handler": e.handler_name,
                            "handler_qualified_name": e.handler_qualified_name,
                            "file": e.file_path,
                            "line": e.line,
                        })
                    })
                    .collect();
                let result = serde_json::json!({
                    "endpoints": endpoints,
                    "count": endpoints.len(),
                });
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => CallToolResult::error(vec![Content::text(format!(
                "API endpoints query failed: {e}"
            ))]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }
}
