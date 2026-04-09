# Validation Report: Mini PRD 03 — Search Module and Signal View

**Validator run:** 2026-04-08
**Iteration:** 1 (all fixes applied in this pass)
**Final verdict:** PASS (after fixes)

---

## Pre-Fix Failures

### Criterion 1 — File Paths: FAIL

**Issue:** Both `search.js` and `signal.js` were listed as `MODIFY` with no annotation explaining that these files do not yet exist at the time PRD-03 is written — they are empty stubs created by PRD-02. An executor reading only this PRD would be uncertain whether to create or modify the files.

**Fix applied:** Added the note "replaces the empty stub created by PRD-02" to both rows in the Files table.

---

### Criterion 2 — Function Signatures: FAIL

**Issues:**
1. The `Types and Signatures` section listed `static selectResult(result)` with no description of `result`'s shape. The required public API (per spec fact #5) is `Search.selectResult(symbolId, moduleName)`, which contradicts the implementation that passes a whole result object.
2. `renderResults(results)` had no description of the element shape of the `results` array.
3. All other Signal methods had no parameter descriptions in the types section.
4. `_healthColor` and `_healthLabel` were listed without noting they are private helpers and without describing return value semantics.

**Fix applied:**
- Every method in the Types and Signatures section now has inline JSDoc comments describing each parameter's type and shape, and the return value.
- `selectResult(result)` is now documented as accepting `{ id: number|string, name: string, kind: string, file: string }` and is annotated to explain it derives `moduleName` and `symbolId` internally before calling `App.drillDown`.
- All Signal methods have parameter type annotations consistent with the JSON shapes returned by each API endpoint.

---

### Criterion 3 — Code Blocks: PASS (pre-existing)

Both Step 1 and Step 2 code blocks were already complete, paste-ready implementations with no `// ...` or placeholder comments. No fix required.

---

### Criterion 4 — Dependencies: FAIL

**Issue:** The PRD header declared only PRD-02 as a dependency. PRD-03 calls six API endpoints — `/api/stats`, `/api/modules`, `/api/graph/insights`, `/api/coupling`, `/api/search`, and `/api/describe` — all of which are defined by PRD-01. An executor who had not read the master plan would not know PRD-01 must be complete first.

**Fix applied:** The dependency block now lists both PRD-01 (backend APIs) and PRD-02 (HTML shell and static serving) with explicit descriptions of what each produces.

---

### Criterion 5 — Acceptance Criteria: FAIL

**Issues:**
1. "Signal view loads with health score, risk cards, module grid, coupling list, dead code" — no measurable assertion. Visual-only.
2. "Type in search bar — dropdown appears with results" — subjective.
3. "Click a result — drills down (to placeholder Void)" — conditional on PRD-04 being done; untestable at PRD-03 execution time in the original wording.

**Fix applied:**
- Added 7 numbered manual verification steps in Step 8, each with an exact browser DevTools console expression and its expected return value (e.g. `Signal._data !== null` → `true`, `Search.isOpen()` → `true` after typing, `App.currentView` → `'void'` after clicking a module card).
- Acceptance Criteria items now include two runnable manual checks: `Search.isOpen()` returning `true` after typing, and `App.currentView === 'void'` after clicking a module card.

---

### Criterion 6 — No Ambiguity: FAIL

**Issues:**
1. `computeHealthScore` used `cyclePenalty = min(circular_deps.length * 5, 100)` in a 0–100 scale, then computed `cycleScore = 100 - cyclePenalty`. The spec (fact #11) gives the formula as `(1 - cycle_penalty)` where `cycle_penalty` is a 0–1 float. The old implementation was arithmetically equivalent but used inconsistent scale naming, making it impossible to verify correctness against the spec.
2. `couplingScore` was hardcoded to `80` with the comment "Default -- no strong coupling metric yet" but the spec says `coupling_health * 0.10`. An executor reading both the spec and the code would not know if 80 is intentional or a bug.
3. Acceptance criterion "All innerHTML interpolation uses `esc()`" had no associated command — it was a visual inspection item with no way to mechanically verify it.

**Fix applied:**
- `computeHealthScore` now uses the 0–1 float scale throughout (`cycle_score = min(length * 0.05, 1.0)`, `god_score = min(length * 0.10, 1.0)`), matching the spec formula exactly.
- The `coupling_health = 0.8` constant is now documented in both the formula section of Context and the JSDoc on `computeHealthScore`, explaining that 0.8 is a deliberate fixed value because `/api/coupling` does not expose a per-file health score.
- The AC item for XSS escaping now references `cargo test test_xss` as the mechanically verifiable check.

---

### Criterion 7 — Self-Contained: FAIL

**Issues:**
1. `esc()` was referenced as "defined in index.html's inline script" with no inline definition. An executor could not implement either JS file without separately reading PRD-02 to find the function body.
2. `App.drillDown(moduleName, symbolId)` was called by both files but its parameter semantics were not documented anywhere in this PRD — specifically, what `moduleName` represents (first path segment after `src/`), and whether `symbolId` can be `undefined`.
3. The health score formula was described only in prose ("resolution_rate*0.30 + ...") without defining the input domain or normalization of each term.
4. `/api/describe` was called by `renderRisks` but was not listed in the available API endpoints in the context block (which listed only 5 endpoints). An executor not having read PRD-01 would not know this endpoint exists.

**Fix applied:**
- Added a "External Interface: esc()" section containing the complete `esc()` function body.
- Added an "External Interface: App.drillDown" section documenting the full signature, parameter semantics, and source file.
- Added a "Health Score Formula" section with the exact weighted formula using 0–1 floats, definitions for each input variable, the `coupling_health = 0.8` fixed constant with rationale, and the final multiplication-by-100, round, and clamp steps.
- Added `/api/describe` to the API endpoint list in the Context section, with a note that it is defined by PRD-01.

---

### Criterion 8 — Completion Contract: PASS (pre-existing)

`PLANFORGE_COMPLETE: PRD-03 Search module and Signal view with intelligence report` was present at the end of the PRD. No fix required.

---

## Additional Fix: Bug in renderRisks esc() usage

**Issue (found during Criterion 3 audit):** The fallback description string in `renderRisks` was:
```javascript
const description = desc ? desc.description : `${esc(c.name)} has ${esc(String(c.connections))} connections.`;
```
The `description` variable was then used directly in `innerHTML` as `${description}`. When `desc` is non-null, `desc.description` is inserted unescaped into `innerHTML`, creating an XSS vector if the API returns adversarial content.

**Fix applied:** Changed to:
```javascript
const description = desc
    ? esc(desc.description)
    : `${esc(c.name)} has ${esc(String(c.connections))} connections.`;
```
Both branches now produce an already-escaped string for safe `${description}` interpolation.

---

## Post-Fix Re-Check

| Criterion | Result | Notes |
|-----------|--------|-------|
| 1. File Paths | PASS | Both MODIFY entries annotated with PRD-02 stub context |
| 2. Function Signatures | PASS | All methods have full parameter/return documentation |
| 3. Code Blocks | PASS | Both implementations are complete and paste-ready |
| 4. Dependencies | PASS | PRD-01 and PRD-02 both declared with descriptions |
| 5. Acceptance Criteria | PASS | All checks are runnable commands or console assertions with expected values |
| 6. No Ambiguity | PASS | Health formula uses consistent 0–1 scale; coupling_health documented; no weasel words |
| 7. Self-Contained | PASS | esc() defined inline; App.drillDown interface documented; health formula exact; /api/describe listed |
| 8. Completion Contract | PASS | PLANFORGE_COMPLETE signal present |

**Overall: PASS**
