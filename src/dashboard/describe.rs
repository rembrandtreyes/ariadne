use crate::db::{query, Database};
use serde::Serialize;

/// Level C description result -- full narrative with architectural context.
#[derive(Debug, Serialize)]
pub struct DescribeResult {
    pub description: String,
    pub role: String,
    pub risk_level: String,
    pub risk_score: f64,
    pub metrics: DescribeMetrics,
}

#[derive(Debug, Serialize)]
pub struct DescribeMetrics {
    pub fan_in: i64,
    pub fan_out: i64,
    pub modification_count: i64,
    pub author_count: i64,
    pub is_volatile: bool,
    pub blast_radius: usize,
    pub coupled_file_count: usize,
    pub max_coupling_strength: f64,
}

/// Generate a Level C narrative description for a symbol.
///
/// Composes a natural-language explanation from structural signals:
/// callers, callees, fan-in/out, churn, coupling, blast radius, dead code status.
pub fn describe_symbol(db: &Database, symbol_id: i64) -> anyhow::Result<DescribeResult> {
    let sym = query::symbol_by_id(db, symbol_id)?
        .ok_or_else(|| anyhow::anyhow!("Symbol not found: {}", symbol_id))?;

    let file_path = query::file_path_by_id(db, sym.file_id)?;
    let callers = query::get_dependents(db, sym.id)?;
    let callees = query::get_dependencies(db, sym.id)?;
    let couplings = query::get_file_couplings(db, sym.file_id)?;

    // Blast radius counts every transitive caller. Graceful on graph-build failure
    // so a describe request never crashes on a partially-indexed DB.
    let blast_radius = query::build_call_graph(db, None)
        .map(|graph| {
            crate::graph::blast_radius::analyze_blast_radius(&graph, sym.id as u64, Some(10), false)
                .total_affected
        })
        .unwrap_or(0);

    // Get health data if available — by resolved symbol, so a bare-name
    // collision elsewhere in the index can't swap in another symbol's health.
    let health = query::get_symbol_health_data_for(db, &sym).ok();

    let fan_in = health
        .as_ref()
        .map(|h| h.fan_in)
        .unwrap_or(callers.len() as i64);
    let fan_out = health
        .as_ref()
        .map(|h| h.fan_out)
        .unwrap_or(callees.len() as i64);
    let modification_count = health.as_ref().map(|h| h.modification_count).unwrap_or(0);
    let author_count = health.as_ref().map(|h| h.author_count).unwrap_or(0);
    let is_volatile = health.as_ref().map(|h| h.is_volatile).unwrap_or(false);
    let is_dead = sym.is_dead;

    let coupled_file_count = couplings.len();
    let max_coupling_strength = couplings.iter().map(|c| c.strength).fold(0.0_f64, f64::max);

    // Compute a simple risk score
    let fan_in_score = (fan_in as f64 / 20.0).min(1.0);
    let churn_score = if is_volatile {
        0.8
    } else {
        (modification_count as f64 / 30.0).min(1.0)
    };
    let coupling_score = max_coupling_strength;
    let dead_score = if is_dead { 0.5 } else { 0.0 };
    let risk_score =
        (fan_in_score * 0.3 + churn_score * 0.3 + coupling_score * 0.2 + dead_score * 0.2).min(1.0);

    // Determine role from file path and kind
    let module_name = extract_module(&file_path);
    let role = infer_role(&sym.kind, &module_name, fan_in, is_dead);

    // Determine risk level
    let risk_level = if risk_score >= 0.8 {
        "critical"
    } else if risk_score >= 0.6 {
        "high"
    } else if risk_score >= 0.4 {
        "medium"
    } else {
        "low"
    };

    // Build the description
    let description = build_narrative(
        &sym.name,
        &sym.kind,
        &module_name,
        &file_path,
        &callers,
        &callees,
        fan_in,
        fan_out,
        modification_count,
        is_volatile,
        is_dead,
        &couplings,
        risk_score,
    );

    Ok(DescribeResult {
        description,
        role,
        risk_level: risk_level.to_string(),
        risk_score,
        metrics: DescribeMetrics {
            fan_in,
            fan_out,
            modification_count,
            author_count,
            is_volatile,
            blast_radius,
            coupled_file_count,
            max_coupling_strength,
        },
    })
}

