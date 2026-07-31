use rmcp::model::*;

use crate::analysis::health::{
    compute_grade, generate_summary, metric_score_coupling, metric_score_cycles,
    metric_score_dead_code, metric_score_modularity,
};
use crate::db::query;
use crate::graph;

use super::AriadneService;

impl AriadneService {
    pub(crate) fn tool_get_codebase_health(&self) -> CallToolResult {
        let start = std::time::Instant::now();
        let mut degraded_fields = Vec::new();
        let mut scores = Vec::new();

        // 1. Dead code ratio
        let dead_code_ratio: Option<f64> = match self.with_db(|db| {
            let dead = query::count_dead(db)?;
            let total: i64 = db
                .conn()
                .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?;
            Ok::<_, anyhow::Error>(if total > 0 {
                dead as f64 / total as f64
            } else {
                0.0
            })
        }) {
            Ok(Ok(ratio)) => {
                scores.push(metric_score_dead_code(ratio));
                Some((ratio * 1000.0).round() / 1000.0)
            }
            _ => {
                degraded_fields.push("dead_code_ratio".to_string());
                None
            }
        };

        // 2. Cycle count (uses cached graph)
        let cycle_count: Option<usize> = match self
            .with_cached_graph(|g| Ok(graph::circular::detect_circular_dependencies(g).len()))
        {
            Ok(count) => {
                scores.push(metric_score_cycles(count));
                Some(count)
            }
            _ => {
                degraded_fields.push("cycle_count".to_string());
                None
            }
        };

        // 3. Coupling density (average strength of top 50 pairs)
        let coupling_density: Option<f64> =
            match self.with_db(|db| query::get_top_couplings(db, 50)) {
                Ok(Ok(rows)) if !rows.is_empty() => {
                    let avg = rows.iter().map(|r| r.strength).sum::<f64>() / rows.len() as f64;
                    let avg = (avg * 1000.0).round() / 1000.0;
                    scores.push(metric_score_coupling(avg));
                    Some(avg)
                }
                Ok(Ok(_)) => {
                    scores.push(100.0); // No coupling data = no coupling problems
                    Some(0.0)
                }
                _ => {
                    degraded_fields.push("coupling_density".to_string());
                    None
                }
            };

        // 4. Modularity score (average across communities)
        let modularity_score: Option<f64> = match self.with_db(query::get_communities) {
            Ok(Ok(rows)) if !rows.is_empty() => {
                let avg = rows.iter().map(|c| c.modularity).sum::<f64>() / rows.len() as f64;
                let avg = (avg * 1000.0).round() / 1000.0;
                scores.push(metric_score_modularity(avg));
                Some(avg)
            }
            Ok(Ok(_)) => {
                // No communities detected — neutral score
                Some(0.0)
            }
            _ => {
                degraded_fields.push("modularity_score".to_string());
                None
            }
        };

        let grade = compute_grade(&scores);
        let summary = generate_summary(
            grade,
            dead_code_ratio,
            cycle_count,
            coupling_density,
            modularity_score,
        );

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let result = serde_json::json!({
            "grade": grade,
            "dead_code_ratio": dead_code_ratio,
            "cycle_count": cycle_count,
            "coupling_density": coupling_density,
            "modularity_score": modularity_score,
            "summary": summary,
            "elapsed_ms": elapsed_ms,
            "degraded_fields": degraded_fields,
        });

        let json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
        CallToolResult::success(vec![Content::text(json)])
    }
}
