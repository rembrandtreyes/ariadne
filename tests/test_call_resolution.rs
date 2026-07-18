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
        "import_file_affinity",
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

#[test]
fn test_stale_resolution_labels_reset_when_target_symbol_vanishes() {
    // Watch-mode reindexing deletes a file's symbols; calls.callee_symbol_id
    // is ON DELETE SET NULL, which nulls the target but leaves the old
    // resolution label and confidence behind. Re-running resolution must
    // reset those rows — most passes skip anything not labeled 'unresolved',
    // so a stale label otherwise pins the edge in a lying state forever
    // (NULL target, 0.98 confidence, counted as resolved).
    use ariadne::db::write;

    let db = Database::open_in_memory().expect("Failed to open in-memory DB");
    let svc = write::insert_service(&db, "test", "/tmp/t", "monolith", "rust").unwrap();
    let file_a = write::insert_file(&db, svc, "src/a.rs", "/tmp/t/src/a.rs", "rust", 0.0).unwrap();
    let file_b = write::insert_file(&db, svc, "src/b.rs", "/tmp/t/src/b.rs", "rust", 0.0).unwrap();

    write::insert_symbol(
        &db, file_a, "foo", "a::foo", "function", 1, 3, true, false, "", "", None,
    )
    .unwrap();
    let bar = write::insert_symbol(
        &db, file_b, "bar", "b::bar", "function", 1, 5, true, false, "", "", None,
    )
    .unwrap();

    // b.rs imports foo from a.rs (already resolved) and bar() calls foo().
    // All interpolated values are i64 ids returned by the insert helpers.
    db.conn()
        .execute_batch(&format!(
            "INSERT INTO imports (file_id, imported_name, module_path, resolved_file_id, line)
             VALUES ({file_b}, 'foo', 'a', {file_a}, 1);
             INSERT INTO calls (caller_symbol_id, callee_name, file_id, line, confidence, resolution)
             VALUES ({bar}, 'foo', {file_b}, 2, 0.5, 'unresolved');"
        ))
        .unwrap();

    resolve_calls(&db).expect("initial resolution should succeed");
    let (resolution, callee): (String, Option<i64>) = db
        .conn()
        .query_row("SELECT resolution, callee_symbol_id FROM calls", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(
        resolution, "import_guided",
        "precondition: call resolves via the import"
    );
    assert!(callee.is_some(), "precondition: call points at foo");

    // Simulate the watch path: a.rs is re-indexed, its old symbols deleted.
    write::delete_file_data(&db, file_a).expect("delete file data");

    let (stale_resolution, callee): (String, Option<i64>) = db
        .conn()
        .query_row("SELECT resolution, callee_symbol_id FROM calls", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert!(
        callee.is_none(),
        "FK SET NULL must clear the vanished target"
    );
    assert_eq!(
        stale_resolution, "import_guided",
        "precondition: the stale label survives the FK action — this is the lie"
    );

    // Re-running resolution must reset the label instead of skipping the row.
    resolve_calls(&db).expect("re-resolution should succeed");
    let (resolution, confidence, callee): (String, f64, Option<i64>) = db
        .conn()
        .query_row(
            "SELECT resolution, confidence, callee_symbol_id FROM calls",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert!(
        callee.is_none(),
        "foo no longer exists; the call cannot re-resolve"
    );
    assert_ne!(
        resolution, "import_guided",
        "a NULL-target call must not keep a pointing label after re-resolution"
    );
    assert!(
        confidence <= 0.5,
        "confidence must drop with the reset, got {confidence}"
    );
}
