use rmcp::model::*;

use crate::db::query;
use crate::search;

use super::{get_string_param, AriadneService};

impl AriadneService {
    pub(crate) fn tool_search_symbol(&self, params: &CallToolRequestParam) -> CallToolResult {
        let q = get_string_param(params, "query");
        if q.len() > 500 {
            return CallToolResult::error(vec![Content::text(
                "Query too long: maximum 500 characters allowed",
            )]);
        }
        match self.with_db(|db| {
            let opts = search::SearchOptions {
                limit: Some(20),
                fuzzy: true,
                ..Default::default()
            };
            search::search(db, &q, &opts)
        }) {
            Ok(Ok(results)) => {
                let json = serde_json::to_string_pretty(&results).unwrap_or_else(|_| "[]".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => CallToolResult::error(vec![Content::text(format!("Search error: {e}"))]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_context(&self, params: &CallToolRequestParam) -> CallToolResult {
        let symbol_name = get_string_param(params, "symbol");
        match self.with_db(|db| {
            let sym = query::find_symbol_by_name(db, &symbol_name)?;
            let sym = sym.ok_or_else(|| anyhow::anyhow!("Symbol not found: {symbol_name}"))?;

            let file_path =
                query::file_path_by_id(db, sym.file_id).unwrap_or_else(|_| "unknown".into());
            let dependents = query::get_dependents(db, sym.id).unwrap_or_default();
            let dependencies = query::get_dependencies(db, sym.id).unwrap_or_default();
            let couplings = query::get_file_couplings(db, sym.file_id).unwrap_or_default();

            Ok::<_, anyhow::Error>(serde_json::json!({
                "symbol": {
                    "id": sym.id,
                    "name": sym.name,
                    "qualified_name": sym.qualified_name,
                    "kind": sym.kind,
                    "file": file_path,
                    "line_start": sym.line_start,
                    "line_end": sym.line_end,
                    "is_dead": sym.is_dead,
                    "is_test": sym.is_test,
                },
                "callers": dependents.iter().map(|d| serde_json::json!({
                    "name": d.name, "qualified_name": d.qualified_name, "kind": d.kind,
                })).collect::<Vec<_>>(),
                "callees": dependencies.iter().map(|d| serde_json::json!({
                    "name": d.name, "qualified_name": d.qualified_name, "kind": d.kind,
                })).collect::<Vec<_>>(),
                "coupled_files": couplings.iter().map(|c| serde_json::json!({
                    "path": c.coupled_path, "strength": c.strength, "co_changes": c.co_changes,
                })).collect::<Vec<_>>(),
            }))
        }) {
            Ok(Ok(context)) => CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&context).unwrap_or_else(|_| "{}".into()),
            )]),
            Ok(Err(e)) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_dependents(&self, params: &CallToolRequestParam) -> CallToolResult {
        let symbol_name = get_string_param(params, "symbol");
        match self.with_db(|db| {
            let sym = query::find_symbol_by_name(db, &symbol_name)?
                .ok_or_else(|| anyhow::anyhow!("Symbol not found: {symbol_name}"))?;
            query::get_dependents(db, sym.id)
        }) {
            Ok(Ok(deps)) => {
                let json = serde_json::to_string_pretty(&deps).unwrap_or_else(|_| "[]".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_dependencies(&self, params: &CallToolRequestParam) -> CallToolResult {
        let symbol_name = get_string_param(params, "symbol");
        match self.with_db(|db| {
            let sym = query::find_symbol_by_name(db, &symbol_name)?
                .ok_or_else(|| anyhow::anyhow!("Symbol not found: {symbol_name}"))?;
            query::get_dependencies(db, sym.id)
        }) {
            Ok(Ok(deps)) => {
                let json = serde_json::to_string_pretty(&deps).unwrap_or_else(|_| "[]".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_why_symbol(&self, params: &CallToolRequestParam) -> CallToolResult {
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

        // Get callers, callees, and file path from DB
        let (file_path, callers, callees, couplings) = match self.with_db(|db| {
            let fp = query::file_path_by_id(db, sym.file_id).unwrap_or_else(|_| "unknown".into());
            let callers = query::get_dependents(db, sym.id).unwrap_or_default();
            let callees = query::get_dependencies(db, sym.id).unwrap_or_default();
            let couplings = query::get_file_couplings(db, sym.file_id).unwrap_or_default();
            (fp, callers, callees, couplings)
        }) {
            Ok(result) => result,
            Err(e) => return CallToolResult::error(vec![Content::text(format!("{e}"))]),
        };

        // Get blast radius from cached graph
        let blast_radius = self
            .with_cached_graph(|graph| {
                Ok(crate::graph::blast_radius::analyze_blast_radius(
                    graph,
                    sym.id as u64,
                    Some(5),
                    false,
                ))
            })
            .ok();

        let result = serde_json::json!({
            "symbol": {
                "name": sym.name,
                "qualified_name": sym.qualified_name,
                "kind": sym.kind,
                "file": file_path,
                "line_start": sym.line_start,
                "line_end": sym.line_end,
                "is_dead": sym.is_dead,
                "is_test": sym.is_test,
            },
            "callers": callers.iter().map(|d| serde_json::json!({
                "name": d.name, "qualified_name": d.qualified_name, "kind": d.kind,
            })).collect::<Vec<_>>(),
            "callees": callees.iter().map(|d| serde_json::json!({
                "name": d.name, "qualified_name": d.qualified_name, "kind": d.kind,
            })).collect::<Vec<_>>(),
            "caller_count": callers.len(),
            "callee_count": callees.len(),
            "blast_radius": blast_radius,
            "coupled_files": couplings.iter().map(|c| serde_json::json!({
                "path": c.coupled_path, "strength": c.strength, "co_changes": c.co_changes,
            })).collect::<Vec<_>>(),
        });

        let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
        CallToolResult::success(vec![Content::text(json)])
    }

    pub(crate) fn tool_get_heritage(&self, params: &CallToolRequestParam) -> CallToolResult {
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
        match self.with_db(|db| query::get_heritage(db, sym.id)) {
            Ok(Ok(rows)) => {
                let parents: Vec<_> = rows
                    .iter()
                    .filter(|r| r.child_symbol_id == sym.id)
                    .map(|r| {
                        serde_json::json!({
                            "parent_name": r.parent_name,
                            "parent_qualified_name": r.parent_qualified_name,
                            "kind": r.kind,
                            "resolved": r.parent_symbol_id.is_some(),
                        })
                    })
                    .collect();
                let children: Vec<_> = rows
                    .iter()
                    .filter(|r| r.parent_symbol_id == Some(sym.id))
                    .map(|r| {
                        serde_json::json!({
                            "child_name": r.child_name,
                            "kind": r.kind,
                        })
                    })
                    .collect();
                let result = serde_json::json!({
                    "symbol": sym.name,
                    "qualified_name": sym.qualified_name,
                    "parents": parents,
                    "children": children,
                    "parent_count": parents.len(),
                    "child_count": children.len(),
                });
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => {
                CallToolResult::error(vec![Content::text(format!("Heritage query failed: {e}"))])
            }
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_symbol_history(&self, params: &CallToolRequestParam) -> CallToolResult {
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

        let file_path = match self.with_db(|db| query::file_path_by_id(db, sym.file_id)) {
            Ok(Ok(p)) => p,
            _ => "unknown".to_string(),
        };

        match self.with_db(|db| query::get_symbol_history(db, sym.id)) {
            Ok(Ok(Some(history))) => {
                let result = serde_json::json!({
                    "symbol": {
                        "name": sym.name,
                        "qualified_name": sym.qualified_name,
                        "kind": sym.kind,
                        "file": file_path,
                        "line_start": sym.line_start,
                        "line_end": sym.line_end,
                    },
                    "history": {
                        "created_at": history.created_at,
                        "last_modified_at": history.last_modified_at,
                        "modification_count": history.modification_count,
                        "author_count": history.author_count,
                        "is_volatile": history.is_volatile,
                    },
                });
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Ok(None)) => {
                let result = serde_json::json!({
                    "symbol": {
                        "name": sym.name,
                        "qualified_name": sym.qualified_name,
                        "kind": sym.kind,
                        "file": file_path,
                    },
                    "history": null,
                    "note": "No git history available. Ensure the project has been indexed with a git repository present."
                });
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => {
                CallToolResult::error(vec![Content::text(format!("History query failed: {e}"))])
            }
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_execution_flows(&self, params: &CallToolRequestParam) -> CallToolResult {
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
        match self.with_db(|db| query::get_execution_flows(db, sym.id)) {
            Ok(Ok(steps)) => {
                // Group steps by flow name
                let mut flows: std::collections::HashMap<String, Vec<serde_json::Value>> =
                    std::collections::HashMap::new();
                for step in &steps {
                    flows
                        .entry(step.flow_name.clone())
                        .or_default()
                        .push(serde_json::json!({
                            "step": step.step_order,
                            "depth": step.depth,
                            "symbol": step.symbol_name,
                            "qualified_name": step.qualified_name,
                            "file": step.file_path,
                            "kind": step.kind,
                        }));
                }
                let result = serde_json::json!({
                    "symbol": sym.name,
                    "flows": flows,
                    "flow_count": flows.len(),
                    "total_steps": steps.len(),
                });
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => {
                CallToolResult::error(vec![Content::text(format!("Flow query failed: {e}"))])
            }
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }
}
