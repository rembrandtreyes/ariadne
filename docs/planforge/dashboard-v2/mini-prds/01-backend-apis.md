# Mini PRD 01: Backend API Endpoints (Describe, Source Modification, Health Timestamp)

> **Dependency:** none -- can execute independently
> **Produces:** `/api/describe` endpoint, modified `/api/source` with context param, `last_indexed` in `/api/health`
> **Estimated steps:** 12

## Context

This PRD adds the remaining Rust backend endpoints needed by the Dashboard v2 frontend. It creates the `/api/describe` endpoint that generates Level C narrative descriptions for symbols, modifies the existing `/api/source` endpoint to support a configurable context window and return line counts, and adds a `last_indexed` timestamp to the `/api/health` response for frontend polling. Tasks 1 and 2 (modules + coupling endpoints) are already implemented and committed.

## Files

| Action | Path | Purpose |
|--------|------|---------|
| CREATE | `/Users/rembrandt/loremllc/ariadne/src/dashboard/describe.rs` | Level C narrative description generator |
| MODIFY | `/Users/rembrandt/loremllc/ariadne/src/dashboard/api.rs` | Add `DescribeQuery`, `describe` handler; modify `SourceQuery`, `SourceResult`, `fetch_source` |
| MODIFY | `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs` | Add `pub mod describe;`, `/api/describe` route, update `health_handler` |
| MODIFY | `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs` | Add tests for describe, source full body, and integration |

## Steps

### Step 1: Create the describe module

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/describe.rs`
**Location:** New file

```rust
use crate::db::{query, Database};
use serde::Serialize;

/// Level C description result -- full narrative with architectural context.
#[derive(Debug, Serialize)]
pub struct DescribeResult {
    pub description: String,
    pub role: String,
    pub risk_level: String,
    pub risk_score: f64,
    pub metrics: DescribeMetrics,
}

#[derive(Debug, Serialize)]
pub struct DescribeMetrics {
    pub fan_in: i64,
    pub fan_out: i64,
    pub modification_count: i64,
    pub author_count: i64,
    pub is_volatile: bool,
    pub blast_radius: usize,
    pub coupled_file_count: usize,
    pub max_coupling_strength: f64,
}

/// Generate a Level C narrative description for a symbol.
///
/// Composes a natural-language explanation from structural signals:
/// callers, callees, fan-in/out, churn, coupling, blast radius, dead code status.
pub fn describe_symbol(db: &Database, symbol_id: i64) -> anyhow::Result<DescribeResult> {
    let sym = query::symbol_by_id(db, symbol_id)?
        .ok_or_else(|| anyhow::anyhow!("Symbol not found: {}", symbol_id))?;

    let file_path = query::file_path_by_id(db, sym.file_id).unwrap_or_else(|_| "unknown".into());
    let callers = query::get_dependents(db, sym.id).unwrap_or_default();
    let callees = query::get_dependencies(db, sym.id).unwrap_or_default();
    let couplings = query::get_file_couplings(db, sym.file_id).unwrap_or_default();

    // Get health data if available
    let health = query::get_symbol_health_data(db, &sym.name).ok().flatten();

    let fan_in = health
        .as_ref()
        .map(|h| h.fan_in)
        .unwrap_or(callers.len() as i64);
    let fan_out = health
        .as_ref()
        .map(|h| h.fan_out)
        .unwrap_or(callees.len() as i64);
    let modification_count = health.as_ref().map(|h| h.modification_count).unwrap_or(0);
    let author_count = health.as_ref().map(|h| h.author_count).unwrap_or(0);
    let is_volatile = health.as_ref().map(|h| h.is_volatile).unwrap_or(false);
    let is_dead = sym.is_dead;

    let coupled_file_count = couplings.len();
    let max_coupling_strength = couplings
        .iter()
        .map(|c| c.strength)
        .fold(0.0_f64, f64::max);

    // Compute a simple risk score
    let fan_in_score = (fan_in as f64 / 20.0).min(1.0);
    let churn_score = if is_volatile {
        0.8
    } else {
        (modification_count as f64 / 30.0).min(1.0)
    };
    let coupling_score = max_coupling_strength;
    let dead_score = if is_dead { 0.5 } else { 0.0 };
    let risk_score =
        (fan_in_score * 0.3 + churn_score * 0.3 + coupling_score * 0.2 + dead_score * 0.2)
            .min(1.0);

    // Determine role from file path and kind
    let module_name = extract_module(&file_path);
    let role = infer_role(&sym.kind, &module_name, fan_in, is_dead);

    // Determine risk level
    let risk_level = if risk_score >= 0.8 {
        "critical"
    } else if risk_score >= 0.6 {
        "high"
    } else if risk_score >= 0.4 {
        "medium"
    } else {
        "low"
    };

    // Build the description
    let description = build_narrative(
        &sym.name,
        &sym.kind,
        &module_name,
        &file_path,
        &callers,
        &callees,
        fan_in,
        fan_out,
        modification_count,
        is_volatile,
        is_dead,
        &couplings,
        risk_score,
    );

    Ok(DescribeResult {
        description,
        role,
        risk_level: risk_level.to_string(),
        risk_score,
        metrics: DescribeMetrics {
            fan_in,
            fan_out,
            modification_count,
            author_count,
            is_volatile,
            blast_radius: 0,
            coupled_file_count,
            max_coupling_strength,
        },
    })
}

