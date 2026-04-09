# Mini PRD 03: Search Module and Signal View

> **Dependencies:**
> - PRD-01 (backend APIs must be deployed: `/api/stats`, `/api/modules`, `/api/graph/insights`, `/api/coupling`, `/api/search`, `/api/describe`)
> - PRD-02 (HTML shell, CSS, static file serving, and stub JS files must exist)
> **Produces:** Functional `search.js` with symbol search and dropdown, functional `signal.js` with intelligence report sections
> **Estimated steps:** 8

## Context

This PRD implements the two primary JavaScript modules for the Signal (landing page) view. The search module provides global symbol search with debounced input (200ms), keyboard navigation, and drill-down triggering. The Signal module fetches data from all backend APIs and renders the intelligence report: health score hero, risk cards, module grid, coupling pairs, and dead code. Both modules depend on the HTML shell from PRD-02 being in place.

### External Interface: esc()

The global `esc()` function is defined in `index.html` by PRD-02 and is available to all loaded scripts:

```javascript
function esc(s) {
    if (s == null) return '';
    return String(s)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}
```

Every `${...}` inside an `innerHTML` assignment must be wrapped with `esc()`.

### External Interface: App.drillDown

`App.drillDown` is a static async method defined in `index.html` by PRD-02:

```javascript
static async drillDown(moduleName, symbolId)
```

- `moduleName` (string): The top-level source module name extracted from the file path, e.g. `"dashboard"`. Pass the first path segment after stripping the leading `src/` prefix.
- `symbolId` (number | undefined): The integer ID of the symbol to focus. Pass `undefined` (or omit) when drilling to a module without a specific symbol focus.

`App` is a class defined in `index.html` inline `<script>` and is available as a global when these scripts execute.

### Health Score Formula

```
score = resolution_rate * 0.30
      + (1 - dead_ratio)  * 0.25
      + (1 - cycle_score) * 0.20
      + (1 - god_score)   * 0.15
      + coupling_health   * 0.10
```

Where:
- `resolution_rate` = `stats.resolution_rate` (0–1 float from `/api/stats`)
- `dead_ratio` = `stats.dead_functions / stats.symbols` (clamp denominator to 1 if 0)
- `cycle_score` = `min(circular_deps.length * 0.05, 1.0)` — each circular dep adds 5 penalty points out of 100; capped at 1.0
- `god_score` = `min(god_files.length * 0.10, 1.0)` — each god file adds 10 penalty points; capped at 1.0
- `coupling_health` = `0.8` (fixed constant; no per-file coupling health is available from `/api/coupling`)

Final score is multiplied by 100 and rounded to nearest integer, then clamped to [0, 100].

## Files

| Action | Path | Purpose |
|--------|------|---------|
| MODIFY | `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/search.js` | Full search implementation — replaces the empty stub created by PRD-02 |
| MODIFY | `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/signal.js` | Full Signal view implementation — replaces the empty stub created by PRD-02 |

## Steps

### Step 1: Implement search.js

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/search.js`
**Location:** Replace entire file

```javascript
// Ariadne Dashboard v2 -- Search
'use strict';

class Search {
    static _debounceTimer = null;
    static _results = [];
    static _selectedIndex = -1;
    static _dropdownEl = null;
    static _isOpen = false;

    static init() {
        const input = document.getElementById('search-input');
        if (!input) return;

        input.addEventListener('input', () => {
            clearTimeout(Search._debounceTimer);
            const term = input.value.trim();
            if (term.length < 2) {
                Search.close();
                return;
            }
            Search._debounceTimer = setTimeout(() => Search.query(term), 200);
        });

        input.addEventListener('keydown', (e) => {
            if (!Search._isOpen) return;
            if (e.key === 'ArrowDown') {
                e.preventDefault();
                Search._selectedIndex = Math.min(Search._selectedIndex + 1, Search._results.length - 1);
                Search._highlightSelected();
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                Search._selectedIndex = Math.max(Search._selectedIndex - 1, 0);
                Search._highlightSelected();
            } else if (e.key === 'Enter' && Search._selectedIndex >= 0) {
                e.preventDefault();
                const r = Search._results[Search._selectedIndex];
                if (r) Search.selectResult(r);
            } else if (e.key === 'Escape') {
                Search.close();
            }
        });

        input.addEventListener('focus', () => {
            if (input.value.trim().length >= 2) {
                Search.query(input.value.trim());
            }
        });
    }

