mod architecture;
mod file;
mod graph;
mod impact;
mod symbol;

use std::sync::{Arc, Mutex};

use rmcp::model::*;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::ServerHandler;

use crate::db::query;
use crate::db::Database;

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

/// Return the 24 Ariadne MCP tools.
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
        files_param_tool(
            "compute_file_risk",
            "Compute per-file risk scores (0.0-1.0) for changed files. Combines churn velocity, coupling degree, fan-in count, and dead code proximity into a single risk assessment with confidence tracking. Use for PR review triage.",
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
        string_param_tool(
            "get_symbol_history",
            "Get temporal history for a symbol: creation date, last modification, change frequency, author count, and volatility. Reveals code stability patterns unavailable from static analysis.",
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
pub(crate) fn get_string_param(params: &CallToolRequestParam, key: &str) -> String {
    params
        .arguments
        .as_ref()
        .and_then(|args| args.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Extract an optional integer parameter from the tool call arguments.
pub(crate) fn get_int_param(params: &CallToolRequestParam, key: &str) -> Option<i64> {
    params
        .arguments
        .as_ref()
        .and_then(|args| args.get(key))
        .and_then(|v| v.as_i64())
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

    pub(crate) fn with_db<F, T>(&self, f: F) -> anyhow::Result<T>
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
    pub(crate) fn with_cached_graph<F, T>(&self, f: F) -> anyhow::Result<T>
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
            // Symbol tools
            "search_symbol" => self.tool_search_symbol(params),
            "get_context" => self.tool_get_context(params),
            "get_dependents" => self.tool_get_dependents(params),
            "get_dependencies" => self.tool_get_dependencies(params),
            "why_symbol" => self.tool_why_symbol(params),
            "get_heritage" => self.tool_get_heritage(params),
            "get_execution_flows" => self.tool_get_execution_flows(params),
            "get_symbol_history" => self.tool_get_symbol_history(params),
            // File tools
            "get_imports" => self.tool_get_imports(params),
            "get_file_summary" => self.tool_get_file_summary(params),
            "get_complexity" => self.tool_get_complexity(),
            "get_file_dependencies" => self.tool_get_file_dependencies(params),
            "get_file_dependents" => self.tool_get_file_dependents(params),
            // Graph tools
            "get_call_chain" => self.tool_get_call_chain(params),
            "blast_radius" => self.tool_blast_radius(params),
            "detect_cycles" => self.tool_detect_cycles(),
            "get_boundaries" => self.tool_get_boundaries(),
            // Impact tools
            "diff_impact" => self.tool_diff_impact(params),
            "affected_tests" => self.tool_affected_tests(params),
            "compute_file_risk" => self.tool_compute_file_risk(params),
            // Architecture tools
            "find_dead_code" => self.tool_find_dead_code(),
            "get_coupling" => self.tool_get_coupling(),
            "get_communities" => self.tool_get_communities(),
            "get_api_endpoints" => self.tool_get_api_endpoints(),
            _ => CallToolResult::error(vec![Content::text(format!("Unknown tool: {}", name))]),
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
