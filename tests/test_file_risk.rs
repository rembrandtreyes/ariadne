use ariadne::db::{query, write, Database};

/// Create a test DB with two files, symbols, and various risk signals.
fn setup_risk_db() -> (Database, i64, i64, i64) {
    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test-svc", "/tmp/test", "monolith", "rust")
        .expect("insert service");

    let file_a = write::insert_file(
        &db,
        svc,
        "src/auth.rs",
        "/tmp/test/src/auth.rs",
        "rust",
        0.0,
    )
    .expect("insert file_a");

    let file_b = write::insert_file(
        &db,
        svc,
        "src/utils.rs",
        "/tmp/test/src/utils.rs",
        "rust",
        0.0,
    )
    .expect("insert file_b");

    // File A: 3 symbols (login, hash_pw, validate)
    let sym_login = write::insert_symbol(
        &db,
        file_a,
        "login",
        "auth::login",
        "function",
        1,
        20,
        true,
        false,
        "fn login()",
        "",
        None,
    )
    .expect("insert login");

    let sym_hash = write::insert_symbol(
        &db,
        file_a,
        "hash_pw",
        "auth::hash_pw",
        "function",
        25,
        40,
        false,
        false,
        "fn hash_pw()",
        "",
        None,
    )
    .expect("insert hash_pw");

    let sym_dead = write::insert_symbol(
        &db,
        file_a,
        "old_auth",
        "auth::old_auth",
        "function",
        45,
        60,
        false,
        false,
        "fn old_auth()",
        "",
        None,
    )
    .expect("insert old_auth");

    // Mark old_auth as dead
    db.conn()
        .execute(
            "UPDATE symbols SET is_dead = 1 WHERE id = ?1",
            rusqlite::params![sym_dead],
        )
        .expect("mark dead");

    // File B: 2 symbols (format_output, parse_input)
    let sym_format = write::insert_symbol(
        &db,
        file_b,
        "format_output",
        "utils::format_output",
        "function",
        1,
        15,
        true,
        false,
        "fn format_output()",
        "",
        None,
    )
    .expect("insert format_output");

    let _sym_parse = write::insert_symbol(
        &db,
        file_b,
        "parse_input",
        "utils::parse_input",
        "function",
        20,
        35,
        true,
        false,
        "fn parse_input()",
        "",
        None,
    )
    .expect("insert parse_input");

    // Symbol history: file_a symbols have high churn
    write::insert_symbol_history(&db, sym_login, Some(1000), Some(5000), 25, 5, true)
        .expect("history login");
    write::insert_symbol_history(&db, sym_hash, Some(1000), Some(4000), 15, 3, false)
        .expect("history hash_pw");

    // Coupling: file_a coupled to file_b
    write::insert_coupling(&db, file_a, file_b, 8, 0.75).expect("insert coupling");

    // Resolved call from file_b's format_output -> file_a's login (external fan-in for file_a)
    db.conn()
        .execute(
            "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution)
             VALUES (?1, ?2, 'login', ?3, 5, 1.0, 'exact')",
            rusqlite::params![sym_format, sym_login, file_b],
        )
        .expect("insert resolved call");

    (db, file_a, file_b, svc)
}

#[test]
fn test_file_risk_data_with_full_signals() {
    let (db, file_a, _file_b, _svc) = setup_risk_db();

    let data = query::get_file_risk_data(&db, file_a)
        .expect("query should succeed")
        .expect("data should exist");

    assert_eq!(data.path, "src/auth.rs");
    assert_eq!(data.total_symbols, 3);
    assert_eq!(data.dead_symbols, 1);
    assert_eq!(data.total_modifications, 40); // 25 + 15
    assert_eq!(data.max_authors, 5);
    assert_eq!(data.volatile_count, 1);
    assert_eq!(data.symbols_with_history, 2); // login + hash_pw
    assert_eq!(data.coupled_files, 1);
    assert!((data.max_coupling_strength - 0.75).abs() < f64::EPSILON);
    assert_eq!(data.external_fan_in, 1); // format_output calls login
}

