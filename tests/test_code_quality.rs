use ariadne::db::{query, write, Database};

/// Minimal DB with one service and file — no symbols.
fn setup_minimal_db() -> (Database, i64, i64) {
    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test-svc", "/tmp/test", "monolith", "rust")
        .expect("insert service");
    let file = write::insert_file(
        &db,
        svc,
        "src/main.rs",
        "/tmp/test/src/main.rs",
        "rust",
        0.0,
    )
    .expect("insert file");
    (db, svc, file)
}

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

// ---- get_complexity_hotspots edge cases ----

#[test]
fn test_complexity_hotspots_empty_db() {
    let (db, _, _) = setup_minimal_db();
    let hotspots = query::get_complexity_hotspots(&db, 10).expect("query should succeed");
    assert!(hotspots.is_empty(), "Empty DB should return no hotspots");
}

#[test]
fn test_complexity_hotspots_limit_zero() {
    let db = setup_quality_db();
    let hotspots = query::get_complexity_hotspots(&db, 0).expect("query should succeed");
    assert!(hotspots.is_empty(), "limit=0 should return empty result");
}

#[test]
fn test_complexity_hotspots_limit_exceeds_symbol_count() {
    let db = setup_quality_db();
    // setup_quality_db has 4 eligible (non-dead, non-test) symbols; limit 1000 should return all
    let hotspots = query::get_complexity_hotspots(&db, 1000).expect("query should succeed");
    assert!(
        !hotspots.is_empty(),
        "Should return symbols when limit > count"
    );
    assert!(
        hotspots.len() <= 4,
        "Cannot return more symbols than exist: got {}",
        hotspots.len()
    );
}

#[test]
fn test_complexity_hotspots_excludes_dead_symbols() {
    let (db, _, file) = setup_minimal_db();
    let sym = write::insert_symbol(
        &db,
        file,
        "dead_fn",
        "main::dead_fn",
        "function",
        1,
        10,
        false,
        false,
        "fn dead_fn()",
        "",
        None,
    )
    .expect("insert symbol");
    db.conn()
        .execute(
            "UPDATE symbols SET is_dead = 1 WHERE id = ?1",
            rusqlite::params![sym],
        )
        .expect("mark dead");
    let hotspots = query::get_complexity_hotspots(&db, 10).expect("query should succeed");
    assert!(
        hotspots.is_empty(),
        "Dead symbols must be excluded from hotspots"
    );
}

#[test]
fn test_complexity_hotspots_no_symbol_history_uses_zero_defaults() {
    let (db, _, file) = setup_minimal_db();
    // Symbol with no symbol_history row — modification_count and is_volatile default to 0
    write::insert_symbol(
        &db,
        file,
        "no_history",
        "main::no_history",
        "function",
        1,
        5,
        true,
        false,
        "fn no_history()",
        "",
        None,
    )
    .expect("insert symbol");
    let hotspots = query::get_complexity_hotspots(&db, 10).expect("query should succeed");
    assert_eq!(hotspots.len(), 1);
    assert_eq!(hotspots[0].name, "no_history");
    assert_eq!(
        hotspots[0].modification_count, 0,
        "No history → modification_count = 0"
    );
    assert!(!hotspots[0].is_volatile, "No history → is_volatile = false");
}

#[test]
fn test_complexity_hotspots_high_churn_surfaces_symbol() {
    // Score = fan_in + fan_out + modification_count * 0.1 + (volatile ? 10 : 0)
    // high_churn: 0 + 0 + 100 * 0.1 = 10.0
    // low_churn:  fan_in=1 + fan_out=0 + 0 = 1.0
    // high_churn must rank first despite zero fan counts
    let (db, _, file) = setup_minimal_db();
    let sym_high = write::insert_symbol(
        &db,
        file,
        "high_churn",
        "main::high_churn",
        "function",
        1,
        10,
        true,
        false,
        "fn high_churn()",
        "",
        None,
    )
    .expect("insert high_churn");
    let sym_low = write::insert_symbol(
        &db,
        file,
        "low_churn",
        "main::low_churn",
        "function",
        11,
        20,
        true,
        false,
        "fn low_churn()",
        "",
        None,
    )
    .expect("insert low_churn");
    write::insert_symbol_history(&db, sym_high, None, None, 100, 1, false)
        .expect("history high_churn");
    write::insert_symbol_history(&db, sym_low, None, None, 0, 1, false).expect("history low_churn");
    // Add one caller to low_churn so it has fan_in=1
    db.conn()
        .execute(
            "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution)
             VALUES (?1, ?2, 'low_churn', ?3, 1, 1.0, 'exact')",
            rusqlite::params![sym_high, sym_low, file],
        )
        .expect("insert call");
    let hotspots = query::get_complexity_hotspots(&db, 10).expect("query should succeed");
    assert_eq!(hotspots.len(), 2);
    assert_eq!(
        hotspots[0].name, "high_churn",
        "high_churn (score=10.0) must outrank low_churn (score=1.0)"
    );
}

