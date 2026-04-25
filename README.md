<h1 align="center">
  :thread: Ariadne
</h1>

<p align="center">
  <strong>The thread through the labyrinth -- universal dependency graph for AI coding agents</strong>
</p>


<p align="center">
  <a href="#install">Install</a> |
  <a href="#quick-start">Quick Start</a> |
  <a href="#features">Features</a> |
  <a href="#how-it-works">How It Works</a> |
  <a href="#mcp-integration">MCP Integration</a> |
  <a href="#claude-code-integration">Claude Code</a> |
  <a href="#performance">Performance</a> |
  <a href="#configuration">Configuration</a>
</p>

<p align="center">
  <a href="https://crates.io/crates/ariadne"><img src="https://img.shields.io/crates/v/ariadne.svg" alt="Crates.io"></a>
  <a href="https://github.com/loremllc/ariadne/actions"><img src="https://github.com/loremllc/ariadne/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://docs.rs/ariadne"><img src="https://docs.rs/ariadne/badge.svg" alt="Docs"></a>
  <a href="https://github.com/loremllc/ariadne/blob/main/LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License"></a>
</p>

---

## Why?

AI coding agents are flying blind. They generate code without understanding how it connects to the rest of your codebase. Every "just add this function" risks breaking three callers. Every refactor is a game of whack-a-mole.

**Ariadne gives your AI agent a map of your entire codebase** -- every function, every call, every dependency, across every language, in milliseconds.

The landscape of code intelligence for AI agents moved fast in 2025-2026. Here's where Ariadne sits against the current generation of graph/memory backends:

| Capability | Ariadne | GitNexus | Codebase-Memory | Greptile | Potpie | Understand-Anything | code-review-graph |
|---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| Deterministic parsing (no LLM, no embeddings) | Yes | Yes | Yes | No | No | No | Partial |
| Resolved call edges (not semantic similarity) | Yes | Yes | Yes | No | Partial | Yes | Yes |
| Local-only, no cloud/API required | Yes | Yes | Yes | No | No | Yes | Yes |
| Sub-5ms resident query latency | Yes | Partial | Yes | No | No | No | No |
| MCP server | Yes | Yes | Yes | No | Partial | Yes | Yes |
| LSP server | Yes | No | No | No | No | No | No |
| REST dashboard | Yes | No | No | No | No | Yes | No |
| CLI in the same binary | Yes | Yes | Yes | No | No | No | No |
| Blast radius with certainty tiers (WILL/MAY break) | Yes | Partial | No | No | No | No | Yes |
| Temporal + structural fusion (git churn × fan-in) | Yes | No | No | No | No | No | No |
| Ordered affected-tests mapping | Yes | No | No | No | No | No | Yes |
| Community detection (graph clustering) | Yes | No | No | No | No | No | No |
| Architectural rules enforcement | Yes | No | No | No | No | No | No |
| SCIP export (Sourcegraph interop) | Yes | No | No | No | No | No | No |
| Cross-service API tracing | Yes | No | No | No | Partial | No | No |
| File watcher + incremental re-index | Yes | No | Partial | No | No | No | No |
| Single static binary, zero runtime deps | Yes | No | Yes | No | No | No | No |

Ariadne's moat is the **fusion** — no single competitor combines a resolved deterministic call graph with git-temporal history, community detection, architectural boundary enforcement, SCIP export, an LSP, AND a sub-5ms-latency MCP server in one local static binary. RAG/embedding systems (Greptile, Potpie) can't produce directed call graphs. Pure graph tools (GitNexus, Codebase-Memory) don't fuse temporal signal. Ariadne does both, offline, at agent-loop speed.

## Install

**Homebrew:**
```bash
brew install loremllc/tap/ariadne
```

**Cargo:**
```bash
cargo install ariadne
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
ariadne index .

# Search for any symbol
ariadne search "UserService"

# See what breaks if you change a function
ariadne blast-radius "auth::validate_token" --depth 5

# Find dead code
ariadne dead-code --threshold 80

# Which tests need to run for your changes?
ariadne affected-tests --diff origin/main..HEAD

# Start the MCP server for your AI agent
ariadne serve

# Launch the visual dashboard
ariadne dash
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
      "command": "ariadne",
      "args": ["serve"]
    }
  }
}
```

### LSP Server
Get dependency-aware editor features: go-to-definition that understands your whole graph, find-all-references across languages, and hover information showing blast radius.

```bash
ariadne lsp
```

