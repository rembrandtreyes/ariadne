# Mini PRD 02: Static File Serving, CSS Design System, and HTML Shell

> **Dependency:** none -- can execute independently
> **Produces:** Static file serving routes in Axum, CSS design system, HTML shell with Signal+Void markup, updated XSS regression test
> **Estimated steps:** 10

## Context

This PRD sets up the frontend infrastructure for Dashboard v2. It creates placeholder JS files, adds Axum routes to serve them via `include_str!`, writes the complete CSS design system with design tokens, and rewrites `index.html` as the v2 HTML shell containing both Signal and Void view containers. This is the foundation that all JS module PRDs depend on. The existing `style.css` has minimal graph-only styles that will be replaced. The existing `index.html` is a full graph-only dashboard that will be completely rewritten.

## Files

| Action | Path | Purpose |
|--------|------|---------|
| CREATE | `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/signal.js` | Signal view placeholder |
| CREATE | `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/void-renderer.js` | Void renderer placeholder |
| CREATE | `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/detail-panel.js` | Detail panel placeholder |
| CREATE | `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/search.js` | Search module placeholder |
| CREATE | `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/source-modal.js` | Source modal placeholder |
| MODIFY | `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs` | Add embed constants and static file routes |
| MODIFY | `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/style.css` | Complete CSS design system |
| MODIFY | `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/index.html` | Complete v2 HTML shell rewrite |
| MODIFY | `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs` | Update XSS regression test for new HTML structure |

## Steps

### Step 1: Create placeholder JS files

Create each file with a minimal working placeholder. Each file defines a class with the same method names that later PRDs will implement with full logic.

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/signal.js`

```javascript
// Ariadne Dashboard v2 -- Signal View
'use strict';

class Signal {
    static async init() { console.log('Signal.init placeholder'); }
    static show() {}
    static hide() {}
    static saveScrollPosition() {}
    static restoreScrollPosition() {}
}
```

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/void-renderer.js`

NOTE: This is a SEPARATE file from the existing `graph-renderer.js`. Do NOT modify `graph-renderer.js`.

```javascript
// Ariadne Dashboard v2 -- Void Renderer
'use strict';

class Void {
    static async init() { console.log('Void.init placeholder'); }
    static async show(focusModule, focusSymbol) {}
    static hide() {}
    static setMode(mode) {}
    static resetLayout() {}
}
```

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/detail-panel.js`

```javascript
// Ariadne Dashboard v2 -- Detail Panel
'use strict';

class DetailPanel {
    static async open(symbolId) { console.log('DetailPanel.open placeholder', symbolId); }
    static close() {}
    static isOpen() { return false; }
}
```

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/search.js`

```javascript
// Ariadne Dashboard v2 -- Search
'use strict';

class Search {
    static init() { console.log('Search.init placeholder'); }
    static focus() {}
    static close() {}
    static isOpen() { return false; }
}
```

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/source-modal.js`

```javascript
// Ariadne Dashboard v2 -- Source Modal
'use strict';

class SourceModal {
    static open(sourceData) { console.log('SourceModal.open placeholder'); }
    static close() {}
    static isOpen() { return false; }
}
```

**Verify:** All 5 files exist in `src/dashboard/static/`
**Expected:** Files created

### Step 2: Add embed constants in mod.rs

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs`
**Location:** After the existing constants at the bottom of the file. The file currently ends with:

```rust
const INDEX_HTML: &str = include_str!("static/index.html");
const GRAPH_RENDERER_JS: &str = include_str!("static/graph-renderer.js");
```

Add these constants after those lines:

```rust
const STYLE_CSS: &str = include_str!("static/style.css");
const SIGNAL_JS: &str = include_str!("static/signal.js");
const VOID_RENDERER_JS: &str = include_str!("static/void-renderer.js");
const DETAIL_PANEL_JS: &str = include_str!("static/detail-panel.js");
const SEARCH_JS: &str = include_str!("static/search.js");
const SOURCE_MODAL_JS: &str = include_str!("static/source-modal.js");
```

**Verify:** `cargo check`
**Expected:** Compiles (constants may be unused temporarily)

### Step 3: Add static file route handlers in mod.rs

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs`
**Location:** After the `graph_renderer_js_handler` function

```rust
async fn style_css_handler() -> (
    axum::http::StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        STYLE_CSS,
    )
}

