use crate::db::Database;

/// Phase 12: Build the service-level dependency topology.
///
/// Aggregates resolved API calls and imports to create service-to-service
/// edges, representing the high-level architecture of a multi-service system.
pub fn build_topology(db: &Database) -> anyhow::Result<()> {
    let conn = db.conn();

    // Clear existing service edges for a clean rebuild
    conn.execute("DELETE FROM service_edges", [])?;

    // Build edges from resolved API calls
    let mut stmt = conn.prepare(
        "SELECT ac.service_id, ac.resolved_service_id, COUNT(*)
         FROM api_calls ac
         WHERE ac.resolved_service_id IS NOT NULL
           AND ac.service_id != ac.resolved_service_id
         GROUP BY ac.service_id, ac.resolved_service_id",
    )?;

    let edges: Vec<(i64, i64, i32)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    for (from_id, to_id, call_count) in &edges {
        let confidence = if *call_count > 5 { 0.9 } else { 0.6 };
        crate::db::write::insert_service_edge(
            db,
            *from_id,
            *to_id,
            "http",
            *call_count,
            confidence,
        )?;
    }

    Ok(())
}
