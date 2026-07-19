use std::borrow::Cow;
use std::sync::Arc;

use ariadne::commands::resolve_symbol;
use ariadne::db::{query, Database};
use ariadne::mcp::tools::AriadneService;
use rmcp::model::*;
use rmcp::service::{AtomicU32RequestIdProvider, Peer, RequestContext, RoleServer};
use rmcp::ServerHandler;

/// Two files each defining a `process` function (bare-name collision), plus a
/// unique `alpha` symbol as the control. `billing.process` gets the lower
/// rowid so a LIMIT-1 silent pick would always grab it over `orders.process`.
fn setup_collision_db() -> Database {
    let db = Database::open_in_memory().expect("in-memory db");
    populate_collision_fixture(&db);
    db
}

fn populate_collision_fixture(db: &Database) {
    let conn = db.conn();

    conn.execute(
        "INSERT INTO services (id, name, type, repo_path) VALUES (1, 'test', 'monolith', '/test')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO files (id, service_id, path, absolute_path, language, last_modified, last_indexed) \
         VALUES (1, 1, 'src/billing.py', '/test/src/billing.py', 'python', 0.0, 0.0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO files (id, service_id, path, absolute_path, language, last_modified, last_indexed) \
         VALUES (2, 1, 'src/orders.py', '/test/src/orders.py', 'python', 0.0, 0.0)",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end) \
         VALUES (1, 1, 'process', 'billing.process', 'function', 10, 20)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end) \
         VALUES (2, 2, 'process', 'orders.process', 'function', 5, 15)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO symbols (id, file_id, name, qualified_name, kind, line_start, line_end) \
         VALUES (3, 1, 'alpha', 'billing.alpha', 'function', 30, 40)",
        [],
    )
    .unwrap();
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

/// Helper: build a CallToolRequestParam with a single `symbol` argument.
fn symbol_request(tool: &str, symbol: &str) -> CallToolRequestParam {
    let mut args = serde_json::Map::new();
    args.insert("symbol".to_string(), serde_json::json!(symbol));
    CallToolRequestParam {
        name: Cow::Owned(tool.to_string()),
        arguments: Some(args),
    }
}

/// (a) A name with exactly one match resolves exactly as before.
#[test]
fn unique_name_resolves_as_before() {
    let db = setup_collision_db();

    let sym = resolve_symbol(&db, "alpha").expect("unique name must resolve");
    assert_eq!(sym.id, 3);
    assert_eq!(sym.qualified_name, "billing.alpha");

    let found = query::find_symbol_by_name(&db, "alpha").unwrap();
    assert_eq!(found.expect("must be found").id, 3);
}

/// (b) A bare-name collision must surface as an error at the CLI resolver,
/// not silently pick the lowest rowid.
#[test]
fn cli_resolver_errors_on_bare_name_collision() {
    let db = setup_collision_db();

    let result = resolve_symbol(&db, "process");
    assert!(
        result.is_err(),
        "colliding bare name must not silently resolve; got {:?}",
        result.map(|s| s.qualified_name)
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.to_lowercase().contains("ambiguous"),
        "error must say the symbol is ambiguous; got: {msg}"
    );
}

/// (c) A qualified-name query resolves uniquely despite the bare-name collision.
#[test]
fn qualified_name_resolves_despite_collision() {
    let db = setup_collision_db();

    let sym = resolve_symbol(&db, "orders.process").expect("qualified name must resolve");
    assert_eq!(sym.id, 2);
    assert_eq!(sym.qualified_name, "orders.process");
}

/// (d) The documented-but-unimplemented `module:func` suffix lookup must match
/// against qualified_name — "orders:process" is unambiguous even though the
/// bare suffix "process" collides (and the collision's lowest rowid is the
/// WRONG file).
#[test]
fn module_colon_suffix_resolves_against_qualified_name() {
    let db = setup_collision_db();

    let sym = resolve_symbol(&db, "orders:process").expect("module:func lookup must resolve");
    assert_eq!(
        sym.qualified_name, "orders.process",
        "suffix lookup must use the module qualifier, not pick the lowest rowid"
    );
}

/// (e) An MCP tool given an ambiguous bare name must return a structured
/// candidates listing, not a silent pick.
#[tokio::test]
async fn mcp_ambiguous_symbol_returns_structured_candidates() {
    let service = AriadneService::new(setup_collision_db());
    let ctx = test_context();

    let result = service
        .call_tool(symbol_request("why_symbol", "process"), ctx)
        .await
        .expect("call_tool should succeed");

    assert_eq!(
        result.is_error,
        Some(true),
        "ambiguous symbol must not silently resolve to one candidate"
    );
    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value =
        serde_json::from_str(text).expect("ambiguity reply must be structured JSON");
    assert_eq!(parsed["error"], "ambiguous_symbol");
    let candidates = parsed["candidates"]
        .as_array()
        .expect("must list candidates");
    assert_eq!(candidates.len(), 2, "both collisions must be listed");
    // Deterministic order: file path, then line, then id.
    assert_eq!(candidates[0]["qualified_name"], "billing.process");
    assert_eq!(candidates[1]["qualified_name"], "orders.process");
    for c in candidates {
        for key in ["id", "name", "qualified_name", "file", "line", "kind"] {
            assert!(c.get(key).is_some(), "candidate missing {key}: {c}");
        }
    }
}

