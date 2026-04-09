# Validation Report: Mini PRD 05 — Detail Panel, Source Modal, and Integration

**Mini PRD:** `docs/planforge/dashboard-v2/mini-prds/05-drilldown-navigation-integration.md`
**Validator:** Plan Validator agent
**Date:** 2026-04-08
**Iterations:** 2 (initial assessment, one rewrite pass, re-check passed)

---

## Final Result: PASS (after rewrite)

All 8 haiku-readiness criteria pass in the rewritten mini PRD.

---

## Pre-Validation: Codebase State Check

Files confirmed to exist before validation:
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/api.rs` — exists; does NOT yet contain `describe`, `DescribeQuery`, or `context` field on `SourceQuery` (those are added by PRD-01)
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs` — exists; does NOT yet have `/api/describe` route
- `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs` — exists
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/` — contains `app.js`, `graph-renderer.js`, `index.html`, `style.css` only

Files confirmed NOT to exist:
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/detail-panel.js` — does not exist
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/source-modal.js` — does not exist

---

## Criterion-by-Criterion Findings

### Criterion 1 — File Paths (Absolute paths only)

**Original:** FAIL
**After fix:** PASS

Failures found in original:
1. Files table listed both JS files as `MODIFY` but neither file exists. The correct action is `CREATE`.
2. `tests/test_dashboard.rs` was not listed in the Files table at all, even though the context (item 13) explicitly requires an integration test covering all v2 endpoints to be added there.

Fixes applied:
- Both JS file entries changed from `MODIFY` to `CREATE`.
- `tests/test_dashboard.rs` added to Files table as `MODIFY` with purpose "Add v2 integration test covering all endpoints".
- All file paths in the PRD use absolute paths rooted at `/Users/rembrandt/loremllc/ariadne/`.

---

### Criterion 2 — Function Signatures (All methods documented with parameter names; Rust test functions fully typed)

**Original:** FAIL
**After fix:** PASS

Failures found in original:
1. The Types and Signatures section showed a single collapsed `render(headerEl, contentEl, data)` method. The required public API (context item 8) lists 10 individual render sub-methods: `renderHeader`, `renderDescription`, `renderSource`, `renderCallers`, `renderCallees`, `renderRiskFactors`, `renderBlastRadius`, `renderIssues`. None of these appeared in the Types section.
2. `SourceModal.render(sourceData)` was required by context item 9 but missing from the Types section entirely.
3. Return types were absent for all JS methods in the Types section.
4. The Rust test function `test_dashboard_v2_all_endpoints` was missing from the PRD entirely.

Fixes applied:
- Types and Signatures section rewritten to document all required methods with parameter names, parameter types, return types, and a one-line description of each.
- `_renderRiskBar(label, value, maxVal)` documented as an internal helper.
- `SourceModal.render(sourceData)` added with full parameter shape and return type.
- Rust test function signature added: `async fn test_dashboard_v2_all_endpoints()` annotated `#[tokio::test]`, no parameters, returns `()`.

---

### Criterion 3 — Code Blocks (Complete implementations; no `// ...`; both JS classes fully implemented)

**Original:** PARTIAL FAIL
**After fix:** PASS

Failures found in original:
1. Both JS class implementations were present and copy-pasteable (no stubs). This was already a strength of the original PRD.
2. However, `SourceModal` was missing the public `render(sourceData)` method required by context item 9. The original had all logic inlined inside `open()` with no separate `render()` method.
3. No Rust integration test code block was present. Context item 13 states the test must cover all v2 endpoints (stats, modules, insights, coupling, describe, source). An executor reading only this PRD could not write that test.

Fixes applied:
- `SourceModal.render(sourceData)` extracted as a separate public static method; `open()` calls `render()`.
- Complete Rust integration test `test_dashboard_v2_all_endpoints` added as a code block in Step 4, covering: stats, modules, insights, coupling, search-then-describe, source.
- Import block replacement for `tests/test_dashboard.rs` provided as a copy-pasteable code block.

---

### Criterion 4 — Dependencies (Dependencies declared: PRD-01, 02, 03, 04)

**Original:** PARTIAL FAIL
**After fix:** PASS

Failures found in original:
1. The dependency header listed PRD-01 through PRD-04 by name, which is correct.
2. However, the PRD did not document which specific identifiers PRD-01 must have added to `src/dashboard/api.rs` before this PRD's Rust test can compile. An executor would not know to look for `DescribeQuery`, `describe`, `SourceQuery { context: Option<u32> }`, or `SourceResult { line_count }`.
3. The API response shapes were not documented in the PRD itself (they were only in the calling context), making the PRD not self-contained.

Fixes applied:
- Context section adds an explicit "Prerequisites from PRD-01" block listing exact Rust identifiers required.
- API response shapes for all three endpoints documented verbatim in the Context section.
- DOM containers required from PRD-02 documented verbatim.
- Step 4 adds a prerequisite check note for the executor.

---

### Criterion 5 — Acceptance Criteria (Runnable commands)

**Original:** FAIL
**After fix:** PASS

Failures found in original:
1. Step 4 referenced `cargo test test_xss -- --nocapture`. The actual test function is named `test_xss_regression_html_escaping`. This command would find zero matching tests and exit 0 without running anything, silently passing.
2. The Acceptance Criteria section listed `cargo test test_xss` (same wrong name).
3. No acceptance criterion for `ls` verification that the new files actually exist.
4. No acceptance criterion for the v2 integration test (it was not in the PRD at all).

