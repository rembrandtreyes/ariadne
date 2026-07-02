use std::collections::{HashSet, VecDeque};

use rmcp::model::*;

use crate::db::query;

use super::{get_int_param, get_string_param, AriadneService};

/// Collect file dependencies transitively up to `max_depth` levels.
/// When `forward` is true, follows dependencies (what this file calls).
/// When `forward` is false, follows dependents (what calls this file).
pub(crate) fn collect_transitive_file_deps(
    db: &crate::db::Database,
    start_file_id: i64,
    max_depth: usize,
    forward: bool,
) -> anyhow::Result<Vec<query::FileDependency>> {
    let mut visited = HashSet::new();
    visited.insert(start_file_id);

    let mut queue = VecDeque::new();
    queue.push_back((start_file_id, 0usize));

    let mut all_deps = Vec::new();

    while let Some((file_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let deps = if forward {
            query::get_file_dependencies(db, file_id)?
        } else {
            query::get_file_dependents(db, file_id)?
        };

        for dep in deps {
            if visited.insert(dep.file_id) {
                queue.push_back((dep.file_id, depth + 1));
                all_deps.push(dep);
            }
        }
    }

    Ok(all_deps)
}

impl AriadneService {
    pub(crate) fn tool_get_imports(&self, params: &CallToolRequestParam) -> CallToolResult {
        let file_path = get_string_param(params, "file_path");
        match self.with_db(|db| {
            let file = query::find_file_by_path(db, &file_path)?
                .ok_or_else(|| anyhow::anyhow!("File not found: {file_path}"))?;
            query::get_file_imports(db, file.id)
        }) {
            Ok(Ok(imports)) => {
                let json = serde_json::to_string_pretty(&imports).unwrap_or_else(|_| "[]".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_file_summary(&self, params: &CallToolRequestParam) -> CallToolResult {
        let file_path_param = get_string_param(params, "file_path");
        match self.with_db(|db| {
            let file = query::find_file_by_path(db, &file_path_param)?
                .ok_or_else(|| anyhow::anyhow!("File not found: {file_path_param}"))?;
            let symbols = query::get_file_symbols(db, file.id).unwrap_or_default();
            let imports = query::get_file_imports(db, file.id).unwrap_or_default();
            Ok::<_, anyhow::Error>(serde_json::json!({
                "file": file.path, "language": file.language,
                "parse_error_count": query::get_file_parse_error_count(db, file.id)?,
                "symbols": symbols.iter().map(|s| serde_json::json!({
                    "name": s.name, "kind": s.kind,
                    "line_start": s.line_start, "line_end": s.line_end, "is_dead": s.is_dead,
                })).collect::<Vec<_>>(),
                "imports": imports.iter().map(|i| serde_json::json!({
                    "name": i.imported_name, "module": i.module_path,
                    "line": i.line, "resolved": i.resolved_symbol_id.is_some(),
                })).collect::<Vec<_>>(),
                "symbol_count": symbols.len(), "import_count": imports.len(),
            }))
        }) {
            Ok(Ok(summary)) => CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "{}".into()),
            )]),
            Ok(Err(e)) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_complexity(&self) -> CallToolResult {
        match self.with_db(|db| {
            let files = query::count_files(db)?;
            let symbols = query::count_symbols(db)?;
            let calls = query::count_calls(db)?;
            let resolved_calls = query::count_resolved_calls(db)?;
            let dead_functions = query::count_dead(db)?;
            let resolution_rate = query::resolution_rate(db)?;
            let languages = query::get_languages(db)?;
            Ok::<_, anyhow::Error>(serde_json::json!({
                "files": files,
                "symbols": symbols,
                "calls": calls,
                "resolved_calls": resolved_calls,
                "dead_functions": dead_functions,
                "resolution_rate": resolution_rate,
                "languages": languages,
            }))
        }) {
            Ok(Ok(stats)) => CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&stats).unwrap_or_else(|_| "{}".into()),
            )]),
            Ok(Err(e)) => {
                CallToolResult::error(vec![Content::text(format!("Complexity query failed: {e}"))])
            }
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_file_dependencies(
        &self,
        params: &CallToolRequestParam,
    ) -> CallToolResult {
        let file_path = get_string_param(params, "file_path");
        let depth = get_int_param(params, "depth").unwrap_or(1).clamp(1, 5) as usize;

        match self.with_db(|db| {
            let file = query::find_file_by_path(db, &file_path)?
                .ok_or_else(|| anyhow::anyhow!("File not found: {file_path}"))?;
            collect_transitive_file_deps(db, file.id, depth, true)
        }) {
            Ok(Ok(deps)) => {
                let files: Vec<_> = deps
                    .iter()
                    .map(|d| {
                        serde_json::json!({
                            "file": d.path,
                            "language": d.language,
                            "connections": d.connections.iter().map(|c| {
                                serde_json::json!({
                                    "from": c.from_symbol,
                                    "to": c.to_symbol,
                                })
                            }).collect::<Vec<_>>(),
                            "connection_count": d.connections.len(),
                        })
                    })
                    .collect();
                let result = serde_json::json!({
                    "file": file_path,
                    "direction": "dependencies",
                    "depth": depth,
                    "dependencies": files,
                    "count": files.len(),
                });
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }

    pub(crate) fn tool_get_file_dependents(&self, params: &CallToolRequestParam) -> CallToolResult {
        let file_path = get_string_param(params, "file_path");
        let depth = get_int_param(params, "depth").unwrap_or(1).clamp(1, 5) as usize;

        match self.with_db(|db| {
            let file = query::find_file_by_path(db, &file_path)?
                .ok_or_else(|| anyhow::anyhow!("File not found: {file_path}"))?;
            collect_transitive_file_deps(db, file.id, depth, false)
        }) {
            Ok(Ok(deps)) => {
                let files: Vec<_> = deps
                    .iter()
                    .map(|d| {
                        serde_json::json!({
                            "file": d.path,
                            "language": d.language,
                            "connections": d.connections.iter().map(|c| {
                                serde_json::json!({
                                    "from": c.from_symbol,
                                    "to": c.to_symbol,
                                })
                            }).collect::<Vec<_>>(),
                            "connection_count": d.connections.len(),
                        })
                    })
                    .collect();
                let result = serde_json::json!({
                    "file": file_path,
                    "direction": "dependents",
                    "depth": depth,
                    "dependents": files,
                    "count": files.len(),
                });
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
                CallToolResult::success(vec![Content::text(json)])
            }
            Ok(Err(e)) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
            Err(e) => CallToolResult::error(vec![Content::text(format!("{e}"))]),
        }
    }
}
