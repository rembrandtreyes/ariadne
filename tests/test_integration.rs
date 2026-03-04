use ariadne_graph::db::Database;

#[test]
fn test_database_creation() {
    let db = Database::open_in_memory().expect("should create in-memory database");
    let conn = db.conn();

    // Verify tables exist
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(tables.contains(&"symbols".to_string()));
    assert!(tables.contains(&"files".to_string()));
    assert!(tables.contains(&"calls".to_string()));
}
