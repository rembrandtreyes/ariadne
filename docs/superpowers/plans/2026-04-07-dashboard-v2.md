# Dashboard v2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the existing graph-only dashboard with a two-view intelligence product (Signal + Void) that helps vibe coders understand codebases through narrative descriptions, spatial architecture, and drill-down exploration.

**Architecture:** The dashboard is a vanilla HTML/CSS/JS SPA embedded in the Rust binary via `include_str!`. Three new API endpoints (`/api/describe`, `/api/modules`, `/api/coupling`) provide module-level aggregation and Level C narrative descriptions. The frontend is split into 6 JS files (signal, void-renderer, detail-panel, search, source-modal, style.css) served from the existing Axum server. Navigation is drill-down: Signal is the landing page, clicking items transitions to Void.

**Tech Stack:** Rust (Axum, rusqlite, serde), Vanilla JS/CSS, SVG for connections

**Spec:** `docs/superpowers/specs/2026-04-07-dashboard-v2-design.md`

---

## Task 1: New API — `/api/modules` endpoint

Adds module-level aggregation grouped by top-level directory. This is the data backbone for both Signal module cards and Void module nodes.

**Files:**
- Modify: `src/db/query.rs` (add `get_module_summaries` function)
- Modify: `src/dashboard/api.rs` (add `ModuleSummary`, `ModuleFile`, `ModulesResponse` structs and `modules` handler)
- Modify: `src/dashboard/mod.rs` (add `/api/modules` route)
- Test: `tests/test_dashboard.rs` (add `test_dashboard_modules_handler`)

- [ ] **Step 1: Write the failing test**

Add to `tests/test_dashboard.rs`:

```rust
#[tokio::test]
async fn test_dashboard_modules_handler() {
    let (_dir, state) = setup_indexed_db();

    let result = ariadne::dashboard::api::modules(axum::extract::State(state))
        .await
        .expect("modules should succeed");
    let data = result.0;

    assert!(
        !data.modules.is_empty(),
        "expected at least one module, got empty"
    );

    let first = &data.modules[0];
    assert!(!first.name.is_empty(), "module name should not be empty");
    assert!(first.symbol_count > 0, "module should have symbols");
    assert!(first.file_count > 0, "module should have files");
    assert!(
        first.health >= 0.0 && first.health <= 1.0,
        "health should be 0-1, got {}",
        first.health
    );
    assert!(
        first.risk >= 0.0 && first.risk <= 1.0,
        "risk should be 0-1, got {}",
        first.risk
    );
    assert!(
        !first.files.is_empty(),
        "module should have file-level breakdown"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_dashboard_modules_handler -- --nocapture`
Expected: FAIL — `modules` function does not exist

- [ ] **Step 3: Add query function in `src/db/query.rs`**

Add at the end of the file, before the closing:

```rust
/// Module-level summary for dashboard, grouped by top-level directory.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleFileSummary {
    pub name: String,
    pub symbol_count: u64,
    pub dead_count: u64,
    pub risk: f64,
    pub health: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleSummary {
    pub name: String,
    pub path: String,
    pub symbol_count: u64,
    pub file_count: u64,
    pub health: f64,
    pub risk: f64,
    pub dead_count: u64,
    pub cycle_count: u64,
    pub god_objects: u64,
    pub files: Vec<ModuleFileSummary>,
}

/// Build module summaries grouped by top-level source directory.
///
/// Groups files by the first directory component of their path (e.g., "src/pipeline/foo.rs" → "pipeline").
/// Computes per-module symbol counts, dead code counts, and per-file breakdowns.
pub fn get_module_summaries(db: &Database) -> anyhow::Result<Vec<ModuleSummary>> {
    let conn = db.conn();

    // Get all files with their symbol counts and dead counts
    let mut stmt = conn.prepare(
        "SELECT f.id, f.path,
                COUNT(s.id) as sym_count,
                SUM(CASE WHEN s.is_dead = 1 THEN 1 ELSE 0 END) as dead_count
         FROM files f
         LEFT JOIN symbols s ON s.file_id = f.id
         GROUP BY f.id
         ORDER BY f.path",
    )?;

    struct FileInfo {
        _id: i64,
        path: String,
        sym_count: u64,
        dead_count: u64,
    }

    let file_infos: Vec<FileInfo> = stmt
        .query_map([], |row| {
            Ok(FileInfo {
                _id: row.get(0)?,
                path: row.get(1)?,
                sym_count: row.get(2)?,
                dead_count: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Group by module (first directory component after any "src/" prefix)
    let mut module_map: std::collections::BTreeMap<String, Vec<&FileInfo>> =
        std::collections::BTreeMap::new();

    for fi in &file_infos {
        let path = fi.path.strip_prefix("src/").unwrap_or(&fi.path);
        let module_name = match path.split('/').next() {
            Some(first) if path.contains('/') => first.to_string(),
            _ => "root".to_string(),
        };
        module_map.entry(module_name).or_default().push(fi);
    }

    // Build module summaries
    let mut modules = Vec::new();
    for (name, files) in &module_map {
        let symbol_count: u64 = files.iter().map(|f| f.sym_count).sum();
        let dead_count: u64 = files.iter().map(|f| f.dead_count).sum();
        let file_count = files.len() as u64;

        let file_summaries: Vec<ModuleFileSummary> = files
            .iter()
            .filter(|f| f.sym_count > 0)
            .map(|f| {
                let dead_ratio = if f.sym_count > 0 {
                    f.dead_count as f64 / f.sym_count as f64
                } else {
                    0.0
                };
                let file_risk = dead_ratio.min(1.0);
                ModuleFileSummary {
                    name: f
                        .path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&f.path)
                        .to_string(),
                    symbol_count: f.sym_count,
                    dead_count: f.dead_count,
                    risk: file_risk,
                    health: (1.0 - file_risk).max(0.0),
                }
            })
            .collect();

        let dead_ratio = if symbol_count > 0 {
            dead_count as f64 / symbol_count as f64
        } else {
            0.0
        };
        let module_risk = dead_ratio.min(1.0);

        modules.push(ModuleSummary {
            name: name.clone(),
            path: format!("src/{}", name),
            symbol_count,
            file_count,
            health: (1.0 - module_risk).max(0.0),
            risk: module_risk,
            dead_count,
            cycle_count: 0, // Enhanced in a later task with insights cross-reference
            god_objects: 0,  // Enhanced in a later task
            files: file_summaries,
        });
    }

    // Sort by symbol count descending (largest modules first)
    modules.sort_by(|a, b| b.symbol_count.cmp(&a.symbol_count));

    Ok(modules)
}
```