async fn signal_js_handler() -> (
    axum::http::StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        SIGNAL_JS,
    )
}

async fn void_renderer_js_handler() -> (
    axum::http::StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        VOID_RENDERER_JS,
    )
}

async fn detail_panel_js_handler() -> (
    axum::http::StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        DETAIL_PANEL_JS,
    )
}

async fn search_js_handler() -> (
    axum::http::StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        SEARCH_JS,
    )
}

async fn source_modal_js_handler() -> (
    axum::http::StatusCode,
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        SOURCE_MODAL_JS,
    )
}
```

**Verify:** `cargo check`
**Expected:** Compiles

### Step 4: Add routes to the router in mod.rs

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs`
**Location:** In the `serve` function, in the router chain. Add after the existing `.route("/graph-renderer.js", axum::routing::get(graph_renderer_js_handler))` line and BEFORE `.fallback(axum::routing::get(index_handler))`.

```rust
        .route("/style.css", axum::routing::get(style_css_handler))
        .route("/signal.js", axum::routing::get(signal_js_handler))
        .route(
            "/void-renderer.js",
            axum::routing::get(void_renderer_js_handler),
        )
        .route(
            "/detail-panel.js",
            axum::routing::get(detail_panel_js_handler),
        )
        .route("/search.js", axum::routing::get(search_js_handler))
        .route(
            "/source-modal.js",
            axum::routing::get(source_modal_js_handler),
        )
```

IMPORTANT: The existing route for `/graph-renderer.js` already uses `graph_renderer_js_handler`. The new `/void-renderer.js` route uses `void_renderer_js_handler` -- a DIFFERENT handler serving a DIFFERENT file. Do NOT remove or replace the existing graph-renderer route.

**Verify:** `cargo check`
**Expected:** Compiles without errors

### Step 5: Write the complete CSS design system

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/style.css`
**Location:** Replace the entire file content

```css
/* Ariadne Dashboard v2 Styles */

/* ======= DESIGN TOKENS ======= */
:root {
    /* Background hierarchy */
    --bg-void: #06080C;
    --bg-surface: #0F1318;
    --bg-elevated: #161C24;
    --bg-card: #1A2029;
    --bg-hover: #1E2530;

    /* Text hierarchy */
    --text-primary: #E2E4E8;
    --text-secondary: #8A8F9C;
    --text-muted: #4A4F5C;

    /* Accent */
    --accent-primary: #D4A853;
    --accent-glow: rgba(212, 168, 83, 0.15);
    --accent-glow-strong: rgba(212, 168, 83, 0.35);

    /* Health colors */
    --health-green: #4ADE80;
    --health-yellow: #FACC15;
    --health-orange: #FB923C;
    --health-red: #F87171;

    /* Borders */
    --border-subtle: rgba(255, 255, 255, 0.06);
    --border-active: rgba(212, 168, 83, 0.3);

    /* Spacing */
    --space-xs: 4px;
    --space-sm: 8px;
    --space-md: 16px;
    --space-lg: 24px;
    --space-xl: 32px;
    --space-2xl: 48px;

    /* Radii */
    --radius-sm: 6px;
    --radius-md: 10px;
    --radius-lg: 16px;
    --radius-xl: 24px;

    /* Transitions */
    --transition-fast: 150ms ease;
    --transition-normal: 250ms ease;
    --transition-slow: 400ms ease;

    /* Fonts */
    --font-sans: 'Outfit', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    --font-mono: 'JetBrains Mono', 'Fira Code', monospace;
    --font-serif: 'Instrument Serif', Georgia, serif;
}

/* ======= RESET ======= */
*, *::before, *::after {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

html {
    scroll-behavior: smooth;
}

body {
    font-family: var(--font-sans);
    background: var(--bg-void);
    color: var(--text-primary);
    line-height: 1.6;
    -webkit-font-smoothing: antialiased;
    overflow-x: hidden;
}

::-webkit-scrollbar { width: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--border-subtle); border-radius: 3px; }
::-webkit-scrollbar-thumb:hover { background: var(--text-muted); }

/* ======= TYPOGRAPHY ======= */
.mono { font-family: var(--font-mono); }
.serif { font-family: var(--font-serif); }

/* ======= TOP BAR ======= */
.top-bar {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    z-index: 100;
    height: 56px;
    display: flex;
    align-items: center;
    padding: 0 var(--space-lg);
    background: rgba(6, 8, 12, 0.85);
    backdrop-filter: blur(12px);
    border-bottom: 1px solid var(--border-subtle);
}

