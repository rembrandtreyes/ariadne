# Validation Report: Mini PRD 03 — Search + Signal View

**Validator run:** 2026-04-07  
**Iteration:** 1 (fix applied in same pass)  
**Final verdict:** PASS (after fixes)

---

## Pre-Fix Failures

### Criterion 1 — File Paths: FAIL

**Issue:** Both `src/dashboard/static/search.js` and `src/dashboard/static/signal.js` are listed as MODIFY in the Files table, but neither file exists yet when Mini PRD 03 runs in isolation. They are created as empty stubs by Mini PRD 02. The table gave no note explaining this, leaving an executor uncertain whether to create or modify them.

**Fix applied:** Files table retains MODIFY action with an explicit note: "Empty stub created by Mini PRD 02 — replace with full implementation."

---

### Criterion 2 — Function Signatures: FAIL

**Issues:**
1. Step 1 code blocks in both Task 9 and Task 10 used stub class outlines with `/* ... */` comment bodies — not actual implementations.
2. In the original Types section, `selectResult(symbolId, moduleName)` had no parameter types.
3. `renderResults(results: Array)` had an untyped array element type.
4. `renderRisks` was declared `static` (synchronous) in the original types section but calls `await fetch(...)` — return type should be `Promise<void>`.

**Fix applied:** Replaced stub class outlines with complete implementations in Step 1 of each task. Updated Types section with full parameter and return types on every method, including correcting `renderRisks` to `async renderRisks(...): Promise<void>`.

---

### Criterion 3 — Code Blocks: FAIL

**Issue:** Both Task 9 Step 1 and Task 10 Step 1 contained class skeletons with `/* ... */` placeholder method bodies. These are not copy-pasteable implementations — an executor would have to invent behavior.

**Fix applied:** Both Step 1 blocks are now complete, paste-ready JavaScript implementations with no placeholder comments.

---

### Criterion 4 — Dependencies: FAIL

**Issue:** The dependency line listed only Mini PRD 02 as a prerequisite. Mini PRD 01 (Backend APIs) is an implicit dependency — all six `/api/*` endpoints consumed by these modules are defined there. An executor who hadn't read the master plan would not know Mini PRD 01 must be complete first.

**Fix applied:** Dependency line updated to: "Dependencies: Mini PRD 01 (backend APIs must exist), Mini PRD 02 (HTML shell and static serving must exist)."

---

### Criterion 5 — Acceptance Criteria: FAIL

**Issues:**
1. AC item 1: "Signal view loads with health score, risks, modules, coupling, dead code" — no measurable assertion.
2. AC item 2: "verify dropdown appears" — subjective visual check.
3. AC item 5: "triggers drill-down (will show Void once Task 11 is done)" — conditional on a future task, which makes the criterion untestable at the time Mini PRD 03 is executed.

**Fix applied:** Manual verification steps replaced with browser devtools console commands and their exact expected return values (`true`/`false`/`> 0`). Removed the conditional "(will show Void once Task 11 is done)" clause.

---

### Criterion 6 — No Ambiguity: FAIL

**Issues:**
1. "top 3-5 most-connected symbols" — ambiguous range. An executor cannot know whether to fetch 3 or 5.
2. "verify dropdown appears with results" — subjective.
3. "Verify: Signal view loads with real data, all sections populated" — subjective.

**Fix applied:**
1. Changed to exactly 3 god objects (`insights.god_objects.slice(0, 3)`), encoded in the implementation code itself.
2. Manual checks replaced with console assertions (see Criterion 5 fix).

---

### Criterion 7 — Self-Contained: FAIL

**Issues:**
1. Both modules call `App.drillDown(moduleName, symbolId)` but the document never defined this interface — parameter types, where `App` comes from, or what `null` means for `symbolId`.
2. `computeHealthScore` listed five weighted components by name but gave no arithmetic: what is the input domain of each component? How is each normalized before weighting? What does "dead_code_ratio" mean concretely?
3. `esc()` was referenced as "defined in index.html" with no definition, making it impossible to use or replicate in isolation.

**Fix applied:**
1. Added "External Interface: App.drillDown" section defining the full signature, parameter semantics, and source file.
2. `computeHealthScore` formula section now shows exact arithmetic for all five components, including definitions of `dead_code_ratio`, `clamp`, denominator guards, and final rounding/clamping. The formula is also embedded verbatim in JSDoc on the method.
3. Added the exact `esc()` implementation inline in the Existing Code Context section.

---

## Post-Fix Re-Check

| Criterion | Result | Notes |
|-----------|--------|-------|
| 1. File Paths | PASS | Both paths correct; action labels annotated with Mini PRD 02 context |
| 2. Function Signatures | PASS | All methods have typed params and return types; `renderRisks` correctly `async` |
| 3. Code Blocks | PASS | Both Step 1 blocks are complete, copy-pasteable implementations |
| 4. Dependencies | PASS | Both Mini PRD 01 and 02 declared |
| 5. Acceptance Criteria | PASS | All manual checks use console commands with exact expected return values |
| 6. No Ambiguity | PASS | No ambiguous ranges or subjective phrases remain |
| 7. Self-Contained | PASS | App.drillDown interface defined; health formula exact; esc() defined inline |

**Overall: PASS**