- [ ] **Step 4: Add API handler in `src/dashboard/api.rs`**

Add the response structs and handler after the `source` function:

```rust
#[derive(Serialize)]
pub struct ModulesResponse {
    pub modules: Vec<crate::db::query::ModuleSummary>,
}

pub async fn modules(State(db_path): State<DbState>) -> Result<Json<ModulesResponse>, ApiError> {
    let db = open_db(&db_path)?;
    let mods = crate::db::query::get_module_summaries(&db)
        .map_err(|_| ApiError::query_failed("Failed to build module summaries."))?;
    Ok(Json(ModulesResponse { modules: mods }))
}
```

- [ ] **Step 5: Add route in `src/dashboard/mod.rs`**

Add to the router chain in the `serve` function, after the `/api/source` route:

```rust
.route("/api/modules", axum::routing::get(api::modules))
```

- [ ] **Step 6: Run tests to verify it passes**

Run: `cargo test test_dashboard_modules_handler -- --nocapture`
Expected: PASS

- [ ] **Step 7: Run full test suite for regressions**

Run: `cargo test`
Expected: All existing tests PASS

- [ ] **Step 8: Commit**

```bash
git add src/db/query.rs src/dashboard/api.rs src/dashboard/mod.rs tests/test_dashboard.rs
git commit -m "feat(dashboard): add /api/modules endpoint for module-level aggregation"
```

---

## Task 2: New API — `/api/coupling` endpoint

Returns top N coupled file pairs with module-level grouping and cycle detection.

