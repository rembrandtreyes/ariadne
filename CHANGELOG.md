# Changelog

All notable changes to Ariadne will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-03-12

Initial open source release.

### Added

**Core graph engine**
- Universal dependency graph built on tree-sitter and SQLite
- Symbol extraction, call resolution, and import resolution pipeline
- Blast radius analysis — trace all transitive dependents of any symbol
- Dead code detection via graph reachability from exported entry points
- Affected test mapping — determine which tests cover a changed symbol
- Community detection (Louvain algorithm) for finding logical modules
- Structural coupling metrics between files and packages

**Language support (9 parsers)**
- Python, JavaScript, TypeScript, Go, Java, Rust, C#, Ruby, PHP

**MCP server** (Model Context Protocol)
- `search_symbol` — full-text and fuzzy symbol search
- `get_context` — callers, callees, imports, dead code status for any symbol
- `get_imports` — file-level import graph with resolution status
- `get_dependents` / `get_dependencies` — directed graph traversal
- `get_call_chain` — Mermaid diagram of full call tree
- `blast_radius` — categorized impact analysis (certain/likely/possible)
- `get_dead_code` — unreachable symbols with confidence scores
- `get_affected_tests` — tests to run for a given change set
- `check_arch_rules` — architectural boundary enforcement
- `get_community` — community/module membership for any symbol
- `get_service_topology` — cross-service API call tracing

**LSP server**
- Go-to-definition with full-graph awareness
- Find-all-references across languages
- Hover information showing blast radius summary

**Web dashboard**
- Interactive force-directed dependency graph
- Symbol search and drill-down
- Bind to 127.0.0.1 only; read-only endpoints

**File watcher**
- Incremental re-indexing on file change (inotify/FSEvents/kqueue)
- `--serve` and `--dash` flags to keep MCP server and dashboard live

**Architectural rules**
- TOML-based boundary definitions (`ariadne.toml`)
- `from` / `to` glob patterns with `error` / `warning` severity
- CI-friendly exit codes

**Cross-service topology**
- `ariadne-workspace.toml` multi-service configuration
- HTTP API call tracing across service boundaries
- OpenAPI and custom schema resolution

**SCIP export**
- Exports the dependency graph as a SCIP index for Sourcegraph and other tools

**Plugin system** (optional feature flag)
- WASM plugin interface for custom language parsers
- WIT-defined `get-info` / `parse-file` interface

**Output formats**
- Plain text, JSON, Mermaid diagram

**Developer experience**
- Single binary, zero runtime dependencies (SQLite bundled)
- `cargo install ariadne`
- Homebrew tap: `brew install loremllc/tap/ariadne`
- Multi-platform CI (Linux, macOS, Windows)
- GitHub Action for CI integration

[0.1.0]: https://github.com/loremllc/ariadne/releases/tag/v0.1.0