#[test]
fn test_complexity_hotspots_volatile_bonus_outranks_higher_fan_counts() {
    // score = fan_in + fan_out + modification_count*0.1 + (volatile ? 10 : 0)
    // symbol_a: fan_in=3 + fan_out=3 + 0 = 6.0 (not volatile)
    // symbol_b: fan_in=1 + fan_out=1 + 10 = 12.0 (volatile)
    // symbol_b must rank first despite fewer connections
    let (db, _, file) = setup_minimal_db();
    let sym_a = write::insert_symbol(
        &db,
        file,
        "high_fan",
        "main::high_fan",
        "function",
        1,
        10,
        true,
        false,
        "fn high_fan()",
        "",
        None,
    )
    .expect("insert high_fan");
    let sym_b = write::insert_symbol(
        &db,
        file,
        "volatile_sym",
        "main::volatile_sym",
        "function",
        11,
        20,
        true,
        false,
        "fn volatile_sym()",
        "",
        None,
    )
    .expect("insert volatile_sym");
    write::insert_symbol_history(&db, sym_a, None, None, 0, 1, false).expect("history a");
    write::insert_symbol_history(&db, sym_b, None, None, 0, 1, true).expect("history b (volatile)");
    // Give high_fan 3 callers and 3 callees
    for i in 0..3i64 {
        let caller = write::insert_symbol(
            &db,
            file,
            &format!("caller_{i}"),
            &format!("main::caller_{i}"),
            "function",
            (30 + i * 5) as u32,
            (34 + i * 5) as u32,
            false,
            false,
            "",
            "",
            None,
        )
        .expect("insert caller");
        let callee = write::insert_symbol(
            &db,
            file,
            &format!("callee_{i}"),
            &format!("main::callee_{i}"),
            "function",
            (60 + i * 5) as u32,
            (64 + i * 5) as u32,
            false,
            false,
            "",
            "",
            None,
        )
        .expect("insert callee");
        db.conn()
            .execute(
                "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution)
                 VALUES (?1, ?2, 'high_fan', ?3, ?4, 1.0, 'exact')",
                rusqlite::params![caller, sym_a, file, i],
            )
            .expect("insert fan-in");
        db.conn()
            .execute(
                "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution)
                 VALUES (?1, ?2, 'callee', ?3, ?4, 1.0, 'exact')",
                rusqlite::params![sym_a, callee, file, i + 10],
            )
            .expect("insert fan-out");
    }
    // Give volatile_sym 1 caller and 1 callee
    let caller_b = write::insert_symbol(
        &db,
        file,
        "caller_b",
        "main::caller_b",
        "function",
        90,
        94,
        false,
        false,
        "",
        "",
        None,
    )
    .expect("insert caller_b");
    let callee_b = write::insert_symbol(
        &db,
        file,
        "callee_b",
        "main::callee_b",
        "function",
        95,
        99,
        false,
        false,
        "",
        "",
        None,
    )
    .expect("insert callee_b");
    db.conn()
        .execute(
            "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution)
             VALUES (?1, ?2, 'volatile_sym', ?3, 1, 1.0, 'exact')",
            rusqlite::params![caller_b, sym_b, file],
        )
        .expect("insert volatile fan-in");
    db.conn()
        .execute(
            "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution)
             VALUES (?1, ?2, 'callee_b', ?3, 2, 1.0, 'exact')",
            rusqlite::params![sym_b, callee_b, file],
        )
        .expect("insert volatile fan-out");
    let hotspots = query::get_complexity_hotspots(&db, 2).expect("query should succeed");
    assert_eq!(hotspots.len(), 2);
    assert_eq!(
        hotspots[0].name, "volatile_sym",
        "volatile_sym (score=12.0) must outrank high_fan (score=6.0)"
    );
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
        (0.0..=1.0).contains(&score),
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

// ---- get_god_objects tests ----

#[test]
fn test_god_objects_empty_db() {
    let (db, _, _) = setup_minimal_db();
    let gods = query::get_god_objects(&db, 1, 10).expect("query should succeed");
    assert!(gods.is_empty(), "Empty DB should return no god objects");
}

#[test]
fn test_god_objects_threshold_filters() {
    let db = setup_quality_db();
    // fan_in: login=1, hash_pw=1, format_output=0, parse_input=0, old_auth=dead(excluded)

    let at_one = query::get_god_objects(&db, 1, 10).expect("query should succeed");
    assert_eq!(
        at_one.len(),
        2,
        "threshold=1 should return login + hash_pw (both fan_in=1)"
    );

    let at_two = query::get_god_objects(&db, 2, 10).expect("query should succeed");
    assert!(
        at_two.is_empty(),
        "threshold=2 should return no symbols (no fan_in >= 2 in fixture)"
    );
}

#[test]
fn test_god_objects_excludes_dead_and_test() {
    let db = setup_quality_db();
    let gods = query::get_god_objects(&db, 0, 10).expect("query should succeed");
    // threshold=0 includes fan_in=0 symbols; excludes is_dead=1 (old_auth) and is_test=1
    assert_eq!(
        gods.len(),
        4,
        "Should exclude old_auth (dead); got {:?}",
        gods.iter().map(|g| &g.name).collect::<Vec<_>>()
    );
    for g in &gods {
        assert_ne!(g.name, "old_auth", "dead symbol must not appear");
        assert!(!g.is_dead, "is_dead must be false in results");
    }
}

#[test]
fn test_get_god_objects_mcp_tool() {
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
    args.insert("threshold".to_string(), serde_json::json!(1));
    args.insert("limit".to_string(), serde_json::json!(10));
    let req = CallToolRequestParam {
        name: Cow::Owned("get_god_objects".to_string()),
        arguments: Some(args),
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(service.call_tool(req, ctx))
        .expect("should succeed");

    assert!(
        result.is_error != Some(true),
        "get_god_objects should not error"
    );

    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("should be valid JSON");

    assert!(parsed.get("god_objects").is_some());
    assert!(parsed.get("count").is_some());
    assert!(parsed.get("threshold").is_some());

    let objects = parsed["god_objects"].as_array().expect("array");
    assert_eq!(objects.len(), 2, "threshold=1 should yield 2 symbols");
    for obj in objects {
        assert!(obj.get("symbol").is_some());
        assert!(obj.get("fan_in").is_some());
        assert!(obj.get("file").is_some());
    }
}

// ---- get_entry_points tests ----

/// Seed a DB with three categories of entry points: 1 framework, 1 HTTP handler, 1 `main`.
/// Returns (db, service_id, file_id, sym_fw_id, sym_http_id, sym_main_id).
fn setup_entry_points_db() -> (Database, i64, i64, i64, i64, i64) {
    let (db, svc, file) = setup_minimal_db();

    let sym_fw = write::insert_symbol(
        &db,
        file,
        "handleClick",
        "ui::handleClick",
        "function",
        10,
        15,
        true,
        false,
        "fn handleClick()",
        "",
        None,
    )
    .expect("insert framework symbol");
    db.conn()
        .execute(
            "UPDATE symbols SET is_entry_point = 1 WHERE id = ?1",
            [sym_fw],
        )
        .expect("mark framework");

    let sym_http = write::insert_symbol(
        &db,
        file,
        "create_user",
        "routes::create_user",
        "function",
        30,
        40,
        true,
        false,
        "fn create_user()",
        "",
        None,
    )
    .expect("insert http handler");
    write::insert_api_endpoint(
        &db,
        svc,
        "POST",
        "/users",
        Some(sym_http),
        Some(file),
        Some(30),
    )
    .expect("insert api endpoint");

    let sym_main = write::insert_symbol(
        &db,
        file,
        "main",
        "main",
        "function",
        100,
        120,
        true,
        false,
        "fn main()",
        "",
        None,
    )
    .expect("insert main");

    (db, svc, file, sym_fw, sym_http, sym_main)
}

#[test]
fn test_entry_points_empty_db() {
    let (db, _, _) = setup_minimal_db();
    let points = query::get_entry_points(&db, None, 100).expect("query should succeed");
    assert!(points.is_empty(), "Empty DB should return no entry points");
}

#[test]
fn test_entry_points_returns_all_three_categories() {
    let (db, _, _, _, _, _) = setup_entry_points_db();
    let points = query::get_entry_points(&db, None, 100).expect("query should succeed");

    let categories: std::collections::HashSet<&str> =
        points.iter().map(|p| p.category.as_str()).collect();
    assert!(categories.contains("framework"), "missing framework");
    assert!(categories.contains("http"), "missing http");
    assert!(categories.contains("main"), "missing main");
    assert_eq!(points.len(), 3, "expected exactly 3 entry points");
}

#[test]
fn test_entry_points_category_filter() {
    let (db, _, _, _, _, _) = setup_entry_points_db();

    let http = query::get_entry_points(&db, Some("http"), 100).expect("query should succeed");
    assert_eq!(http.len(), 1, "http filter should return 1");
    assert_eq!(http[0].name, "create_user");

    let fw = query::get_entry_points(&db, Some("framework"), 100).expect("query should succeed");
    assert_eq!(fw.len(), 1, "framework filter should return 1");
    assert_eq!(fw[0].name, "handleClick");

    let main = query::get_entry_points(&db, Some("main"), 100).expect("query should succeed");
    assert_eq!(main.len(), 1, "main filter should return 1");
    assert_eq!(main[0].name, "main");

    let none = query::get_entry_points(&db, Some("bogus"), 100).expect("query should succeed");
    assert!(none.is_empty(), "unknown category returns empty");
}

#[test]
fn test_entry_points_excludes_dead() {
    let (db, _, file, _, _, _) = setup_entry_points_db();

    // Add a dead framework-flagged symbol; it must NOT appear.
    let dead = write::insert_symbol(
        &db,
        file,
        "oldHandler",
        "ui::oldHandler",
        "function",
        200,
        210,
        true,
        false,
        "fn oldHandler()",
        "",
        None,
    )
    .expect("insert dead sym");
    db.conn()
        .execute(
            "UPDATE symbols SET is_entry_point = 1, is_dead = 1 WHERE id = ?1",
            [dead],
        )
        .expect("mark dead entry");

    let points = query::get_entry_points(&db, None, 100).expect("query should succeed");
    assert_eq!(points.len(), 3, "dead entry must be excluded");
    assert!(
        points.iter().all(|p| p.name != "oldHandler"),
        "dead symbol leaked into results"
    );
}

#[test]
fn test_get_entry_points_mcp_tool() {
    use std::borrow::Cow;
    use std::sync::Arc;

    use ariadne::mcp::tools::AriadneService;
    use rmcp::model::*;
    use rmcp::service::{AtomicU32RequestIdProvider, Peer, RequestContext, RoleServer};
    use rmcp::ServerHandler;

    let (db, _, _, _, _, _) = setup_entry_points_db();
    let service = AriadneService::new(db);

    let provider = Arc::new(AtomicU32RequestIdProvider::default());
    let client_info = ClientInfo::default();
    let (peer, _rx) = Peer::<RoleServer>::new(provider, client_info);
    let ct = tokio_util::sync::CancellationToken::new();
    let id = RequestId::Number(1);
    let ctx = RequestContext { ct, id, peer };

    let mut args = serde_json::Map::new();
    args.insert("category".to_string(), serde_json::json!("all"));
    let req = CallToolRequestParam {
        name: Cow::Owned("get_entry_points".to_string()),
        arguments: Some(args),
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(service.call_tool(req, ctx))
        .expect("should succeed");

    assert!(
        result.is_error != Some(true),
        "get_entry_points should not error"
    );

    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("should be valid JSON");

    assert!(parsed.get("entry_points").is_some());
    assert_eq!(parsed["count"].as_u64(), Some(3));
    assert_eq!(parsed["category"].as_str(), Some("all"));

    let points = parsed["entry_points"].as_array().expect("array");
    for p in points {
        assert!(p.get("symbol").is_some());
        assert!(p.get("category").is_some());
        assert!(p.get("file").is_some());
    }
}