.top-bar__logo {
    font-family: var(--font-serif);
    font-size: 20px;
    color: var(--accent-primary);
    margin-right: var(--space-xl);
    cursor: pointer;
}

.top-bar__search {
    flex: 1;
    max-width: 400px;
    position: relative;
}

.top-bar__search input {
    width: 100%;
    padding: 8px 12px 8px 32px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 13px;
    outline: none;
    transition: border-color var(--transition-fast);
}

.top-bar__search input:focus {
    border-color: var(--border-active);
}

.top-bar__search input::placeholder {
    color: var(--text-muted);
}

.top-bar__search-icon {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-muted);
    font-size: 14px;
    pointer-events: none;
}

.top-bar__stats {
    margin-left: auto;
    display: flex;
    gap: var(--space-md);
    font-size: 12px;
    color: var(--text-muted);
    font-family: var(--font-mono);
}

.top-bar__stat-value {
    color: var(--text-secondary);
}

/* ======= BACK BUTTON ======= */
.back-btn {
    position: fixed;
    top: 68px;
    left: var(--space-lg);
    z-index: 90;
    padding: 6px 14px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-size: 13px;
    cursor: pointer;
    transition: all var(--transition-fast);
}

.back-btn:hover {
    color: var(--accent-primary);
    border-color: var(--border-active);
}

/* ======= SIGNAL VIEW ======= */
.signal-view {
    padding: 72px var(--space-lg) var(--space-2xl);
    max-width: 1200px;
    margin: 0 auto;
    transition: opacity var(--transition-normal);
}

.signal-hero {
    text-align: center;
    padding: var(--space-2xl) 0;
}

.signal-hero__score {
    font-family: var(--font-serif);
    font-size: 72px;
    font-weight: 300;
    color: var(--text-primary);
    line-height: 1;
}

.signal-hero__label {
    font-size: 14px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 2px;
    margin-top: var(--space-sm);
}

.signal-hero__summary {
    font-size: 16px;
    color: var(--text-secondary);
    margin-top: var(--space-md);
    max-width: 600px;
    margin-left: auto;
    margin-right: auto;
}

.signal-hero__stats {
    display: flex;
    justify-content: center;
    gap: var(--space-xl);
    margin-top: var(--space-lg);
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--text-muted);
}

.signal-hero__stat-value {
    color: var(--accent-primary);
    font-size: 18px;
    display: block;
}

.signal-section {
    margin-top: var(--space-2xl);
}

.signal-section__title {
    font-family: var(--font-serif);
    font-size: 24px;
    color: var(--text-primary);
    margin-bottom: var(--space-md);
    padding-bottom: var(--space-sm);
    border-bottom: 1px solid var(--border-subtle);
}

/* Risk cards */
.risk-cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: var(--space-md);
}

.risk-card {
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    padding: var(--space-md);
    cursor: pointer;
    transition: all var(--transition-fast);
}

.risk-card:hover {
    border-color: var(--border-active);
    background: var(--bg-hover);
}

.risk-card__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--space-sm);
}

.risk-card__name {
    font-family: var(--font-mono);
    font-size: 14px;
    color: var(--accent-primary);
}

.risk-card__badge {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 20px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
}

.risk-card__badge--critical { background: rgba(248, 113, 113, 0.15); color: var(--health-red); }
.risk-card__badge--high { background: rgba(251, 146, 60, 0.15); color: var(--health-orange); }
.risk-card__badge--medium { background: rgba(250, 204, 21, 0.15); color: var(--health-yellow); }
.risk-card__badge--low { background: rgba(74, 222, 128, 0.15); color: var(--health-green); }

.risk-card__description {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.5;
}

/* Module grid */
.module-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: var(--space-md);
}

.module-card {
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    padding: var(--space-md);
    cursor: pointer;
    transition: all var(--transition-fast);
}

.module-card:hover {
    border-color: var(--border-active);
    background: var(--bg-hover);
}

.module-card__name {
    font-family: var(--font-mono);
    font-size: 15px;
    color: var(--text-primary);
    margin-bottom: var(--space-xs);
}

.module-card__stats {
    display: flex;
    gap: var(--space-md);
    font-size: 12px;
    color: var(--text-muted);
    font-family: var(--font-mono);
    margin-bottom: var(--space-sm);
}

