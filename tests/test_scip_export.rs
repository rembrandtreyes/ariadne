use ariadne::analysis::scip_export::{export_scip, ScipIndex};
use ariadne::db::{write, Database};

/// Create a multi-file, multi-language test DB for SCIP export tests.
fn setup_multi_file_db() -> Database {
    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test-svc", "/tmp/test", "monolith", "python")
        .expect("insert service");

    // File 1: Python
    let f1 = write::insert_file(
        &db,
        svc,
        "src/auth.py",
        "/tmp/test/src/auth.py",
        "python",
        0.0,
    )
    .expect("insert file 1");

    // File 2: JavaScript
    let f2 = write::insert_file(
        &db,
        svc,
        "src/api.js",
        "/tmp/test/src/api.js",
        "javascript",
        0.0,
    )
    .expect("insert file 2");

    // Symbols in file 1
    let _s1 = write::insert_symbol(
        &db, f1, "login", "auth.login", "function", 1, 10, true, false, "def login()", "[]", None,
    )
    .expect("insert symbol 1");

    let s2 = write::insert_symbol(
        &db,
        f1,
        "hash_password",
        "auth.hash_password",
        "function",
        12,
        20,
        true,
        false,
        "def hash_password(pw)",
        "[]",
        None,
    )
    .expect("insert symbol 2");

    // Symbols in file 2
    let _s3 = write::insert_symbol(
        &db,
        f2,
        "fetchUser",
        "api.fetchUser",
        "function",
        1,
        15,
        true,
        false,
        "function fetchUser()",
        "[]",
        None,
    )
    .expect("insert symbol 3");

    // Call from file 2 referencing symbol in file 1 (cross-file reference)
    let conn = db.conn();
    conn.execute(
        "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution)
         VALUES (?1, ?2, 'auth.hash_password', ?3, 5, 1.0, 'resolved')",
        rusqlite::params![_s3, s2, f2],
    )
    .expect("insert cross-file call");

    // Intra-file call in file 1 (login calls hash_password)
    conn.execute(
        "INSERT INTO calls (caller_symbol_id, callee_name, file_id, line, confidence, resolution)
         VALUES (?1, 'auth.hash_password', ?2, 3, 0.5, 'unresolved')",
        rusqlite::params![_s1, f1],
    )
    .expect("insert intra-file call");

    db
}

#[test]
fn test_scip_export_multi_file() {
    let db = setup_multi_file_db();
    let output = std::env::temp_dir().join("test_scip_multi_file.json");
    let root = std::path::PathBuf::from("/tmp/test");

    export_scip(&db, &output, &root).expect("export should succeed");

    let content = std::fs::read_to_string(&output).expect("read output");
    let index: ScipIndex = serde_json::from_str(&content).expect("parse JSON");

    assert_eq!(
        index.documents.len(),
        2,
        "should produce 2 documents for 2 files"
    );

    // Verify languages are correct
    let py_doc = index
        .documents
        .iter()
        .find(|d| d.relative_path == "src/auth.py")
        .expect("should have python doc");
    assert_eq!(py_doc.language, "python");

    let js_doc = index
        .documents
        .iter()
        .find(|d| d.relative_path == "src/api.js")
        .expect("should have javascript doc");
    assert_eq!(js_doc.language, "javascript");

    // Python file: 2 symbol definitions + 1 call reference
    assert_eq!(py_doc.symbols.len(), 2, "python file has 2 symbol definitions");
    assert_eq!(
        py_doc.occurrences.len(),
        3,
        "python file has 2 defs + 1 ref"
    );

    // JavaScript file: 1 symbol definition + 1 call reference
    assert_eq!(js_doc.symbols.len(), 1, "js file has 1 symbol definition");
    assert_eq!(js_doc.occurrences.len(), 2, "js file has 1 def + 1 ref");

    let _ = std::fs::remove_file(&output);
}

#[test]
fn test_scip_export_cross_file_references() {
    let db = setup_multi_file_db();
    let output = std::env::temp_dir().join("test_scip_cross_ref.json");
    let root = std::path::PathBuf::from("/tmp/test");

    export_scip(&db, &output, &root).expect("export");

    let content = std::fs::read_to_string(&output).expect("read");
    let index: ScipIndex = serde_json::from_str(&content).expect("parse");

    let js_doc = index
        .documents
        .iter()
        .find(|d| d.relative_path == "src/api.js")
        .expect("js doc");

    // The JS file should have a reference to auth.hash_password (symbol_roles = 0)
    let refs: Vec<_> = js_doc
        .occurrences
        .iter()
        .filter(|o| o.symbol_roles == 0)
        .collect();
    assert_eq!(refs.len(), 1, "js file should have 1 cross-file reference");
    assert_eq!(
        refs[0].symbol, "auth.hash_password",
        "reference should point to auth.hash_password"
    );

    let _ = std::fs::remove_file(&output);
}

#[test]
fn test_scip_export_metadata_project_root() {
    let db = setup_multi_file_db();
    let output = std::env::temp_dir().join("test_scip_metadata.json");
    let root = std::path::PathBuf::from("/my/project");

    export_scip(&db, &output, &root).expect("export");

    let content = std::fs::read_to_string(&output).expect("read");
    let index: ScipIndex = serde_json::from_str(&content).expect("parse");

    assert_eq!(index.metadata.version, 1);
    assert_eq!(index.metadata.tool_info.name, "ariadne");
    assert_eq!(
        index.metadata.project_root, "file:///my/project",
        "project_root should use file:// URI prefix"
    );

    let _ = std::fs::remove_file(&output);
}
