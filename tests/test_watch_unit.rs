#![cfg(feature = "watch")]

use std::fs;
use std::path::Path;
use std::time::Duration;

use ariadne::config::RepoConfig;
use ariadne::db::Database;
use ariadne::pipeline::run_full_pipeline;
use ariadne::watch::{incremental, FileWatcher};

#[test]
fn test_file_watcher_creation() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let _watcher = FileWatcher::new(dir.path()).expect("should create FileWatcher");
}

#[test]
fn test_poll_empty_returns_empty() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let watcher = FileWatcher::new(dir.path()).expect("should create FileWatcher");

    let events = watcher.poll_events(Duration::from_millis(100));
    assert!(
        events.is_empty(),
        "polling a quiet directory should return no events"
    );
}

#[test]
fn test_file_watcher_detects_new_file() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let watcher = FileWatcher::new(dir.path()).expect("should create FileWatcher");

    // Give the watcher a moment to initialize
    std::thread::sleep(Duration::from_millis(200));

    // Write a new file into the watched directory
    let file_path = dir.path().join("hello.py");
    fs::write(&file_path, "def hello():\n    pass\n").expect("should write file");

    // Poll with a generous timeout to allow the OS to deliver the event
    let events = watcher.poll_events(Duration::from_secs(2));
    assert!(
        !events.is_empty(),
        "watcher should detect the new file creation"
    );

    // Verify at least one event references our file
    let has_our_file = events
        .iter()
        .any(|e| e.paths.iter().any(|p| p.ends_with("hello.py")));
    assert!(
        has_our_file,
        "at least one event should reference hello.py, got events: {:?}",
        events
    );
}

// ---------------------------------------------------------------------------
// Incremental reindex correctness — the watch path must produce the same
// index a full pipeline run would. Every divergence here silently corrupts
// answers served to agents for the rest of the session.
// ---------------------------------------------------------------------------

/// Helper: full-pipeline index of a real directory, DB kept in its own tempdir.
fn indexed(root: &Path) -> Database {
    let db = Database::open_in_memory().expect("in-memory db");
    run_full_pipeline(&db, root, &RepoConfig::default()).expect("pipeline");
    db
}

fn count(db: &Database, sql: &str) -> i64 {
    db.conn()
        .query_row(sql, [], |r| r.get(0))
        .expect("count query")
}

#[test]
fn test_incremental_reindex_marks_test_file_symbols() {
    // The full pipeline marks every symbol in tests/test_*.rs as a test so
    // dead-code seeds them and affected_tests can find them. The watch path
    // must apply the same rule, or saving a test file strips its status.
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .unwrap();
    fs::write(
        root.join("tests/test_int.rs"),
        "pub fn check_add() { assert_eq!(4, 4); }\n",
    )
    .unwrap();

    let db = indexed(root);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM symbols WHERE name = 'check_add' AND is_test = 1"
        ),
        1,
        "precondition: full pipeline marks integration-test symbols"
    );

    // Simulate a watch-mode save of the test file with a new helper added.
    fs::write(
        root.join("tests/test_int.rs"),
        "pub fn check_add() { assert_eq!(4, 4); }\npub fn check_more() { assert_eq!(5, 5); }\n",
    )
    .unwrap();
    let changed = root.join("tests/test_int.rs");
    incremental::reindex_files(&db, root, &[&changed]).expect("reindex");

    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM symbols WHERE name = 'check_add' AND is_test = 1"
        ),
        1,
        "reindexed test file must keep its test marking"
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM symbols WHERE name = 'check_more' AND is_test = 1"
        ),
        1,
        "symbols added under watch must get the same test marking the full pipeline applies"
    );
}

#[test]
fn test_incremental_reindex_preserves_service_assignment() {
    use ariadne::db::write;

    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("gateway")).unwrap();
    fs::create_dir_all(root.join("auth")).unwrap();
    fs::write(root.join("gateway/g.py"), "def gate():\n    pass\n").unwrap();
    fs::write(root.join("auth/a.py"), "def check():\n    pass\n").unwrap();

    let db = Database::open_in_memory().expect("in-memory db");
    let gateway = write::insert_service(
        &db,
        "gateway",
        &root.join("gateway").to_string_lossy(),
        "service",
        "python",
    )
    .unwrap();
    let auth = write::insert_service(
        &db,
        "auth",
        &root.join("auth").to_string_lossy(),
        "service",
        "python",
    )
    .unwrap();
    // g.py is already indexed under gateway; a.py is new to the index.
    let g_abs = root.join("gateway/g.py");
    write::insert_file(
        &db,
        gateway,
        "gateway/g.py",
        &g_abs.to_string_lossy(),
        "python",
        0.0,
    )
    .unwrap();

    let a_abs = root.join("auth/a.py");
    incremental::reindex_files(&db, root, &[&g_abs, &a_abs]).expect("reindex");

    let g_service: i64 = db
        .conn()
        .query_row(
            "SELECT service_id FROM files WHERE absolute_path = ?1",
            [g_abs.to_string_lossy()],
            |r| r.get(0),
        )
        .unwrap();
    let a_service: i64 = db
        .conn()
        .query_row(
            "SELECT service_id FROM files WHERE absolute_path = ?1",
            [a_abs.to_string_lossy()],
            |r| r.get(0),
        )
        .unwrap();

    assert_eq!(
        g_service, gateway,
        "existing files keep their service on reindex"
    );
    assert_eq!(
        a_service, auth,
        "new files must map to the service whose path prefixes them — \
         defaulting to the first service breaks cross-service tracing under watch"
    );
}

