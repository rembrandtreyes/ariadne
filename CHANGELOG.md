# Changelog

All notable changes to Ariadne will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

**Five reference-truth substrate gaps** (each with red-first regression tests;
found by a blinded head-to-head eval against agent+ripgrep on a real Next.js
repo):

- Discovery and watch mode now honor in-repo `.gitignore` files (root and
  nested, negation patterns, no `.git` directory required) — a gitignored
  worktree tree no longer pollutes the index with duplicate repo copies.
  The user's global gitignore and `.git/info/exclude` are deliberately NOT
  honored: they vary by machine and would make identical trees index
  differently
- tsconfig path-alias imports (`@/lib/x`) now resolve end-to-end: parsers
  classify them external (they can't know the aliases), so resolution now
  gives external-marked rows one alias-match chance and reclassifies them
  internal on success; alias expansions are root-relative (a `./src/*`
  replacement is no longer joined to the importing file's directory). On a
  790-file Next.js repo this took `@/` import resolution from 0 to 2,579/2,579
  and made route→lib call edges land (no more false "test-only caller"
  verdicts)
- Symbol-name collisions no longer resolve silently to the lowest rowid:
  a collision-aware resolver returns Unique/Ambiguous/NotFound with
  deterministic (path, line, id) candidate order; the CLI prints the candidate
  list and exits nonzero, MCP tools return a structured `ambiguous_symbol`
  payload, and the dashboard returns 409 with candidates
- Missing reference edge types: aliased references (`const x = fn`) now emit
  an edge; renamed imports (`import { helper as h }`) keep the original
  exported name (new `imports.original_name` column + migration) through
  import and call resolution; re-exports (`export { a } from './b'`) create
  import rows instead of wrongly marking same-named locals as exported;
  `satisfies`/`as`/parenthesized arrow values still classify as functions
- Name-fallback resolution passes (import-file-affinity, same-service,
  global) gained total deterministic tie-breaks (is_exported, path, line, id)
  instead of implicit rowid order

**Deterministic output across runs** — identical input now produces identical
output on every surface; the hash-iteration-order bug class was swept from the
whole pipeline (each fix carries a red-first regression test):

- Community IDs were minted in `HashMap` key order and differed on every
  index run; BFS seeds are now sorted (verified: 3 consecutive full indexes
  of the same repo produce byte-identical `communities --json`)
- `services.primary_language` was a random pick from a language set on
  multi-language repos; languages are now ordered (file count desc, name asc)
  so the dominant language wins deterministically
- `blast_radius` `affected_files` and `affected-tests` `test_files` were raw
  hash-set drains; both are now sorted
- Circular-dependency output followed randomized petgraph `NodeIndex`
  assignment; cycles are now canonical (members and cycle list sorted), and
  graph node loading no longer follows hash-set drain order
- Coupling queries ordered by `strength` alone — equal-strength ties plus
  `LIMIT` could return a different row set across runs, feeding
  `coupling_density` and the health grade; all three coupling readers now use
  a total order and coupling inserts are sorted for stable database bytes

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

**Parse truthfulness** (TSX grammar dispatch + per-file syntax-error visibility)
- Fixed: `.tsx` files now parse with the TSX grammar. Previously every `.tsx`
  file went through `LANGUAGE_TYPESCRIPT`, which cannot parse JSX — React
  component symbols and render edges were silently dropped, so the JSX
  tracking advertised above was unreachable for `.tsx` until now
- Every parser records a per-file `parse_error_count` (tree-sitter
  ERROR/MISSING nodes), persisted on the `files` table with an idempotent
  column migration on open — existing `.ariadne.db` files upgrade in place
- `ariadne index` warns which files parsed with syntax errors;
  `get_file_summary` (MCP) exposes `parse_error_count` so agents can judge
  per-file graph trust; parser I/O failures are logged instead of silently
  dropping the file from the graph
- Dashboard `describe` propagates DB errors instead of rendering them as
  empty metrics
- LSP publishes a parse-health warning diagnostic on files whose last index
  parse produced syntax errors; the diagnostics builder is extracted from the
  LSP plumbing and behaviorally tested for the first time
- Answer-bearing MCP tools (`blast_radius`, `diff_impact`, `affected_tests`,
  `propose_edit_plan`, `why_symbol`, `get_context`) carry a top-level
  `parse_warnings` block when the index contains parse-broken files — impact
  answers built on missing edges now say so instead of undercounting
  silently; the key is absent on a clean index

**Quality**
- `cargo clippy --all-targets` exits 0 (first time in project history)
- MCP call-graph cache rebuilds only when pipeline generation changes —
  eliminates per-tool-call O(symbols+calls) SQLite scan
- `tests/test_mcp_tools.rs` guardrail asserts exact `Vec<Tool>` length so
  tool-count drift is caught at CI time
- `tests/test_docs_parity.rs` asserts README + lib.rs + AGENT-GUIDE.md +
  CHANGELOG stay in sync with `all_tools()` — shipped-docs lies now fail CI

### Fixed

**Graph you can trust**
- **MCP graph tools no longer truncate silently at 10,000 call edges.** The
  cached MCP call graph was built with an arbitrary `LIMIT 10000` on the
  edges query, so on any repo past that size `blast_radius`,
  `detect_cycles`, `get_call_chain`, `get_dependency_path`, and
  `propose_edit_plan` computed on an arbitrary subgraph — `blast_radius`
  could answer "nothing breaks" for a symbol with live callers. The MCP
  server now builds the full graph like every other surface (CLI, LSP,
  dashboard); regression-tested with a 10,050-edge fixture
- **Stale resolution labels reset on re-resolution.** Deleting a symbol
  (watch-mode reindex, file deletion) nulls inbound `calls.callee_symbol_id`
  via FK `SET NULL` but left the old `resolution`/`confidence` in place —
  rows claimed `import_guided`/0.98 while pointing at nothing, inflating
  resolution stats, and most re-resolution passes skipped them forever. A
  reset pass now runs first in `resolve_calls`
- **`exclude` glob patterns work as documented.** `exclude =
  ["generated/**"]` was compared by exact component-name equality and
  matched nothing; patterns now compile to real globs (bare names keep the
  historical exact-component behavior)
- **Watch mode honors exclude rules.** The watcher accepted any file with a
  known extension — saves under `target/`, `node_modules/`, `vendor/`, or
  user-excluded paths got indexed (a `cargo build` alone injects generated
  `.rs` files). Watch now shares discovery's `PathFilter`
- **Watch-mode reindexing produces full-pipeline-identical data.** The
  incremental path had drifted from `parse_all`: integration-test files
  lost their `is_test` marking (skewing `affected_tests` and dead-code
  seeds), call-insert failures were silently discarded, and files were
  reassigned to the first service in the DB (breaking cross-service tracing
  under watch — new files now match services by path prefix, existing files
  keep their assignment). Both paths now share one `ingest_parse_result`
- **Watch batches re-run entry-point marking, dead code, and flows.**
  Post-reindex resolution previously skipped framework entry-point rules —
  a route handler edited under watch could be flagged dead, the worst
  possible advice to hand an agent. Coupling/git-history/communities still
  refresh on full `ariadne index` (documented freshness contract)
- Each watch batch commits in one `BEGIN IMMEDIATE` transaction: a branch
  switch is one WAL commit instead of thousands of autocommits, readers
  never observe half-applied batches, and failures roll back cleanly
- `PRAGMA busy_timeout=5000` on file-backed connections — concurrent
  watch writes and MCP/dashboard reads wait instead of failing with
  instant `SQLITE_BUSY`
- Unreadable/non-UTF8 source files are logged when skipped instead of
  vanishing from the index without a tell

**Docs made true**
- README: removed crates.io/docs.rs badges and `cargo install ariadne`
  (that crate name belongs to the unrelated diagnostics library — publish
  under a distinct package name is planned); `dead-code --threshold` and
  `watch --dash` phantom flags corrected; nonexistent GitHub Action replaced
  with a plain-CLI CI recipe; LSP hover claim matched to what hover actually
  shows; plugin system labeled experimental (runtime ships, indexer wiring
  does not); scale projections labeled as extrapolations; watch-vs-index
  incremental semantics stated precisely
- `CLAUDE.md` structure map: 10 → 32 MCP tools, 14 → 15 pipeline phases,
  dropped deleted `output/` module, added `commands/`
- `docs/CONTEXT.md`: 15-phase, corrected dead `src/mcp/tools.rs` path
- Pipeline phase doc-comments renumbered to a canonical scheme (discovery =
  Phase 0 untimed; 1–15 timed in execution order) — previously two pairs of
  phases shared numbers 6 and 10, which is how "14-phase" shipped while 15
  phases ran
- MCP `get_info` instructions: "31-tool quick-reference matrix" → 32
- `serve --http` help text now states the flag is rejected by design
  (no-auth transport), matching runtime behavior

### Changed

- Call-resolution dotted pass prepares its three lookup statements once
  instead of recompiling SQL per unresolved call
- Removed dead `RESOLUTION_RESOLVED` constant
- New doc-parity invariants: pipeline phase count checked against the
  *executed* pipeline (`PipelineStats.phase_durations`), MCP `get_info`
  instruction string checked against the tool count, CLAUDE.md checked for
  tool count, and every `ariadne` invocation in README bash blocks verified
  against the real binary's `--help` output — phantom flags now fail CI
- New watch-mode regression suite: FTS freshness after reindex, test-file
  marking parity, service preservation, stale-edge relabeling, dead-code
  stability across incremental batches; plus discovery `PathFilter` tests
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
