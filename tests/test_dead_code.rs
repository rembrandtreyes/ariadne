use ariadne::config::RepoConfig;
use ariadne::db::query::{count_dead, get_dead_symbols};
use ariadne::db::Database;
use ariadne::pipeline::dead_code::detect_dead_code;
use ariadne::pipeline::run_full_pipeline;
use std::path::Path;

/// Running detect_dead_code on a fresh in-memory database should succeed
/// and report zero dead symbols since there are no symbols at all.
#[test]
fn test_dead_code_empty_db_succeeds() {
    let db = Database::open_in_memory().unwrap();
    let result = detect_dead_code(&db);
    assert!(
        result.is_ok(),
        "detect_dead_code should succeed on empty DB"
    );
    let dead_count = count_dead(&db).unwrap();
    assert_eq!(dead_count, 0, "empty DB should have zero dead symbols");
}

/// After indexing python_repo, dead code detection should identify at least
/// one unreachable function. All symbols returned by get_dead_symbols must
/// have is_dead set to true.
#[test]
fn test_dead_code_detects_unreachable_function() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let config = RepoConfig::default();
    let fixture = Path::new("tests/fixtures/python_repo");

    run_full_pipeline(&db, fixture, &config).unwrap();

    let dead_symbols = get_dead_symbols(&db).unwrap();
    assert!(
        !dead_symbols.is_empty(),
        "expected at least one dead symbol in python_repo"
    );
    for sym in &dead_symbols {
        assert!(
            sym.is_dead,
            "symbol '{}' returned by get_dead_symbols should have is_dead=true",
            sym.name
        );
    }
}

/// Entry points (main, run, etc.) must never be marked as dead code.
/// Verifies the invariant: no symbol can be both dead and an entry point.
#[test]
fn test_dead_code_entry_points_not_dead() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let config = RepoConfig::default();
    let fixture = Path::new("tests/fixtures/python_repo");

    run_full_pipeline(&db, fixture, &config).unwrap();

    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM symbols WHERE is_dead = 1 AND is_entry_point = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 0,
        "no entry point should be marked as dead, found {}",
        count
    );
}

/// Test functions must never be marked as dead code, even if they are
/// not called by any other symbol.
#[test]
fn test_dead_code_tests_not_dead() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let config = RepoConfig::default();
    let fixture = Path::new("tests/fixtures/rust_repo");

    run_full_pipeline(&db, fixture, &config).unwrap();

    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM symbols WHERE is_dead = 1 AND is_test = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 0,
        "no test symbol should be marked as dead, found {}",
        count
    );
}

/// Exported symbols must never be marked as dead code since they may
/// be consumed by external callers we cannot see.
#[test]
fn test_dead_code_exported_not_dead() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let config = RepoConfig::default();
    let fixture = Path::new("tests/fixtures/rust_repo");

    run_full_pipeline(&db, fixture, &config).unwrap();

    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM symbols WHERE is_dead = 1 AND is_exported = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 0,
        "no exported symbol should be marked as dead, found {}",
        count
    );
}
