use std::sync::Arc;

use ariadne::dashboard::api::{
    coupling, describe, graph_data, modules, search_symbols, source, stats, CouplingQuery, DbState,
    DescribeQuery, SearchQuery, SourceQuery,
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
