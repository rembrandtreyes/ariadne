# Lens 4: State/Mutation Report

## Summary

ISSUES(2) fixed

---

## Shared File Analysis

### `src/dashboard/mod.rs` — SAFE ADDITIVE

PRD-01 and PRD-02 both modify this file but touch disjoint sections:

| PRD | Changes |
|-----|---------|
| PRD-01 | Adds `pub mod describe;` after `pub mod api;`; adds `.route("/api/describe", ...)` to router chain; replaces `health_handler` function body |
| PRD-02 | Adds 6 `include_str!` constants at end of file; adds 6 static-file handler functions after `graph_renderer_js_handler`; adds 6 `.route(...)` calls to router chain |

No PRD modifies the same function body as the other. No PRD adds `.route()` calls to the same path. Batch 1 parallel execution may produce a trivially-resolvable merge conflict (both add `.route()` lines to the same chain), but the changes themselves do not overlap. **SAFE ADDITIVE.**

### `tests/test_dashboard.rs` — SAFE ADDITIVE

| PRD | Changes |
|-----|---------|
| PRD-01 | Updates import block (adds `describe`, `source`, `DescribeQuery`, `SourceQuery`); adds 3 new test functions: `test_dashboard_describe_handler`, `test_dashboard_source_full_body`, `test_dashboard_v2_endpoints_basic` |
| PRD-02 | Replaces `test_xss_regression_html_escaping` function body |
| PRD-05 | Updates import block to same final content as PRD-01 (idempotent); adds new test function `test_dashboard_v2_all_endpoints` |

PRD-01 and PRD-02 touch different functions. PRD-05 runs after both (Batch 3) so its import re-replacement is idempotent. All three test function names are unique — no duplicate function names. **SAFE ADDITIVE.**

---

## CSS Conflicts

CLEAN. Only PRD-02 writes `style.css`. PRDs 03–05 are pure JavaScript and add no inline `<style>` blocks. Inline `style` attribute values applied via JavaScript (e.g., `el.style.cssText = ...` in Void node orbs, glow colors via `el.style.boxShadow`) use only hardcoded CSS variable references and numeric pixel values — no class-based CSS conflicts with `style.css`.

---

## DOM State Conflicts

CLEAN. Initial state is consistent across all PRDs:

| Element | Initial class state | Set by | Show action | Hide action |
|---------|---------------------|--------|-------------|-------------|
| `#signal-view` | visible (no `hidden`) | PRD-02 HTML | `Signal.show()` removes `hidden`/`fade-out`, adds `fade-in` | `Signal.hide()` adds `fade-out`, then `hidden` after 250ms |
| `#void-view` | `hidden` | PRD-02 HTML | `Void.show()` removes `hidden`/`fade-out`, adds `fade-in` | `Void.hide()` adds `fade-out`, then `hidden` after 250ms |
| `#detail-panel` | off-screen (`translateX(100%)`) | PRD-02 CSS | `DetailPanel.open()` adds `detail-panel--open` | `DetailPanel.close()` removes `detail-panel--open` |
| `#source-modal` | `hidden` | PRD-02 HTML | `SourceModal.open()` removes `hidden` | `SourceModal.close()` adds `hidden` |

The CSS selector in PRD-02 (`style.css`) uses `.detail-panel--open` and PRD-05's `detail-panel.js` also uses `detail-panel--open` in `classList.add/remove`. **Consistent.**

The HUD buttons in PRD-02's HTML use `data-mode="architecture"` / `data-mode="risk"` / `data-mode="coupling"`. PRD-04's `Void.setMode()` queries `'.void-hud__btn[data-mode]'`. **Consistent.**

No Signal/Void init double-fire: PRD-02's `index.html` has exactly one `DOMContentLoaded` handler — `document.addEventListener('DOMContentLoaded', () => App.init())` — which calls `Signal.init()` once via `App.init()`. No second inline boot exists.

---

## Database State

CLEAN. No PRD adds any `CREATE TABLE`, `ALTER TABLE`, or migration SQL. All database interactions are read-only queries against the existing schema. No schema mutation risk.

---

## Global JavaScript State

### `App.currentView` — CLEAN

Set to `'void'` in `App.drillDown` (after both `Void.show()` and the back-button DOM update complete). Set to `'signal'` in `App.goBack` (after both `Void.hide()` and `Signal.show()` complete). Both mutators are `async` and called sequentially — `drillDown` and `goBack` are never concurrent because they are triggered by user gestures. No race condition.

### `App.signalScrollY` — REMOVED (no longer exists in PRD-02 after fix)

