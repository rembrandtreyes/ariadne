use ariadne::db::{query, write, Database};

/// Create a test DB with a service and 3 files for coupling tests.
fn setup_coupling_db() -> (Database, i64, i64, i64) {
    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test-svc", "/tmp/test", "monolith", "python")
        .expect("insert service");
    let f1 = write::insert_file(
        &db,
        svc,
        "src/auth.py",
        "/tmp/test/src/auth.py",
        "python",
        0.0,
    )
    .expect("insert file a");
    let f2 = write::insert_file(
        &db,
        svc,
        "src/users.py",
        "/tmp/test/src/users.py",
        "python",
        0.0,
    )
    .expect("insert file b");
    let f3 = write::insert_file(&db, svc, "src/db.py", "/tmp/test/src/db.py", "python", 0.0)
        .expect("insert file c");
    (db, f1, f2, f3)
}

#[test]
fn test_insert_and_query_coupling() {
    let (db, f1, f2, _f3) = setup_coupling_db();

    write::insert_coupling(&db, f1, f2, 5, 0.25).expect("insert coupling");

    let results = query::get_file_couplings(&db, f1).expect("query couplings");
    assert_eq!(results.len(), 1, "should have 1 coupling record");
    assert_eq!(results[0].co_changes, 5);
    assert!((results[0].strength - 0.25).abs() < f64::EPSILON);
    assert!(
        results[0].coupled_path.contains("users.py"),
        "coupled_path should reference the other file, got: {}",
        results[0].coupled_path
    );
}

#[test]
fn test_get_top_couplings_ordering() {
    let (db, f1, f2, f3) = setup_coupling_db();

    write::insert_coupling(&db, f1, f2, 2, 0.1).expect("insert low");
    write::insert_coupling(&db, f1, f3, 8, 0.5).expect("insert high");
    write::insert_coupling(&db, f2, f3, 4, 0.3).expect("insert mid");

    let results = query::get_top_couplings(&db, 10).expect("query top couplings");
    assert_eq!(results.len(), 3, "should have 3 coupling records");
    assert!(
        results[0].strength >= results[1].strength,
        "first should have highest strength"
    );
    assert!(
        results[1].strength >= results[2].strength,
        "second should be >= third"
    );
    assert!((results[0].strength - 0.5).abs() < f64::EPSILON);
    assert!((results[2].strength - 0.1).abs() < f64::EPSILON);
}

#[test]
fn test_coupling_upsert_replaces() {
    let (db, f1, f2, _f3) = setup_coupling_db();

    write::insert_coupling(&db, f1, f2, 2, 0.1).expect("insert first");
    write::insert_coupling(&db, f1, f2, 7, 0.4).expect("insert replacement");

    let results = query::get_top_couplings(&db, 10).expect("query couplings");
    assert_eq!(
        results.len(),
        1,
        "INSERT OR REPLACE should keep only 1 record"
    );
    assert_eq!(results[0].co_changes, 7, "co_changes should be updated");
    assert!((results[0].strength - 0.4).abs() < f64::EPSILON);
}

#[test]
fn test_get_file_couplings_bidirectional() {
    let (db, f1, f2, _f3) = setup_coupling_db();

    write::insert_coupling(&db, f1, f2, 3, 0.2).expect("insert coupling");

    // Query from file_a side
    let from_a = query::get_file_couplings(&db, f1).expect("query from a");
    assert_eq!(from_a.len(), 1, "should find coupling from file_a side");

    // Query from file_b side
    let from_b = query::get_file_couplings(&db, f2).expect("query from b");
    assert_eq!(from_b.len(), 1, "should find coupling from file_b side");

    // Both should reference the same coupling
    assert_eq!(from_a[0].co_changes, from_b[0].co_changes);
}
