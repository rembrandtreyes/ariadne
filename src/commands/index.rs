use std::path::Path;

use console::style;
use indicatif::{ProgressBar, ProgressStyle};

use super::DB_FILENAME;

fn new_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

pub fn cmd_index(path: &Path, full: bool) -> anyhow::Result<()> {
    let root = std::fs::canonicalize(path)?;

    let workspace_config = crate::config::workspace::load(&root)?;
    if !workspace_config.services.is_empty() {
        return cmd_index_workspace(&root, full, &workspace_config);
    }

    let pb = new_spinner("Scanning...");

    let config = crate::config::repo::load(&root)?;

    let db_path = root.join(DB_FILENAME);
    if full && db_path.exists() {
        std::fs::remove_file(&db_path)?;
    }
    let db = crate::db::Database::open(&db_path)?;

    let discovery = crate::pipeline::discovery::discover(&root, &config)?;
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

    let stats = crate::pipeline::run_full_pipeline(&db, &root, &config)?;

    pb.finish_and_clear();

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

    if !stats.parse_error_files.is_empty() {
        let shown = stats
            .parse_error_files
            .iter()
            .take(5)
            .map(|(path, n)| format!("{path} ({n})"))
            .collect::<Vec<_>>()
            .join(", ");
        let suffix = if stats.parse_error_files.len() > 5 {
            ", …"
        } else {
            ""
        };
        println!(
            "{} {} file(s) had syntax errors — graph may be incomplete: {shown}{suffix}",
            style("⚠").yellow().bold(),
            stats.parse_error_files.len(),
        );
    }

    if !stats.phase_durations.is_empty() && stats.duration_ms > 0 {
        println!();
        for phase in &stats.phase_durations {
            let pct = (phase.duration_ms as f64 / stats.duration_ms as f64) * 100.0;
            let bar = if pct >= 30.0 {
                style(format!(
                    "{:<20} {:>5}ms  ({pct:>4.0}%)",
                    phase.name, phase.duration_ms
                ))
                .yellow()
            } else {
                style(format!(
                    "{:<20} {:>5}ms  ({pct:>4.0}%)",
                    phase.name, phase.duration_ms
                ))
                .dim()
            };
            println!("  {bar}");
        }
    }

    Ok(())
}

fn cmd_index_workspace(
    root: &Path,
    full: bool,
    workspace: &crate::config::WorkspaceConfig,
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
    let db = crate::db::Database::open(&db_path)?;

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

        let pb = new_spinner(&format!("Indexing {}...", svc.name));

        let svc_config = crate::config::repo::load(&svc_root)?;
        let stats = crate::pipeline::run_full_pipeline(&db, &svc_root, &svc_config)?;
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
