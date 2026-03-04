# Ariadne Security Audit Report

**Date:** 2026-03-03
**Auditor:** Echo (PAI Security Audit)
**Target:** ariadne-graph v0.1.0 — Universal dependency graph for AI agents
**Tech Stack:** Rust, SQLite (rusqlite), Axum, tree-sitter, wasmtime, tower-lsp, git2, rmcp (MCP)

---

## Executive Summary

Ariadne is a Rust CLI/server tool with multiple network-facing interfaces (MCP server, HTTP dashboard, LSP server). The codebase benefits significantly from Rust's memory safety guarantees — no `unsafe` blocks were found, and all SQL queries use parameterized statements for standard queries. However, **2 findings of concern** were identified: a **stored XSS vulnerability** in the dashboard and an **FTS5 query injection** in the search subsystem. The remaining findings are low-severity or informational.

| Severity | Count |
|----------|-------|
| Critical | 1 |
| Medium   | 2 |
| Low      | 3 |
| Info     | 2 |
| **Total** | **8** |

---

## Findings

### FINDING-1: Stored XSS via innerHTML in Dashboard

| Field | Value |
|-------|-------|
| **Severity** | Critical |
| **CWE** | CWE-79 (Improper Neutralization of Input During Web Page Generation) |
| **Location** | `src/dashboard/mod.rs:168`, `src/dashboard/mod.rs:230` |

**Attack Scenario:**
An attacker creates a repository containing a file with a symbol name like `<img src=x onerror=alert(document.cookie)>`. When a developer indexes this repo with `ariadne index` and opens the dashboard, the malicious symbol name is injected directly into the DOM via `innerHTML`, executing arbitrary JavaScript in the developer's browser.

**Evidence — Vulnerable Code (line 168):**
```javascript
searchResults.innerHTML = results.map(r =>
    `<div class="result" data-id="${r.id}"><div class="rname">${r.name}</div><div class="rkind">${r.kind} · ${r.file}</div></div>`
).join('');
```

**Evidence — Vulnerable Code (line 230):**
```javascript
tooltip.innerHTML = `<div class="name">${d.name}</div><div class="detail">${d.kind} · ${d.file}</div>`;
```

Both locations interpolate database-sourced symbol names, kinds, and file paths directly into HTML without sanitization.

**Remediation:**
```javascript
// Add an escape function
function esc(s) {
    const d = document.createElement('div');
    d.textContent = s;
    return d.innerHTML;
}

// Use it in all innerHTML assignments
searchResults.innerHTML = results.map(r =>
    `<div class="result" data-id="${esc(r.id)}"><div class="rname">${esc(r.name)}</div><div class="rkind">${esc(r.kind)} · ${esc(r.file)}</div></div>`
).join('');

tooltip.innerHTML = `<div class="name">${esc(d.name)}</div><div class="detail">${esc(d.kind)} · ${esc(d.file)}</div>`;
```

Or better: use `textContent` and DOM APIs instead of innerHTML.

---

### FINDING-2: Dashboard API has no authentication or CORS protection

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **CWE** | CWE-306 (Missing Authentication for Critical Function) |
| **Location** | `src/dashboard/mod.rs:27-37` |

**Attack Scenario:**
The dashboard binds to `127.0.0.1:1337` with no authentication and no CORS headers. While binding to localhost prevents remote access, any web page loaded in the developer's browser can make fetch requests to `http://127.0.0.1:1337/api/graph` and read the full dependency graph data. This is a classic DNS rebinding / cross-origin localhost attack vector.

**Evidence:**
```rust
let app = axum::Router::new()
    .route("/api/health", axum::routing::get(health_handler))
    .route("/api/stats", axum::routing::get(api::stats))
    .route("/api/graph", axum::routing::get(api::graph_data))
    .route("/api/search", axum::routing::get(api::search_symbols))
    .fallback(axum::routing::get(index_handler))
    .with_state(state);
// No CORS middleware, no auth middleware
```

**Remediation:**
Add restrictive CORS that only allows the dashboard's own origin:
```rust
use tower_http::cors::{CorsLayer, AllowOrigin};

let cors = CorsLayer::new()
    .allow_origin(AllowOrigin::exact(
        format!("http://127.0.0.1:{}", config.port).parse().unwrap()
    ));

let app = axum::Router::new()
    // ... routes ...
    .layer(cors)
    .with_state(state);
```

