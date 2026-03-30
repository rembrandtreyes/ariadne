//! WASM Plugin Host for Ariadne — Component Model Implementation
//!
//! Provides a sandboxed runtime for loading and executing language parser plugins
//! compiled to WebAssembly components. Plugins are loaded via wasmtime with NO
//! filesystem or network access — the WASM guest can only compute and return data
//! through the typed ABI generated from the WIT interface.
//!
//! # Architecture
//!
//! ```text
//! +------------------+       +-------------------+
//! |  Ariadne Host    |  WIT  |  WASM Component   |
//! |                  | typed |  (sandboxed)       |
//! |  WasmPluginHost  +------>+  get-info()        |
//! |  LoadedPlugin    +------>+  parse-file()      |
//! |                  |       |                    |
//! |  (full OS access)|       |  (NO fs/net/env)   |
//! +------------------+       +-------------------+
//! ```
//!
//! Plugins are compiled to the component model (`cargo component build`) and
//! implement the `ariadne:plugin/language-parser` interface defined in
//! `src/plugins/wit/ariadne-plugin.wit`.

use std::path::Path;

use anyhow::{Context, Result};
use tracing::{debug, info};

use exports::ariadne::plugin::language_parser::SymbolKind;

// Generate typed Rust bindings from the WIT world definition.
// This produces the `AriadnePlugin` struct and all WIT record/enum types.
wasmtime::component::bindgen!({
    world: "ariadne-plugin",
    path: "src/plugins/wit/ariadne-plugin.wit",
});

// ---------------------------------------------------------------------------
// Public data types (the host-side representation, kept stable)
// ---------------------------------------------------------------------------

/// Result of parsing a file through a WASM plugin.
#[derive(Debug, Default)]
pub struct PluginParseResult {
    pub symbols: Vec<PluginSymbol>,
    pub imports: Vec<PluginImport>,
    pub calls: Vec<PluginCall>,
}

/// A symbol discovered in the source file.
#[derive(Debug)]
pub struct PluginSymbol {
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub line_start: u32,
    pub line_end: u32,
    pub is_exported: bool,
}

/// An import/dependency reference found in the source file.
#[derive(Debug)]
pub struct PluginImport {
    pub imported_name: String,
    pub module_path: String,
    pub line: u32,
}

/// A call-site linking a caller symbol to a callee symbol.
#[derive(Debug)]
pub struct PluginCall {
    pub caller_name: String,
    pub callee_name: String,
    pub line: u32,
}

/// Metadata about the language a plugin handles.
#[derive(Debug, Clone)]
pub struct PluginLanguageInfo {
    pub name: String,
    pub extensions: Vec<String>,
    pub version: String,
}

// ---------------------------------------------------------------------------
// WasmPluginHost
// ---------------------------------------------------------------------------

/// WASM plugin host backed by wasmtime.
///
/// Creates a single `Engine` shared across all loaded plugins. The engine
/// does NOT enable WASI — plugins run in a pure computation sandbox.
pub struct WasmPluginHost {
    engine: wasmtime::Engine,
}

impl WasmPluginHost {
    /// Create a new plugin host with a default wasmtime engine.
    pub fn new() -> Result<Self> {
        let mut config = wasmtime::Config::new();
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);

        // ISC-C8: Enable fuel-based execution limits to prevent infinite-loop DoS.
        config.consume_fuel(true);

        let engine = wasmtime::Engine::new(&config)
            .map_err(anyhow::Error::from)
            .context("Failed to create wasmtime engine")?;

        info!("WASM plugin host initialized (component model, wasmtime engine ready)");
        Ok(Self { engine })
    }

    /// Load a WASM component plugin from a `.wasm` file on disk.
    ///
    /// The file must be a WebAssembly component (produced by `cargo component build`)
    /// implementing the `ariadne:plugin/language-parser` interface.
    pub fn load_plugin(&self, path: &Path) -> Result<LoadedPlugin> {
        // ISC-C9: Reject oversized plugin files before reading into memory.
        const MAX_PLUGIN_SIZE: u64 = 50 * 1024 * 1024; // 50 MB

        let metadata = std::fs::metadata(path)
            .with_context(|| format!("Failed to stat plugin file: {}", path.display()))?;
        if metadata.len() > MAX_PLUGIN_SIZE {
            anyhow::bail!(
                "Plugin file too large: {} bytes (max {} bytes)",
                metadata.len(),
                MAX_PLUGIN_SIZE
            );
        }

        let wasm_bytes = std::fs::read(path)
            .with_context(|| format!("Failed to read plugin file: {}", path.display()))?;

        debug!(
            "Compiling WASM component from {} ({} bytes)",
            path.display(),
            wasm_bytes.len()
        );

        let component = wasmtime::component::Component::new(&self.engine, &wasm_bytes)
            .map_err(anyhow::Error::from)
            .with_context(|| format!("Failed to compile WASM component: {}", path.display()))?;

        let plugin_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        info!(
            "Loaded component plugin '{}' from {}",
            plugin_name,
            path.display()
        );

        Ok(LoadedPlugin {
            engine: self.engine.clone(),
            component,
            plugin_name,
        })
    }
}

// ---------------------------------------------------------------------------
// LoadedPlugin
// ---------------------------------------------------------------------------

/// A compiled WASM component plugin ready to be called.
///
/// Each call to `get_info()` or `parse_file()` creates a fresh `Store` and
/// `Instance`, ensuring complete isolation between invocations.
pub struct LoadedPlugin {
    engine: wasmtime::Engine,
    component: wasmtime::component::Component,
    plugin_name: String,
}

