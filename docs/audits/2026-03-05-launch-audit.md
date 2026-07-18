# Launch Audit Report — Ariadne

> **Status (2026-07-16): historical.** Both blocking issues and all three
> warnings below were subsequently fixed — the dead-code test passes, wasmtime
> is at 42 (CVEs cleared), and fmt/clippy run clean in CI. Kept for the record;
> re-run a fresh audit before the actual launch.

Generated: 2026-03-05

## Summary
- **Overall Grade:** C
- **Pass:** 13 checks
- **Warning:** 3 checks
- **Fail:** 2 checks (BLOCKING)
- **Blocking Issues:** 2 (must fix before launch)

---

## Blocking Issues (Must Fix)

### BLOCK-1: FAILING TEST — `test_dead_code_detects_unreachable_function`
**File:** `tests/test_dead_code.rs:36`
**Severity:** BLOCKING — CI will fail on every push/PR
**Detail:** After running `run_full_pipeline` on `tests/fixtures/python_repo`, the test asserts `get_dead_symbols()` returns at least one result. It returns empty. Pipeline reports "Call resolution: 1/2 resolved (50%), 50% categorized, 1 unresolved."

**Root cause:** `UserService.get_user` and `UserService.create_user` in the fixture are never called but likely getting flagged `is_exported = 1` by the Python parser (no leading underscore heuristic). The dead code BFS seeds from exported symbols, so they appear reachable.

**Fix options:**
1. Tighten Python parser: only export symbols in `__all__`, top-level module functions, or explicitly `@property` decorated — not all instance methods.
2. Add an explicit unreachable standalone function to the fixture to guarantee detection.
3. Debug: `RUST_LOG=debug cargo test test_dead_code_detects_unreachable_function -- --nocapture`

**Verification:** `cargo test test_dead_code_detects_unreachable_function` must pass.

---

### BLOCK-2: WASMTIME — 2 Medium CVEs (6.9/10)
**File:** `Cargo.lock` (wasmtime v28.0.1)
**Severity:** BLOCKING — medium severity vulnerabilities in plugin execution engine
- `RUSTSEC-2026-0020`: Guest-controlled resource exhaustion in WASI (6.9) — malicious wasm plugin can exhaust host resources
- `RUSTSEC-2026-0021`: Panic adding excessive fields to `wasi:http/types.fields` (6.9) — DoS via crafted plugin input

**Fix:** Upgrade wasmtime in `Cargo.toml`:
```toml
# Change:
wasmtime = "28"
# To (fixed range):
wasmtime = "40"
```
Note: wasmtime 40 has API changes. Audit `src/plugins/host.rs` after upgrade.

**Verification:** `cargo audit` — must show 0 vulnerabilities.

---

## Warnings (Should Fix Before Launch)

### WARN-1: `cargo fmt --check` FAILS
**File:** `src/dashboard/api.rs:121`
**Risk:** CI `fmt` job will block merges
**Fix:** `cargo fmt` (30 seconds)
**Verification:** `cargo fmt --check`

### WARN-2: Unmaintained dependency warnings (4)
All transitive via wasmtime/notify/indicatif. Most resolve when wasmtime is upgraded.
- `fxhash 0.2.1` (RUSTSEC-2025-0057) — via wasmtime
- `instant 0.1.13` (RUSTSEC-2024-0384) — via notify
- `number_prefix 0.4.0` (RUSTSEC-2025-0119) — via indicatif
- `paste 1.0.15` (RUSTSEC-2024-0436) — via wasmtime/rmcp

### WARN-3: Additional low-severity wasmtime CVEs (2)
- `RUSTSEC-2025-0046` (3.3 low): `fd_renumber` WASIp1 panic — fixed in wasmtime 40
- `RUSTSEC-2025-0118` (1.8 low): Unsound shared linear memory — fixed in wasmtime 40

Both resolved by the BLOCK-2 wasmtime upgrade.

---

## Passed (13/15 checks)

| # | Check | Evidence |
|---|-------|---------|
| C1 | No unwrap/expect in production paths | All .unwrap()/.expect() in non-test code are on infallible ops (hardcoded strings, test-scoped DB helpers) |
| C2 | No SQL injection vectors | format!() uses only internal constant `RESOLUTION_UNRESOLVED`. All user input uses params![] binding |
| C4 | cargo build --release succeeds | Completed 1m 45s, zero errors |
| C6 | cargo clippy clean | `cargo clippy --all-features -- -D warnings` — zero diagnostics |
| C7 | CI pipeline present and correct | Matrix build (ubuntu/macos/windows), test, clippy -D warnings, rustfmt, rustsec/audit-check |
| C8 | Dashboard API security | Binds 127.0.0.1 only, CORS restricted to localhost:{port}, read-only GETs, no external exposure |
| C9 | Error handling properly propagated | All pipeline fns return anyhow::Result<()>; API uses .and_then()/.ok() for graceful degradation |
| C10 | Documentation present | README.md — install, quick start, MCP integration, configuration, language matrix, architecture |
| C11 | Path traversal protection | file_path from MCP tools = DB lookup key only (parameterized SQL). No filesystem access from user input |
| C12 | MCP tool input validation | get_string_param/get_int_param helpers; DB lookup returns error on missing file |
| A1 | No hardcoded credentials | Grepped sk_live_, sk_test_, eyJhbG, password=, api_key= — zero matches |
| A2 | DB files in .gitignore | .ariadne.db, **/.ariadne.db-shm, **/.ariadne.db-wal all excluded |
| A3 | No panic paths from user input | Input → parameterized SQL → DB lookup only. No fs traversal. No untrusted size arithmetic |

---

## Pre-Launch Manual Checklist

- [ ] Fix failing test: `cargo test test_dead_code_detects_unreachable_function` must pass
- [ ] Upgrade wasmtime to 40.x — verify `src/plugins/host.rs` still compiles
- [ ] Run `cargo fmt` to fix formatting drift in `src/dashboard/api.rs`
- [ ] Run `cargo audit` after wasmtime upgrade — verify 0 vulnerabilities
- [ ] Run `cargo test --all-features` — all tests must pass clean
- [ ] Push and verify CI passes all jobs (test matrix, fmt, audit)
- [ ] Verify `demo.gif` referenced in README exists in repo
- [ ] Verify `LICENSE-MIT` exists (referenced in README badge)
- [ ] Verify crates.io Cargo.toml metadata (description, keywords, categories, repository field)

---

## Category Grades

| Category | Grade | Notes |
|----------|-------|-------|
| Security | A | No secrets, no SQL injection, parameterized queries, local-only dashboard |
| Build | A | Release build clean in 1m 45s |
| Tests | F | 1 failing test blocks CI |
| Static Analysis | A | Clippy clean |
| Dependencies | D | 2 medium CVEs in wasmtime; 4 unmaintained |
| CI/CD | A | Comprehensive multi-OS pipeline |
| Documentation | A | README is thorough and well-structured |
| Error Handling | A | Proper Result propagation throughout |

**Overall: Grade C** — 2 blocking issues. Estimated fix time: 2-3 hours (fmt: 30s, wasmtime upgrade: 1hr, dead code test debug: 1-2hr).
