pub mod registry;

#[cfg(feature = "plugins")]
pub mod host;

use std::path::{Path, PathBuf};

/// Plugin metadata describing a discovered parser plugin.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub extensions: Vec<String>,
    pub version: String,
    pub path: PathBuf,
}

/// Get the plugin directories in priority order.
pub fn plugin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // 1. Repo-local plugins
    dirs.push(PathBuf::from(".ariadne/plugins"));

    // 2. User-global plugins
    if let Some(home) = home_dir() {
        dirs.push(home.join(".ariadne/plugins"));
    }

    dirs
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Return the global plugin directory (~/.ariadne/plugins), creating it if needed.
fn global_plugin_dir() -> anyhow::Result<PathBuf> {
    let home = home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let dir = home.join(".ariadne/plugins");
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// Install a plugin from a local .wasm file path.
///
/// Validates the file extension, copies to ~/.ariadne/plugins/,
/// and returns the installed path.
pub fn install_plugin(wasm_path: &Path) -> anyhow::Result<PathBuf> {
    // Validate it's a .wasm file
    match wasm_path.extension().and_then(|e| e.to_str()) {
        Some("wasm") => {}
        _ => anyhow::bail!(
            "Plugin file must have a .wasm extension: {}",
            wasm_path.display()
        ),
    }

    if !wasm_path.exists() {
        anyhow::bail!("Plugin file not found: {}", wasm_path.display());
    }

    // Reject symlinks to prevent following links to unintended files
    let metadata = std::fs::symlink_metadata(wasm_path)?;
    if metadata.file_type().is_symlink() {
        anyhow::bail!(
            "Symlinks are not supported for plugin installation: {}",
            wasm_path.display()
        );
    }

    let file_name = wasm_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid file path: {}", wasm_path.display()))?;

    let dest_dir = global_plugin_dir()?;
    let dest = dest_dir.join(file_name);

    std::fs::copy(wasm_path, &dest)?;

    Ok(dest)
}

/// Initialize a new plugin scaffold directory.
///
/// Creates the directory structure:
///   ariadne-{name}/
///   ├── Cargo.toml
///   ├── src/lib.rs
///   ├── wit/ariadne-plugin.wit
///   └── README.md
pub fn init_plugin(name: &str, output_dir: &Path) -> anyhow::Result<()> {
    if output_dir.exists() {
        anyhow::bail!("Directory already exists: {}", output_dir.display());
    }

    // Create directory structure
    std::fs::create_dir_all(output_dir.join("src"))?;
    std::fs::create_dir_all(output_dir.join("wit"))?;

    // Write Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "ariadne-{name}"
version = "0.1.0"
edition = "2021"
description = "Ariadne language parser plugin for {name}"

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = "0.36"

[package.metadata.component]
package = "ariadne:plugin"
target.path = "wit/ariadne-plugin.wit"
"#,
        name = name,
    );
    std::fs::write(output_dir.join("Cargo.toml"), cargo_toml)?;

    // Write src/lib.rs
    let lib_rs = format!(
        r#"// Ariadne plugin: {name}
//
// Build with:
//   cargo component build --release
//
// Install with:
//   ariadne plugin install target/wasm32-wasip1/release/ariadne_{name_underscore}.wasm

// TODO: Generate bindings with wit-bindgen and implement the language-parser interface.
//
// You need to implement two functions from the WIT contract:
//
//   get-info() -> language-info
//     Return the language name, file extensions, and plugin version.
//
//   parse-file(source: string, file-path: string) -> parse-result
//     Parse the given source code and return symbols, imports, calls,
//     API endpoints, and API call sites.
//
// See wit/ariadne-plugin.wit for the full type definitions.
"#,
        name = name,
        name_underscore = name.replace('-', "_"),
    );
    std::fs::write(output_dir.join("src/lib.rs"), lib_rs)?;

    // Copy the WIT file
    let wit_content = include_str!("wit/ariadne-plugin.wit");
    std::fs::write(output_dir.join("wit/ariadne-plugin.wit"), wit_content)?;

    // Write README.md
    let readme = format!(
        r#"# ariadne-{name}

Ariadne language parser plugin for **{name}**.

## Building

```bash
cargo component build --release
```

## Installing

```bash
ariadne plugin install target/wasm32-wasip1/release/ariadne_{name_underscore}.wasm
```

## WIT Contract

See `wit/ariadne-plugin.wit` for the interface this plugin must implement.
"#,
        name = name,
        name_underscore = name.replace('-', "_"),
    );
    std::fs::write(output_dir.join("README.md"), readme)?;

    Ok(())
}

/// Remove a plugin by name.
///
/// Searches ~/.ariadne/plugins/ for a matching .wasm file and deletes it.
pub fn remove_plugin(name: &str) -> anyhow::Result<()> {
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        anyhow::bail!("Plugin name must not contain path separators or '..'");
    }

    let dir = global_plugin_dir()?;

    // Try exact match first: {name}.wasm
    let exact = dir.join(format!("{}.wasm", name));
    if exact.exists() {
        std::fs::remove_file(&exact)?;
        return Ok(());
    }

    // Try with ariadne- prefix: ariadne-{name}.wasm
    let prefixed = dir.join(format!("ariadne-{}.wasm", name));
    if prefixed.exists() {
        std::fs::remove_file(&prefixed)?;
        return Ok(());
    }

    anyhow::bail!(
        "Plugin not found: {}. Looked for {}.wasm and ariadne-{}.wasm in {}",
        name,
        name,
        name,
        dir.display()
    )
}
