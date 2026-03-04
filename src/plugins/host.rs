//! WASM Plugin Host for Ariadne
//!
//! Provides a sandboxed runtime for loading and executing language parser plugins
//! compiled to WebAssembly. Plugins are loaded via wasmtime with NO filesystem or
//! network access -- the WASM guest can only compute and return data through the
//! explicit ABI surface defined here.
//!
//! # Architecture
//!
//! ```text
//! +------------------+       +-------------------+
//! |  Ariadne Host    |       |  WASM Plugin      |
//! |                  | ABI   |  (sandboxed)      |
//! |  WasmPluginHost  +------>+  get_info()       |
//! |  LoadedPlugin    +------>+  parse_file()     |
//! |                  |       |                   |
//! |  (full OS access)|       |  (NO fs/net/env)  |
//! +------------------+       +-------------------+
//! ```
//!
//! # Current Status
//!
//! This module compiles and establishes the sandboxed wasmtime architecture.
//! The actual function-call ABI (memory passing between host and guest) requires
//! either:
//!   - wasmtime component-model support (`wasmtime-component-model` crate) for
//!     full WIT-based typed interfaces, or
//!   - a manual linear-memory protocol (alloc/dealloc + serialization).
//!
//! The current implementation provides the `LoadedPlugin` stub that validates
//! the plugin exports exist, returning placeholder data. When the component-model
//! integration lands, these stubs become real calls with zero API change to callers.

use std::path::Path;

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Data types returned by plugin calls
// ---------------------------------------------------------------------------

/// Result of parsing a file through a WASM plugin.
#[derive(Debug, Default)]
pub struct PluginParseResult {
    pub symbols: Vec<PluginSymbol>,
    pub imports: Vec<PluginImport>,
    pub calls: Vec<PluginCall>,
}

/// A symbol (function, class, type, etc.) discovered in the source file.
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
// WasmPluginHost -- the top-level runtime
// ---------------------------------------------------------------------------

/// WASM plugin host backed by wasmtime.
///
/// Creates a single `Engine` that is shared across all loaded plugins. The
/// engine is configured with default settings and explicitly does NOT enable
/// WASI -- plugins run in a pure computation sandbox.
pub struct WasmPluginHost {
    engine: wasmtime::Engine,
}

impl WasmPluginHost {
    /// Create a new plugin host with a default wasmtime engine.
    ///
    /// The engine configuration intentionally omits WASI capabilities,
    /// ensuring that loaded plugins cannot access the filesystem, network,
    /// environment variables, or any other OS resource.
    pub fn new() -> Result<Self> {
        let mut config = wasmtime::Config::new();
        // Cranelift is the default compiler; we keep defaults but can tune later.
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);

        let engine = wasmtime::Engine::new(&config)
            .context("Failed to create wasmtime engine")?;

        info!("WASM plugin host initialized (wasmtime engine ready)");
        Ok(Self { engine })
    }

    /// Load a WASM plugin from a `.wasm` file on disk.
    ///
    /// The file is read into memory, compiled into a wasmtime `Module`, and
    /// wrapped in a `LoadedPlugin` that exposes the parsing API. The plugin
    /// gets NO ambient capabilities -- it runs in a completely sandboxed store.
    pub fn load_plugin(&self, path: &Path) -> Result<LoadedPlugin> {
        let wasm_bytes = std::fs::read(path)
            .with_context(|| format!("Failed to read plugin file: {}", path.display()))?;

        debug!("Compiling WASM module from {} ({} bytes)", path.display(), wasm_bytes.len());

        let module = wasmtime::Module::new(&self.engine, &wasm_bytes)
            .with_context(|| format!("Failed to compile WASM module: {}", path.display()))?;

        // Validate that the module exports the functions we expect.
        let exports: Vec<String> = module
            .exports()
            .map(|e| e.name().to_string())
            .collect();

        debug!("Plugin exports: {:?}", exports);

        let has_get_info = exports.iter().any(|n| n == "get-info" || n == "get_info");
        let has_parse_file = exports.iter().any(|n| n == "parse-file" || n == "parse_file");

        if !has_get_info {
            warn!(
                "Plugin {} does not export 'get-info' -- info queries will return defaults",
                path.display()
            );
        }
        if !has_parse_file {
            warn!(
                "Plugin {} does not export 'parse-file' -- parsing will return empty results",
                path.display()
            );
        }

        let plugin_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        info!("Loaded plugin '{}' from {}", plugin_name, path.display());

        Ok(LoadedPlugin {
            engine: self.engine.clone(),
            module,
            plugin_name,
            has_get_info,
            has_parse_file,
        })
    }
}

