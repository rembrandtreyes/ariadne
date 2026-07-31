//! Codebase health scoring shared by the MCP `get_codebase_health` tool and
//! the dashboard `/api/overview` endpoint — one grading model, two transports.

use serde::Serialize;

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

pub fn metric_score_dead_code(ratio: f64) -> f64 {
    if ratio <= DEAD_CODE_EXCELLENT {
        100.0
    } else if ratio <= DEAD_CODE_ACCEPTABLE {
        70.0
    } else {
        30.0
    }
}

pub fn metric_score_cycles(count: usize) -> f64 {
    match count {
        0 => 100.0,
        1..=2 => 70.0,
        3..=5 => 50.0,
        _ => 20.0,
    }
}

pub fn metric_score_coupling(density: f64) -> f64 {
    if density <= COUPLING_LOW {
        100.0
    } else if density <= COUPLING_HIGH {
        60.0
    } else {
        25.0
    }
}

pub fn metric_score_modularity(score: f64) -> f64 {
    // Modularity scores range 0.0–1.0; higher is better.
    (score * 100.0).clamp(0.0, 100.0)
}

/// Numeric overall score: average of the available metric scores, or None
/// when no metric could be computed.
pub fn compute_score(scores: &[f64]) -> Option<f64> {
    if scores.is_empty() {
        None
    } else {
        Some(scores.iter().sum::<f64>() / scores.len() as f64)
    }
}

pub fn compute_grade(scores: &[f64]) -> &'static str {
    let avg = match compute_score(scores) {
        None => return "C", // No data defaults to middle grade
        Some(avg) => avg,
    };
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

pub fn generate_summary(
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

/// Full health report computed from a database: raw metric values, per-metric
/// scores, overall grade + numeric score, and which metrics degraded.
#[derive(Debug, Clone, Serialize)]
pub struct HealthReport {
    pub grade: String,
    /// 0–100 average of the available metric scores; None when nothing computed.
    pub score: Option<f64>,
    pub dead_code_ratio: Option<f64>,
    pub cycle_count: Option<usize>,
    pub coupling_density: Option<f64>,
    pub modularity_score: Option<f64>,
    pub metric_scores: MetricScores,
    pub summary: String,
    pub degraded_fields: Vec<String>,
}

/// Per-metric 0–100 scores backing the overall grade, so UIs can show the
/// breakdown instead of re-deriving it client-side.
#[derive(Debug, Clone, Serialize)]
pub struct MetricScores {
    pub dead_code: Option<f64>,
    pub cycles: Option<f64>,
    pub coupling: Option<f64>,
    pub modularity: Option<f64>,
}

/// Compute the health report against an open database. Each metric degrades
/// independently: a failed query drops that metric rather than failing the report.
pub fn compute_health_report(db: &crate::db::Database) -> HealthReport {
    let mut degraded_fields = Vec::new();
    let mut scores = Vec::new();

    // 1. Dead code ratio
    let (dead_code_ratio, dead_code_score) = match (|| {
        let dead = crate::db::query::count_dead(db)?;
        let total: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?;
        Ok::<_, anyhow::Error>(if total > 0 {
            dead as f64 / total as f64
        } else {
            0.0
        })
    })() {
        Ok(ratio) => {
            let s = metric_score_dead_code(ratio);
            scores.push(s);
            (Some((ratio * 1000.0).round() / 1000.0), Some(s))
        }
        Err(_) => {
            degraded_fields.push("dead_code_ratio".to_string());
            (None, None)
        }
    };

    // 2. Cycle count (engine SCC detection, length >= 2)
    let (cycle_count, cycle_score) = match crate::db::query::build_call_graph(db, None) {
        Ok(graph) => {
            let count = crate::graph::circular::detect_circular_dependencies(&graph).len();
            let s = metric_score_cycles(count);
            scores.push(s);
            (Some(count), Some(s))
        }
        Err(_) => {
            degraded_fields.push("cycle_count".to_string());
            (None, None)
        }
    };

    // 3. Coupling density (average strength of top 50 pairs)
    let (coupling_density, coupling_score) = match crate::db::query::get_top_couplings(db, 50) {
        Ok(rows) if !rows.is_empty() => {
            let avg = rows.iter().map(|r| r.strength).sum::<f64>() / rows.len() as f64;
            let avg = (avg * 1000.0).round() / 1000.0;
            let s = metric_score_coupling(avg);
            scores.push(s);
            (Some(avg), Some(s))
        }
        Ok(_) => {
            // No coupling data = no coupling problems
            scores.push(100.0);
            (Some(0.0), Some(100.0))
        }
        Err(_) => {
            degraded_fields.push("coupling_density".to_string());
            (None, None)
        }
    };

    // 4. Modularity score (average across communities)
    let (modularity_score, modularity_metric_score) = match crate::db::query::get_communities(db) {
        Ok(rows) if !rows.is_empty() => {
            let avg = rows.iter().map(|c| c.modularity).sum::<f64>() / rows.len() as f64;
            let avg = (avg * 1000.0).round() / 1000.0;
            let s = metric_score_modularity(avg);
            scores.push(s);
            (Some(avg), Some(s))
        }
        Ok(_) => {
            // No communities detected — neutral value, no score contribution
            (Some(0.0), None)
        }
        Err(_) => {
            degraded_fields.push("modularity_score".to_string());
            (None, None)
        }
    };

    let grade = compute_grade(&scores);
    let score = compute_score(&scores).map(|s| (s * 10.0).round() / 10.0);
    let summary = generate_summary(
        grade,
        dead_code_ratio,
        cycle_count,
        coupling_density,
        modularity_score,
    );

    HealthReport {
        grade: grade.to_string(),
        score,
        dead_code_ratio,
        cycle_count,
        coupling_density,
        modularity_score,
        metric_scores: MetricScores {
            dead_code: dead_code_score,
            cycles: cycle_score,
            coupling: coupling_score,
            modularity: modularity_metric_score,
        },
        summary,
        degraded_fields,
    }
}
