use ariadne::db::{query, write, Database};

/// Create a test DB with symbols that have varying health signals.
fn setup_quality_db() -> Database {
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

    // File A: 3 symbols with varying health
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
    db.conn()
        .execute(
            "UPDATE symbols SET is_dead = 1 WHERE id = ?1",
            rusqlite::params![sym_dead],
        )
        .expect("mark dead");

    // File B: 2 clean symbols
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

    // History: login is volatile and heavily modified
    write::insert_symbol_history(&db, sym_login, Some(1000), Some(5000), 25, 5, true)
        .expect("history login");
    write::insert_symbol_history(&db, sym_hash, Some(1000), Some(4000), 15, 3, false)
        .expect("history hash_pw");

    // Coupling: file_a coupled to file_b
    write::insert_coupling(&db, file_a, file_b, 8, 0.75).expect("insert coupling");

    // Calls: format_output -> login (cross-file), login -> hash_pw (intra-file)
    db.conn()
        .execute(
            "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution)
             VALUES (?1, ?2, 'login', ?3, 5, 1.0, 'exact')",
            rusqlite::params![sym_format, sym_login, file_b],
        )
        .expect("insert call format->login");

    db.conn()
        .execute(
            "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution)
             VALUES (?1, ?2, 'hash_pw', ?3, 10, 1.0, 'exact')",
            rusqlite::params![sym_login, sym_hash, file_a],
        )
        .expect("insert call login->hash");

    db
}

// ---- Unit tests for query layer ----

#[test]
fn test_symbol_health_data_with_history() {
    let db = setup_quality_db();
    let data = query::get_symbol_health_data(&db, "login")
        .expect("query should succeed")
        .expect("data should exist");

    assert_eq!(data.name, "login");
    assert_eq!(data.kind, "function");
    assert_eq!(data.fan_in, 1); // format_output calls login
    assert_eq!(data.fan_out, 1); // login calls hash_pw
    assert_eq!(data.modification_count, 25);
    assert_eq!(data.author_count, 5);
    assert!(data.is_volatile);
}

#[test]
fn test_symbol_health_data_no_history() {
    let db = setup_quality_db();
    let data = query::get_symbol_health_data(&db, "format_output")
        .expect("query should succeed")
        .expect("data should exist");

    assert_eq!(data.name, "format_output");
    assert_eq!(data.fan_in, 0); // nobody calls format_output
    assert_eq!(data.fan_out, 1); // format_output calls login
    assert_eq!(data.modification_count, 0);
    assert_eq!(data.author_count, 0);
    assert!(!data.is_volatile);
}

#[test]
fn test_symbol_health_data_not_found() {
    let db = setup_quality_db();
    let data = query::get_symbol_health_data(&db, "nonexistent").expect("query should succeed");
    assert!(data.is_none(), "nonexistent symbol should return None");
}

#[test]
fn test_complexity_hotspots() {
    let db = setup_quality_db();
    let hotspots = query::get_complexity_hotspots(&db, 50).expect("query should succeed");

    // login has fan_in=1 + fan_out=1 + volatile=true → should appear
    // Hotspots are sorted by combined score descending
    assert!(!hotspots.is_empty(), "should find at least one hotspot");

    // Login should be the top hotspot (highest combined signals)
    assert_eq!(hotspots[0].name, "login");
}

#[test]
fn test_code_smell_candidates() {
    let db = setup_quality_db();
    let smells = query::get_code_smell_candidates(&db).expect("query should succeed");

    // login has: is_volatile=true, modification_count=25, external fan-in
    // Should produce at least a "high_volatility" smell
    let volatile_smells: Vec<_> = smells
        .iter()
        .filter(|s| s.is_volatile && s.modification_count > 10)
        .collect();
    assert!(!volatile_smells.is_empty(), "should detect volatile symbol");
}

// ---- MCP integration tests ----

