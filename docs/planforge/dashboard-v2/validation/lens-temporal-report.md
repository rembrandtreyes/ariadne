# Lens 2: Temporal Intent Preservation Report

## Summary

ISSUES(2) fixed

Two hard violations were found and fixed in the mini PRDs. The existing draft of this report contained three misdiagnoses (one false positive on PRD-04's `Void.show()` async contract, one false positive on the `ModuleFileSummary` field name, and one inaccurate `localStorage` key description) that have been corrected below.

---

## Hard Violations Found and Fixed

### PRD-04: `selectModule` called `DetailPanel.open(moduleName)` with a string — intent violated

**Master plan intent (Task 11, Task 12):** When the user clicks a module node in Void, `selectModule(moduleName)` highlights the node. The detail panel opens via `DetailPanel.open(symbolId)` with an integer symbol ID, and only when a `symbolId` is available (passed through `Void.show(focusModule, focusSymbol)` → `DetailPanel.open(focusSymbol)`, or via `App.drillDown(moduleName, symbolId)` → `DetailPanel.open(symbolId)`).

**Violation found:** The validated `selectModule` implementation contained:
```javascript
DetailPanel.open(moduleName);  // moduleName is a string like "pipeline"
```
`DetailPanel.open` only accepts integer symbol IDs; passing a string module name causes a `/api/describe?id=pipeline` request that returns an error. The comment even acknowledged this was wrong: "pass the module name as fallback". This silently broke the detail panel for all direct node clicks.

**Fix applied (PRD-04):** Removed the erroneous `DetailPanel.open(moduleName)` call from `selectModule`. The method now only highlights the node and scrolls it into view. Updated the method's docstring, types section, and smoke test step to reflect this. The detail panel flow through `Void.show(focusModule, focusSymbol)` at line 150 is unaffected and correct.

---

### PRD-02: `App.drillDown` saved scroll to `App.signalScrollY` — `Signal.restoreScrollPosition()` read `Signal._scrollY` (always 0)

**Master plan intent (Task 14):** `App.drillDown` saves the Signal scroll position before the transition. `App.goBack` restores it. The mechanism uses `Signal.saveScrollPosition()` (sets `Signal._scrollY`) and `Signal.restoreScrollPosition()` (reads `Signal._scrollY`).

**Violation found:** The original draft of `App.drillDown` in PRD-02 set:
```javascript
App.signalScrollY = window.scrollY;  // wrote to App class field
```
but `App.goBack` called `Signal.restoreScrollPosition()` which reads `Signal._scrollY` — a different field that was never written. The scroll position always restored to 0.

**Fix applied (PRD-02):** Changed `App.signalScrollY = window.scrollY;` to `Signal.saveScrollPosition();`. Also removed the now-unused `static signalScrollY = 0;` field from the `App` class. PRD-02's `App.goBack` correctly calls `Signal.restoreScrollPosition()` and was not changed.

---

## Soft Violations Fixed

### PRD-04: Docstring and smoke test step referred to `DetailPanel.open()` being called from `selectModule`

The method's leading comment said "Opens DetailPanel with the module's first symbol id if DetailPanel is available" and the smoke test step (item 13) described `DetailPanel.open()` being called with the module name. Both were updated to accurately describe the fixed behavior: `selectModule` does not call `DetailPanel.open()`.

---

## Discrepancies in Prior Temporal Report Draft

The prior draft of this report contained three claims that are factually incorrect against the actual PRD content:

1. **False: `DetailPanel.show(moduleName)` called** — The prior report stated PRD-04 calls `DetailPanel.show(moduleName)`. The method in the code was `DetailPanel.open(moduleName)` (not `.show`). The violation was real but misidentified as a non-existent method name.

2. **False: `Void.show()` async/sync inconsistency** — The prior report claimed PRD-04 defines `show()` as synchronous. It does not — PRD-04 line 128 explicitly defines `static async show(focusModule, focusSymbol)`. PRD-02's `await Void.show(moduleName, symbolId)` is consistent with this. No violation exists.

3. **False: `ModuleFileSummary` field `path` in PRD-04** — The prior report claimed PRD-04's API shape doc uses `path` for file entries while PRD-01's Rust struct uses `name`. PRD-04's API example at line 36 correctly shows `"name": "mod.rs"` — no inconsistency.

4. **Inaccurate: localStorage key cited as `ariadne-void-positions`** — The prior report's "Confirmed Preserved" section cited the localStorage key as `ariadne-void-positions` (single key). PRD-04 uses per-module keys `ariadne_void_pos_{moduleName}` (e.g., `ariadne_void_pos_pipeline`). The implementation is correct and matches the master plan; the report description was wrong.

---

## Feature Coverage Verification

- [x] describe.rs narrative/risk formula: present in PRD-01 — `fan_in*0.3 + churn*0.3 + coupling*0.2 + dead*0.2`, `build_narrative()` template function
- [x] Source context param + line_count: present in PRD-01 — `SourceQuery { id, context: Option<u32> }`, `SourceResult { line_count }`, `unwrap_or(0)` default
- [x] Signal health score formula: present in PRD-03 — exact 5-component weighted formula: `resolution_rate*0.30 + (1-dead_ratio)*0.25 + (1-cycle_score)*0.20 + (1-god_score)*0.15 + coupling_health*0.10`
- [x] Void 3-layer auto-layout: present in PRD-04 — `_classifyLayer()` uses in/out coupling degree; Interface=0-incoming, Data=0-outgoing, Core=both; x positions at 15%/50%/85%
- [x] Void flow particles: present in PRD-04 — `requestAnimationFrame` + `SVGPathElement.getPointAtLength()`, one `<circle>` per path, 40px/s speed, staggered starts
- [x] Void 3 color modes: present in PRD-04 — `setMode('architecture'|'risk'|'coupling')` recolors node glows; risk inverts health scale; coupling normalizes pair count
- [x] Detail panel line_count threshold (< 25 / >= 25): present in PRD-05 — `lineCount < 25 ? lines : lines.slice(0, 15)` + "View full source" button
- [x] Search debounce 200ms: present in PRD-03 — `setTimeout(() => Search.query(term), 200)`, min 2 chars to trigger
- [x] App polling 30s: present in PRD-02 — `setInterval(async () => { ... }, 30000)`
- [x] Cascading Escape: present in PRD-02 — order: `SourceModal.isOpen()` → `DetailPanel.isOpen()` → `Search.isOpen()` → `App.currentView === 'void'`
- [x] Signal view: hero section, risk cards (top 5 most-connected), module grid (clickable), coupling rows (clickable), dead code grid (clickable) — all present in PRD-03
- [x] Void view: ambient background orbs, flow particles, drag-to-reposition with localStorage persistence, 3 color modes — all present in PRD-04
- [x] Detail panel: Level C description section, source code with line_count threshold, callers (clickable), callees (clickable), risk factor bars — all present in PRD-05
- [x] App.init() calls Signal.init() then Search.init(): present in PRD-02
- [x] Signal.show/hide, Void.show/hide — correct method signatures used in PRD-02's App controller
- [x] DetailPanel.open(symbolId) — integer symbolId signature preserved across PRD-01, PRD-05 (implementation), and now PRD-04 (after fix)
- [x] CSS design tokens (--bg-void, --accent-primary, --health-green, spacing, radii, transitions, fonts): present in PRD-02 style.css
- [x] last_indexed in /api/health: present in PRD-01 — `db.get_metadata("last_indexed")` in health handler

## API Contract Verification

All JS fetch field names match Rust `#[derive(Serialize)]` struct field names:

| JS field | Rust struct | PRD |
|----------|-------------|-----|
| `modules[*].name` | `ModuleSummary.name` | PRD-01 |
| `modules[*].health` | `ModuleSummary.health` | PRD-01 |
| `modules[*].risk` | `ModuleSummary.risk` | PRD-01 |
| `modules[*].symbol_count` | `ModuleSummary.symbol_count` | PRD-01 |
| `modules[*].file_count` | `ModuleSummary.file_count` | PRD-01 |
| `modules[*].dead_count` | `ModuleSummary.dead_count` | PRD-01 |
| `modules[*].files[*].name` | `ModuleFileSummary.name` | PRD-01/PRD-04 |
| `pairs[*].from_module` | `CouplingPairSummary.from_module` | PRD-01 |
| `pairs[*].to_module` | `CouplingPairSummary.to_module` | PRD-01 |
| `pairs[*].from_file` | `CouplingPairSummary.from_file` | PRD-01 |
| `pairs[*].to_file` | `CouplingPairSummary.to_file` | PRD-01 |
| `pairs[*].strength` | `CouplingPairSummary.strength` | PRD-01 |
| `description` | `DescribeResult.description` | PRD-01 |
| `risk_score` | `DescribeResult.risk_score` | PRD-01 |
| `risk_level` | `DescribeResult.risk_level` | PRD-01 |
| `metrics.fan_in` | `DescribeMetrics.fan_in` | PRD-01 |
| `metrics.blast_radius` | `DescribeMetrics.blast_radius` | PRD-01 |
| `code` | `SourceResult.code` | PRD-01 |
| `line_start` | `SourceResult.line_start` | PRD-01 |
| `line_end` | `SourceResult.line_end` | PRD-01 |
| `line_count` | `SourceResult.line_count` | PRD-01 |
| `language` | `SourceResult.language` | PRD-01 |
| `file` | `SourceResult.file` | PRD-01 |
| `last_indexed` | `serde_json::json!` key | PRD-01 |
