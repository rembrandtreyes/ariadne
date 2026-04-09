# Mini PRD 01: DB Query Function for Graph Stats

> **Dependency:** none — can execute independently
> **Produces:** `pub struct GraphStats` type and `pub fn get_graph_stats(db: &Database) -> anyhow::Result<GraphStats>` in `/Users/rembrandt/loremllc/ariadne/src/db/query.rs`; unit test `test_graph_stats` in the same file.
> **Estimated steps:** 2

## Context

The Ariadne dashboard needs a `/api/graph-stats` endpoint that returns the total number
of nodes (symbols), total number of edges (calls), and the timestamp of the last indexing
run. This mini PRD adds the database query layer only — a pure addition to
`src/db/query.rs` with no changes to any other file. It adds a new public struct
`GraphStats` and a function `get_graph_stats` that reads from three tables (`symbols`,
`calls`, `services`) and returns the aggregate data. A unit test verifies the function
works against an in-memory database.

## Files

| Action | Path | Purpose |
|--------|------|---------|
| MODIFY | `/Users/rembrandt/loremllc/ariadne/src/db/query.rs` | Add `GraphStats` struct and `get_graph_stats` function with unit test |

## Steps

### Step 1: Add `GraphStats` struct and `get_graph_stats` function

**File:** `/Users/rembrandt/loremllc/ariadne/src/db/query.rs`
**Location:** At the end of the file (after all existing public functions, before any
`#[cfg(test)]` block if one exists — there is none currently in this file).

```rust
/// Graph-level aggregate statistics.
#[derive(Debug, Clone, Serialize)]
pub struct GraphStats {
    /// Total number of symbol nodes in the dependency graph.
    pub node_count: u64,
    /// Total number of call edges in the dependency graph.
    pub edge_count: u64,
    /// Unix epoch (seconds, fractional) of the most recent indexing run across all
    /// services, or `None` if no services have been indexed yet.
    pub last_indexed: Option<f64>,
}

/// Return aggregate graph statistics: node count, edge count, last indexed timestamp.
///
/// Reads `COUNT(*) FROM symbols`, `COUNT(*) FROM calls`, and
/// `MAX(last_indexed) FROM services`. All three are single-row scalar queries and
/// execute in O(1) time when SQLite's internal counters are used.
pub fn get_graph_stats(db: &Database) -> anyhow::Result<GraphStats> {
    let conn = db.conn();

    let node_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM symbols",
        [],
        |row| row.get(0),
    )?;

    let edge_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM calls",
        [],
        |row| row.get(0),
    )?;

    let last_indexed: Option<f64> = conn.query_row(
        "SELECT MAX(last_indexed) FROM services",
        [],
        |row| row.get(0),
    )?;

    Ok(GraphStats {
        node_count: node_count as u64,
        edge_count: edge_count as u64,
        last_indexed,
    })
}
```

**Verify:** `cargo build 2>&1 | grep error`
**Expected:** No output (no compile errors)

### Step 2: Add unit test for `get_graph_stats`

**File:** `/Users/rembrandt/loremllc/ariadne/src/db/query.rs`
**Location:** Append to the end of the file, after the code added in Step 1.

```rust
#[cfg(test)]
mod query_graph_stats_tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn test_graph_stats_empty_db() {
        let db = Database::open_in_memory().expect("open in-memory db");
        let stats = get_graph_stats(&db).expect("get_graph_stats should succeed");
        assert_eq!(stats.node_count, 0, "empty db: node_count should be 0");
        assert_eq!(stats.edge_count, 0, "empty db: edge_count should be 0");
        assert!(
            stats.last_indexed.is_none(),
            "empty db: last_indexed should be None"
        );
    }
}
```

**Verify:** `cargo test query_graph_stats_tests::test_graph_stats_empty_db -- --nocapture`
**Expected:** `test query_graph_stats_tests::test_graph_stats_empty_db ... ok`

## Acceptance Criteria

- [ ] `cargo test query_graph_stats_tests::test_graph_stats_empty_db` → PASS (exit 0)
- [ ] `cargo clippy -- -D warnings` → exit 0 (no warnings)
- [ ] `cargo build` → exit 0 (no compile errors)

## Types and Signatures

All public types and function signatures introduced by this mini PRD:

```rust
// New type
#[derive(Debug, Clone, Serialize)]
pub struct GraphStats {
    pub node_count: u64,
    pub edge_count: u64,
    pub last_indexed: Option<f64>,
}

// New function
pub fn get_graph_stats(db: &Database) -> anyhow::Result<GraphStats>;
```

## Imports

No new imports are required. `Serialize` is already imported at the top of
`/Users/rembrandt/loremllc/ariadne/src/db/query.rs` via `use serde::Serialize;`.
`Database` is already in scope via `use super::Database;`.

```rust
// In /Users/rembrandt/loremllc/ariadne/src/db/query.rs
// No new imports needed — serde::Serialize and super::Database are already imported.
```

## Completion Contract

```
**Tests that must pass before signaling done:**
- `cargo test query_graph_stats_tests::test_graph_stats_empty_db` → exit 0
- `cargo clippy -- -D warnings` → exit 0

**Files this mini PRD is permitted to touch:**
- `/Users/rembrandt/loremllc/ariadne/src/db/query.rs`

**Completion signal:**
When every test above passes and every acceptance criterion is met, output exactly this
line and nothing after it:

PLANFORGE_COMPLETE: PRD-01 get_graph_stats query function added to src/db/query.rs with unit test

Do not output the completion signal until all tests pass.
Do not output it speculatively or as a placeholder.
```
