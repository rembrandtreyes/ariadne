use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::Database;

/// Shared state: just the path to the database file.
/// We open a fresh read connection per request since rusqlite::Connection is !Send.
pub type DbState = Arc<PathBuf>;

/// API error with sanitized message — never leaks file paths or internals.
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    pub message: &'static str,
}

impl ApiError {
    fn query_failed(message: &'static str) -> Self {
        Self {
            code: "query_failed",
            message,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.code {
            "db_unavailable" => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self)).into_response()
    }
}

fn open_db(state: &DbState) -> Result<Database, ApiError> {
    Database::open(state.as_ref()).map_err(|_| ApiError {
        code: "db_unavailable",
        message: "Database is unavailable. Ensure ariadne index has been run.",
    })
}

#[derive(Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Serialize, Clone)]
pub struct GraphNode {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file: String,
    pub group: u32,
    pub in_degree: u32,
    pub out_degree: u32,
    pub is_dead: bool,
    pub line_start: u32,
    pub line_end: u32,
    pub signature: String,
}

#[derive(Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub confidence: f64,
}

#[derive(Serialize)]
pub struct Stats {
    pub files: u64,
    pub symbols: u64,
    pub calls: u64,
    pub resolution_rate: f64,
    pub dead_functions: u64,
    pub languages: Vec<String>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

#[derive(Deserialize)]
pub struct NeighborhoodQuery {
    pub id: i64,
    pub depth: Option<u32>,
}

#[derive(Serialize)]
pub struct Insights {
    pub circular_deps: Vec<Vec<String>>,
    pub most_connected: Vec<InsightNode>,
    pub dead_code_count: u64,
    pub god_files: Vec<InsightNode>,
    pub dead_code: Vec<DeadCodeEntry>,
}

#[derive(Serialize)]
pub struct InsightNode {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file: String,
    pub connections: u32,
}

#[derive(Serialize)]
pub struct DeadCodeEntry {
    pub id: String,
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub file: String,
    pub line_start: u32,
}

pub async fn graph_data(State(db_path): State<DbState>) -> Result<Json<GraphData>, ApiError> {
    let db = open_db(&db_path)?;
    let data =
        build_graph_data(&db).map_err(|_| ApiError::query_failed("Failed to build graph data."))?;
    Ok(Json(data))
}

pub async fn stats(State(db_path): State<DbState>) -> Result<Json<Stats>, ApiError> {
    let db = open_db(&db_path)?;
    let s = build_stats(&db).map_err(|_| ApiError::query_failed("Failed to load stats."))?;
    Ok(Json(s))
}

pub async fn search_symbols(
    State(db_path): State<DbState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<GraphNode>>, ApiError> {
    let q = query.q.unwrap_or_default();
    if q.is_empty() {
        return Ok(Json(Vec::new()));
    }
    if q.len() > 500 {
        return Ok(Json(vec![]));
    }
    let db = open_db(&db_path)?;
    let results = do_search(&db, &q).map_err(|_| ApiError::query_failed("Search query failed."))?;
    Ok(Json(results))
}

pub async fn neighborhood(
    State(db_path): State<DbState>,
    Query(query): Query<NeighborhoodQuery>,
) -> Result<Json<GraphData>, ApiError> {
    let db = open_db(&db_path)?;
    let data = build_neighborhood(&db, &query.id.to_string(), query.depth)
        .map_err(|_| ApiError::query_failed("Failed to build neighborhood graph."))?;
    Ok(Json(data))
}

pub async fn insights(State(db_path): State<DbState>) -> Result<Json<Insights>, ApiError> {
    let db = open_db(&db_path)?;
    let data =
        build_insights(&db).map_err(|_| ApiError::query_failed("Failed to build insights."))?;
    Ok(Json(data))
}

fn build_graph_data(db: &Database) -> anyhow::Result<GraphData> {
    let conn = db.conn();

    let mut sym_stmt = conn.prepare(
        "SELECT s.id, s.name, s.kind, f.path, s.is_dead, s.line_start, s.line_end, s.signature
         FROM symbols s JOIN files f ON s.file_id = f.id
         LIMIT 5000",
    )?;

    let mut nodes: Vec<GraphNode> = sym_stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            let kind: String = row.get(2)?;
            let group = match kind.as_str() {
                "function" => 0,
                "class" => 1,
                "method" => 2,
                "interface" => 3,
                _ => 4,
            };
            Ok(GraphNode {
                id: id.to_string(),
                name: row.get(1)?,
                kind,
                file: row.get(3)?,
                group,
                in_degree: 0,
                out_degree: 0,
                is_dead: row.get(4).unwrap_or(false),
                line_start: row.get(5).unwrap_or(0),
                line_end: row.get(6).unwrap_or(0),
                signature: row.get(7).unwrap_or_default(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();

    let mut call_stmt = conn.prepare(
        "SELECT caller_symbol_id, callee_symbol_id, confidence
         FROM calls
         WHERE callee_symbol_id IS NOT NULL
         LIMIT 10000",
    )?;

    let edges: Vec<GraphEdge> = call_stmt
        .query_map([], |row| {
            let src: i64 = row.get(0)?;
            let tgt: i64 = row.get(1)?;
            Ok(GraphEdge {
                source: src.to_string(),
                target: tgt.to_string(),
                confidence: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, rusqlite::Error>>()?
        .into_iter()
        .filter(|e| node_ids.contains(&e.source) && node_ids.contains(&e.target))
        .collect();

    // Compute in_degree and out_degree
    let mut in_degree: HashMap<String, u32> = HashMap::new();
    let mut out_degree: HashMap<String, u32> = HashMap::new();
    for edge in &edges {
        *out_degree.entry(edge.source.clone()).or_insert(0) += 1;
        *in_degree.entry(edge.target.clone()).or_insert(0) += 1;
    }
    for node in &mut nodes {
        node.in_degree = in_degree.get(&node.id).copied().unwrap_or(0);
        node.out_degree = out_degree.get(&node.id).copied().unwrap_or(0);
    }

    Ok(GraphData { nodes, edges })
}

fn build_neighborhood(
    db: &Database,
    start_id: &str,
    depth: Option<u32>,
) -> anyhow::Result<GraphData> {
    let depth = depth.unwrap_or(2).min(5);
    let conn = db.conn();

    // BFS to find all reachable node IDs within depth hops
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();

    visited.insert(start_id.to_string());
    queue.push_back((start_id.to_string(), 0));

    while let Some((current_id, current_depth)) = queue.pop_front() {
        if current_depth >= depth {
            continue;
        }

        let current_id_num: i64 = match current_id.parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        // Get outgoing neighbors (caller -> callee)
        let mut out_stmt = conn.prepare_cached(
            "SELECT CAST(callee_symbol_id AS TEXT) FROM calls
             WHERE caller_symbol_id = ?1 AND callee_symbol_id IS NOT NULL",
        )?;
        let outgoing: Vec<String> = out_stmt
            .query_map(params![current_id_num], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        // Get incoming neighbors (callee <- caller)
        let mut in_stmt = conn.prepare_cached(
            "SELECT CAST(caller_symbol_id AS TEXT) FROM calls
             WHERE callee_symbol_id = ?1",
        )?;
        let incoming: Vec<String> = in_stmt
            .query_map(params![current_id_num], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        for neighbor_id in outgoing.into_iter().chain(incoming.into_iter()) {
            if visited.insert(neighbor_id.clone()) {
                queue.push_back((neighbor_id, current_depth + 1));
            }
        }
    }

    if visited.is_empty() {
        return Ok(GraphData {
            nodes: Vec::new(),
            edges: Vec::new(),
        });
    }

    // Build a comma-separated list of IDs for the IN clause
    let id_list: Vec<String> = visited.iter().cloned().collect();
    let placeholders: String = id_list.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    // Query symbols for the neighborhood
    let query_str = format!(
        "SELECT s.id, s.name, s.kind, f.path, s.is_dead, s.line_start, s.line_end, s.signature
         FROM symbols s JOIN files f ON s.file_id = f.id
         WHERE s.id IN ({})",
        placeholders
    );
    let mut sym_stmt = conn.prepare(&query_str)?;

    let id_params: Vec<Box<dyn rusqlite::types::ToSql>> = id_list
        .iter()
        .map(|id| {
            let n: i64 = id.parse().unwrap_or(0);
            Box::new(n) as Box<dyn rusqlite::types::ToSql>
        })
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        id_params.iter().map(|p| p.as_ref()).collect();

    let mut nodes: Vec<GraphNode> = sym_stmt
        .query_map(param_refs.as_slice(), |row| {
            let id: i64 = row.get(0)?;
            let kind: String = row.get(2)?;
            let group = match kind.as_str() {
                "function" => 0,
                "class" => 1,
                "method" => 2,
                "interface" => 3,
                _ => 4,
            };
            Ok(GraphNode {
                id: id.to_string(),
                name: row.get(1)?,
                kind,
                file: row.get(3)?,
                group,
                in_degree: 0,
                out_degree: 0,
                is_dead: row.get(4).unwrap_or(false),
                line_start: row.get(5).unwrap_or(0),
                line_end: row.get(6).unwrap_or(0),
                signature: row.get(7).unwrap_or_default(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();

    // Query edges between neighborhood nodes
    let edge_query = format!(
        "SELECT caller_symbol_id, callee_symbol_id, confidence
         FROM calls
         WHERE callee_symbol_id IS NOT NULL
           AND caller_symbol_id IN ({0})
           AND callee_symbol_id IN ({0})",
        placeholders
    );
    let mut edge_stmt = conn.prepare(&edge_query)?;

    // Need two copies of param_refs for the two IN clauses
    let mut double_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    for id in &id_list {
        let n: i64 = id.parse().unwrap_or(0);
        double_params.push(Box::new(n));
    }
    for id in &id_list {
        let n: i64 = id.parse().unwrap_or(0);
        double_params.push(Box::new(n));
    }
    let double_refs: Vec<&dyn rusqlite::types::ToSql> =
        double_params.iter().map(|p| p.as_ref()).collect();

    let edges: Vec<GraphEdge> = edge_stmt
        .query_map(double_refs.as_slice(), |row| {
            let src: i64 = row.get(0)?;
            let tgt: i64 = row.get(1)?;
            Ok(GraphEdge {
                source: src.to_string(),
                target: tgt.to_string(),
                confidence: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, rusqlite::Error>>()?
        .into_iter()
        .filter(|e| node_ids.contains(&e.source) && node_ids.contains(&e.target))
        .collect();

    // Compute degrees
    let mut in_deg: HashMap<String, u32> = HashMap::new();
    let mut out_deg: HashMap<String, u32> = HashMap::new();
    for edge in &edges {
        *out_deg.entry(edge.source.clone()).or_insert(0) += 1;
        *in_deg.entry(edge.target.clone()).or_insert(0) += 1;
    }
    for node in &mut nodes {
        node.in_degree = in_deg.get(&node.id).copied().unwrap_or(0);
        node.out_degree = out_deg.get(&node.id).copied().unwrap_or(0);
    }

    Ok(GraphData { nodes, edges })
}

fn build_insights(db: &Database) -> anyhow::Result<Insights> {
    let conn = db.conn();

    // most_connected: top 10 by total degree (caller + callee occurrences)
    let mut mc_stmt = conn.prepare(
        "SELECT s.id, s.name, s.kind, f.path,
                (SELECT COUNT(*) FROM calls c WHERE c.caller_symbol_id = s.id) +
                (SELECT COUNT(*) FROM calls c WHERE c.callee_symbol_id = s.id) AS total
         FROM symbols s
         JOIN files f ON s.file_id = f.id
         ORDER BY total DESC
         LIMIT 10",
    )?;
    let most_connected: Vec<InsightNode> = mc_stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            Ok(InsightNode {
                id: id.to_string(),
                name: row.get(1)?,
                kind: row.get(2)?,
                file: row.get(3)?,
                connections: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // god_files: symbols with 50+ connections
    let mut gf_stmt = conn.prepare(
        "SELECT * FROM (
             SELECT s.id, s.name, s.kind, f.path,
                    (SELECT COUNT(*) FROM calls c WHERE c.caller_symbol_id = s.id) +
                    (SELECT COUNT(*) FROM calls c WHERE c.callee_symbol_id = s.id) AS total
             FROM symbols s
             JOIN files f ON s.file_id = f.id
         ) WHERE total >= 50
         ORDER BY total DESC",
    )?;
    let god_files: Vec<InsightNode> = gf_stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            Ok(InsightNode {
                id: id.to_string(),
                name: row.get(1)?,
                kind: row.get(2)?,
                file: row.get(3)?,
                connections: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let dead_code_count = crate::db::query::count_dead(db)?;

    // circular_deps: find pairs where A calls B and B calls A (simplest cycle detection)
    // Then also look for longer cycles up to limit of 10
    let mut cycle_stmt = conn.prepare(
        "SELECT DISTINCT
            CAST(c1.caller_symbol_id AS TEXT),
            CAST(c1.callee_symbol_id AS TEXT)
         FROM calls c1
         JOIN calls c2 ON c1.caller_symbol_id = c2.callee_symbol_id
                      AND c1.callee_symbol_id = c2.caller_symbol_id
         WHERE c1.callee_symbol_id IS NOT NULL
           AND c2.callee_symbol_id IS NOT NULL
           AND c1.caller_symbol_id < c1.callee_symbol_id
         LIMIT 10",
    )?;
    let circular_deps: Vec<Vec<String>> = cycle_stmt
        .query_map([], |row| {
            let a: String = row.get(0)?;
            let b: String = row.get(1)?;
            Ok(vec![a, b])
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // dead code list: join with files to get path, limit to 200 entries
    let mut dead_stmt = conn.prepare(
        "SELECT s.id, s.name, s.qualified_name, s.kind, f.path, s.line_start
         FROM symbols s JOIN files f ON s.file_id = f.id
         WHERE s.is_dead = 1
         ORDER BY f.path, s.line_start
         LIMIT 200",
    )?;
    let dead_code: Vec<DeadCodeEntry> = dead_stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            Ok(DeadCodeEntry {
                id: id.to_string(),
                name: row.get(1)?,
                qualified_name: row.get(2)?,
                kind: row.get(3)?,
                file: row.get(4)?,
                line_start: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Insights {
        circular_deps,
        most_connected,
        dead_code_count,
        god_files,
        dead_code,
    })
}

fn build_stats(db: &Database) -> anyhow::Result<Stats> {
    let files = crate::db::query::count_files(db)?;
    let symbols = crate::db::query::count_symbols(db)?;
    let calls = crate::db::query::count_calls(db)?;
    let resolution_rate = crate::db::query::resolution_rate(db)?;
    let dead_functions = crate::db::query::count_dead(db)?;
    let languages = crate::db::query::get_languages(db)?;

    Ok(Stats {
        files,
        symbols,
        calls,
        resolution_rate,
        dead_functions,
        languages,
    })
}

#[derive(Serialize)]
pub struct SourceResult {
    pub code: String,
    pub line_start: u32,
    pub line_end: u32,
    pub language: String,
    pub file: String,
}

#[derive(Deserialize)]
pub struct SourceQuery {
    pub id: i64,
}

pub async fn source(
    State(db_path): State<DbState>,
    Query(query): Query<SourceQuery>,
) -> Result<Json<SourceResult>, ApiError> {
    let db = open_db(&db_path)?;
    let result = fetch_source(&db, query.id)
        .map_err(|_| ApiError::query_failed("Failed to fetch source code."))?;
    Ok(Json(result))
}

fn fetch_source(db: &Database, symbol_id: i64) -> anyhow::Result<SourceResult> {
    let conn = db.conn();

    let (absolute_path, line_start, line_end, language, file_path): (
        String,
        u32,
        u32,
        String,
        String,
    ) = conn.query_row(
        "SELECT f.absolute_path, s.line_start, s.line_end, f.language, f.path
         FROM symbols s JOIN files f ON s.file_id = f.id
         WHERE s.id = ?1",
        params![symbol_id],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        },
    )?;

    // Validate the path to prevent directory traversal attacks.
    // Canonicalize resolves symlinks and .. components.
    let canonical = std::path::Path::new(&absolute_path)
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("Source file not accessible"))?;
    if canonical.to_string_lossy().contains("..") {
        anyhow::bail!("Invalid source path");
    }

    let content = std::fs::read_to_string(&canonical)?;
    let all_lines: Vec<&str> = content.lines().collect();
    let total = all_lines.len() as u32;

    // 3 lines of context above/below; line numbers are 1-indexed
    let context: u32 = 3;
    let start_idx = line_start.saturating_sub(context + 1) as usize;
    let end_idx = (line_end + context).min(total) as usize;

    let code = all_lines[start_idx..end_idx].join("\n");

    Ok(SourceResult {
        code,
        line_start,
        line_end,
        language,
        file: file_path,
    })
}

fn do_search(db: &Database, query: &str) -> anyhow::Result<Vec<GraphNode>> {
    let conn = db.conn();

    let pattern = format!("%{}%", crate::db::escape_like(query));
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, s.kind, f.path, s.is_dead, s.line_start, s.line_end, s.signature
         FROM symbols s JOIN files f ON s.file_id = f.id
         WHERE s.name LIKE ?1 ESCAPE '\\' OR s.qualified_name LIKE ?1 ESCAPE '\\'
         LIMIT 50",
    )?;

    let results: Vec<GraphNode> = stmt
        .query_map(params![pattern], |row| {
            let id: i64 = row.get(0)?;
            let kind: String = row.get(2)?;
            let group = match kind.as_str() {
                "function" => 0,
                "class" => 1,
                "method" => 2,
                "interface" => 3,
                _ => 4,
            };
            Ok(GraphNode {
                id: id.to_string(),
                name: row.get(1)?,
                kind,
                file: row.get(3)?,
                group,
                in_degree: 0,
                out_degree: 0,
                is_dead: row.get(4).unwrap_or(false),
                line_start: row.get(5).unwrap_or(0),
                line_end: row.get(6).unwrap_or(0),
                signature: row.get(7).unwrap_or_default(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}
