use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::graph::NodeIndex;
use petgraph::visit::NodeFiltered;
use petgraph::Direction;
use rmcp::model::*;

use crate::db::query;

use super::{get_string_param, AriadneService};

/// Compute a 0.0–1.0 risk score from raw file signals, with confidence tracking.
fn compute_risk(data: &query::FileRiskData) -> (f64, f64, String, Vec<&'static str>) {
    let mut available_signals: Vec<&'static str> = Vec::new();
    let mut scores: Vec<f64> = Vec::new();

    // Churn score: total modifications across symbols, normalized
    if data.symbols_with_history > 0 {
        available_signals.push("churn");
        let churn = (data.total_modifications as f64 / 50.0).min(1.0);
        scores.push(churn);
    }

    // Coupling score: number of coupled files + max strength
    if data.coupled_files > 0 {
        available_signals.push("coupling");
        let coupling =
            (data.coupled_files as f64 / 10.0).min(1.0) * 0.5 + data.max_coupling_strength * 0.5;
        scores.push(coupling.min(1.0));
    }

    // Fan-in score: external callers
    // Always available (0 fan-in is valid data, not missing data)
    available_signals.push("fan_in");
    let fan_in = (data.external_fan_in as f64 / 50.0).min(1.0);
    scores.push(fan_in);

    // Dead code proximity: fraction of dead symbols in file
    available_signals.push("dead_code");
    let dead_code = if data.total_symbols > 0 {
        data.dead_symbols as f64 / data.total_symbols as f64
    } else {
        0.0
    };
    scores.push(dead_code);

    let confidence = available_signals.len() as f64 / 4.0;
    let risk_score = if scores.is_empty() {
        0.0
    } else {
        scores.iter().sum::<f64>() / scores.len() as f64
    };

    let risk_level = match risk_score {
        s if s >= 0.75 => "critical",
        s if s >= 0.5 => "high",
        s if s >= 0.25 => "medium",
        _ => "low",
    };

    (
        risk_score,
        confidence,
        risk_level.to_string(),
        available_signals,
    )
}

