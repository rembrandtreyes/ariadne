use ariadne::db::query;
use ariadne::db::Database;

/// Helper to create a minimal database with files and symbols for testing file dependencies.
fn setup_db_with_file_deps() -> Database {
    let db = Database::open_in_memory().expect("should open in-memory db");
    let conn = db.conn();

    // Create a service
    conn.execute(
        "INSERT INTO services (id, name, type, repo_path) VALUES (1, 'test', 'monolith', '/test')",
        [],
    )
    .unwrap();

    // Create files: a.py, b.py, c.py, d.py
    conn.execute(
        "INSERT INTO files (id, service_id, path, absolute_path, language, last_modified, last_indexed) \
         VALUES (1, 1, 'src/a.py', '/test/src/a.py', 'python', 0.0, 0.0)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO files (id, service_id, path, absolute_path, language, last_modified, last_indexed) \
         VALUES (2, 1, 'src/b.py', '/test/src/b.py', 'python', 0.0, 0.0)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO files (id, service_id, path, absolute_path, language, last_modified, last_indexed) \
         VALUES (3, 1, 'src/c.py', '/test/src/c.py', 'python', 0.0, 0.0)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO files (id, service_id, path, absolute_path, language, last_modified, last_indexed) \
         VALUES (4, 1, 'src/d.py', '/test/src/d.py', 'python', 0.0, 0.0)",
        [],
    ).unwrap();

    // Symbols: func_a in a.py, func_b in b.py, func_c in c.py, func_d in d.py
    conn.execute(
        "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end) \
         VALUES (1, 1, 'func_a', 'a.func_a', 'function', 1, 5)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end) \
         VALUES (2, 2, 'func_b', 'b.func_b', 'function', 1, 5)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end) \
         VALUES (3, 3, 'func_c', 'c.func_c', 'function', 1, 5)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end) \
         VALUES (4, 4, 'func_d', 'd.func_d', 'function', 1, 5)",
        [],
    )
    .unwrap();

    db
}

fn add_call(db: &Database, caller_sym_id: i64, callee_sym_id: i64, file_id: i64) {
    db.conn()
        .execute(
            "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution) \
             VALUES (?1, ?2, 'resolved', ?3, 10, 1.0, 'import')",
            rusqlite::params![caller_sym_id, callee_sym_id, file_id],
        )
        .unwrap();
}

#[test]
fn file_with_no_dependencies_returns_empty() {
    let db = setup_db_with_file_deps();
    // file 1 (a.py) has no calls to other files
    let deps = query::get_file_dependencies(&db, 1).unwrap();
    assert!(
        deps.is_empty(),
        "expected no dependencies, got {}",
        deps.len()
    );
}

#[test]
fn file_with_no_dependents_returns_empty() {
    let db = setup_db_with_file_deps();
    // file 1 (a.py) has nothing calling it
    let deps = query::get_file_dependents(&db, 1).unwrap();
    assert!(
        deps.is_empty(),
        "expected no dependents, got {}",
        deps.len()
    );
}

#[test]
fn direct_file_dependencies_returned_with_connections() {
    let db = setup_db_with_file_deps();
    // func_a (file 1) calls func_b (file 2) and func_c (file 3)
    add_call(&db, 1, 2, 1);
    add_call(&db, 1, 3, 1);

    let deps = query::get_file_dependencies(&db, 1).unwrap();
    assert_eq!(deps.len(), 2, "expected 2 dependencies");

    let paths: Vec<&str> = deps.iter().map(|d| d.path.as_str()).collect();
    assert!(paths.contains(&"src/b.py"), "should depend on b.py");
    assert!(paths.contains(&"src/c.py"), "should depend on c.py");

    // Check that connections include symbol names
    let b_dep = deps.iter().find(|d| d.path == "src/b.py").unwrap();
    assert_eq!(b_dep.connections.len(), 1);
    assert_eq!(b_dep.connections[0].from_symbol, "func_a");
    assert_eq!(b_dep.connections[0].to_symbol, "func_b");
}