    static focus() {
        const input = document.getElementById('search-input');
        if (input) input.focus();
    }

    static isOpen() {
        return Search._isOpen;
    }

    static close() {
        Search._isOpen = false;
        Search._results = [];
        Search._selectedIndex = -1;
        if (Search._dropdownEl) {
            Search._dropdownEl.remove();
            Search._dropdownEl = null;
        }
    }

    static async query(term) {
        try {
            const res = await fetch(`/api/search?q=${encodeURIComponent(term)}`);
            if (!res.ok) return;
            const results = await res.json();
            Search._results = results.slice(0, 10);
            Search._selectedIndex = -1;
            Search.renderResults(Search._results);
        } catch (e) {
            console.error('Search error:', e);
        }
    }

    static renderResults(results) {
        Search.close();
        if (results.length === 0) return;

        Search._isOpen = true;
        Search._results = results;

        const container = document.getElementById('search-container');
        const dropdown = document.createElement('div');
        dropdown.className = 'search-dropdown';

        let html = '';
        for (let i = 0; i < results.length; i++) {
            const r = results[i];
            const fileName = r.file ? r.file.split('/').pop() : '';
            html += `<div class="search-result" data-index="${esc(String(i))}" onclick="Search._selectByIndex(${i})">
                <span class="search-result__name">${esc(r.name)}</span>
                <span class="search-result__kind">${esc(r.kind)}</span>
                <span class="search-result__file">${esc(fileName)}</span>
            </div>`;
        }
        html += `<div class="search-hint">
            <span><kbd>&#8593;&#8595;</kbd> navigate</span>
            <span><kbd>Enter</kbd> select</span>
            <span><kbd>Esc</kbd> close</span>
        </div>`;

        dropdown.innerHTML = html;
        container.appendChild(dropdown);
        Search._dropdownEl = dropdown;
    }

    static _highlightSelected() {
        if (!Search._dropdownEl) return;
        const items = Search._dropdownEl.querySelectorAll('.search-result');
        items.forEach((el, i) => {
            el.classList.toggle('search-result--selected', i === Search._selectedIndex);
        });
    }

    static _selectByIndex(index) {
        const r = Search._results[index];
        if (r) Search.selectResult(r);
    }

    // result: object with shape { id: number|string, name: string, kind: string, file: string }
    // Extracts moduleName from result.file and calls App.drillDown(moduleName, symbolId).
    static selectResult(result) {
        Search.close();
        document.getElementById('search-input').value = '';

        // Extract module name from file path
        const filePath = result.file || '';
        const pathWithoutSrc = filePath.startsWith('src/') ? filePath.slice(4) : filePath;
        const moduleName = pathWithoutSrc.includes('/') ? pathWithoutSrc.split('/')[0] : 'root';

        const symbolId = parseInt(result.id, 10);
        if (typeof App !== 'undefined' && App.drillDown) {
            App.drillDown(moduleName, symbolId);
        }
    }
}
```

**Verify:** `cargo build`
**Expected:** Compiles without errors

### Step 2: Implement signal.js

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/signal.js`
**Location:** Replace entire file