Note: `tower-http` with `cors` feature is already in `Cargo.toml` but unused.

---

### FINDING-3: FTS5 Query Injection in Search

| Field | Value |
|-------|-------|
| **Severity** | Medium |
| **CWE** | CWE-89 (Improper Neutralization of Special Elements in SQL) |
| **Location** | `src/search/mod.rs:88` |

**Attack Scenario:**
User search input is appended with `*` and passed directly to SQLite FTS5 MATCH. FTS5 has its own query syntax supporting `AND`, `OR`, `NOT`, `NEAR`, column filters (`name:`, `qualified_name:`), and prefix queries. A crafted search like `" OR name:*` could manipulate matching logic. More critically, a complex FTS5 expression with many NEAR clauses could cause excessive CPU usage (DoS).

**Evidence:**
```rust
let fts_query = format!("{}*", query);  // No sanitization
let mut stmt = conn.prepare(
    "... WHERE symbols_fts MATCH ?1 ..."
)?;
```

The query IS parameterized (preventing SQL injection), but FTS5 MATCH interprets the parameter value as an FTS5 query expression, not a literal string.

**Remediation:**
Wrap the query in double quotes to force literal matching:
```rust
// Escape internal double quotes, then wrap
let escaped = query.replace('"', "\"\"");
let fts_query = format!("\"{}\"*", escaped);
```

---

### FINDING-4: LIKE Pattern Wildcard Injection

| Field | Value |
|-------|-------|
| **Severity** | Low |
| **CWE** | CWE-943 (Improper Neutralization of Special Elements in Data Query Logic) |
| **Location** | `src/dashboard/api.rs:169`, `src/search/mod.rs:134`, `src/lsp/mod.rs:61,165,262,375,573` |

**Attack Scenario:**
User search input containing `%` or `_` characters is not escaped before being used in LIKE patterns. Searching for `%` returns all results. The LSP paths use `format!("%{}", suffix)` where `suffix` comes from file URIs — less controllable but still technically unescaped.

**Evidence:**
```rust
let pattern = format!("%{}%", query);  // api.rs:169
let pattern = format!("%{query}%");     // search/mod.rs:134
```

**Remediation:**
```rust
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}
let pattern = format!("%{}%", escape_like(query));
```
And add `ESCAPE '\\'` to the LIKE clause.

---

### FINDING-5: Plugin Install Follows Symlinks

| Field | Value |
|-------|-------|
| **Severity** | Low |
| **CWE** | CWE-22 (Improper Limitation of a Pathname to a Restricted Directory) |
| **Location** | `src/plugins/mod.rs:52-72` |

**Attack Scenario:**
`install_plugin` uses `std::fs::copy` which follows symlinks. If an attacker provides a path to a symlink pointing to a sensitive file (e.g., `/etc/shadow`), the file would be copied to `~/.ariadne/plugins/`. However, this requires local CLI access, limiting the practical impact.

**Evidence:**
```rust
pub fn install_plugin(wasm_path: &Path) -> anyhow::Result<PathBuf> {
    // Only checks extension, not symlink status
    match wasm_path.extension().and_then(|e| e.to_str()) {
        Some("wasm") => {}
        _ => anyhow::bail!("..."),
    }
    std::fs::copy(wasm_path, &dest)?;  // Follows symlinks
```

**Remediation:**
```rust
let metadata = std::fs::symlink_metadata(wasm_path)?;
if metadata.file_type().is_symlink() {
    anyhow::bail!("Symlinks are not supported for plugin installation");
}
```

---

### FINDING-6: Plugin Removal Path Traversal

| Field | Value |
|-------|-------|
| **Severity** | Low |
| **CWE** | CWE-22 (Improper Limitation of a Pathname to a Restricted Directory) |
| **Location** | `src/plugins/mod.rs:179-203` |

**Attack Scenario:**
`remove_plugin("../../important-file")` would construct a path like `~/.ariadne/plugins/../../important-file.wasm` and attempt to delete it if it exists. Requires CLI access.

**Evidence:**
```rust
let exact = dir.join(format!("{}.wasm", name));
if exact.exists() {
    std::fs::remove_file(&exact)?;
```

**Remediation:**
```rust
if name.contains('/') || name.contains('\\') || name.contains("..") {
    anyhow::bail!("Plugin name must not contain path separators");
}
```

