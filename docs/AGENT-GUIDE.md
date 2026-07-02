# Ariadne MCP — Agent Usage Guide

This guide is for AI coding agents (Claude Code, Cursor, Continue, and any
other MCP-compatible client) driving Ariadne's 32-tool MCP surface. It
answers the question agents ask most often: **"which tool should I call?"**

Ariadne returns pre-resolved structured data. A caller-trace that would
cost ~12K tokens via `Grep + Read` returns in ~400 tokens via
`get_context`. Picking the right tool compounds those savings across a
session. The sections below are a decision tree — read the scenario,
call the first tool, and only reach for subsequent tools if the answer
isn't enough.

---

## Tool descriptions that follow use generic placeholders

All examples below use generic symbol names (`UserService`, `generatePost`,
`my_handler`). Replace them with symbols from your own codebase when you
make real calls.

---

## Onboarding an unfamiliar codebase

You've just been pointed at a repository and need to build a mental model.

1. **`get_entry_points`** — where execution starts: `main`, HTTP handlers,
   framework callbacks. Filter by category (`"framework"`, `"http"`, `"main"`,
   or `"all"`). Always call this first.
2. **`get_god_objects`** — which symbols are the most depended on (high
   fan-in). Default threshold is 20. These are the high-leverage symbols
   — understanding them pays back across every later question.
3. **`get_codebase_health`** — one-call snapshot: letter grade A–F, dead
   code ratio, cycle count, coupling density, modularity score, plus a
   natural-language summary.
4. **`get_complexity`** — file/symbol/call counts, resolution rate,
   language breakdown. Useful for sizing.

After these four calls you have: where it starts, what's structurally
risky, how healthy it is, and how big it is. Stop here unless the user
has a specific next question.

---

## Reviewing a pull request or planning a change

You know which files changed and need to scope review.

1. **`diff_impact(changed_files)`** — unified one-call summary: affected
   symbols, blast radius, affected tests, review focus. Often the only
   call you need for a small diff.
2. **`compute_file_risk(changed_files)`** — per-file 0.0–1.0 risk score
   combining churn velocity, coupling, fan-in, and dead-code proximity.
   Identifies which file in the diff deserves the closest read.
3. **`blast_radius(symbol)`** — for a specific symbol in the diff, what
   breaks. Returns WILL-BREAK direct dependents and MAY-BREAK transitive
   dependents, grouped by depth.
4. **`affected_tests(changed_files)`** — the minimum test set to run.
   Avoid running the whole suite when you can run the 3 tests that
   actually cover the change.

---

## Refactoring preparation

You're about to rename, split, or restructure a symbol and need the
dependency cone before starting.

1. **`propose_edit_plan(symbol)`** — single composed call returning the
   leaves-first edit order over the dependent cone, plus the affected
   tests, plus the execution flows that pass through this symbol. Use
   this first when refactoring; the `edit_order` array IS the safe
   sequence for updating callers (depth-1 callers before transitive
   ones). Sets `cycle_detected: true` and falls back to BFS-depth
   ordering when the cone contains a cycle.
2. **`blast_radius(symbol)`** — everyone you might break. Use
   `max_depth` to widen or narrow the cone. Prefer `propose_edit_plan`
   when you need the *order* to edit; prefer `blast_radius` when you
   only need the WILL/MAY breakdown.
3. **`get_execution_flows(symbol)`** — the ordered call paths that reach
   this symbol from entry points. Tells you which execution contexts
   you're touching, not just which callers.
4. **`get_dependents(symbol)`** — raw list of direct callers. Pair with
   `blast_radius` when you need the names, not the summary.
5. **`get_symbol_health(symbol)`** — 0.0–1.0 score fusing stability,
   connectivity, and dead-code status. Low-health symbols refactor with
   less review overhead; high-health symbols are load-bearing and
   deserve more care.
6. **`compute_file_risk(file_path)`** — should I be especially careful
   editing here?

---

## Understanding a specific symbol

You need to explain one function or class — why it exists, who uses it,
how it evolved.

