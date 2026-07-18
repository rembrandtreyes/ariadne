use std::borrow::Cow;
use std::sync::Arc;

use ariadne::db::Database;
use ariadne::mcp::tools::AriadneService;
use rmcp::model::*;
use rmcp::service::{AtomicU32RequestIdProvider, Peer, RequestContext, RoleServer};
use rmcp::ServerHandler;

/// Helper: create an AriadneService backed by an in-memory DB.
fn make_service() -> AriadneService {
    let db = Database::open_in_memory().expect("in-memory db");
    AriadneService::new(db)
}

/// Helper: build a `RequestContext<RoleServer>` suitable for tests.
fn test_context() -> RequestContext<RoleServer> {
    let provider = Arc::new(AtomicU32RequestIdProvider::default());
    let client_info = ClientInfo::default();
    let (peer, _rx) = Peer::<RoleServer>::new(provider, client_info);
    let ct = tokio_util::sync::CancellationToken::new();
    let id = RequestId::Number(1);
    RequestContext { ct, id, peer }
}

/// Helper: build a CallToolRequestParam.
fn tool_request(
    name: &str,
    args: Option<serde_json::Map<String, serde_json::Value>>,
) -> CallToolRequestParam {
    CallToolRequestParam {
        name: Cow::Owned(name.to_string()),
        arguments: args,
    }
}

#[test]
fn test_service_creation() {
    let _service = make_service();
}

#[test]
fn test_server_info() {
    let service = make_service();
    let info = service.get_info();
    assert_eq!(info.server_info.name, "ariadne");
    assert!(!info.server_info.version.is_empty());
    assert!(info.instructions.is_some());
}

#[tokio::test]
async fn test_list_tools_returns_full_surface() {
    let service = make_service();
    let ctx = test_context();
    let result = service
        .list_tools(None, ctx)
        .await
        .expect("list_tools should succeed");
    assert_eq!(result.tools.len(), 32, "Ariadne exposes 32 MCP tools");
}

#[tokio::test]
async fn test_search_tool_dispatch() {
    let service = make_service();
    let ctx = test_context();

    let mut args = serde_json::Map::new();
    args.insert("query".to_string(), serde_json::json!("nonexistent_xyz"));

    let req = tool_request("search_symbol", Some(args));
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    // The search should return a success result (possibly empty list)
    assert!(
        result.is_error != Some(true),
        "search_symbol should not error on empty DB"
    );
    assert!(
        !result.content.is_empty(),
        "should have at least one content block"
    );
}

#[tokio::test]
async fn test_get_complexity_tool() {
    let service = make_service();
    let ctx = test_context();

    let req = tool_request("get_complexity", None);
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert!(
        result.is_error != Some(true),
        "get_complexity should not error"
    );

    // Parse the content as JSON and verify expected keys
    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("should be valid JSON");
    assert!(parsed.get("files").is_some(), "should have 'files' key");
    assert!(parsed.get("symbols").is_some(), "should have 'symbols' key");
    assert!(parsed.get("calls").is_some(), "should have 'calls' key");
    assert!(
        parsed.get("dead_functions").is_some(),
        "should have 'dead_functions' key"
    );
    assert!(
        parsed.get("resolution_rate").is_some(),
        "should have 'resolution_rate' key"
    );
    assert!(
        parsed.get("languages").is_some(),
        "should have 'languages' key"
    );
}

#[tokio::test]
async fn test_unknown_tool_error() {
    let service = make_service();
    let ctx = test_context();

    let req = tool_request("nonexistent_tool", None);
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert_eq!(
        result.is_error,
        Some(true),
        "unknown tool should return error"
    );

    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    assert!(
        text.contains("Unknown tool"),
        "error message should mention unknown tool, got: {}",
        text
    );
}

#[tokio::test]
async fn test_find_dead_code_empty_db() {
    let service = make_service();
    let ctx = test_context();

    let req = tool_request("find_dead_code", None);
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert!(
        result.is_error != Some(true),
        "find_dead_code should not error on empty DB"
    );
}