#[test]
fn test_file_risk_data_low_risk_file() {
    let (db, _file_a, file_b, _svc) = setup_risk_db();

    let data = query::get_file_risk_data(&db, file_b)
        .expect("query should succeed")
        .expect("data should exist");

    assert_eq!(data.path, "src/utils.rs");
    assert_eq!(data.total_symbols, 2);
    assert_eq!(data.dead_symbols, 0);
    assert_eq!(data.total_modifications, 0); // no history for file_b symbols
    assert_eq!(data.symbols_with_history, 0);
    assert_eq!(data.coupled_files, 1); // coupled to file_a
    assert_eq!(data.external_fan_in, 0); // nobody calls file_b from outside
}

#[test]
fn test_file_risk_data_empty_file() {
    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test-svc", "/tmp/test", "monolith", "rust")
        .expect("insert service");
    let file_id = write::insert_file(
        &db,
        svc,
        "src/empty.rs",
        "/tmp/test/src/empty.rs",
        "rust",
        0.0,
    )
    .expect("insert file");

    let data = query::get_file_risk_data(&db, file_id).expect("query should succeed");
    assert!(data.is_none(), "empty file should return None");
}

#[test]
fn test_file_risk_data_no_git_history() {
    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test-svc", "/tmp/test", "monolith", "rust")
        .expect("insert service");
    let file_id = write::insert_file(&db, svc, "src/new.rs", "/tmp/test/src/new.rs", "rust", 0.0)
        .expect("insert file");

    // Add a symbol but no history
    write::insert_symbol(
        &db,
        file_id,
        "new_fn",
        "new::new_fn",
        "function",
        1,
        10,
        true,
        false,
        "fn new_fn()",
        "",
        None,
    )
    .expect("insert symbol");

    let data = query::get_file_risk_data(&db, file_id)
        .expect("query should succeed")
        .expect("data should exist");

    assert_eq!(data.symbols_with_history, 0);
    assert_eq!(data.total_modifications, 0);
    assert_eq!(data.coupled_files, 0);
    assert_eq!(data.external_fan_in, 0);
}

