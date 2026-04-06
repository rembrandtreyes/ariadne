use std::sync::{Arc, Mutex};

use rmcp::model::*;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::ServerHandler;

use crate::db::query;
use crate::db::Database;
use crate::search;

/// Build a JSON Schema object for tool parameters.
fn make_schema(properties: serde_json::Value, required: Vec<&str>) -> Arc<JsonObject> {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), serde_json::json!("object"));
    map.insert("properties".to_string(), properties);
    map.insert("required".to_string(), serde_json::json!(required));
    Arc::new(map)
}

/// Build a tool with a single required string parameter.
fn string_param_tool(
    name: &'static str,
    desc: &'static str,
    param: &'static str,
    param_desc: &'static str,
) -> Tool {
    Tool::new(
        name,
        desc,
        make_schema(
            serde_json::json!({
                param: { "type": "string", "description": param_desc }
            }),
            vec![param],
        ),
    )
}

/// Build a tool with no parameters.
fn no_param_tool(name: &'static str, desc: &'static str) -> Tool {
    Tool::new(name, desc, Arc::new(serde_json::Map::new()))
}

/// Build a tool accepting a comma-separated list of file paths.
fn files_param_tool(name: &'static str, desc: &'static str) -> Tool {
    Tool::new(
        name,
        desc,
        make_schema(
            serde_json::json!({
                "changed_files": {
                    "type": "string",
                    "description": "Comma-separated list of changed file paths (relative to project root)"
                }
            }),
            vec!["changed_files"],
        ),
    )
}

/// Return the 22 Ariadne MCP tools.
fn all_tools() -> Vec<Tool> {
    vec![
        string_param_tool(
            "search_symbol",
            "Search for symbols by name using full-text and fuzzy matching. Returns name, kind, file, line for each match.",
            "query",
            "Search query string",
        ),
        string_param_tool(
            "get_context",
            "Get full context for a symbol: callers, callees, file, signature, dead status, coupled files",
            "symbol",
            "Symbol name or qualified name",
        ),
        string_param_tool(
            "get_imports",
            "List all imports for a given file with resolution status",
            "file_path",
            "Relative file path",
        ),
        string_param_tool(
            "get_dependents",
            "Find all symbols that depend on (call) the given symbol (upstream callers)",
            "symbol",
            "Symbol name or qualified name",
        ),
        string_param_tool(
            "get_dependencies",
            "Find all symbols that the given symbol depends on (downstream callees)",
            "symbol",
            "Symbol name or qualified name",
        ),
        string_param_tool(
            "get_call_chain",
            "Trace the full call chain from a symbol to its transitive callees, returned as Mermaid flowchart",
            "symbol",
            "Symbol name or qualified name",
        ),
        string_param_tool(
            "blast_radius",
            "Compute the blast radius of changing a symbol — shows WILL BREAK and MAY BREAK dependents grouped by depth",
            "symbol",
            "Symbol name or qualified name",
        ),
        no_param_tool(
            "find_dead_code",
            "Find unreachable functions and methods that are never called",
        ),
        string_param_tool(
            "get_file_summary",
            "Get a summary of all symbols and imports in a file",
            "file_path",
            "Relative file path",
        ),
        no_param_tool(
            "get_complexity",
            "Get codebase statistics: file count, symbol count, call count, resolution rate, dead functions, languages",
        ),
        no_param_tool(
            "detect_cycles",
            "Detect circular dependencies in the call graph using Kosaraju SCC. Returns cycles with involved symbol names and cycle length.",
        ),
        no_param_tool(
            "get_boundaries",
            "Analyze module boundaries: symbol counts, internal vs external calls, cross-boundary call details, and approximate modularity scores.",
        ),
        files_param_tool(
            "diff_impact",
            "Unified change-impact analysis: given changed files, returns affected symbols, blast radius summary, affected tests, and review focus in one call.",
        ),
        files_param_tool(
            "affected_tests",
            "Find test functions that transitively depend on symbols in the changed files.",
        ),
        string_param_tool(
            "why_symbol",
            "Explain a symbol's role: metadata, callers, callees, blast radius, and coupled files.",
            "symbol",
            "Symbol name or qualified name",
        ),
        string_param_tool(
            "get_heritage",
            "Get inheritance hierarchy for a symbol: parent classes/interfaces and child subclasses.",
            "symbol",
            "Symbol name or qualified name",
        ),
        string_param_tool(
            "get_execution_flows",
            "Trace execution flows passing through a symbol — ordered call paths from entry points.",
            "symbol",
            "Symbol name or qualified name",
        ),
        no_param_tool(
            "get_coupling",
            "Get the top coupled file pairs by git co-change strength, revealing implicit dependencies.",
        ),
        no_param_tool(
            "get_communities",
            "List detected module communities with symbol counts and modularity scores.",
        ),
        no_param_tool(
            "get_api_endpoints",
            "List all detected API endpoints with HTTP method, path, handler symbol, and file location.",
        ),
        Tool::new(
            "get_file_dependencies",
            "Get files that a given file depends on (via resolved call edges). Returns each dependency file with the connecting symbol pairs.",
            make_schema(
                serde_json::json!({
                    "file_path": {
                        "type": "string",
                        "description": "Relative file path"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Transitive depth (1 = direct only, 2+ = follow transitive deps). Default: 1",
                        "default": 1
                    }
                }),
                vec!["file_path"],
            ),
        ),
        Tool::new(
            "get_file_dependents",
            "Get files that depend on a given file (via resolved call edges). Returns each dependent file with the connecting symbol pairs.",
            make_schema(
                serde_json::json!({
                    "file_path": {
                        "type": "string",
                        "description": "Relative file path"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Transitive depth (1 = direct only, 2+ = follow transitive deps). Default: 1",
                        "default": 1
                    }
                }),
                vec!["file_path"],
            ),
        ),
    ]
}

