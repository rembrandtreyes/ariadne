use std::path::PathBuf;

use clap::{Parser, Subcommand};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Subcommand, Debug)]
enum PluginAction {
    /// List installed plugins
    List,
    /// Install a plugin from a .wasm file
    Install {
        /// Path to the .wasm file
        path: PathBuf,
    },
    /// Scaffold a new plugin project
    Init {
        /// Name for the new plugin (e.g., "kotlin")
        name: String,
    },
    /// Remove an installed plugin
    Remove {
        /// Name of the plugin to remove
        name: String,
    },
}

/// Ariadne -- the thread through the labyrinth.
/// Universal dependency graph analysis for AI agents.
#[derive(Parser, Debug)]
#[command(name = "ariadne", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Index a codebase, building the dependency graph database
    Index {
        /// Path to the repository root
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Force a full re-index, ignoring incremental cache
        #[arg(long)]
        full: bool,
    },

    /// Search for symbols, files, or patterns in the graph
    Search {
        /// The query string to search for
        query: String,

        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Compute the blast radius of changing a symbol
    BlastRadius {
        /// The symbol name (e.g., "login_user" or "auth:login_user")
        symbol: String,

        /// Include cross-service dependencies in the analysis
        #[arg(long)]
        cross_service: bool,

        /// Maximum traversal depth
        #[arg(long, default_value = "10")]
        depth: u32,

        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Trace the call chain to or from a symbol
    CallChain {
        /// The symbol name
        symbol: String,

        /// Include cross-service call chains
        #[arg(long)]
        cross_service: bool,

        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Detect dead (unreachable) code in the codebase
    DeadCode {
        /// Output results as JSON
        #[arg(long)]
        json: bool,

        /// Minimum confidence threshold (0-100) to report
        #[arg(long, default_value = "80")]
        threshold: u32,
    },

    /// Display statistics about the indexed codebase
    Stats {
        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Start the MCP server for AI agent integration
    Serve {
        /// Enable HTTP/SSE transport on the given address (e.g., "127.0.0.1:3000")
        #[arg(long)]
        http: Option<String>,
    },

    /// Detect module communities in the dependency graph
    Communities {
        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Analyze module boundaries in a monolith codebase
    Boundaries {
        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Find tests affected by recent code changes
    AffectedTests {
        /// Git diff reference (e.g., HEAD~1, main..HEAD)
        #[arg(long, default_value = "HEAD~1")]
        diff: String,

        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Check architectural rules defined in ariadne.toml
    CheckRules {
        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Display service dependency topology as a Mermaid diagram
    Topology {
        /// Output results as JSON instead of Mermaid
        #[arg(long)]
        json: bool,
    },

    /// Launch the interactive web dashboard
    Dash {
        /// Port to bind on
        #[arg(long, default_value = "1337")]
        port: u16,
    },

    /// Export the index in SCIP (Sourcegraph) format
    ExportScip {
        /// Output file path
        #[arg(default_value = "index.scip.json")]
        output: PathBuf,
    },

    /// Manage language parser plugins
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },

    /// Start the LSP server for IDE integration
    #[cfg(feature = "lsp")]
    Lsp,

    /// Watch for file changes and re-index incrementally
    #[cfg(feature = "watch")]
    Watch {
        /// Also start the MCP server
        #[arg(long)]
        serve: bool,

        /// Debounce interval in milliseconds
        #[arg(long, default_value = "500")]
        debounce: u64,
    },
}

const DB_FILENAME: &str = ".ariadne.db";

fn require_db(root: &std::path::Path) -> anyhow::Result<ariadne::db::Database> {
    let db_path = root.join(DB_FILENAME);
    if !db_path.exists() {
        anyhow::bail!(
            "No index found at {}. Run `ariadne index` first.",
            db_path.display()
        );
    }
    ariadne::db::Database::open(&db_path)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("ariadne=info".parse().unwrap_or_default()),
        )
        .with_target(false)
        .without_time()
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Index { path, full } => cmd_index(&path, full),
        Commands::Search { query, json } => cmd_search(&query, json),
        Commands::BlastRadius {
            symbol,
            cross_service,
            depth,
            json,
        } => cmd_blast_radius(&symbol, cross_service, depth, json),
        Commands::CallChain {
            symbol,
            cross_service,
            json,
        } => cmd_call_chain(&symbol, cross_service, json),
        Commands::DeadCode { json, threshold } => cmd_dead_code(json, threshold),
        Commands::Stats { json } => cmd_stats(json),
        // SECURITY: The --http flag is intentionally unused. Wiring up HTTP/SSE transport
        // would expose all 10 MCP tools over the network without authentication.
        // Only stdio transport (OS process isolation) is safe for the current threat model.
        Commands::Serve { http: _ } => ariadne::mcp::serve_stdio().await,
        Commands::Communities { json } => cmd_communities(json),
        Commands::Boundaries { json } => cmd_boundaries(json),
        Commands::AffectedTests { diff, json } => cmd_affected_tests(&diff, json),
        Commands::CheckRules { json } => cmd_check_rules(json),
        Commands::Topology { json } => cmd_topology(json),
        Commands::Dash { port } => cmd_dash(port).await,
        Commands::ExportScip { output } => cmd_export_scip(&output),
        Commands::Plugin { action } => cmd_plugin(action),
        #[cfg(feature = "lsp")]
        Commands::Lsp => {
            let root = std::env::current_dir()?;
            let db_path = root.join(DB_FILENAME);
            ariadne::lsp::serve_lsp_stdio(db_path).await
        }
        #[cfg(feature = "watch")]
        Commands::Watch { serve, debounce } => {
            if serve {
                let root = std::env::current_dir()?;
                let db_path = root.join(DB_FILENAME);
                if !db_path.exists() {
                    anyhow::bail!("No index found. Run `ariadne index` first.");
                }
                ariadne::watch::watch_and_serve(root, db_path, debounce).await
            } else {
                cmd_watch(debounce)
            }
        }
    }
}

fn cmd_index(path: &std::path::Path, full: bool) -> anyhow::Result<()> {
    let root = std::fs::canonicalize(path)?;

    // Check for workspace mode
    let workspace_config = ariadne::config::workspace::load(&root)?;
    if !workspace_config.services.is_empty() {
        return cmd_index_workspace(&root, full, &workspace_config);
    }

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    pb.set_message("Scanning...");
    pb.enable_steady_tick(std::time::Duration::from_millis(80));

    // Load config
    let config = ariadne::config::repo::load(&root)?;

    // Open (or create) the database
    let db_path = root.join(DB_FILENAME);
    if full && db_path.exists() {
        std::fs::remove_file(&db_path)?;
    }
    let db = ariadne::db::Database::open(&db_path)?;

    // Run the auto-detection to show detected languages
    let discovery = ariadne::pipeline::discovery::discover(&root, &config)?;
    let lang_names: Vec<String> = discovery
        .languages
        .iter()
        .map(|l| l.display_name().to_string())
        .collect();
    pb.set_message(format!(
        "Scanning... detected {}",
        if lang_names.is_empty() {
            "no languages".to_string()
        } else {
            lang_names.join(", ")
        }
    ));

    // Run the full pipeline
    let stats = ariadne::pipeline::run_full_pipeline(&db, &root, &config)?;

    pb.finish_and_clear();

    // Print summary in the style from the blueprint
    let resolution_pct = (stats.resolution_rate * 100.0) as u32;
    println!(
        "{} {} {} {} ({resolution_pct}% resolved) {} {} dead functions {} {:.1}s",
        style("✓").green().bold(),
        style(format!("{}", stats.symbols_found)).cyan(),
        style("symbols").dim(),
        style("·").dim(),
        style("·").dim(),
        stats.dead_functions,
        style("·").dim(),
        stats.duration_ms as f64 / 1000.0,
    );

    Ok(())
}

fn cmd_index_workspace(
    root: &std::path::Path,
    full: bool,
    workspace: &ariadne::config::WorkspaceConfig,
) -> anyhow::Result<()> {
    println!(
        "{} Workspace mode: indexing {} services\n",
        style("⚡").bold(),
        style(workspace.services.len()).cyan()
    );

    let db_path = root.join(DB_FILENAME);
    if full && db_path.exists() {
        std::fs::remove_file(&db_path)?;
    }
    let db = ariadne::db::Database::open(&db_path)?;

    let mut total_symbols = 0usize;
    let start = std::time::Instant::now();

    for svc in &workspace.services {
        let svc_root = root.join(&svc.path);
        if !svc_root.exists() {
            eprintln!(
                "  {} Service path not found: {} ({})",
                style("⚠").yellow(),
                svc.name,
                svc_root.display()
            );
            continue;
        }

        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        pb.set_message(format!("Indexing {}...", svc.name));
        pb.enable_steady_tick(std::time::Duration::from_millis(80));

        let svc_config = ariadne::config::repo::load(&svc_root)?;
        let stats = ariadne::pipeline::run_full_pipeline(&db, &svc_root, &svc_config)?;
        total_symbols += stats.symbols_found;

        pb.finish_and_clear();

        let pct = (stats.resolution_rate * 100.0) as u32;
        println!(
            "  {} {} — {} symbols ({pct}% resolved)",
            style("✓").green(),
            style(&svc.name).bold(),
            stats.symbols_found,
        );
    }

    let elapsed = start.elapsed().as_secs_f64();
    println!(
        "\n{} {} total symbols across {} services in {:.1}s",
        style("✓").green().bold(),
        style(total_symbols).cyan(),
        workspace.services.len(),
        elapsed,
    );

    Ok(())
}

fn cmd_search(query: &str, json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let options = ariadne::search::SearchOptions {
        limit: Some(20),
        fuzzy: true,
        ..Default::default()
    };

    let results = ariadne::search::search(&db, query, &options)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    if results.is_empty() {
        println!("No symbols found matching \"{}\"", query);
        return Ok(());
    }

    println!(
        "{} results for \"{}\":\n",
        style(results.len()).cyan(),
        style(query).bold()
    );

    for r in &results {
        let kind = style(&r.kind).dim();
        let name = style(&r.name).bold();
        let file = style(&r.file).dim();
        let line = style(r.line).dim();
        println!("  {kind:<12} {name:<40} {file}:{line}");
    }

    Ok(())
}

fn cmd_blast_radius(symbol: &str, cross_service: bool, depth: u32, json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let sym = ariadne::db::query::find_symbol_by_name(&db, symbol)?
        .ok_or_else(|| anyhow::anyhow!("Symbol not found: {}", symbol))?;

    let graph = ariadne::db::query::build_call_graph(&db, None)?;
    let result = if cross_service {
        ariadne::graph::blast_radius::analyze_blast_radius_cross_service(
            &graph,
            &db,
            sym.id as u64,
            Some(depth),
        )
    } else {
        ariadne::graph::blast_radius::analyze_blast_radius(
            &graph,
            sym.id as u64,
            Some(depth),
            false,
        )
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

fn cmd_call_chain(symbol: &str, cross_service: bool, json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let sym = ariadne::db::query::find_symbol_by_name(&db, symbol)?
        .ok_or_else(|| anyhow::anyhow!("Symbol not found: {}", symbol))?;

    let graph = ariadne::db::query::build_call_graph(&db, None)?;
    let mermaid =
        ariadne::graph::call_chain::extract_call_chain(&graph, sym.id, cross_service);

    if json {
        println!("{}", serde_json::json!({ "mermaid": mermaid }));
    } else {
        println!("{}", mermaid);
    }

    Ok(())
}

fn cmd_dead_code(json: bool, threshold: u32) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let mut dead = ariadne::db::query::get_dead_symbols(&db)?;

    // Dead code detected via reachability BFS has implicit 100% confidence.
    // Threshold filters by confidence (0-100); values above 100 suppress all output.
    dead.retain(|_| threshold <= 100);

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
            let file_path = ariadne::db::query::file_path_by_id(&db, sym.file_id)
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

fn cmd_stats(json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let file_count = ariadne::db::query::count_files(&db)?;
    let sym_count = ariadne::db::query::count_symbols(&db)?;
    let call_count = ariadne::db::query::count_calls(&db)?;
    let resolved = ariadne::db::query::count_resolved_calls(&db)?;
    let dead_count = ariadne::db::query::count_dead(&db)?;
    let rate = ariadne::db::query::resolution_rate(&db)?;
    let languages = ariadne::db::query::get_languages(&db)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "files": file_count,
            "symbols": sym_count,
            "calls": call_count,
            "resolved": resolved,
            "dead_functions": dead_count,
            "resolution_rate": rate,
            "languages": languages,
        }))?);
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

fn cmd_communities(json: bool) -> anyhow::Result<()> {
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
        .filter_map(|r| r.ok())
        .collect();

    // Populate members for each community
    for comm in &mut communities {
        let mut mem_stmt = conn.prepare(
            "SELECT s.qualified_name FROM symbols s WHERE s.community_id = ?1 ORDER BY s.name",
        )?;
        comm.members = mem_stmt
            .query_map(rusqlite::params![comm.id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
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

fn cmd_boundaries(json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    let analysis = ariadne::analysis::boundaries::analyze_boundaries(&db)?;

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

fn cmd_affected_tests(diff_ref: &str, json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let _db = require_db(&root)?;

    let result = ariadne::analysis::affected_tests::find_affected_tests(&_db, diff_ref)?;

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

fn cmd_check_rules(json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;
    let config = ariadne::config::repo::load(&root)?;

    if config.rules.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &ariadne::analysis::arch_rules::RuleCheckResult {
                        violations: Vec::new(),
                        rules_passed: 0,
                        rules_failed: 0,
                        has_errors: false,
                    }
                )?
            );
        } else {
            println!("No architectural rules defined in ariadne.toml");
        }
        return Ok(());
    }

    let result = ariadne::analysis::arch_rules::check_rules(&db, &config.rules)?;

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

fn cmd_topology(json: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;
    let conn = db.conn();

    #[derive(serde::Serialize)]
    struct ServiceEdge {
        from: String,
        to: String,
        protocol: String,
        call_count: i32,
        confidence: f64,
    }

    let mut stmt = conn.prepare(
        "SELECT sf.name, st.name, se.protocol, se.call_count, se.confidence
         FROM service_edges se
         JOIN services sf ON se.from_service_id = sf.id
         JOIN services st ON se.to_service_id = st.id
         ORDER BY se.call_count DESC",
    )?;

    let edges: Vec<ServiceEdge> = stmt
        .query_map([], |row| {
            Ok(ServiceEdge {
                from: row.get(0)?,
                to: row.get(1)?,
                protocol: row.get(2)?,
                call_count: row.get(3)?,
                confidence: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Also get all services for the diagram
    let mut svc_stmt = conn.prepare("SELECT name FROM services ORDER BY name")?;
    let services: Vec<String> = svc_stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    if services.is_empty() {
        println!("No services found. Run `ariadne index` first.");
        return Ok(());
    }

    if json {
        #[derive(serde::Serialize)]
        struct TopologyResult {
            services: Vec<String>,
            edges: Vec<ServiceEdge>,
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&TopologyResult {
                services: services.clone(),
                edges,
            })?
        );
    } else {
        println!("graph LR");
        for svc in &services {
            // Sanitize name for Mermaid
            let safe = svc.replace(['-', ' '], "_");
            println!("    {}[{}]", safe, svc);
        }
        for edge in &edges {
            let from = edge.from.replace(['-', ' '], "_");
            let to = edge.to.replace(['-', ' '], "_");
            println!(
                "    {} -->|{} ({})| {}",
                from, edge.protocol, edge.call_count, to
            );
        }
    }

    Ok(())
}

async fn cmd_dash(port: u16) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db_path = root.join(DB_FILENAME);
    if !db_path.exists() {
        anyhow::bail!("No index found. Run `ariadne index` first.");
    }

    let config = ariadne::dashboard::DashboardConfig { port };
    ariadne::dashboard::serve(config, &db_path).await
}

fn cmd_export_scip(output: &std::path::Path) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db = require_db(&root)?;

    ariadne::analysis::scip_export::export_scip(&db, output, &root)?;

    println!(
        "{} Exported SCIP index to {}",
        style("✓").green().bold(),
        output.display()
    );

    Ok(())
}

fn cmd_plugin(action: PluginAction) -> anyhow::Result<()> {
    match action {
        PluginAction::List => {
            let mut registry = ariadne::plugins::registry::PluginRegistry::new();
            registry.discover()?;
            let plugins = registry.list();
            if plugins.is_empty() {
                println!("No plugins installed.");
                println!("\nInstall a plugin:");
                println!("  ariadne plugin install ./path/to/plugin.wasm");
                println!("\nScaffold a new plugin:");
                println!("  ariadne plugin init my-language");
            } else {
                println!("{} installed:\n", plugins.len());
                for p in plugins {
                    println!(
                        "  {} v{} ({})",
                        style(&p.name).bold(),
                        &p.version,
                        if p.extensions.is_empty() {
                            "no extensions".to_string()
                        } else {
                            p.extensions.join(", ")
                        }
                    );
                }
            }
            Ok(())
        }
        PluginAction::Install { path } => {
            let installed = ariadne::plugins::install_plugin(&path)?;
            println!(
                "{} Installed plugin to {}",
                style("✓").green().bold(),
                installed.display()
            );
            Ok(())
        }
        PluginAction::Init { name } => {
            let output = std::env::current_dir()?.join(format!("ariadne-{}", name));
            ariadne::plugins::init_plugin(&name, &output)?;
            println!(
                "{} Scaffolded plugin at {}",
                style("✓").green().bold(),
                output.display()
            );
            Ok(())
        }
        PluginAction::Remove { name } => {
            ariadne::plugins::remove_plugin(&name)?;
            println!("{} Removed plugin: {}", style("✓").green().bold(), name);
            Ok(())
        }
    }
}

#[cfg(feature = "watch")]
fn cmd_watch(debounce: u64) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db_path = root.join(DB_FILENAME);
    if !db_path.exists() {
        anyhow::bail!("No index found. Run `ariadne index` first.");
    }
    ariadne::watch::watch_and_reindex(&root, &db_path, debounce)
}