### Web Dashboard
Visual exploration of your dependency graph with interactive force-directed layouts, search, filtering, and drill-down.

```bash
ariadne dash --port 1337
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
ariadne watch --serve --dash
```

## Claude Code Integration

Ariadne is purpose-built for AI coding agents. Add it to Claude Code's MCP config and your agent gains full codebase awareness — no more blind grep loops.

**`~/.claude/claude_desktop_config.json` (or Claude Code MCP settings):**

```json
{
  "mcpServers": {
    "ariadne": {
      "command": "ariadne",
      "args": ["serve"]
    }
  }
}
```

Run `ariadne index .` once in your project root first. The MCP server reads from the local `.ariadne.db` index.

### Available MCP Tools

Ariadne ships 32 MCP tools grouped by intent. See [`docs/AGENT-GUIDE.md`](docs/AGENT-GUIDE.md) for decision-tree guidance on which tool to pick for which task.

**Onboarding & triage** — "what am I looking at?"

| Tool | What it answers |
|---|---|
| `get_entry_points` | "Where does execution start — `main`, HTTP handlers, framework callbacks?" |
| `get_god_objects` | "Which symbols are most depended on? Handle these carefully." |
| `get_codebase_health` | "Letter grade A–F with dead code ratio, cycles, coupling, modularity." |
| `compute_file_risk` | "Per-file 0–1 risk score combining churn, coupling, fan-in, dead-proximity." |
| `get_complexity_hotspots` | "Top 50 symbols by combined fan-in × fan-out × churn × volatility." |

**Change impact** — "what breaks if I do X?"

| Tool | What it answers |
|---|---|
| `blast_radius` | "What breaks if I change this — categorized by certainty (WILL / MAY)." |
| `diff_impact` | "Given changed files: affected symbols + blast radius + affected tests in one call." |
| `affected_tests` | "Minimum test set that transitively covers changed files." |
| `propose_edit_plan` | "Ordered edit sequence for callers when refactoring a symbol — leaves-first, with cycle detection." |
| `get_dependency_path` | "Shortest directed call path from symbol A to symbol B." |

**Deep-dive on a specific symbol** — "explain this code"

| Tool | What it answers |
|---|---|
| `why_symbol` | "One narrative: role, callers, callees, blast radius, coupled files." |
| `get_symbol_history` | "Creation date, last modification, author count, volatility — git temporal." |
| `get_symbol_health` | "0–1 score fusing stability, connectivity, dead-code status." |
| `get_heritage` | "Inheritance hierarchy: parent classes/interfaces + child subclasses." |
| `get_execution_flows` | "Ordered call paths from entry points that pass through this symbol." |

**Navigation** — "find the thing"

| Tool | What it answers |
|---|---|
| `search_symbol` | "Find anything named X across the whole codebase (FTS + fuzzy)." |
| `get_context` | "Callers, callees, file, signature, dead status, coupled files." |
| `get_imports` | "All imports for a file with resolution status." |
| `get_file_summary` | "Every symbol and import in a file." |
| `get_dependents` | "Upstream callers of a symbol." |
| `get_dependencies` | "Downstream callees of a symbol." |
| `get_file_dependencies` | "Files this file depends on (with connecting symbol pairs + transitive depth)." |
| `get_file_dependents` | "Files that depend on this file (with connecting symbol pairs + transitive depth)." |

**Structural exploration** — "what's the shape?"

| Tool | What it answers |
|---|---|
| `get_call_chain` | "Full call tree from a symbol, rendered as Mermaid flowchart." |
| `detect_cycles` | "Circular dependencies via Kosaraju SCC — returns cycles with symbols + length." |
| `get_boundaries` | "Module boundaries: internal vs cross-boundary call details + modularity scores." |
| `get_coupling` | "Top coupled file pairs by git co-change strength (implicit dependencies)." |
| `get_communities` | "Detected module communities with symbol counts + modularity scores." |
| `get_api_endpoints` | "All detected HTTP/RPC endpoints with method, path, handler, file." |
| `get_code_smells` | "Volatility spikes, bottlenecks, shotgun-surgery risks, dead code." |
| `find_dead_code` | "Unreachable functions and methods (never called from any entry point)." |
| `get_complexity` | "Codebase-wide counts: files, symbols, calls, dead, resolution rate, languages." |

### Before and After

