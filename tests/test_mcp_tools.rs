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
async fn test_list_tools_returns_ten() {
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