#[tokio::test]
async fn test_diff_impact_empty_db() {
    let service = make_service();
    let ctx = test_context();

    let mut args = serde_json::Map::new();
    args.insert(
        "changed_files".to_string(),
        serde_json::json!("src/main.rs,src/lib.rs"),
    );

    let req = tool_request("diff_impact", Some(args));
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert!(
        result.is_error != Some(true),
        "diff_impact should not error on empty DB"
    );

    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("should be valid JSON");
    assert!(
        parsed.get("changed_files").is_some(),
        "should have changed_files key"
    );
    assert!(
        parsed.get("affected_tests").is_some(),
        "should have affected_tests key"
    );
    assert!(
        parsed.get("review_focus").is_some(),
        "should have review_focus key"
    );
}

#[tokio::test]
async fn test_diff_impact_missing_param() {
    let service = make_service();
    let ctx = test_context();

    let req = tool_request("diff_impact", None);
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert_eq!(
        result.is_error,
        Some(true),
        "diff_impact without changed_files should error"
    );
}

#[tokio::test]
async fn test_affected_tests_empty_db() {
    let service = make_service();
    let ctx = test_context();

    let mut args = serde_json::Map::new();
    args.insert(
        "changed_files".to_string(),
        serde_json::json!("src/main.rs"),
    );

    let req = tool_request("affected_tests", Some(args));
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert!(
        result.is_error != Some(true),
        "affected_tests should not error on empty DB"
    );

    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("should be valid JSON");
    assert!(
        parsed.get("affected_tests").is_some(),
        "should have affected_tests key"
    );
    assert_eq!(
        parsed.get("count").and_then(|v| v.as_u64()),
        Some(0),
        "empty DB should have zero affected tests"
    );
}

#[tokio::test]
async fn test_why_symbol_not_found() {
    let service = make_service();
    let ctx = test_context();

    let mut args = serde_json::Map::new();
    args.insert(
        "symbol".to_string(),
        serde_json::json!("nonexistent_symbol_xyz"),
    );

    let req = tool_request("why_symbol", Some(args));
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert_eq!(
        result.is_error,
        Some(true),
        "why_symbol for nonexistent symbol should error"
    );
}

#[tokio::test]
async fn test_tool_names_include_new_tools() {
    let service = make_service();
    let ctx = test_context();
    let result = service
        .list_tools(None, ctx)
        .await
        .expect("list_tools should succeed");

    let names: Vec<String> = result.tools.iter().map(|t| t.name.to_string()).collect();
    assert!(
        names.contains(&"diff_impact".to_string()),
        "should have diff_impact"
    );
    assert!(
        names.contains(&"affected_tests".to_string()),
        "should have affected_tests"
    );
    assert!(
        names.contains(&"why_symbol".to_string()),
        "should have why_symbol"
    );
    assert!(
        names.contains(&"get_heritage".to_string()),
        "should have get_heritage"
    );
    assert!(
        names.contains(&"get_execution_flows".to_string()),
        "should have get_execution_flows"
    );
    assert!(
        names.contains(&"get_coupling".to_string()),
        "should have get_coupling"
    );
    assert!(
        names.contains(&"get_communities".to_string()),
        "should have get_communities"
    );
    assert!(
        names.contains(&"get_api_endpoints".to_string()),
        "should have get_api_endpoints"
    );
}

#[tokio::test]
async fn test_get_heritage_symbol_not_found() {
    let service = make_service();
    let ctx = test_context();

    let mut args = serde_json::Map::new();
    args.insert(
        "symbol".to_string(),
        serde_json::json!("nonexistent_class_xyz"),
    );

    let req = tool_request("get_heritage", Some(args));
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert_eq!(
        result.is_error,
        Some(true),
        "get_heritage for nonexistent symbol should error"
    );
}

