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
