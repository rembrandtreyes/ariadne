# Mini PRD 05: Detail Panel, Source Modal, and Integration

> **Dependency:** Requires PRD-01 (backend APIs for /api/describe, /api/source with context param, DescribeQuery, SourceQuery), PRD-02 (HTML shell with #detail-panel, #source-modal containers), PRD-03 (Signal.init() and Search.init()), PRD-04 (Void.show() and Void.selectModule())
> **Produces:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/detail-panel.js` (CREATE), `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/source-modal.js` (CREATE), updated integration test in `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs`
> **Estimated steps:** 12

## Context

This PRD completes the Dashboard v2 frontend by implementing the detail panel (right slide-in showing Level C descriptions, source code, callers/callees, and risk metrics), the source modal (full-screen code viewer for long functions), and final integration testing. The detail panel is the primary drilldown destination -- clicking a symbol in Void or search opens it. The source modal opens from a "View full source" button in the detail panel when `line_count >= 25`.

**Prerequisites from PRD-01 that must exist before executing this PRD:**
- `src/dashboard/api.rs` exports: `describe`, `DescribeQuery`, `source`, `SourceQuery { id: i64, context: Option<u32> }`, `SourceResult { code, line_start, line_end, line_count, language, file }`
- `/api/describe?id=N` route registered in `src/dashboard/mod.rs`

**API response shapes (from PRD-01):**
- `/api/describe?id=N` returns `{ description: string, role: string, risk_level: string, risk_score: number, metrics: { fan_in, fan_out, modification_count, author_count, is_volatile, blast_radius, coupled_file_count, max_coupling_strength } }`
- `/api/source?id=N&context=0` returns `{ code: string, line_start: number, line_end: number, line_count: number, language: string, file: string }`
- `/api/graph/neighborhood?id=N&depth=1` returns `{ nodes: Array<{ id: string, name: string, kind: string, file: string }>, edges: Array<{ source: string, target: string }> }`

**DOM containers from PRD-02 index.html:**
- `#detail-panel` — slide-in panel root
- `#detail-header` — panel header container (inside #detail-panel)
- `#detail-content` — panel body container (inside #detail-panel)
- `#source-modal` — full-screen modal root
- `#source-modal-header` — modal header container
- `#source-modal-code` — modal code container
- `.detail-panel--open` CSS class toggles panel visibility

**Source display rule:** if `line_count < 25`, show full code inline in the detail panel; if `line_count >= 25`, show first 15 lines inline and render a "View full source" button that calls `SourceModal.open(source)`.

**Syntax highlighting colors:** keywords → `<span class="syn-keyword">`, strings → `<span class="syn-string">`, comments → `<span class="syn-comment">`, numbers → `<span class="syn-number">`. CSS must define these classes (done in PRD-02).

**XSS rule:** every `${...}` inside an `innerHTML` assignment must be wrapped with `esc()`. `esc()` is defined in `index.html` (from PRD-02) and is available globally. `highlightSyntax` calls `esc(line)` first and then wraps with `<span>` tags — that output is safe to interpolate without a second `esc()` call.

## Files

| Action | Path | Purpose |
|--------|------|---------|
| CREATE | `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/detail-panel.js` | Full detail panel implementation |
| CREATE | `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/source-modal.js` | Full source modal implementation |
| MODIFY | `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs` | Add v2 integration test covering all endpoints |

## Steps

### Step 1: Create detail-panel.js

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/detail-panel.js`
**Location:** Create new file (does not exist yet)

```javascript
// Ariadne Dashboard v2 -- Detail Panel
'use strict';

class DetailPanel {
    static _open = false;
    static _currentSymbolId = null;
    static _lastSource = null;

    static async open(symbolId) {
        DetailPanel._currentSymbolId = symbolId;
        DetailPanel._open = true;

        const panel = document.getElementById('detail-panel');
        const content = document.getElementById('detail-content');
        const header = document.getElementById('detail-header');
        if (!panel || !content || !header) return;

        content.innerHTML = '<div style="padding: 24px; color: var(--text-muted);">Loading...</div>';
        panel.classList.add('detail-panel--open');

        try {
            const data = await DetailPanel.fetchData(symbolId);
            DetailPanel.renderHeader(header, data.selfNode, data.describe);
            content.innerHTML =
                DetailPanel.renderDescription(data.describe) +
                DetailPanel.renderSource(data.source) +
                DetailPanel.renderRiskFactors(data.describe ? data.describe.metrics : null) +
                DetailPanel.renderBlastRadius(data.describe ? data.describe.metrics : null) +
                DetailPanel.renderCallers(data.callers) +
                DetailPanel.renderCallees(data.callees) +
                DetailPanel.renderIssues(data.selfNode, data.describe);
        } catch (e) {
            console.error('DetailPanel error:', e);
            content.innerHTML = '<div style="padding: 24px; color: var(--text-muted);">Failed to load details.</div>';
        }
    }

    static close() {
        DetailPanel._open = false;
        DetailPanel._currentSymbolId = null;
        const panel = document.getElementById('detail-panel');
        if (panel) panel.classList.remove('detail-panel--open');
    }

    static isOpen() {
        return DetailPanel._open;
    }

    static async fetchData(symbolId) {
        const [descRes, sourceRes, neighborRes] = await Promise.all([
            fetch(`/api/describe?id=${symbolId}`),
            fetch(`/api/source?id=${symbolId}&context=0`),
            fetch(`/api/graph/neighborhood?id=${symbolId}&depth=1`),
        ]);

        const describe = descRes.ok ? await descRes.json() : null;
        const source = sourceRes.ok ? await sourceRes.json() : null;
        const neighborhood = neighborRes.ok ? await neighborRes.json() : null;

        let callers = [];
        let callees = [];
        if (neighborhood) {
            const selfId = String(symbolId);
            for (const edge of (neighborhood.edges || [])) {
                if (String(edge.target) === selfId) {
                    const node = (neighborhood.nodes || []).find(n => String(n.id) === String(edge.source));
                    if (node) callers.push(node);
                } else if (String(edge.source) === selfId) {
                    const node = (neighborhood.nodes || []).find(n => String(n.id) === String(edge.target));
                    if (node) callees.push(node);
                }
            }
        }

        const selfNode = neighborhood
            ? (neighborhood.nodes || []).find(n => String(n.id) === String(symbolId))
            : null;

        return { describe, source, callers, callees, selfNode };
    }

    static renderHeader(headerEl, selfNode, describe) {
        const name = selfNode ? selfNode.name : (describe ? describe.role : 'Symbol');
        const file = selfNode ? selfNode.file : '';
        headerEl.innerHTML = `
            <div>
                <div class="detail-panel__name">${esc(name)}</div>
                <div class="detail-panel__file">${esc(file)}</div>
            </div>
            <button class="detail-panel__close" onclick="DetailPanel.close()">&times;</button>
        `;
    }

    static renderDescription(describe) {
        if (!describe) return '';
        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Description</div>
            <div class="detail-panel__description">${esc(describe.description)}</div>
        </div>`;
    }

    static renderSource(source) {
        if (!source || !source.code) return '';

        const lines = source.code.split('\n');
        const lineCount = source.line_count || lines.length;
        const showLines = lineCount < 25 ? lines : lines.slice(0, 15);
        const startLine = source.line_start || 1;

        let codeHtml = '';
        for (let i = 0; i < showLines.length; i++) {
            const lineNum = startLine + i;
            const highlighted = DetailPanel.highlightSyntax(showLines[i], source.language || '');
            codeHtml += `<div><span class="detail-panel__code-line-num">${esc(String(lineNum))}</span>${highlighted}</div>`;
        }

        let viewMore = '';
        if (lineCount >= 25) {
            viewMore = `<button class="detail-panel__view-source" onclick="SourceModal.open(DetailPanel._lastSource)">View full source (${esc(String(lineCount))} lines)</button>`;
        }

        DetailPanel._lastSource = source;

        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Source</div>
            <div class="detail-panel__code">${codeHtml}</div>
            ${viewMore}
        </div>`;
    }

    static renderCallers(callers) {
        if (!callers || callers.length === 0) return '';
        let callerHtml = '';
        for (const c of callers.slice(0, 10)) {
            callerHtml += `<li class="detail-panel__symbol-item" onclick="DetailPanel.open(${esc(String(c.id))})">${esc(c.name)} <span style="color:var(--text-muted)">${esc(c.kind)}</span></li>`;
        }
        if (callers.length > 10) {
            callerHtml += `<li class="detail-panel__symbol-item" style="color:var(--text-muted)">... and ${esc(String(callers.length - 10))} more</li>`;
        }
        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Called By (${esc(String(callers.length))})</div>
            <ul class="detail-panel__symbol-list">${callerHtml}</ul>
        </div>`;
    }

    static renderCallees(callees) {
        if (!callees || callees.length === 0) return '';
        let calleeHtml = '';
        for (const c of callees.slice(0, 10)) {
            calleeHtml += `<li class="detail-panel__symbol-item" onclick="DetailPanel.open(${esc(String(c.id))})">${esc(c.name)} <span style="color:var(--text-muted)">${esc(c.kind)}</span></li>`;
        }
        if (callees.length > 10) {
            calleeHtml += `<li class="detail-panel__symbol-item" style="color:var(--text-muted)">... and ${esc(String(callees.length - 10))} more</li>`;
        }
        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Depends On (${esc(String(callees.length))})</div>
            <ul class="detail-panel__symbol-list">${calleeHtml}</ul>
        </div>`;
    }

    static renderRiskFactors(metrics) {
        if (!metrics) return '';
        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Risk Factors</div>
            ${DetailPanel._renderRiskBar('Fan In', metrics.fan_in, 20)}
            ${DetailPanel._renderRiskBar('Fan Out', metrics.fan_out, 20)}
            ${DetailPanel._renderRiskBar('Churn', metrics.modification_count, 30)}
            ${DetailPanel._renderRiskBar('Coupling', Math.round(metrics.max_coupling_strength * 100), 100)}
        </div>`;
    }

    static renderBlastRadius(metrics) {
        if (!metrics || metrics.blast_radius === 0) return '';
        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Blast Radius</div>
            <div style="font-size:13px;color:var(--text-muted)">Changing this symbol could affect <strong>${esc(String(metrics.blast_radius))}</strong> downstream symbols.</div>
        </div>`;
    }

    static renderIssues(selfNode, describe) {
        if (!describe) return '';
        const riskLevel = describe.risk_level;
        const riskScore = describe.risk_score;
        return `<div class="detail-panel__section">
            <div class="detail-panel__section-title">Assessment</div>
            <div style="display:flex;align-items:center;gap:8px;">
                <span class="risk-card__badge risk-card__badge--${esc(riskLevel)}">${esc(riskLevel)}</span>
                <span style="font-size:13px;color:var(--text-muted)">Risk score: ${esc(String(Math.round(riskScore * 100)))}%</span>
            </div>
        </div>`;
    }

    static highlightSyntax(line, language) {
        // Call esc() first to prevent XSS, then apply highlighting spans
        let result = esc(line);

        // Comments (run first to avoid re-highlighting inside comments)
        result = result.replace(/(\/\/.*$)/gm, '<span class="syn-comment">$1</span>');

        // Strings (esc() converts " to &quot; and ' to &#39;)
        result = result.replace(/(&quot;[^&]*?&quot;)/g, '<span class="syn-string">$1</span>');
        result = result.replace(/(&#39;[^&]*?&#39;)/g, '<span class="syn-string">$1</span>');

        // Numbers
        result = result.replace(/\b(\d+\.?\d*)\b/g, '<span class="syn-number">$1</span>');

        // Language keywords
        const rustKeywords = /\b(fn|let|mut|const|pub|use|mod|struct|enum|impl|trait|where|self|Self|return|if|else|match|for|while|loop|break|continue|async|await|move|dyn|Box|Vec|Option|Result|Some|None|Ok|Err|true|false)\b/g;
        const jsKeywords = /\b(function|const|let|var|return|if|else|for|while|class|static|async|await|new|this|import|export|default|try|catch|throw|typeof|instanceof|true|false|null|undefined)\b/g;
        const pyKeywords = /\b(def|class|return|if|elif|else|for|while|import|from|as|with|try|except|raise|True|False|None|self|lambda|yield|async|await|pass|break|continue)\b/g;

        const lang = (language || '').toLowerCase();
        let keywords = rustKeywords;
        if (lang === 'javascript' || lang === 'typescript' || lang === 'js' || lang === 'ts') {
            keywords = jsKeywords;
        } else if (lang === 'python' || lang === 'py') {
            keywords = pyKeywords;
        }

        result = result.replace(keywords, '<span class="syn-keyword">$1</span>');

        return result;
    }

    static _renderRiskBar(label, value, maxVal) {
        const pct = Math.min(Math.round((value / maxVal) * 100), 100);
        const color = pct >= 80 ? 'var(--health-red)' : pct >= 50 ? 'var(--health-orange)' : pct >= 25 ? 'var(--health-yellow)' : 'var(--health-green)';
        return `<div class="detail-panel__risk-bar">
            <span class="detail-panel__risk-bar-label">${esc(label)}</span>
            <div class="detail-panel__risk-bar-track">
                <div class="detail-panel__risk-bar-fill" style="width:${esc(String(pct))}%;background:${esc(color)}"></div>
            </div>
            <span class="detail-panel__risk-bar-value">${esc(String(value))}</span>
        </div>`;
    }
}
```

**Verify:** `ls /Users/rembrandt/loremllc/ariadne/src/dashboard/static/detail-panel.js`
**Expected:** File exists

### Step 2: Create source-modal.js

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/source-modal.js`
**Location:** Create new file (does not exist yet)

```javascript
// Ariadne Dashboard v2 -- Source Modal
'use strict';

class SourceModal {
    static _open = false;

    static open(sourceData) {
        if (!sourceData) return;

        SourceModal._open = true;

        const modal = document.getElementById('source-modal');
        const header = document.getElementById('source-modal-header');
        const codeEl = document.getElementById('source-modal-code');
        if (!modal || !header || !codeEl) return;

        SourceModal.render(sourceData);

        modal.classList.remove('hidden');
    }

    static render(sourceData) {
        const header = document.getElementById('source-modal-header');
        const codeEl = document.getElementById('source-modal-code');
        if (!header || !codeEl) return;

        const lineRange = sourceData.line_start && sourceData.line_end
            ? ` (L${esc(String(sourceData.line_start))}-${esc(String(sourceData.line_end))})`
            : '';
        header.innerHTML = `
            <span class="source-modal__path">${esc(sourceData.file || '')}${lineRange}</span>
            <button class="source-modal__close" onclick="SourceModal.close()">&times;</button>
        `;

        const lines = (sourceData.code || '').split('\n');
        const startLine = sourceData.line_start || 1;
        const language = sourceData.language || '';

        let codeHtml = '';
        for (let i = 0; i < lines.length; i++) {
            const lineNum = startLine + i;
            const highlighted = DetailPanel.highlightSyntax(lines[i], language);
            codeHtml += `<div><span class="detail-panel__code-line-num">${esc(String(lineNum))}</span>${highlighted}</div>`;
        }

        codeEl.innerHTML = codeHtml;
    }

    static close() {
        SourceModal._open = false;
        const modal = document.getElementById('source-modal');
        if (modal) modal.classList.add('hidden');
    }

    static isOpen() {
        return SourceModal._open;
    }
}
```

**Verify:** `ls /Users/rembrandt/loremllc/ariadne/src/dashboard/static/source-modal.js`
**Expected:** File exists

### Step 3: Verify all innerHTML uses esc()

Audit both files. Every `${...}` inside an `innerHTML` assignment must use `esc()` unless the value was already produced by `highlightSyntax()` (which calls `esc(line)` internally before adding span tags).

Checklist for `detail-panel.js`:
- `${esc(name)}`, `${esc(file)}` in `renderHeader` — OK
- `${esc(describe.description)}` in `renderDescription` — OK
- `${esc(String(lineNum))}` in `renderSource` — OK
- `${highlighted}` in `renderSource` — safe: `highlightSyntax` calls `esc(line)` first
- `${esc(String(lineCount))}` in `renderSource` — OK
- `${esc(String(c.id))}`, `${esc(c.name)}`, `${esc(c.kind)}` in `renderCallers`/`renderCallees` — OK
- `${esc(String(callers.length))}`, `${esc(String(callees.length))}` — OK
- `${esc(String(callers.length - 10))}`, `${esc(String(callees.length - 10))}` — OK
- `${esc(label)}`, `${esc(String(pct))}`, `${esc(color)}`, `${esc(String(value))}` in `_renderRiskBar` — OK
- `${esc(String(metrics.blast_radius))}` in `renderBlastRadius` — OK
- `${esc(riskLevel)}`, `${esc(String(Math.round(riskScore * 100)))}` in `renderIssues` — OK

Checklist for `source-modal.js`:
- `${esc(sourceData.file || '')}` in `render` — OK
- `${esc(String(sourceData.line_start))}`, `${esc(String(sourceData.line_end))}` in `render` — OK
- `${esc(String(lineNum))}` in `render` — OK
- `${highlighted}` in `render` — safe: delegates to `DetailPanel.highlightSyntax` which calls `esc(line)` first

**Verify:** Visual inspection of both files
**Expected:** All template interpolations in innerHTML use esc() or are output of highlightSyntax()

### Step 4: Add v2 integration test to test_dashboard.rs

**File:** `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs`
**Location:** Update the existing import block at the top of the file, then add test at the end of the file

**Prerequisite check:** Before editing, confirm that `src/dashboard/api.rs` exports `describe`, `DescribeQuery`, and `SourceQuery` with `context: Option<u32>` field (added by PRD-01). If PRD-01 is not yet executed, skip this step.

Replace the existing import block:
```rust
use ariadne::dashboard::api::{
    coupling, graph_data, modules, search_symbols, stats, CouplingQuery, DbState, SearchQuery,
};
```

With:
```rust
use ariadne::dashboard::api::{
    coupling, describe, graph_data, modules, search_symbols, source, stats, CouplingQuery,
    DbState, DescribeQuery, SearchQuery, SourceQuery,
};
```

Then add this test at the end of the file (after the last closing `}`):

```rust
#[tokio::test]
async fn test_dashboard_v2_all_endpoints() {
    let (_dir, state) = setup_indexed_db();

    // Stats
    let stats_result = stats(State(state.clone())).await;
    assert!(stats_result.is_ok(), "stats endpoint failed");

    // Modules
    let modules_result = modules(State(state.clone())).await;
    assert!(modules_result.is_ok(), "modules endpoint failed");

    // Insights
    let insights_result = ariadne::dashboard::api::insights(State(state.clone())).await;
    assert!(insights_result.is_ok(), "insights endpoint failed");

    // Coupling
    let coupling_query = CouplingQuery { limit: Some(5) };
    let coupling_result = coupling(State(state.clone()), Query(coupling_query)).await;
    assert!(coupling_result.is_ok(), "coupling endpoint failed");

    // Search -> Describe -> Source chain
    let search_query = SearchQuery {
        q: Some("greet".to_string()),
    };
    let search_result = search_symbols(State(state.clone()), Query(search_query))
        .await
        .expect("search should succeed");
    assert!(
        !search_result.0.is_empty(),
        "need at least one symbol for v2 integration test"
    );

    let symbol_id: i64 = search_result.0[0].id.parse().expect("id should be numeric");

    let desc_query = DescribeQuery { id: symbol_id };
    let desc_result = describe(State(state.clone()), Query(desc_query))
        .await
        .expect("describe endpoint failed");
    assert!(
        !desc_result.0.description.is_empty(),
        "description should not be empty"
    );
    assert!(
        desc_result.0.risk_score >= 0.0 && desc_result.0.risk_score <= 1.0,
        "risk_score should be 0-1, got {}",
        desc_result.0.risk_score
    );

    let source_query = SourceQuery {
        id: symbol_id,
        context: Some(0),
    };
    let source_result = source(State(state.clone()), Query(source_query))
        .await
        .expect("source endpoint failed");
    assert!(
        !source_result.0.code.is_empty(),
        "source code should not be empty"
    );
    assert!(
        source_result.0.line_count > 0,
        "line_count should be > 0"
    );
}
```

**Verify:** `cargo test test_dashboard_v2_all_endpoints -- --nocapture`
**Expected:** test result: ok. 1 passed; 0 failed

### Step 5: Run the XSS regression test

**Verify:** `cargo test test_xss_regression_html_escaping -- --nocapture`
**Expected:** test result: ok. 1 passed; 0 failed

### Step 6: Run cargo build

**Verify:** `cargo build`
**Expected:** Compiles without errors or warnings

### Step 7: Run full test suite

**Verify:** `cargo test`
**Expected:** test result: ok. N passed; 0 failed; 0 ignored

### Step 8: Run clippy

**Verify:** `cargo clippy -- -D warnings`
**Expected:** Finished ... without any warning lines

### Step 9: Run format check

**Verify:** `cargo fmt --check`
**Expected:** exits 0 with no output

### Step 10: Verify both JS files exist

**Verify:** `ls /Users/rembrandt/loremllc/ariadne/src/dashboard/static/`
**Expected:** Output contains `detail-panel.js` and `source-modal.js`

### Step 11: Verify detail-panel.js exposes all required methods

**Verify:** `grep -c "static " /Users/rembrandt/loremllc/ariadne/src/dashboard/static/detail-panel.js`
**Expected:** 13 or greater (open, close, isOpen, fetchData, renderHeader, renderDescription, renderSource, renderCallers, renderCallees, renderRiskFactors, renderBlastRadius, renderIssues, highlightSyntax, _renderRiskBar)

### Step 12: Manual end-to-end test

**Verify:** `cargo run -- index . && cargo run -- dash`
**Expected:** Walk through the complete user flow in a browser at http://127.0.0.1:1337:

1. Signal view loads with health score, risks, modules list, coupling list, dead code count
2. Type "greet" in the search bar: dropdown appears with at least one result
3. Click the first search result: Void view loads and detail panel slides in from the right
4. Detail panel shows: description text, source code lines with line numbers, risk factor bars
5. Click a caller or callee item in the panel: panel header and body update to show the new symbol
6. If the symbol has 25 or more source lines: a "View full source" button is present
7. Click "View full source": source modal opens full-screen with all lines and line numbers
8. Press Escape: source modal closes (modal hidden class applied)
9. Press Escape again: detail panel closes (detail-panel--open class removed)
10. Click the "Signal" back button: returns to Signal view

## Acceptance Criteria

- [ ] `ls /Users/rembrandt/loremllc/ariadne/src/dashboard/static/detail-panel.js` exits 0
- [ ] `ls /Users/rembrandt/loremllc/ariadne/src/dashboard/static/source-modal.js` exits 0
- [ ] `cargo test test_xss_regression_html_escaping -- --nocapture` exits 0
- [ ] `cargo test test_dashboard_v2_all_endpoints -- --nocapture` exits 0
- [ ] `cargo test` exits 0 with 0 failed
- [ ] `cargo clippy -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] Detail panel shows description, source lines with line numbers, risk factor bars, callers, callees
- [ ] Clicking a caller or callee inside the panel re-opens the panel with the clicked symbol
- [ ] Source modal opens for symbols with line_count >= 25 via "View full source" button
- [ ] Source modal closes by calling SourceModal.close()
- [ ] All innerHTML template interpolations use esc() or use highlightSyntax() output

## Types and Signatures

```javascript
// /Users/rembrandt/loremllc/ariadne/src/dashboard/static/detail-panel.js
class DetailPanel {
    // static fields
    static _open: boolean
    static _currentSymbolId: number | null
    static _lastSource: object | null   // stores last SourceResult for SourceModal

    // public API
    static async open(symbolId: number): Promise<void>
        // Sets _open=true, adds detail-panel--open class, calls fetchData then renderHeader+all render methods
    static close(): void
        // Sets _open=false, removes detail-panel--open class from #detail-panel
    static isOpen(): boolean
        // Returns DetailPanel._open
    static async fetchData(symbolId: number): Promise<{ describe: object|null, source: object|null, callers: Array, callees: Array, selfNode: object|null }>
        // Parallel fetch of /api/describe, /api/source, /api/graph/neighborhood; returns parsed JSON or null per endpoint
    static renderHeader(headerEl: HTMLElement, selfNode: object|null, describe: object|null): void
        // Sets headerEl.innerHTML with symbol name, file path, and close button
    static renderDescription(describe: object|null): string
        // Returns HTML string for description section or '' if describe is null
    static renderSource(source: object|null): string
        // Returns HTML string for source code section; shows first 15 lines if line_count>=25, adds "View full source" button
    static renderCallers(callers: Array<{ id: string, name: string, kind: string }>): string
        // Returns HTML string for "Called By" section or '' if callers is empty
    static renderCallees(callees: Array<{ id: string, name: string, kind: string }>): string
        // Returns HTML string for "Depends On" section or '' if callees is empty
    static renderRiskFactors(metrics: object|null): string
        // Returns HTML string for risk factor bars (fan_in, fan_out, modification_count, max_coupling_strength) or '' if metrics is null
    static renderBlastRadius(metrics: object|null): string
        // Returns HTML string for blast radius section or '' if metrics is null or metrics.blast_radius is 0
    static renderIssues(selfNode: object|null, describe: object|null): string
        // Returns HTML string for assessment section showing risk_level badge and risk_score percentage, or '' if describe is null
    static highlightSyntax(line: string, language: string): string
        // Calls esc(line) then applies regex spans for keywords/strings/comments/numbers; handles rust, javascript/typescript, python
    static _renderRiskBar(label: string, value: number, maxVal: number): string
        // Returns HTML string for a labeled progress bar; color thresholds: >=80% red, >=50% orange, >=25% yellow, else green
}

// /Users/rembrandt/loremllc/ariadne/src/dashboard/static/source-modal.js
class SourceModal {
    // static fields
    static _open: boolean

    // public API
    static open(sourceData: { code: string, line_start: number, line_end: number, line_count: number, language: string, file: string }): void
        // Sets _open=true, calls render(sourceData), removes 'hidden' class from #source-modal
    static render(sourceData: { code: string, line_start: number, line_end: number, language: string, file: string }): void
        // Populates #source-modal-header with file path + line range + close button; populates #source-modal-code with all lines via DetailPanel.highlightSyntax
    static close(): void
        // Sets _open=false, adds 'hidden' class to #source-modal
    static isOpen(): boolean
        // Returns SourceModal._open
}
```

```rust
// In tests/test_dashboard.rs (updated import, requires PRD-01 complete)
use ariadne::dashboard::api::{
    coupling, describe, graph_data, modules, search_symbols, source, stats, CouplingQuery,
    DbState, DescribeQuery, SearchQuery, SourceQuery,
};

// New test function
async fn test_dashboard_v2_all_endpoints()  // #[tokio::test], no parameters, returns ()
```

## Imports

```javascript
// detail-panel.js: no imports -- vanilla JS
// Requires: esc() defined in index.html (global scope)
// Requires: SourceModal class loaded after detail-panel.js (for "View full source" button onclick)

// source-modal.js: no imports -- vanilla JS
// Requires: esc() defined in index.html (global scope)
// Requires: DetailPanel class loaded before source-modal.js (for DetailPanel.highlightSyntax call in render())
```

```rust
// tests/test_dashboard.rs: axum::extract::{Query, State} already imported at top of file
```

## Completion Contract

**Tests that must pass before signaling done:**
- `cargo test test_xss_regression_html_escaping -- --nocapture` -> exit 0
- `cargo test test_dashboard_v2_all_endpoints -- --nocapture` -> exit 0
- `cargo test` -> exit 0
- `cargo clippy -- -D warnings` -> exit 0
- `cargo fmt --check` -> exit 0

**Files this mini PRD is permitted to touch:**
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/detail-panel.js` (CREATE)
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/source-modal.js` (CREATE)
- `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs` (MODIFY: update import block, add test_dashboard_v2_all_endpoints)

**Completion signal:**
PLANFORGE_COMPLETE: PRD-05 Detail panel, source modal, and full v2 integration