#[test]
fn direct_file_dependents_returned_correctly() {
    let db = setup_db_with_file_deps();
    // func_a (file 1) and func_c (file 3) both call func_b (file 2)
    add_call(&db, 1, 2, 1);
    add_call(&db, 3, 2, 3);

    let deps = query::get_file_dependents(&db, 2).unwrap();
    assert_eq!(deps.len(), 2, "expected 2 dependents");

    let paths: Vec<&str> = deps.iter().map(|d| d.path.as_str()).collect();
    assert!(paths.contains(&"src/a.py"), "a.py should be a dependent");
    assert!(paths.contains(&"src/c.py"), "c.py should be a dependent");
}

#[test]
fn self_calls_excluded_from_file_dependencies() {
    let db = setup_db_with_file_deps();
    // Add a second symbol in file 1
    db.conn()
        .execute(
            "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end) \
             VALUES (10, 1, 'helper_a', 'a.helper_a', 'function', 10, 15)",
            [],
        )
        .unwrap();

    // func_a calls helper_a (same file) — should not appear as a dependency
    add_call(&db, 1, 10, 1);

    let deps = query::get_file_dependencies(&db, 1).unwrap();
    assert!(deps.is_empty(), "self-file calls should be excluded");
}

#[test]
fn circular_file_dependencies_dont_loop() {
    let db = setup_db_with_file_deps();
    // a.py -> b.py -> a.py (circular)
    add_call(&db, 1, 2, 1);
    add_call(&db, 2, 1, 2);

    // Direct deps should work fine (no infinite loop)
    let deps_a = query::get_file_dependencies(&db, 1).unwrap();
    assert_eq!(deps_a.len(), 1);
    assert_eq!(deps_a[0].path, "src/b.py");

    let deps_b = query::get_file_dependencies(&db, 2).unwrap();
    assert_eq!(deps_b.len(), 1);
    assert_eq!(deps_b[0].path, "src/a.py");
}

#[test]
fn multiple_connections_between_same_files_grouped() {
    let db = setup_db_with_file_deps();
    // Add a second symbol in file 1
    db.conn()
        .execute(
            "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end) \
             VALUES (10, 1, 'other_a', 'a.other_a', 'function', 10, 15)",
            [],
        )
        .unwrap();

    // Both func_a and other_a call func_b — should appear as one FileDependency with 2 connections
    add_call(&db, 1, 2, 1);
    add_call(&db, 10, 2, 1);

    let deps = query::get_file_dependencies(&db, 1).unwrap();
    assert_eq!(deps.len(), 1, "should be one file dependency");
    assert_eq!(deps[0].connections.len(), 2, "should have 2 connections");
}

// ---------------------------------------------------------------------------
// Import-edge union (a2 fix): references that exist only as resolved imports
// (const-only, type-only, fn-import-without-call) must appear in the
// file-level dependency surfaces, labeled by kind.
// ---------------------------------------------------------------------------

/// Two files where e.py imports a symbol from a.py but never CALLS anything:
/// the classic const-only import that the call-edge join misses.
fn add_import_only_file(db: &Database) -> i64 {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO files (id, service_id, path, absolute_path, language, last_modified, last_indexed) \
         VALUES (5, 1, 'src/e.py', '/test/src/e.py', 'python', 0.0, 0.0)",
        [],
    )
    .unwrap();
    // e.py imports CONST_A (symbol id 10) from a.py — resolved, no call rows.
    conn.execute(
        "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end) \
         VALUES (10, 1, 'CONST_A', 'a.CONST_A', 'constant', 8, 8)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO imports (file_id, imported_name, module_path, line, is_external, resolved_file_id, resolved_symbol_id) \
         VALUES (5, 'CONST_A', './a', 1, 0, 1, 10)",
        [],
    )
    .unwrap();
    5
}

#[test]
fn test_file_dependents_includes_import_only_references() {
    let db = setup_db_with_file_deps();
    let importer_id = add_import_only_file(&db);

    // a.py (file 1) is imported by e.py but never called by it.
    let dependents = query::get_file_dependents(&db, 1).expect("query");
    let importer = dependents.iter().find(|d| d.file_id == importer_id);
    assert!(
        importer.is_some(),
        "a file that IMPORTS the target without calling it must be a dependent, got {:?}",
        dependents.iter().map(|d| &d.path).collect::<Vec<_>>()
    );
    let conn = importer.unwrap();
    assert!(
        conn.connections
            .iter()
            .any(|c| c.kind == "import" && c.to_symbol == "CONST_A"),
        "the connection must be kind=import to CONST_A, got {:?}",
        conn.connections
    );
}