1. **`why_symbol(symbol)`** — single narrative with role, callers,
   callees, blast radius, and coupled files. Start here for
   human-readable explanations.
2. **`get_symbol_history(symbol)`** — creation date, last modification,
   change frequency, author count, volatility. Temporal context that
   static analysis can't provide.
3. **`get_heritage(symbol)`** — parent classes/interfaces + child
   subclasses. Use for symbols in inheritance hierarchies.
4. **`get_context(symbol)`** — raw structured data: callers, callees,
   file, signature, dead status, coupled files. Prefer this when you
   want JSON, not prose.
5. **`get_dependency_path(from_symbol, to_symbol)`** — how does A
   eventually call B? Returns the shortest directed path or
   `reachable=false` (not an error) if none exists.

---

## Structural exploration

You're answering meta-questions about the codebase shape itself.

1. **`detect_cycles`** — circular dependencies via Kosaraju SCC. Shows
   cycles with member symbols + length.
2. **`get_communities`** — module communities found by graph clustering
   (Louvain algorithm). Logical groupings independent of directory
   structure.
3. **`get_boundaries`** — per-module symbol counts, internal vs
   cross-boundary calls, modularity scores.
4. **`get_coupling`** — file pairs that change together (git co-change
   strength). Reveals implicit dependencies that don't show up in
   imports.
5. **`find_dead_code`** — unreachable functions and methods.
6. **`get_code_smells`** — bottlenecks, shotgun surgery risks, high
   volatility spikes, dead code.
7. **`get_api_endpoints`** — HTTP/RPC endpoints with method, path,
   handler symbol, file.

---

## Tool overlap — which to prefer when two seem equivalent

- **`get_context` vs `why_symbol`**: Both return callers + callees.
  Prefer `why_symbol` for human-readable narrative you'll show the user.
  Prefer `get_context` when you want raw structured JSON for further
  processing.
- **`find_dead_code` vs `get_code_smells`**: `find_dead_code` returns
  only unreachable symbols. `get_code_smells` returns multiple smell
  categories (dead + volatility + fan-in bottlenecks + fan-out shotgun
  risks). Use `find_dead_code` when you only want the dead list; use
  `get_code_smells` for a wider audit.
- **`get_symbol_health` vs `compute_file_risk`**: Symbol-level versus
  file-level. Ask by file when triaging a PR; ask by symbol when
  refactoring one function.
- **`blast_radius` vs `get_dependents`**: `get_dependents` is raw direct
  callers. `blast_radius` groups transitive dependents by depth and
  labels them WILL-BREAK vs MAY-BREAK. Use `blast_radius` when planning
  a change; use `get_dependents` when you need the exact list.

---

## Token economics — why picking the right tool matters

| Operation | Without Ariadne | With Ariadne | Savings |
|---|---|---|---|
| File structure exploration | ~3,000 tokens (`Read` the file) | ~150 tokens (`get_file_summary`) | ~20× |
| Caller trace across 6 files | ~12,000 tokens (`Grep + Read × 6`) | ~400 tokens (`get_context`) | ~30× |
| Change impact assessment | ~8,000 tokens (manual trace) | ~300 tokens (`diff_impact`) | ~25× |
| Risky-file detection in PR | Multiple reads + judgement | ~200 tokens (`compute_file_risk`) | ~40× |

Ariadne's resident MCP server keeps the call graph in memory between
requests. Latencies are sub-5ms after the first call (<2ms for MCP
tools that hit SQLite FTS, <5ms for graph traversal).

---

## When to use REST vs MCP

Five MCP tools also ship as `GET /api/...` routes for use from the dashboard
or from non-MCP clients. The MCP surface is canonical; the REST surface is
the same data over HTTP for browsers, curl, and CI scripts.

