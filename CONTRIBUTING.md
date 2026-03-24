# Contributing to Ariadne

Thank you for your interest in contributing to Ariadne, the universal dependency graph tool for AI agents.

## Getting Started

**Prerequisites:**
- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- SQLite (bundled via `rusqlite`, no system install needed)

**Build:**
```bash
cargo build            # Debug build
cargo build --release  # Release build
```

**Test:**
```bash
cargo test --all-features
```

## Development Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/loremllc/ariadne.git
   cd ariadne
   ```

2. Build the project:
   ```bash
   cargo build
   ```

3. Run on a test repository:
   ```bash
   cargo run -- index tests/fixtures/python_repo
   cargo run -- dash
   ```

## Project Structure

```
src/
  parse/       — Tree-sitter language parsers (symbol extraction)
  db/          — SQLite schema, queries, and migrations
  pipeline/    — Indexing pipeline orchestration
  graph/       — Dependency graph construction and traversal
  analysis/    — Code analysis (coupling, complexity, blast radius)
  search/      — Full-text search (FTS5) over symbols
  dashboard/   — Axum-based web dashboard and visualization
  mcp/         — Model Context Protocol server for AI agents
  lsp/         — Language Server Protocol implementation
  watch/       — File system watcher for incremental re-indexing
  plugins/     — WASM plugin host for extensibility
```

## Running Tests

```bash
# Run all tests with all feature flags (matches CI)
cargo test --all-features

# Run a specific test
cargo test --test test_parsers
```

Test fixtures are located in `tests/fixtures/`. These contain small sample repositories for parser and integration testing.

## Development Workflows

### Testing with a real codebase

```bash
# Index a real project
cargo run -- index ~/path/to/some/project
cargo run -- dash   # Dashboard at http://localhost:1337
```

### Testing MCP integration

```bash
cargo run -- serve  # Starts MCP server
# Add to Claude Code settings for local testing:
# "mcpServers": { "ariadne": { "command": "/path/to/ariadne", "args": ["serve"] } }
```

### Writing a WASM plugin

See `docs/PLUGINS.md` for the full plugin development guide.

## Code Style

- **Formatting:** Run `cargo fmt` before committing. All code must pass `cargo fmt --check`.
- **Linting:** Run `cargo clippy` and address all warnings. CI enforces `cargo clippy -- -D warnings`.
- **No unsafe code:** Do not introduce `unsafe` blocks. The codebase is entirely safe Rust.
- **Error handling:** Use `anyhow::Result` for application errors. Avoid `.unwrap()` outside of tests.

## Pull Requests

- **Branch naming:** Use `feat/description`, `fix/description`, or `chore/description`.
- **One logical change per PR.** Keep PRs focused and reviewable.
- **PR description:** Explain what changed and why. Include before/after if relevant.
- **CI must pass:** All tests, formatting checks, and clippy lints must pass before merge.
- **Keep commits clean:** Squash fixup commits before requesting review.

**PR Checklist:** Before submitting, ensure:
- [ ] `cargo fmt` passes
- [ ] `cargo clippy --all-features -- -D warnings` passes
- [ ] `cargo test --all-features` passes
- [ ] New public functions have doc comments
- [ ] New parsers have fixture files in `tests/fixtures/`

**Tests:** PRs adding features or fixing bugs must include tests. Stub tests (`assert!(true)`) will not be accepted.

## Reporting Issues

Report bugs and request features via [GitHub Issues](https://github.com/loremllc/ariadne/issues).

**Security vulnerabilities** should be reported privately. Do not open a public issue for security concerns. Instead, email the maintainers directly or use GitHub's private vulnerability reporting feature.

## License

By contributing, you agree that your contributions will be licensed under the same dual license as the project (MIT OR Apache-2.0).
