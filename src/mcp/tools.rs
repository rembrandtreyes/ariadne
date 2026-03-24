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

/// Return the 10 Phase 1 Ariadne MCP tools.
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

pub struct AriadneService {
    db: Arc<Mutex<Database>>,
    /// Cached call graph — built once on first blast_radius/get_call_chain call,
    /// then reused for all subsequent requests. Eliminates the O(symbols+calls)
    /// SQLite scan on every MCP tool invocation.
    graph_cache: Arc<Mutex<Option<crate::graph::CallGraph>>>,
}

impl Clone for AriadneService {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            graph_cache: Arc::clone(&self.graph_cache),
        }
    }
}

impl AriadneService {
    pub fn new(db: Database) -> Self {
        Self {
            db: Arc::new(Mutex::new(db)),
            graph_cache: Arc::new(Mutex::new(None)),
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

    /// Run `f` with a reference to the call graph, building and caching it on
    /// first use. Lock order is always cache → db to prevent deadlocks.
    fn with_cached_graph<F, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(&crate::graph::CallGraph) -> anyhow::Result<T>,
    {
        let mut cache = self
            .graph_cache
            .lock()
            .map_err(|e| anyhow::anyhow!("Graph cache lock error: {e}"))?;

        if cache.is_none() {
            let graph = {
                let db = self
                    .db
                    .lock()
                    .map_err(|e| anyhow::anyhow!("Database lock error: {e}"))?;
                query::build_call_graph(&db, Some(10000))?
            };
            *cache = Some(graph);
        }

        f(cache.as_ref().unwrap())
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
            Ok(crate::graph::call_chain::extract_call_chain(graph, sym.id, false))
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
