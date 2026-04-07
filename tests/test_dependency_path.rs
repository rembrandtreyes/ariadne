use ariadne::db::Database;
use ariadne::graph::traversal::find_shortest_path;
use ariadne::graph::{CallEdge, CallGraph, SymbolNode};
use ariadne::mcp::tools::AriadneService;
use rmcp::model::*;
use rmcp::service::{AtomicU32RequestIdProvider, Peer, RequestContext, RoleServer};
use rmcp::ServerHandler;
use std::borrow::Cow;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Graph-layer fixtures (no DB needed — test find_shortest_path directly)
// ---------------------------------------------------------------------------

fn make_node(id: i64, name: &str) -> SymbolNode {
    SymbolNode {
        id,
        name: name.to_string(),
        qualified_name: format!("mod::{name}"),
        kind: "function".to_string(),
        file_path: "src/lib.rs".to_string(),
        is_dead: false,
        is_test: false,
    }
}

fn make_edge() -> CallEdge {
    CallEdge {
        confidence: 1.0,
        resolution: "import".to_string(),
        line: 1,
    }
}

fn linear_graph() -> CallGraph {
    // A → B → C
    let mut g = CallGraph::new();
    g.add_symbol(make_node(1, "a"));
    g.add_symbol(make_node(2, "b"));
    g.add_symbol(make_node(3, "c"));
    g.add_call(1, 2, make_edge());
    g.add_call(2, 3, make_edge());
    g
}

// ---------------------------------------------------------------------------
// Traversal unit tests
// ---------------------------------------------------------------------------

#[test]
fn test_direct_path() {
    // A → B: path should be [1, 2]
    let g = linear_graph();
    let path = find_shortest_path(&g, 1, 2).expect("path should exist");
    assert_eq!(path, vec![1, 2]);
}

#[test]
fn test_transitive_path() {
    // A → B → C: shortest path from A to C is [1, 2, 3]
    let g = linear_graph();
    let path = find_shortest_path(&g, 1, 3).expect("path should exist");
    assert_eq!(path, vec![1, 2, 3]);
}

#[test]
fn test_no_path_returns_none() {
    // C has no outgoing edges — no path from C to A
    let g = linear_graph();
    assert!(find_shortest_path(&g, 3, 1).is_none());
}

#[test]
fn test_same_symbol_returns_singleton() {
    let g = linear_graph();
    let path = find_shortest_path(&g, 2, 2).expect("same symbol always reachable");
    assert_eq!(path, vec![2]);
}

#[test]
fn test_missing_symbol_returns_none() {
    let g = linear_graph();
    assert!(find_shortest_path(&g, 99, 1).is_none());
    assert!(find_shortest_path(&g, 1, 99).is_none());
}

#[test]
fn test_shortest_path_prefers_direct_over_longer() {
    // Diamond: A → B → D and A → D (direct).  Shortest A→D is length 1.
    let mut g = CallGraph::new();
    g.add_symbol(make_node(1, "a"));
    g.add_symbol(make_node(2, "b"));
    g.add_symbol(make_node(3, "d"));
    g.add_call(1, 2, make_edge()); // A → B
    g.add_call(2, 3, make_edge()); // B → D
    g.add_call(1, 3, make_edge()); // A → D (direct)
    let path = find_shortest_path(&g, 1, 3).expect("path exists");
    // Direct path has 1 hop (length 2 nodes)
    assert_eq!(path.len(), 2, "should prefer direct path: {:?}", path);
    assert_eq!(path[0], 1);
    assert_eq!(path[1], 3);
}

// ---------------------------------------------------------------------------
// MCP tool registration test
// ---------------------------------------------------------------------------

fn test_context() -> RequestContext<RoleServer> {
    let provider = Arc::new(AtomicU32RequestIdProvider::default());
    let client_info = ClientInfo::default();
    let (peer, _rx) = Peer::<RoleServer>::new(provider, client_info);
    let ct = tokio_util::sync::CancellationToken::new();
    let id = RequestId::Number(1);
    RequestContext { ct, id, peer }
}

#[tokio::test]
async fn test_tool_is_registered() {
    let db = Database::open_in_memory().expect("in-memory db");
    let service = AriadneService::new(db);
    let ctx = test_context();
    let result = service.list_tools(None, ctx).await.unwrap();
    let names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(
        names.contains(&"get_dependency_path"),
        "get_dependency_path should be registered; got: {names:?}"
    );
}

#[tokio::test]
async fn test_unknown_symbol_returns_error_not_panic() {
    let db = Database::open_in_memory().expect("in-memory db");
    let service = AriadneService::new(db);
    let ctx = test_context();
    let mut args = serde_json::Map::new();
    args.insert(
        "from_symbol".to_string(),
        serde_json::json!("nonexistent_a"),
    );
    args.insert("to_symbol".to_string(), serde_json::json!("nonexistent_b"));
    let req = CallToolRequestParam {
        name: Cow::Borrowed("get_dependency_path"),
        arguments: Some(args),
    };
    let result = service.call_tool(req, ctx).await.unwrap();
    // Should be an error result (symbol not found), not a panic or empty result
    assert!(
        result.is_error.unwrap_or(false),
        "missing symbols should return error"
    );
}
