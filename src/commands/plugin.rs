use std::path::PathBuf;

use clap::Subcommand;
use console::style;

#[derive(Subcommand, Debug)]
pub enum PluginAction {
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

pub fn cmd_plugin(action: PluginAction) -> anyhow::Result<()> {
    match action {
        PluginAction::List => {
            let mut registry = crate::plugins::registry::PluginRegistry::new();
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
            let installed = crate::plugins::install_plugin(&path)?;
            println!(
                "{} Installed plugin to {}",
                style("✓").green().bold(),
                installed.display()
            );
            Ok(())
        }
        PluginAction::Init { name } => {
            let output = std::env::current_dir()?.join(format!("ariadne-{}", name));
            crate::plugins::init_plugin(&name, &output)?;
            println!(
                "{} Scaffolded plugin at {}",
                style("✓").green().bold(),
                output.display()
            );
            Ok(())
        }
        PluginAction::Remove { name } => {
            crate::plugins::remove_plugin(&name)?;
            println!("{} Removed plugin: {}", style("✓").green().bold(), name);
            Ok(())
        }
    }
}