fn extract_module(file_path: &str) -> String {
    let path = file_path.strip_prefix("src/").unwrap_or(file_path);
    match path.split('/').next() {
        Some(first) if path.contains('/') => first.to_string(),
        _ => "root".to_string(),
    }
}

fn infer_role(kind: &str, module: &str, fan_in: i64, is_dead: bool) -> String {
    if is_dead {
        return "unreachable".to_string();
    }
    if fan_in == 0 {
        return "entry_point".to_string();
    }
    match module {
        "pipeline" => "core_pipeline".to_string(),
        "parse" => "parser".to_string(),
        "db" => "data_access".to_string(),
        "graph" => "graph_analysis".to_string(),
        "mcp" => "mcp_tool".to_string(),
        "analysis" => "analysis".to_string(),
        "dashboard" => "dashboard_api".to_string(),
        "search" => "search".to_string(),
        _ => format!("{}_{}", module, kind),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_narrative(
    name: &str,
    kind: &str,
    module: &str,
    file_path: &str,
    callers: &[query::SymbolRow],
    callees: &[query::SymbolRow],
    fan_in: i64,
    fan_out: i64,
    modification_count: i64,
    is_volatile: bool,
    is_dead: bool,
    couplings: &[query::CouplingRow],
    risk_score: f64,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    let kind_label = match kind {
        "function" => "function",
        "method" => "method",
        "class" => "class",
        "interface" => "interface",
        _ => "symbol",
    };

    if is_dead {
        parts.push(format!(
            "{} is an unreachable {} in {} ({}). No code path leads to it -- safe to remove.",
            name, kind_label, module, file_path
        ));
        return parts.join(" ");
    }

    parts.push(format!(
        "{} is a {} in the {} module ({}).",
        name, kind_label, module, file_path
    ));

    // Callers context
    if !callers.is_empty() {
        let caller_names: Vec<&str> = callers.iter().take(3).map(|c| c.name.as_str()).collect();
        if callers.len() <= 3 {
            parts.push(format!("It is called by {}.", caller_names.join(", ")));
        } else {
            parts.push(format!(
                "It is called by {} and {} others ({} total callers).",
                caller_names.join(", "),
                callers.len() - 3,
                callers.len()
            ));
        }
    } else {
        parts.push("It has no known callers -- it may be an entry point or unused.".to_string());
    }

    // Callees context
    if !callees.is_empty() {
        let callee_names: Vec<&str> = callees.iter().take(3).map(|c| c.name.as_str()).collect();
        if callees.len() <= 3 {
            parts.push(format!("It depends on {}.", callee_names.join(", ")));
        } else {
            parts.push(format!(
                "It depends on {} and {} others.",
                callee_names.join(", "),
                callees.len() - 3
            ));
        }
    }

    // Risk assessment
    if risk_score >= 0.8 {
        let mut risk_reasons = Vec::new();
        if fan_in > 15 {
            risk_reasons.push(format!("{} incoming dependencies", fan_in));
        }
        if is_volatile || modification_count > 20 {
            risk_reasons.push("high modification frequency".to_string());
        }
        if !couplings.is_empty() {
            risk_reasons.push(format!("coupled with {} other files", couplings.len()));
        }
        if !risk_reasons.is_empty() {
            parts.push(format!(
                "This is a critical risk point: {}.",
                risk_reasons.join(", ")
            ));
        }
    } else if risk_score >= 0.5 {
        parts.push(format!(
            "With {} callers and {} callees, this is a moderately connected symbol.",
            fan_in, fan_out
        ));
    }

    // Coupling context
    if let Some(strongest) = couplings.first() {
        if strongest.strength > 0.7 {
            parts.push(format!(
                "Tightly coupled with {} (strength {:.2}) -- changes to one often require changes to the other.",
                strongest.coupled_path, strongest.strength
            ));
        }
    }

    parts.join(" ")
}