impl LoadedPlugin {
    /// Query the plugin for its language metadata via the `get-info` export.
    pub fn get_info(&self) -> Result<PluginLanguageInfo> {
        let mut store = wasmtime::Store::new(&self.engine, ());
        // ISC-C8: Fuel limit prevents runaway plugins.
        store
            .set_fuel(1_000_000_000)
            .map_err(anyhow::Error::from)
            .context("Failed to set fuel on store")?;

        let linker = wasmtime::component::Linker::<()>::new(&self.engine);
        let bindings = AriadnePlugin::instantiate(&mut store, &self.component, &linker)
            .map_err(anyhow::Error::from)
            .context("Failed to instantiate plugin for get_info")?;

        let info = bindings
            .ariadne_plugin_language_parser()
            .call_get_info(&mut store)
            .map_err(anyhow::Error::from)
            .context("Failed to call get-info export")?;

        debug!(
            "Plugin '{}' get-info: name={}, extensions={:?}",
            self.plugin_name, info.name, info.extensions
        );

        Ok(PluginLanguageInfo {
            name: info.name,
            extensions: info.extensions,
            version: info.version,
        })
    }

    /// Parse a source file through the plugin's `parse-file` export.
    ///
    /// Returns symbols, imports, and call-sites extracted from the source.
    pub fn parse_file(&self, source: &str, path: &str) -> Result<PluginParseResult> {
        let mut store = wasmtime::Store::new(&self.engine, ());
        // ISC-C8: Parsing gets more fuel than metadata queries.
        store
            .set_fuel(10_000_000_000)
            .map_err(anyhow::Error::from)
            .context("Failed to set fuel on store")?;

        let linker = wasmtime::component::Linker::<()>::new(&self.engine);
        let bindings = AriadnePlugin::instantiate(&mut store, &self.component, &linker)
            .map_err(anyhow::Error::from)
            .context("Failed to instantiate plugin for parse_file")?;

        let result = bindings
            .ariadne_plugin_language_parser()
            .call_parse_file(&mut store, source, path)
            .map_err(anyhow::Error::from)
            .context("Failed to call parse-file export")?;

        debug!(
            "Plugin '{}' parsed {} ({} symbols, {} imports, {} calls)",
            self.plugin_name,
            path,
            result.symbols.len(),
            result.imports.len(),
            result.calls.len()
        );

        Ok(PluginParseResult {
            symbols: result
                .symbols
                .into_iter()
                .map(|s| PluginSymbol {
                    name: s.name,
                    qualified_name: s.qualified_name,
                    kind: symbol_kind_str(&s.kind).to_string(),
                    line_start: s.line_start,
                    line_end: s.line_end,
                    is_exported: s.is_exported,
                })
                .collect(),
            imports: result
                .imports
                .into_iter()
                .map(|i| PluginImport {
                    imported_name: i.imported_name,
                    module_path: i.module_path,
                    line: i.line,
                })
                .collect(),
            calls: result
                .calls
                .into_iter()
                .map(|c| PluginCall {
                    caller_name: c.caller_name,
                    callee_name: c.callee_name,
                    line: c.line,
                })
                .collect(),
        })
    }

    /// Return the plugin's name (derived from filename).
    pub fn name(&self) -> &str {
        &self.plugin_name
    }
}

/// Map a WIT `symbol-kind` enum variant to a stable string representation.
fn symbol_kind_str(kind: &SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Method => "method",
        SymbolKind::Class => "class",
        SymbolKind::InterfaceType => "interface",
        SymbolKind::Module => "module",
        SymbolKind::Variable => "variable",
        SymbolKind::Constant => "constant",
        SymbolKind::TypeAlias => "type_alias",
        SymbolKind::EnumType => "enum",
        SymbolKind::TraitType => "trait",
    }
}

impl std::fmt::Debug for LoadedPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedPlugin")
            .field("plugin_name", &self.plugin_name)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_creates_engine() {
        let host = WasmPluginHost::new().expect("Engine creation should succeed");
        drop(host);
    }

    #[test]
    fn test_load_nonexistent_plugin_fails() {
        let host = WasmPluginHost::new().expect("Engine creation should succeed");
        let result = host.load_plugin(Path::new("/nonexistent/plugin.wasm"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_wasm_fails() {
        let host = WasmPluginHost::new().expect("Engine creation should succeed");

        let dir = std::env::temp_dir().join("ariadne-test-plugins");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("bad-plugin.wasm");
        std::fs::write(&path, b"this is not wasm").ok();

        let result = host.load_plugin(&path);
        assert!(result.is_err(), "Invalid WASM bytes should fail to compile");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_parse_result_default() {
        let result = PluginParseResult::default();
        assert!(result.symbols.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.calls.is_empty());
    }

    #[test]
    fn test_symbol_kind_str_coverage() {
        // Every variant must map to a non-empty string.
        let kinds = [
            SymbolKind::Function,
            SymbolKind::Method,
            SymbolKind::Class,
            SymbolKind::InterfaceType,
            SymbolKind::Module,
            SymbolKind::Variable,
            SymbolKind::Constant,
            SymbolKind::TypeAlias,
            SymbolKind::EnumType,
            SymbolKind::TraitType,
        ];
        for k in &kinds {
            assert!(!symbol_kind_str(k).is_empty());
        }
    }
}
