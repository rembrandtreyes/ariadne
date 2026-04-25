//! Integration tests for the `propose_edit_plan` MCP tool.
//!
//! `propose_edit_plan(symbol)` composes get_dependents + affected_tests +
//! get_execution_flows and orders the dependent set leaves-first (callees
//! within the cone before their callers), so an agent refactoring the
//! target can update callers in a safe topological sequence. On cycle
//! detection, the tool falls back to BFS-depth-grouped ordering and
//! flags `cycle_detected: true`.

use std::borrow::Cow;
use std::sync::Arc;

use ariadne::db::{write, Database};
use ariadne::mcp::tools::AriadneService;
use rmcp::model::*;
use rmcp::service::{AtomicU32RequestIdProvider, Peer, RequestContext, RoleServer};
use rmcp::ServerHandler;
use rusqlite::params;

// ---------------------------------------------------------------------------
// Fixture helpers (cloned from tests/test_affected_tests.rs)
// ---------------------------------------------------------------------------

fn setup_db(symbols: &[(&str, bool)], edges: &[(usize, usize)]) -> (Database, Vec<i64>) {
    let db = Database::open_in_memory().expect("in-memory db");
    let svc = write::insert_service(&db, "test-svc", "/tmp/test", "monolith", "rust")
        .expect("insert service");
    let file = write::insert_file(
        &db,
        svc,
        "src/module.rs",
        "/tmp/test/src/module.rs",
        "rust",
        0.0,
    )
    .expect("insert file");

    let mut sym_ids = Vec::with_capacity(symbols.len());
    for (i, (name, is_test)) in symbols.iter().enumerate() {
        let qname = format!("module::{}", name);
        let id = write::insert_symbol(
            &db,
            file,
            name,
            &qname,
            "function",
            (i as u32) * 10 + 1,
            (i as u32) * 10 + 9,
            true,
            *is_test,
            &format!("fn {}()", name),
            "[]",
            None,
        )
        .expect("insert symbol");
        sym_ids.push(id);
    }

    let conn = db.conn();
    for (caller_idx, callee_idx) in edges {
        conn.execute(
            "INSERT INTO calls (caller_symbol_id, callee_symbol_id, callee_name, file_id, line, confidence, resolution)
             VALUES (?1, ?2, ?3, ?4, ?5, 1.0, 'resolved')",
            params![
                sym_ids[*caller_idx],
                sym_ids[*callee_idx],
                symbols[*callee_idx].0,
                file,
                (*caller_idx as u32) * 10 + 3,
            ],
        )
        .expect("insert call edge");
    }

    (db, sym_ids)
}

fn test_context() -> RequestContext<RoleServer> {
    let provider = Arc::new(AtomicU32RequestIdProvider::default());
    let client_info = ClientInfo::default();
    let (peer, _rx) = Peer::<RoleServer>::new(provider, client_info);
    let ct = tokio_util::sync::CancellationToken::new();
    let id = RequestId::Number(1);
    RequestContext { ct, id, peer }
}

fn tool_request(name: &str, symbol: &str) -> CallToolRequestParam {
    let mut args = serde_json::Map::new();
    args.insert("symbol".to_string(), serde_json::json!(symbol));
    CallToolRequestParam {
        name: Cow::Owned(name.to_string()),
        arguments: Some(args),
    }
}