```javascript
// Ariadne Dashboard v2 -- Signal View
'use strict';

class Signal {
    static _scrollY = 0;
    static _data = null;

    static async init() {
        try {
            Signal._data = await Signal.fetchData();
            Signal.renderHero(Signal._data.stats, Signal._data.insights);
            Signal.renderTopStats(Signal._data.stats);
            Signal.renderRisks(Signal._data.insights, Signal._data.modules);
            Signal.renderModules(Signal._data.modules);
            Signal.renderCoupling(Signal._data.coupling);
            Signal.renderDeadCode(Signal._data.insights);
        } catch (e) {
            console.error('Signal init error:', e);
        }
    }

    static async fetchData() {
        const [statsRes, modulesRes, insightsRes, couplingRes] = await Promise.all([
            fetch('/api/stats'),
            fetch('/api/modules'),
            fetch('/api/graph/insights'),
            fetch('/api/coupling?limit=10'),
        ]);

        const stats = await statsRes.json();
        const modulesData = await modulesRes.json();
        const insights = await insightsRes.json();
        const couplingData = await couplingRes.json();

        return {
            stats,
            modules: modulesData.modules || [],
            insights,
            coupling: couplingData.pairs || [],
        };
    }

    // Computes health score 0-100 using the weighted formula:
    //   resolution_rate(30%) + (1-dead_ratio)(25%) + (1-cycle_score)(20%) + (1-god_score)(15%) + coupling_health(10%)
    // coupling_health is fixed at 0.8 (no per-file coupling health available from /api/coupling).
    static computeHealthScore(stats, insights) {
        const resolutionRate = stats.resolution_rate || 0;
        const deadRatio = stats.symbols > 0 ? (stats.dead_functions || 0) / stats.symbols : 0;
        const cycleScore = Math.min((insights.circular_deps || []).length * 0.05, 1.0);
        const godScore = Math.min((insights.god_files || []).length * 0.10, 1.0);
        const couplingHealth = 0.8;

        const raw =
            resolutionRate    * 0.30 +
            (1 - deadRatio)   * 0.25 +
            (1 - cycleScore)  * 0.20 +
            (1 - godScore)    * 0.15 +
            couplingHealth    * 0.10;

        return Math.max(0, Math.min(100, Math.round(raw * 100)));
    }

    static _healthColor(score) {
        if (score >= 80) return 'var(--health-green)';
        if (score >= 60) return 'var(--health-yellow)';
        if (score >= 40) return 'var(--health-orange)';
        return 'var(--health-red)';
    }

    static _healthLabel(score) {
        if (score >= 80) return 'Healthy';
        if (score >= 60) return 'Moderate';
        if (score >= 40) return 'At Risk';
        return 'Critical';
    }

    // stats: object from /api/stats — fields: symbols(number), files(number), calls(number), dead_functions(number), resolution_rate(float 0-1), languages(string[])
    // insights: object from /api/graph/insights — fields: circular_deps(array), god_files(array), most_connected(array), dead_code(array)
    static renderHero(stats, insights) {
        const score = Signal.computeHealthScore(stats, insights);
        const color = Signal._healthColor(score);
        const label = Signal._healthLabel(score);

        const el = document.getElementById('signal-hero');
        if (!el) return;

        el.innerHTML = `
            <div class="signal-hero__score" style="color: ${esc(color)}">${esc(String(score))}</div>
            <div class="signal-hero__label">${esc(label)}</div>
            <div class="signal-hero__summary">
                ${esc(String(stats.symbols || 0))} symbols across ${esc(String(stats.files || 0))} files in ${esc(String((stats.languages || []).length))} languages.
                ${esc(String(stats.dead_functions || 0))} unreachable symbols detected.
            </div>
            <div class="signal-hero__stats">
                <div><span class="signal-hero__stat-value">${esc(String(stats.files || 0))}</span> files</div>
                <div><span class="signal-hero__stat-value">${esc(String(stats.symbols || 0))}</span> symbols</div>
                <div><span class="signal-hero__stat-value">${esc(String(stats.calls || 0))}</span> calls</div>
                <div><span class="signal-hero__stat-value">${esc(String(Math.round((stats.resolution_rate || 0) * 100)))}%</span> resolved</div>
            </div>
        `;
    }

    // stats: object from /api/stats — fields: files(number), symbols(number), languages(string[])
    static renderTopStats(stats) {
        const el = document.getElementById('top-stats');
        if (!el) return;
        el.innerHTML = `
            <span><span class="top-bar__stat-value">${esc(String(stats.files || 0))}</span> files</span>
            <span><span class="top-bar__stat-value">${esc(String(stats.symbols || 0))}</span> symbols</span>
            <span><span class="top-bar__stat-value">${esc(String((stats.languages || []).length))}</span> langs</span>
        `;
    }

    // insights: object from /api/graph/insights — uses insights.most_connected (top 5 by connection count) as risk candidates
    // modules: array from /api/modules (unused in this method, reserved for future use)
    // Fetches narrative descriptions for each candidate from /api/describe?id=<id> (defined in PRD-01).
    static async renderRisks(insights, modules) {
        const container = document.getElementById('risk-cards');
        if (!container) return;

        const candidates = (insights.most_connected || []).slice(0, 5);
        if (candidates.length === 0) {
            container.innerHTML = '<div style="color: var(--text-muted); font-size: 13px;">No significant risks detected.</div>';
            return;
        }

        const descPromises = candidates.map(async (c) => {
            try {
                const res = await fetch(`/api/describe?id=${encodeURIComponent(c.id)}`);
                if (res.ok) return await res.json();
            } catch (_) {}
            return null;
        });

        const descriptions = await Promise.all(descPromises);

        let html = '';
        for (let i = 0; i < candidates.length; i++) {
            const c = candidates[i];
            const desc = descriptions[i];
            const riskLevel = desc ? desc.risk_level : 'low';
            const description = desc
                ? esc(desc.description)
                : `${esc(c.name)} has ${esc(String(c.connections))} connections.`;

            const filePath = c.file || '';
            const pathWithoutSrc = filePath.startsWith('src/') ? filePath.slice(4) : filePath;
            const moduleName = pathWithoutSrc.includes('/') ? pathWithoutSrc.split('/')[0] : 'root';

            html += `<div class="risk-card" onclick="App.drillDown('${esc(moduleName)}', ${esc(String(c.id))})">
                <div class="risk-card__header">
                    <span class="risk-card__name">${esc(c.name)}</span>
                    <span class="risk-card__badge risk-card__badge--${esc(riskLevel)}">${esc(riskLevel)}</span>
                </div>
                <div class="risk-card__description">${description}</div>
            </div>`;
        }

        container.innerHTML = html;
    }

    // modules: array of module objects from /api/modules — each has fields: name(string), symbol_count(number), file_count(number), dead_count(number), health(float 0-1), files(array of {health: float})
    static renderModules(modules) {
        const container = document.getElementById('module-grid');
        if (!container) return;

        if (!modules || modules.length === 0) {
            container.innerHTML = '<div style="color: var(--text-muted); font-size: 13px;">No modules found. Run ariadne index first.</div>';
            return;
        }

        let html = '';
        for (const m of modules) {
            const healthPct = Math.round((m.health || 0) * 100);
            const healthColor = Signal._healthColor(healthPct);

            let sparkline = '';
            const files = (m.files || []).slice(0, 20);
            for (const f of files) {
                const h = Math.round((f.health || 0) * 100);
                const barH = Math.max(3, Math.round(h / 100 * 20));
                const color = Signal._healthColor(h);
                sparkline += `<div class="module-card__file-bar" style="height:${esc(String(barH))}px;background:${esc(color)}"></div>`;
            }

            html += `<div class="module-card" onclick="App.drillDown('${esc(m.name)}')">
                <div class="module-card__name">${esc(m.name)}</div>
                <div class="module-card__stats">
                    <span>${esc(String(m.symbol_count))} symbols</span>
                    <span>${esc(String(m.file_count))} files</span>
                    <span>${esc(String(m.dead_count))} dead</span>
                </div>
                <div class="module-card__health-bar">
                    <div class="module-card__health-fill" style="width:${esc(String(healthPct))}%;background:${esc(healthColor)}"></div>
                </div>
                <div class="module-card__files">${sparkline}</div>
            </div>`;
        }

        container.innerHTML = html;
    }

    // coupling: array of pair objects from /api/coupling — each has fields: from_file(string), to_file(string), from_module(string), strength(float 0-1)
    static renderCoupling(coupling) {
        const container = document.getElementById('coupling-list');
        if (!container) return;

        if (!coupling || coupling.length === 0) {
            container.innerHTML = '<div style="color: var(--text-muted); font-size: 13px;">No coupling data available.</div>';
            return;
        }

        let html = '';
        for (const c of coupling) {
            const strengthPct = Math.round((c.strength || 0) * 100);
            const color = c.strength >= 0.7 ? 'var(--health-red)' : c.strength >= 0.4 ? 'var(--health-orange)' : 'var(--health-yellow)';
            const fromFile = (c.from_file || '').split('/').pop() || c.from_file;
            const toFile = (c.to_file || '').split('/').pop() || c.to_file;

            html += `<div class="coupling-row" onclick="App.drillDown('${esc(c.from_module)}')">
                <div class="coupling-row__files">
                    <span>${esc(fromFile)}</span>
                    <span class="coupling-row__arrow">&#8596;</span>
                    <span>${esc(toFile)}</span>
                </div>
                <span class="coupling-row__strength" style="color:${esc(color)}">${esc(String(strengthPct))}%</span>
                <div class="coupling-row__bar">
                    <div class="coupling-row__bar-fill" style="width:${esc(String(strengthPct))}%;background:${esc(color)}"></div>
                </div>
            </div>`;
        }

        container.innerHTML = html;
    }

    // insights: object from /api/graph/insights — uses insights.dead_code (array of {id: number, name: string, file: string})
    static renderDeadCode(insights) {
        const container = document.getElementById('dead-code-grid');
        if (!container) return;

        const deadCode = (insights.dead_code || []).slice(0, 20);
        if (deadCode.length === 0) {
            container.innerHTML = '<div style="color: var(--text-muted); font-size: 13px;">No dead code detected.</div>';
            return;
        }

        let html = '';
        for (const d of deadCode) {
            const fileName = (d.file || '').split('/').pop() || d.file;
            const filePath = d.file || '';
            const pathWithoutSrc = filePath.startsWith('src/') ? filePath.slice(4) : filePath;
            const moduleName = pathWithoutSrc.includes('/') ? pathWithoutSrc.split('/')[0] : 'root';

            html += `<div class="dead-code-item" onclick="App.drillDown('${esc(moduleName)}', ${esc(String(d.id))})">
                <span class="dead-code-item__icon">&#9679;</span>
                <span class="dead-code-item__name">${esc(d.name)}</span>
                <span class="dead-code-item__file">${esc(fileName)}</span>
            </div>`;
        }

        container.innerHTML = html;
    }

    static show() {
        const el = document.getElementById('signal-view');
        if (el) {
            el.classList.remove('hidden', 'fade-out');
            el.classList.add('fade-in');
        }
    }

    static hide() {
        const el = document.getElementById('signal-view');
        if (el) {
            el.classList.add('fade-out');
            setTimeout(() => el.classList.add('hidden'), 250);
        }
    }

    static saveScrollPosition() {
        Signal._scrollY = window.scrollY;
    }

    static restoreScrollPosition() {
        window.scrollTo(0, Signal._scrollY);
    }
}
```

