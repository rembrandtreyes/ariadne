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
}
