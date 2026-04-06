use ariadne::db::{query, write, Database};
use ariadne::pipeline::community::detect_communities;
use rusqlite::params;

/// Create a test DB and insert symbols with resolved call edges.
/// Returns (db, symbol_ids) where symbol_ids maps indices to DB IDs.
fn setup_community_db(symbol_count: usize, edges: &[(usize, usize)]) -> (Database, Vec<i64>) {
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

    let mut sym_ids = Vec::with_capacity(symbol_count);
    for i in 0..symbol_count {
        let name = format!("func_{}", i);
        let qname = format!("module.func_{}", i);
        let id = write::insert_symbol(
            &db,
            file,
            &name,
            &qname,
            "function",
            (i as u32) * 10 + 1,
            (i as u32) * 10 + 9,
            true,
            false,
            &format!("def {}()", name),
            "[]",
            None,
        )
        .expect("insert symbol");
        sym_ids.push(id);
    }

    // Insert resolved call edges (with callee_symbol_id set)
    let conn = db.conn();
    for (caller_idx, callee_idx) in edges {
        conn.execute(
            "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution)
             VALUES (?1, ?2, ?3, ?4, ?5, 1.0, 'resolved')",
            params![
                sym_ids[*caller_idx],
                sym_ids[*callee_idx],
                format!("func_{}", callee_idx),
                file,
                (*caller_idx as u32) * 10 + 3,
            ],
        )
        .expect("insert call edge");
    }

    (db, sym_ids)
}

#[test]
fn test_detect_single_community() {
    // A -> B, B -> C: all connected
    let (db, _sym_ids) = setup_community_db(3, &[(0, 1), (1, 2)]);

    detect_communities(&db).expect("detect communities");

    let communities = query::get_communities(&db).expect("get communities");
    assert_eq!(communities.len(), 1, "3 connected symbols = 1 community");
    assert_eq!(communities[0].symbol_count, 3);
    assert!(
        communities[0].internal_edges > 0,
        "should have internal edges"
    );
}

#[test]
fn test_detect_two_disconnected_communities() {
    // (A -> B) and (C -> D): two disconnected pairs
    let (db, _sym_ids) = setup_community_db(4, &[(0, 1), (2, 3)]);

    detect_communities(&db).expect("detect communities");

    let communities = query::get_communities(&db).expect("get communities");
    assert_eq!(
        communities.len(),
        2,
        "two disconnected pairs = 2 communities"
    );
    // Both communities should have 2 symbols each
    assert_eq!(communities[0].symbol_count, 2);
    assert_eq!(communities[1].symbol_count, 2);
}

#[test]
fn test_detect_communities_updates_symbol_community_id() {
    // A -> B, B -> C: all connected, single community
    let (db, sym_ids) = setup_community_db(3, &[(0, 1), (1, 2)]);

    detect_communities(&db).expect("detect communities");

    // All 3 symbols should share the same community_id
    let conn = db.conn();
    let mut community_ids: Vec<Option<i64>> = Vec::new();
    for sym_id in &sym_ids {
        let cid: Option<i64> = conn
            .query_row(
                "SELECT community_id FROM symbols WHERE id = ?1",
                params![sym_id],
                |row| row.get(0),
            )
            .expect("query community_id");
        community_ids.push(cid);
    }

    assert!(
        community_ids[0].is_some(),
        "community_id should be set for symbol 0"
    );
    assert_eq!(
        community_ids[0], community_ids[1],
        "symbols 0 and 1 should share community"
    );
    assert_eq!(
        community_ids[1], community_ids[2],
        "symbols 1 and 2 should share community"
    );
}

#[test]
fn test_detect_communities_idempotent() {
    let (db, _sym_ids) = setup_community_db(3, &[(0, 1), (1, 2)]);

    detect_communities(&db).expect("first run");
    detect_communities(&db).expect("second run");

    let communities = query::get_communities(&db).expect("get communities");
    assert_eq!(
        communities.len(),
        1,
        "running twice should not double communities"
    );
    assert_eq!(communities[0].symbol_count, 3);
}

#[test]
fn test_detect_communities_empty_graph() {
    let db = Database::open_in_memory().expect("in-memory db");

    detect_communities(&db).expect("detect on empty graph");

    let communities = query::get_communities(&db).expect("get communities");
    assert_eq!(communities.len(), 0, "empty graph = 0 communities");
}