**Verify:** `cargo build`
**Expected:** Compiles without errors

### Step 3: Verify XSS regression test passes

The search.js and signal.js files use `esc()` for all `innerHTML` interpolation. After PRD-02 updates `test_xss_regression_html_escaping`, this test checks all four JS files for proper escaping.

**Verify:** `cargo test test_xss -- --nocapture`
**Expected:** PASS

### Step 4: Verify all innerHTML uses esc()

Manually audit both files. Every `${...}` inside an `innerHTML` assignment must be wrapped with `esc()`.

In `search.js`:
- `${esc(String(i))}` in `data-index` attribute — OK
- `${esc(r.name)}` — OK
- `${esc(r.kind)}` — OK
- `${esc(fileName)}` — OK
- `onclick="Search._selectByIndex(${i})"` — `i` is a loop integer produced by JS, never from user data. Safe.

In `signal.js`:
- All `${esc(String(...))}` and `${esc(...)}` patterns — OK
- The fallback description in `renderRisks` applies `esc()` before assigning — OK
- Inline `style` attribute values using CSS variable names (e.g. `var(--health-green)`) are passed through `esc()` — OK

**Verify:** Visual inspection
**Expected:** All user-data interpolations use esc()

### Step 5: Run cargo build to verify include_str! embedding