PRD-02 previously declared `static signalScrollY = 0` and set it in `drillDown` via `App.signalScrollY = window.scrollY`. However, `Signal.restoreScrollPosition()` reads `Signal._scrollY` (not `App.signalScrollY`), and `Signal._scrollY` was only updated by `Signal.saveScrollPosition()` which was never called. This caused scroll restore to always jump to position 0 — a soft bug. **Fixed (see Hard Violations Fixed below).**

### Signal/Void `_scrollY` — CLEAN after fix

`Signal._scrollY` is now populated correctly because `App.drillDown` calls `Signal.saveScrollPosition()`. `Signal.restoreScrollPosition()` reads `Signal._scrollY`. Single writer, single reader, no concurrency. **CLEAN.**

---

## Hard Violations Fixed

### Fix 1 — Double `DetailPanel.open()` on drilldown

**Location:** `App.drillDown` in PRD-02 `index.html` App controller

**Problem:** `App.drillDown(moduleName, symbolId)` called `await DetailPanel.open(symbolId)` synchronously after `await Void.show(moduleName, symbolId)`. But `Void.show()` already schedules `DetailPanel.open(focusSymbol)` via `setTimeout(100ms)` when `focusSymbol` is provided. This caused `DetailPanel.open()` to fire twice: once immediately (from `App.drillDown`) and once ~100ms later (from `Void.show()`'s timeout). Result: two parallel `/api/describe` + `/api/source` + `/api/graph/neighborhood` requests, a loading-state flash, and the content rendered by the first call overwritten by the second.

**Fix applied in PRD-02 (`index.html` App controller `drillDown` method):**

Removed the redundant `if (symbolId) { await DetailPanel.open(symbolId); }` block. `Void.show()` is the sole caller of `DetailPanel.open()` when a `symbolId` is present.

```javascript
// BEFORE (broken):
static async drillDown(moduleName, symbolId) {
    App.signalScrollY = window.scrollY;
    Signal.hide();
    await new Promise(r => setTimeout(r, 200));
    await Void.show(moduleName, symbolId);
    App.currentView = 'void';
    document.getElementById('back-btn').classList.remove('hidden');
    if (symbolId) {
        await DetailPanel.open(symbolId);   // <-- duplicate call
    }
}

// AFTER (fixed):
static async drillDown(moduleName, symbolId) {
    Signal.saveScrollPosition();
    Signal.hide();
    await new Promise(r => setTimeout(r, 200));
    await Void.show(moduleName, symbolId);
    App.currentView = 'void';
    document.getElementById('back-btn').classList.remove('hidden');
    // DetailPanel.open() is handled inside Void.show() via setTimeout(100ms) when symbolId is provided.
    // Do NOT call it again here -- that would open the panel twice (double API call + content flash).
}
```

---

## Soft Violations Fixed

### Fix 2 — Scroll position stored in wrong variable

**Location:** `App.drillDown` in PRD-02 `index.html`

**Problem:** `App.drillDown` stored `window.scrollY` in `App.signalScrollY` (a static field on `App`). But `App.goBack` calls `Signal.restoreScrollPosition()` which reads `Signal._scrollY`. `Signal._scrollY` was initialized to `0` and never updated unless `Signal.saveScrollPosition()` was called — which it wasn't. Scroll restore always jumped to top.

**Fix applied in PRD-02 (same edit as Fix 1):** Replaced `App.signalScrollY = window.scrollY` with `Signal.saveScrollPosition()`. Removed the unused `static signalScrollY = 0` field. Scroll save/restore now uses the single canonical path: `Signal.saveScrollPosition()` writes `Signal._scrollY`, `Signal.restoreScrollPosition()` reads `Signal._scrollY`.

---

## Additional Notes

- The dependency graph notes `CREATE` vs `MODIFY` label inconsistency for `detail-panel.js` and `source-modal.js` in PRD-05 (they already exist as stubs from PRD-02 when PRD-05 runs). This is a spec label ambiguity only — the step prose says "Create new file (does not exist yet)" which is stale, but the action (write the full content) is correct. PRD-05 runs after PRD-02 in Batch 3 so the stubs will be present. An agent must overwrite the stubs. Low risk: most write tools overwrite by default.
- PRD-03's XSS test verification (Step 3) passes a note that `void-renderer.js` is not in the XSS test's JS file list — confirmed by reading PRD-02's `test_xss_regression_html_escaping`, which checks `search.js`, `signal.js`, `detail-panel.js`, `source-modal.js` but not `void-renderer.js`. PRD-04's XSS audit is manual-only. This is acceptable: `void-renderer.js` uses `esc()` in all `innerHTML` template interpolations (verified by reading PRD-04 source), and the test covers the highest-risk files.