fn extract_module(file_path: &str) -> String {
    let path = file_path.strip_prefix("src/").unwrap_or(file_path);
    match path.split('/').next() {
        Some(first) if path.contains('/') => first.to_string(),
        _ => "root".to_string(),
    }
}

fn infer_role(kind: &str, module: &str, fan_in: i64, is_dead: bool) -> String {
    if is_dead {
        return "unreachable".to_string();
    }
    if fan_in == 0 {
        return "entry_point".to_string();
    }
    match module {
        "pipeline" => "core_pipeline".to_string(),
        "parse" => "parser".to_string(),
        "db" => "data_access".to_string(),
        "graph" => "graph_analysis".to_string(),
        "mcp" => "mcp_tool".to_string(),
        "analysis" => "analysis".to_string(),
        "dashboard" => "dashboard_api".to_string(),
        "search" => "search".to_string(),
        _ => format!("{}_{}", module, kind),
    }
}

fn build_narrative(
    name: &str,
    kind: &str,
    module: &str,
    file_path: &str,
    callers: &[query::SymbolRow],
    callees: &[query::SymbolRow],
    fan_in: i64,
    fan_out: i64,
    modification_count: i64,
    is_volatile: bool,
    is_dead: bool,
    couplings: &[query::CouplingRow],
    risk_score: f64,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    let kind_label = match kind {
        "function" => "function",
        "method" => "method",
        "class" => "class",
        "interface" => "interface",
        _ => "symbol",
    };

    if is_dead {
        parts.push(format!(
            "{} is an unreachable {} in {} ({}). No code path leads to it -- safe to remove.",
            name, kind_label, module, file_path
        ));
        return parts.join(" ");
    }

    parts.push(format!(
        "{} is a {} in the {} module ({}).",
        name, kind_label, module, file_path
    ));

    // Callers context
    if !callers.is_empty() {
        let caller_names: Vec<&str> = callers.iter().take(3).map(|c| c.name.as_str()).collect();
        if callers.len() <= 3 {
            parts.push(format!("It is called by {}.", caller_names.join(", ")));
        } else {
            parts.push(format!(
                "It is called by {} and {} others ({} total callers).",
                caller_names.join(", "),
                callers.len() - 3,
                callers.len()
            ));
        }
    } else {
        parts.push(
            "It has no known callers -- it may be an entry point or unused.".to_string(),
        );
    }

    // Callees context
    if !callees.is_empty() {
        let callee_names: Vec<&str> = callees.iter().take(3).map(|c| c.name.as_str()).collect();
        if callees.len() <= 3 {
            parts.push(format!("It depends on {}.", callee_names.join(", ")));
        } else {
            parts.push(format!(
                "It depends on {} and {} others.",
                callee_names.join(", "),
                callees.len() - 3
            ));
        }
    }

    // Risk assessment
    if risk_score >= 0.8 {
        let mut risk_reasons = Vec::new();
        if fan_in > 15 {
            risk_reasons.push(format!("{} incoming dependencies", fan_in));
        }
        if is_volatile || modification_count > 20 {
            risk_reasons.push("high modification frequency".to_string());
        }
        if !couplings.is_empty() {
            risk_reasons.push(format!("coupled with {} other files", couplings.len()));
        }
        if !risk_reasons.is_empty() {
            parts.push(format!(
                "This is a critical risk point: {}.",
                risk_reasons.join(", ")
            ));
        }
    } else if risk_score >= 0.5 {
        parts.push(format!(
            "With {} callers and {} callees, this is a moderately connected symbol.",
            fan_in, fan_out
        ));
    }

    // Coupling context
    if let Some(strongest) = couplings.first() {
        if strongest.strength > 0.7 {
            parts.push(format!(
                "Tightly coupled with {} (strength {:.2}) -- changes to one often require changes to the other.",
                strongest.coupled_path, strongest.strength
            ));
        }
    }

    parts.join(" ")
}
```

**Verify:** File is created and contains no syntax errors
**Expected:** File exists at path

### Step 2: Register the describe module in mod.rs

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs`
**Location:** At the top of the file, after `pub mod api;`