/// Extract a string parameter from the tool call arguments.
fn get_string_param(params: &CallToolRequestParam, key: &str) -> String {
    params
        .arguments
        .as_ref()
        .and_then(|args| args.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Extract an optional integer parameter from the tool call arguments.
fn get_int_param(params: &CallToolRequestParam, key: &str) -> Option<i64> {
    params
        .arguments
        .as_ref()
        .and_then(|args| args.get(key))
        .and_then(|v| v.as_i64())
}

/// Collect file dependencies transitively up to `max_depth` levels.
/// When `forward` is true, follows dependencies (what this file calls).
/// When `forward` is false, follows dependents (what calls this file).
fn collect_transitive_file_deps(
    db: &crate::db::Database,
    start_file_id: i64,
    max_depth: usize,
    forward: bool,
) -> anyhow::Result<Vec<query::FileDependency>> {
    use std::collections::{HashSet, VecDeque};

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

pub struct AriadneService {
    db: Arc<Mutex<Database>>,
    /// Cached call graph — rebuilt when pipeline_generation changes.
    /// Eliminates the O(symbols+calls) SQLite scan on every MCP tool invocation.
    graph_cache: Arc<Mutex<Option<crate::graph::CallGraph>>>,
    /// The pipeline generation when the cache was last built.
    /// Compared against the DB's `pipeline_generation` metadata to detect staleness.
    cached_generation: Arc<Mutex<u64>>,
}

impl Clone for AriadneService {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            graph_cache: Arc::clone(&self.graph_cache),
            cached_generation: Arc::clone(&self.cached_generation),
        }
    }
}

impl AriadneService {
    pub fn new(db: Database) -> Self {
        Self {
            db: Arc::new(Mutex::new(db)),
            graph_cache: Arc::new(Mutex::new(None)),
            cached_generation: Arc::new(Mutex::new(0)),
        }
    }

    fn with_db<F, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(&Database) -> T,
    {
        let db = self
            .db
            .lock()
            .map_err(|e| anyhow::anyhow!("Database lock error: {e}"))?;
        Ok(f(&db))
    }

    /// Run `f` with a reference to the call graph, rebuilding it when the
    /// pipeline generation changes (e.g. after watch-mode reindex).
    /// Lock order is always cache → db to prevent deadlocks.
    fn with_cached_graph<F, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(&crate::graph::CallGraph) -> anyhow::Result<T>,
    {
        let mut cache = self
            .graph_cache
            .lock()
            .map_err(|e| anyhow::anyhow!("Graph cache lock error: {e}"))?;

        // Check if the pipeline has been re-indexed since we last built the cache
        let db_generation = {
            let db = self
                .db
                .lock()
                .map_err(|e| anyhow::anyhow!("Database lock error: {e}"))?;
            db.get_metadata("pipeline_generation")
                .ok()
                .flatten()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0)
        };

        let mut gen = self
            .cached_generation
            .lock()
            .map_err(|e| anyhow::anyhow!("Generation lock error: {e}"))?;

        if cache.is_none() || db_generation != *gen {
            let graph = {
                let db = self
                    .db
                    .lock()
                    .map_err(|e| anyhow::anyhow!("Database lock error: {e}"))?;
                query::build_call_graph(&db, Some(10000))?
            };
            *cache = Some(graph);
            *gen = db_generation;
        }

        let graph = cache
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Graph cache is unexpectedly empty"))?;
        f(graph)
    }

    fn dispatch(&self, name: &str, params: &CallToolRequestParam) -> CallToolResult {
        match name {
            "search_symbol" => self.tool_search_symbol(params),
            "get_context" => self.tool_get_context(params),
            "get_imports" => self.tool_get_imports(params),
            "get_dependents" => self.tool_get_dependents(params),
            "get_dependencies" => self.tool_get_dependencies(params),
            "get_call_chain" => self.tool_get_call_chain(params),
            "blast_radius" => self.tool_blast_radius(params),
            "find_dead_code" => self.tool_find_dead_code(),
            "get_file_summary" => self.tool_get_file_summary(params),
            "get_complexity" => self.tool_get_complexity(),
            "detect_cycles" => self.tool_detect_cycles(),
            "get_boundaries" => self.tool_get_boundaries(),
            "diff_impact" => self.tool_diff_impact(params),
            "affected_tests" => self.tool_affected_tests(params),
            "why_symbol" => self.tool_why_symbol(params),
            "get_heritage" => self.tool_get_heritage(params),
            "get_execution_flows" => self.tool_get_execution_flows(params),
            "get_coupling" => self.tool_get_coupling(),
            "get_communities" => self.tool_get_communities(),
            "get_api_endpoints" => self.tool_get_api_endpoints(),
            "get_file_dependencies" => self.tool_get_file_dependencies(params),
            "get_file_dependents" => self.tool_get_file_dependents(params),
            _ => CallToolResult::error(vec![Content::text(format!("Unknown tool: {}", name))]),
        }
    }

    fn tool_search_symbol(&self, params: &CallToolRequestParam) -> CallToolResult {
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

    fn tool_get_context(&self, params: &CallToolRequestParam) -> CallToolResult {
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

    fn tool_get_imports(&self, params: &CallToolRequestParam) -> CallToolResult {
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

    fn tool_get_dependents(&self, params: &CallToolRequestParam) -> CallToolResult {
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

    fn tool_get_dependencies(&self, params: &CallToolRequestParam) -> CallToolResult {
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

    fn tool_get_call_chain(&self, params: &CallToolRequestParam) -> CallToolResult {
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

    fn tool_blast_radius(&self, params: &CallToolRequestParam) -> CallToolResult {
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

    fn tool_find_dead_code(&self) -> CallToolResult {
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

    fn tool_get_file_summary(&self, params: &CallToolRequestParam) -> CallToolResult {
        let file_path_param = get_string_param(params, "file_path");
        match self.with_db(|db| {
            let file = query::find_file_by_path(db, &file_path_param)?
                .ok_or_else(|| anyhow::anyhow!("File not found: {file_path_param}"))?;
            let symbols = query::get_file_symbols(db, file.id).unwrap_or_default();
            let imports = query::get_file_imports(db, file.id).unwrap_or_default();
            Ok::<_, anyhow::Error>(serde_json::json!({
                "file": file.path, "language": file.language,
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

    fn tool_get_complexity(&self) -> CallToolResult {
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

    fn tool_detect_cycles(&self) -> CallToolResult {
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

    fn tool_get_boundaries(&self) -> CallToolResult {
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

    fn tool_diff_impact(&self, params: &CallToolRequestParam) -> CallToolResult {
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

    fn tool_affected_tests(&self, params: &CallToolRequestParam) -> CallToolResult {
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

    fn tool_why_symbol(&self, params: &CallToolRequestParam) -> CallToolResult {
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

    fn tool_get_heritage(&self, params: &CallToolRequestParam) -> CallToolResult {
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

    fn tool_get_execution_flows(&self, params: &CallToolRequestParam) -> CallToolResult {
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

    fn tool_get_coupling(&self) -> CallToolResult {
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

    fn tool_get_communities(&self) -> CallToolResult {
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

    fn tool_get_api_endpoints(&self) -> CallToolResult {
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

    fn tool_get_file_dependencies(&self, params: &CallToolRequestParam) -> CallToolResult {
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

    fn tool_get_file_dependents(&self, params: &CallToolRequestParam) -> CallToolResult {
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

impl ServerHandler for AriadneService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "ariadne".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some(
                "Ariadne -- universal dependency graph for AI coding agents. \
                 Query symbols, blast radius, call chains, dead code, and more."
                    .to_string(),
            ),
        }
    }

    fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::Error>> + Send + '_ {
        std::future::ready(Ok(ListToolsResult {
            tools: all_tools(),
            next_cursor: None,
        }))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::Error>> + Send + '_ {
        let result = self.dispatch(&request.name, &request);
        std::future::ready(Ok(result))
    }
}
