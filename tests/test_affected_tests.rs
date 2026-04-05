use ariadne::analysis::affected_tests::AffectedTestsResult;
use ariadne::db::{write, Database};
use rusqlite::params;
use std::collections::{HashSet, VecDeque};

/// Create a test DB with symbols and call edges for reverse-BFS tests.
/// Returns (db, symbol_ids) where symbol_ids[i] corresponds to the i-th symbol created.
fn setup_affected_tests_db(
    symbols: &[(&str, bool)], // (name, is_test)
    edges: &[(usize, usize)], // (caller_idx, callee_idx) — caller calls callee
) -> (Database, Vec<i64>) {
    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test-svc", "/tmp/test", "monolith", "python")
        .expect("insert service");
    let file = write::insert_file(
        &db,
        svc,
        "src/module.py",
        "/tmp/test/src/module.py",
        "python",
        0.0,
    )
    .expect("insert file");

    let mut sym_ids = Vec::with_capacity(symbols.len());
    for (i, (name, is_test)) in symbols.iter().enumerate() {
        let qname = format!("module.{}", name);
        let id = write::insert_symbol(
            &db,
            file,
            name,
            &qname,
            "function",
            (i as u32) * 10 + 1,
            (i as u32) * 10 + 9,
            true,
            *is_test,
            &format!("def {}()", name),
            "[]",
            None,
        )
        .expect("insert symbol");
        sym_ids.push(id);
    }

    let conn = db.conn();
    for (caller_idx, callee_idx) in edges {
        conn.execute(
            "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution)
             VALUES (?1, ?2, ?3, ?4, ?5, 1.0, 'resolved')",
            params![
                sym_ids[*caller_idx],
                sym_ids[*callee_idx],
                symbols[*callee_idx].0,
                file,
                (*caller_idx as u32) * 10 + 3,
            ],
        )
        .expect("insert call edge");
    }

    (db, sym_ids)
}

/// Simulate the reverse BFS from `find_affected_tests` — walk callers backward
/// from a set of start symbols and collect all reachable symbols.
fn reverse_bfs(db: &Database, start_ids: &[i64]) -> HashSet<i64> {
    let conn = db.conn();

    // Build reverse adjacency: callee -> [callers]
    let mut callers_of: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    let mut stmt = conn
        .prepare("SELECT caller_symbol_id, callee_symbol_id FROM calls WHERE callee_symbol_id IS NOT NULL")
        .expect("prepare");
    let calls: Vec<(i64, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect");

    for (caller, callee) in &calls {
        callers_of.entry(*callee).or_default().push(*caller);
    }

    let mut visited: HashSet<i64> = start_ids.iter().copied().collect();
    let mut queue: VecDeque<i64> = start_ids.iter().copied().collect();

    while let Some(sym_id) = queue.pop_front() {
        if let Some(callers) = callers_of.get(&sym_id) {
            for &caller_id in callers {
                if visited.insert(caller_id) {
                    queue.push_back(caller_id);
                }
            }
        }
    }

    visited
}

#[test]
fn test_affected_tests_result_serialization() {
    let result = AffectedTestsResult {
        test_files: vec!["tests/test_auth.py".to_string()],
        test_functions: vec!["test_login".to_string(), "test_logout".to_string()],
        changed_files: 2,
        total_tests_affected: 2,
    };

    let json = serde_json::to_value(&result).expect("should serialize");
    assert!(json.get("test_files").is_some());
    assert!(json.get("test_functions").is_some());
    assert_eq!(json["changed_files"], 2);
    assert_eq!(json["total_tests_affected"], 2);
    assert_eq!(
        json["test_functions"].as_array().unwrap().len(),
        2,
        "should have 2 test functions"
    );
}

#[test]
fn test_reverse_bfs_reaches_test_from_callee() {
    // test_handler -> handler: test calls handler
    // Starting from handler, reverse BFS should reach test_handler
    let (db, sym_ids) = setup_affected_tests_db(
        &[("handler", false), ("test_handler", true)],
        &[(1, 0)], // test_handler calls handler
    );

    let reached = reverse_bfs(&db, &[sym_ids[0]]); // start from handler
    assert!(
        reached.contains(&sym_ids[1]),
        "reverse BFS from handler should reach test_handler"
    );

    // Verify test_handler is actually a test
    let conn = db.conn();
    let is_test: bool = conn
        .query_row(
            "SELECT is_test FROM symbols WHERE id = ?1",
            params![sym_ids[1]],
            |row| row.get(0),
        )
        .expect("query is_test");
    assert!(is_test, "test_handler should be marked as test");
}

#[test]
fn test_reverse_bfs_transitive_reachability() {
    // test_a -> middleware -> util: transitive chain
    // Starting from util, reverse BFS should reach test_a through middleware
    let (db, sym_ids) = setup_affected_tests_db(
        &[("util", false), ("middleware", false), ("test_a", true)],
        &[(1, 0), (2, 1)], // middleware->util, test_a->middleware
    );

    let reached = reverse_bfs(&db, &[sym_ids[0]]); // start from util

    assert!(
        reached.contains(&sym_ids[1]),
        "should reach middleware from util"
    );
    assert!(
        reached.contains(&sym_ids[2]),
        "should transitively reach test_a from util through middleware"
    );

    // Filter to tests only
    let conn = db.conn();
    let test_ids: Vec<i64> = reached
        .iter()
        .filter(|id| {
            let is_test: bool = conn
                .query_row(
                    "SELECT is_test FROM symbols WHERE id = ?1",
                    params![id],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            is_test
        })
        .copied()
        .collect();

    assert_eq!(test_ids.len(), 1, "only test_a should be a test symbol");
    assert_eq!(test_ids[0], sym_ids[2]);
}

#[test]
fn test_reverse_bfs_isolated_symbol_no_tests() {
    // orphan has no callers, test_x has no relationship to orphan
    let (db, sym_ids) = setup_affected_tests_db(
        &[("orphan", false), ("test_x", true)],
        &[], // no edges
    );

    let reached = reverse_bfs(&db, &[sym_ids[0]]); // start from orphan

    // Should only contain orphan itself
    assert_eq!(reached.len(), 1, "isolated symbol reaches only itself");
    assert!(reached.contains(&sym_ids[0]));
    assert!(
        !reached.contains(&sym_ids[1]),
        "test_x should not be reachable from orphan"
    );
}
