use ariadne::db::{query, write, Database};

/// Create a test DB with a service, file, and symbols for history tests.
fn setup_history_db() -> (Database, i64, i64, i64, i64) {
    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test-svc", "/tmp/test", "monolith", "rust")
        .expect("insert service");
    let file_id = write::insert_file(
        &db,
        svc,
        "src/auth.rs",
        "/tmp/test/src/auth.rs",
        "rust",
        0.0,
    )
    .expect("insert file");

    let sym1 = write::insert_symbol(
        &db,
        file_id,
        "login",
        "auth::login",
        "function",
        1,
        20,
        true,
        false,
        "fn login()",
        "",
        None,
    )
    .expect("insert symbol 1");
    let sym2 = write::insert_symbol(
        &db,
        file_id,
        "hash_pw",
        "auth::hash_pw",
        "function",
        25,
        40,
        false,
        false,
        "fn hash_pw()",
        "",
        None,
    )
    .expect("insert symbol 2");

    (db, svc, file_id, sym1, sym2)
}

#[test]
fn test_insert_and_query_symbol_history() {
    let (db, _svc, _file_id, sym1, _sym2) = setup_history_db();

    write::insert_symbol_history(&db, sym1, Some(1000), Some(5000), 7, 3, false)
        .expect("insert history");

    let history = query::get_symbol_history(&db, sym1)
        .expect("query history")
        .expect("history should exist");

    assert_eq!(history.symbol_id, sym1);
    assert_eq!(history.symbol_name, "login");
    assert_eq!(history.qualified_name, "auth::login");
    assert_eq!(history.kind, "function");
    assert_eq!(history.file_path, "src/auth.rs");
    assert_eq!(history.created_at, Some(1000));
    assert_eq!(history.last_modified_at, Some(5000));
    assert_eq!(history.modification_count, 7);
    assert_eq!(history.author_count, 3);
    assert!(!history.is_volatile);
}

#[test]
fn test_symbol_history_not_found() {
    let (db, _svc, _file_id, _sym1, _sym2) = setup_history_db();

    // Query without inserting history
    let history = query::get_symbol_history(&db, 9999).expect("query should succeed");
    assert!(history.is_none(), "should return None for missing history");
}

#[test]
fn test_symbol_history_volatile_flag() {
    let (db, _svc, _file_id, sym1, sym2) = setup_history_db();

    // sym1: not volatile
    write::insert_symbol_history(&db, sym1, Some(1000), Some(5000), 2, 1, false)
        .expect("insert non-volatile");
    // sym2: volatile (>3 modifications in last 30 days)
    write::insert_symbol_history(&db, sym2, Some(1000), Some(5000), 8, 4, true)
        .expect("insert volatile");

    let h1 = query::get_symbol_history(&db, sym1)
        .expect("query")
        .expect("exists");
    let h2 = query::get_symbol_history(&db, sym2)
        .expect("query")
        .expect("exists");

    assert!(!h1.is_volatile);
    assert!(h2.is_volatile);
}

#[test]
fn test_symbol_history_upsert() {
    let (db, _svc, _file_id, sym1, _sym2) = setup_history_db();

    write::insert_symbol_history(&db, sym1, Some(1000), Some(3000), 3, 1, false)
        .expect("insert first");
    write::insert_symbol_history(&db, sym1, Some(1000), Some(6000), 10, 5, true).expect("upsert");

    let history = query::get_symbol_history(&db, sym1)
        .expect("query")
        .expect("exists");

    assert_eq!(history.modification_count, 10, "should be updated");
    assert_eq!(history.author_count, 5, "should be updated");
    assert!(history.is_volatile, "should be updated to volatile");
    assert_eq!(history.last_modified_at, Some(6000));
}

#[test]
fn test_symbol_history_null_timestamps() {
    let (db, _svc, _file_id, sym1, _sym2) = setup_history_db();

    // No git history available — timestamps are None
    write::insert_symbol_history(&db, sym1, None, None, 0, 0, false)
        .expect("insert with null timestamps");

    let history = query::get_symbol_history(&db, sym1)
        .expect("query")
        .expect("exists");

    assert_eq!(history.created_at, None);
    assert_eq!(history.last_modified_at, None);
    assert_eq!(history.modification_count, 0);
    assert_eq!(history.author_count, 0);
}

#[test]
fn test_clear_all_data_includes_symbol_history() {
    let (db, _svc, _file_id, sym1, _sym2) = setup_history_db();

    write::insert_symbol_history(&db, sym1, Some(1000), Some(5000), 5, 2, false)
        .expect("insert history");

    // Verify it exists
    let exists = query::get_symbol_history(&db, sym1)
        .expect("query")
        .is_some();
    assert!(exists, "should exist before clear");

    // Clear and re-create schema (clear_all_data removes all rows)
    write::clear_all_data(&db).expect("clear all data");

    // After clear, the table still exists but is empty
    // Can't query by sym1 anymore since symbols table is also cleared
    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM symbol_history", [], |row| row.get(0))
        .expect("count query");
    assert_eq!(count, 0, "symbol_history should be empty after clear");
}
