//! Pipeline Phase 15: Schema Resolution
//!
//! Reads: `files`, `symbols`, `api_calls`, `api_endpoints`, filesystem (OpenAPI/protobuf files)
//! Writes: `api_endpoints`, `metadata`, `api_calls` (boosts confidence), `service_edges`
//!
//! Scans for OpenAPI specs and protobuf definitions, parses them into
//! API endpoint records, links endpoints to handler symbols, and boosts
//! resolution confidence for matching API calls.

use crate::db::Database;
use rusqlite::params;
use std::fs;
use std::path::Path;

/// Phase 15: Resolve schema definitions and link them to symbols.
///
/// Scans for schema definition files (OpenAPI, protobuf) and parses them
/// to extract API endpoint definitions. Links parsed endpoints to handler
/// symbols and boosts matching API call confidence.
pub fn resolve_schemas(db: &Database, root: &Path) -> anyhow::Result<()> {
    let conn = db.conn();

    // Get the service_id (first service, or default to 1)
    let service_id: i64 = conn
        .query_row("SELECT id FROM services ORDER BY id LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap_or(1);

    // --- OpenAPI spec resolution ---

    let openapi_patterns = [
        "openapi.yaml",
        "openapi.yml",
        "openapi.json",
        "swagger.yaml",
        "swagger.yml",
        "swagger.json",
    ];

    for pattern in &openapi_patterns {
        let schema_path = root.join(pattern);
        if schema_path.exists() {
            // Record in metadata
            conn.execute(
                "INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)",
                params![
                    format!("schema:{}", pattern),
                    schema_path.to_string_lossy().to_string()
                ],
            )?;

            // Try to parse the OpenAPI spec
            if let Err(e) = parse_openapi(conn, &schema_path, service_id) {
                tracing::warn!("Failed to parse OpenAPI spec {}: {}", pattern, e);
            }
        }
    }

    // --- Walk for additional OpenAPI files and proto files ---

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !["node_modules", ".git", "target", "vendor", ".venv"].contains(&name.as_ref())
        })
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();

        match ext {
            "proto" => {
                conn.execute(
                    "INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)",
                    params![
                        format!("schema:proto:{}", path.display()),
                        path.to_string_lossy().to_string()
                    ],
                )?;

                if let Err(e) = parse_proto(conn, path, service_id) {
                    tracing::warn!("Failed to parse proto file {}: {}", path.display(), e);
                }
            }
            "yaml" | "yml" | "json" => {
                // Check for additional OpenAPI files not in the root
                // (e.g. api/openapi.yaml, docs/swagger.json)
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_lowercase();

                let is_openapi = file_name.starts_with("openapi")
                    || file_name.starts_with("swagger")
                    || file_name.contains("openapi")
                    || file_name.contains("swagger");

                if is_openapi && path.parent() != Some(root) {
                    conn.execute(
                        "INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)",
                        params![
                            format!(
                                "schema:{}",
                                path.strip_prefix(root).unwrap_or(path).display()
                            ),
                            path.to_string_lossy().to_string()
                        ],
                    )?;

                    if let Err(e) = parse_openapi(conn, path, service_id) {
                        tracing::warn!("Failed to parse OpenAPI spec {}: {}", path.display(), e);
                    }
                }
            }
            _ => {}
        }
    }

    // --- Boost confidence: match api_calls to schema-derived endpoints ---

    boost_schema_matches(conn, service_id)?;

    Ok(())
}

/// Parse an OpenAPI spec file and insert endpoints into api_endpoints.
fn parse_openapi(conn: &rusqlite::Connection, path: &Path, service_id: i64) -> anyhow::Result<()> {
    let content = fs::read_to_string(path)?;

    // Parse the OpenAPI document (serde_yaml handles both YAML and JSON)
    let spec: openapiv3::OpenAPI = if path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        == "json"
    {
        serde_json::from_str(&content)?
    } else {
        serde_yaml::from_str(&content)?
    };

    // Look up the file_id for this schema file
    let file_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM files WHERE absolute_path = ?1",
            params![path.to_string_lossy().to_string()],
            |row| row.get(0),
        )
        .ok();

    // Iterate over all paths and methods
    for (path_str, path_item) in &spec.paths.paths {
        let path_item = match path_item {
            openapiv3::ReferenceOr::Item(item) => item,
            openapiv3::ReferenceOr::Reference { .. } => continue,
        };

        // Each HTTP method on this path
        let methods: Vec<(&str, Option<&openapiv3::Operation>)> = vec![
            ("GET", path_item.get.as_ref()),
            ("POST", path_item.post.as_ref()),
            ("PUT", path_item.put.as_ref()),
            ("DELETE", path_item.delete.as_ref()),
            ("PATCH", path_item.patch.as_ref()),
            ("HEAD", path_item.head.as_ref()),
            ("OPTIONS", path_item.options.as_ref()),
            ("TRACE", path_item.trace.as_ref()),
        ];

        for (method, operation) in methods {
            if let Some(op) = operation {
                // Try to find a handler symbol that matches this endpoint
                let handler_symbol_id = find_handler_symbol(conn, path_str, method, op);

                conn.execute(
                    "INSERT OR IGNORE INTO api_endpoints
                     (service_id, handler_symbol_id, method, path_pattern, protocol, file_id, schema_source)
                     VALUES (?1, ?2, ?3, ?4, 'http', ?5, 'openapi')",
                    params![service_id, handler_symbol_id, method, path_str, file_id],
                )?;
            }
        }
    }

    Ok(())
}

