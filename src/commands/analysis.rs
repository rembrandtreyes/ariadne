use console::style;

use super::require_db;

pub fn cmd_dead_code(json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let dead = crate::db::query::get_dead_symbols(&db)?;

    if dead.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No dead code detected.");
        }
        return Ok(());
    }

    if json {
        let json_output = serde_json::to_string_pretty(&dead)?;
        println!("{}", json_output);
    } else {
        println!(
            "{} dead functions found:\n",
            style(dead.len()).yellow().bold()
        );
        for sym in &dead {
            let file_path = crate::db::query::file_path_by_id(&db, sym.file_id)
                .unwrap_or_else(|_| "unknown".to_string());
            println!(
                "  {} {} {}:{}",
                style(&sym.kind).dim(),
                style(&sym.name).bold(),
                style(&file_path).dim(),
                sym.line_start
            );
        }
    }

    Ok(())
}

pub fn cmd_stats(json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let file_count = crate::db::query::count_files(&db)?;
    let sym_count = crate::db::query::count_symbols(&db)?;
    let call_count = crate::db::query::count_calls(&db)?;
    let resolved = crate::db::query::count_resolved_calls(&db)?;
    let dead_count = crate::db::query::count_dead(&db)?;
    let rate = crate::db::query::resolution_rate(&db)?;
    let languages = crate::db::query::get_languages(&db)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "files": file_count,
                "symbols": sym_count,
                "calls": call_count,
                "resolved": resolved,
                "dead_functions": dead_count,
                "resolution_rate": rate,
                "languages": languages,
            }))?
        );
        return Ok(());
    }

    println!(
        "\n{}\n",
        style("Ariadne Index Statistics").bold().underlined()
    );
    println!("  Files:          {}", style(file_count).cyan());
    println!("  Symbols:        {}", style(sym_count).cyan());
    println!("  Calls:          {}", style(call_count).cyan());
    println!(
        "  Resolved:       {} ({:.0}%)",
        style(resolved).green(),
        rate * 100.0
    );
    println!("  Dead functions: {}", style(dead_count).yellow());
    println!(
        "  Languages:      {}",
        style(if languages.is_empty() {
            "none".to_string()
        } else {
            languages.join(", ")
        })
        .cyan()
    );
    println!();

    Ok(())
}