**Without Ariadne** — agent wants to know what breaks if `generatePost` is renamed:
```
1. rg "generatePost" src/           # scan all files
2. Read each match file              # 6 LLM context window reads
3. Trace callers manually            # miss transitive dependents
4. Ask again: "what calls those?"    # another round-trip
5. Still uncertain about indirect callers
Total: 4-6 LLM turns, partial picture
```

**With Ariadne** — same question:
```
blast_radius("generatePost")
→ WILL BREAK (direct): [PostEditor, usePostDraft, ScheduleModal]
→ MAY BREAK (indirect): [PublishQueue, analytics/track]
Total: 1 tool call, complete picture, <5ms
```

### Keep your index fresh

Run Ariadne as a background service that watches for file changes and keeps the MCP server alive:

```bash
ariadne watch --serve
```

This does two things at once: re-indexes only changed files when they save, and keeps the MCP server resident so queries are <2ms instead of ~20ms cold-start per call.

For CI or one-shot use, `ariadne serve` starts the server against a static index.

### Steer your AI agent

Claude Code and other MCP-compatible agents will pick up Ariadne's tools automatically, but adding a `CLAUDE.md` (or equivalent project instruction file) gets you consistent tool preference and real token savings:

```markdown
# Code Search — Use Ariadne MCP Tools

Ariadne is indexed for this codebase. Prefer Ariadne MCP tools over
built-in Grep/Glob/Read for code navigation. Built-in tools read raw
files and burn tokens; Ariadne returns pre-resolved semantic data in
one call.

## Tool Substitution Table

| Instead of this              | Use this Ariadne tool   |
|------------------------------|-------------------------|
| Grep for a function/class    | `search_symbol`         |
| Read a file for its shape    | `get_file_summary`      |
| Multiple Reads to trace callers | `get_context`        |
| Grep to find who uses symbol | `get_dependents`        |
| Grep to assess change impact | `blast_radius`          |
| Glob/stats on the codebase   | `get_complexity`        |

## When to use built-in tools

- Reading a file to understand *implementation* (not structure)
- Writing or editing files — Ariadne is read-only
- Searching for string literals, comments, or patterns (not symbols)
- Before `ariadne index .` has been run
```

Typical token reduction: a file-structure exploration that costs ~3K tokens via `Read` costs ~150 tokens via `get_file_summary`. A caller-trace across 6 files via `Grep + Read` costs ~12K tokens; `get_context` returns the same data in ~400 tokens.

### Setup checklist

```bash
# 1. Index your project (run once per project, re-run after major changes)
ariadne index .

# 2. Add to .gitignore — the DB is local, not for committing
echo ".ariadne.db" >> .gitignore

# 3. Start the watch server (persistent MCP + live re-index)
ariadne watch --serve

# 4. Verify Claude Code sees the tools — in a Claude Code session:
#    "List the available Ariadne MCP tools"
#    Should return: search_symbol, get_context, blast_radius, ...
```

Add `ariadne watch --serve` to your project's dev startup script (Makefile, `package.json` scripts, Procfile) so it runs alongside your dev server.

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

## Performance

Benchmarked on **voicepost** — a real-world Next.js/TypeScript codebase:
267 source files · 4,652 symbols · ~48K lines of code

| Operation | Ariadne | grep (text scan) | Notes |
|---|---|---|---|
| Cold index | 0.73s | — | tree-sitter parse + SQLite write, 267 files |
| Re-index | ~0.7s | — | full re-parse (incremental tracking in progress) |
| Symbol search (MCP) | <2ms | 17ms | FTS5 SQLite vs. full text scan |
| Blast radius (MCP) | <5ms | N/A | graph traversal, 82 dependents found |
| Symbol search (CLI) | ~20ms | 17ms | includes binary cold-start; MCP server is resident |

Ariadne's query latency is **sub-millisecond** after the initial index because:
- Symbols, calls, and imports are pre-resolved into SQLite rows
- An in-memory petgraph handles graph traversal at O(V+E)
- FTS5 full-text index on symbol names enables fuzzy search without scanning files

### Scale Projections

| Codebase size | Index time | Search (MCP) | Blast radius (MCP) |
|---|---|---|---|
| Small (1K symbols) | <1s | <1ms | <1ms |
| Medium (10K symbols) | ~5s | <5ms | <5ms |
| Large (100K symbols) | ~45s | <10ms | <20ms |
| Monorepo (1M symbols) | ~8min | <15ms | <50ms |

Index time scales with file count (parse cost). Query time scales with graph density (edge count), not file count — a 1M-symbol query is still under 50ms because petgraph traversal is O(V+E) from the root node, not a full-graph scan.

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