**Files:**
- Modify: `src/db/query.rs` (add `get_top_couplings` function)
- Modify: `src/dashboard/api.rs` (add `CouplingPair`, `CouplingResponse` structs and handler)
- Modify: `src/dashboard/mod.rs` (add route)
- Test: `tests/test_dashboard.rs` (add test)

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn test_dashboard_coupling_handler() {
    let (_dir, state) = setup_indexed_db();

    let query = ariadne::dashboard::api::CouplingQuery { limit: Some(10) };
    let result = ariadne::dashboard::api::coupling(
        axum::extract::State(state),
        axum::extract::Query(query),
    )
    .await
    .expect("coupling should succeed");
    let data = result.0;

    // The python fixture may not have coupling data (requires git history),
    // but the endpoint should return successfully with an empty list
    assert!(
        data.pairs.len() <= 10,
        "should respect the limit parameter"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_dashboard_coupling_handler -- --nocapture`
Expected: FAIL — `coupling` function does not exist

- [ ] **Step 3: Add query function in `src/db/query.rs`**

```rust
/// A coupling pair with module-level grouping for the dashboard.
#[derive(Debug, Clone, Serialize)]
pub struct CouplingPairSummary {
    pub from_module: String,
    pub to_module: String,
    pub from_file: String,
    pub to_file: String,
    pub strength: f64,
    pub co_changes: i32,
    pub is_cycle: bool,
}

/// Get top N coupled file pairs, annotated with module names and cycle info.
pub fn get_top_couplings(db: &Database, limit: i64) -> anyhow::Result<Vec<CouplingPairSummary>> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT fa.path, fb.path, c.co_changes, c.strength
         FROM coupling c
         JOIN files fa ON c.file_a_id = fa.id
         JOIN files fb ON c.file_b_id = fb.id
         ORDER BY c.strength DESC
         LIMIT ?1",
    )?;

    let pairs: Vec<CouplingPairSummary> = stmt
        .query_map(params![limit], |row| {
            let path_a: String = row.get(0)?;
            let path_b: String = row.get(1)?;
            let co_changes: i32 = row.get(2)?;
            let strength: f64 = row.get(3)?;

            // Extract module name from path
            let mod_a = extract_module_name(&path_a);
            let mod_b = extract_module_name(&path_b);

            Ok(CouplingPairSummary {
                from_module: mod_a,
                to_module: mod_b,
                from_file: path_a,
                to_file: path_b,
                strength,
                co_changes,
                is_cycle: false, // Set below after cross-referencing
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(pairs)
}

/// Extract module name from a file path (e.g., "src/pipeline/foo.rs" → "pipeline").
fn extract_module_name(path: &str) -> String {
    let path = path.strip_prefix("src/").unwrap_or(path);
    match path.split('/').next() {
        Some(first) if path.contains('/') => first.to_string(),
        _ => "root".to_string(),
    }
}
```

- [ ] **Step 4: Add API handler in `src/dashboard/api.rs`**

```rust
#[derive(Deserialize)]
pub struct CouplingQuery {
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct CouplingResponse {
    pub pairs: Vec<crate::db::query::CouplingPairSummary>,
}

pub async fn coupling(
    State(db_path): State<DbState>,
    Query(query): Query<CouplingQuery>,
) -> Result<Json<CouplingResponse>, ApiError> {
    let db = open_db(&db_path)?;
    let limit = query.limit.unwrap_or(10).min(50);
    let pairs = crate::db::query::get_top_couplings(&db, limit)
        .map_err(|_| ApiError::query_failed("Failed to load coupling data."))?;
    Ok(Json(CouplingResponse { pairs }))
}
```

- [ ] **Step 5: Add route in `src/dashboard/mod.rs`**

```rust
.route("/api/coupling", axum::routing::get(api::coupling))
```

- [ ] **Step 6: Run tests**

Run: `cargo test test_dashboard_coupling_handler -- --nocapture`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/db/query.rs src/dashboard/api.rs src/dashboard/mod.rs tests/test_dashboard.rs
git commit -m "feat(dashboard): add /api/coupling endpoint for coupling pairs"
```

---

## Task 3: New API — `/api/describe` endpoint (Level C descriptions)

Template-based narrative descriptions for symbols. The core differentiator for vibe coders.

**Files:**
- Create: `src/dashboard/describe.rs`
- Modify: `src/dashboard/mod.rs` (add `pub mod describe;` and route)
- Modify: `src/dashboard/api.rs` (add handler)
- Test: `tests/test_dashboard.rs`

- [ ] **Step 1: Write the failing test**

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

    let desc_query = ariadne::dashboard::api::DescribeQuery { id: symbol_id };
    let result = ariadne::dashboard::api::describe(
        axum::extract::State(state),
        axum::extract::Query(desc_query),
    )
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

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_dashboard_describe_handler -- --nocapture`
Expected: FAIL — `describe` function does not exist

- [ ] **Step 3: Create `src/dashboard/describe.rs`**

```rust
use crate::db::{query, Database};
use serde::Serialize;

/// Level C description result — full narrative with architectural context.
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
    // Load health data
    let sym = query::find_symbol_by_id(db, symbol_id)?
        .ok_or_else(|| anyhow::anyhow!("Symbol not found: {}", symbol_id))?;

    let file_path = query::file_path_by_id(db, sym.file_id).unwrap_or_else(|_| "unknown".into());
    let callers = query::get_dependents(db, sym.id).unwrap_or_default();
    let callees = query::get_dependencies(db, sym.id).unwrap_or_default();
    let couplings = query::get_file_couplings(db, sym.file_id).unwrap_or_default();

    // Get health data if available
    let health = query::get_symbol_health_data(db, &sym.name).ok().flatten();

    let fan_in = health.as_ref().map(|h| h.fan_in).unwrap_or(callers.len() as i64);
    let fan_out = health.as_ref().map(|h| h.fan_out).unwrap_or(callees.len() as i64);
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
    let churn_score = if is_volatile { 0.8 } else { (modification_count as f64 / 30.0).min(1.0) };
    let coupling_score = max_coupling_strength;
    let dead_score = if is_dead { 0.5 } else { 0.0 };
    let risk_score = (fan_in_score * 0.3 + churn_score * 0.3 + coupling_score * 0.2 + dead_score * 0.2)
        .min(1.0);

    // Determine role from file path and kind
    let module_name = extract_module(&file_path);
    let role = infer_role(&sym.kind, &module_name, &file_path, fan_in, is_dead);

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
            blast_radius: 0, // Requires CallGraph — enhanced in future task
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

fn infer_role(kind: &str, module: &str, _file_path: &str, fan_in: i64, is_dead: bool) -> String {
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

    // Opening: what it is and where it lives
    let kind_label = match kind {
        "function" => "function",
        "method" => "method",
        "class" => "class",
        "interface" => "interface",
        _ => "symbol",
    };

    if is_dead {
        parts.push(format!(
            "{} is an unreachable {} in {} ({}). No code path leads to it — safe to remove.",
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
            parts.push(format!(
                "It is called by {}.",
                caller_names.join(", ")
            ));
        } else {
            parts.push(format!(
                "It is called by {} and {} others ({} total callers).",
                caller_names.join(", "),
                callers.len() - 3,
                callers.len()
            ));
        }
    } else {
        parts.push("It has no known callers — it may be an entry point or unused.".to_string());
    }

    // Callees context
    if !callees.is_empty() {
        let callee_names: Vec<&str> = callees.iter().take(3).map(|c| c.name.as_str()).collect();
        if callees.len() <= 3 {
            parts.push(format!(
                "It depends on {}.",
                callee_names.join(", ")
            ));
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
                "Tightly coupled with {} (strength {:.2}) — changes to one often require changes to the other.",
                strongest.coupled_path, strongest.strength
            ));
        }
    }

    parts.join(" ")
}
```

- [ ] **Step 4: Register the module in `src/dashboard/mod.rs`**

Add at the top of the file:

```rust
pub mod describe;
```

- [ ] **Step 5: Add the `find_symbol_by_id` query function**

The describe module needs to look up symbols by integer ID. Add to `src/db/query.rs`:

```rust
/// Find a symbol by its integer ID.
pub fn find_symbol_by_id(db: &Database, id: i64) -> anyhow::Result<Option<SymbolRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT s.id, s.name, s.qualified_name, s.kind, s.file_id,
                s.line_start, s.line_end, s.is_dead, s.is_test, s.is_exported,
                s.is_entry_point, s.signature
         FROM symbols s WHERE s.id = ?1",
    )?;
    let result = stmt
        .query_row(params![id], |row| {
            Ok(SymbolRow {
                id: row.get(0)?,
                name: row.get(1)?,
                qualified_name: row.get(2)?,
                kind: row.get(3)?,
                file_id: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
                is_dead: row.get(7)?,
                is_test: row.get(8).unwrap_or(false),
                is_exported: row.get(9).unwrap_or(false),
                is_entry_point: row.get(10).unwrap_or(false),
                signature: row.get(11).unwrap_or_default(),
            })
        })
        .optional()?;
    Ok(result)
}
```

Note: Check the existing `SymbolRow` struct fields first and adjust the query columns to match. The struct likely already exists — the columns above may not all be in the struct. Match whatever fields `SymbolRow` has. If it doesn't have `is_exported`/`is_entry_point`/`signature`, drop those columns from the query.

- [ ] **Step 6: Add API handler in `src/dashboard/api.rs`**

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

- [ ] **Step 7: Add route in `src/dashboard/mod.rs`**

```rust
.route("/api/describe", axum::routing::get(api::describe))
```

- [ ] **Step 8: Run tests**

Run: `cargo test test_dashboard_describe_handler -- --nocapture`
Expected: PASS

- [ ] **Step 9: Run full test suite**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 10: Commit**

```bash
git add src/dashboard/describe.rs src/dashboard/api.rs src/dashboard/mod.rs src/db/query.rs tests/test_dashboard.rs
git commit -m "feat(dashboard): add /api/describe endpoint with Level C narrative descriptions"
```

---

## Task 4: Modify `/api/source` endpoint

Change the source endpoint to return full function bodies instead of 3-line context windows.

**Files:**
- Modify: `src/dashboard/api.rs` (update `SourceQuery`, `SourceResult`, `fetch_source`)
- Test: `tests/test_dashboard.rs`

- [ ] **Step 1: Write the failing test**

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
    let source_query = ariadne::dashboard::api::SourceQuery {
        id: symbol_id,
        context: Some(0),
    };
    let result = ariadne::dashboard::api::source(
        axum::extract::State(state),
        axum::extract::Query(source_query),
    )
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

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_dashboard_source_full_body -- --nocapture`
Expected: FAIL — `SourceQuery` doesn't have a `context` field, `SourceResult` doesn't have `line_count`

- [ ] **Step 3: Modify structs and `fetch_source` in `src/dashboard/api.rs`**

Update `SourceQuery`:
```rust
#[derive(Deserialize)]
pub struct SourceQuery {
    pub id: i64,
    pub context: Option<u32>,
}
```

Update `SourceResult`:
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

Update `source` handler to pass context:
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

Update `fetch_source` to accept context parameter:
```rust
fn fetch_source(db: &Database, symbol_id: i64, context: u32) -> anyhow::Result<SourceResult> {
```

And change the line extraction:
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

- [ ] **Step 4: Run tests**

Run: `cargo test test_dashboard_source -- --nocapture`
Expected: PASS (both old and new tests)

- [ ] **Step 5: Commit**

```bash
git add src/dashboard/api.rs tests/test_dashboard.rs
git commit -m "feat(dashboard): modify /api/source to support full function body and context parameter"
```

---

## Task 5: Add `last_indexed` to `/api/health`

Enable the frontend to detect re-indexing.

**Files:**
- Modify: `src/dashboard/mod.rs` (update `health_handler`)
- Modify: `src/db/query.rs` (add `get_last_indexed` function)

- [ ] **Step 1: Add query function in `src/db/query.rs`**

```rust
/// Get the last indexed timestamp from metadata, if available.
pub fn get_last_indexed(db: &Database) -> anyhow::Result<Option<String>> {
    let result = db
        .conn()
        .query_row(
            "SELECT value FROM metadata WHERE key = 'last_indexed'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(result)
}
```

- [ ] **Step 2: Update health handler in `src/dashboard/mod.rs`**

```rust
async fn health_handler(
    State(db_path): State<api::DbState>,
) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    let db_result = crate::db::Database::open(db_path.as_ref());
    let db_ok = db_result.is_ok();
    let last_indexed = db_result
        .ok()
        .and_then(|db| crate::db::query::get_last_indexed(&db).ok().flatten())
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

- [ ] **Step 3: Run existing tests for regressions**

Run: `cargo test`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add src/dashboard/mod.rs src/db/query.rs
git commit -m "feat(dashboard): add last_indexed timestamp to /api/health for polling"
```

---

## Task 6: Static file serving infrastructure

Set up the Axum routes to serve the new multi-file frontend. Each JS/CSS file is embedded via `include_str!` and served on its own route.

**Files:**
- Modify: `src/dashboard/mod.rs` (add routes and handlers for each static file)
- Create: `src/dashboard/static/style.css` (empty placeholder)
- Create: `src/dashboard/static/signal.js` (empty placeholder)
- Create: `src/dashboard/static/void-renderer.js` (empty placeholder)
- Create: `src/dashboard/static/detail-panel.js` (empty placeholder)
- Create: `src/dashboard/static/search.js` (empty placeholder)
- Create: `src/dashboard/static/source-modal.js` (empty placeholder)

- [ ] **Step 1: Create placeholder static files**

Create each file with a minimal comment:

`src/dashboard/static/style.css`:
```css
/* Ariadne Dashboard v2 Styles */
```

`src/dashboard/static/signal.js`:
```javascript
// Ariadne Dashboard v2 — Signal View
'use strict';
```

`src/dashboard/static/void-renderer.js`:
```javascript
// Ariadne Dashboard v2 — Void Renderer
'use strict';
```

`src/dashboard/static/detail-panel.js`:
```javascript
// Ariadne Dashboard v2 — Detail Panel
'use strict';
```

`src/dashboard/static/search.js`:
```javascript
// Ariadne Dashboard v2 — Search
'use strict';
```

`src/dashboard/static/source-modal.js`:
```javascript
// Ariadne Dashboard v2 — Source Modal
'use strict';
```

- [ ] **Step 2: Add embed constants and route handlers in `src/dashboard/mod.rs`**

Add the embed constants:
```rust
const STYLE_CSS: &str = include_str!("static/style.css");
const SIGNAL_JS: &str = include_str!("static/signal.js");
const VOID_RENDERER_JS: &str = include_str!("static/void-renderer.js");
const DETAIL_PANEL_JS: &str = include_str!("static/detail-panel.js");
const SEARCH_JS: &str = include_str!("static/search.js");
const SOURCE_MODAL_JS: &str = include_str!("static/source-modal.js");
```

Add route handlers:
```rust
async fn style_css_handler() -> (axum::http::StatusCode, [(axum::http::header::HeaderName, &'static str); 1], &'static str) {
    (axum::http::StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "text/css")], STYLE_CSS)
}

async fn signal_js_handler() -> (axum::http::StatusCode, [(axum::http::header::HeaderName, &'static str); 1], &'static str) {
    (axum::http::StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "application/javascript")], SIGNAL_JS)
}

async fn void_renderer_js_handler() -> (axum::http::StatusCode, [(axum::http::header::HeaderName, &'static str); 1], &'static str) {
    (axum::http::StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "application/javascript")], VOID_RENDERER_JS)
}

async fn detail_panel_js_handler() -> (axum::http::StatusCode, [(axum::http::header::HeaderName, &'static str); 1], &'static str) {
    (axum::http::StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "application/javascript")], DETAIL_PANEL_JS)
}

async fn search_js_handler() -> (axum::http::StatusCode, [(axum::http::header::HeaderName, &'static str); 1], &'static str) {
    (axum::http::StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "application/javascript")], SEARCH_JS)
}

async fn source_modal_js_handler() -> (axum::http::StatusCode, [(axum::http::header::HeaderName, &'static str); 1], &'static str) {
    (axum::http::StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "application/javascript")], SOURCE_MODAL_JS)
}
```

Add routes to the router:
```rust
.route("/style.css", axum::routing::get(style_css_handler))
.route("/signal.js", axum::routing::get(signal_js_handler))
.route("/void-renderer.js", axum::routing::get(void_renderer_js_handler))
.route("/detail-panel.js", axum::routing::get(detail_panel_js_handler))
.route("/search.js", axum::routing::get(search_js_handler))
.route("/source-modal.js", axum::routing::get(source_modal_js_handler))
```

- [ ] **Step 3: Run tests for regressions**

Run: `cargo test`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add src/dashboard/mod.rs src/dashboard/static/style.css src/dashboard/static/signal.js src/dashboard/static/void-renderer.js src/dashboard/static/detail-panel.js src/dashboard/static/search.js src/dashboard/static/source-modal.js
git commit -m "feat(dashboard): add static file serving infrastructure for v2 frontend"
```

---

## Task 7: CSS Design System (`style.css`)

The complete visual design system — design tokens, shared components, Signal styles, Void styles, animations.

**Files:**
- Modify: `src/dashboard/static/style.css`

- [ ] **Step 1: Write the full style.css**

This is the complete CSS file. See the design spec for visual tokens. Reference the mockup at `mockups/concept-10-signal.html` for Signal styles and `mockups/concept-8-void.html` for Void styles. The CSS should include:

1. CSS custom properties for all design tokens (colors, spacing, radii, transitions)
2. Reset and base styles (*, body, scrollbar)
3. Typography classes (.mono, .serif)
4. Top bar styles
5. Signal view styles (hero, risk cards, module grid, coupling rows, dead code grid)
6. Void view styles (ambient background, dot grid, module nodes, glass morphism, glow effects)
7. Detail panel styles (slide-in, sections, file list, risk bars)
8. Source modal styles (full-screen overlay, code block, line numbers)
9. Search overlay styles (dropdown, results, keyboard hints)
10. Transition animations (drill-down, fade, slide, scale)
11. Utility classes (hidden, visible, active states)

The worker implementing this task should read the mockups in `mockups/` for exact pixel values and copy the styles, adapting them to use CSS custom properties.

- [ ] **Step 2: Verify cargo build succeeds**

Run: `cargo build`
Expected: Compiles (style.css is embedded via include_str!)

- [ ] **Step 3: Commit**

```bash
git add src/dashboard/static/style.css
git commit -m "feat(dashboard): add complete CSS design system for v2"
```

---

## Task 8: HTML Shell (`index.html`)

The main HTML document — contains both Signal and Void view markup, loads all JS/CSS files.

**Files:**
- Modify: `src/dashboard/static/index.html` (complete rewrite)

- [ ] **Step 1: Write the index.html**

The HTML shell should contain:

1. `<head>`: charset, viewport, title "Ariadne", Google Fonts links (JetBrains Mono, Outfit, Instrument Serif), link to style.css
2. Top bar: logo, search input, stats placeholders
3. `<div id="signal-view">`: Signal markup (hero, risks section, modules section, coupling section, dead code section) — all with empty placeholder containers that JS fills
4. `<div id="void-view" class="hidden">`: Void markup (ambient orbs, dot grid, layer labels, SVG connections container, module nodes container, bottom HUD)
5. Detail panel markup (hidden by default, slide-in container with section placeholders)
6. Source modal markup (hidden by default, full-screen overlay)
7. Search overlay markup (hidden by default)
8. Back button ("← Signal", hidden by default)
9. Toast container for re-index notifications
10. `<script>` tags loading all 5 JS files
11. An `esc()` function for XSS prevention (must be present for the XSS regression test)
12. Initialization script that calls `Signal.init()` on DOMContentLoaded

Important: The existing XSS regression test checks for `function esc(` and `${esc(r.name)}` patterns. The new HTML must include the `esc()` function and use it for all innerHTML interpolation.

- [ ] **Step 2: Verify cargo build succeeds**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 3: Update XSS regression test**

The existing test at `tests/test_dashboard.rs::test_xss_regression_html_escaping` checks for specific patterns. Update it to check the new file structure. The test should verify:
- `esc()` function exists in index.html
- Any innerHTML with `${` uses `esc()`
- Each JS file that uses innerHTML also has or references `esc()`

- [ ] **Step 4: Run tests**

Run: `cargo test test_xss -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/dashboard/static/index.html tests/test_dashboard.rs
git commit -m "feat(dashboard): add v2 HTML shell with Signal and Void views"
```

---

## Task 9: Search Module (`search.js`)

Global symbol search with dropdown results and drill-down navigation.

**Files:**
- Modify: `src/dashboard/static/search.js`

- [ ] **Step 1: Implement search.js**

The search module should expose a `Search` class/object with:

```javascript
class Search {
    static init() { /* Attach event listeners to search input */ }
    static focus() { /* Focus the search input, show overlay */ }
    static close() { /* Close dropdown, clear input */ }
    static async query(term) { /* Fetch /api/search?q=term, render dropdown */ }
    static renderResults(results) { /* Build dropdown HTML using esc() for all interpolated values */ }
    static selectResult(symbolId, moduleName) { /* Trigger drill-down to Void */ }
}
```

Key behaviors:
- Debounce input (200ms) before fetching
- Dropdown shows max 10 results: name, kind badge, file path, signature
- Arrow keys navigate results, Enter selects
- Escape closes dropdown
- Clicking a result calls `App.drillDown(moduleName, symbolId)`
- All interpolated values use `esc()` for XSS prevention

- [ ] **Step 2: Test manually**

Run: `cargo run -- index . && cargo run -- dash`
Open: `http://localhost:1337`
Type in search bar, verify dropdown appears with results

- [ ] **Step 3: Commit**

```bash
git add src/dashboard/static/search.js
git commit -m "feat(dashboard): add search module with symbol search and drill-down"
```

---

## Task 10: Signal View (`signal.js`)

The intelligence report landing page — fetches data, renders hero/risks/modules/coupling/dead-code sections.

**Files:**
- Modify: `src/dashboard/static/signal.js`

- [ ] **Step 1: Implement signal.js**

The Signal module should expose:

```javascript
class Signal {
    static async init() { /* Fetch all data, render all sections */ }
    static async fetchData() { /* Parallel fetch: /api/stats, /api/modules, /api/graph/insights, /api/coupling */ }
    static renderHero(stats, insights) { /* Render health score, summary, stats row */ }
    static renderRisks(insights, modules) { /* Render top risk cards with Level C descriptions */ }
    static renderModules(modules) { /* Render 2-column module grid */ }
    static renderCoupling(coupling) { /* Render coupling pairs */ }
    static renderDeadCode(insights) { /* Render dead code grid */ }
    static computeHealthScore(stats, insights) { /* Compute 0-100 score per spec formula */ }
    static show() { /* Show signal view, hide void view */ }
    static hide() { /* Hide signal view */ }
    static saveScrollPosition() { /* Save scroll for restore on back */ }
    static restoreScrollPosition() { /* Restore scroll position */ }
}
```

Key implementation details:
- For risk cards, fetch descriptions from `/api/describe?id=X` for the top 3-5 most-connected symbols
- Health score formula: resolution_rate(30%) + dead_code_ratio(25%) + cycle_penalty(20%) + god_penalty(15%) + coupling_health(10%)
- Module cards are clickable — `onclick` calls `App.drillDown(moduleName)`
- Risk cards are clickable — `onclick` calls `App.drillDown(moduleName, symbolId)`
- Coupling rows are clickable — `onclick` calls `App.drillDown(fromModule)`
- All HTML interpolation uses `esc()`

- [ ] **Step 2: Test manually**

Run: `cargo run -- dash`
Verify: Signal view loads with real data, all sections populated

- [ ] **Step 3: Commit**

```bash
git add src/dashboard/static/signal.js
git commit -m "feat(dashboard): add Signal view with intelligence report sections"
```

---

## Task 11: Void Renderer (`void-renderer.js`)

The spatial architecture map — module nodes, SVG connections, ambient effects, auto-layout, drag-to-reposition.

**Files:**
- Modify: `src/dashboard/static/void-renderer.js`

- [ ] **Step 1: Implement void-renderer.js**

The Void module should expose:

```javascript
class Void {
    static async init() { /* Fetch module data, create nodes and connections */ }
    static show(focusModule, focusSymbol) { /* Show void view, highlight target */ }
    static hide() { /* Hide void view */ }
    static createNodes(modules) { /* Create glass-morphism DOM nodes for each module */ }
    static autoLayout(nodes) { /* 3-layer layout: Interface/Core/Data */ }
    static loadSavedPositions() { /* Read from localStorage */ }
    static savePosition(moduleName, x, y) { /* Write to localStorage */ }
    static resetLayout() { /* Clear localStorage, recompute auto-layout */ }
    static drawConnections(modules, coupling) { /* SVG bezier paths between modules */ }
    static createAmbientBackground() { /* Ambient gradient orbs */ }
    static enableDrag(nodeEl) { /* mousedown/move/up for node repositioning */ }
    static selectModule(moduleName) { /* Highlight node, open detail panel */ }
    static setMode(mode) { /* Switch Architecture/Risk/Coupling coloring */ }
    static animateFlowParticles() { /* Dots traveling along high-coupling edges */ }
}
```

Key implementation details:
- Auto-layout assigns modules to layers based on their dependencies:
  - Interface: modules with 0 incoming module-level deps (entry points)
  - Data: modules with 0 outgoing module-level deps (leaf nodes)
  - Core: everything else
- Nodes are positioned DOM elements (not canvas) for better interaction
- SVG connections use `<path>` with cubic bezier curves
- Drag uses `mousedown` → `mousemove` → `mouseup` with `transform: translate()`
- Node glow color based on health: green (#4ADE80), yellow (#FACC15), orange (#FB923C), red (#F87171)
- Flow particles use `requestAnimationFrame` + `SVGPathElement.getPointAtLength()`
- File sparklines inside nodes: flex bars colored by file health

- [ ] **Step 2: Test manually**

Run: `cargo run -- dash`
Click a module in Signal → verify Void loads with nodes and connections

- [ ] **Step 3: Commit**

```bash
git add src/dashboard/static/void-renderer.js
git commit -m "feat(dashboard): add Void renderer with spatial architecture map"
```

---

## Task 12: Detail Panel (`detail-panel.js`)

The right slide-in panel showing Level C descriptions, source code, callers/callees, risk factors.

**Files:**
- Modify: `src/dashboard/static/detail-panel.js`

- [ ] **Step 1: Implement detail-panel.js**

```javascript
class DetailPanel {
    static async open(symbolId) { /* Fetch data, render panel, slide in */ }
    static close() { /* Slide out, clear content */ }
    static async fetchData(symbolId) { /* Parallel: /api/describe, /api/source, /api/graph/neighborhood */ }
    static renderHeader(symbol) { /* Name, file path, close button */ }
    static renderDescription(describe) { /* Level C narrative paragraph */ }
    static renderSource(source) { /* Code block with line numbers and highlighting */ }
    static renderCallers(callers) { /* Clickable caller list */ }
    static renderCallees(callees) { /* Clickable callee list */ }
    static renderRiskFactors(metrics) { /* Fan-in/out/churn/coupling bars */ }
    static renderBlastRadius(metrics) { /* Affected symbol count */ }
    static renderIssues(symbol, insights) { /* Dead code, cycles, god object warnings */ }
    static highlightSyntax(code, language) { /* Lightweight keyword/string/comment coloring */ }
    static isOpen() { /* Return whether panel is currently visible */ }
}
```

Key implementation details:
- Source code: if `line_count < 25`, show full code inline. If >= 25, show first 15 lines + "View full source" button
- "View full source" button calls `SourceModal.open(source)`
- Callers/callees are clickable — clicking calls `DetailPanel.open(newSymbolId)` (re-renders panel in place)
- Risk factor bars use colored div widths proportional to values
- `highlightSyntax` is a simple regex-based colorizer:
  - Keywords: language-specific (fn, let, if, for, return, etc.) → purple
  - Strings: single/double quoted → green
  - Comments: // and /* */ → gray
  - Numbers → cyan
- All HTML interpolation uses `esc()`

- [ ] **Step 2: Test manually**

Run: `cargo run -- dash`
Click a module → verify detail panel slides in with description, source, callers/callees

- [ ] **Step 3: Commit**

```bash
git add src/dashboard/static/detail-panel.js
git commit -m "feat(dashboard): add detail panel with descriptions, source preview, and navigation"
```

---

## Task 13: Source Modal (`source-modal.js`)

Full-screen source code viewer for long functions.

**Files:**
- Modify: `src/dashboard/static/source-modal.js`

- [ ] **Step 1: Implement source-modal.js**

```javascript
class SourceModal {
    static open(sourceData) { /* Show full-screen overlay with complete source code */ }
    static close() { /* Hide overlay */ }
    static render(sourceData) { /* File path header, line numbers, highlighted code */ }
    static isOpen() { /* Return whether modal is currently visible */ }
}
```

Key implementation details:
- Full-screen glass-morphism overlay (rgba bg + backdrop-filter blur)
- File path and line range in header
- Line numbers column (fixed width, muted color)
- Code uses same `highlightSyntax` from DetailPanel (call it directly or move to a shared util)
- Escape key closes modal
- Click outside the code area closes modal

- [ ] **Step 2: Test manually**

Find a long function, click "View full source" → verify modal opens with full code

- [ ] **Step 3: Commit**

```bash
git add src/dashboard/static/source-modal.js
git commit -m "feat(dashboard): add full-screen source modal for long functions"
```

---

## Task 14: Drill-Down Navigation & App Controller

The app-level controller that wires Signal, Void, DetailPanel, Search together and manages view transitions.

**Files:**
- Modify: `src/dashboard/static/index.html` (add App controller script at bottom)

- [ ] **Step 1: Add App controller to index.html**

Add a `<script>` block at the bottom of index.html (after all other script imports):

```javascript
class App {
    static currentView = 'signal'; // 'signal' | 'void'
    static signalScrollY = 0;

    static async init() {
        await Signal.init();
        Search.init();
        App.setupKeyboard();
        App.setupPolling();
    }

    static async drillDown(moduleName, symbolId) {
        // Save Signal scroll position
        App.signalScrollY = window.scrollY;

        // Start transition animation
        Signal.hide(); // fade out (200ms)

        await new Promise(r => setTimeout(r, 200));

        // Show Void focused on target
        await Void.show(moduleName, symbolId);
        App.currentView = 'void';

        // Show back button
        document.getElementById('back-btn').classList.remove('hidden');

        // If symbolId provided, open detail panel
        if (symbolId) {
            await DetailPanel.open(symbolId);
        }
    }

    static async goBack() {
        DetailPanel.close();
        Void.hide();

        await new Promise(r => setTimeout(r, 200));

        Signal.show();
        Signal.restoreScrollPosition();
        App.currentView = 'signal';

        document.getElementById('back-btn').classList.add('hidden');
    }

    static setupKeyboard() {
        document.addEventListener('keydown', (e) => {
            if (e.key === '/') {
                e.preventDefault();
                Search.focus();
            }
            if (e.key === 'Escape') {
                // Cascading close: modal → panel → search → back to Signal
                if (SourceModal.isOpen()) {
                    SourceModal.close();
                } else if (DetailPanel.isOpen()) {
                    DetailPanel.close();
                } else if (Search.isOpen && Search.isOpen()) {
                    Search.close();
                } else if (App.currentView === 'void') {
                    App.goBack();
                }
            }
        });
    }

    static setupPolling() {
        let lastIndexed = null;
        setInterval(async () => {
            try {
                const res = await fetch('/api/health');
                const data = await res.json();
                if (lastIndexed && data.last_indexed && data.last_indexed !== lastIndexed) {
                    App.showReindexToast();
                }
                lastIndexed = data.last_indexed || lastIndexed;
            } catch (_) { /* ignore polling errors */ }
        }, 30000);
    }

    static showReindexToast() {
        const toast = document.getElementById('reindex-toast');
        toast.classList.remove('hidden');
        toast.querySelector('button').onclick = () => {
            toast.classList.add('hidden');
            window.location.reload();
        };
    }
}

document.addEventListener('DOMContentLoaded', () => App.init());
```

- [ ] **Step 2: Test full flow manually**

Run: `cargo run -- index . && cargo run -- dash`
Test:
1. Dashboard loads Signal view
2. Click a module card → animated transition to Void
3. Module is highlighted, detail panel opens
4. Click "← Signal" → returns to Signal
5. Press `/` → search focuses
6. Type a symbol name → results appear
7. Click a result → drills into Void with that symbol
8. Press Escape → cascading close behavior

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add src/dashboard/static/index.html
git commit -m "feat(dashboard): add App controller with drill-down navigation and keyboard shortcuts"
```

---

## Task 15: Integration Testing & Polish

Final integration pass — verify all components work together, fix any rough edges.

**Files:**
- Modify: `tests/test_dashboard.rs` (add integration test)
- Any files that need fixes

- [ ] **Step 1: Add integration test for new endpoints**

```rust
#[tokio::test]
async fn test_dashboard_v2_all_endpoints() {
    let (_dir, state) = setup_indexed_db();

    // Stats
    let stats_result = stats(State(state.clone())).await;
    assert!(stats_result.is_ok(), "stats endpoint failed");

    // Modules
    let modules_result = ariadne::dashboard::api::modules(State(state.clone())).await;
    assert!(modules_result.is_ok(), "modules endpoint failed");

    // Insights
    let insights_result = ariadne::dashboard::api::insights(State(state.clone())).await;
    assert!(insights_result.is_ok(), "insights endpoint failed");

    // Coupling
    let coupling_query = ariadne::dashboard::api::CouplingQuery { limit: Some(5) };
    let coupling_result = ariadne::dashboard::api::coupling(
        State(state.clone()),
        Query(coupling_query),
    ).await;
    assert!(coupling_result.is_ok(), "coupling endpoint failed");

    // Search → Describe chain
    let search_query = SearchQuery { q: Some("greet".to_string()) };
    let search_result = search_symbols(State(state.clone()), Query(search_query)).await.unwrap();
    if !search_result.0.is_empty() {
        let id: i64 = search_result.0[0].id.parse().unwrap();

        let desc_query = ariadne::dashboard::api::DescribeQuery { id };
        let desc_result = ariadne::dashboard::api::describe(
            State(state.clone()),
            Query(desc_query),
        ).await;
        assert!(desc_result.is_ok(), "describe endpoint failed");

        let source_query = ariadne::dashboard::api::SourceQuery { id, context: Some(0) };
        let source_result = ariadne::dashboard::api::source(
            State(state.clone()),
            Query(source_query),
        ).await;
        assert!(source_result.is_ok(), "source endpoint failed");
    }
}
```

- [ ] **Step 2: Run full test suite**

Run: `cargo test`
Expected: All PASS

- [ ] **Step 3: Run clippy**

Run: `cargo clippy`
Expected: No warnings in new code

- [ ] **Step 4: Run format check**

Run: `cargo fmt --check`
Expected: No formatting issues

- [ ] **Step 5: Manual end-to-end test**

Run: `cargo run -- index . && cargo run -- dash`
Walk through the complete user flow:
1. Signal loads with health score, risks, modules, coupling, dead code
2. Search works and navigates
3. Drill-down transitions smoothly
4. Void shows modules with connections
5. Detail panel shows descriptions and source code
6. Source modal works for long functions
7. Back navigation works
8. Escape cascading close works

- [ ] **Step 6: Commit**

```bash
git add tests/test_dashboard.rs
git commit -m "test(dashboard): add v2 integration tests for all new endpoints"
```

---

## Summary

| Task | Description | New Files | Modified Files |
|------|-------------|-----------|----------------|
| 1 | `/api/modules` endpoint | — | query.rs, api.rs, mod.rs, test |
| 2 | `/api/coupling` endpoint | — | query.rs, api.rs, mod.rs, test |
| 3 | `/api/describe` endpoint | describe.rs | api.rs, mod.rs, query.rs, test |
| 4 | Modify `/api/source` | — | api.rs, test |
| 5 | `last_indexed` in health | — | mod.rs, query.rs |
| 6 | Static file serving | 6 placeholder JS/CSS files | mod.rs |
| 7 | CSS design system | — | style.css |
| 8 | HTML shell | — | index.html |
| 9 | Search module | — | search.js |
| 10 | Signal view | — | signal.js |
| 11 | Void renderer | — | void-renderer.js |
| 12 | Detail panel | — | detail-panel.js |
| 13 | Source modal | — | source-modal.js |
| 14 | App controller | — | index.html |
| 15 | Integration tests | — | test_dashboard.rs |

Tasks 1-5 (Rust backend) are independent and can be parallelized.
Tasks 6-8 (infrastructure) are sequential.
Tasks 9-13 (JS modules) are mostly independent but depend on Task 8.
Task 14 depends on Tasks 9-13.
Task 15 depends on everything.