impl AriadneService {
    /// Parse comma-separated changed_files parameter into a Vec of trimmed paths.
    fn parse_changed_files(params: &CallToolRequestParam) -> Vec<String> {
        get_string_param(params, "changed_files")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Resolve file paths to their symbol IDs from the database (batched query).
    fn resolve_file_symbols(&self, paths: &[String]) -> Result<Vec<i64>, String> {
        match self.with_db(|db| query::resolve_paths_to_symbol_ids(db, paths)) {
            Ok(Ok(ids)) => Ok(ids),
            Ok(Err(e)) => Err(format!("Error resolving file symbols: {e}")),
            Err(e) => Err(format!("{e}")),
        }
    }

    pub(crate) fn tool_diff_impact(&self, params: &CallToolRequestParam) -> CallToolResult {
        let paths = Self::parse_changed_files(params);
        if paths.is_empty() {
            return CallToolResult::error(vec![Content::text(
                "changed_files parameter is required (comma-separated file paths)",
            )]);
        }

        let symbol_ids = match self.resolve_file_symbols(&paths) {
            Ok(ids) => ids,
            Err(e) => return CallToolResult::error(vec![Content::text(e)]),
        };

        // Compute blast radius and find affected tests using the cached graph
        match self.with_cached_graph(|graph| {
            let mut affected_symbols = Vec::new();
            let mut affected_tests = Vec::new();
            let mut all_affected_files: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();

            for &sym_id in &symbol_ids {
                if let Some(node_idx) = graph.find_node(sym_id as u64) {
                    // BFS to find all upstream callers (blast radius)
                    let mut visited = std::collections::HashSet::new();
                    let mut queue = std::collections::VecDeque::new();
                    queue.push_back((node_idx, 0usize));

                    while let Some((idx, depth)) = queue.pop_front() {
                        if depth > 5 || !visited.insert(idx) {
                            continue;
                        }
                        if let Some(sym) = graph.get_symbol(idx) {
                            if idx != node_idx {
                                if sym.is_test {
                                    affected_tests.push(serde_json::json!({
                                        "name": sym.name,
                                        "qualified_name": sym.qualified_name,
                                        "file": sym.file_path,
                                    }));
                                } else {
                                    affected_symbols.push(serde_json::json!({
                                        "name": sym.name,
                                        "qualified_name": sym.qualified_name,
                                        "kind": sym.kind,
                                        "file": sym.file_path,
                                        "depth": depth,
                                    }));
                                }
                                *all_affected_files.entry(sym.file_path.clone()).or_insert(0) += 1;
                            }
                        }
                        for caller_idx in graph.callers_of(idx) {
                            if !visited.contains(&caller_idx) {
                                queue.push_back((caller_idx, depth + 1));
                            }
                        }
                    }
                }
            }

            // Deduplicate tests by name
            affected_tests.sort_by(|a, b| {
                a.get("name")
                    .and_then(|v| v.as_str())
                    .cmp(&b.get("name").and_then(|v| v.as_str()))
            });
            affected_tests.dedup_by(|a, b| {
                a.get("name").and_then(|v| v.as_str())
                    == b.get("name").and_then(|v| v.as_str())
            });

            // Build review focus: files ranked by how many affected symbols they contain
            let mut review_focus: Vec<_> = all_affected_files.into_iter().collect();
            review_focus.sort_by(|a, b| b.1.cmp(&a.1));
            let review_focus: Vec<_> = review_focus
                .into_iter()
                .take(20)
                .map(|(path, count)| serde_json::json!({"file": path, "affected_symbol_count": count}))
                .collect();

            Ok(serde_json::json!({
                "changed_files": paths,
                "directly_changed_symbols": symbol_ids.len(),
                "affected_symbols": affected_symbols.len(),
                "affected_symbol_details": &affected_symbols[..affected_symbols.len().min(50)],
                "affected_tests": affected_tests,
                "affected_test_count": affected_tests.len(),
                "review_focus": review_focus,
                "truncated": affected_symbols.len() > 50,
            }))
        }) {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Err(e) => CallToolResult::error(vec![Content::text(format!("Diff impact failed: {e}"))]),
        }
    }

    pub(crate) fn tool_compute_file_risk(&self, params: &CallToolRequestParam) -> CallToolResult {
        let paths = Self::parse_changed_files(params);
        if paths.is_empty() {
            return CallToolResult::error(vec![Content::text(
                "changed_files parameter is required (comma-separated file paths)",
            )]);
        }

        let mut file_risks = Vec::new();
        for path in &paths {
            match self.with_db(|db| {
                let file = query::find_file_by_path(db, path)?;
                match file {
                    Some(f) => query::get_file_risk_data(db, f.id),
                    None => Ok(None),
                }
            }) {
                Ok(Ok(Some(data))) => {
                    let (risk_score, confidence, risk_level, available_signals) =
                        compute_risk(&data);
                    file_risks.push(serde_json::json!({
                        "file": data.path,
                        "risk_score": (risk_score * 1000.0).round() / 1000.0,
                        "risk_level": risk_level,
                        "confidence": (confidence * 100.0).round() / 100.0,
                        "available_signals": available_signals,
                        "factors": {
                            "churn": {
                                "total_modifications": data.total_modifications,
                                "max_authors": data.max_authors,
                                "volatile_symbols": data.volatile_count,
                            },
                            "coupling": {
                                "coupled_files": data.coupled_files,
                                "max_strength": (data.max_coupling_strength * 1000.0).round() / 1000.0,
                            },
                            "fan_in": {
                                "external_callers": data.external_fan_in,
                            },
                            "dead_code": {
                                "dead_symbols": data.dead_symbols,
                                "total_symbols": data.total_symbols,
                            },
                        },
                    }));
                }
                Ok(Ok(None)) => {
                    file_risks.push(serde_json::json!({
                        "file": path,
                        "risk_score": 0.0,
                        "risk_level": "low",
                        "confidence": 0.0,
                        "available_signals": [],
                        "note": "File has no indexed symbols",
                    }));
                }
                Ok(Err(e)) => {
                    return CallToolResult::error(vec![Content::text(format!(
                        "Error analyzing {path}: {e}"
                    ))]);
                }
                Err(e) => return CallToolResult::error(vec![Content::text(format!("{e}"))]),
            }
        }

        // Sort by risk_score descending
        file_risks.sort_by(|a, b| {
            b.get("risk_score")
                .and_then(|v| v.as_f64())
                .partial_cmp(&a.get("risk_score").and_then(|v| v.as_f64()))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let result = serde_json::json!({
            "files": file_risks,
            "file_count": file_risks.len(),
            "interpretation": {
                "low": "0.0-0.25: Stable, low risk of introducing bugs",
                "medium": "0.25-0.5: Moderate risk, standard review recommended",
                "high": "0.5-0.75: High risk, careful review and testing recommended",
                "critical": "0.75-1.0: Very high risk, consider senior review and extra test coverage",
            },
        });

        let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
        CallToolResult::success(vec![Content::text(json)])
    }

    pub(crate) fn tool_affected_tests(&self, params: &CallToolRequestParam) -> CallToolResult {
        let paths = Self::parse_changed_files(params);
        if paths.is_empty() {
            return CallToolResult::error(vec![Content::text(
                "changed_files parameter is required (comma-separated file paths)",
            )]);
        }

        let symbol_ids = match self.resolve_file_symbols(&paths) {
            Ok(ids) => ids,
            Err(e) => return CallToolResult::error(vec![Content::text(e)]),
        };

        match self.with_cached_graph(|graph| {
            let mut tests = Vec::new();
            let mut visited_global = std::collections::HashSet::new();

            for &sym_id in &symbol_ids {
                if let Some(node_idx) = graph.find_node(sym_id as u64) {
                    let mut visited = std::collections::HashSet::new();
                    let mut queue = std::collections::VecDeque::new();
                    queue.push_back(node_idx);

                    while let Some(idx) = queue.pop_front() {
                        if !visited.insert(idx) {
                            continue;
                        }
                        if let Some(sym) = graph.get_symbol(idx) {
                            if sym.is_test && visited_global.insert(sym.name.clone()) {
                                tests.push(serde_json::json!({
                                    "name": sym.name,
                                    "qualified_name": sym.qualified_name,
                                    "file": sym.file_path,
                                }));
                            }
                        }
                        for caller_idx in graph.callers_of(idx) {
                            if !visited.contains(&caller_idx) {
                                queue.push_back(caller_idx);
                            }
                        }
                    }
                }
            }

            Ok(serde_json::json!({
                "changed_files": paths,
                "affected_tests": tests,
                "count": tests.len(),
            }))
        }) {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Err(e) => {
                CallToolResult::error(vec![Content::text(format!("Affected tests failed: {e}"))])
            }
        }
    }

    /// Compose `get_dependents` + `affected_tests` + `get_execution_flows` into
    /// an ordered edit plan. The dependent set is sorted (depth_asc, id_asc),
    /// putting depth-1 callers first — a valid topological order on DAGs and a
    /// stable BFS-depth fallback when a cycle is detected in the dependent
    /// cone.
    pub(crate) fn tool_propose_edit_plan(&self, params: &CallToolRequestParam) -> CallToolResult {
        let symbol_name = get_string_param(params, "symbol");
        if symbol_name.is_empty() {
            return CallToolResult::error(vec![Content::text("symbol parameter is required")]);
        }

        // Resolve the target symbol (structured summary on miss — never tool error).
        let target = match self.with_db(|db| query::find_symbol_by_name(db, &symbol_name)) {
            Ok(Ok(Some(s))) => s,
            Ok(Ok(None)) => {
                let result = serde_json::json!({
                    "symbol": serde_json::Value::Null,
                    "total_dependents": 0,
                    "edit_order": [],
                    "affected_tests": [],
                    "execution_flows": {},
                    "cycle_detected": false,
                    "ordering_strategy": "topological",
                    "summary": format!("Symbol not found: {symbol_name}"),
                });
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                return CallToolResult::success(vec![Content::text(json)]);
            }
            Ok(Err(e)) | Err(e) => {
                return CallToolResult::error(vec![Content::text(format!("{e}"))])
            }
        };

        // Pull execution flows from the DB (independent of the cached graph).
        let flows = self
            .with_db(|db| query::get_execution_flows(db, target.id))
            .unwrap_or_else(|_| Ok(Vec::new()))
            .unwrap_or_default();

        let target_file_path = self
            .with_db(|db| query::file_path_by_id(db, target.file_id))
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_else(|| "unknown".into());

        // Build the ordered plan from the cached call graph (BFS upstream + cycle check).
        let plan = self
            .with_cached_graph(|graph| Ok(build_edit_plan(graph, target.id)))
            .unwrap_or_else(|_| EditPlanOrdering::empty());

        // Group execution flow steps by flow name for the response payload.
        let mut grouped_flows: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
        for step in &flows {
            grouped_flows
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

        let edit_order_json: Vec<serde_json::Value> = plan
            .edit_order
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                serde_json::json!({
                    "step": i + 1,
                    "symbol_id": entry.symbol_id,
                    "name": entry.name,
                    "qualified_name": entry.qualified_name,
                    "kind": entry.kind,
                    "file": entry.file_path,
                    "depth": entry.depth,
                    "is_test": entry.is_test,
                })
            })
            .collect();

        let affected_tests_json: Vec<serde_json::Value> = plan
            .edit_order
            .iter()
            .filter(|e| e.is_test)
            .map(|e| {
                serde_json::json!({
                    "name": e.name,
                    "qualified_name": e.qualified_name,
                    "file": e.file_path,
                })
            })
            .collect();

        let summary = if plan.cycle_detected {
            format!(
                "Edit plan for '{}' — {} dependents (cycle detected, BFS-depth fallback ordering)",
                target.name,
                plan.edit_order.len()
            )
        } else {
            format!(
                "Edit plan for '{}' — {} dependents in topological order (leaf-callees first)",
                target.name,
                plan.edit_order.len()
            )
        };

        let result = serde_json::json!({
            "symbol": {
                "name": target.name,
                "qualified_name": target.qualified_name,
                "kind": target.kind,
                "file": target_file_path,
            },
            "total_dependents": plan.edit_order.len(),
            "edit_order": edit_order_json,
            "affected_tests": affected_tests_json,
            "affected_test_count": affected_tests_json.len(),
            "execution_flows": grouped_flows,
            "flow_count": grouped_flows.len(),
            "cycle_detected": plan.cycle_detected,
            "ordering_strategy": if plan.cycle_detected {
                "bfs_depth_fallback"
            } else {
                "topological"
            },
            "summary": summary,
        });

        let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
        CallToolResult::success(vec![Content::text(json)])
    }
}

/// One row of the proposed edit order.
struct EditPlanEntry {
    symbol_id: i64,
    name: String,
    qualified_name: String,
    kind: String,
    file_path: String,
    depth: u32,
    is_test: bool,
}

/// Result of ordering the dependent cone. `cycle_detected` flips the response's
/// `ordering_strategy` field; the in-array order is always `(depth_asc, id_asc)`,
/// which is topologically valid on DAGs and stable on cyclic cones.
struct EditPlanOrdering {
    edit_order: Vec<EditPlanEntry>,
    cycle_detected: bool,
}

impl EditPlanOrdering {
    fn empty() -> Self {
        Self {
            edit_order: Vec::new(),
            cycle_detected: false,
        }
    }
}

/// BFS upstream from `target_id`, then sort the dependent set by
/// (depth_asc, symbol_id_asc) and detect cycles via `petgraph::algo::toposort`
/// over the induced subgraph.
fn build_edit_plan(graph: &crate::graph::CallGraph, target_id: i64) -> EditPlanOrdering {
    let Some(&target_idx) = graph.symbol_index.get(&target_id) else {
        return EditPlanOrdering::empty();
    };

    // BFS upstream (incoming edges = callers) recording depth.
    let mut depth: HashMap<NodeIndex, u32> = HashMap::new();
    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut queue: VecDeque<(NodeIndex, u32)> = VecDeque::new();
    queue.push_back((target_idx, 0));
    visited.insert(target_idx);

    while let Some((node, d)) = queue.pop_front() {
        depth.insert(node, d);
        for caller in graph.graph.neighbors_directed(node, Direction::Incoming) {
            if visited.insert(caller) {
                queue.push_back((caller, d + 1));
            }
        }
    }

    // Cycle detection on the induced subgraph (dependent set including target).
    let dependent_set = visited.clone();
    let filtered = NodeFiltered::from_fn(&graph.graph, |n: NodeIndex| dependent_set.contains(&n));
    let cycle_detected = petgraph::algo::toposort(&filtered, None).is_err();

    // Order: depth asc, then symbol_id asc — deterministic, topologically valid on DAGs,
    // stable BFS-layer ordering on cyclic cones.
    let mut sorted: Vec<NodeIndex> = visited
        .iter()
        .copied()
        .filter(|n| *n != target_idx)
        .collect();
    sorted.sort_by_key(|n| (depth[n], graph.graph[*n].id));

    let edit_order: Vec<EditPlanEntry> = sorted
        .iter()
        .map(|&n| {
            let sym = &graph.graph[n];
            EditPlanEntry {
                symbol_id: sym.id,
                name: sym.name.clone(),
                qualified_name: sym.qualified_name.clone(),
                kind: sym.kind.clone(),
                file_path: sym.file_path.clone(),
                depth: depth[&n],
                is_test: sym.is_test,
            }
        })
        .collect();

    EditPlanOrdering {
        edit_order,
        cycle_detected,
    }
}
