# Mini PRD 02: Handler, Route Registration, and Integration Test

> **Dependency:** Requires Mini PRD 01 (provides `GraphStats` struct and `get_graph_stats` function in `src/db/query.rs`)
> **Produces:** `pub struct GraphStatsResponse`, `pub async fn graph_stats(...)` handler in `src/dashboard/api.rs`; route `/api/graph-stats` in `src/dashboard/mod.rs`; integration test `test_graph_stats_handler` in `tests/test_dashboard.rs`
> **Estimated steps:** 3

## Context

After Mini PRD 01 adds the database query layer (`get_graph_stats`), this mini PRD wires
it into the Ariadne dashboard web server. It adds: (1) a serializable response type
`GraphStatsResponse` and Axum handler `graph_stats` in `src/dashboard/api.rs`; (2) a new
route `/api/graph-stats` in the Axum router in `src/dashboard/mod.rs`; and (3) an
integration test in `tests/test_dashboard.rs`. The handler converts the `Option<f64>`
Unix-epoch timestamp from the DB layer into an `Option<String>` ISO 8601 datetime string
using only the Rust standard library (no `chrono` dependency — `chrono` is not in
`Cargo.toml`). The new route is a pure addition alongside existing routes; no existing
routes are changed.

The existing `/api/stats` endpoint returns detailed analytics (file count, resolution
rate, dead functions, languages) and is unrelated to this new endpoint. This endpoint
is intentionally minimal: node count, edge count, last indexed timestamp.

## Files

| Action | Path | Purpose |
|--------|------|---------|
| MODIFY | `/Users/rembrandt/loremllc/ariadne/src/dashboard/api.rs` | Add `GraphStatsResponse` struct and `graph_stats` async handler |
| MODIFY | `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs` | Register `/api/graph-stats` route in the Axum router |
| MODIFY | `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs` | Add `test_graph_stats_handler` integration test |

## Steps

