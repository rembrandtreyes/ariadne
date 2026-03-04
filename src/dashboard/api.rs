use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::Database;

/// Shared state: just the path to the database file.
/// We open a fresh read connection per request since rusqlite::Connection is !Send.
pub type DbState = Arc<PathBuf>;

fn open_db(state: &DbState) -> Option<Database> {
    Database::open(state.as_ref()).ok()
}

#[derive(Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Serialize)]
pub struct GraphNode {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file: String,
    pub group: u32,
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

pub async fn graph_data(State(db_path): State<DbState>) -> Json<GraphData> {
    let data = open_db(&db_path).and_then(|db| build_graph_data(&db).ok());
    Json(data.unwrap_or(GraphData {
        nodes: Vec::new(),
        edges: Vec::new(),
    }))
}

pub async fn stats(State(db_path): State<DbState>) -> Json<Stats> {
    let s = open_db(&db_path).and_then(|db| build_stats(&db).ok());
    Json(s.unwrap_or(Stats {
        files: 0,
        symbols: 0,
        calls: 0,
        resolution_rate: 0.0,
        dead_functions: 0,
        languages: Vec::new(),
    }))
}

pub async fn search_symbols(
    State(db_path): State<DbState>,
    Query(query): Query<SearchQuery>,
) -> Json<Vec<GraphNode>> {
    let q = query.q.unwrap_or_default();
    if q.is_empty() {
        return Json(Vec::new());
    }
    let results = open_db(&db_path)
        .and_then(|db| do_search(&db, &q).ok())
        .unwrap_or_default();
    Json(results)
}

fn build_graph_data(db: &Database) -> anyhow::Result<GraphData> {
    let conn = db.conn();

    let mut sym_stmt = conn.prepare(
        "SELECT s.id, s.name, s.kind, f.path
         FROM symbols s JOIN files f ON s.file_id = f.id
         LIMIT 500",
    )?;

    let nodes: Vec<GraphNode> = sym_stmt
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
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let node_ids: std::collections::HashSet<String> =
        nodes.iter().map(|n| n.id.clone()).collect();

    let mut call_stmt = conn.prepare(
        "SELECT caller_symbol_id, callee_symbol_id, confidence
         FROM calls
         WHERE callee_symbol_id IS NOT NULL
         LIMIT 2000",
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
        .filter_map(|r| r.ok())
        .filter(|e| node_ids.contains(&e.source) && node_ids.contains(&e.target))
        .collect();

    Ok(GraphData { nodes, edges })
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

fn do_search(db: &Database, query: &str) -> anyhow::Result<Vec<GraphNode>> {
    let conn = db.conn();

    let pattern = format!("%{}%", crate::db::escape_like(query));
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, s.kind, f.path
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
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}
