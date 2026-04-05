use ariadne::config::RepoConfig;
use ariadne::db::query::get_dead_symbols;
use ariadne::db::Database;
use ariadne::pipeline::run_full_pipeline;
use std::path::Path;

/// Helper: index a fixture and return the names of dead symbols.
fn dead_symbol_names(fixture: &str) -> Vec<String> {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let config = RepoConfig::default();
    let fixture_path = Path::new(fixture);

    run_full_pipeline(&db, fixture_path, &config).unwrap();

    get_dead_symbols(&db)
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect()
}

/// Helper: index a fixture and return names of symbols marked as entry points.
fn entry_point_names(fixture: &str) -> Vec<String> {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    let config = RepoConfig::default();
    let fixture_path = Path::new(fixture);

    run_full_pipeline(&db, fixture_path, &config).unwrap();

    let mut stmt = db
        .conn()
        .prepare("SELECT name FROM symbols WHERE is_entry_point = 1")
        .unwrap();
    stmt.query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<String>, _>>()
        .unwrap()
}

// ── Next.js Tests ───────────────────────────────────────────────────────

/// Next.js page default exports must be marked as entry points.
#[test]
fn test_nextjs_page_default_export_is_entry_point() {
    let entries = entry_point_names("tests/fixtures/nextjs_app");
    assert!(
        entries.contains(&"Page".to_string()),
        "Page component from page.tsx should be an entry point, got: {:?}",
        entries
    );
}

/// Next.js layout default exports must be marked as entry points.
#[test]
fn test_nextjs_layout_is_entry_point() {
    let entries = entry_point_names("tests/fixtures/nextjs_app");
    assert!(
        entries.contains(&"RootLayout".to_string()),
        "RootLayout from layout.tsx should be an entry point, got: {:?}",
        entries
    );
}

/// Next.js API route exports (GET, POST) must be marked as entry points.
#[test]
fn test_nextjs_api_route_handlers_are_entry_points() {
    let entries = entry_point_names("tests/fixtures/nextjs_app");
    assert!(
        entries.contains(&"GET".to_string()),
        "GET from route.ts should be an entry point, got: {:?}",
        entries
    );
    assert!(
        entries.contains(&"POST".to_string()),
        "POST from route.ts should be an entry point, got: {:?}",
        entries
    );
}

/// Next.js generateMetadata must be marked as entry point.
#[test]
fn test_nextjs_generate_metadata_is_entry_point() {
    let entries = entry_point_names("tests/fixtures/nextjs_app");
    assert!(
        entries.contains(&"generateMetadata".to_string()),
        "generateMetadata should be an entry point, got: {:?}",
        entries
    );
}

/// Next.js page/layout entry points must NOT appear as dead code.
#[test]
fn test_nextjs_entry_points_not_dead() {
    let dead = dead_symbol_names("tests/fixtures/nextjs_app");
    assert!(
        !dead.contains(&"Page".to_string()),
        "Page should not be dead"
    );
    assert!(
        !dead.contains(&"RootLayout".to_string()),
        "RootLayout should not be dead"
    );
    assert!(!dead.contains(&"GET".to_string()), "GET should not be dead");
    assert!(
        !dead.contains(&"POST".to_string()),
        "POST should not be dead"
    );
    assert!(
        !dead.contains(&"generateMetadata".to_string()),
        "generateMetadata should not be dead"
    );
}

/// Genuinely unused functions should still be detected as dead.
#[test]
fn test_nextjs_genuinely_dead_code_still_detected() {
    let dead = dead_symbol_names("tests/fixtures/nextjs_app");
    assert!(
        dead.contains(&"internalHelper".to_string()),
        "internalHelper should be dead, got: {:?}",
        dead
    );
}

// ── Express Tests ───────────────────────────────────────────────────────

/// Express exported handlers should not be dead (they're exported, which
/// already seeds BFS). This test verifies the baseline behavior holds.
#[test]
fn test_express_exported_handlers_not_dead() {
    let dead = dead_symbol_names("tests/fixtures/express_app");
    // getUsers and createUser are exported via module.exports
    // They should be reachable via export + call chain
    assert!(
        !dead.contains(&"getUsers".to_string()),
        "getUsers should not be dead, got: {:?}",
        dead
    );
    assert!(
        !dead.contains(&"createUser".to_string()),
        "createUser should not be dead, got: {:?}",
        dead
    );
}

/// Genuinely unused functions in Express apps should be dead.
#[test]
fn test_express_genuinely_dead_code_still_detected() {
    let dead = dead_symbol_names("tests/fixtures/express_app");
    assert!(
        dead.contains(&"unusedHelper".to_string()),
        "unusedHelper should be dead, got: {:?}",
        dead
    );
}

// ── Framework Detection Tests ───────────────────────────────────────────

/// Framework detection should identify Next.js from next.config.js.
#[test]
fn test_framework_detection_nextjs() {
    let config = ariadne::config::autodetect::detect(Path::new("tests/fixtures/nextjs_app"));
    assert!(
        config.frameworks.contains(&"nextjs".to_string()),
        "should detect nextjs framework, got: {:?}",
        config.frameworks
    );
}

/// Framework detection should identify node from package.json.
#[test]
fn test_framework_detection_express() {
    let config = ariadne::config::autodetect::detect(Path::new("tests/fixtures/express_app"));
    assert!(
        config.frameworks.contains(&"node".to_string()),
        "should detect node framework, got: {:?}",
        config.frameworks
    );
}
