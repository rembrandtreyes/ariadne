use console::style;

use super::{require_db, resolve_symbol};

pub fn cmd_blast_radius(
    symbol: &str,
    cross_service: bool,
    depth: u32,
    json: bool,
) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let sym = resolve_symbol(&db, symbol)?;

    let graph = crate::db::query::build_call_graph(&db, None)?;
    let result = if cross_service {
        crate::graph::blast_radius::analyze_blast_radius_cross_service(
            &graph,
            &db,
            sym.id as u64,
            Some(depth),
        )
    } else {
        crate::graph::blast_radius::analyze_blast_radius(&graph, sym.id as u64, Some(depth), false)
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    if result.total_affected == 0 {
        println!("No dependents found for \"{}\"", symbol);
        return Ok(());
    }

    println!(
        "\n{} Blast radius for {}: {} symbols affected\n",
        style("⚡").bold(),
        style(symbol).cyan().bold(),
        style(result.total_affected).yellow()
    );

    if !result.direct_dependents.is_empty() {
        println!(
            "  {} WILL BREAK ({}):",
            style("■").red(),
            result.direct_dependents.len()
        );
        for dep in &result.direct_dependents {
            println!(
                "    {} {} ({})",
                style(&dep.name).bold(),
                style(&dep.file_path).dim(),
                style(&dep.kind).dim()
            );
        }
    }

    if !result.transitive_dependents.is_empty() {
        println!(
            "\n  {} MAY BREAK ({}):",
            style("■").yellow(),
            result.transitive_dependents.len()
        );
        for dep in &result.transitive_dependents {
            println!(
                "    {} {} (depth {})",
                style(&dep.name).bold(),
                style(&dep.file_path).dim(),
                dep.depth
            );
        }
    }

    if !result.affected_files.is_empty() {
        println!(
            "\n  {} files affected: {}",
            style(result.affected_files.len()).cyan(),
            result.affected_files.join(", ")
        );
    }

    if !result.affected_services.is_empty() {
        println!(
            "\n  {} services in blast radius: {}",
            style(result.affected_services.len()).cyan(),
            result.affected_services.join(" → ")
        );
    }

    Ok(())
}

pub fn cmd_why(symbol: &str, json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let sym = resolve_symbol(&db, symbol)?;

    let file_path =
        crate::db::query::file_path_by_id(&db, sym.file_id).unwrap_or_else(|_| "unknown".into());

    let callers = crate::db::query::get_dependents(&db, sym.id).unwrap_or_default();
    let callees = crate::db::query::get_dependencies(&db, sym.id).unwrap_or_default();

    let graph = crate::db::query::build_call_graph(&db, None)?;
    let blast =
        crate::graph::blast_radius::analyze_blast_radius(&graph, sym.id as u64, Some(10), false);

    if json {
        let output = serde_json::json!({
            "symbol": {
                "name": sym.name,
                "qualified_name": sym.qualified_name,
                "kind": sym.kind,
                "file": file_path,
                "line_start": sym.line_start,
                "line_end": sym.line_end,
                "is_dead": sym.is_dead,
                "is_test": sym.is_test,
            },
            "callers": callers.iter().map(|c| serde_json::json!({
                "name": c.name, "qualified_name": c.qualified_name, "kind": c.kind,
            })).collect::<Vec<_>>(),
            "callees": callees.iter().map(|c| serde_json::json!({
                "name": c.name, "qualified_name": c.qualified_name, "kind": c.kind,
            })).collect::<Vec<_>>(),
            "blast_radius": {
                "total_affected": blast.total_affected,
                "direct": blast.direct_dependents.len(),
                "transitive": blast.transitive_dependents.len(),
                "affected_files": blast.affected_files,
            },
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    let dead_marker = if sym.is_dead {
        format!(" {}", style("DEAD CODE").red().bold())
    } else {
        String::new()
    };
    let test_marker = if sym.is_test {
        format!(" {}", style("[test]").dim())
    } else {
        String::new()
    };

    println!(
        "\n{} {} {}{}{}\n   {} {}:{}-{}\n",
        style("⚡").bold(),
        style(&sym.qualified_name).cyan().bold(),
        style(&sym.kind).dim(),
        dead_marker,
        test_marker,
        style("→").dim(),
        style(&file_path).dim(),
        sym.line_start,
        sym.line_end,
    );

    if callers.is_empty() {
        println!(
            "  {} No callers (entry point or unused)",
            style("↑").green()
        );
    } else {
        println!(
            "  {} {} {}:\n",
            style("↑").green(),
            style(callers.len()).green().bold(),
            if callers.len() == 1 {
                "caller"
            } else {
                "callers"
            }
        );
        for c in &callers {
            let caller_file =
                crate::db::query::file_path_by_id(&db, c.file_id).unwrap_or_else(|_| "?".into());
            println!(
                "    {} {} ({}:{})",
                style("·").dim(),
                style(&c.name).bold(),
                style(&caller_file).dim(),
                c.line_start,
            );
        }
    }

    println!();

    if callees.is_empty() {
        println!("  {} No callees (leaf function)", style("↓").blue());
    } else {
        println!(
            "  {} {} {}:\n",
            style("↓").blue(),
            style(callees.len()).blue().bold(),
            if callees.len() == 1 {
                "callee"
            } else {
                "callees"
            }
        );
        for c in &callees {
            let callee_file =
                crate::db::query::file_path_by_id(&db, c.file_id).unwrap_or_else(|_| "?".into());
            println!(
                "    {} {} ({}:{})",
                style("·").dim(),
                style(&c.name).bold(),
                style(&callee_file).dim(),
                c.line_start,
            );
        }
    }

    println!();

    if blast.total_affected == 0 {
        println!(
            "  {} Blast radius: {} — changes here affect nothing else",
            style("◉").dim(),
            style("0").green(),
        );
    } else {
        println!(
            "  {} Blast radius: {} symbols across {} files",
            style("◉").yellow(),
            style(blast.total_affected).yellow().bold(),
            style(blast.affected_files.len()).yellow(),
        );
        if !blast.direct_dependents.is_empty() {
            println!(
                "    {} {} direct (will break)",
                style("■").red(),
                blast.direct_dependents.len(),
            );
        }
        if !blast.transitive_dependents.is_empty() {
            println!(
                "    {} {} transitive (may break)",
                style("■").yellow(),
                blast.transitive_dependents.len(),
            );
        }
    }

    println!();

    Ok(())
}

pub fn cmd_call_chain(symbol: &str, cross_service: bool, json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let sym = resolve_symbol(&db, symbol)?;

    let graph = crate::db::query::build_call_graph(&db, None)?;
    let mermaid = crate::graph::call_chain::extract_call_chain(&graph, sym.id, cross_service);

    if json {
        println!("{}", serde_json::json!({ "mermaid": mermaid }));
    } else {
        println!("{}", mermaid);
    }

    Ok(())
}