| MCP tool | REST mirror |
|---|---|
| `get_entry_points` | `GET /api/entry_points?category=&limit=` |
| `get_complexity_hotspots` | `GET /api/complexity_hotspots?limit=` |
| `get_god_objects` | `GET /api/god_objects?threshold=&limit=` |
| `get_dependency_path` | `GET /api/dependency_path?from=&to=` |
| `propose_edit_plan` | `GET /api/propose_edit_plan?symbol=` |

**Use MCP from your agent.** Sub-5ms cached call graph, structured tool
descriptions, no JSON parsing, and the full 32-tool surface.

**Use REST when wiring the dashboard, scripting with curl, or testing latency
from outside the MCP transport.** The five mirrored routes return identical
data to their MCP counterparts but with `BTreeMap`-backed JSON for
deterministic ordering across repeat queries (the MCP `HashMap` payloads are
agent-consumed and order-tolerant). Missing-symbol queries return structured
200 OK with a `summary` describing the miss, never 5xx — the same shape
across all five routes. Measured latency: 1-34ms on a 1MB self-index.

The remaining 27 MCP tools are MCP-only by design — they return rich nested
structures (Mermaid call chains, multi-bucket impact reports) that fit MCP's
structured-content model better than HTTP query strings.

---

## Answer trust: `parse_warnings`

Answer-bearing tools (`blast_radius`, `diff_impact`, `affected_tests`,
`propose_edit_plan`, `why_symbol`, `get_context`) include a top-level
`parse_warnings` block **only when** the index contains files that parsed
with syntax errors. Files listed there have missing graph edges, so the
answer may undercount (blast radius, affected tests) or mis-order edits.
A "symbol not found" miss on a dirty index carries the block too — the
symbol may be unindexed precisely because its file failed to parse.

When you see `parse_warnings`: re-index after fixing the syntax, or verify
the listed files manually. No `parse_warnings` key means the index is
clean — trust the answer.

---

## What Ariadne won't do

- **Execute or modify code** — Ariadne is read-only. It tells you what
  breaks, not how to fix it. Pair with your agent's file-edit tools.
- **Semantic similarity search** — Ariadne uses resolved call edges,
  not embeddings. Use it when you know the symbol; use RAG when you
  only have a fuzzy English description.
- **Index as you type** — Ariadne re-indexes on file save via
  `ariadne watch --serve`. Edits in your current IDE buffer aren't
  visible to the MCP server until saved.

(`propose_edit_plan` previously listed here as "won't do — planned"
ships in the current release. See the Refactoring section above.)

---

## Quick-reference matrix — 32 tools by intent

Full descriptions live in the [README's Available MCP Tools table](../README.md#available-mcp-tools).
Use this matrix for rapid selection:

| Scenario | First tool | Fallback |
|---|---|---|
| New to codebase | `get_entry_points` | `get_codebase_health` |
| Handle-with-care warning | `get_god_objects` | `get_complexity_hotspots` |
| PR review triage | `diff_impact` | `compute_file_risk` |
| Blast impact of a rename | `blast_radius` | `get_dependents` |
| Order to update callers when refactoring | `propose_edit_plan` | `blast_radius` |
| Which tests to re-run | `affected_tests` | `diff_impact` |
| Explain a function | `why_symbol` | `get_context` |
| Stability / churn profile | `get_symbol_history` | `get_symbol_health` |
| Two-symbol relationship | `get_dependency_path` | `get_call_chain` |
| Circular imports | `detect_cycles` | `get_boundaries` |
| Dead code sweep | `find_dead_code` | `get_code_smells` |
| Modules without directories | `get_communities` | `get_coupling` |
| API surface inventory | `get_api_endpoints` | `get_file_summary` |
| Class hierarchy | `get_heritage` | — |
| Execution paths through X | `get_execution_flows` | `get_call_chain` |
| Overall shape | `get_complexity` | `get_codebase_health` |

---

## Further reading

- [`README.md`](../README.md) — installation, competitive positioning, full tool descriptions
- [`CHANGELOG.md`](../CHANGELOG.md) — release history
- [`src/mcp/tools/mod.rs`](../src/mcp/tools/mod.rs) — canonical tool definitions (the source of truth)
