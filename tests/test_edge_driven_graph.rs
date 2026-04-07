use ariadne::db::query;
use ariadne::db::Database;
use ariadne::graph::blast_radius::analyze_blast_radius;
use ariadne::graph::circular::detect_circular_dependencies;
use rusqlite::params;

/// Helper: insert a service and file, return file_id.
fn setup_file(db: &Database) -> i64 {
    db.conn()
        .execute(
            "INSERT OR IGNORE INTO services (id, name, repo_path) VALUES (1, 'test', '/tmp')",
            [],
        )
        .unwrap();
    db.conn()
        .execute(
            "INSERT INTO files (service_id, path, absolute_path, language, last_modified, last_indexed)
             VALUES (1, 'src/test.rs', '/tmp/test.rs', 'rust', 0.0, 0.0)",
            [],
        )
        .unwrap();
    db.conn().last_insert_rowid()
}

/// Helper: insert a symbol and return its ID.
fn insert_symbol(db: &Database, name: &str, file_id: i64) -> i64 {
    db.conn()
        .execute(
            "INSERT INTO symbols (file_id, name, qualified_name, kind, line_start, line_end, is_dead, is_test)
             VALUES (?1, ?2, ?3, 'function', 1, 10, 0, 0)",
            params![file_id, name, format!("mod::{name}")],
        )
        .unwrap();
    db.conn().last_insert_rowid()
}

/// Helper: insert a resolved call edge.
fn insert_call(db: &Database, caller_id: i64, callee_id: i64, callee_name: &str, file_id: i64) {
    db.conn()
        .execute(
            "INSERT INTO calls (file_id, caller_symbol_id, callee_symbol_id, callee_name, line, confidence, resolution)
             VALUES (?1, ?2, ?3, ?4, 1, 1.0, 'resolved')",
            params![file_id, caller_id, callee_id, callee_name],
        )
        .unwrap();
}

/// Edge-driven graph loading: only symbols with edges appear in the graph.
/// Fixture: A→B→C→A (cycle), D→E (chain), F (isolated, zero edges).
#[test]
fn test_edge_driven_graph_cycle_detection() {
    let db = Database::open_in_memory().unwrap();
    let file_id = setup_file(&db);

    let a = insert_symbol(&db, "A", file_id);
    let b = insert_symbol(&db, "B", file_id);
    let c = insert_symbol(&db, "C", file_id);
    let d = insert_symbol(&db, "D", file_id);
    let e = insert_symbol(&db, "E", file_id);
    let f = insert_symbol(&db, "F", file_id);

    // Edges: A→B, B→C, C→A (cycle), D→E (chain)
    insert_call(&db, a, b, "B", file_id);
    insert_call(&db, b, c, "C", file_id);
    insert_call(&db, c, a, "A", file_id);
    insert_call(&db, d, e, "E", file_id);

    let graph = query::build_call_graph(&db, None).unwrap();

    // Only 5 symbols should be in the graph (A-E have edges, F does not)
    assert_eq!(
        graph.node_count(),
        5,
        "only edge-referenced symbols should be loaded"
    );

    // F should not be in the graph
    assert!(
        graph.find_node(f as u64).is_none(),
        "isolated symbol F should not be in edge-driven graph"
    );

    // Cycle detection should find exactly one cycle: {A, B, C}
    let cycles = detect_circular_dependencies(&graph);
    assert_eq!(cycles.len(), 1, "should find exactly one cycle");
    assert_eq!(cycles[0].cycle_length, 3, "cycle should have 3 symbols");

    let mut cycle_names: Vec<String> = cycles[0].symbols.clone();
    cycle_names.sort();
    assert_eq!(cycle_names, vec!["mod::A", "mod::B", "mod::C"]);
}