pub fn cmd_communities(json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.symbol_count, c.internal_edges, c.external_edges, c.modularity
         FROM communities c ORDER BY c.symbol_count DESC",
    )?;

    #[derive(serde::Serialize)]
    struct CommunityInfo {
        id: i64,
        name: String,
        symbol_count: i32,
        internal_edges: i32,
        external_edges: i32,
        modularity: f64,
        members: Vec<String>,
    }

    let mut communities: Vec<CommunityInfo> = stmt
        .query_map([], |row| {
            Ok(CommunityInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                symbol_count: row.get(2)?,
                internal_edges: row.get(3)?,
                external_edges: row.get(4)?,
                modularity: row.get(5)?,
                members: Vec::new(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Populate members for each community
    for comm in &mut communities {
        let mut mem_stmt = conn.prepare(
            "SELECT s.qualified_name FROM symbols s WHERE s.community_id = ?1 ORDER BY s.name",
        )?;
        comm.members = mem_stmt
            .query_map(rusqlite::params![comm.id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
    }

    if communities.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No communities detected. Run `ariadne index` first.");
        }
        return Ok(());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&communities)?);
    } else {
        println!(
            "\n{} {} detected:\n",
            style(communities.len()).cyan().bold(),
            if communities.len() == 1 {
                "community"
            } else {
                "communities"
            }
        );
        for comm in &communities {
            println!(
                "  {} {} ({} symbols, modularity: {:.2})",
                style("●").cyan(),
                style(&comm.name).bold(),
                comm.symbol_count,
                comm.modularity,
            );
            for member in &comm.members {
                println!("    {} {}", style("·").dim(), member);
            }
            println!();
        }
    }

    Ok(())
}

pub fn cmd_boundaries(json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let analysis = crate::analysis::boundaries::analyze_boundaries(&db)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&analysis)?);
        return Ok(());
    }

    if analysis.total_modules == 0 {
        println!("No module boundaries detected. Run `ariadne index` first.");
        return Ok(());
    }

    println!(
        "\n{} Module Boundary Analysis: {} modules, avg modularity {:.2}\n",
        style("=").cyan().bold(),
        style(analysis.total_modules).cyan(),
        analysis.avg_modularity,
    );

    // Module summary
    for m in &analysis.modules {
        let mod_style = if m.modularity >= 0.5 {
            style(format!("{:.2}", m.modularity)).green()
        } else {
            style(format!("{:.2}", m.modularity)).yellow()
        };
        println!(
            "  {} {} ({} symbols, modularity: {}, internal: {}, external: {})",
            style("*").cyan(),
            style(&m.name).bold(),
            m.symbol_count,
            mod_style,
            style(m.internal_calls).green(),
            style(m.external_calls).yellow(),
        );
        for f in &m.top_files {
            println!("      {} {}", style("-").dim(), style(f).dim());
        }
    }

    // Cross-boundary calls (top 10)
    if !analysis.cross_boundary_calls.is_empty() {
        println!(
            "\n  {} Cross-boundary calls (top 10):\n",
            style(">>").yellow().bold()
        );
        for cb in analysis.cross_boundary_calls.iter().take(10) {
            println!(
                "    {} -> {} ({} calls)",
                style(&cb.from_module).bold(),
                style(&cb.to_module).bold(),
                style(cb.call_count).yellow(),
            );
            let display_symbols: Vec<&str> =
                cb.symbols.iter().take(5).map(|s| s.as_str()).collect();
            if !display_symbols.is_empty() {
                println!("      symbols: {}", style(display_symbols.join(", ")).dim());
            }
        }
    }

    // Boundary violations
    if analysis.boundary_violations > 0 {
        println!(
            "\n  {} {} boundary violation(s): modules where external calls exceed internal calls",
            style("!").red().bold(),
            style(analysis.boundary_violations).red().bold(),
        );
        for m in &analysis.modules {
            if m.external_calls > m.internal_calls {
                println!(
                    "    {} {} (external: {} > internal: {})",
                    style("!").red(),
                    style(&m.name).bold(),
                    m.external_calls,
                    m.internal_calls,
                );
            }
        }
    } else {
        println!(
            "\n  {} No boundary violations detected.",
            style("ok").green().bold()
        );
    }

    println!();
    Ok(())
}

pub fn cmd_affected_tests(diff_ref: &str, json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let result = crate::analysis::affected_tests::find_affected_tests(&db, diff_ref)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        if result.total_tests_affected == 0 {
            println!("No tests affected by changes in {}", diff_ref);
            return Ok(());
        }

        println!(
            "\n{} {} affected by changes ({} files changed):\n",
            style(result.total_tests_affected).yellow().bold(),
            if result.total_tests_affected == 1 {
                "test"
            } else {
                "tests"
            },
            result.changed_files
        );

        for test in &result.test_functions {
            println!("  {} {}", style("●").yellow(), style(test).bold());
        }

        if !result.test_files.is_empty() {
            println!("\n  Test files: {}", result.test_files.join(", "));
        }
    }

    Ok(())
}

pub fn cmd_check_rules(json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;
    let config = crate::config::repo::load(&root)?;

    if config.rules.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&crate::analysis::arch_rules::RuleCheckResult {
                    violations: Vec::new(),
                    rules_passed: 0,
                    rules_failed: 0,
                    has_errors: false,
                })?
            );
        } else {
            println!("No architectural rules defined in ariadne.toml");
        }
        return Ok(());
    }

    let result = crate::analysis::arch_rules::check_rules(&db, &config.rules)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else if result.violations.is_empty() {
        println!(
            "{} All {} architectural rules passed",
            style("✓").green().bold(),
            result.rules_passed
        );
    } else {
        println!(
            "\n{} {} rule violations found ({} rules passed, {} failed):\n",
            style("✗").red().bold(),
            result.violations.len(),
            result.rules_passed,
            result.rules_failed,
        );

        for v in &result.violations {
            let sev = if v.severity == "error" {
                style(&v.severity).red()
            } else {
                style(&v.severity).yellow()
            };
            println!(
                "  {} [{}] {} -> {}",
                sev,
                style(&v.rule_name).bold(),
                &v.from_file,
                &v.to_file,
            );
            if let Some(line) = v.line {
                println!("       at line {}", line);
            }
        }
    }

    // Exit with code 1 if there are error-severity violations
    if result.has_errors {
        std::process::exit(1);
    }

    Ok(())
}
