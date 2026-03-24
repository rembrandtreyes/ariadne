use ariadne::config::RepoConfig;
use ariadne::db::Database;
use ariadne::pipeline::{call_resolution::resolve_calls, run_full_pipeline};
use std::path::Path;

/// Helper: run the full pipeline on the python_repo fixture and return the database.
fn setup_python_repo_db() -> Database {
    let db = Database::open_in_memory().expect("Failed to open in-memory DB");
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/python_repo");
    let config = RepoConfig::default();
    run_full_pipeline(&db, &fixture_path, &config).expect("Pipeline should succeed");
    db
}

#[test]
fn test_resolve_calls_populates_callee_symbol_id() {
    let db = setup_python_repo_db();
    let conn = db.conn();

    // After the full pipeline (which includes call resolution), at least some
    // calls should have their callee_symbol_id populated.
    let resolved_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calls WHERE callee_symbol_id IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .expect("Query should succeed");

    let total_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM calls", [], |r| r.get(0))
        .expect("Query should succeed");

    // If there are calls at all, at least one should be resolved.
    if total_count > 0 {
        assert!(
            resolved_count > 0,
            "Expected at least one call with callee_symbol_id populated, got 0 out of {total_count}"
        );
    }
}

#[test]
fn test_resolve_calls_confidence_range() {
    let db = setup_python_repo_db();
    let conn = db.conn();

    // Every call's confidence value should be in the range [0.0, 1.0].
    let out_of_range: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calls WHERE confidence < 0.0 OR confidence > 1.0",
            [],
            |r| r.get(0),
        )
        .expect("Query should succeed");

    assert_eq!(
        out_of_range, 0,
        "All confidence values must be between 0.0 and 1.0, but {out_of_range} are out of range"
    );
}

#[test]
fn test_resolve_calls_idempotent() {
    let db = setup_python_repo_db();
    let conn = db.conn();

    // Snapshot the state after the first pipeline run (which already called resolve_calls).
    let count_before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calls WHERE callee_symbol_id IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .expect("Query should succeed");

    // Run resolve_calls again -- results should be the same.
    resolve_calls(&db).expect("Second resolve_calls should succeed");

    let count_after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calls WHERE callee_symbol_id IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .expect("Query should succeed");

    assert_eq!(
        count_before, count_after,
        "resolve_calls should be idempotent: resolved count changed from {count_before} to {count_after}"
    );
}

#[test]
fn test_resolve_calls_resolution_field_set() {
    let db = setup_python_repo_db();
    let conn = db.conn();

    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM calls", [], |r| r.get(0))
        .expect("Query should succeed");

    if total == 0 {
        // No calls to check -- pass vacuously.
        return;
    }

    // Every resolved call (callee_symbol_id IS NOT NULL) should have a
    // non-'unresolved' resolution string.
    let bad_resolution: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calls WHERE callee_symbol_id IS NOT NULL AND resolution = 'unresolved'",
            [],
            |r| r.get(0),
        )
        .expect("Query should succeed");

    assert_eq!(
        bad_resolution, 0,
        "Resolved calls must have a resolution strategy set, but {bad_resolution} still say 'unresolved'"
    );

    // Also check that the resolution field is one of the known values.
    let known_resolutions = [
        "import_guided",
        "same_file",
        "dotted_same_file",
        "dotted_import_guided",
        "dotted_same_service",
        "same_service",
        "global",
        "external",
        "builtin",
        "method_call",
        "local",
        "unresolved",
    ];
    let placeholders: String = known_resolutions
        .iter()
        .map(|r| format!("'{r}'"))
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!("SELECT COUNT(*) FROM calls WHERE resolution NOT IN ({placeholders})");
    let unknown: i64 = conn
        .query_row(&query, [], |r| r.get(0))
        .expect("Query should succeed");

    assert_eq!(
        unknown, 0,
        "All resolution values should be known strategies, but {unknown} have unexpected values"
    );
}

#[test]
fn test_resolve_calls_empty_db_succeeds() {
    // resolve_calls on a fresh database with schema but no data should not error.
    let db = Database::open_in_memory().expect("Failed to open in-memory DB");
    resolve_calls(&db).expect("resolve_calls on empty DB should succeed without errors");
}