Add this line immediately after `pub mod api;`:

```rust
pub mod describe;
```

**Verify:** `cargo check`
**Expected:** Compiles without errors

### Step 3: Add the DescribeQuery and describe handler in api.rs

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/api.rs`
**Location:** After the `coupling` handler function (after the closing brace of `pub async fn coupling`)

Add these items:

```rust
#[derive(Deserialize)]
pub struct DescribeQuery {
    pub id: i64,
}

pub async fn describe(
    State(db_path): State<DbState>,
    Query(query): Query<DescribeQuery>,
) -> Result<Json<crate::dashboard::describe::DescribeResult>, ApiError> {
    let db = open_db(&db_path)?;
    let result = crate::dashboard::describe::describe_symbol(&db, query.id)
        .map_err(|_| ApiError::query_failed("Failed to generate description."))?;
    Ok(Json(result))
}
```

**Verify:** `cargo check`
**Expected:** Compiles without errors

### Step 4: Add the /api/describe route in mod.rs

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs`
**Location:** In the `serve` function's router chain, after the `.route("/api/coupling", ...)` line

Add this route:

```rust
        .route("/api/describe", axum::routing::get(api::describe))
```

**Verify:** `cargo check`
**Expected:** Compiles without errors

### Step 5: Modify SourceQuery to add context parameter

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/api.rs`
**Location:** Find the existing `SourceQuery` struct and replace it

Replace:
```rust
#[derive(Deserialize)]
pub struct SourceQuery {
    pub id: i64,
}
```

With:
```rust
#[derive(Deserialize)]
pub struct SourceQuery {
    pub id: i64,
    pub context: Option<u32>,
}
```

**Verify:** `cargo check`
**Expected:** Compiles without errors

### Step 6: Add line_count to SourceResult

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/api.rs`
**Location:** Find the existing `SourceResult` struct and replace it

Replace:
```rust
#[derive(Serialize)]
pub struct SourceResult {
    pub code: String,
    pub line_start: u32,
    pub line_end: u32,
    pub language: String,
    pub file: String,
}
```

With:
```rust
#[derive(Serialize)]
pub struct SourceResult {
    pub code: String,
    pub line_start: u32,
    pub line_end: u32,
    pub line_count: u32,
    pub language: String,
    pub file: String,
}
```

**Verify:** `cargo check`
**Expected:** Compiles (will have error in `fetch_source` until next step)

