<h1 align="center">
  :thread: Ariadne
</h1>

<p align="center">
  <strong>The thread through the labyrinth -- universal dependency graph for AI coding agents</strong>
</p>

<p align="center">
  <img src="demo.gif" width="700" alt="Ariadne demo">
</p>

<p align="center">
  <a href="#install">Install</a> |
  <a href="#quick-start">Quick Start</a> |
  <a href="#features">Features</a> |
  <a href="#how-it-works">How It Works</a> |
  <a href="#mcp-integration">MCP Integration</a> |
  <a href="#configuration">Configuration</a>
</p>

<p align="center">
  <a href="https://crates.io/crates/ariadne-graph"><img src="https://img.shields.io/crates/v/ariadne-graph.svg" alt="Crates.io"></a>
  <a href="https://github.com/loremllc/ariadne/actions"><img src="https://github.com/loremllc/ariadne/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://docs.rs/ariadne-graph"><img src="https://docs.rs/ariadne-graph/badge.svg" alt="Docs"></a>
  <a href="https://github.com/loremllc/ariadne/blob/main/LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License"></a>
</p>

---

## Why?

AI coding agents are flying blind. They generate code without understanding how it connects to the rest of your codebase. Every "just add this function" risks breaking three callers. Every refactor is a game of whack-a-mole.

**Ariadne gives your AI agent a map of your entire codebase** -- every function, every call, every dependency, across every language, in milliseconds.

| Capability | Ariadne | Axon | Sourcegraph | CodeScene | dependency-cruiser |
|---|:---:|:---:|:---:|:---:|:---:|
| Polyglot (9+ languages) | Yes | No | Yes | No | JS only |
| Sub-second queries | Yes | Yes | No | No | Yes |
| MCP native | Yes | No | No | No | No |
| Blast radius analysis | Yes | No | Partial | Yes | No |
| Dead code detection | Yes | No | No | Yes | No |
| Affected test mapping | Yes | No | No | No | No |
| Cross-service tracing | Yes | No | Yes | No | No |
| Architectural rules | Yes | No | No | Yes | Yes |
| Community detection | Yes | No | No | Yes | No |
| SCIP export | Yes | No | Yes | No | No |
| File watcher + live re-index | Yes | No | No | No | No |
| Web dashboard | Yes | No | Yes | Yes | No |
| LSP server | Yes | No | No | No | No |
| Plugin system (WASM) | Yes | No | No | No | Yes |
| GitHub Action | Yes | No | No | Yes | Yes |
| Single binary, zero deps | Yes | No | No | No | No |

## Install

**Homebrew:**
```bash
brew install loremllc/tap/ariadne
```

**Cargo:**
```bash
cargo install ariadne-graph
```

**From source:**
```bash
git clone https://github.com/loremllc/ariadne
cd ariadne
cargo build --release
```

## Quick Start

```bash
# Index your codebase (takes seconds)
ariadne-graph index .

# Search for any symbol
ariadne-graph search "UserService"

# See what breaks if you change a function
ariadne-graph blast-radius "auth::validate_token" --depth 5

# Find dead code
ariadne-graph dead-code --threshold 80

# Which tests need to run for your changes?
ariadne-graph affected-tests --diff origin/main..HEAD

# Start the MCP server for your AI agent
ariadne-graph serve

# Launch the visual dashboard
ariadne-graph dash
```

## Features

### Dependency Graph Analysis
Ariadne builds a complete dependency graph of your codebase by parsing source files with tree-sitter grammars. It understands function calls, imports, class hierarchies, and API endpoints across nine languages.

### Blast Radius
Ask "what breaks if I change this?" and get an instant answer. Ariadne traces all direct and transitive dependents of any symbol, across files, modules, and services.

### Dead Code Detection
Find functions, classes, and methods that are never called. Ariadne uses graph reachability from entry points to identify unreachable code with confidence scores.

### Affected Tests
Map code changes to the tests that exercise them. Stop running your entire test suite -- run only what matters.

### Cross-Service Tracing
In a microservices architecture, Ariadne traces API calls across service boundaries. Define your services in `ariadne-workspace.toml` and see the full picture.

### Architectural Rules
Define boundaries in `ariadne.toml` and enforce them in CI. Prevent the kind of dependency spaghetti that makes codebases unmaintainable.

```toml
[[rules]]
name = "no-direct-db-from-handlers"
from = "src/handlers/**"
to = "src/db/**"
severity = "error"
```

### MCP Integration
Ariadne speaks the Model Context Protocol natively. Point your AI agent at Ariadne and it gains full codebase awareness:

```json
{
  "mcpServers": {
    "ariadne": {
      "command": "ariadne-graph",
      "args": ["serve"]
    }
  }
}
```

### LSP Server
Get dependency-aware editor features: go-to-definition that understands your whole graph, find-all-references across languages, and hover information showing blast radius.

```bash
ariadne-graph lsp
```

### Web Dashboard
Visual exploration of your dependency graph with interactive force-directed layouts, search, filtering, and drill-down.

```bash
ariadne-graph dash --port 1337
```

### Plugin System
Extend Ariadne with WASM plugins for custom languages or analysis passes. Plugins implement a simple WIT interface:

```wit
get-info: func() -> language-info;
parse-file: func(source: string, file-path: string) -> parse-result;
```

### File Watcher
Keep your index fresh automatically. Ariadne watches for file changes and incrementally re-indexes only what changed.

```bash
ariadne-graph watch --serve --dash
```

## How It Works

1. **Parse** -- Tree-sitter grammars extract symbols, calls, and imports from source files
2. **Store** -- Everything goes into a local SQLite database (`.ariadne.db`)
3. **Resolve** -- Import paths are resolved to actual symbols; call targets are linked
4. **Graph** -- An in-memory petgraph is built for fast traversal queries
5. **Query** -- Sub-millisecond lookups for any analysis: blast radius, dead code, affected tests
6. **Serve** -- Results are exposed via MCP, LSP, HTTP dashboard, or CLI

## Configuration

### Repository Config (`ariadne.toml`)

```toml
languages = ["python", "typescript", "go"]
exclude = ["vendor/**", "generated/**"]
entry_points = ["src/main.py", "src/index.ts"]

[[rules]]
name = "no-circular-imports"
from = "src/**"
to = "src/**"
type = "no-circular"
severity = "error"
```

### Workspace Config (`ariadne-workspace.toml`)

```toml
[[services]]
name = "api-gateway"
path = "gateway"
base_url = "http://localhost:8000"
language = "python"

[[services]]
name = "auth-service"
path = "auth"
base_url = "http://localhost:8001"
language = "typescript"
```

## GitHub Actions

```yaml
- uses: loremllc/ariadne/.github/actions/ariadne@main
  with:
    check-rules: 'true'
    dead-code-threshold: '10'
    affected-tests: 'true'
```

## Supported Languages

| Language | Parser | Symbol Extraction | Call Resolution | Import Resolution |
|---|---|---|---|---|
| Python | tree-sitter | Yes | Yes | Yes |
| JavaScript | tree-sitter | Yes | Yes | Yes |
| TypeScript | tree-sitter | Yes | Yes | Yes |
| Go | tree-sitter | Yes | Yes | Yes |
| Java | tree-sitter | Yes | Yes | Yes |
| Rust | tree-sitter | Yes | Yes | Yes |
| C# | tree-sitter | Yes | Yes | Yes |
| Ruby | tree-sitter | Yes | Yes | Yes |
| PHP | tree-sitter | Yes | Yes | Yes |

More languages can be added via WASM plugins.

## License

Licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
