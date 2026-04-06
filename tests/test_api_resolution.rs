use ariadne::db::{write, Database};
use ariadne::pipeline::api_resolution::resolve_api_boundaries;
use rusqlite::params;

/// Create a test DB with 2 services, files, and symbols for API resolution tests.
fn setup_api_db() -> (Database, i64, i64) {
    let db = Database::open_in_memory().expect("in-memory db");

    let svc_a = write::insert_service(
        &db,
        "frontend",
        "/tmp/frontend",
        "microservice",
        "typescript",
    )
    .expect("insert service A");
    let svc_b = write::insert_service(&db, "backend", "/tmp/backend", "microservice", "python")
        .expect("insert service B");

    let _file_a = write::insert_file(
        &db,
        svc_a,
        "src/api.ts",
        "/tmp/frontend/src/api.ts",
        "typescript",
        0.0,
    )
    .expect("insert file A");
    let _file_b = write::insert_file(
        &db,
        svc_b,
        "app/routes.py",
        "/tmp/backend/app/routes.py",
        "python",
        0.0,
    )
    .expect("insert file B");

    (db, svc_a, svc_b)
}

#[test]
fn test_resolve_matching_endpoint() {
    let (db, svc_a, svc_b) = setup_api_db();

    // Service B exposes GET /api/users
    write::insert_api_endpoint(&db, svc_b, "GET", "/api/users", None, None, None)
        .expect("insert endpoint");

    // Service A calls GET /api/users
    let call_id = write::insert_api_call(&db, svc_a, "GET", "/api/users", None, None, None, false)
        .expect("insert api call");

    resolve_api_boundaries(&db).expect("resolve");

    let conn = db.conn();
    let (resolved_ep, resolved_svc): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT resolved_endpoint_id, resolved_service_id FROM api_calls WHERE id = ?1",
            params![call_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("query resolved call");

    assert!(
        resolved_ep.is_some(),
        "resolved_endpoint_id should be set after matching"
    );
    assert_eq!(
        resolved_svc,
        Some(svc_b),
        "resolved_service_id should point to the backend service"
    );
}

#[test]
fn test_resolve_no_match_method_mismatch() {
    let (db, svc_a, svc_b) = setup_api_db();

    // Service B exposes GET /api/users
    write::insert_api_endpoint(&db, svc_b, "GET", "/api/users", None, None, None)
        .expect("insert endpoint");

    // Service A calls POST /api/users (method mismatch)
    let call_id = write::insert_api_call(&db, svc_a, "POST", "/api/users", None, None, None, false)
        .expect("insert api call");

    resolve_api_boundaries(&db).expect("resolve");

    let conn = db.conn();
    let resolved_ep: Option<i64> = conn
        .query_row(
            "SELECT resolved_endpoint_id FROM api_calls WHERE id = ?1",
            params![call_id],
            |row| row.get(0),
        )
        .expect("query");

    assert!(resolved_ep.is_none(), "POST should not match GET endpoint");
}

#[test]
fn test_resolve_skips_already_resolved() {
    let (db, svc_a, svc_b) = setup_api_db();

    let ep_id = write::insert_api_endpoint(&db, svc_b, "GET", "/api/users", None, None, None)
        .expect("insert endpoint");

    // Insert call with resolved_endpoint_id already set
    let conn = db.conn();
    conn.execute(
        "INSERT INTO api_calls (service_id, method, url_pattern, resolved_endpoint_id, resolved_service_id)
         VALUES (?1, 'GET', '/api/users', ?2, ?3)",
        params![svc_a, ep_id, svc_b],
    )
    .expect("insert pre-resolved call");

    let call_id = conn.last_insert_rowid();

    // Add a second endpoint that could also match
    let ep2_id = write::insert_api_endpoint(&db, svc_b, "GET", "/api/users/v2", None, None, None)
        .expect("insert endpoint 2");

    resolve_api_boundaries(&db).expect("resolve");

    // Should still point to the original endpoint, not re-resolved
    let resolved_ep: Option<i64> = conn
        .query_row(
            "SELECT resolved_endpoint_id FROM api_calls WHERE id = ?1",
            params![call_id],
            |row| row.get(0),
        )
        .expect("query");

    assert_eq!(
        resolved_ep,
        Some(ep_id),
        "pre-resolved call should not be re-resolved to ep2 ({})",
        ep2_id
    );
}

#[test]
fn test_resolve_multiple_calls() {
    let (db, svc_a, svc_b) = setup_api_db();

    // Two endpoints on backend
    write::insert_api_endpoint(&db, svc_b, "GET", "/api/users", None, None, None)
        .expect("insert users endpoint");
    write::insert_api_endpoint(&db, svc_b, "POST", "/api/orders", None, None, None)
        .expect("insert orders endpoint");

    // Three calls from frontend
    let c1 = write::insert_api_call(&db, svc_a, "GET", "/api/users", None, None, None, false)
        .expect("call 1");
    let c2 = write::insert_api_call(&db, svc_a, "POST", "/api/orders", None, None, None, false)
        .expect("call 2");
    let c3 = write::insert_api_call(&db, svc_a, "DELETE", "/api/items", None, None, None, false)
        .expect("call 3");

    resolve_api_boundaries(&db).expect("resolve");

    let conn = db.conn();
    let check = |id: i64| -> bool {
        let resolved: Option<i64> = conn
            .query_row(
                "SELECT resolved_endpoint_id FROM api_calls WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .expect("query");
        resolved.is_some()
    };

    assert!(check(c1), "GET /api/users should resolve");
    assert!(check(c2), "POST /api/orders should resolve");
    assert!(
        !check(c3),
        "DELETE /api/items should NOT resolve (no matching endpoint)"
    );
}
