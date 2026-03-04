use crate::db::Database;
use rusqlite::params;

/// Phase 11: Resolve API boundaries between services.
///
/// Matches API call sites (fetch, requests.get, etc.) to known
/// API endpoints based on URL pattern matching.
pub fn resolve_api_boundaries(db: &Database) -> anyhow::Result<()> {
    let conn = db.conn();

    // Get all unresolved API calls
    let mut call_stmt = conn.prepare(
        "SELECT id, method, url_pattern, service_id FROM api_calls WHERE resolved_endpoint_id IS NULL",
    )?;

    let calls: Vec<(i64, String, String, i64)> = call_stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (call_id, method, url_pattern, _source_service_id) in &calls {
        // Try to match the URL pattern to a known endpoint
        let matched: Option<(i64, i64)> = conn
            .query_row(
                "SELECT id, service_id FROM api_endpoints
                 WHERE method = ?1 AND ?2 LIKE '%' || path_pattern || '%'
                 LIMIT 1",
                params![method, url_pattern],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        if let Some((endpoint_id, target_service_id)) = matched {
            conn.execute(
                "UPDATE api_calls SET resolved_endpoint_id = ?1, resolved_service_id = ?2 WHERE id = ?3",
                params![endpoint_id, target_service_id, call_id],
            )?;
        }
    }

    Ok(())
}