#[tokio::test]
async fn test_get_execution_flows_symbol_not_found() {
    let service = make_service();
    let ctx = test_context();

    let mut args = serde_json::Map::new();
    args.insert(
        "symbol".to_string(),
        serde_json::json!("nonexistent_func_xyz"),
    );

    let req = tool_request("get_execution_flows", Some(args));
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert_eq!(
        result.is_error,
        Some(true),
        "get_execution_flows for nonexistent symbol should error"
    );
}

#[tokio::test]
async fn test_get_coupling_empty_db() {
    let service = make_service();
    let ctx = test_context();

    let req = tool_request("get_coupling", None);
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert!(
        result.is_error != Some(true),
        "get_coupling should not error on empty DB"
    );

    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("should be valid JSON");
    assert!(
        parsed.get("coupled_pairs").is_some(),
        "should have 'coupled_pairs' key"
    );
    assert_eq!(
        parsed.get("count").and_then(|v| v.as_u64()),
        Some(0),
        "empty DB should have zero couplings"
    );
}

#[tokio::test]
async fn test_get_communities_empty_db() {
    let service = make_service();
    let ctx = test_context();

    let req = tool_request("get_communities", None);
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert!(
        result.is_error != Some(true),
        "get_communities should not error on empty DB"
    );

    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("should be valid JSON");
    assert!(
        parsed.get("communities").is_some(),
        "should have 'communities' key"
    );
    assert_eq!(
        parsed.get("count").and_then(|v| v.as_u64()),
        Some(0),
        "empty DB should have zero communities"
    );
}

#[tokio::test]
async fn test_get_api_endpoints_empty_db() {
    let service = make_service();
    let ctx = test_context();

    let req = tool_request("get_api_endpoints", None);
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert!(
        result.is_error != Some(true),
        "get_api_endpoints should not error on empty DB"
    );

    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("should be valid JSON");
    assert!(
        parsed.get("endpoints").is_some(),
        "should have 'endpoints' key"
    );
    assert_eq!(
        parsed.get("count").and_then(|v| v.as_u64()),
        Some(0),
        "empty DB should have zero endpoints"
    );
}

#[tokio::test]
async fn test_get_file_summary_includes_parse_error_count() {
    use ariadne::db::write;

    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test", "/tmp/t", "monolith", "typescript").unwrap();
    let file_id = write::insert_file(
        &db,
        svc,
        "src/app.tsx",
        "/tmp/t/src/app.tsx",
        "typescript",
        0.0,
    )
    .unwrap();
    write::set_file_parse_error_count(&db, file_id, 3).unwrap();

    let service = AriadneService::new(db);
    let ctx = test_context();
    let mut args = serde_json::Map::new();
    args.insert("file_path".to_string(), serde_json::json!("src/app.tsx"));
    let req = tool_request("get_file_summary", Some(args));
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert!(
        result.is_error != Some(true),
        "get_file_summary should not error"
    );

    let payload = serde_json::to_string(&result.content).expect("serializable content");
    assert!(
        payload.contains("parse_error_count"),
        "file summary must expose parse_error_count so agents can judge trust — got: {payload}"
    );
    assert!(
        payload.contains('3'),
        "recorded parse_error_count value should round-trip into the summary"
    );
}

// ---------------------------------------------------------------------------
// parse_warnings — answer-level trust propagation
// ---------------------------------------------------------------------------

/// Helper: in-memory service with one symbol, optionally one parse-broken file.
fn service_with_symbol(dirty: bool) -> AriadneService {
    use ariadne::db::write;
    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test", "/tmp/t", "monolith", "rust").unwrap();
    let file = write::insert_file(&db, svc, "src/a.rs", "/tmp/t/src/a.rs", "rust", 0.0).unwrap();
    write::insert_symbol(
        &db,
        file,
        "target_fn",
        "a::target_fn",
        "function",
        1,
        5,
        true,
        false,
        "",
        "",
        None,
    )
    .unwrap();
    if dirty {
        let broken = write::insert_file(
            &db,
            svc,
            "src/broken.tsx",
            "/tmp/t/src/broken.tsx",
            "typescript",
            0.0,
        )
        .unwrap();
        write::set_file_parse_error_count(&db, broken, 4).unwrap();
    }
    AriadneService::new(db)
}