Fixes applied:
- All references to `test_xss` replaced with the correct name `test_xss_regression_html_escaping`.
- `ls` acceptance criteria added for both JS files.
- `cargo test test_dashboard_v2_all_endpoints -- --nocapture` added as an acceptance criterion.
- Each acceptance criterion has an explicit expected output or exit code.

---

### Criterion 6 — No Ambiguity (Zero weasel words)

**Original:** FAIL
**After fix:** PASS

Failures found in original:
1. The Files table used `MODIFY` for files that do not exist. `MODIFY` is ambiguous when the file does not exist — it could mean "create if absent, modify if present" or it could mean "this step will fail because the file is missing".
2. `highlightSyntax` in `SourceModal` said "Reuse DetailPanel's highlighter if available, otherwise basic escaping." The `typeof DetailPanel !== 'undefined'` conditional guard is an undefined-behavior branch — either `DetailPanel` is always loaded before `SourceModal` (by load order) or it is not. The "otherwise" branch is dead code that implies uncertainty about load order.
3. Step 9 manual test listed subjective items like "animated transition to Void" and "search works and navigates" with no runnable command.

Fixes applied:
- Files table entries corrected to `CREATE` for both JS files.
- `SourceModal._highlightSyntax` removed; `render()` calls `DetailPanel.highlightSyntax` directly with no conditional. Load order is the executor's responsibility (documented in the Imports section).
- Step 12 manual test rewritten with numbered, concrete user actions and expected observable outcomes (no subjective terms like "smoothly" or "correctly").

---

### Criterion 7 — Self-Contained (Executor can write all JS and tests from this PRD alone)

**Original:** FAIL
**After fix:** PASS

Failures found in original:
1. The Rust integration test was not present in the PRD. Context item 13 required it but there was no Step, no code block, and no acceptance criterion for it.
2. `tests/test_dashboard.rs` was not in the Files table, so an executor following the Files table would not know to touch it.
3. The API response shapes for `/api/describe`, `/api/source`, and `/api/graph/neighborhood` were not documented in the PRD body — they were only in the calling context metadata. A self-contained PRD must include all information needed to implement it.
4. The DOM container IDs `#detail-header`, `#detail-content`, `#source-modal-header`, `#source-modal-code` were not documented in the PRD (only `#detail-panel` and `#source-modal` were mentioned).

Fixes applied:
- Step 4 added with complete Rust integration test code block and import replacement.
- `tests/test_dashboard.rs` added to Files table.
- API response shapes documented in Context section.
- DOM container IDs `#detail-header`, `#detail-content`, `#source-modal-header`, `#source-modal-code` added to Context section with explanations.
- Imports section documents load order dependency between detail-panel.js and source-modal.js.

---

### Criterion 8 — Completion Contract (`PLANFORGE_COMPLETE: PRD-05 [description]`)

**Original:** PASS (minor issue)
**After fix:** PASS

The original PRD had a valid Completion Contract with `PLANFORGE_COMPLETE: PRD-05 Detail panel, source modal, and full integration`. However the test list in the Completion Contract referenced `cargo test test_xss` (wrong test name — see Criterion 5). Fixed to `cargo test test_xss_regression_html_escaping`. The v2 integration test added to the Completion Contract. The Files permitted list updated to include `tests/test_dashboard.rs`.

---

## Summary of All Changes Made to Mini PRD

| Section | Change |
|---------|--------|
| Header `> Produces:` | Updated to list absolute paths for both CREATE files and the MODIFY test file |
| Files table | Changed `MODIFY` to `CREATE` for both JS files; added `tests/test_dashboard.rs` as MODIFY |
| Context section | Added Prerequisites from PRD-01, API response shapes, DOM container IDs, source display rule, XSS rule |
| Step 1 (detail-panel.js) | Extracted 8 `renderX` methods as separate named public static methods instead of collapsed into `render()`; `renderSource` stores `_lastSource` before returning HTML; `renderHeader` uses `.detail-close` class matching PRD-02 DOM |
| Step 2 (source-modal.js) | Added `render(sourceData)` as a separate public static method; `open()` calls `render()`; removed conditional `typeof DetailPanel` guard |
| Step 3 (esc audit) | Updated checklist to match new method names; line range escaping in source-modal corrected |
| Step 4 (NEW) | Added complete Rust integration test for all v2 endpoints with import block replacement |
| Step 5 | Corrected `test_xss` to `test_xss_regression_html_escaping` |
| Steps 6-12 | Renumbered; Step 10 adds `ls` verification; Step 11 adds `grep -c` method count check |
| Acceptance Criteria | Added `ls` file-existence checks; added v2 integration test command; corrected XSS test name |
| Types and Signatures | Rewritten: all 14 methods documented with parameter names, types, return types, and descriptions |
| Imports | Load order dependency between detail-panel.js and source-modal.js documented |
| Completion Contract | Test list updated with correct test names and v2 integration test; Files list updated |

---

## Files Modified

- `/Users/rembrandt/loremllc/ariadne/docs/planforge/dashboard-v2/mini-prds/05-drilldown-navigation-integration.md` — rewritten in place
