# Dependency Graph

## Execution Batches

### Batch 1 (no dependencies — execute first)
- Mini PRD 01: `db-query-function` — produces: `GraphStats` struct, `get_graph_stats` function in `src/db/query.rs`

### Batch 2 (after Batch 1 — PRD-01 must be complete)
- Mini PRD 02: `handler-route-test` — depends on: PRD-01 — produces: `GraphStatsResponse` struct, `graph_stats` handler in `src/dashboard/api.rs`; `/api/graph-stats` route in `src/dashboard/mod.rs`; `test_graph_stats_handler` test in `tests/test_dashboard.rs`

## Visual

```
PRD-01 (db query) ──→ PRD-02 (handler + route + test)
```

## Artifact-Level Dependencies

| Artifact | Produced By | Consumed By |
|----------|-------------|-------------|
| `GraphStats` struct (`src/db/query.rs`) | PRD-01 | PRD-02 (`graph_stats` handler) |
| `get_graph_stats` fn (`src/db/query.rs`) | PRD-01 | PRD-02 (`graph_stats` handler) |
| `GraphStatsResponse` struct (`src/dashboard/api.rs`) | PRD-02 | `tests/test_dashboard.rs` |
| `graph_stats` handler (`src/dashboard/api.rs`) | PRD-02 | `src/dashboard/mod.rs` route registration, integration test |
| `/api/graph-stats` route (`src/dashboard/mod.rs`) | PRD-02 | End user / HTTP clients |

## Files Modified Per PRD

| File | PRD-01 | PRD-02 |
|------|--------|--------|
| `src/db/query.rs` | MODIFY | — |
| `src/dashboard/api.rs` | — | MODIFY |
| `src/dashboard/mod.rs` | — | MODIFY |
| `tests/test_dashboard.rs` | — | MODIFY |

No file is modified by both PRDs — there are no conflicting mutations.