/// Helper: call a tool with a `symbol` arg and return the response text.
async fn call_symbol_tool(service: &AriadneService, tool: &str, symbol: &str) -> String {
    let ctx = test_context();
    let mut args = serde_json::Map::new();
    args.insert("symbol".to_string(), serde_json::json!(symbol));
    let req = tool_request(tool, Some(args));
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    serde_json::to_string(&result.content).expect("serializable content")
}

#[tokio::test]
async fn test_blast_radius_parse_warnings_present_when_index_dirty() {
    let service = service_with_symbol(true);
    let payload = call_symbol_tool(&service, "blast_radius", "target_fn").await;
    assert!(
        payload.contains("parse_warnings"),
        "blast_radius must carry parse_warnings when the index has parse-broken \
         files — its answer may undercount. Got: {payload}"
    );
    assert!(
        payload.contains("broken.tsx"),
        "parse_warnings should name the broken file"
    );
}

#[tokio::test]
async fn test_blast_radius_parse_warnings_absent_when_index_clean() {
    let service = service_with_symbol(false);
    let payload = call_symbol_tool(&service, "blast_radius", "target_fn").await;
    assert!(
        !payload.contains("parse_warnings"),
        "parse_warnings must be ABSENT (not null/empty) on a clean index — \
         always-on noise is the rejected health-tool pattern. Got: {payload}"
    );
}

#[tokio::test]
async fn test_get_context_parse_warnings_present_when_index_dirty() {
    let service = service_with_symbol(true);
    let payload = call_symbol_tool(&service, "get_context", "target_fn").await;
    assert!(
        payload.contains("parse_warnings"),
        "get_context must carry parse_warnings on a dirty index. Got: {payload}"
    );
}

#[tokio::test]
async fn test_get_context_parse_warnings_absent_when_index_clean() {
    let service = service_with_symbol(false);
    let payload = call_symbol_tool(&service, "get_context", "target_fn").await;
    assert!(
        !payload.contains("parse_warnings"),
        "parse_warnings must be absent on a clean index. Got: {payload}"
    );
}

#[tokio::test]
async fn test_propose_edit_plan_miss_carries_parse_warnings_when_dirty() {
    // The killer case: "symbol not found" on a dirty index — the symbol may be
    // missing precisely BECAUSE its file failed to parse. The structured miss
    // must say so.
    let service = service_with_symbol(true);
    let payload = call_symbol_tool(&service, "propose_edit_plan", "GhostComponent").await;
    assert!(
        payload.contains("not found") && payload.contains("parse_warnings"),
        "propose_edit_plan's structured miss must carry parse_warnings on a \
         dirty index — the symbol may be unindexed due to the parse failure. \
         Got: {payload}"
    );
}

// ---------------------------------------------------------------------------
// DB-error propagation — tools must error, not render empty data
// ---------------------------------------------------------------------------

/// Helper: service whose coupling table is gone — queries against it must Err.
fn service_with_broken_table(drop_sql: &str) -> AriadneService {
    use ariadne::db::write;
    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test", "/tmp/t", "monolith", "rust").unwrap();
    let file = write::insert_file(&db, svc, "src/a.rs", "/tmp/t/src/a.rs", "rust", 0.0).unwrap();
    write::insert_symbol(
        &db,
        file,
        "target_fn",
        "a::target_fn",
        "function",
        1,
        5,
        true,
        false,
        "",
        "",
        None,
    )
    .unwrap();
    db.conn().execute_batch(drop_sql).expect("drop table");
    AriadneService::new(db)
}

