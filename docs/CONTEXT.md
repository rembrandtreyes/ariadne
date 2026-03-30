# Context Routing Index

| Working On | Read First |
|------------|------------|
| Adding a new language parser | `src/parse/mod.rs`, `src/parse/types.rs`, any existing parser (e.g., `src/parse/python.rs`) |
| Pipeline phases | `src/pipeline/mod.rs` — 14-phase ordering and orchestration |
| Database schema or queries | `src/db/schema.rs`, `src/db/query.rs`, `src/db/write.rs` |
| MCP server / tools | `src/mcp/mod.rs`, `src/mcp/tools.rs` |
| Graph analysis | `src/graph/mod.rs` — CallGraph struct, then submodules |
| Dashboard / REST API | `src/dashboard/mod.rs`, `src/dashboard/api.rs` |
| CLI commands | `src/main.rs` — clap command definitions |
| Plugin system | `docs/PLUGINS.md`, `src/plugins/mod.rs` |
| Testing patterns | `tests/test_integration.rs` — canonical integration test example |
| Code style | `.claude/rules/code-style.md` |