### Step 7: Update the source handler and fetch_source function

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/api.rs`
**Location:** Replace the existing `source` async handler function

Replace:
```rust
pub async fn source(
    State(db_path): State<DbState>,
    Query(query): Query<SourceQuery>,
) -> Result<Json<SourceResult>, ApiError> {
    let db = open_db(&db_path)?;
    let result = fetch_source(&db, query.id)
        .map_err(|_| ApiError::query_failed("Failed to fetch source code."))?;
    Ok(Json(result))
}
```

With:
```rust
pub async fn source(
    State(db_path): State<DbState>,
    Query(query): Query<SourceQuery>,
) -> Result<Json<SourceResult>, ApiError> {
    let db = open_db(&db_path)?;
    let context = query.context.unwrap_or(0);
    let result = fetch_source(&db, query.id, context)
        .map_err(|_| ApiError::query_failed("Failed to fetch source code."))?;
    Ok(Json(result))
}
```

Then replace the `fetch_source` function signature and body. Find:
```rust
fn fetch_source(db: &Database, symbol_id: i64) -> anyhow::Result<SourceResult> {
```

Replace with:
```rust
fn fetch_source(db: &Database, symbol_id: i64, context: u32) -> anyhow::Result<SourceResult> {
```

Then in the body of `fetch_source`, replace the context and return section. Find:
```rust
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
```

Replace with:
```rust
    let start_idx = line_start.saturating_sub(context + 1) as usize;
    let end_idx = (line_end + context).min(total) as usize;

    let code = all_lines[start_idx..end_idx].join("\n");
    let line_count = (end_idx - start_idx) as u32;

    Ok(SourceResult {
        code,
        line_start,
        line_end,
        line_count,
        language,
        file: file_path,
    })
```

**Verify:** `cargo check`
**Expected:** Compiles without errors

### Step 8: Add last_indexed to the health handler

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs`
**Location:** Replace the existing `health_handler` function

Replace:
```rust
async fn health_handler(
    State(db_path): State<api::DbState>,
) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    let db_ok = crate::db::Database::open(db_path.as_ref()).is_ok();
    let status = if db_ok {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        axum::Json(serde_json::json!({
            "status": if db_ok { "ok" } else { "error" },
            "version": env!("CARGO_PKG_VERSION"),
            "db": if db_ok { "connected" } else { "unavailable" },
        })),
    )
}
```

With:
```rust
async fn health_handler(
    State(db_path): State<api::DbState>,
) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    let db_result = crate::db::Database::open(db_path.as_ref());
    let db_ok = db_result.is_ok();
    let last_indexed = db_result
        .ok()
        .and_then(|db| db.get_metadata("last_indexed").ok().flatten())
        .unwrap_or_default();

    let status = if db_ok {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        axum::Json(serde_json::json!({
            "status": if db_ok { "ok" } else { "error" },
            "version": env!("CARGO_PKG_VERSION"),
            "db": if db_ok { "connected" } else { "unavailable" },
            "last_indexed": last_indexed,
        })),
    )
}
```

Note: The `Database` struct already has a `get_metadata(key)` method that queries the `metadata` table. No new query function in `query.rs` is needed.

**Verify:** `cargo check`
**Expected:** Compiles without errors

### Step 9: Add the describe handler test

**File:** `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs`
**Location:** At the end of the file

First, update the imports at the top of the file. The existing import block is:
```rust
use ariadne::dashboard::api::{
    coupling, graph_data, modules, search_symbols, stats, CouplingQuery, DbState, SearchQuery,
};
```

Replace it with:
```rust
use ariadne::dashboard::api::{
    coupling, describe, graph_data, modules, search_symbols, source, stats, CouplingQuery,
    DbState, DescribeQuery, SearchQuery, SourceQuery,
};
```

Then add this test at the end of the file:

```rust
#[tokio::test]
async fn test_dashboard_describe_handler() {
    let (_dir, state) = setup_indexed_db();

    // First find a symbol ID via search
    let query = SearchQuery {
        q: Some("greet".to_string()),
    };
    let search_result = search_symbols(State(state.clone()), Query(query))
        .await
        .expect("search should succeed");
    let results = &search_result.0;
    assert!(!results.is_empty(), "need at least one symbol to describe");

    let symbol_id: i64 = results[0].id.parse().expect("id should be numeric");

    let desc_query = DescribeQuery { id: symbol_id };
    let result = describe(State(state), Query(desc_query))
        .await
        .expect("describe should succeed");
    let data = result.0;

    assert!(
        !data.description.is_empty(),
        "description should not be empty"
    );
    assert!(
        data.risk_score >= 0.0 && data.risk_score <= 1.0,
        "risk_score should be 0-1, got {}",
        data.risk_score
    );
}
```

**Verify:** `cargo test test_dashboard_describe_handler -- --nocapture`
**Expected:** PASS

### Step 10: Add the source full body test

**File:** `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs`
**Location:** After the test added in Step 9

```rust
#[tokio::test]
async fn test_dashboard_source_full_body() {
    let (_dir, state) = setup_indexed_db();

    // Find a symbol to get source for
    let query = SearchQuery {
        q: Some("greet".to_string()),
    };
    let search_result = search_symbols(State(state.clone()), Query(query))
        .await
        .expect("search should succeed");
    let results = &search_result.0;
    assert!(!results.is_empty());

    let symbol_id: i64 = results[0].id.parse().unwrap();
    let source_query = SourceQuery {
        id: symbol_id,
        context: Some(0),
    };
    let result = source(State(state), Query(source_query))
        .await
        .expect("source should succeed");
    let data = result.0;

    assert!(!data.code.is_empty(), "source code should not be empty");
    assert!(data.line_count > 0, "line_count should be > 0");
    assert!(
        data.line_start <= data.line_end,
        "line_start should be <= line_end"
    );
}
```

**Verify:** `cargo test test_dashboard_source_full_body -- --nocapture`
**Expected:** PASS

### Step 11: Add the v2 integration test

**File:** `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs`
**Location:** After the test added in Step 10

NOTE: This test is named `test_dashboard_v2_endpoints_basic` to avoid a duplicate function name — PRD-05 adds a more comprehensive test also named `test_dashboard_v2_all_endpoints` that supersedes this one with additional `line_count` assertions.

```rust
#[tokio::test]
async fn test_dashboard_v2_endpoints_basic() {
    let (_dir, state) = setup_indexed_db();

    // Stats
    let stats_result = stats(State(state.clone())).await;
    assert!(stats_result.is_ok(), "stats endpoint failed");

    // Modules
    let modules_result = modules(State(state.clone())).await;
    assert!(modules_result.is_ok(), "modules endpoint failed");

    // Insights
    let insights_result =
        ariadne::dashboard::api::insights(State(state.clone())).await;
    assert!(insights_result.is_ok(), "insights endpoint failed");

    // Coupling
    let coupling_query = CouplingQuery { limit: Some(5) };
    let coupling_result = coupling(State(state.clone()), Query(coupling_query)).await;
    assert!(coupling_result.is_ok(), "coupling endpoint failed");

    // Search -> Describe chain
    let search_query = SearchQuery {
        q: Some("greet".to_string()),
    };
    let search_result = search_symbols(State(state.clone()), Query(search_query))
        .await
        .unwrap();
    if !search_result.0.is_empty() {
        let id: i64 = search_result.0[0].id.parse().unwrap();

        let desc_query = DescribeQuery { id };
        let desc_result = describe(State(state.clone()), Query(desc_query)).await;
        assert!(desc_result.is_ok(), "describe endpoint failed");

        let source_query = SourceQuery {
            id,
            context: Some(0),
        };
        let source_result = source(State(state.clone()), Query(source_query)).await;
        assert!(source_result.is_ok(), "source endpoint failed");
    }
}
```

**Verify:** `cargo test test_dashboard_v2_endpoints_basic -- --nocapture`
**Expected:** PASS

### Step 12: Run full test suite and clippy

**Verify:** `cargo test`
**Expected:** All tests PASS

**Verify:** `cargo clippy -- -D warnings`
**Expected:** No warnings in new code

## Acceptance Criteria

- [ ] `cargo test test_dashboard_describe_handler` -> PASS
- [ ] `cargo test test_dashboard_source_full_body` -> PASS
- [ ] `cargo test test_dashboard_v2_endpoints_basic` -> PASS
- [ ] `cargo test` -> ALL PASS (no regressions)
- [ ] `cargo clippy -- -D warnings` -> no warnings

## Types and Signatures

```rust
// In src/dashboard/describe.rs
pub struct DescribeResult {
    pub description: String,
    pub role: String,
    pub risk_level: String,
    pub risk_score: f64,
    pub metrics: DescribeMetrics,
}

pub struct DescribeMetrics {
    pub fan_in: i64,
    pub fan_out: i64,
    pub modification_count: i64,
    pub author_count: i64,
    pub is_volatile: bool,
    pub blast_radius: usize,
    pub coupled_file_count: usize,
    pub max_coupling_strength: f64,
}

pub fn describe_symbol(db: &Database, symbol_id: i64) -> anyhow::Result<DescribeResult>

// In src/dashboard/api.rs (new)
pub struct DescribeQuery { pub id: i64 }
pub async fn describe(State(db_path): State<DbState>, Query(query): Query<DescribeQuery>) -> Result<Json<DescribeResult>, ApiError>

// In src/dashboard/api.rs (modified)
pub struct SourceQuery { pub id: i64, pub context: Option<u32> }
pub struct SourceResult { pub code: String, pub line_start: u32, pub line_end: u32, pub line_count: u32, pub language: String, pub file: String }
```

## Imports

```rust
// In src/dashboard/describe.rs
use crate::db::{query, Database};
use serde::Serialize;

// In tests/test_dashboard.rs (updated import line)
use ariadne::dashboard::api::{
    coupling, describe, graph_data, modules, search_symbols, source, stats, CouplingQuery,
    DbState, DescribeQuery, SearchQuery, SourceQuery,
};
```

## Completion Contract

**Tests that must pass before signaling done:**
- `cargo test test_dashboard_describe_handler` -> exit 0
- `cargo test test_dashboard_source_full_body` -> exit 0
- `cargo test test_dashboard_v2_endpoints_basic` -> exit 0
- `cargo test` -> exit 0
- `cargo clippy -- -D warnings` -> exit 0

**Files this mini PRD is permitted to touch:**
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/describe.rs` (CREATE)
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/api.rs`
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs`
- `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs`

**Completion signal:**
PLANFORGE_COMPLETE: PRD-01 Backend APIs -- describe endpoint, source modification, health timestamp