#[tokio::test]
async fn test_get_context_propagates_db_errors() {
    let service = service_with_broken_table("DROP TABLE coupling");
    let ctx = test_context();
    let mut args = serde_json::Map::new();
    args.insert("symbol".to_string(), serde_json::json!("target_fn"));
    let req = tool_request("get_context", Some(args));
    let result = service.call_tool(req, ctx).await.expect("call_tool");
    assert_eq!(
        result.is_error,
        Some(true),
        "get_context must surface DB errors, not render empty coupled_files"
    );
}

#[tokio::test]
async fn test_why_symbol_propagates_db_errors() {
    let service = service_with_broken_table("DROP TABLE coupling");
    let ctx = test_context();
    let mut args = serde_json::Map::new();
    args.insert("symbol".to_string(), serde_json::json!("target_fn"));
    let req = tool_request("why_symbol", Some(args));
    let result = service.call_tool(req, ctx).await.expect("call_tool");
    assert_eq!(
        result.is_error,
        Some(true),
        "why_symbol must surface DB errors, not render empty callers/couplings"
    );
}

#[tokio::test]
async fn test_get_file_summary_propagates_db_errors() {
    let service = service_with_broken_table("DROP TABLE imports");
    let ctx = test_context();
    let mut args = serde_json::Map::new();
    args.insert("file_path".to_string(), serde_json::json!("src/a.rs"));
    let req = tool_request("get_file_summary", Some(args));
    let result = service.call_tool(req, ctx).await.expect("call_tool");
    assert_eq!(
        result.is_error,
        Some(true),
        "get_file_summary must surface DB errors, not render an empty file"
    );
}

// ---------------------------------------------------------------------------
// Graph completeness — the cached MCP graph must never silently truncate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_blast_radius_sees_edges_beyond_ten_thousand() {
    use ariadne::db::write;

    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test", "/tmp/t", "monolith", "rust").unwrap();
    let file =
        write::insert_file(&db, svc, "src/big.rs", "/tmp/t/src/big.rs", "rust", 0.0).unwrap();

    let sym = |name: &str, line: u32| {
        write::insert_symbol(
            &db,
            file,
            name,
            &format!("big::{name}"),
            "function",
            line,
            line + 1,
            true,
            false,
            "",
            "",
            None,
        )
        .unwrap()
    };
    let hub = sym("hub", 1);
    let noise = sym("noise", 3);
    let deep_target = sym("deep_target", 5);
    let sentinel_caller = sym("sentinel_caller", 7);

    // 10,049 noise edges first, then ONE sentinel edge inserted last. A graph
    // built with an arbitrary 10K-edge LIMIT drops the sentinel, and
    // blast_radius silently omits a real dependent — the worst possible
    // failure for a change-impact tool. All interpolated values are i64s
    // returned by insert_symbol, so building SQL by format! is safe here.
    let insert = |rows: &[String]| {
        db.conn()
            .execute_batch(&format!(
                "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, \
                 file_id, line, confidence, resolution) VALUES {};",
                rows.join(",")
            ))
            .unwrap();
    };
    let mut rows = Vec::with_capacity(500);
    for i in 0..10_049u32 {
        rows.push(format!(
            "({hub}, {noise}, 'noise', {file}, {}, 0.95, 'same_file')",
            10 + i
        ));
        if rows.len() == 500 {
            insert(&rows);
            rows.clear();
        }
    }
    rows.push(format!(
        "({sentinel_caller}, {deep_target}, 'deep_target', {file}, 9, 0.95, 'same_file')"
    ));
    insert(&rows);

    let service = AriadneService::new(db);
    let payload = call_symbol_tool(&service, "blast_radius", "deep_target").await;
    assert!(
        payload.contains("sentinel_caller"),
        "blast_radius must compute on ALL resolved call edges — a truncated \
         graph silently omits real dependents exactly on the large codebases \
         where agents lean on it hardest. Got: {payload}"
    );
}