**Verify:** `cargo build`
**Expected:** Compiles without errors

### Step 6: Run full test suite

**Verify:** `cargo test`
**Expected:** All tests PASS

### Step 7: Run clippy

**Verify:** `cargo clippy -- -D warnings`
**Expected:** No warnings

### Step 8: Manual smoke test

**Verify:** `cargo run -- index . && cargo run -- dash`
**Expected:**
1. Open http://localhost:1337
2. Signal view loads. Open browser DevTools console and verify: `Signal._data !== null` evaluates to `true`.
3. Verify hero section: `document.getElementById('signal-hero').children.length > 0` evaluates to `true`.
4. Verify module grid: `document.getElementById('module-grid').children.length > 0` evaluates to `true`.
5. Type 3+ characters in search bar. Verify: `Search.isOpen()` evaluates to `true`.
6. Press Escape. Verify: `Search.isOpen()` evaluates to `false`.
7. Click a module card. Verify: `App.currentView` evaluates to `'void'`.

## Acceptance Criteria

- [ ] `search.js` implements debounced search (200ms) with dropdown showing `name`, `kind`, `file` fields
- [ ] `signal.js` fetches data from 4 APIs in parallel and renders all 5 sections: hero, risks, modules, coupling, dead code
- [ ] All `innerHTML` interpolation uses `esc()` — verified by `cargo test test_xss`
- [ ] `cargo build` -> exit 0
- [ ] `cargo test test_xss -- --nocapture` -> PASS
- [ ] `cargo test` -> ALL PASS
- [ ] Manual: `Search.isOpen()` returns `true` after typing 3 characters into search input
- [ ] Manual: `App.currentView === 'void'` after clicking a module card