#[test]
fn test_edge_driven_graph_blast_radius() {
    let db = Database::open_in_memory().unwrap();
    let file_id = setup_file(&db);

    let a = insert_symbol(&db, "A", file_id);
    let b = insert_symbol(&db, "B", file_id);
    let c = insert_symbol(&db, "C", file_id);

    insert_call(&db, a, b, "B", file_id);
    insert_call(&db, b, c, "C", file_id);

    let graph = query::build_call_graph(&db, None).unwrap();

    // blast_radius traverses reverse edges (callers), so changing C affects B and A
    let blast = analyze_blast_radius(&graph, c as u64, Some(10), false);
    assert_eq!(
        blast.total_affected, 2,
        "changing C should affect B (direct) and A (transitive)"
    );
}

#[test]
fn test_edge_driven_graph_empty() {
    let db = Database::open_in_memory().unwrap();
    let file_id = setup_file(&db);

    // Only isolated symbols, no edges
    let _a = insert_symbol(&db, "A", file_id);
    let _b = insert_symbol(&db, "B", file_id);

    let graph = query::build_call_graph(&db, None).unwrap();

    assert_eq!(graph.node_count(), 0, "no edges means no symbols in graph");
    assert_eq!(graph.edge_count(), 0);
}

#[test]
fn test_edge_driven_graph_limit() {
    let db = Database::open_in_memory().unwrap();
    let file_id = setup_file(&db);

    let a = insert_symbol(&db, "A", file_id);
    let b = insert_symbol(&db, "B", file_id);
    let c = insert_symbol(&db, "C", file_id);

    insert_call(&db, a, b, "B", file_id);
    insert_call(&db, b, c, "C", file_id);

    // Limit to 1 edge — only 2 symbols should load
    let graph = query::build_call_graph(&db, Some(1)).unwrap();
    assert_eq!(graph.node_count(), 2, "limit=1 edge means 2 symbols");
    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn test_batch_resolve_paths_to_symbol_ids() {
    let db = Database::open_in_memory().unwrap();
    db.conn()
        .execute(
            "INSERT OR IGNORE INTO services (id, name, repo_path) VALUES (1, 'test', '/tmp')",
            [],
        )
        .unwrap();
    db.conn()
        .execute(
            "INSERT INTO files (service_id, path, absolute_path, language, last_modified, last_indexed)
             VALUES (1, 'src/foo.rs', '/tmp/foo.rs', 'rust', 0.0, 0.0)",
            [],
        )
        .unwrap();
    let f1 = db.conn().last_insert_rowid();
    db.conn()
        .execute(
            "INSERT INTO files (service_id, path, absolute_path, language, last_modified, last_indexed)
             VALUES (1, 'src/bar.rs', '/tmp/bar.rs', 'rust', 0.0, 0.0)",
            [],
        )
        .unwrap();
    let f2 = db.conn().last_insert_rowid();

    let s1 = insert_symbol(&db, "alpha", f1);
    let s2 = insert_symbol(&db, "beta", f1);
    let s3 = insert_symbol(&db, "gamma", f2);

    let paths = vec!["src/foo.rs".to_string(), "src/bar.rs".to_string()];
    let ids = query::resolve_paths_to_symbol_ids(&db, &paths).unwrap();

    assert_eq!(ids.len(), 3, "should find 3 symbols across 2 files");
    assert!(ids.contains(&s1));
    assert!(ids.contains(&s2));
    assert!(ids.contains(&s3));
}

#[test]
fn test_batch_resolve_empty_paths() {
    let db = Database::open_in_memory().unwrap();
    let ids = query::resolve_paths_to_symbol_ids(&db, &[]).unwrap();
    assert!(ids.is_empty());
}

#[test]
fn test_batch_resolve_nonexistent_path() {
    let db = Database::open_in_memory().unwrap();
    let paths = vec!["nonexistent.rs".to_string()];
    let ids = query::resolve_paths_to_symbol_ids(&db, &paths).unwrap();
    assert!(ids.is_empty(), "nonexistent file should return empty");
}
