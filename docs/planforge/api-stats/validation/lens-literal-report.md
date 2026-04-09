# Lens 1: Literal Accuracy Report

**Date:** 2026-04-07
**PRDs scanned:** PRD-01, PRD-02
**Result:** CLEAN (both PRDs)

## PRD-01 Findings

### SQL Queries
- `SELECT COUNT(*) FROM symbols` — `symbols` table exists in schema (`src/db/schema.rs` line 28). CLEAN.
- `SELECT COUNT(*) FROM calls` — `calls` table exists in schema (`src/db/schema.rs` line 58). CLEAN.
- `SELECT MAX(last_indexed) FROM services` — `services.last_indexed REAL` column exists at line 12. CLEAN. Returns `NULL` if no rows — handled correctly as `Option<f64>`.

### Struct and Types
- `#[derive(Debug, Clone, Serialize)]` on `GraphStats` — `Serialize` from `serde` already imported at top of `src/db/query.rs` (`use serde::Serialize;`). CLEAN.
- `Database` type used in `fn get_graph_stats(db: &Database)` — `use super::Database;` is at top of file. CLEAN.
- `anyhow::Result` return type — `anyhow` is a project dependency in `Cargo.toml`. CLEAN.

### Unit Test
- `Database::open_in_memory()` — method exists in `src/db/mod.rs`. CLEAN.
- Test module `query_graph_stats_tests` uses `use super::*;` and `use crate::db::Database;` — both in scope. CLEAN.

**Verdict: CLEAN. No HARD violations. No SOFT violations.**

## PRD-02 Findings

### Function Calls and Types
- `crate::db::query::get_graph_stats(&db)` — will exist after PRD-01 completes. Dependency declared. CLEAN.
- `open_db(&db_path)` — exists in `src/dashboard/api.rs` at line 44. CLEAN.
- `ApiError::query_failed(...)` — `query_failed` associated function exists on `ApiError` at line 27. CLEAN.
- `GraphStats` fields accessed: `.node_count`, `.edge_count`, `.last_indexed` — match PRD-01 struct definition exactly. CLEAN.
- `stats.last_indexed.map(epoch_to_iso8601)` — `epoch_to_iso8601` takes `f64`, returns `String`. `Option<f64>.map(fn(f64) -> String)` produces `Option<String>`. CLEAN.

### Axum Types
- `State<DbState>` extractor — `DbState = Arc<PathBuf>` is in scope. CLEAN.
- `Json<GraphStatsResponse>` — `Json` already imported via `use axum::Json;` in `api.rs`. CLEAN.
- `Result<Json<GraphStatsResponse>, ApiError>` — `ApiError` implements `IntoResponse` (line 34). CLEAN.

### Route Registration
- `.route("/api/graph-stats", axum::routing::get(api::graph_stats))` — `api::graph_stats` will be `pub async fn` in `src/dashboard/api.rs`. CLEAN.

### `epoch_to_iso8601` Algorithm
- Uses only integer arithmetic on `u64`/`i64` — no overflow risk for dates in the current millennium.
- Hinnant's civil-from-days algorithm is mathematically correct for Gregorian calendar.
- `format!` with `{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z"` produces valid ISO 8601. CLEAN.

### Integration Test
- `setup_indexed_db()` — already defined in `tests/test_dashboard.rs`. CLEAN.
- `graph_stats(State(state)).await` — correct handler call signature. CLEAN.
- Import update adds `graph_stats` to existing `use ariadne::dashboard::api::{...}` — `graph_stats` will be a public fn. CLEAN.

**Verdict: CLEAN. No HARD violations.**

**SOFT violation auto-fixed:**
- `assert!(data.edge_count >= 0, ...)` replaced with `let _ = data.edge_count;` — `u64 >= 0` is always true, would trigger `clippy::absurd_extreme_comparisons`. Fixed in place during validation.