.module-card__health-bar {
    height: 4px;
    background: var(--bg-elevated);
    border-radius: 2px;
    overflow: hidden;
}

.module-card__health-fill {
    height: 100%;
    border-radius: 2px;
    transition: width var(--transition-normal);
}

.module-card__files {
    display: flex;
    gap: 2px;
    margin-top: var(--space-sm);
    height: 20px;
    align-items: flex-end;
}

.module-card__file-bar {
    flex: 1;
    min-width: 3px;
    max-width: 12px;
    border-radius: 1px;
    transition: height var(--transition-fast);
}

/* Coupling rows */
.coupling-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-sm);
}

.coupling-row {
    display: flex;
    align-items: center;
    gap: var(--space-md);
    padding: var(--space-sm) var(--space-md);
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all var(--transition-fast);
    font-size: 13px;
}

.coupling-row:hover {
    border-color: var(--border-active);
    background: var(--bg-hover);
}

.coupling-row__files {
    flex: 1;
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    font-family: var(--font-mono);
    color: var(--text-secondary);
    min-width: 0;
}

.coupling-row__arrow {
    color: var(--text-muted);
    flex-shrink: 0;
}

.coupling-row__strength {
    font-family: var(--font-mono);
    font-size: 12px;
    flex-shrink: 0;
}

.coupling-row__bar {
    width: 60px;
    height: 4px;
    background: var(--bg-elevated);
    border-radius: 2px;
    overflow: hidden;
    flex-shrink: 0;
}

.coupling-row__bar-fill {
    height: 100%;
    border-radius: 2px;
}

/* Dead code grid */
.dead-code-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: var(--space-sm);
}

.dead-code-item {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    padding: var(--space-sm) var(--space-md);
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    font-size: 13px;
    cursor: pointer;
    transition: all var(--transition-fast);
}

.dead-code-item:hover {
    border-color: var(--border-active);
    background: var(--bg-hover);
}

.dead-code-item__icon {
    color: var(--health-red);
    font-size: 10px;
    flex-shrink: 0;
}

.dead-code-item__name {
    font-family: var(--font-mono);
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.dead-code-item__file {
    margin-left: auto;
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 120px;
}

/* ======= VOID VIEW ======= */
.void-view {
    position: fixed;
    top: 56px;
    left: 0;
    right: 0;
    bottom: 0;
    background: var(--bg-void);
    overflow: hidden;
    transition: opacity var(--transition-normal);
}

.void-ambient {
    position: absolute;
    inset: 0;
    pointer-events: none;
    overflow: hidden;
}

.void-ambient__orb {
    position: absolute;
    border-radius: 50%;
    filter: blur(80px);
    opacity: 0.15;
}

.void-dot-grid {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background-image: radial-gradient(circle, var(--text-muted) 0.5px, transparent 0.5px);
    background-size: 30px 30px;
    opacity: 0.3;
}

.void-layers {
    position: absolute;
    top: var(--space-lg);
    right: var(--space-lg);
    display: flex;
    flex-direction: column;
    gap: var(--space-xs);
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 1px;
}

.void-connections {
    position: absolute;
    inset: 0;
    pointer-events: none;
}

.void-connections path {
    fill: none;
    stroke-linecap: round;
}

.void-nodes {
    position: absolute;
    inset: 0;
}

.void-node {
    position: absolute;
    background: rgba(22, 28, 36, 0.7);
    backdrop-filter: blur(12px);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    padding: var(--space-md);
    min-width: 160px;
    cursor: grab;
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
    user-select: none;
}

.void-node:active {
    cursor: grabbing;
}

.void-node:hover,
.void-node--selected {
    border-color: var(--border-active);
}

.void-node--selected {
    box-shadow: 0 0 20px var(--accent-glow);
}

.void-node__name {
    font-family: var(--font-mono);
    font-size: 14px;
    color: var(--text-primary);
    margin-bottom: var(--space-xs);
}

.void-node__stats {
    font-size: 11px;
    color: var(--text-muted);
    font-family: var(--font-mono);
}

.void-node__glow {
    position: absolute;
    inset: -2px;
    border-radius: var(--radius-md);
    opacity: 0.4;
    pointer-events: none;
}

.void-node__sparkline {
    display: flex;
    gap: 1px;
    margin-top: var(--space-sm);
    height: 16px;
    align-items: flex-end;
}

.void-node__spark-bar {
    flex: 1;
    min-width: 2px;
    max-width: 8px;
    border-radius: 1px;
}

.void-hud {
    position: absolute;
    bottom: var(--space-lg);
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    gap: var(--space-sm);
    padding: var(--space-sm) var(--space-md);
    background: rgba(22, 28, 36, 0.8);
    backdrop-filter: blur(12px);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
}

.void-hud__btn {
    padding: 6px 14px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
    transition: all var(--transition-fast);
}

.void-hud__btn:hover {
    color: var(--text-secondary);
}

.void-hud__btn--active {
    color: var(--accent-primary);
    border-color: var(--border-active);
    background: var(--accent-glow);
}

/* ======= DETAIL PANEL ======= */
.detail-panel {
    position: fixed;
    top: 56px;
    right: 0;
    bottom: 0;
    width: 420px;
    background: var(--bg-surface);
    border-left: 1px solid var(--border-subtle);
    transform: translateX(100%);
    transition: transform var(--transition-slow);
    overflow-y: auto;
    z-index: 80;
}

.detail-panel--open {
    transform: translateX(0);
}

.detail-panel__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-md) var(--space-lg);
    border-bottom: 1px solid var(--border-subtle);
}

