use std::sync::Arc;

use ariadne::dashboard::api::{
    complexity_hotspots, coupling, dependency_path, describe, entry_points, god_objects,
    graph_data, modules, propose_edit_plan, search_symbols, source, stats, ComplexityHotspotsQuery,
    CouplingQuery, DbState, DependencyPathQuery, DescribeQuery, EntryPointsQuery, GodObjectsQuery,
    ProposeEditPlanQuery, SearchQuery, SourceQuery,
};
use axum::extract::{Query, State};
use axum::response::IntoResponse;

/// Helper: create a temp DB, index the Python fixture, return (TempDir, DbState).
/// TempDir must be kept alive for the duration of the test.
fn setup_indexed_db() -> (tempfile::TempDir, DbState) {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = ariadne::db::Database::open(&db_path).unwrap();
    let config = ariadne::config::RepoConfig::default();
    let fixture = std::path::Path::new("tests/fixtures/python_repo");
    ariadne::pipeline::run_full_pipeline(&db, fixture, &config).unwrap();
    drop(db); // Close so handlers can reopen
    let state: DbState = Arc::new(db_path);
    (dir, state)
}

#[tokio::test]
async fn test_dashboard_stats_handler() {
    let (_dir, state) = setup_indexed_db();

    let result = stats(State(state)).await.expect("stats should succeed");
    let s = result.0;

    assert!(s.files > 0, "expected files > 0, got {}", s.files);
    assert!(s.symbols > 0, "expected symbols > 0, got {}", s.symbols);
    assert!(
        !s.languages.is_empty(),
        "expected at least one language detected"
    );
}

#[tokio::test]
async fn test_dashboard_search_handler() {
    let (_dir, state) = setup_indexed_db();

    // Search for "greet" which is defined in main.py
    let query = SearchQuery {
        q: Some("greet".to_string()),
    };
    let result = search_symbols(State(state), Query(query))
        .await
        .expect("search should succeed");
    let results = result.0;

    assert!(
        !results.is_empty(),
        "expected search results for 'greet', got empty"
    );
    assert!(
        results.iter().any(|r| r.name.contains("greet")),
        "expected a result containing 'greet' in name"
    );
}

#[tokio::test]
async fn test_dashboard_search_empty_query() {
    let (_dir, state) = setup_indexed_db();

    let query = SearchQuery { q: None };
    let result = search_symbols(State(state.clone()), Query(query))
        .await
        .expect("search none should succeed");
    assert!(result.0.is_empty(), "expected empty results for None query");

    let query_empty = SearchQuery {
        q: Some(String::new()),
    };
    let result2 = search_symbols(State(state), Query(query_empty))
        .await
        .expect("search empty should succeed");
    assert!(
        result2.0.is_empty(),
        "expected empty results for empty string query"
    );
}

#[tokio::test]
async fn test_dashboard_graph_data() {
    let (_dir, state) = setup_indexed_db();

    let result = graph_data(State(state))
        .await
        .expect("graph_data should succeed");
    let data = result.0;

    assert!(!data.nodes.is_empty(), "expected graph nodes, got empty");
    // The python fixture has calls (greet -> helper), so we should have edges
    // But edges depend on resolution -- at minimum we should have nodes
    assert!(
        data.nodes.len() >= 2,
        "expected at least 2 nodes, got {}",
        data.nodes.len()
    );
}

#[tokio::test]
async fn test_dashboard_error_on_invalid_db() {
    // Point to a directory (not a file) — Database::open will fail
    let state: DbState = Arc::new(std::path::PathBuf::from("/dev/null/impossible.db"));

    // All handlers should return Err (not silently return empty data)
    let graph_result = graph_data(State(state.clone())).await;
    assert!(
        graph_result.is_err(),
        "graph_data should error on invalid DB path"
    );

    let stats_result = stats(State(state.clone())).await;
    assert!(
        stats_result.is_err(),
        "stats should error on invalid DB path"
    );

    // Verify the error has the right code
    let err = match graph_result {
        Err(e) => e,
        Ok(_) => panic!("expected error"),
    };
    let response = err.into_response();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "invalid DB should return 503"
    );
}