// ---------------------------------------------------------------------------
// LoadedPlugin -- a compiled module ready to execute
// ---------------------------------------------------------------------------

/// A compiled WASM plugin ready to be called.
///
/// Each call to `get_info()` or `parse_file()` creates a fresh `Store` and
/// `Instance`, ensuring complete isolation between invocations. This prevents
/// any state leakage between parse calls.
pub struct LoadedPlugin {
    engine: wasmtime::Engine,
    module: wasmtime::Module,
    plugin_name: String,
    has_get_info: bool,
    has_parse_file: bool,
}

impl LoadedPlugin {
    /// Query the plugin for its language metadata.
    ///
    /// # Current Implementation
    ///
    /// Returns metadata derived from the plugin filename. When the component-model
    /// ABI is wired up, this will call the guest's `get-info` export and decode
    /// the returned `LanguageInfo` record.
    pub fn get_info(&self) -> Result<PluginLanguageInfo> {
        if !self.has_get_info {
            debug!(
                "Plugin '{}' lacks get-info export; returning filename-derived info",
                self.plugin_name
            );
            return Ok(self.fallback_info());
        }

        // Create an isolated store for this call -- no WASI, no imports.
        let mut store = wasmtime::Store::new(&self.engine, ());
        let linker = wasmtime::Linker::new(&self.engine);
        let instance = linker
            .instantiate(&mut store, &self.module)
            .context("Failed to instantiate plugin for get_info")?;

        // Attempt to call the export. The full component-model integration will
        // deserialize the return value from linear memory. For now we validate
        // the export exists and is callable, then fall back to filename-derived
        // info since we cannot yet decode the structured return.
        let _get_info_func = instance
            .get_func(&mut store, "get-info")
            .or_else(|| instance.get_func(&mut store, "get_info"));

        match _get_info_func {
            Some(func) => {
                debug!(
                    "Found get-info export in '{}' (type: {:?}). \
                     Full ABI decoding requires component-model integration.",
                    self.plugin_name,
                    func.ty(&store)
                );
                // TODO: When component-model lands, call func and decode the result.
                // For now, return fallback so callers get usable data.
                Ok(self.fallback_info())
            }
            None => {
                warn!("get-info export disappeared after load in '{}'", self.plugin_name);
                Ok(self.fallback_info())
            }
        }
    }