#[test]
fn test_file_dependencies_includes_outgoing_imports() {
    let db = setup_db_with_file_deps();
    let importer_id = add_import_only_file(&db);

    // e.py depends on a.py through its import even with zero calls.
    let deps = query::get_file_dependencies(&db, importer_id).expect("query");
    let target = deps.iter().find(|d| d.file_id == 1);
    assert!(
        target.is_some(),
        "an imported file must appear in dependencies, got {:?}",
        deps.iter().map(|d| &d.path).collect::<Vec<_>>()
    );
}

#[test]
fn test_call_connections_are_kind_call_and_not_duplicated() {
    let db = setup_db_with_file_deps();
    // b.py (file 2) calls func_a in a.py (call edge), and we ALSO give b.py a
    // resolved import of func_a — same pair via both tables.
    add_call(&db, 2, 1, 2);
    db.conn()
        .execute(
            "INSERT INTO imports (file_id, imported_name, module_path, line, is_external, resolved_file_id, resolved_symbol_id) \
             VALUES (2, 'func_a', './a', 1, 0, 1, 1)",
            [],
        )
        .unwrap();

    let dependents = query::get_file_dependents(&db, 1).expect("query");
    let b = dependents
        .iter()
        .find(|d| d.file_id == 2)
        .expect("b.py is a dependent");
    let call_rows: Vec<_> = b.connections.iter().filter(|c| c.kind == "call").collect();
    let import_rows: Vec<_> = b
        .connections
        .iter()
        .filter(|c| c.kind == "import")
        .collect();
    assert!(!call_rows.is_empty(), "call edge must remain kind=call");
    assert!(
        import_rows.len() <= 1,
        "no duplicated import connections, got {:?}",
        b.connections
    );
    // No exact duplicates overall
    let mut seen = std::collections::HashSet::new();
    for c in &b.connections {
        assert!(
            seen.insert((c.kind.clone(), c.from_symbol.clone(), c.to_symbol.clone())),
            "duplicate connection row: {:?}",
            c
        );
    }
}

#[test]
fn test_renamed_import_connection_shows_original_name() {
    let db = setup_db_with_file_deps();
    let conn = db.conn();
    conn.execute(
        "INSERT INTO files (id, service_id, path, absolute_path, language, last_modified, last_indexed) \
         VALUES (6, 1, 'src/f.py', '/test/src/f.py', 'python', 0.0, 0.0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO imports (file_id, imported_name, module_path, line, is_external, resolved_file_id, resolved_symbol_id, original_name) \
         VALUES (6, 'aliased_a', './a', 1, 0, 1, 1, 'func_a')",
        [],
    )
    .unwrap();

    let dependents = query::get_file_dependents(&db, 1).expect("query");
    let f = dependents
        .iter()
        .find(|d| d.file_id == 6)
        .expect("f.py is a dependent via renamed import");
    assert!(
        f.connections
            .iter()
            .any(|c| c.kind == "import" && c.to_symbol == "func_a"),
        "renamed import must surface the ORIGINAL exported name, got {:?}",
        f.connections
    );
}

#[test]
fn test_symbol_dependents_include_module_scope_import_references() {
    let db = setup_db_with_file_deps();
    let conn = db.conn();
    // Importing file with a <module> symbol (TS/JS convention) importing func_a.
    conn.execute(
        "INSERT INTO files (id, service_id, path, absolute_path, language, last_modified, last_indexed) \
         VALUES (7, 1, 'src/g.ts', '/test/src/g.ts', 'typescript', 0.0, 0.0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end) \
         VALUES (20, 7, '<module>', '<module>', 'module', 0, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO imports (file_id, imported_name, module_path, line, is_external, resolved_file_id, resolved_symbol_id) \
         VALUES (7, 'func_a', '../a', 1, 0, 1, 1)",
        [],
    )
    .unwrap();

    // Symbol-level: func_a (id 1) — g.ts imports it but never calls it.
    let dependents = query::get_dependents(&db, 1).expect("query");
    assert!(
        dependents.iter().any(|s| s.file_id == 7),
        "an import-only referencing file must surface among symbol dependents, got file_ids {:?}",
        dependents.iter().map(|s| s.file_id).collect::<Vec<_>>()
    );
}
