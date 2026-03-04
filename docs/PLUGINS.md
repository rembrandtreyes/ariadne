# Ariadne Plugin System

Ariadne supports extending language support through **WebAssembly (WASM) plugins**.
Plugins implement a stable ABI contract defined in WIT (WebAssembly Interface Types),
allowing any language that compiles to `wasm32-wasip1` to add first-class parsing
support to Ariadne.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [The WIT Contract](#the-wit-contract)
3. [Step-by-Step: Build a Plugin](#step-by-step-build-a-plugin)
4. [Security Model](#security-model)
5. [Performance Characteristics](#performance-characteristics)
6. [Plugin Discovery Order](#plugin-discovery-order)
7. [Troubleshooting](#troubleshooting)

---

## Architecture Overview

```
 Your Code (.kt, .scala, etc.)
        |
        v
 +-----------------+
 | Ariadne Host    |  <-- Rust runtime with wasmtime
 |  Plugin Registry|
 +-----------------+
        |
        v  (calls via WIT ABI)
 +-----------------+
 | WASM Plugin     |  <-- Your plugin (.wasm component)
 | get_info()      |
 | parse_file()    |
 +-----------------+
```

Each plugin is a **WASM component** that exports exactly two functions:

- **`get-info()`** -- Returns metadata: language name, file extensions, version.
- **`parse-file(source, file-path)`** -- Parses source code and returns symbols,
  imports, calls, API endpoints, and API call sites.

The host loads plugins at startup, queries `get-info()` to learn which file
extensions each plugin handles, and dispatches `parse-file()` when those files
are encountered during graph construction.

---

## The WIT Contract

This is the canonical contract every plugin must implement. It lives at
`src/plugins/wit/ariadne-plugin.wit` in the Ariadne repository.

```wit
// ariadne-plugin.wit — the stable ABI contract

package ariadne:plugin@1.0.0;

interface language-parser {
    // Metadata
    record language-info {
        name: string,
        extensions: list<string>,
        version: string,
    }

    // Parsed output types (match Ariadne's internal types)
    record parsed-symbol {
        name: string,
        qualified-name: string,
        kind: symbol-kind,
        line-start: u32,
        line-end: u32,
        is-exported: bool,
        signature: string,
        decorators: list<string>,
        parent-name: option<string>,
    }

    record parsed-import {
        imported-name: string,
        module-path: string,
        line: u32,
        is-external: bool,
    }

    record parsed-call {
        caller-name: string,
        callee-name: string,
        line: u32,
    }

    record api-endpoint {
        method: string,
        path-pattern: string,
        handler-name: string,
        line: u32,
    }

    record api-call-site {
        method: string,
        url-pattern: string,
        caller-name: string,
        line: u32,
        is-dynamic: bool,
    }

    enum symbol-kind {
        function,
        method,
        class,
        interface,
        module,
        variable,
        constant,
        type-alias,
        enum-type,
        trait-type,
    }

    record parse-result {
        symbols: list<parsed-symbol>,
        imports: list<parsed-import>,
        calls: list<parsed-call>,
        api-endpoints: list<api-endpoint>,
        api-calls: list<api-call-site>,
    }

    // The two functions every plugin must implement
    get-info: func() -> language-info;
    parse-file: func(source: string, file-path: string) -> parse-result;
}

world ariadne-plugin {
    export language-parser;
}
```

### Key Types

| Type | Purpose |
|------|---------|
| `language-info` | Plugin metadata -- name, extensions, version |
| `parsed-symbol` | A function, class, interface, or other named entity |
| `parsed-import` | An import/dependency reference |
| `parsed-call` | A function/method call relationship |
| `api-endpoint` | An HTTP endpoint definition (e.g., `@GetMapping`) |
| `api-call-site` | An outbound HTTP call (e.g., `httpClient.get(...)`) |
| `symbol-kind` | Enum: function, method, class, interface, module, variable, constant, type-alias, enum-type, trait-type |

---

## Step-by-Step: Build a Plugin

### Prerequisites

- Rust toolchain with `wasm32-wasip1` target:
  ```bash
  rustup target add wasm32-wasip1
  ```
- `cargo-component` for building WASM components:
  ```bash
  cargo install cargo-component
  ```

### 1. Scaffold the Project

```bash
mkdir ariadne-kotlin && cd ariadne-kotlin
mkdir -p src wit
```

Create `Cargo.toml`:

```toml
[package]
name = "ariadne-kotlin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = { version = "0.36", default-features = false }

[package.metadata.component]
package = "ariadne:plugin"

[package.metadata.component.target]
world = "ariadne-plugin"
path = "wit"
```

### 2. Copy the WIT Contract

Copy `ariadne-plugin.wit` into `wit/ariadne-plugin.wit`. This defines the ABI
your plugin must satisfy.

### 3. Implement the Plugin

In `src/lib.rs`, use `wit_bindgen::generate!` to produce the Rust bindings and
implement the `Guest` trait:

```rust
// Generate bindings from the WIT contract
wit_bindgen::generate!({
    world: "ariadne-plugin",
    path: "wit",
});

struct KotlinPlugin;

// Export this struct as the implementation
export!(KotlinPlugin);

impl Guest for KotlinPlugin {
    fn get_info() -> LanguageInfo {
        LanguageInfo {
            name: "kotlin".to_string(),
            extensions: vec![".kt".to_string(), ".kts".to_string()],
            version: "0.1.0".to_string(),
        }
    }

    fn parse_file(source: String, _file_path: String) -> ParseResult {
        // Your parsing logic here
        // ...
    }
}
```

See `examples/ariadne-kotlin/` for a complete working example.

### 4. Build the Plugin

```bash
cargo component build --release
```

This produces a WASM component at:
```
target/wasm32-wasip1/release/ariadne_kotlin.wasm
```

### 5. Install the Plugin

Copy the `.wasm` file to one of the plugin directories:

```bash
# Repo-local (highest priority)
mkdir -p .ariadne/plugins
cp target/wasm32-wasip1/release/ariadne_kotlin.wasm .ariadne/plugins/

# Or user-global
mkdir -p ~/.ariadne/plugins
cp target/wasm32-wasip1/release/ariadne_kotlin.wasm ~/.ariadne/plugins/
```

### 6. Test It

Run Ariadne on a project containing `.kt` files:

```bash
ariadne analyze ./my-kotlin-project
```

Ariadne will automatically discover and load your plugin for `.kt` and `.kts` files.

---

## Security Model

Plugins run inside a **strict WASM sandbox** with the following guarantees:

| Capability | Status |
|------------|--------|
| Filesystem access | **Denied** -- plugins cannot read or write files |
| Network access | **Denied** -- no sockets, no HTTP calls |
| System calls | **Denied** -- no process spawning, no environment variables |
| Memory | **Isolated** -- each plugin gets its own linear memory |
| CPU time | **Bounded** -- host enforces execution time limits |

### What Plugins Receive

Plugins receive **only** the data passed through the WIT interface:

- `get-info()` -- no arguments, returns metadata.
- `parse-file(source, file-path)` -- receives the file contents as a string and
  the file path (for context). The plugin cannot access the filesystem itself.

### Why This Matters

- **Untrusted plugins are safe to load.** A malicious plugin cannot exfiltrate
  code, phone home, or modify your repository.
- **Deterministic execution.** Given the same inputs, a plugin must produce the
  same outputs. No hidden state, no side effects.
- **Crash isolation.** If a plugin traps (panics), only that plugin's execution
  fails. The host continues processing with built-in parsers.

---

## Performance Characteristics

WASM plugins run at approximately **70-80% of native tree-sitter speed**. This
is the expected performance profile:

| Metric | Native (tree-sitter) | WASM Plugin |
|--------|---------------------|-------------|
| Parse throughput | Baseline (100%) | ~70-80% |
| Memory overhead | Shared process | +2-4 MB per plugin instance |
| Startup cost | None (linked) | ~5-15ms (JIT compilation) |
| Subsequent calls | Native speed | Near-native (JIT cached) |

### Why 70-80%?

- **wasmtime JIT** compiles WASM to native code on first invocation, then caches.
- The primary overhead comes from **ABI marshaling** -- copying strings and
  structs across the host/guest boundary.
- For typical source files (< 10K lines), the difference is **imperceptible**
  (sub-millisecond).

### Optimization Tips

- Minimize allocations in `parse-file()`. Reuse buffers where possible.
- Return only what you find. Empty vectors for unused fields (e.g., `api_endpoints`)
  have near-zero cost.
- Prefer simple string matching over complex regex for line-by-line parsers.
  Regex engines add significant WASM binary size.

---

## Plugin Discovery Order

Ariadne searches for plugins in the following order. **The first match wins** --
if two plugins handle the same extension, the higher-priority one is used.

| Priority | Location | Scope |
|----------|----------|-------|
| 1 (highest) | `.ariadne/plugins/` | Repository-local |
| 2 | `~/.ariadne/plugins/` | User-global |
| 3 (lowest) | Built-in parsers | Compiled into Ariadne |

### How Discovery Works

1. On startup, Ariadne scans each plugin directory for `*.wasm` files.
2. For each `.wasm` file, it instantiates the component and calls `get-info()`.
3. The returned `extensions` list is registered in the plugin registry.
4. If a built-in parser already handles an extension, the plugin **overrides** it.

This means you can:

- **Override built-in parsers** by placing a plugin in `.ariadne/plugins/`.
- **Share plugins across projects** by placing them in `~/.ariadne/plugins/`.
- **Pin per-repo plugins** that are versioned with your codebase.

---

## Troubleshooting

### Plugin not loading

- Verify the file has a `.wasm` extension.
- Check that `get-info()` returns valid extensions (must start with `.`).
- Run with `ARIADNE_LOG=debug` to see plugin loading output.

### Parse errors

- Plugins must return a valid `parse-result` even for empty or malformed files.
  Return empty vectors rather than trapping.
- If `parse-file()` traps, Ariadne logs a warning and skips the file.

### Build failures

- Ensure you have `wasm32-wasip1` target: `rustup target add wasm32-wasip1`
- Ensure `cargo-component` is installed: `cargo install cargo-component`
- Verify your `wit/` directory contains `ariadne-plugin.wit`.