.detail-panel__name {
    font-family: var(--font-mono);
    font-size: 16px;
    color: var(--accent-primary);
}

.detail-panel__file {
    font-size: 12px;
    color: var(--text-muted);
    font-family: var(--font-mono);
}

.detail-panel__close {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 18px;
    cursor: pointer;
    padding: 4px;
}

.detail-panel__close:hover {
    color: var(--text-primary);
}

.detail-panel__section {
    padding: var(--space-md) var(--space-lg);
    border-bottom: 1px solid var(--border-subtle);
}

.detail-panel__section-title {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--text-muted);
    margin-bottom: var(--space-sm);
}

.detail-panel__description {
    font-size: 14px;
    color: var(--text-secondary);
    line-height: 1.6;
}

.detail-panel__code {
    background: var(--bg-elevated);
    border-radius: var(--radius-sm);
    padding: var(--space-md);
    overflow-x: auto;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.6;
    max-height: 300px;
    overflow-y: auto;
}

.detail-panel__code-line-num {
    color: var(--text-muted);
    user-select: none;
    display: inline-block;
    width: 3em;
    text-align: right;
    margin-right: 1em;
}

.detail-panel__view-source {
    display: inline-block;
    margin-top: var(--space-sm);
    padding: 4px 12px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-size: 12px;
    cursor: pointer;
    transition: all var(--transition-fast);
}

.detail-panel__view-source:hover {
    color: var(--accent-primary);
    border-color: var(--border-active);
}

.detail-panel__symbol-list {
    list-style: none;
}

.detail-panel__symbol-item {
    padding: 4px 0;
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: color var(--transition-fast);
}

.detail-panel__symbol-item:hover {
    color: var(--accent-primary);
}

.detail-panel__risk-bar {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    margin-bottom: var(--space-xs);
    font-size: 12px;
    color: var(--text-muted);
}

.detail-panel__risk-bar-label {
    width: 80px;
    flex-shrink: 0;
}

.detail-panel__risk-bar-track {
    flex: 1;
    height: 4px;
    background: var(--bg-elevated);
    border-radius: 2px;
    overflow: hidden;
}

.detail-panel__risk-bar-fill {
    height: 100%;
    border-radius: 2px;
}

.detail-panel__risk-bar-value {
    width: 30px;
    text-align: right;
    font-family: var(--font-mono);
    flex-shrink: 0;
}

/* ======= SOURCE MODAL ======= */
.source-modal {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(6, 8, 12, 0.85);
    backdrop-filter: blur(8px);
    transition: opacity var(--transition-normal);
}

.source-modal__content {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    width: 90vw;
    max-width: 900px;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.source-modal__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-md) var(--space-lg);
    border-bottom: 1px solid var(--border-subtle);
}

.source-modal__path {
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--text-secondary);
}

.source-modal__close {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 20px;
    cursor: pointer;
    padding: 4px;
}

.source-modal__close:hover {
    color: var(--text-primary);
}

.source-modal__code {
    flex: 1;
    overflow: auto;
    padding: var(--space-md) var(--space-lg);
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 1.7;
}