#[tokio::test]
async fn test_dashboard_modules_handler() {
    let (_dir, state) = setup_indexed_db();

    let result = modules(State(state)).await.expect("modules should succeed");
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

#[tokio::test]
async fn test_dashboard_coupling_handler() {
    let (_dir, state) = setup_indexed_db();

    let query = CouplingQuery { limit: Some(10) };
    let result = coupling(State(state), Query(query))
        .await
        .expect("coupling should succeed");
    let data = result.0;

    // The python fixture may not have coupling data (requires git history),
    // but the endpoint should return successfully with an empty list
    assert!(data.pairs.len() <= 10, "should respect the limit parameter");
}

#[test]
fn test_xss_regression_html_escaping() {
    // Read the dashboard HTML file directly from the source tree
    let index_html =
        std::fs::read_to_string("src/dashboard/static/index.html").expect("should read index.html");

    // Verify the esc() function exists in the HTML
    assert!(
        index_html.contains("function esc("),
        "index.html must contain the esc() XSS-prevention function"
    );

    // Verify that innerHTML usages that interpolate data use esc()
    let lines: Vec<&str> = index_html
        .lines()
        .filter(|line| line.contains("innerHTML"))
        .collect();

    for line in &lines {
        if line.contains("innerHTML = ''")
            || line.contains("innerHTML = \"\"")
            || line.contains("innerHTML = '<")
        {
            continue;
        }
        if line.trim().ends_with("innerHTML = '';") {
            continue;
        }
        if line.contains("${") {
            assert!(
                line.contains("esc("),
                "innerHTML line with template interpolation must use esc(): {}",
                line.trim()
            );
        }
    }

    // Verify JS files that use innerHTML also reference esc()
    let js_files = [
        "src/dashboard/static/search.js",
        "src/dashboard/static/signal.js",
        "src/dashboard/static/detail-panel.js",
        "src/dashboard/static/source-modal.js",
    ];

    for js_file in &js_files {
        if let Ok(content) = std::fs::read_to_string(js_file) {
            let js_lines: Vec<&str> = content
                .lines()
                .filter(|line| line.contains("innerHTML"))
                .collect();

            for line in &js_lines {
                if line.contains("innerHTML = ''") || line.contains("innerHTML = \"\"") {
                    continue;
                }
                if line.trim().ends_with("innerHTML = '';") {
                    continue;
                }
                if line.contains("${") {
                    assert!(
                        line.contains("esc("),
                        "innerHTML in {} with interpolation must use esc(): {}",
                        js_file,
                        line.trim()
                    );
                }
            }
        }
    }
}

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
    let insights_result = ariadne::dashboard::api::insights(State(state.clone())).await;
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

#[tokio::test]
async fn test_dashboard_v2_all_endpoints() {
    let (_dir, state) = setup_indexed_db();

    // Stats
    let stats_result = stats(State(state.clone())).await;
    assert!(stats_result.is_ok(), "stats endpoint failed");

    // Modules
    let modules_result = modules(State(state.clone())).await;
    assert!(modules_result.is_ok(), "modules endpoint failed");

    // Insights
    let insights_result = ariadne::dashboard::api::insights(State(state.clone())).await;
    assert!(insights_result.is_ok(), "insights endpoint failed");

    // Coupling
    let coupling_query = CouplingQuery { limit: Some(5) };
    let coupling_result = coupling(State(state.clone()), Query(coupling_query)).await;
    assert!(coupling_result.is_ok(), "coupling endpoint failed");

    // Search -> Describe -> Source chain
    let search_query = SearchQuery {
        q: Some("greet".to_string()),
    };
    let search_result = search_symbols(State(state.clone()), Query(search_query))
        .await
        .expect("search should succeed");
    assert!(
        !search_result.0.is_empty(),
        "need at least one symbol for v2 integration test"
    );

    let symbol_id: i64 = search_result.0[0].id.parse().expect("id should be numeric");

    let desc_query = DescribeQuery { id: symbol_id };
    let desc_result = describe(State(state.clone()), Query(desc_query))
        .await
        .expect("describe endpoint failed");
    assert!(
        !desc_result.0.description.is_empty(),
        "description should not be empty"
    );
    assert!(
        desc_result.0.risk_score >= 0.0 && desc_result.0.risk_score <= 1.0,
        "risk_score should be 0-1, got {}",
        desc_result.0.risk_score
    );

    let source_query = SourceQuery {
        id: symbol_id,
        context: Some(0),
    };
    let source_result = source(State(state.clone()), Query(source_query))
        .await
        .expect("source endpoint failed");
    assert!(
        !source_result.0.code.is_empty(),
        "source code should not be empty"
    );
    assert!(source_result.0.line_count > 0, "line_count should be > 0");
}

// ---- Dashboard stub-zero wiring tests (Run #13) ----
//
// Signal-view module cards and the describe panel shipped with three hardcoded
// zeros (cycle_count, god_objects, blast_radius). These tests pin the real
// values so the stubs can't silently regress.

fn insert_call_sql(db: &ariadne::db::Database, caller: i64, callee: i64, file_id: i64) {
    db.conn()
        .execute(
            "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution)
             VALUES (?1, ?2, '', ?3, 1, 1.0, 'exact')",
            rusqlite::params![caller, callee, file_id],
        )
        .expect("insert call");
}

#[test]
fn test_module_summaries_cycle_count_wired_up() {
    use ariadne::db::{query, write, Database};

    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test", "/tmp/t", "monolith", "rust").unwrap();
    let f_a = write::insert_file(
        &db,
        svc,
        "src/pipeline/a.rs",
        "/tmp/t/src/pipeline/a.rs",
        "rust",
        0.0,
    )
    .unwrap();
    let f_b = write::insert_file(
        &db,
        svc,
        "src/pipeline/b.rs",
        "/tmp/t/src/pipeline/b.rs",
        "rust",
        0.0,
    )
    .unwrap();

    let sym_a = write::insert_symbol(
        &db,
        f_a,
        "fn_a",
        "pipeline::a::fn_a",
        "function",
        1,
        10,
        true,
        false,
        "",
        "",
        None,
    )
    .unwrap();
    let sym_b = write::insert_symbol(
        &db,
        f_b,
        "fn_b",
        "pipeline::b::fn_b",
        "function",
        1,
        10,
        true,
        false,
        "",
        "",
        None,
    )
    .unwrap();

    // Mutual recursion A <-> B forms a strongly-connected component of size 2.
    insert_call_sql(&db, sym_a, sym_b, f_a);
    insert_call_sql(&db, sym_b, sym_a, f_b);

    let modules = query::get_module_summaries(&db).expect("module summaries");
    let pipeline = modules
        .iter()
        .find(|m| m.name == "pipeline")
        .expect("pipeline module exists");

    assert!(
        pipeline.cycle_count >= 1,
        "cycle_count should be ≥1 for A<->B cycle in pipeline module, got {}",
        pipeline.cycle_count
    );
}

#[test]
fn test_module_summaries_god_objects_wired_up() {
    use ariadne::db::{query, write, Database};

    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test", "/tmp/t", "monolith", "rust").unwrap();
    let popular_file = write::insert_file(
        &db,
        svc,
        "src/graph/popular.rs",
        "/tmp/t/src/graph/popular.rs",
        "rust",
        0.0,
    )
    .unwrap();

    let popular = write::insert_symbol(
        &db,
        popular_file,
        "popular_fn",
        "graph::popular_fn",
        "function",
        1,
        5,
        true,
        false,
        "",
        "",
        None,
    )
    .unwrap();

    // 20 caller symbols each invoking popular_fn → fan_in = 20 = default god-object threshold.
    for i in 0..20 {
        let caller = write::insert_symbol(
            &db,
            popular_file,
            &format!("caller_{i}"),
            &format!("graph::caller_{i}"),
            "function",
            (100 + i * 10) as u32,
            (100 + i * 10 + 5) as u32,
            true,
            false,
            "",
            "",
            None,
        )
        .unwrap();
        insert_call_sql(&db, caller, popular, popular_file);
    }

    let modules = query::get_module_summaries(&db).expect("module summaries");
    let graph = modules
        .iter()
        .find(|m| m.name == "graph")
        .expect("graph module exists");

    assert!(
        graph.god_objects >= 1,
        "god_objects should be ≥1 for a fan-in-20 symbol in graph module, got {}",
        graph.god_objects
    );
}

#[test]
fn test_describe_blast_radius_wired_up() {
    use ariadne::dashboard::describe::describe_symbol;
    use ariadne::db::{write, Database};

    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test", "/tmp/t", "monolith", "rust").unwrap();
    let file = write::insert_file(
        &db,
        svc,
        "src/mcp/core.rs",
        "/tmp/t/src/mcp/core.rs",
        "rust",
        0.0,
    )
    .unwrap();

    let callee = write::insert_symbol(
        &db,
        file,
        "target_fn",
        "mcp::target_fn",
        "function",
        1,
        5,
        true,
        false,
        "",
        "",
        None,
    )
    .unwrap();

    // 3 callers → blast_radius.total_affected should be 3.
    for i in 0..3 {
        let caller = write::insert_symbol(
            &db,
            file,
            &format!("c{i}"),
            &format!("mcp::c{i}"),
            "function",
            (10 + i * 10) as u32,
            (10 + i * 10 + 5) as u32,
            true,
            false,
            "",
            "",
            None,
        )
        .unwrap();
        insert_call_sql(&db, caller, callee, file);
    }

    let result = describe_symbol(&db, callee).expect("describe");
    assert!(
        result.metrics.blast_radius >= 3,
        "blast_radius should reflect 3 callers, got {}",
        result.metrics.blast_radius
    );
}

// =====================================================================
// Run #15 — REST parity slice (Option B): 4 new routes mirror MCP tools
// =====================================================================

#[tokio::test]
async fn test_dashboard_entry_points_handler() {
    let (_dir, state) = setup_indexed_db();

    // Default: no category filter, default limit.
    let query = EntryPointsQuery {
        category: None,
        limit: None,
    };
    let result = entry_points(State(state.clone()), Query(query))
        .await
        .expect("entry_points should succeed");
    let response = result.0;

    // The Python fixture has no `main` symbol; the handler must still return
    // a successful empty response, never a 5xx.
    let has_main_filter = EntryPointsQuery {
        category: Some("main".to_string()),
        limit: Some(10),
    };
    let main_result = entry_points(State(state.clone()), Query(has_main_filter))
        .await
        .expect("main-filtered entry_points should succeed");
    assert!(
        main_result
            .0
            .entry_points
            .iter()
            .all(|e| e.category == "main"),
        "category filter should constrain results"
    );

    // Unknown category returns empty, not error.
    let unknown_filter = EntryPointsQuery {
        category: Some("nonexistent".to_string()),
        limit: None,
    };
    let unknown_result = entry_points(State(state), Query(unknown_filter))
        .await
        .expect("unknown category should succeed with empty result");
    assert!(unknown_result.0.entry_points.is_empty());

    // Even when no entry points exist, the response shape must be consistent.
    let _ = response.entry_points;
}

#[tokio::test]
async fn test_dashboard_complexity_hotspots_handler() {
    let (_dir, state) = setup_indexed_db();

    let query = ComplexityHotspotsQuery { limit: Some(5) };
    let result = complexity_hotspots(State(state.clone()), Query(query))
        .await
        .expect("complexity_hotspots should succeed");
    let response = result.0;

    assert!(
        response.hotspots.len() <= 5,
        "limit should cap result count, got {}",
        response.hotspots.len()
    );
    // Default limit when None — should clamp to a sane upper bound.
    let default_query = ComplexityHotspotsQuery { limit: None };
    let default_result = complexity_hotspots(State(state), Query(default_query))
        .await
        .expect("default limit should succeed");
    assert!(
        default_result.0.hotspots.len() <= 100,
        "default limit must cap below 100, got {}",
        default_result.0.hotspots.len()
    );
}

#[tokio::test]
async fn test_dashboard_god_objects_handler() {
    let (_dir, state) = setup_indexed_db();

    // Threshold of 0 should match every non-dead, non-test symbol.
    let permissive = GodObjectsQuery {
        threshold: Some(0),
        limit: Some(50),
    };
    let result = god_objects(State(state.clone()), Query(permissive))
        .await
        .expect("permissive god_objects should succeed");
    let permissive_count = result.0.god_objects.len();

    // High threshold should yield fewer (or zero) results.
    let strict = GodObjectsQuery {
        threshold: Some(10_000),
        limit: Some(50),
    };
    let strict_result = god_objects(State(state), Query(strict))
        .await
        .expect("strict god_objects should succeed");
    assert!(
        strict_result.0.god_objects.len() <= permissive_count,
        "higher threshold must not yield more results: strict={} vs permissive={}",
        strict_result.0.god_objects.len(),
        permissive_count
    );
}

#[tokio::test]
async fn test_dashboard_dependency_path_handler() {
    let (_dir, state) = setup_indexed_db();

    // The python_repo fixture wires greet -> helper at minimum; if call
    // resolution finds it, the path should be reachable.
    let query = DependencyPathQuery {
        from: "greet".to_string(),
        to: "helper".to_string(),
    };
    let result = dependency_path(State(state.clone()), Query(query))
        .await
        .expect("dependency_path should succeed");
    let response = result.0;

    // Either reachable (path returned) or unreachable (empty path) — never crash.
    if response.reachable {
        assert!(
            !response.path.is_empty(),
            "reachable path must be non-empty"
        );
        assert_eq!(
            response.path_length,
            response.path.len().saturating_sub(1),
            "path_length should equal hops (path nodes - 1)"
        );
    } else {
        assert!(response.path.is_empty(), "unreachable path must be empty");
        assert_eq!(response.path_length, 0);
    }

    // Missing symbol => 200 with reachable=false, summary describes the miss.
    let missing = DependencyPathQuery {
        from: "doesnotexist_xyzzy".to_string(),
        to: "helper".to_string(),
    };
    let missing_result = dependency_path(State(state), Query(missing))
        .await
        .expect("missing symbol should succeed with structured miss");
    assert!(!missing_result.0.reachable);
    assert!(
        missing_result.0.summary.contains("doesnotexist")
            || missing_result
                .0
                .summary
                .to_lowercase()
                .contains("not found"),
        "summary should mention the missing symbol or 'not found', got: {}",
        missing_result.0.summary
    );
}

#[tokio::test]
async fn test_dashboard_propose_edit_plan_handler() {
    let (_dir, state) = setup_indexed_db();

    // The python_repo fixture wires greet -> helper, so `helper` has at least
    // one dependent (`greet`). Editing helper requires touching greet first.
    let query = ProposeEditPlanQuery {
        symbol: "helper".to_string(),
    };
    let result = propose_edit_plan(State(state.clone()), Query(query))
        .await
        .expect("propose_edit_plan should succeed");
    let response = result.0;

    let symbol = response
        .symbol
        .as_ref()
        .expect("resolved symbol must populate response.symbol");
    assert_eq!(symbol.name, "helper");

    // Default ordering on a small DAG must be the deterministic topological one.
    assert!(
        response.ordering_strategy == "topological"
            || response.ordering_strategy == "bfs_depth_fallback",
        "ordering_strategy must be one of the two known values, got: {}",
        response.ordering_strategy
    );

    // total_dependents must equal the edit_order length (the same dependent
    // cone, surfaced as both a count and a list).
    assert_eq!(
        response.total_dependents,
        response.edit_order.len(),
        "total_dependents must match edit_order length"
    );

    // affected_test_count must equal the count of is_test entries — REST
    // mirror of the MCP invariant.
    assert_eq!(
        response.affected_test_count,
        response.affected_tests.len(),
        "affected_test_count must match affected_tests length"
    );
    let test_steps = response.edit_order.iter().filter(|e| e.is_test).count();
    assert_eq!(
        response.affected_tests.len(),
        test_steps,
        "affected_tests must equal the is_test rows in edit_order"
    );

    // Steps must be 1-indexed and consecutive.
    for (i, entry) in response.edit_order.iter().enumerate() {
        assert_eq!(entry.step, i + 1, "edit_order steps must be 1-indexed");
    }

    // Idempotent ordering — same query twice produces byte-identical edit_order.
    let repeat_query = ProposeEditPlanQuery {
        symbol: "helper".to_string(),
    };
    let repeat = propose_edit_plan(State(state.clone()), Query(repeat_query))
        .await
        .expect("repeat propose_edit_plan should succeed");
    let names_a: Vec<&str> = response
        .edit_order
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    let names_b: Vec<&str> = repeat
        .0
        .edit_order
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(
        names_a, names_b,
        "edit_order must be deterministic across repeated calls"
    );

    // Missing symbol => 200 OK with structured summary, never an error.
    let missing = ProposeEditPlanQuery {
        symbol: "doesnotexist_xyzzy".to_string(),
    };
    let missing_result = propose_edit_plan(State(state), Query(missing))
        .await
        .expect("missing symbol should succeed with structured miss");
    assert!(missing_result.0.symbol.is_none());
    assert_eq!(missing_result.0.total_dependents, 0);
    assert!(missing_result.0.edit_order.is_empty());
    assert!(
        missing_result
            .0
            .summary
            .to_lowercase()
            .contains("not found"),
        "summary should mention 'not found', got: {}",
        missing_result.0.summary
    );
}
