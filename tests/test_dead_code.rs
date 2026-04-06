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
    let config = RepoConfig::default();
    let result = detect_dead_code(&db, &config);
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

/// Custom entry points from config must be respected by dead code detection.
/// A symbol named in entry_points should never be marked dead.
#[test]
fn test_dead_code_custom_entry_points_respected() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let fixture = Path::new("tests/fixtures/python_repo");

    // First run with default config to find dead symbols
    let default_config = RepoConfig::default();
    run_full_pipeline(&db, fixture, &default_config).unwrap();

    let dead_before = get_dead_symbols(&db).unwrap();
    if dead_before.is_empty() {
        // If no dead symbols, we can't test the entry_points override
        return;
    }

    // Pick the first dead symbol and mark it as a custom entry point
    let target_name = dead_before[0].name.clone();

    let custom_config = RepoConfig {
        entry_points: Some(vec![target_name.clone()]),
        ..Default::default()
    };

    // Re-run pipeline with custom entry points
    run_full_pipeline(&db, fixture, &custom_config).unwrap();

    let dead_after = get_dead_symbols(&db).unwrap();
    let still_dead: Vec<_> = dead_after
        .iter()
        .filter(|s| s.name == target_name)
        .collect();
    assert!(
        still_dead.is_empty(),
        "symbol '{}' was listed in entry_points but is still marked dead",
        target_name
    );
}

/// Default behavior should be preserved when no entry_points are configured.
/// This ensures the entry_points feature is purely additive.
#[test]
fn test_dead_code_default_behavior_preserved() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let fixture = Path::new("tests/fixtures/python_repo");

    // Run with default config (entry_points = None)
    let config = RepoConfig::default();
    run_full_pipeline(&db, fixture, &config).unwrap();
    let dead_default = count_dead(&db).unwrap();

    // Run again with explicitly empty entry_points
    let config_empty = RepoConfig {
        entry_points: Some(vec![]),
        ..Default::default()
    };
    run_full_pipeline(&db, fixture, &config_empty).unwrap();
    let dead_empty = count_dead(&db).unwrap();

    assert_eq!(
        dead_default, dead_empty,
        "empty entry_points should produce same results as None"
    );
}