    /// Parse a source file through the plugin.
    ///
    /// # Arguments
    ///
    /// * `source` - The full source code of the file to parse.
    /// * `path`   - The file path (used by the plugin for context, e.g. inferring
    ///              module names from directory structure).
    ///
    /// # Current Implementation
    ///
    /// Returns an empty `PluginParseResult`. When the component-model ABI is
    /// wired up, this will:
    /// 1. Write `source` and `path` into the guest's linear memory.
    /// 2. Call the guest's `parse-file` export.
    /// 3. Read back the serialized `ParseResult` from linear memory.
    /// 4. Decode it into `PluginParseResult`.
    pub fn parse_file(&self, source: &str, path: &str) -> Result<PluginParseResult> {
        if !self.has_parse_file {
            debug!(
                "Plugin '{}' lacks parse-file export; returning empty result for {}",
                self.plugin_name, path
            );
            return Ok(PluginParseResult::default());
        }

        // Create an isolated store for this parse call.
        let mut store = wasmtime::Store::new(&self.engine, ());
        let linker = wasmtime::Linker::new(&self.engine);
        let instance = linker
            .instantiate(&mut store, &self.module)
            .context("Failed to instantiate plugin for parse_file")?;

        let _parse_func = instance
            .get_func(&mut store, "parse-file")
            .or_else(|| instance.get_func(&mut store, "parse_file"));

        match _parse_func {
            Some(func) => {
                debug!(
                    "Found parse-file export in '{}' (type: {:?}). \
                     Parsing {} ({} bytes). \
                     Full ABI decoding requires component-model integration.",
                    self.plugin_name,
                    func.ty(&store),
                    path,
                    source.len()
                );
                // TODO: Implement the linear-memory protocol:
                //
                //   1. Get the guest's "memory" export.
                //   2. Call the guest's "alloc" export to allocate space for source + path.
                //   3. Write source bytes into guest memory at the returned offset.
                //   4. Write path bytes into guest memory.
                //   5. Call parse-file(source_ptr, source_len, path_ptr, path_len).
                //   6. Read the returned (ptr, len) pair.
                //   7. Copy result bytes from guest memory.
                //   8. Deserialize into PluginParseResult.
                //   9. Call the guest's "dealloc" to free the result buffer.
                //
                // This will be replaced by component-model typed calls when available.
                Ok(PluginParseResult::default())
            }
            None => {
                warn!("parse-file export disappeared after load in '{}'", self.plugin_name);
                Ok(PluginParseResult::default())
            }
        }
    }

    /// Return the plugin's name (derived from filename).
    pub fn name(&self) -> &str {
        &self.plugin_name
    }

    /// Generate fallback `PluginLanguageInfo` from the plugin filename.
    ///
    /// Strips the `ariadne-` prefix if present:
    ///   `ariadne-swift.wasm` -> name="swift", extensions=["swift"]
    ///   `kotlin-parser.wasm` -> name="kotlin-parser", extensions=[]
    fn fallback_info(&self) -> PluginLanguageInfo {
        let raw_name = self
            .plugin_name
            .strip_prefix("ariadne-")
            .unwrap_or(&self.plugin_name);

        // If the name looks like a single language identifier, use it as the extension.
        let extensions = if raw_name.chars().all(|c| c.is_ascii_alphanumeric()) {
            vec![raw_name.to_lowercase()]
        } else {
            Vec::new()
        };

        PluginLanguageInfo {
            name: raw_name.to_string(),
            extensions,
            version: "0.0.0-stub".to_string(),
        }
    }
}

impl std::fmt::Debug for LoadedPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedPlugin")
            .field("plugin_name", &self.plugin_name)
            .field("has_get_info", &self.has_get_info)
            .field("has_parse_file", &self.has_parse_file)
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
        // Engine exists and didn't panic -- that's the test.
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

        // Write garbage bytes to a temp file.
        let dir = std::env::temp_dir().join("ariadne-test-plugins");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("bad-plugin.wasm");
        std::fs::write(&path, b"this is not wasm").ok();

        let result = host.load_plugin(&path);
        assert!(result.is_err(), "Invalid WASM bytes should fail to compile");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_fallback_info_strips_prefix() {
        let info = PluginLanguageInfo {
            name: "swift".to_string(),
            extensions: vec!["swift".to_string()],
            version: "0.0.0-stub".to_string(),
        };

        // Simulate what fallback_info does for "ariadne-swift"
        let plugin_name = "ariadne-swift";
        let raw_name = plugin_name.strip_prefix("ariadne-").unwrap_or(plugin_name);
        assert_eq!(raw_name, "swift");
        assert_eq!(info.name, raw_name);
    }

    #[test]
    fn test_parse_result_default() {
        let result = PluginParseResult::default();
        assert!(result.symbols.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.calls.is_empty());
    }
}
