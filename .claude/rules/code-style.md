---
description: Rust code style and conventions for Ariadne
globs:
  - "**/*.rs"
---

## Code Style

- Use 4-space indentation (rustfmt default)
- Use `anyhow::Result` for fallible functions; define custom errors with `thiserror`
- Prefer `#[derive(Debug, Clone, Serialize, Deserialize)]` on public structs
- Module-per-directory with `mod.rs` re-exports
- No unsafe code — maintain this invariant

## Testing Conventions

- Integration tests in `tests/test_*.rs` naming pattern
- Use `Database::open_in_memory()` for test fixtures
- Use `tempfile::TempDir` for filesystem tests

## Error Handling

- `anyhow::Result` at function boundaries
- `thiserror` for domain-specific error enums
- Propagate errors with `?` operator — avoid `.unwrap()` outside tests
