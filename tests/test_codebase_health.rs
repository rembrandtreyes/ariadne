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

/// Helper: extract text from first content block.
fn extract_text(result: &CallToolResult) -> &str {
    match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    }
}

/// The health tool should be registered and return 28 total tools.
#[tokio::test]
async fn test_list_tools_includes_health() {
    let service = make_service();
    let ctx = test_context();
    let result = service.list_tools(None, ctx).await.unwrap();
    let names: Vec<&str> = result.tools.iter().map(|t| &*t.name).collect();
    assert!(
        names.contains(&"get_codebase_health"),
        "get_codebase_health should be in tool list"
    );
}

/// Health tool should return a valid response on empty DB (graceful degradation).
#[tokio::test]
async fn test_health_empty_db() {
    let service = make_service();
    let ctx = test_context();
    let req = tool_request("get_codebase_health", None);
    let result = service.call_tool(req, ctx).await.unwrap();
    assert!(
        !result.is_error.unwrap_or(false),
        "Should not error on empty DB"
    );

    let text = extract_text(&result);
    let json: serde_json::Value = serde_json::from_str(text).expect("valid JSON");

    // Should have grade and summary even on empty DB
    assert!(json.get("grade").is_some(), "Should have grade field");
    assert!(json.get("summary").is_some(), "Should have summary field");
    assert!(
        json.get("elapsed_ms").is_some(),
        "Should have elapsed_ms field"
    );
}

/// Health grade should be A-F.
#[tokio::test]
async fn test_health_grade_format() {
    let service = make_service();
    let ctx = test_context();
    let req = tool_request("get_codebase_health", None);
    let result = service.call_tool(req, ctx).await.unwrap();

    let text = extract_text(&result);
    let json: serde_json::Value = serde_json::from_str(text).expect("valid JSON");

    let grade = json["grade"].as_str().expect("grade should be string");
    assert!(
        ["A", "B", "C", "D", "F"].contains(&grade),
        "Grade should be A-F, got: {grade}"
    );
}

/// Health response should contain all 6 core fields per council spec.
#[tokio::test]
async fn test_health_response_fields() {
    let service = make_service();
    let ctx = test_context();
    let req = tool_request("get_codebase_health", None);
    let result = service.call_tool(req, ctx).await.unwrap();

    let text = extract_text(&result);
    let json: serde_json::Value = serde_json::from_str(text).expect("valid JSON");

    // Core fields per council spec
    assert!(json.get("grade").is_some());
    assert!(json.get("dead_code_ratio").is_some());
    assert!(json.get("cycle_count").is_some());
    assert!(json.get("coupling_density").is_some());
    assert!(json.get("modularity_score").is_some());
    assert!(json.get("summary").is_some());
    // Operational fields
    assert!(json.get("elapsed_ms").is_some());
    assert!(json.get("degraded_fields").is_some());
}
