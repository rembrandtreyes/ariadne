# Changelog

All notable changes to Ariadne will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

**22 new MCP tools** (total now 32, up from 10 at 0.1.0) — grouped by intent
and documented in [`docs/AGENT-GUIDE.md`](docs/AGENT-GUIDE.md).

Onboarding & triage:
- `get_entry_points` — `main`, HTTP handlers, framework callbacks with category filter
- `get_god_objects` — high fan-in symbols with parameterizable threshold
- `get_codebase_health` — one-call letter grade A–F with natural-language summary
- `compute_file_risk` — per-file 0.0–1.0 risk scoring for PR triage
- `get_complexity_hotspots` — top 50 symbols by combined fan-in × fan-out × churn

Change impact:
- `diff_impact` — unified change-impact report in a single call
- `affected_tests` — minimum test set for a set of changed files
- `get_dependency_path` — shortest directed call path between two symbols (BFS)
- `propose_edit_plan` — leaves-first ordered edit sequence for callers when
  refactoring a symbol; topological sort over the dependent cone with
  cycle detection and BFS-depth fallback on cyclic graphs

Deep-dive on a symbol:
- `why_symbol` — single-narrative explainer with role, callers, callees, blast radius
- `get_symbol_history` — git temporal analysis (creation, churn, authors, volatility)
- `get_symbol_health` — 0.0–1.0 score fusing stability, connectivity, dead status
- `get_heritage` — inheritance hierarchy (parent + child classes/interfaces)
- `get_execution_flows` — ordered call paths through a symbol from entry points

Navigation:
- `get_file_dependencies` / `get_file_dependents` — transitive file-level graph traversal

Structural exploration:
- `detect_cycles` — Kosaraju SCC with member symbols + length
- `get_boundaries` — module-boundary modularity scores
- `get_coupling` — top file pairs by git co-change strength
- `get_communities` — Louvain clustering of logical modules
- `get_api_endpoints` — detected HTTP/RPC endpoints with handler links
- `get_code_smells` — bottleneck / shotgun-surgery / volatility / dead detection

**Dashboard v2 (Signal + Void views)**
- Signal view: hero 0-100 health score, top risks with narrative descriptions,
  module grid with per-file sparklines, coupling list, dead code grid
- Void view: force-directed / architecture / risk / coupling canvas with HUD
  mode switching, click-through drill-down from Signal
- Module summaries now surface real cycle counts (Kosaraju SCC per module)
  and real god-object counts (fan-in-based, per module) — previously stubbed
  to zero
- Describe panel now shows real blast radius per symbol — previously stubbed

**Indexing pipeline**
- 15-phase indexing (up from documented 14): structure, parsing,
  import_resolution, call_resolution, heritage, framework_entry_points,
  dead_code, flow, coupling, git_history, search_index, api_resolution,
  service_topology, community, schema_resolution
- Pipeline wrapped in a single SQLite `BEGIN / COMMIT / ROLLBACK` via
  `execute_batch` — crashed indexes no longer leave partial data
- Synthetic `<module>` symbol anchors module-level call edges in JS/TS
  parsers (previously dropped)
- JSX prop bare-identifier tracking for JavaScript and TypeScript (`.jsx`,
  `.tsx`, `.ts`)
- Edge-driven graph loading with batched N+1 query elimination

**Dashboard REST parity** (Signal-view intelligence over HTTP, 5 of 32 MCP tools mirrored)
- `GET /api/entry_points?category=&limit=` mirrors `get_entry_points`
- `GET /api/complexity_hotspots?limit=` mirrors `get_complexity_hotspots`
- `GET /api/god_objects?threshold=&limit=` mirrors `get_god_objects`
- `GET /api/dependency_path?from=&to=` mirrors `get_dependency_path`
- `GET /api/propose_edit_plan?symbol=` mirrors `propose_edit_plan` —
  typed response with deterministic `BTreeMap` JSON ordering for stable
  repeat-query output (the MCP version's `HashMap` is agent-consumed and
  order-tolerant)
- Signal view "Entry Points" card surfaces 12 framework/HTTP/main entry
  points with click-through drill-down into the detail panel
- All five routes measured at 1-34ms latency on a 1MB self-index;
  no per-request cache needed under the 200ms budget

**Quality**
- `cargo clippy --all-targets` exits 0 (first time in project history)
- MCP call-graph cache rebuilds only when pipeline generation changes —
  eliminates per-tool-call O(symbols+calls) SQLite scan
- `tests/test_mcp_tools.rs` guardrail asserts exact `Vec<Tool>` length so
  tool-count drift is caught at CI time
- `tests/test_docs_parity.rs` asserts README + lib.rs + AGENT-GUIDE.md +
  CHANGELOG stay in sync with `all_tools()` — shipped-docs lies now fail CI

### Changed

- README competitor comparison updated from 2015-era tools
  (dependency-cruiser, Axon, Sourcegraph, CodeScene) to current 2025/2026
  HIGH threats (GitNexus, Codebase-Memory, Greptile, Potpie,
  Understand-Anything, code-review-graph) with moat-aligned capability axes
- README MCP tools table regenerated from `all_tools()` — now lists all
  32 tools grouped by intent (onboarding, change impact, deep-dive,
  navigation, structural exploration)
- MCP server `instructions` payload points agents to the onboarding triad
  and the decision tree
- `src/lib.rs` module docstring tool count corrected (was 29, reality 32)
- `docs/AGENT-GUIDE.md` Refactoring section now leads with `propose_edit_plan`
  for leaves-first edit ordering when refactoring a symbol's callers

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