/* ======= SEARCH OVERLAY ======= */
.search-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    margin-top: 4px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    max-height: 400px;
    overflow-y: auto;
    z-index: 110;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
}

.search-result {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    padding: var(--space-sm) var(--space-md);
    cursor: pointer;
    transition: background var(--transition-fast);
}

.search-result:hover,
.search-result--selected {
    background: var(--bg-hover);
}

.search-result__name {
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--text-primary);
}

.search-result__kind {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 10px;
    background: var(--accent-glow);
    color: var(--accent-primary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    flex-shrink: 0;
}

.search-result__file {
    margin-left: auto;
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 180px;
}

.search-hint {
    padding: var(--space-sm) var(--space-md);
    font-size: 11px;
    color: var(--text-muted);
    border-top: 1px solid var(--border-subtle);
    display: flex;
    gap: var(--space-md);
}

.search-hint kbd {
    display: inline-block;
    padding: 1px 5px;
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 10px;
}

/* ======= TOAST ======= */
.toast {
    position: fixed;
    bottom: var(--space-lg);
    right: var(--space-lg);
    z-index: 300;
    padding: var(--space-md) var(--space-lg);
    background: var(--bg-elevated);
    border: 1px solid var(--border-active);
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    gap: var(--space-md);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
}

.toast__message {
    font-size: 13px;
    color: var(--text-secondary);
}

.toast__btn {
    padding: 4px 12px;
    background: var(--accent-primary);
    border: none;
    border-radius: var(--radius-sm);
    color: var(--bg-void);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
}

/* ======= UTILITY ======= */
.hidden { display: none !important; }
.fade-out { opacity: 0; pointer-events: none; }
.fade-in { opacity: 1; pointer-events: auto; }

/* Syntax highlighting */
.syn-keyword { color: #C792EA; }
.syn-string { color: #C3E88D; }
.syn-comment { color: #546E7A; font-style: italic; }
.syn-number { color: #F78C6C; }
.syn-function { color: #82AAFF; }
.syn-type { color: #FFCB6B; }
```

**Verify:** `cargo build` (style.css is embedded via include_str!)
**Expected:** Compiles

### Step 6: Write the v2 HTML shell

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/index.html`
**Location:** Replace entire file

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Ariadne</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500&family=Outfit:wght@300;400;500;600&family=Instrument+Serif&display=swap" rel="stylesheet">
    <link rel="stylesheet" href="/style.css">
</head>
<body>

<!-- Top Bar -->
<div class="top-bar">
    <div class="top-bar__logo" onclick="App.goBack()">Ariadne</div>
    <div class="top-bar__search" id="search-container">
        <span class="top-bar__search-icon">/</span>
        <input type="text" id="search-input" placeholder="Search symbols..." autocomplete="off">
    </div>
    <div class="top-bar__stats" id="top-stats"></div>
</div>

<!-- Back Button -->
<button class="back-btn hidden" id="back-btn" onclick="App.goBack()">&#8592; Signal</button>

<!-- Signal View -->
<div class="signal-view" id="signal-view">
    <div class="signal-hero" id="signal-hero"></div>

    <div class="signal-section" id="signal-risks-section">
        <div class="signal-section__title">Top Risks</div>
        <div class="risk-cards" id="risk-cards"></div>
    </div>

    <div class="signal-section" id="signal-modules-section">
        <div class="signal-section__title">Modules</div>
        <div class="module-grid" id="module-grid"></div>
    </div>

    <div class="signal-section" id="signal-coupling-section">
        <div class="signal-section__title">Coupling</div>
        <div class="coupling-list" id="coupling-list"></div>
    </div>

    <div class="signal-section" id="signal-dead-section">
        <div class="signal-section__title">Dead Code</div>
        <div class="dead-code-grid" id="dead-code-grid"></div>
    </div>
</div>

<!-- Void View -->
<div class="void-view hidden" id="void-view">
    <div class="void-ambient" id="void-ambient"></div>
    <div class="void-dot-grid"></div>
    <div class="void-layers" id="void-layers"></div>
    <svg class="void-connections" id="void-connections"></svg>
    <div class="void-nodes" id="void-nodes"></div>
    <div class="void-hud" id="void-hud">
        <button class="void-hud__btn void-hud__btn--active" data-mode="architecture" onclick="Void.setMode('architecture')">Architecture</button>
        <button class="void-hud__btn" data-mode="risk" onclick="Void.setMode('risk')">Risk</button>
        <button class="void-hud__btn" data-mode="coupling" onclick="Void.setMode('coupling')">Coupling</button>
        <button class="void-hud__btn" onclick="Void.resetLayout()">Reset</button>
    </div>
</div>

<!-- Detail Panel -->
<div class="detail-panel" id="detail-panel">
    <div class="detail-panel__header" id="detail-header"></div>
    <div id="detail-content"></div>
</div>

<!-- Source Modal -->
<div class="source-modal hidden" id="source-modal" onclick="if(event.target===this)SourceModal.close()">
    <div class="source-modal__content">
        <div class="source-modal__header" id="source-modal-header"></div>
        <div class="source-modal__code" id="source-modal-code"></div>
    </div>
</div>

<!-- Reindex Toast -->
<div class="toast hidden" id="reindex-toast">
    <span class="toast__message">Codebase re-indexed. Refresh for latest data.</span>
    <button class="toast__btn">Refresh</button>
</div>

<!-- XSS Prevention -->
<script>
function esc(s) {
    if (s == null) return '';
    return String(s)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}
</script>

<!-- JS Modules -->
<script src="/search.js"></script>
<script src="/signal.js"></script>
<script src="/void-renderer.js"></script>
<script src="/detail-panel.js"></script>
<script src="/source-modal.js"></script>

<!-- App Controller -->
<script>
class App {
    static currentView = 'signal';

    static async init() {
        await Signal.init();
        Search.init();
        App.setupKeyboard();
        App.setupPolling();
    }

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

    static async goBack() {
        DetailPanel.close();
        Void.hide();
        await new Promise(r => setTimeout(r, 200));
        Signal.show();
        Signal.restoreScrollPosition();
        App.currentView = 'signal';
        document.getElementById('back-btn').classList.add('hidden');
    }

    static setupKeyboard() {
        document.addEventListener('keydown', (e) => {
            if (e.key === '/' && document.activeElement.tagName !== 'INPUT') {
                e.preventDefault();
                Search.focus();
            }
            if (e.key === 'Escape') {
                if (SourceModal.isOpen()) {
                    SourceModal.close();
                } else if (DetailPanel.isOpen()) {
                    DetailPanel.close();
                } else if (Search.isOpen && Search.isOpen()) {
                    Search.close();
                } else if (App.currentView === 'void') {
                    App.goBack();
                }
            }
        });
    }

    static setupPolling() {
        let lastIndexed = null;
        setInterval(async () => {
            try {
                const res = await fetch('/api/health');
                const data = await res.json();
                if (lastIndexed && data.last_indexed && data.last_indexed !== lastIndexed) {
                    App.showReindexToast();
                }
                lastIndexed = data.last_indexed || lastIndexed;
            } catch (_) { /* ignore polling errors */ }
        }, 30000);
    }

    static showReindexToast() {
        const toast = document.getElementById('reindex-toast');
        toast.classList.remove('hidden');
        toast.querySelector('button').onclick = () => {
            toast.classList.add('hidden');
            window.location.reload();
        };
    }
}

document.addEventListener('DOMContentLoaded', () => App.init());
</script>
</body>
</html>
```

**Verify:** `cargo build`
**Expected:** Compiles

### Step 7: Update the XSS regression test

**File:** `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs`
**Location:** Replace the entire `test_xss_regression_html_escaping` test function

The existing test checks for patterns like `${esc(r.name)}` and `${esc(r.id)}` which were in the old index.html and `graph-renderer.js`. The new v2 HTML moves search rendering to `search.js` and removes the old graph tooltip. Update the test to check the new file structure:

```rust
#[test]
fn test_xss_regression_html_escaping() {
    // Read the dashboard HTML file directly from the source tree
    let index_html =
        std::fs::read_to_string("src/dashboard/static/index.html").expect("should read index.html");

    // Verify the esc() function exists in the HTML
    assert!(
        index_html.contains("function esc("),
        "index.html must contain the esc() XSS-prevention function"
    );

    // Verify that innerHTML usages that interpolate data use esc()
    let lines: Vec<&str> = index_html
        .lines()
        .filter(|line| line.contains("innerHTML"))
        .collect();

    for line in &lines {
        if line.contains("innerHTML = ''")
            || line.contains("innerHTML = \"\"")
            || line.contains("innerHTML = '<")
        {
            continue;
        }
        if line.trim().ends_with("innerHTML = '';") {
            continue;
        }
        if line.contains("${") {
            assert!(
                line.contains("esc("),
                "innerHTML line with template interpolation must use esc(): {}",
                line.trim()
            );
        }
    }

    // Verify JS files that use innerHTML also reference esc()
    let js_files = [
        "src/dashboard/static/search.js",
        "src/dashboard/static/signal.js",
        "src/dashboard/static/detail-panel.js",
        "src/dashboard/static/source-modal.js",
    ];

    for js_file in &js_files {
        if let Ok(content) = std::fs::read_to_string(js_file) {
            let js_lines: Vec<&str> = content
                .lines()
                .filter(|line| line.contains("innerHTML"))
                .collect();

            for line in &js_lines {
                if line.contains("innerHTML = ''")
                    || line.contains("innerHTML = \"\"")
                {
                    continue;
                }
                if line.trim().ends_with("innerHTML = '';") {
                    continue;
                }
                if line.contains("${") {
                    assert!(
                        line.contains("esc("),
                        "innerHTML in {} with interpolation must use esc(): {}",
                        js_file,
                        line.trim()
                    );
                }
            }
        }
    }
}
```

**Verify:** `cargo test test_xss -- --nocapture`
**Expected:** PASS

### Step 8: Verify app.js still exists (backward compat)

The old `app.js` file exists but is NOT loaded by the new `index.html`. Confirm it still exists. Do NOT delete it.

**Verify:** File exists at `src/dashboard/static/app.js`
**Expected:** File exists

### Step 9: Run the full test suite

**Verify:** `cargo test`
**Expected:** All tests PASS

### Step 10: Run clippy

**Verify:** `cargo clippy -- -D warnings`
**Expected:** No warnings in new code

## Acceptance Criteria

- [ ] `ls /Users/rembrandt/loremllc/ariadne/src/dashboard/static/signal.js /Users/rembrandt/loremllc/ariadne/src/dashboard/static/void-renderer.js /Users/rembrandt/loremllc/ariadne/src/dashboard/static/detail-panel.js /Users/rembrandt/loremllc/ariadne/src/dashboard/static/search.js /Users/rembrandt/loremllc/ariadne/src/dashboard/static/source-modal.js` -> exit 0, all 5 paths printed
- [ ] `cargo build` -> exit 0, compiles (all include_str! files resolve)
- [ ] `cargo test test_xss` -> exit 0, PASS
- [ ] `cargo test` -> exit 0, ALL PASS (no regressions)
- [ ] `cargo clippy -- -D warnings` -> exit 0, no warnings

## Types and Signatures

No new Rust types introduced. All changes are to static file serving infrastructure and frontend assets.

```rust
// New constants in src/dashboard/mod.rs
const STYLE_CSS: &str = include_str!("static/style.css");
const SIGNAL_JS: &str = include_str!("static/signal.js");
const VOID_RENDERER_JS: &str = include_str!("static/void-renderer.js");
const DETAIL_PANEL_JS: &str = include_str!("static/detail-panel.js");
const SEARCH_JS: &str = include_str!("static/search.js");
const SOURCE_MODAL_JS: &str = include_str!("static/source-modal.js");

// New handlers (all same signature pattern):
async fn style_css_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str)
async fn signal_js_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str)
async fn void_renderer_js_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str)
async fn detail_panel_js_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str)
async fn search_js_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str)
async fn source_modal_js_handler() -> (StatusCode, [(HeaderName, &'static str); 1], &'static str)
```

## Imports

No new imports needed -- existing imports in `mod.rs` already cover `axum::http::StatusCode` and `axum::http::header::HeaderName`.

## Completion Contract

**Tests that must pass before signaling done:**
- `cargo test test_xss` -> exit 0
- `cargo test` -> exit 0
- `cargo clippy -- -D warnings` -> exit 0
- `cargo build` -> exit 0

**Files this mini PRD is permitted to touch:**
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/signal.js` (CREATE)
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/void-renderer.js` (CREATE)
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/detail-panel.js` (CREATE)
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/search.js` (CREATE)
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/source-modal.js` (CREATE)
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/mod.rs`
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/style.css`
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/index.html`
- `/Users/rembrandt/loremllc/ariadne/tests/test_dashboard.rs`

**Completion signal:**
PLANFORGE_COMPLETE: PRD-02 Static file serving infrastructure, CSS design system, and HTML shell
