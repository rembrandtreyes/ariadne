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

// ---------------------------------------------------------------------------
// Gap 4/5 — renamed imports, route→lib alias edges, dangling imports,
// deterministic name-fallback tie-breaks
// ---------------------------------------------------------------------------

/// Helper: run the full pipeline over an ad-hoc TS fixture written to a
/// tempdir. Returns (db, tempdir guard).
fn setup_ts_fixture(files: &[(&str, &str)]) -> (Database, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    for (rel, content) in files {
        let path = tmp.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    let db = Database::open_in_memory().expect("Failed to open in-memory DB");
    run_full_pipeline(&db, tmp.path(), &RepoConfig::default()).expect("Pipeline should succeed");
    (db, tmp)
}

#[test]
fn test_renamed_import_call_resolves_to_original_symbol() {
    // `import { helper as h }` + `h()` must produce a call edge to the
    // `helper` symbol in the target file, import-guided.
    let (db, _tmp) = setup_ts_fixture(&[
        ("src/util.ts", "export function helper() { return 1; }\n"),
        (
            "src/app.ts",
            "import { helper as h } from './util';\nexport function run() { return h(); }\n",
        ),
    ]);
    let conn = db.conn();
    let (resolution, callee_name): (String, String) = conn
        .query_row(
            "SELECT c.resolution, s.name FROM calls c
             JOIN symbols s ON s.id = c.callee_symbol_id
             WHERE c.callee_name = 'h'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("call h() must resolve to a symbol");
    assert_eq!(
        callee_name, "helper",
        "h() must resolve to the original symbol"
    );
    assert_eq!(resolution, "import_guided");
}

#[test]
fn test_route_to_lib_call_edge_via_alias_import() {
    // Next.js shape: route handler imports lib fn via tsconfig alias; the
    // call edge route→lib must land (this is the timbre analyzeVoice repro).
    let (db, _tmp) = setup_ts_fixture(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions": {"paths": {"@/*": ["./src/*"]}}}"#,
        ),
        (
            "src/lib/voice.ts",
            "export function analyzeVoice(x: string) { return x; }\n",
        ),
        (
            "src/app/api/voice/route.ts",
            "import { analyzeVoice } from '@/lib/voice';\n\
             export async function POST() { return analyzeVoice('a'); }\n",
        ),
    ]);
    let conn = db.conn();
    let (resolution, target_path): (String, String) = conn
        .query_row(
            "SELECT c.resolution, f.path FROM calls c
             JOIN symbols s ON s.id = c.callee_symbol_id
             JOIN files f ON f.id = s.file_id
             JOIN files cf ON cf.id = c.file_id
             WHERE c.callee_name = 'analyzeVoice' AND cf.path LIKE '%route.ts'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("route call to analyzeVoice must resolve");
    assert_eq!(target_path, "src/lib/voice.ts");
    assert_eq!(resolution, "import_guided");
}

#[test]
fn test_dangling_import_after_rename_is_queryable() {
    // After a rename, a stale import of the old name must resolve the FILE
    // but leave resolved_symbol_id NULL — the queryable dangling signal.
    let (db, _tmp) = setup_ts_fixture(&[
        (
            "src/post.ts",
            "export function measureContractionRateV2() { return 2; }\n",
        ),
        (
            "src/check.ts",
            "import { measureContractionRate } from './post';\n\
             export function probe() { return measureContractionRate; }\n",
        ),
    ]);
    let conn = db.conn();
    let (resolved_file, resolved_symbol): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT resolved_file_id, resolved_symbol_id FROM imports
             WHERE imported_name = 'measureContractionRate'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("import row exists");
    assert!(resolved_file.is_some(), "file must resolve");
    assert!(
        resolved_symbol.is_none(),
        "stale symbol name must stay unresolved — that is the dangling-import signal"
    );
}

#[test]
fn test_same_service_fallback_tiebreak_is_path_ordered() {
    // Two exported symbols share a name in one service; a call with no
    // import context falls to pass 4 (same_service). The winner must be
    // decided by (is_exported DESC, path ASC, ...) — never by insert order.
    let db = Database::open_in_memory().expect("db");
    let conn = db.conn();
    conn.execute(
        "INSERT INTO services (name, type, repo_path) VALUES ('svc', 'monolith', '/tmp/svc')",
        [],
    )
    .unwrap();
    let mut file_ids = std::collections::HashMap::new();
    // Insert the LEXICOGRAPHICALLY LATER path first so lowest-rowid != path-asc.
    for path in ["src/zz_dup.ts", "src/aa_dup.ts", "src/caller.ts"] {
        conn.execute(
            "INSERT INTO files (service_id, path, absolute_path, language, last_modified, last_indexed)
             VALUES (1, ?1, ?1, 'typescript', 0.0, 0.0)",
            rusqlite::params![path],
        )
        .unwrap();
        file_ids.insert(path, conn.last_insert_rowid());
    }
    let mut sym_ids = std::collections::HashMap::new();
    for (path, name) in [
        ("src/zz_dup.ts", "dup"),
        ("src/aa_dup.ts", "dup"),
        ("src/caller.ts", "caller"),
    ] {
        conn.execute(
            "INSERT INTO symbols (file_id, name, qualified_name, kind, line_start, line_end, is_exported)
             VALUES (?1, ?2, ?2, 'function', 1, 5, 1)",
            rusqlite::params![file_ids[path], name],
        )
        .unwrap();
        sym_ids.insert(path, conn.last_insert_rowid());
    }
    conn.execute(
        "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line)
         VALUES (?1, NULL, 'dup', ?2, 3)",
        rusqlite::params![sym_ids["src/caller.ts"], file_ids["src/caller.ts"]],
    )
    .unwrap();

    resolve_calls(&db).expect("resolve");

    let conn = db.conn();
    let (winner_path, resolution): (String, String) = conn
        .query_row(
            "SELECT f.path, c.resolution FROM calls c
             JOIN symbols s ON s.id = c.callee_symbol_id
             JOIN files f ON f.id = s.file_id
             WHERE c.callee_name = 'dup'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("dup call resolved");
    assert_eq!(resolution, "same_service");
    assert_eq!(
        winner_path, "src/aa_dup.ts",
        "tie-break must be path-ordered, not insert-ordered"
    );
}

#[test]
fn test_name_fallback_never_resolves_to_unexported_symbols() {
    // Cross-file name-only fallback (passes 4/5) must not land on a symbol
    // that is not exported — a cross-file call to a file-private symbol is
    // impossible in every supported language's semantics.
    let db = Database::open_in_memory().expect("db");
    let conn = db.conn();
    conn.execute(
        "INSERT INTO services (name, type, repo_path) VALUES ('svc', 'monolith', '/tmp/svc')",
        [],
    )
    .unwrap();
    for (id, path) in [(1, "src/private_home.ts"), (2, "src/caller.ts")] {
        conn.execute(
            "INSERT INTO files (id, service_id, path, absolute_path, language, last_modified, last_indexed)
             VALUES (?1, 1, ?2, ?2, 'typescript', 0.0, 0.0)",
            rusqlite::params![id, path],
        )
        .unwrap();
    }
    // `t` is a NON-exported local in private_home.ts.
    conn.execute(
        "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end, is_exported)
         VALUES (1, 1, 't', 't', 'constant', 3, 3, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end, is_exported)
         VALUES (2, 2, 'Caller', 'Caller', 'function', 1, 9, 1)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line)
         VALUES (2, NULL, 't', 2, 5)",
        [],
    )
    .unwrap();

    resolve_calls(&db).expect("resolve");

    let resolved: Option<i64> = db
        .conn()
        .query_row(
            "SELECT callee_symbol_id FROM calls WHERE callee_name = 't'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        resolved, None,
        "name fallback must not resolve a cross-file call to an unexported symbol"
    );
}