/// Try to find a handler symbol that matches an OpenAPI endpoint.
///
/// Looks for symbol names that contain the operation_id or that resemble
/// the path pattern (e.g., a function named `get_users` for `GET /users`).
fn find_handler_symbol(
    conn: &rusqlite::Connection,
    path_str: &str,
    method: &str,
    operation: &openapiv3::Operation,
) -> Option<i64> {
    // Strategy 1: Match by operationId
    if let Some(ref op_id) = operation.operation_id {
        let found: Option<i64> = conn
            .query_row(
                "SELECT id FROM symbols WHERE name = ?1 OR qualified_name LIKE '%' || ?1 LIMIT 1",
                params![op_id],
                |row| row.get(0),
            )
            .ok();
        if found.is_some() {
            return found;
        }
    }

    // Strategy 2: Build a candidate name from method + path segments
    // e.g., GET /users/{id} -> "get_users", "getUsers", "get_user"
    let path_clean = path_str
        .trim_start_matches('/')
        .replace('/', "_")
        .replace(['{', '}'], "");
    let candidate = format!("{}_{}", method.to_lowercase(), path_clean);

    let found: Option<i64> = conn
        .query_row(
            "SELECT id FROM symbols WHERE LOWER(name) = LOWER(?1) LIMIT 1",
            params![candidate],
            |row| row.get(0),
        )
        .ok();
    if found.is_some() {
        return found;
    }

    // Strategy 3: Look for the last path segment as a partial match
    let last_segment = path_str
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .replace(['{', '}'], "");
    if !last_segment.is_empty() {
        let pattern = format!(
            "%{}%{}%",
            method.to_lowercase(),
            last_segment.to_lowercase()
        );
        let found: Option<i64> = conn
            .query_row(
                "SELECT id FROM symbols WHERE LOWER(name) LIKE ?1
                 AND kind IN ('function', 'method')
                 LIMIT 1",
                params![pattern],
                |row| row.get(0),
            )
            .ok();
        if found.is_some() {
            return found;
        }
    }

    None
}