---

### FINDING-7: Unbounded Memory Allocation in Graph Construction

| Field | Value |
|-------|-------|
| **Severity** | Info |
| **CWE** | CWE-400 (Uncontrolled Resource Consumption) |
| **Location** | `src/db/query.rs:315-376` |

**Attack Scenario:**
`build_call_graph` loads ALL symbols and calls from the database into an in-memory graph. On a massive codebase (millions of symbols), this could exhaust system memory. The MCP tools `get_call_chain` and `blast_radius` both call this function without any limit. The dashboard graph query has LIMIT clauses (500/2000) that mitigate this for the HTTP path.

**Remediation:**
Add optional limits to `build_call_graph`, or implement lazy/streaming graph construction for the MCP path.

---

### FINDING-8: Absolute File Paths Stored and Potentially Exposed

| Field | Value |
|-------|-------|
| **Severity** | Info |
| **CWE** | CWE-200 (Exposure of Sensitive Information) |
| **Location** | `src/db/schema.rs:21`, `src/db/query.rs:119-120` |

**Attack Scenario:**
The database stores `absolute_path` for every indexed file. If the `.ariadne.db` file is shared (e.g., committed to a repo or transferred), it leaks the developer's filesystem structure including usernames in home directory paths.

**Remediation:**
- Add `.ariadne.db` to `.gitignore` (already present via `.ariadne/` but the DB file itself at root level is not covered)
- Consider stripping or not storing absolute paths when not needed

---

## Positive Findings

| Area | Assessment |
|------|-----------|
| **Memory Safety** | No `unsafe` blocks anywhere. Rust ownership system fully leveraged. |
| **SQL Parameterization** | All standard SQL queries use `?` parameter binding via `rusqlite::params![]`. No string interpolation into SQL. |
| **WASM Sandbox** | Plugin host explicitly disables WASI. No filesystem/network access for plugins. Fresh store per invocation prevents state leakage. |
| **Error Handling** | Error messages are generic (e.g., "Symbol not found"). No stack traces or internal paths leaked to MCP/API clients. |
| **Git Operations** | `git2` used only for read-only operations (commit history for coupling analysis). No credential handling. |
| **Dependency Versions** | All major dependencies are recent versions. `cargo-audit` was not available to run automated advisory checks — recommend installing and running periodically. |
| **LSP Trust Boundary** | LSP communicates over stdio only, inheriting the trust of the parent process (IDE). No network exposure. |
| **Dashboard Binding** | Binds to `127.0.0.1` only, not `0.0.0.0`. Prevents remote network access. |
| **Input via clap** | CLI arguments parsed through `clap` derive macros with type safety. |

---

## Recommendations (Priority Order)

1. **Fix XSS in dashboard** (Critical) — Escape all user-controllable data before innerHTML injection
2. **Add CORS to dashboard** (Medium) — Use the already-imported `tower-http` CORS middleware
3. **Sanitize FTS5 queries** (Medium) — Quote user input to prevent FTS5 query manipulation
4. **Escape LIKE wildcards** (Low) — Escape `%` and `_` in search patterns
5. **Validate plugin names** (Low) — Reject path separators in plugin names
6. **Install and run `cargo-audit`** (Recommendation) — Add to CI pipeline
7. **Add `.ariadne.db` to root `.gitignore`** (Recommendation) — Prevent accidental commit

---

## Audit Scope

| Phase | Coverage | Notes |
|-------|----------|-------|
| Recon / Attack Surface | Full | MCP, Dashboard, LSP, WASM, Git, CLI identified |
| Authentication & Authorization | Full | Dashboard lacks auth (finding), others use stdio |
| Input Validation | Full | FTS5 injection, LIKE wildcards, XSS identified |
| SQL Injection | Full | All queries parameterized — clean |
| Path Traversal | Full | Plugin paths identified |
| Secrets/Credentials | Full | None found in source |
| CORS/HTTP Security | Full | Missing CORS identified |
| WASM Sandbox | Full | Properly configured, no WASI |
| Dependency Audit | Partial | `cargo-audit` not installed; manual version review only |
| DoS Vectors | Full | Unbounded graph construction identified |
| Error Handling/Info Disclosure | Full | Error messages are safe |
| Git Security | Full | Read-only operations, no credential handling |
| Memory Safety | Full | No unsafe code |
