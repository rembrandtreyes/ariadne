use rmcp::model::*;

use crate::db::query;

use super::{get_string_param, AriadneService};

impl AriadneService {
    /// Parse comma-separated changed_files parameter into a Vec of trimmed paths.
    fn parse_changed_files(params: &CallToolRequestParam) -> Vec<String> {
        get_string_param(params, "changed_files")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Resolve file paths to their symbol IDs from the database.
    fn resolve_file_symbols(&self, paths: &[String]) -> Result<Vec<i64>, String> {
        let mut symbol_ids = Vec::new();
        for path in paths {
            match self.with_db(|db| {
                let file = query::find_file_by_path(db, path)?;
                if let Some(f) = file {
                    let syms = query::get_file_symbols(db, f.id)?;
                    Ok::<Vec<i64>, anyhow::Error>(syms.iter().map(|s| s.id).collect())
                } else {
                    Ok(vec![])
                }
            }) {
                Ok(Ok(ids)) => symbol_ids.extend(ids),
                Ok(Err(e)) => return Err(format!("Error resolving {path}: {e}")),
                Err(e) => return Err(format!("{e}")),
            }
        }
        Ok(symbol_ids)
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