/// The query-level resolver reports ALL candidates, with file paths, in
/// deterministic (file path, line, id) order.
#[test]
fn query_resolver_returns_both_candidates_in_order() {
    let db = setup_collision_db();

    match query::resolve_symbol_by_name(&db, "process", None).unwrap() {
        query::SymbolResolution::Ambiguous(candidates) => {
            assert_eq!(candidates.len(), 2);
            assert_eq!(candidates[0].symbol.qualified_name, "billing.process");
            assert_eq!(candidates[0].file_path, "src/billing.py");
            assert_eq!(candidates[1].symbol.qualified_name, "orders.process");
            assert_eq!(candidates[1].file_path, "src/orders.py");
        }
        other => panic!("expected Ambiguous, got {other:?}"),
    }
}

/// A file-path hint (suffix match) narrows an ambiguous name to Unique; a
/// hint that eliminates every candidate is ignored rather than turning the
/// collision into a miss.
#[test]
fn file_hint_narrows_ambiguity() {
    let db = setup_collision_db();

    match query::resolve_symbol_by_name(&db, "process", Some("orders.py")).unwrap() {
        query::SymbolResolution::Unique(sym) => {
            assert_eq!(sym.qualified_name, "orders.process");
        }
        other => panic!("expected Unique via hint, got {other:?}"),
    }

    match query::resolve_symbol_by_name(&db, "process", Some("no/such/file.py")).unwrap() {
        query::SymbolResolution::Ambiguous(candidates) => assert_eq!(candidates.len(), 2),
        other => panic!("useless hint must leave the collision visible, got {other:?}"),
    }
}

/// Miss semantics are unchanged: unknown names are NotFound / None.
#[test]
fn not_found_semantics_preserved() {
    let db = setup_collision_db();

    assert!(matches!(
        query::resolve_symbol_by_name(&db, "no_such_symbol", None).unwrap(),
        query::SymbolResolution::NotFound
    ));
    assert!(query::find_symbol_by_name(&db, "no_such_symbol")
        .unwrap()
        .is_none());
}

/// propose_edit_plan keeps its soft contract on ambiguity: structured 200-style
/// success with a summary and the candidate list, never a tool error.
#[tokio::test]
async fn mcp_propose_edit_plan_soft_reply_on_ambiguity() {
    let service = AriadneService::new(setup_collision_db());
    let ctx = test_context();

    let result = service
        .call_tool(symbol_request("propose_edit_plan", "process"), ctx)
        .await
        .expect("call_tool should succeed");

    assert!(
        result.is_error != Some(true),
        "propose_edit_plan must soft-reply on ambiguity, not error"
    );
    let text = match &result.content[0].raw {
        RawContent::Text(t) => &t.text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).expect("must be structured JSON");
    assert!(parsed["symbol"].is_null());
    assert_eq!(parsed["total_dependents"], 0);
    let summary = parsed["summary"].as_str().expect("summary is string");
    assert!(
        summary.to_lowercase().contains("ambiguous") && summary.contains("process"),
        "summary must flag the ambiguity; got: {summary}"
    );
    assert_eq!(
        parsed["candidates"].as_array().map(|a| a.len()),
        Some(2),
        "both candidates must be listed"
    );
}

/// Dashboard endpoints return the 409-mapped ambiguous_symbol error with the
/// candidate list instead of silently picking one symbol.
#[tokio::test]
async fn dashboard_endpoints_reject_ambiguous_names_with_candidates() {
    use ariadne::dashboard::api::{
        dependency_path, propose_edit_plan, DependencyPathQuery, ProposeEditPlanQuery,
    };
    use axum::extract::{Query, State};

    // Handlers open the DB from a path, so build the fixture on disk.
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    {
        let db = Database::open(&db_path).unwrap();
        populate_collision_fixture(&db);
    }
    let state: ariadne::dashboard::api::DbState = Arc::new(db_path);

    let err = match dependency_path(
        State(state.clone()),
        Query(DependencyPathQuery {
            from: "process".to_string(),
            to: "alpha".to_string(),
        }),
    )
    .await
    {
        Err(e) => e,
        Ok(_) => panic!("ambiguous 'from' must be rejected"),
    };
    assert_eq!(err.code, "ambiguous_symbol");
    let candidates = err.candidates.as_deref().expect("candidates listed");
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].qualified_name, "billing.process");
    assert_eq!(candidates[1].qualified_name, "orders.process");

    let err = match propose_edit_plan(
        State(state),
        Query(ProposeEditPlanQuery {
            symbol: "process".to_string(),
        }),
    )
    .await
    {
        Err(e) => e,
        Ok(_) => panic!("ambiguous target must be rejected"),
    };
    assert_eq!(err.code, "ambiguous_symbol");
    assert_eq!(err.candidates.as_deref().map(<[_]>::len), Some(2));
}
