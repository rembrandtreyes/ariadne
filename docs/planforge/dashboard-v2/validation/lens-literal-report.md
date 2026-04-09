# Lens 1: Literal Accuracy Report

## Summary

ISSUES(1) fixed

## Hard Violations Found and Fixed

- **PRD-01, Step 11**: Duplicate test function name `test_dashboard_v2_all_endpoints` — PRD-01 Step 11 and PRD-05 Step 4 both instructed adding a function with this exact name to `tests/test_dashboard.rs`. When executed in sequence this produces a Rust compile error (`duplicate definitions`). Fixed by renaming PRD-01's version to `test_dashboard_v2_endpoints_basic` throughout (step body, verify command, acceptance criteria, completion contract). PRD-05's more complete version retains the original name.

## Soft Violations Fixed

None.

## Full Cross-Reference Audit (no additional violations)

### Rust function signatures (all verified correct)

- `query::symbol_by_id(db, id: i64)` — exists at `src/db/query.rs:226`, called correctly in PRD-01 `describe.rs`
- `query::file_path_by_id(db, file_id: i64) -> anyhow::Result<String>` — exists at `src/db/query.rs:265`, called correctly
- `query::get_dependents(db, symbol_id: i64)` — exists at `src/db/query.rs:59`
- `query::get_dependencies(db, symbol_id: i64)` — exists at `src/db/query.rs:76`
- `query::get_file_couplings(db, file_id: i64) -> anyhow::Result<Vec<CouplingRow>>` — exists at `src/db/query.rs:474`
- `query::get_symbol_health_data(db, name: &str)` — exists at `src/db/query.rs:922`; PRD-01 passes `&sym.name` (correct `&str` coercion)
- `db.get_metadata(key: &str)` — exists at `src/db/mod.rs:68`, used correctly in PRD-01 health handler

### Struct field access (all verified correct)

- `SymbolRow` fields used in PRD-01 `describe.rs`: `id, file_id, name, kind, is_dead` — all present (`src/db/query.rs:8–18`)
- `SymbolHealthData` fields used: `fan_in, fan_out, modification_count, author_count, is_volatile` — all present (`src/db/query.rs:904–919`)
- `CouplingRow` fields used: `strength, coupled_path` — both present (`src/db/query.rs:499–506`)
- `ModuleSummary` fields: `name, path, symbol_count, file_count, health, risk, dead_count, cycle_count, god_objects, files` — all present (`src/db/query.rs:1084–1094`), correctly accessed in PRD-03 `signal.js` and PRD-04 `void-renderer.js`
- `ModuleFileSummary` fields: `name, symbol_count, dead_count, risk, health` — all present (`src/db/query.rs:1074–1080`), `f.health` access in PRD-04 correct
- `CouplingPairSummary` fields: `from_module, to_module, from_file, to_file, strength, co_changes, is_cycle` — all present (`src/db/query.rs:1192–1199`), all accessed correctly in PRD-03 `renderCoupling` and PRD-04 `drawConnections`
- `GraphNode` fields: `id (String), name, kind, file, group, in_degree, out_degree, is_dead, line_start, line_end, signature` — all present (`src/dashboard/api.rs:57–70`); `search.js` accesses `r.id, r.name, r.kind, r.file` — correct; tests call `results[0].id.parse()` — correct since `id` is `String`

### Route correctness (all verified)

- `/api/health` — registered `src/dashboard/mod.rs:41`
- `/api/stats` — registered line 42; `signal.js` fetches it
- `/api/search` — registered line 44; `search.js` fetches it
- `/api/graph/neighborhood` — registered lines 45–48; `detail-panel.js` fetches `?id=N&depth=1`, matches `NeighborhoodQuery { id: i64, depth: Option<u32> }`
- `/api/graph/insights` — registered line 49; `signal.js` fetches it
- `/api/source` — registered line 50; `detail-panel.js` fetches `?id=N&context=0`, PRD-01 adds `context` param
- `/api/modules` — registered line 51; `signal.js` and `void-renderer.js` fetch it
- `/api/coupling` — registered line 52; `signal.js` and `void-renderer.js` fetch `?limit=N`
- `/api/describe` — new; PRD-01 Step 4 adds it; `signal.js renderRisks` and `detail-panel.js` fetch `?id=N`

### JavaScript response field access (all verified correct)

- `stats.{files, symbols, calls, resolution_rate, dead_functions, languages}` — matches `Stats` struct (`src/dashboard/api.rs:80–87`)
- `modulesData.modules` — matches `ModulesResponse.modules` (`src/dashboard/api.rs:559`)
- `couplingData.pairs` — matches `CouplingResponse.pairs` (`src/dashboard/api.rs:576`)
- `insights.circular_deps` — matches `Insights.circular_deps` (`src/dashboard/api.rs:102`); PRD-03 uses correct name
- `insights.god_files` — matches `Insights.god_files` (`src/dashboard/api.rs:105`); PRD-03 uses correct name
- `insights.most_connected` — matches `Insights.most_connected` (`src/dashboard/api.rs:103`)
- `insights.dead_code` — matches `Insights.dead_code` (`src/dashboard/api.rs:106`)
- `desc.{description, role, risk_level, risk_score, metrics.{fan_in, fan_out, modification_count, author_count, is_volatile, blast_radius, coupled_file_count, max_coupling_strength}}` — all present in PRD-01 `DescribeResult`/`DescribeMetrics`
- `source.{code, line_start, line_end, line_count, language, file}` — `line_count` added by PRD-01 Step 6; PRD-05 `detail-panel.js renderSource` and `source-modal.js render` both access it after PRD-01 has run
- `neighborhood.nodes[].{id, name, kind, file}` / `neighborhood.edges[].{source, target}` — matches `GraphData`/`GraphNode`/`GraphEdge`; PRD-05 `renderHeader` reads `selfNode.name` and `selfNode.file` from neighborhood — correct

### DOM element IDs referenced in JS (all consistent with PRD-02 HTML)

- `#search-input`, `#search-container` — PRD-03 `search.js`
- `#signal-hero`, `#top-stats`, `#risk-cards`, `#module-grid`, `#coupling-list`, `#dead-code-grid`, `#signal-view` — PRD-03 `signal.js`
- `#void-view`, `#void-nodes`, `#void-connections`, `#void-ambient`, `#void-layers` — PRD-04 `void-renderer.js`
- `#detail-panel`, `#detail-header`, `#detail-content` — PRD-05 `detail-panel.js`
- `#source-modal`, `#source-modal-header`, `#source-modal-code` — PRD-05 `source-modal.js`
- All IDs are defined in PRD-02 HTML shell; no mismatches detected

### Import correctness (all verified)

- `use crate::db::{query, Database};` in `describe.rs` — both modules exist
- `use crate::dashboard::describe::DescribeResult` in `api.rs` — module created by PRD-01 Step 1
- Test import `use ariadne::dashboard::api::{coupling, describe, graph_data, modules, search_symbols, source, stats, CouplingQuery, DbState, DescribeQuery, SearchQuery, SourceQuery}` — all items are public exports from `src/dashboard/api.rs` after PRD-01 runs; existing test file already imports `coupling, graph_data, modules, search_symbols, stats, CouplingQuery, DbState, SearchQuery` with `use axum::extract::{Query, State}` available