## Types and Signatures

No Rust types. JavaScript classes and their methods:

```javascript
// search.js
class Search {
    // Attaches input, keydown, and focus event listeners to #search-input.
    static init()

    // Focuses the #search-input element.
    static focus()

    // Returns true if the search dropdown is currently open, false otherwise.
    static isOpen()

    // Closes the dropdown: removes the DOM element, clears _results and _selectedIndex.
    static close()

    // Fetches /api/search?q=<term>, stores top 10 results, calls renderResults().
    // term: non-empty string, at least 2 characters.
    static async query(term)

    // Builds and appends a .search-dropdown div to #search-container.
    // results: array of { id: number|string, name: string, kind: string, file: string }
    static renderResults(results)

    // Private: highlights the item at Search._selectedIndex in the open dropdown.
    static _highlightSelected()

    // Private: looks up results[index] and calls selectResult().
    // index: integer, 0-based index into Search._results.
    static _selectByIndex(index)

    // Closes the dropdown, clears the input, extracts moduleName from result.file,
    // parses result.id as integer, and calls App.drillDown(moduleName, symbolId).
    // result: { id: number|string, name: string, kind: string, file: string }
    static selectResult(result)
}

// signal.js
class Signal {
    // Fetches all data, renders all 5 sections. Called by App.init() in index.html.
    static async init()

    // Parallel fetch from /api/stats, /api/modules, /api/graph/insights, /api/coupling?limit=10.
    // Returns { stats, modules: array, insights, coupling: array }
    static async fetchData()

    // Returns health score 0-100 (integer) using the weighted formula documented in Context.
    // stats: /api/stats response object
    // insights: /api/graph/insights response object
    static computeHealthScore(stats, insights)

    // Private: returns a CSS variable string for the given score (0-100).
    static _healthColor(score)

    // Private: returns 'Healthy' | 'Moderate' | 'At Risk' | 'Critical' for the given score.
    static _healthLabel(score)

    // Renders the hero section into #signal-hero with score, label, summary, and stat grid.
    // stats: /api/stats response; insights: /api/graph/insights response
    static renderHero(stats, insights)

    // Renders top-bar stats (files, symbols, langs) into #top-stats.
    // stats: /api/stats response
    static renderTopStats(stats)

    // Renders up to 5 risk cards from insights.most_connected into #risk-cards.
    // Fetches /api/describe for each candidate to get description and risk_level.
    // insights: /api/graph/insights response; modules: array (reserved, currently unused)
    static async renderRisks(insights, modules)

    // Renders module cards into #module-grid. Each card has name, symbol/file/dead counts,
    // a health bar, and a per-file sparkline. Clicking calls App.drillDown(m.name).
    // modules: array of { name, symbol_count, file_count, dead_count, health, files }
    static renderModules(modules)

    // Renders coupling rows into #coupling-list. Clicking calls App.drillDown(c.from_module).
    // coupling: array of { from_file, to_file, from_module, strength }
    static renderCoupling(coupling)

    // Renders up to 20 dead code items into #dead-code-grid.
    // Clicking calls App.drillDown(moduleName, d.id).
    // insights: /api/graph/insights response (uses insights.dead_code array)
    static renderDeadCode(insights)

    // Shows #signal-view by adding fade-in class and removing hidden/fade-out.
    static show()

    // Hides #signal-view by adding fade-out class; adds hidden after 250ms.
    static hide()

    // Saves window.scrollY into Signal._scrollY.
    static saveScrollPosition()

    // Restores window scroll position to Signal._scrollY.
    static restoreScrollPosition()
}
```

## Imports

No imports needed — these are vanilla JS files served by the Axum server. They reference `esc()` which is defined in `index.html`'s inline script, and `App` which is defined in `index.html`'s App controller script. Both are available as globals when these scripts execute.

## Completion Contract

**Tests that must pass before signaling done:**
- `cargo test test_xss -- --nocapture` -> exit 0
- `cargo test` -> exit 0
- `cargo clippy -- -D warnings` -> exit 0
- `cargo build` -> exit 0

**Files this mini PRD is permitted to touch:**
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/search.js`
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/signal.js`

**Completion signal:**
PLANFORGE_COMPLETE: PRD-03 Search module and Signal view with intelligence report
