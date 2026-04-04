use std::path::PathBuf;

use clap::{Parser, Subcommand};

use ariadne::commands;
use ariadne::commands::plugin::PluginAction;

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

    /// Explain why a symbol matters: callers, callees, blast radius, status
    Why {
        /// The symbol name (e.g., "login_user" or "auth:login_user")
        symbol: String,

        /// Output results as JSON
        #[arg(long)]
        json: bool,
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
        Commands::Index { path, full } => commands::index::cmd_index(&path, full),
        Commands::Search { query, json } => commands::search::cmd_search(&query, json),
        Commands::BlastRadius {
            symbol,
            cross_service,
            depth,
            json,
        } => commands::graph::cmd_blast_radius(&symbol, cross_service, depth, json),
        Commands::CallChain {
            symbol,
            cross_service,
            json,
        } => commands::graph::cmd_call_chain(&symbol, cross_service, json),
        Commands::DeadCode { json } => commands::analysis::cmd_dead_code(json),
        Commands::Stats { json } => commands::analysis::cmd_stats(json),
        Commands::Serve { http } => {
            if http.is_some() {
                eprintln!(
                    "Warning: --http is not supported (no authentication layer). \
                     Using stdio transport only."
                );
            }
            ariadne::mcp::serve_stdio().await
        }
        Commands::Communities { json } => commands::analysis::cmd_communities(json),
        Commands::Boundaries { json } => commands::analysis::cmd_boundaries(json),
        Commands::AffectedTests { diff, json } => {
            commands::analysis::cmd_affected_tests(&diff, json)
        }
        Commands::CheckRules { json } => commands::analysis::cmd_check_rules(json),
        Commands::Topology { json } => commands::serve::cmd_topology(json),
        Commands::Why { symbol, json } => commands::graph::cmd_why(&symbol, json),
        Commands::Dash { port } => commands::serve::cmd_dash(port).await,
        Commands::ExportScip { output } => commands::serve::cmd_export_scip(&output),
        Commands::Plugin { action } => commands::plugin::cmd_plugin(action),
        #[cfg(feature = "lsp")]
        Commands::Lsp => {
            let root = std::env::current_dir()?;
            let db_path = root.join(commands::DB_FILENAME);
            ariadne::lsp::serve_lsp_stdio(db_path).await
        }
        #[cfg(feature = "watch")]
        Commands::Watch { serve, debounce } => {
            if serve {
                let root = std::env::current_dir()?;
                let db_path = root.join(commands::DB_FILENAME);
                if !db_path.exists() {
                    anyhow::bail!("No index found. Run `ariadne index` first.");
                }
                ariadne::watch::watch_and_serve(root, db_path, debounce).await
            } else {
                commands::watch::cmd_watch(debounce)
            }
        }
    }
}