#[test]
fn test_incremental_reindex_keeps_fts_search_fresh() {
    // search_symbol rides on symbols_fts. delete_file_data removes a file's
    // FTS rows and insert_symbol re-adds them — if either half regresses,
    // every file edited under watch goes invisible to search. A full-index
    // test can't catch that: the pipeline's FTS rebuild phase masks it.
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/a.py"), "def omega():\n    pass\n").unwrap();

    let db = indexed(root);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM symbols_fts WHERE symbols_fts MATCH 'omega'"
        ),
        1,
        "precondition: symbol searchable after full index"
    );

    fs::write(root.join("src/a.py"), "def sigma():\n    pass\n").unwrap();
    let changed = root.join("src/a.py");
    incremental::reindex_files(&db, root, &[&changed]).expect("reindex");

    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM symbols_fts WHERE symbols_fts MATCH 'sigma'"
        ),
        1,
        "renamed symbol must be findable via FTS right after the watch reindex"
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM symbols_fts WHERE symbols_fts MATCH 'omega'"
        ),
        0,
        "the old name must leave the FTS index — ghost hits erode trust in search"
    );
}

#[test]
fn test_incremental_relabels_stale_edges_after_edit() {
    // Cross-file scenario: b.py calls helper() defined in a.py. Editing a.py
    // deletes and re-inserts its symbols; the FK nulls b.py's resolved edge
    // but leaves its old label. Post-reindex resolution must reset that row
    // — otherwise it stays counted as resolved while pointing at nothing.
    const POINTING_LABELS: &str =
        "('import_guided','same_file','dotted_same_file','dotted_import_guided',\
          'dotted_same_service','import_file_affinity','same_service','global')";

    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/a.py"), "def helper():\n    pass\n").unwrap();
    fs::write(root.join("src/b.py"), "def caller():\n    helper()\n").unwrap();

    let db = indexed(root);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM calls WHERE callee_name = 'helper' \
             AND callee_symbol_id IS NOT NULL",
        ),
        1,
        "precondition: cross-file call resolves after full index"
    );

    // Rename helper out from under b.py, watch-style.
    fs::write(root.join("src/a.py"), "def renamed_helper():\n    pass\n").unwrap();
    let changed = root.join("src/a.py");
    incremental::reindex_files(&db, root, &[&changed]).expect("reindex");
    incremental::run_post_reindex_resolution(&db, root, &RepoConfig::default())
        .expect("post-reindex resolution");

    assert_eq!(
        count(
            &db,
            &format!(
                "SELECT COUNT(*) FROM calls WHERE callee_symbol_id IS NULL \
                 AND resolution IN {POINTING_LABELS}"
            ),
        ),
        0,
        "no call may claim a pointing resolution while its target is NULL — \
         those rows inflate resolution stats and mislabel graph edges"
    );
    let (resolution, callee): (String, Option<i64>) = db
        .conn()
        .query_row(
            "SELECT resolution, callee_symbol_id FROM calls WHERE callee_name = 'helper'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(
        callee.is_none(),
        "helper no longer exists; the edge cannot stay resolved"
    );
    assert!(
        !POINTING_LABELS.contains(&format!("'{resolution}'")),
        "stale edge must be relabeled, still says {resolution}"
    );
}

#[test]
fn test_incremental_dead_code_stays_correct_after_reindex() {
    // Route handlers stay alive through export/entry-point seeding that the
    // full pipeline computes. A watch reindex of their file must not flip
    // them dead — "this handler is dead" is the worst advice an agent can
    // relay. unusedHelper must also STAY dead: re-marking must not
    // resurrect genuinely dead code either.
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/express_app");
    let db = indexed(&fixture);

    let dead_before = count(
        &db,
        "SELECT COUNT(*) FROM symbols WHERE name = 'getUsers' AND is_dead = 1",
    );
    assert_eq!(
        dead_before, 0,
        "precondition: exported route handler is alive"
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM symbols WHERE name = 'unusedHelper' AND is_dead = 1"
        ),
        1,
        "precondition: genuinely unused helper is dead"
    );

    let changed = fixture.join("src/routes/users.js");
    incremental::reindex_files(&db, &fixture, &[&changed]).expect("reindex");
    incremental::run_post_reindex_resolution(&db, &fixture, &RepoConfig::default())
        .expect("post-reindex resolution");

    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM symbols WHERE name = 'getUsers' AND is_dead = 1"
        ),
        0,
        "route handler must stay alive after its file is reindexed under watch"
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM symbols WHERE name = 'unusedHelper' AND is_dead = 1"
        ),
        1,
        "dead code must stay dead after a watch reindex"
    );
}
