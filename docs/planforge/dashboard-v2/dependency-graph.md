# Dashboard v2 -- Mini PRD Dependency Graph

## Overview

The master plan tasks 3-15 (tasks 1-2 already completed) have been decomposed into 5 mini PRDs:

| Mini PRD | Name | Master Plan Tasks | Steps |
|----------|------|-------------------|-------|
| 01 | Backend APIs | 3, 4, 5 | 12 |
| 02 | Static Serving + CSS + HTML Shell | 6, 7, 8, 14 | 10 |
| 03 | Search + Signal View | 9, 10 | 8 |
| 04 | Void Renderer | 11 | 7 |
| 05 | Detail Panel + Source Modal + Integration | 12, 13, 15 | 10 |

**Total steps:** 47

**Already completed (not in this graph):**
- Task 1: Modules endpoint (`/api/modules`)
- Task 2: Coupling endpoint (`/api/coupling`)

## Dependency Graph

```
  PRD-01 (Backend APIs)       PRD-02 (Static + CSS + HTML)
        |                            |
        |                     +------+------+
        |                     |             |
        v                     v             v
        |              PRD-03 (Search    PRD-04 (Void
        |               + Signal)        Renderer)
        |                     |             |
        +----------+----------+-------------+
                   |
                   v
           PRD-05 (Detail Panel
            + Source Modal
            + Integration)
```

## Execution Batches

### Batch 1 -- parallel, no dependencies
- **PRD-01: Backend APIs** (12 steps)
  Creates `/api/describe` endpoint, modifies `/api/source` to accept context param,
  adds `last_indexed` to `/api/health`. Pure Rust backend work.
- **PRD-02: Static Serving + CSS + HTML Shell** (10 steps)
  Rewrites `index.html` and `style.css`, creates 5 JS placeholder files,
  adds `include_str!` route handlers, embeds App controller and `esc()` function.

### Batch 2 -- parallel, after Batch 1 completes
- **PRD-03: Search + Signal View** (8 steps)
  Implements `search.js` (debounced input, keyboard nav) and `signal.js`
  (health score, risk table, module grid, coupling pairs, dead code list).
  Depends on PRD-02 for DOM containers and JS stub files.
- **PRD-04: Void Renderer** (7 steps)
  Implements `void-renderer.js` (module nodes, auto-layout, SVG connections,
  drag-to-reposition, localStorage persistence, color modes).
  Depends on PRD-02 for DOM containers and JS stub files.

### Batch 3 -- sequential, after all prior batches complete
- **PRD-05: Detail Panel + Source Modal + Integration** (10 steps)
  Implements `detail-panel.js` (describe/source/neighborhood display,
  syntax highlighting), `source-modal.js` (full-screen viewer),
  and wires drill-down navigation across all views.
  Depends on PRD-01 (API endpoints), PRD-02 (DOM), PRD-03 (Signal drillDown),
  PRD-04 (Void node click).

## Critical Path

```
PRD-02 --> PRD-03 --> PRD-05
PRD-02 --> PRD-04 --> PRD-05
PRD-01 ------------> PRD-05
```

The critical path length is 3 batches. Minimum wall-clock execution:
- Batch 1: max(PRD-01, PRD-02) = 12 steps
- Batch 2: max(PRD-03, PRD-04) = 8 steps
- Batch 3: PRD-05 = 10 steps

## File Ownership

Each mini PRD owns specific files. Where multiple PRDs modify the same file,
execution order prevents conflicts:

| File | 01 | 02 | 03 | 04 | 05 |
|------|----|----|----|----|-----|
| `src/dashboard/api.rs` | MODIFY | | | | |
| `src/dashboard/describe.rs` | CREATE | | | | |
| `src/dashboard/mod.rs` | MODIFY | MODIFY | | | |
| `src/dashboard/static/style.css` | | REWRITE | | | |
| `src/dashboard/static/index.html` | | REWRITE | | | |
| `src/dashboard/static/signal.js` | | CREATE | REWRITE | | |
| `src/dashboard/static/void-renderer.js` | | CREATE | | REWRITE | |
| `src/dashboard/static/detail-panel.js` | | CREATE | | | REWRITE |
| `src/dashboard/static/search.js` | | CREATE | REWRITE | | |
| `src/dashboard/static/source-modal.js` | | CREATE | | | REWRITE |
| `tests/test_dashboard.rs` | MODIFY | MODIFY | | | |

**Conflict notes:**
- `src/dashboard/mod.rs` is modified by both PRD-01 and PRD-02. PRD-01 adds
  the `/api/describe` route and `pub mod describe;`. PRD-02 adds static file
  `include_str!` constants and route handlers. These are additive changes to
  different parts of the file. If running Batch 1 in parallel, merge conflicts
  are straightforward (both add `.route()` lines to the router chain).
- `tests/test_dashboard.rs` is modified by both PRD-01 and PRD-02. Both add
  new test functions (additive). Merge conflicts are trivial.
- JS files created as stubs by PRD-02 are fully rewritten by PRD-03/04/05.
  No merge conflict possible since Batch 2/3 run after PRD-02 completes.

## Key Corrections Applied

These corrections from codebase audit are baked into all mini PRDs:
- Uses `symbol_by_id` (not `find_symbol_by_id` which does not exist)
- `SymbolRow` has no `signature`, `is_exported`, or `is_entry_point` fields
- `Database.get_metadata()` already exists (no new query function needed)
- `DbState = Arc<PathBuf>` pattern: fresh DB connection per request
