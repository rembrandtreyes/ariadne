use crate::db::Database;
use rusqlite::params;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct BoundaryAnalysis {
    pub modules: Vec<ModuleBoundary>,
    pub cross_boundary_calls: Vec<CrossBoundaryCall>,
    pub total_modules: usize,
    pub avg_modularity: f64,
    pub boundary_violations: usize,
}

#[derive(Debug, Serialize)]
pub struct ModuleBoundary {
    pub name: String,
    pub symbol_count: i32,
    pub internal_calls: i32,
    pub external_calls: i32,
    pub modularity: f64,
    pub top_files: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CrossBoundaryCall {
    pub from_module: String,
    pub to_module: String,
    pub call_count: i32,
    pub symbols: Vec<String>,
}

pub fn analyze_boundaries(db: &Database) -> anyhow::Result<BoundaryAnalysis> {
    let conn = db.conn();

    // 1. Query communities for module data
    let mut comm_stmt = conn.prepare(
        "SELECT id, name, symbol_count, internal_edges, external_edges, modularity
         FROM communities ORDER BY symbol_count DESC",
    )?;

    struct RawCommunity {
        id: i64,
        name: String,
        symbol_count: i32,
        internal_edges: i32,
        external_edges: i32,
        modularity: f64,
    }

    let raw_communities: Vec<RawCommunity> = comm_stmt
        .query_map([], |row| {
            Ok(RawCommunity {
                id: row.get(0)?,
                name: row.get(1)?,
                symbol_count: row.get(2)?,
                internal_edges: row.get(3)?,
                external_edges: row.get(4)?,
                modularity: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if raw_communities.is_empty() {
        return Ok(BoundaryAnalysis {
            modules: Vec::new(),
            cross_boundary_calls: Vec::new(),
            total_modules: 0,
            avg_modularity: 0.0,
            boundary_violations: 0,
        });
    }

    // 2. Get top 5 files per community by symbol count
    let mut file_stmt = conn.prepare(
        "SELECT f.path, COUNT(s.id) as sym_count
         FROM symbols s
         JOIN files f ON s.file_id = f.id
         WHERE s.community_id = ?1
         GROUP BY f.id
         ORDER BY sym_count DESC
         LIMIT 5",
    )?;

    let mut modules: Vec<ModuleBoundary> = Vec::new();
    let mut modularity_sum = 0.0;
    let mut boundary_violations = 0usize;

    for comm in &raw_communities {
        let top_files: Vec<String> = file_stmt
            .query_map(params![comm.id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        // A boundary violation: external edges exceed internal edges
        if comm.external_edges > comm.internal_edges {
            boundary_violations += 1;
        }

        modularity_sum += comm.modularity;

        modules.push(ModuleBoundary {
            name: comm.name.clone(),
            symbol_count: comm.symbol_count,
            internal_calls: comm.internal_edges,
            external_calls: comm.external_edges,
            modularity: comm.modularity,
            top_files,
        });
    }

    let total_modules = raw_communities.len();
    let avg_modularity = if total_modules > 0 {
        modularity_sum / total_modules as f64
    } else {
        0.0
    };

    // 3. Query cross-community calls by joining calls -> symbols -> communities
    //    Find calls where caller and callee belong to different communities.
    let mut cross_stmt = conn.prepare(
        "SELECT
            c_from.name AS from_module,
            c_to.name AS to_module,
            COUNT(*) AS call_count,
            GROUP_CONCAT(DISTINCT s_callee.name) AS symbol_names
         FROM calls ca
         JOIN symbols s_caller ON ca.caller_symbol_id = s_caller.id
         JOIN symbols s_callee ON ca.callee_symbol_id = s_callee.id
         JOIN communities c_from ON s_caller.community_id = c_from.id
         JOIN communities c_to ON s_callee.community_id = c_to.id
         WHERE s_caller.community_id != s_callee.community_id
           AND ca.callee_symbol_id IS NOT NULL
         GROUP BY s_caller.community_id, s_callee.community_id
         ORDER BY call_count DESC",
    )?;

    let cross_boundary_calls: Vec<CrossBoundaryCall> = cross_stmt
        .query_map([], |row| {
            let symbol_str: String = row.get(3)?;
            let symbols: Vec<String> = symbol_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            Ok(CrossBoundaryCall {
                from_module: row.get(0)?,
                to_module: row.get(1)?,
                call_count: row.get(2)?,
                symbols,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(BoundaryAnalysis {
        modules,
        cross_boundary_calls,
        total_modules,
        avg_modularity,
        boundary_violations,
    })
}