/// Parse a .proto file using line-by-line text scanning.
///
/// Extracts `service X { ... }` blocks and `rpc MethodName(Request) returns (Response)`
/// declarations, inserting them as gRPC endpoints.
fn parse_proto(conn: &rusqlite::Connection, path: &Path, service_id: i64) -> anyhow::Result<()> {
    let content = fs::read_to_string(path)?;

    // Look up the file_id for this proto file
    let file_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM files WHERE absolute_path = ?1",
            params![path.to_string_lossy().to_string()],
            |row| row.get(0),
        )
        .ok();

    let mut current_service: Option<String> = None;
    let mut brace_depth: i32 = 0;

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Detect `service ServiceName {`
        if trimmed.starts_with("service ") && trimmed.contains('{') {
            let after_service = trimmed.strip_prefix("service ").unwrap_or("").trim();
            // Extract the service name (everything before the `{`)
            let svc_name = after_service
                .split(|c: char| c == '{' || c.is_whitespace())
                .next()
                .unwrap_or("")
                .trim()
                .to_string();

            if !svc_name.is_empty() {
                current_service = Some(svc_name);
                brace_depth = 1;
            }
            continue;
        }

        // Track brace depth when inside a service block
        if current_service.is_some() {
            for ch in trimmed.chars() {
                match ch {
                    '{' => brace_depth += 1,
                    '}' => {
                        brace_depth -= 1;
                        if brace_depth <= 0 {
                            current_service = None;
                            brace_depth = 0;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Detect `rpc MethodName(` inside a service block
        if let Some(ref svc_name) = current_service {
            if trimmed.starts_with("rpc ") {
                let after_rpc = trimmed.strip_prefix("rpc ").unwrap_or("").trim();
                // Extract method name (everything before the `(`)
                let rpc_name = after_rpc.split('(').next().unwrap_or("").trim().to_string();

                if !rpc_name.is_empty() {
                    // gRPC path convention: /package.ServiceName/MethodName
                    let grpc_path = format!("/{}/{}", svc_name, rpc_name);

                    // Try to find a handler symbol matching the rpc method name
                    let handler_symbol_id: Option<i64> = conn
                        .query_row(
                            "SELECT id FROM symbols WHERE name = ?1 OR qualified_name LIKE '%' || ?1 LIMIT 1",
                            params![rpc_name],
                            |row| row.get(0),
                        )
                        .ok();

                    conn.execute(
                        "INSERT OR IGNORE INTO api_endpoints
                         (service_id, handler_symbol_id, method, path_pattern, protocol, file_id, line, schema_source)
                         VALUES (?1, ?2, 'POST', ?3, 'grpc', ?4, ?5, 'proto')",
                        params![
                            service_id,
                            handler_symbol_id,
                            grpc_path,
                            file_id,
                            (line_num + 1) as i64,
                        ],
                    )?;
                }
            }
        }
    }

    Ok(())
}

/// Boost resolution: match existing api_calls to newly-inserted schema endpoints.
///
/// For each unresolved api_call, check if any schema-derived endpoint matches
/// by method and path pattern. If so, set `resolved_endpoint_id` and `resolved_service_id`.
fn boost_schema_matches(conn: &rusqlite::Connection, service_id: i64) -> anyhow::Result<()> {
    // Get all schema-derived endpoints
    let mut ep_stmt = conn.prepare(
        "SELECT id, method, path_pattern, protocol FROM api_endpoints WHERE schema_source IS NOT NULL",
    )?;

    let endpoints: Vec<(i64, String, String, String)> = ep_stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Get all unresolved api_calls
    let mut call_stmt = conn.prepare(
        "SELECT id, method, url_pattern, protocol FROM api_calls WHERE resolved_endpoint_id IS NULL",
    )?;

    let calls: Vec<(i64, String, String, String)> = call_stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (call_id, call_method, call_url, call_protocol) in &calls {
        for (ep_id, ep_method, ep_path, ep_protocol) in &endpoints {
            // Method must match (case-insensitive)
            if !call_method.eq_ignore_ascii_case(ep_method) {
                continue;
            }

            // Protocol must be compatible
            if call_protocol != ep_protocol && ep_protocol != "http" {
                continue;
            }

            // Check if the URL pattern contains the endpoint path
            // e.g., "https://api.example.com/users/{id}" contains "/users/{id}"
            if url_matches_path(call_url, ep_path) {
                conn.execute(
                    "UPDATE api_calls SET resolved_endpoint_id = ?1, resolved_service_id = ?2 WHERE id = ?3",
                    params![ep_id, service_id, call_id],
                )?;

                // Also update any corresponding service_edges confidence
                conn.execute(
                    "UPDATE service_edges SET confidence = MAX(confidence, 0.9)
                     WHERE to_service_id = ?1 AND protocol = ?2",
                    params![service_id, ep_protocol],
                )?;

                break; // One match per call is sufficient
            }
        }
    }

    Ok(())
}

/// Check if a URL pattern matches an endpoint path pattern.
///
/// Handles cases like:
/// - Exact suffix match: "https://api.example.com/users" matches "/users"
/// - Path parameter normalization: "/users/123" matches "/users/{id}"
fn url_matches_path(url: &str, endpoint_path: &str) -> bool {
    // Direct suffix match (most common case)
    if url.ends_with(endpoint_path) || url.contains(endpoint_path) {
        return true;
    }

    // Normalize path parameters in the endpoint: /users/{id} -> /users/
    // and check if the URL starts with the static prefix
    let static_prefix: String = endpoint_path
        .split('/')
        .filter(|seg| !seg.starts_with('{'))
        .collect::<Vec<_>>()
        .join("/");

    if !static_prefix.is_empty() && static_prefix != "/" {
        // Check if URL contains all the static segments in order
        let url_lower = url.to_lowercase();
        let prefix_lower = static_prefix.to_lowercase();
        if url_lower.contains(&prefix_lower) {
            return true;
        }
    }

    false
}