### Step 1: Add `GraphStatsResponse` struct and `graph_stats` handler in `api.rs`

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/api.rs`
**Location:** After the `pub async fn coupling(...)` function definition (which ends
around line 590) and before the `fn fetch_source(...)` function. Specifically, insert
after the closing brace of `coupling` and before the line `fn fetch_source(`.

```rust
/// Response type for the `/api/graph-stats` endpoint.
///
/// Returns the total number of symbol nodes, call edges, and the timestamp of the
/// most recent indexing run in ISO 8601 format (e.g., `"2026-04-07T12:34:56Z"`).
/// `last_indexed` is `null` when no services have been indexed yet.
#[derive(Debug, Clone, Serialize)]
pub struct GraphStatsResponse {
    pub node_count: u64,
    pub edge_count: u64,
    pub last_indexed: Option<String>,
}

/// Convert a Unix epoch float (seconds since 1970-01-01T00:00:00Z) to an ISO 8601
/// datetime string in UTC, e.g., `"2026-04-07T12:34:56Z"`.
///
/// Uses only `std::time` — no `chrono` dependency required.
fn epoch_to_iso8601(epoch_secs: f64) -> String {
    // epoch_secs is seconds since Unix epoch (1970-01-01T00:00:00 UTC).
    // We perform integer division to extract year/month/day/hour/min/sec.
    let secs = epoch_secs as u64;
    // Days since Unix epoch
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;

    // Compute year, month, day from days since epoch using the proleptic Gregorian calendar.
    // Algorithm: http://howardhinnant.github.io/date_algorithms.html#civil_from_days
    let z = days as i64 + 719468;
    let era: i64 = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month prime [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hour, minute, second
    )
}

pub async fn graph_stats(
    State(db_path): State<DbState>,
) -> Result<Json<GraphStatsResponse>, ApiError> {
    let db = open_db(&db_path)?;
    let stats = crate::db::query::get_graph_stats(&db)
        .map_err(|_| ApiError::query_failed("Failed to load graph stats."))?;
    let last_indexed = stats.last_indexed.map(epoch_to_iso8601);
    Ok(Json(GraphStatsResponse {
        node_count: stats.node_count,
        edge_count: stats.edge_count,
        last_indexed,
    }))
}
```

**Verify:** `cargo build 2>&1 | grep error`
**Expected:** No output (no compile errors)

### Step 2: Register `/api/graph-stats` route in `mod.rs`

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs`
**Location:** Inside the `axum::Router::new()` chain in the `serve()` function. Add the
new route after the existing `.route("/api/coupling", ...)` line (which is currently the
last route before `.route("/graph-renderer.js", ...)`). The exact insertion is after
this existing line:

```
        .route("/api/coupling", axum::routing::get(api::coupling))
```

Insert this line immediately after it:

```rust
        .route("/api/graph-stats", axum::routing::get(api::graph_stats))
```

The complete updated router block in `serve()` will look like this:

```rust
    let app = axum::Router::new()
        .route("/api/health", axum::routing::get(health_handler))
        .route("/api/stats", axum::routing::get(api::stats))
        .route("/api/graph", axum::routing::get(api::graph_data))
        .route("/api/search", axum::routing::get(api::search_symbols))
        .route(
            "/api/graph/neighborhood",
            axum::routing::get(api::neighborhood),
        )
        .route("/api/graph/insights", axum::routing::get(api::insights))
        .route("/api/source", axum::routing::get(api::source))
        .route("/api/modules", axum::routing::get(api::modules))
        .route("/api/coupling", axum::routing::get(api::coupling))
        .route("/api/graph-stats", axum::routing::get(api::graph_stats))
        .route(
            "/graph-renderer.js",
            axum::routing::get(graph_renderer_js_handler),
        )
        .fallback(axum::routing::get(index_handler))
        .layer(cors)
        .with_state(state);
```

**Verify:** `cargo build 2>&1 | grep error`
**Expected:** No output (no compile errors)

### Step 3: Add integration test `test_graph_stats_handler`

**File:** `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs`
**Location:** After the existing `test_dashboard_coupling_handler` test function
(which ends around line 178) and before the `test_xss_regression_html_escaping` test.
Add the following import to the top of the file and the test function body.

First, add `graph_stats` and `GraphStatsResponse` (unused in pattern match, but
needed to call the handler) to the existing import at the top of the file. The
current import line is:

```rust
use ariadne::dashboard::api::{
    coupling, graph_data, modules, search_symbols, stats, CouplingQuery, DbState, SearchQuery,
};
```

Replace it with:

```rust
use ariadne::dashboard::api::{
    coupling, graph_data, graph_stats, modules, search_symbols, stats, CouplingQuery, DbState,
    SearchQuery,
};
```

Then, after the `test_dashboard_coupling_handler` function, add:

```rust
#[tokio::test]
async fn test_graph_stats_handler() {
    let (_dir, state) = setup_indexed_db();

    let result = graph_stats(State(state))
        .await
        .expect("graph_stats should succeed");
    let data = result.0;

    assert!(
        data.node_count > 0,
        "expected node_count > 0 after indexing python fixture, got {}",
        data.node_count
    );
    // edge_count is u64 so it is always non-negative; just assert it is present
    let _ = data.edge_count;
    // last_indexed is Some(...) because setup_indexed_db runs the full pipeline,
    // which writes a service row with last_indexed set.
    assert!(
        data.last_indexed.is_some(),
        "expected last_indexed to be Some after indexing, got None"
    );
    let ts = data.last_indexed.as_ref().unwrap();
    assert!(
        ts.contains('T') && ts.ends_with('Z'),
        "expected ISO 8601 timestamp like '2026-04-07T12:34:56Z', got '{}'",
        ts
    );
}
```

**Verify:** `cargo test test_graph_stats_handler -- --nocapture`
**Expected:** `test test_graph_stats_handler ... ok`

## Acceptance Criteria

- [ ] `cargo test test_graph_stats_handler -- --nocapture` → PASS (exit 0)
- [ ] `cargo test query_graph_stats_tests::test_graph_stats_empty_db` → PASS (exit 0, PRD-01 test still passes)
- [ ] `cargo clippy -- -D warnings` → exit 0
- [ ] `cargo build` → exit 0

## Types and Signatures

All public types and function signatures introduced by this mini PRD:

```rust
// New response struct in src/dashboard/api.rs
#[derive(Debug, Clone, Serialize)]
pub struct GraphStatsResponse {
    pub node_count: u64,
    pub edge_count: u64,
    pub last_indexed: Option<String>,
}

// New handler in src/dashboard/api.rs
pub async fn graph_stats(
    State(db_path): State<DbState>,
) -> Result<Json<GraphStatsResponse>, ApiError>;

// Private helper in src/dashboard/api.rs
fn epoch_to_iso8601(epoch_secs: f64) -> String;
```

## Imports

No new `use` imports are needed in `src/dashboard/api.rs` — all types (`Json`, `State`,
`DbState`, `ApiError`, `Serialize`) are already imported at the top of that file.

No new imports are needed in `src/dashboard/mod.rs`.

In `tests/test_dashboard.rs`, the existing import line is modified (not a new `use`
statement added — the existing one is updated in place):

```rust
// In /Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs
// MODIFY existing import (replace, not add):
use ariadne::dashboard::api::{
    coupling, graph_data, graph_stats, modules, search_symbols, stats, CouplingQuery, DbState,
    SearchQuery,
};
```

## Completion Contract

```
**Tests that must pass before signaling done:**
- `cargo test test_graph_stats_handler -- --nocapture` → exit 0
- `cargo test query_graph_stats_tests::test_graph_stats_empty_db` → exit 0
- `cargo clippy -- -D warnings` → exit 0

**Files this mini PRD is permitted to touch:**
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/api.rs`
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs`
- `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs`

**Completion signal:**
When every test above passes and every acceptance criterion is met, output exactly this
line and nothing after it:

PLANFORGE_COMPLETE: PRD-02 graph_stats handler, /api/graph-stats route, and integration test added

Do not output the completion signal until all tests pass.
Do not output it speculatively or as a placeholder.
```
