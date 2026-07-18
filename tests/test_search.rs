use ariadne::config::RepoConfig;
use ariadne::db::Database;
use ariadne::pipeline::run_full_pipeline;
use ariadne::search::{search, SearchOptions};
use std::path::Path;

/// Helper: create a temp DB with the Python fixture indexed.
fn setup_search_db() -> (tempfile::TempDir, Database) {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let config = RepoConfig::default();
    let fixture = Path::new("tests/fixtures/python_repo");
    run_full_pipeline(&db, fixture, &config).unwrap();
    (dir, db)
}

#[test]
fn test_search_finds_known_symbol() {
    let (_dir, db) = setup_search_db();

    let options = SearchOptions::default();
    let results = search(&db, "greet", &options).unwrap();

    assert!(
        !results.is_empty(),
        "expected to find 'greet' in search results"
    );
    assert!(
        results.iter().any(|r| r.name.contains("greet")),
        "expected a result with 'greet' in the name, got: {:?}",
        results.iter().map(|r| &r.name).collect::<Vec<_>>()
    );
}

#[test]
fn test_search_fuzzy_match() {
    let (_dir, db) = setup_search_db();

    let options = SearchOptions {
        fuzzy: true,
        ..Default::default()
    };
    // "greet" misspelled as "greet" with typo -> "gret"
    let results = search(&db, "gret", &options).unwrap();

    assert!(
        !results.is_empty(),
        "fuzzy search for 'gret' should return results"
    );
    // With fuzzy matching, "greet" should appear as a close match
    assert!(
        results
            .iter()
            .any(|r| r.name.contains("greet") || r.name.contains("gret")),
        "fuzzy search should find 'greet' or similar, got: {:?}",
        results.iter().map(|r| &r.name).collect::<Vec<_>>()
    );
}

#[test]
fn test_search_respects_limit() {
    let (_dir, db) = setup_search_db();

    let options = SearchOptions {
        limit: Some(2),
        ..Default::default()
    };
    // Use a broad query that might match many symbols
    let results = search(&db, "e", &options).unwrap();

    assert!(
        results.len() <= 2,
        "expected at most 2 results with limit=2, got {}",
        results.len()
    );
}

#[test]
fn test_fts5_injection_regression() {
    let (_dir, db) = setup_search_db();

    let options = SearchOptions::default();

    // This should not panic or error - it should safely handle SQL injection attempts
    let result = search(&db, "\" OR 1=1 --", &options);
    assert!(
        result.is_ok(),
        "FTS5 injection attempt should not cause an error: {:?}",
        result.err()
    );
}

#[test]
fn test_search_special_chars_in_like() {
    let (_dir, db) = setup_search_db();

    let options = SearchOptions::default();

    // "%" is a LIKE wildcard - should be escaped and not match everything
    let result_percent = search(&db, "%", &options);
    assert!(
        result_percent.is_ok(),
        "search for '%' should not panic: {:?}",
        result_percent.err()
    );

    // "_" is a LIKE single-char wildcard - should be escaped
    let result_underscore = search(&db, "_", &options);
    assert!(
        result_underscore.is_ok(),
        "search for '_' should not panic: {:?}",
        result_underscore.err()
    );
}

#[test]
fn test_search_empty_query_does_not_panic() {
    let (_dir, db) = setup_search_db();

    let options = SearchOptions::default();
    // Empty query should not panic or error.
    // The underlying LIKE search with "%%" may match all symbols,
    // so we only assert it completes without error.
    let result = search(&db, "", &options);
    assert!(
        result.is_ok(),
        "search with empty query should not error: {:?}",
        result.err()
    );
}

#[test]
fn test_search_never_returns_duplicate_symbols() {
    // The fuzzy supplement pass re-finds symbols FTS already returned;
    // without dedup the same symbol id appears twice in one result list
    // (seen live: `ariadne search PathFilter` listed the class twice).
    let (_dir, db) = setup_search_db();

    let options = SearchOptions {
        fuzzy: true,
        limit: Some(50),
        ..SearchOptions::default()
    };
    let results = search(&db, "greet", &options).unwrap();

    let mut seen = std::collections::HashSet::new();
    for r in &results {
        if let Some(id) = r.symbol_id {
            assert!(
                seen.insert(id),
                "symbol id {id} ({}) returned more than once — duplicate \
                 rows make every downstream consumer look broken",
                r.name
            );
        }
    }
}
