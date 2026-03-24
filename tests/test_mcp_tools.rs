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
    assert_eq!(result.tools.len(), 10, "Ariadne exposes 10 MCP tools");
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
