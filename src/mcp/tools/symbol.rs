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

            let mut context = serde_json::json!({
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
            });
            if let Some(health) = super::parse_health_json(db)? {
                if let Some(obj) = context.as_object_mut() {
                    obj.insert("parse_warnings".to_string(), health);
                }
            }
            Ok::<_, anyhow::Error>(context)
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

        let mut result = serde_json::json!({
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

        if let Err(e) = self.attach_parse_warnings(&mut result) {
            return CallToolResult::error(vec![Content::text(format!("{e}"))]);
        }
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

    pub(crate) fn tool_get_symbol_health(&self, params: &CallToolRequestParam) -> CallToolResult {
        let symbol_name = get_string_param(params, "symbol");
        match self.with_db(|db| query::get_symbol_health_data(db, &symbol_name)) {
            Ok(Ok(Some(data))) => {
                let stability_score = 1.0
                    - ((data.modification_count as f64 / 30.0) * 0.5
                        + (data.author_count as f64 / 10.0) * 0.3
                        + if data.is_volatile { 0.2 } else { 0.0 })
                    .min(1.0);
                let complexity_score = (((data.fan_in + data.fan_out) as f64 / 20.0) * 0.7
                    + ((data.line_end - data.line_start) as f64 / 200.0) * 0.3)
                    .min(1.0);

                let health_score = stability_score * 0.5
                    + (1.0 - complexity_score) * 0.3
                    + if data.is_dead { 0.0 } else { 0.2 };
                let health_score = health_score.clamp(0.0, 1.0);

                let confidence: f64 = if data.has_history { 0.8 } else { 0.3 }
                    + if data.fan_in > 0 || data.fan_out > 0 {
                        0.2
                    } else {
                        0.0
                    };
                let confidence = confidence.min(1.0);

                let health_level = match health_score {
                    s if s >= 0.8 => "excellent",
                    s if s >= 0.6 => "good",
                    s if s >= 0.4 => "fair",
                    s if s >= 0.2 => "poor",
                    _ => "critical",
                };

                let mut signals = Vec::new();
                if data.is_volatile {
                    signals.push("high_volatility");
                }
                if data.fan_in > 5 {
                    signals.push("high_fan_in");
                }
                if data.fan_out > 8 {
                    signals.push("high_fan_out");
                }
                if data.is_dead {
                    signals.push("dead_code");
                }
                if data.author_count > 5 {
                    signals.push("many_authors");
                }

                let result = serde_json::json!({
                    "symbol": {
                        "name": data.name,
                        "qualified_name": data.qualified_name,
                        "kind": data.kind,
                        "file": data.file_path,
                        "line_start": data.line_start,
                        "line_end": data.line_end,
                    },
                    "health_score": (health_score * 1000.0).round() / 1000.0,
                    "health_level": health_level,
                    "stability_score": (stability_score * 1000.0).round() / 1000.0,
                    "complexity_score": (complexity_score * 1000.0).round() / 1000.0,
                    "confidence": (confidence * 1000.0).round() / 1000.0,
                    "signals": signals,
                    "metrics": {
                        "fan_in": data.fan_in,
                        "fan_out": data.fan_out,
                        "modification_count": data.modification_count,
                        "author_count": data.author_count,
                        "is_volatile": data.is_volatile,
                        "is_dead": data.is_dead,
                    },
                });
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Ok(None)) => CallToolResult::error(vec![Content::text(format!(
                "Symbol not found: {symbol_name}"
            ))]),
            Ok(Err(e)) => {
                CallToolResult::error(vec![Content::text(format!("Health query failed: {e}"))])
            }
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_complexity_hotspots(&self) -> CallToolResult {
        let total_symbols: i64 = match self.with_db(|db| {
            Ok::<_, anyhow::Error>(db.conn().query_row(
                "SELECT COUNT(*) FROM symbols WHERE is_test = 0 AND is_dead = 0",
                [],
                |row| row.get(0),
            )?)
        }) {
            Ok(Ok(n)) => n,
            _ => 0,
        };

        match self.with_db(|db| query::get_complexity_hotspots(db, 50)) {
            Ok(Ok(hotspots)) => {
                let items: Vec<_> = hotspots
                    .iter()
                    .map(|h| {
                        serde_json::json!({
                            "symbol": h.name,
                            "qualified_name": h.qualified_name,
                            "kind": h.kind,
                            "file": h.file_path,
                            "fan_in": h.fan_in,
                            "fan_out": h.fan_out,
                            "modification_count": h.modification_count,
                            "is_volatile": h.is_volatile,
                        })
                    })
                    .collect();
                let result = serde_json::json!({
                    "hotspots": items,
                    "count": items.len(),
                    "total_symbols": total_symbols,
                });
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => {
                CallToolResult::error(vec![Content::text(format!("Hotspot query failed: {e}"))])
            }
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_code_smells(&self) -> CallToolResult {
        match self.with_db(query::get_code_smell_candidates) {
            Ok(Ok(candidates)) => {
                let mut smells = Vec::new();
                for c in &candidates {
                    if c.is_volatile && c.modification_count > 10 {
                        smells.push(serde_json::json!({
                            "symbol": c.name,
                            "qualified_name": c.qualified_name,
                            "file": c.file_path,
                            "smell_type": "high_volatility",
                            "severity": if c.modification_count > 25 { "high" } else { "medium" },
                            "details": {
                                "modification_count": c.modification_count,
                                "author_count": c.author_count,
                            },
                        }));
                    }
                    if c.fan_in > 5 {
                        smells.push(serde_json::json!({
                            "symbol": c.name,
                            "qualified_name": c.qualified_name,
                            "file": c.file_path,
                            "smell_type": "high_fan_in",
                            "severity": if c.fan_in > 15 { "high" } else { "medium" },
                            "details": { "fan_in": c.fan_in },
                        }));
                    }
                    if c.fan_out > 8 {
                        smells.push(serde_json::json!({
                            "symbol": c.name,
                            "qualified_name": c.qualified_name,
                            "file": c.file_path,
                            "smell_type": "high_fan_out",
                            "severity": if c.fan_out > 15 { "high" } else { "medium" },
                            "details": { "fan_out": c.fan_out },
                        }));
                    }
                    if c.is_dead {
                        smells.push(serde_json::json!({
                            "symbol": c.name,
                            "qualified_name": c.qualified_name,
                            "file": c.file_path,
                            "smell_type": "dead_code",
                            "severity": "medium",
                            "details": { "is_dead": true },
                        }));
                    }
                }
                let result = serde_json::json!({
                    "smells": smells,
                    "count": smells.len(),
                });
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => {
                CallToolResult::error(vec![Content::text(format!("Code smell query failed: {e}"))])
            }
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }
}
