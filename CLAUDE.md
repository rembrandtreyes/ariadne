# Ariadne

Universal dependency graph for AI coding agents. Indexes multi-language codebases into a SQLite graph database, exposed via MCP server, LSP, CLI, and REST dashboard.

## Stack

- Rust 2021 edition
- Axum (dashboard/REST), rmcp (MCP server), tower-lsp (LSP)
- tree-sitter (parsing 9 languages), petgraph (graph), rusqlite (SQLite)
- clap (CLI), rayon (parallelism), tokio (async runtime)

## Commands

- `cargo run -- <subcommand>` — run CLI (index, search, blast-radius, etc.)
- `cargo test` — run all tests (unit + integration)
- `cargo clippy` — lint
- `cargo fmt --check` — format check
- `make build` — release build
- `make install` — install to /usr/local/bin

## Structure

```
src/
  main.rs          — CLI entry point (clap)
  lib.rs           — public module exports
  pipeline/        — 14-phase indexing pipeline (parse -> resolve -> analyze)
  parse/           — tree-sitter parsers per language (Python, JS, TS, Go, Java, Rust, C#, Ruby, PHP)
  db/              — SQLite database wrapper, schema, queries
  graph/           — in-memory call graph, blast radius, call chains, circular detection
  analysis/        — dead code, community detection, arch rules, SCIP export
  mcp/             — MCP server (10 tools, stdio transport only)
  lsp/             — LSP server (diagnostics, hover, code lens)
  dashboard/       — Axum REST API + embedded static frontend
  search/          — full-text and fuzzy symbol search
  config/          — workspace/repo config, language autodetection
  output/          — formatters (text, JSON, Mermaid)
  watch/           — file watcher with debounce and incremental re-index
  plugins/         — WASM plugin runtime (optional)
tests/             — integration tests (test_*.rs pattern)
public/            — dashboard static assets
examples/          — example projects (Kotlin plugin)
```

## Key Patterns

- `anyhow::Result` for all fallible functions; `thiserror` for custom error types
- `LanguageParser` trait in `parse/mod.rs` — all parsers implement this
- Module-per-directory with `mod.rs` re-exports
- No unsafe code

## Off-Limits

- `target/`, `.env*`, `.ariadne.db`
- SSE/HTTP MCP transport (security: no auth — stdio only)

## Context

For detailed routing see `docs/CONTEXT.md`
