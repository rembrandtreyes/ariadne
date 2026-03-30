use std::sync::Arc;

use ariadne::dashboard::api::{graph_data, search_symbols, stats, DbState, SearchQuery};
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
    // Find all innerHTML assignments that contain template literals
    let lines: Vec<&str> = index_html
        .lines()
        .filter(|line| line.contains("innerHTML"))
        .collect();

    for line in &lines {
        // Lines that set innerHTML to '' (empty) or static strings are safe
        if line.contains("innerHTML = ''")
            || line.contains("innerHTML = \"\"")
            || line.contains("innerHTML = '<")
        {
            continue;
        }
        // Lines that clear innerHTML are safe
        if line.trim().ends_with("innerHTML = '';") {
            continue;
        }
        // Lines that use template literals with ${...} must use esc()
        if line.contains("${") {
            assert!(
                line.contains("esc("),
                "innerHTML line with template interpolation must use esc(): {}",
                line.trim()
            );
        }
    }

    // Specifically verify search results in index.html use esc()
    assert!(
        index_html.contains("${esc(r.name)}"),
        "search results must escape r.name with esc()"
    );
    assert!(
        index_html.contains("${esc(r.id)}"),
        "search results must escape r.id with esc()"
    );

    // The graph renderer (tooltip) has been moved to graph-renderer.js (Canvas 2D)
    let renderer_js = std::fs::read_to_string("src/dashboard/static/graph-renderer.js")
        .expect("should read graph-renderer.js");
    assert!(
        renderer_js.contains("esc(node.name)"),
        "tooltip must escape node.name with esc()"
    );
    assert!(
        renderer_js.contains("esc(node.kind)"),
        "tooltip must escape node.kind with esc()"
    );
    assert!(
        renderer_js.contains("esc(node.file)"),
        "tooltip must escape node.file with esc()"
    );
}
