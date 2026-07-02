use ariadne::db::Database;

/// Older .ariadne.db files predate files.parse_error_count. Database::open must
/// bring them forward idempotently — a missing guard would turn the first
/// post-upgrade index into a hard "no such column" failure.
#[test]
fn test_open_migrates_pre_parse_error_count_db() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("old.db");

    // Simulate a pre-column database.
    {
        let db = Database::open(&path).expect("create db");
        db.conn()
            .execute_batch("ALTER TABLE files DROP COLUMN parse_error_count")
            .expect("drop column to simulate old schema");
    }

    // Reopen: the guarded ALTER must restore the column without erroring.
    let db = Database::open(&path).expect("reopen old-schema db");
    let has_column: bool = db
        .conn()
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('files') WHERE name = 'parse_error_count'",
            [],
            |row| row.get(0),
        )
        .expect("pragma query");
    assert!(has_column, "open must add parse_error_count to older DBs");

    // Idempotent: opening again must not fail with duplicate-column.
    drop(db);
    let db2 = Database::open(&path).expect("third open is a no-op migration");
    let count: i64 = db2
        .conn()
        .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
        .expect("query");
    assert_eq!(count, 0);
}