#[test]
fn test_get_symbol_health_mcp_tool() {
    use std::borrow::Cow;
    use std::sync::Arc;

    use ariadne::mcp::tools::AriadneService;
    use rmcp::model::*;
    use rmcp::service::{AtomicU32RequestIdProvider, Peer, RequestContext, RoleServer};
    use rmcp::ServerHandler;

    let db = setup_quality_db();
    let service = AriadneService::new(db);

    let provider = Arc::new(AtomicU32RequestIdProvider::default());
    let client_info = ClientInfo::default();
    let (peer, _rx) = Peer::<RoleServer>::new(provider, client_info);
    let ct = tokio_util::sync::CancellationToken::new();
    let id = RequestId::Number(1);
    let ctx = RequestContext { ct, id, peer };

    let mut args = serde_json::Map::new();
    args.insert("symbol".to_string(), serde_json::json!("login"));

    let req = CallToolRequestParam {
        name: Cow::Owned("get_symbol_health".to_string()),
        arguments: Some(args),
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(service.call_tool(req, ctx))
        .expect("should succeed");

    assert!(
        result.is_error != Some(true),
        "get_symbol_health should not error"
    );

    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("should be valid JSON");

    // Verify structure
    assert!(parsed.get("symbol").is_some());
    assert!(parsed.get("health_score").is_some());
    assert!(parsed.get("health_level").is_some());
    assert!(parsed.get("signals").is_some());
    assert!(parsed.get("confidence").is_some());

    let score = parsed["health_score"].as_f64().unwrap();
    assert!(
        score >= 0.0 && score <= 1.0,
        "health_score should be 0.0-1.0: {score}"
    );

    let level = parsed["health_level"].as_str().unwrap();
    assert!(
        ["excellent", "good", "fair", "poor", "critical"].contains(&level),
        "health_level should be valid: {level}"
    );

    let confidence = parsed["confidence"].as_f64().unwrap();
    assert!(confidence > 0.0, "should have non-zero confidence");
}

#[test]
fn test_get_complexity_hotspots_mcp_tool() {
    use std::borrow::Cow;
    use std::sync::Arc;

    use ariadne::mcp::tools::AriadneService;
    use rmcp::model::*;
    use rmcp::service::{AtomicU32RequestIdProvider, Peer, RequestContext, RoleServer};
    use rmcp::ServerHandler;

    let db = setup_quality_db();
    let service = AriadneService::new(db);

    let provider = Arc::new(AtomicU32RequestIdProvider::default());
    let client_info = ClientInfo::default();
    let (peer, _rx) = Peer::<RoleServer>::new(provider, client_info);
    let ct = tokio_util::sync::CancellationToken::new();
    let id = RequestId::Number(1);
    let ctx = RequestContext { ct, id, peer };

    let req = CallToolRequestParam {
        name: Cow::Owned("get_complexity_hotspots".to_string()),
        arguments: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(service.call_tool(req, ctx))
        .expect("should succeed");

    assert!(
        result.is_error != Some(true),
        "get_complexity_hotspots should not error"
    );

    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("should be valid JSON");

    assert!(parsed.get("hotspots").is_some());
    assert!(parsed.get("count").is_some());
    assert!(parsed.get("total_symbols").is_some());
}

#[test]
fn test_get_code_smells_mcp_tool() {
    use std::borrow::Cow;
    use std::sync::Arc;

    use ariadne::mcp::tools::AriadneService;
    use rmcp::model::*;
    use rmcp::service::{AtomicU32RequestIdProvider, Peer, RequestContext, RoleServer};
    use rmcp::ServerHandler;

    let db = setup_quality_db();
    let service = AriadneService::new(db);

    let provider = Arc::new(AtomicU32RequestIdProvider::default());
    let client_info = ClientInfo::default();
    let (peer, _rx) = Peer::<RoleServer>::new(provider, client_info);
    let ct = tokio_util::sync::CancellationToken::new();
    let id = RequestId::Number(1);
    let ctx = RequestContext { ct, id, peer };

    let req = CallToolRequestParam {
        name: Cow::Owned("get_code_smells".to_string()),
        arguments: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(service.call_tool(req, ctx))
        .expect("should succeed");

    assert!(
        result.is_error != Some(true),
        "get_code_smells should not error"
    );

    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("should be valid JSON");

    assert!(parsed.get("smells").is_some());
    assert!(parsed.get("count").is_some());

    let smells = parsed["smells"].as_array().expect("smells should be array");
    for smell in smells {
        assert!(smell.get("symbol").is_some());
        assert!(smell.get("file").is_some());
        assert!(smell.get("smell_type").is_some());
        assert!(smell.get("severity").is_some());
    }
}
