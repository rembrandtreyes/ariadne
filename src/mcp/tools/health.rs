use rmcp::model::*;

use crate::db::query;
use crate::graph;

use super::AriadneService;

/// Grade thresholds: each metric maps to a 0–100 score via threshold bands.
/// Overall grade = average of available metric scores.
/// A: >=85, B: >=70, C: >=55, D: >=40, F: <40
const GRADE_A: f64 = 85.0;
const GRADE_B: f64 = 70.0;
const GRADE_C: f64 = 55.0;
const GRADE_D: f64 = 40.0;

/// Dead code ratio thresholds: <5% excellent, <15% acceptable, >30% poor.
const DEAD_CODE_EXCELLENT: f64 = 0.05;
const DEAD_CODE_ACCEPTABLE: f64 = 0.15;

/// Coupling density thresholds (average strength of top pairs).
const COUPLING_LOW: f64 = 0.3;
const COUPLING_HIGH: f64 = 0.6;

fn metric_score_dead_code(ratio: f64) -> f64 {
    if ratio <= DEAD_CODE_EXCELLENT {
        100.0
    } else if ratio <= DEAD_CODE_ACCEPTABLE {
        70.0
    } else {
        30.0
    }
}

fn metric_score_cycles(count: usize) -> f64 {
    match count {
        0 => 100.0,
        1..=2 => 70.0,
        3..=5 => 50.0,
        _ => 20.0,
    }
}

fn metric_score_coupling(density: f64) -> f64 {
    if density <= COUPLING_LOW {
        100.0
    } else if density <= COUPLING_HIGH {
        60.0
    } else {
        25.0
    }
}

fn metric_score_modularity(score: f64) -> f64 {
    // Modularity scores range 0.0–1.0; higher is better.
    (score * 100.0).clamp(0.0, 100.0)
}

fn compute_grade(scores: &[f64]) -> &'static str {
    if scores.is_empty() {
        return "C"; // No data defaults to middle grade
    }
    let avg = scores.iter().sum::<f64>() / scores.len() as f64;
    if avg >= GRADE_A {
        "A"
    } else if avg >= GRADE_B {
        "B"
    } else if avg >= GRADE_C {
        "C"
    } else if avg >= GRADE_D {
        "D"
    } else {
        "F"
    }
}

fn generate_summary(
    grade: &str,
    dead_code_ratio: Option<f64>,
    cycle_count: Option<usize>,
    coupling_density: Option<f64>,
    modularity_score: Option<f64>,
) -> String {
    let mut concerns = Vec::new();
    if let Some(r) = dead_code_ratio {
        if r > DEAD_CODE_ACCEPTABLE {
            concerns.push(format!("high dead code ({:.0}%)", r * 100.0));
        }
    }
    if let Some(c) = cycle_count {
        if c > 2 {
            concerns.push(format!("{c} circular dependencies"));
        }
    }
    if let Some(d) = coupling_density {
        if d > COUPLING_HIGH {
            concerns.push(format!("tight coupling (avg {d:.2})"));
        }
    }
    if let Some(m) = modularity_score {
        if m < 0.4 {
            concerns.push(format!("low modularity ({m:.2})"));
        }
    }

    if concerns.is_empty() {
        format!("This codebase scores a {grade} with no major concerns detected.")
    } else {
        format!(
            "This codebase scores a {grade}. Key concerns: {}.",
            concerns.join(", ")
        )
    }
}

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