#[test]
fn test_compute_file_risk_mcp_tool() {
    use std::borrow::Cow;
    use std::sync::Arc;

    use ariadne::mcp::tools::AriadneService;
    use rmcp::model::*;
    use rmcp::service::{AtomicU32RequestIdProvider, Peer, RequestContext, RoleServer};
    use rmcp::ServerHandler;

    let (db, _file_a, _file_b, _svc) = setup_risk_db();
    let service = AriadneService::new(db);

    let provider = Arc::new(AtomicU32RequestIdProvider::default());
    let client_info = ClientInfo::default();
    let (peer, _rx) = Peer::<RoleServer>::new(provider, client_info);
    let ct = tokio_util::sync::CancellationToken::new();
    let id = RequestId::Number(1);
    let ctx = RequestContext { ct, id, peer };

    let mut args = serde_json::Map::new();
    args.insert(
        "changed_files".to_string(),
        serde_json::json!("src/auth.rs,src/utils.rs"),
    );

    let req = CallToolRequestParam {
        name: Cow::Owned("compute_file_risk".to_string()),
        arguments: Some(args),
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(service.call_tool(req, ctx))
        .expect("should succeed");

    assert!(
        result.is_error != Some(true),
        "compute_file_risk should not error"
    );

    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("should be valid JSON");

    // Verify structure
    assert!(parsed.get("files").is_some(), "should have 'files' key");
    assert!(
        parsed.get("interpretation").is_some(),
        "should have 'interpretation' key"
    );

    let files = parsed["files"].as_array().expect("files should be array");
    assert_eq!(files.len(), 2, "should have 2 files");

    // Files should be sorted by risk_score descending
    let first_score = files[0]["risk_score"].as_f64().unwrap();
    let second_score = files[1]["risk_score"].as_f64().unwrap();
    assert!(
        first_score >= second_score,
        "files should be sorted by risk descending: {first_score} >= {second_score}"
    );

    // auth.rs should have higher risk (high churn, coupling, dead code, fan-in)
    assert_eq!(files[0]["file"].as_str().unwrap(), "src/auth.rs");

    // Verify risk_level is present and valid
    let risk_level = files[0]["risk_level"].as_str().unwrap();
    assert!(
        ["low", "medium", "high", "critical"].contains(&risk_level),
        "risk_level should be valid: {risk_level}"
    );

    // Verify confidence is present
    let confidence = files[0]["confidence"].as_f64().unwrap();
    assert!(confidence > 0.0, "should have non-zero confidence");
    assert!(confidence <= 1.0, "confidence should be <= 1.0");

    // Verify available_signals
    let signals = files[0]["available_signals"]
        .as_array()
        .expect("should have signals");
    assert!(!signals.is_empty(), "should have at least one signal");

    // Verify factors breakdown
    assert!(
        files[0].get("factors").is_some(),
        "should have 'factors' key"
    );
    assert!(
        files[0]["factors"].get("churn").is_some(),
        "should have churn factor"
    );
    assert!(
        files[0]["factors"].get("coupling").is_some(),
        "should have coupling factor"
    );
    assert!(
        files[0]["factors"].get("fan_in").is_some(),
        "should have fan_in factor"
    );
    assert!(
        files[0]["factors"].get("dead_code").is_some(),
        "should have dead_code factor"
    );
}

#[test]
fn test_compute_file_risk_missing_param() {
    use std::borrow::Cow;
    use std::sync::Arc;

    use ariadne::mcp::tools::AriadneService;
    use rmcp::model::*;
    use rmcp::service::{AtomicU32RequestIdProvider, Peer, RequestContext, RoleServer};
    use rmcp::ServerHandler;

    let db = Database::open_in_memory().expect("in-memory db");
    let service = AriadneService::new(db);

    let provider = Arc::new(AtomicU32RequestIdProvider::default());
    let client_info = ClientInfo::default();
    let (peer, _rx) = Peer::<RoleServer>::new(provider, client_info);
    let ct = tokio_util::sync::CancellationToken::new();
    let id = RequestId::Number(1);
    let ctx = RequestContext { ct, id, peer };

    let req = CallToolRequestParam {
        name: Cow::Owned("compute_file_risk".to_string()),
        arguments: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(service.call_tool(req, ctx))
        .expect("should succeed");

    assert_eq!(
        result.is_error,
        Some(true),
        "should error when changed_files is missing"
    );
}

#[test]
fn test_compute_file_risk_unknown_file() {
    use std::borrow::Cow;
    use std::sync::Arc;

    use ariadne::mcp::tools::AriadneService;
    use rmcp::model::*;
    use rmcp::service::{AtomicU32RequestIdProvider, Peer, RequestContext, RoleServer};
    use rmcp::ServerHandler;

    let db = Database::open_in_memory().expect("in-memory db");
    let service = AriadneService::new(db);

    let provider = Arc::new(AtomicU32RequestIdProvider::default());
    let client_info = ClientInfo::default();
    let (peer, _rx) = Peer::<RoleServer>::new(provider, client_info);
    let ct = tokio_util::sync::CancellationToken::new();
    let id = RequestId::Number(1);
    let ctx = RequestContext { ct, id, peer };

    let mut args = serde_json::Map::new();
    args.insert(
        "changed_files".to_string(),
        serde_json::json!("src/nonexistent.rs"),
    );

    let req = CallToolRequestParam {
        name: Cow::Owned("compute_file_risk".to_string()),
        arguments: Some(args),
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(service.call_tool(req, ctx))
        .expect("should succeed");

    // Unknown file should still succeed but with zero confidence
    assert!(
        result.is_error != Some(true),
        "unknown file should not error, should return low confidence"
    );

    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("should be valid JSON");
    let files = parsed["files"].as_array().expect("files should be array");
    assert_eq!(files[0]["confidence"].as_f64().unwrap(), 0.0);
}