async fn call_propose_edit_plan(service: &AriadneService, symbol: &str) -> serde_json::Value {
    let ctx = test_context();
    let req = tool_request("propose_edit_plan", symbol);
    let result = service
        .call_tool(req, ctx)
        .await
        .expect("call_tool should succeed");
    assert!(
        result.is_error != Some(true),
        "propose_edit_plan should not raise tool error (use structured summary instead)"
    );
    let text = result
        .content
        .iter()
        .find_map(|c| c.as_text().map(|t| t.text.clone()))
        .expect("text content");
    serde_json::from_str(&text).expect("valid JSON response")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// DAG: target ← c1 ← c2  (c1 calls target, c2 calls c1)
/// edit_order should be [c1, c2] — c1 first because it's depth-1 (closer to
/// target = "leaf within the dependent cone"), c2 second.
#[tokio::test]
async fn test_propose_edit_plan_topological_order() {
    let (db, ids) = setup_db(
        &[("target", false), ("c1", false), ("c2", false)],
        // edges = (caller_idx, callee_idx)
        &[
            (1, 0), // c1 calls target
            (2, 1), // c2 calls c1
        ],
    );
    let service = AriadneService::new(db);
    let response = call_propose_edit_plan(&service, "target").await;

    assert_eq!(response["symbol"]["name"], "target");
    assert_eq!(response["total_dependents"], 2);
    assert_eq!(response["cycle_detected"], false);
    assert_eq!(response["ordering_strategy"], "topological");

    let edit_order = response["edit_order"]
        .as_array()
        .expect("edit_order is array");
    assert_eq!(edit_order.len(), 2, "expected 2 dependents in edit order");
    // c1 is depth 1; c2 is depth 2 — c1 must come first.
    assert_eq!(
        edit_order[0]["name"], "c1",
        "depth-1 caller must precede deeper callers"
    );
    assert_eq!(edit_order[0]["depth"], 1);
    assert_eq!(edit_order[1]["name"], "c2");
    assert_eq!(edit_order[1]["depth"], 2);

    // Symbol id should match the seeded ids.
    assert_eq!(edit_order[0]["symbol_id"].as_i64(), Some(ids[1]));
    assert_eq!(edit_order[1]["symbol_id"].as_i64(), Some(ids[2]));
}

/// Calling propose_edit_plan twice on the same DB must yield byte-identical
/// edit_order arrays — no HashMap-iteration-order dependence.
#[tokio::test]
async fn test_propose_edit_plan_idempotent_ordering() {
    // Diamond dependent set: target ← c1, c2 (both depth 1); c3 calls both c1 and c2.
    let (db, _ids) = setup_db(
        &[
            ("target", false),
            ("c1", false),
            ("c2", false),
            ("c3", false),
        ],
        &[
            (1, 0), // c1 calls target
            (2, 0), // c2 calls target
            (3, 1), // c3 calls c1
            (3, 2), // c3 calls c2
        ],
    );
    let service = AriadneService::new(db);

    let r1 = call_propose_edit_plan(&service, "target").await;
    let r2 = call_propose_edit_plan(&service, "target").await;

    assert_eq!(
        r1["edit_order"], r2["edit_order"],
        "edit_order must be deterministic across repeat calls (secondary sort by symbol_id)"
    );
    assert_eq!(r1["total_dependents"], 3);
    // depth-1 group should come before depth-2 group; within depth-1, sorted by symbol_id (c1 < c2).
    let order = r1["edit_order"].as_array().unwrap();
    assert_eq!(order[0]["name"], "c1");
    assert_eq!(order[1]["name"], "c2");
    assert_eq!(order[2]["name"], "c3");
}

/// Cycle in dependent cone: c1 ← c2 ← c1 (c1 and c2 call each other AND
/// transitively reach target). Tool should detect the cycle, fall back to
/// BFS-depth ordering, set cycle_detected=true, and not crash.
#[tokio::test]
async fn test_propose_edit_plan_cycle_fallback() {
    let (db, _ids) = setup_db(
        &[("target", false), ("c1", false), ("c2", false)],
        &[
            (1, 0), // c1 calls target
            (2, 1), // c2 calls c1
            (1, 2), // c1 calls c2 — cycle between c1 and c2
        ],
    );
    let service = AriadneService::new(db);
    let response = call_propose_edit_plan(&service, "target").await;

    assert_eq!(
        response["cycle_detected"], true,
        "cycle in dependent cone must be flagged"
    );
    assert_eq!(response["ordering_strategy"], "bfs_depth_fallback");
    assert_eq!(response["total_dependents"], 2);
    // Edit order is non-empty even with cycle (BFS depth still terminates).
    let edit_order = response["edit_order"]
        .as_array()
        .expect("edit_order is array");
    assert_eq!(edit_order.len(), 2);
    // c1 (depth-1) must come before c2 (depth-2 via c1) by BFS depth.
    assert_eq!(edit_order[0]["name"], "c1");
}

/// Missing symbol returns 200 with structured summary (mirrors
/// dependency_path / get_entry_points patterns), never tool error.
#[tokio::test]
async fn test_propose_edit_plan_missing_symbol() {
    let (db, _ids) = setup_db(&[("real", false)], &[]);
    let service = AriadneService::new(db);
    let response = call_propose_edit_plan(&service, "NoSuchSymbol").await;

    assert_eq!(response["total_dependents"], 0);
    assert_eq!(response["edit_order"].as_array().map(|a| a.len()), Some(0));
    let summary = response["summary"].as_str().expect("summary is string");
    assert!(
        summary.contains("NoSuchSymbol"),
        "summary must mention the missing symbol name; got: {summary}"
    );
}
