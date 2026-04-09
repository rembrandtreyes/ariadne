# Master Plan: Add `/api/graph-stats` REST Endpoint

## Overview

Add a new REST endpoint to the Ariadne dashboard that returns a concise summary of the
indexed dependency graph. The endpoint responds to `GET /api/graph-stats` and returns:

- `node_count` — total number of symbols/nodes in the dependency graph
- `edge_count` — total number of call relationships/edges in the graph
- `last_indexed` — RFC 3339 timestamp of when the database was last updated (from
  `services.last_indexed` or the `metadata` key-value store)

## Why This Endpoint

The existing `/api/stats` endpoint returns a rich `Stats` struct (file count, resolution
rate, dead functions, languages) aimed at developer analytics. Clients — including
future dashboard panels, external tools, and health probes — need a minimal "how big is
the graph right now?" signal that is cheap to fetch and easy to parse.

## Scope

This is a pure addition. No existing endpoints or handlers are modified. No schema
migration is needed — `last_indexed` already exists in the `services` table and the
`metadata` table can serve as a fallback.

## Implementation Plan

### 1. Add query function in `src/db/query.rs`

Add `pub fn get_graph_stats(db: &Database) -> anyhow::Result<GraphStats>` that runs:
- `SELECT COUNT(*) FROM symbols` → `node_count: u64`
- `SELECT COUNT(*) FROM calls` → `edge_count: u64`
- `SELECT MAX(last_indexed) FROM services` → `last_indexed: Option<f64>` (Unix epoch
  float stored by rusqlite), formatted as RFC 3339 string via `chrono` or manual
  formatting using `std::time`. If NULL (no services indexed yet), return `null`.

The return type `GraphStats` is a plain struct with three fields.

### 2. Add response type and handler in `src/dashboard/api.rs`

Add:
- `pub struct GraphStatsResponse { node_count: u64, edge_count: u64, last_indexed: Option<String> }`  
  with `#[derive(Debug, Clone, Serialize)]`
- `pub async fn graph_stats(State(db_path): State<DbState>) -> Result<Json<GraphStatsResponse>, ApiError>`
  handler that opens the DB, calls `get_graph_stats`, maps the `GraphStats` to
  `GraphStatsResponse` (converting the float timestamp to ISO 8601 string).

### 3. Register route in `src/dashboard/mod.rs`

Add `.route("/api/graph-stats", axum::routing::get(api::graph_stats))` to the Axum
router in `serve()`.

### 4. Add integration test in `tests/test_dashboard.rs`

Add `test_graph_stats_handler` that:
- Uses `setup_indexed_db()` (already defined in that file)
- Calls `graph_stats(State(state)).await`
- Asserts `node_count > 0`, `edge_count >= 0`
- Asserts `last_indexed` is `Some(...)` with a non-empty string

## Decomposition

This plan decomposes into 2 mini PRDs:

1. **PRD-01: DB query function** — `get_graph_stats` in `src/db/query.rs` + unit test
2. **PRD-02: Handler, route, and integration test** — handler in `src/dashboard/api.rs`,
   route registration in `src/dashboard/mod.rs`, integration test in `tests/test_dashboard.rs`

PRD-02 depends on PRD-01 (uses `GraphStats` type and `get_graph_stats` function).
