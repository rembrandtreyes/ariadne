# Lens 3: Stakeholder Interface Report

## Summary
ISSUES(3) fixed — 2 hard, 1 soft

## Boundary 1: Rust API → JS Fetch
CLEAN

All field names consumed by JS match the Rust `Serialize` struct fields exactly.

- `DescribeResult` (`description`, `role`, `risk_level`, `risk_score`, `metrics`): consumed in PRD-03 (`desc.risk_level`, `desc.description`) and PRD-05 (`describe.description`, `describe.role`, `describe.risk_level`, `describe.risk_score`). All match.
- `DescribeMetrics` (`fan_in`, `fan_out`, `modification_count`, `author_count`, `is_volatile`, `blast_radius`, `coupled_file_count`, `max_coupling_strength`): accessed in PRD-05 `renderRiskFactors` and `renderBlastRadius`. All fields match exactly.
- `SourceResult` (`code`, `line_start`, `line_end`, `line_count`, `language`, `file`): accessed in PRD-05 `DetailPanel.renderSource` and `SourceModal.render` as `source.code`, `source.line_start`, `source.line_end`, `source.line_count`, `source.language`, `source.file`. All match.
- `ModulesResponse` → `modulesData.modules`, `ModuleSummary` fields (`name`, `symbol_count`, `file_count`, `dead_count`, `health`, `files`): accessed in PRD-03 signal.js and PRD-04 void-renderer.js. All match.
- `ModuleFileSummary` fields (`health`) accessed as `f.health` in PRD-03 and PRD-04 sparklines. Match.
- Coupling response `pairs` array, fields `from_module`, `to_module`, `from_file`, `to_file`, `strength`: consumed in PRD-03 and PRD-04. All match.
- `Stats` fields (`files`, `symbols`, `calls`, `resolution_rate`, `dead_functions`, `languages`): consumed in PRD-03 signal.js. All match.
- `Insights` fields (`circular_deps`, `most_connected`, `god_files`, `dead_code`): consumed in PRD-03 signal.js. All match.

## Boundary 2: HTML DOM → JS querySelector
CLEAN

Every `getElementById` and `querySelector` call in JS (PRD-03, 04, 05) has a corresponding element defined in PRD-02's HTML shell.

| JS call | HTML element | Status |
|---------|-------------|--------|
| `getElementById('search-input')` | `<input id="search-input">` | OK |
| `getElementById('search-container')` | `<div id="search-container">` | OK |
| `getElementById('top-stats')` | `<div id="top-stats">` | OK |
| `getElementById('signal-view')` | `<div id="signal-view">` | OK |
| `getElementById('signal-hero')` | `<div id="signal-hero">` | OK |
| `getElementById('risk-cards')` | `<div id="risk-cards">` | OK |
| `getElementById('module-grid')` | `<div id="module-grid">` | OK |
| `getElementById('coupling-list')` | `<div id="coupling-list">` | OK |
| `getElementById('dead-code-grid')` | `<div id="dead-code-grid">` | OK |
| `getElementById('void-view')` | `<div id="void-view">` | OK |
| `getElementById('void-ambient')` | `<div id="void-ambient">` | OK |
| `getElementById('void-nodes')` | `<div id="void-nodes">` | OK |
| `getElementById('void-connections')` | `<svg id="void-connections">` | OK |
| `getElementById('void-layers')` | `<div id="void-layers">` | OK |
| `querySelectorAll('.void-hud__btn[data-mode]')` | HUD `<button data-mode="...">` elements in `#void-hud` | OK |
| `getElementById('detail-panel')` | `<div id="detail-panel">` | OK |
| `getElementById('detail-header')` | `<div id="detail-header">` | OK |
| `getElementById('detail-content')` | `<div id="detail-content">` | OK |
| `getElementById('source-modal')` | `<div id="source-modal">` | OK |
| `getElementById('source-modal-header')` | `<div id="source-modal-header">` | OK |
| `getElementById('source-modal-code')` | `<div id="source-modal-code">` | OK |
| `getElementById('back-btn')` | `<button id="back-btn">` | OK |
| `getElementById('reindex-toast')` | `<div id="reindex-toast">` | OK |

## Boundary 3: JS Class → JS Class
CLEAN (after soft fix)

| Method | Defined in | Called by | Status |
|--------|-----------|-----------|--------|
| `App.drillDown(moduleName, symbolId)` | PRD-02 App (inline script) | PRD-03 signal.js risk cards, module cards, coupling rows, dead code items; PRD-03 search.js `selectResult` | OK |
| `Signal.init()` | PRD-03 signal.js | PRD-02 `App.init()` | OK |
| `Signal.show()` | PRD-03 signal.js | PRD-02 `App.goBack()` | OK |
| `Signal.hide()` | PRD-03 signal.js | PRD-02 `App.drillDown()` | OK |
| `Signal.saveScrollPosition()` | PRD-03 signal.js | PRD-02 `App.drillDown()` — was bypassed (soft violation, fixed) | FIXED |
| `Signal.restoreScrollPosition()` | PRD-03 signal.js | PRD-02 `App.goBack()` | OK |
| `Void.show(focusModule, focusSymbol)` | PRD-04 void-renderer.js | PRD-02 `App.drillDown()` as `Void.show(moduleName, symbolId)` | OK |
| `Void.hide()` | PRD-04 void-renderer.js | PRD-02 `App.goBack()` | OK |
| `Void.selectModule(moduleName)` | PRD-04 void-renderer.js | PRD-04 internal — `Void.show()` calls it after timeout | OK |
| `Void.resetLayout()` | PRD-04 void-renderer.js | PRD-02 HTML HUD Reset button `onclick` | OK |
| `Void.setMode(mode)` | PRD-04 void-renderer.js | PRD-02 HTML HUD Architecture/Risk/Coupling button `onclick` attrs | OK |
| `DetailPanel.open(symbolId)` | PRD-05 detail-panel.js | PRD-02 `App.drillDown()`; PRD-04 `Void.show()` (guarded: `typeof DetailPanel !== 'undefined'`) | OK |
| `DetailPanel.close()` | PRD-05 detail-panel.js | PRD-02 `App.goBack()`, keyboard handler; PRD-05 close button `onclick` | OK |
| `DetailPanel.isOpen()` | PRD-05 detail-panel.js | PRD-02 keyboard handler | OK |
| `SourceModal.open(sourceData)` | PRD-05 source-modal.js | PRD-05 detail-panel.js `renderSource` "View full source" button `onclick` | OK |
| `SourceModal.close()` | PRD-05 source-modal.js | PRD-02 keyboard handler; PRD-05 modal close button `onclick` | OK |
| `SourceModal.isOpen()` | PRD-05 source-modal.js | PRD-02 keyboard handler | OK |
| `Search.init()` | PRD-03 search.js | PRD-02 `App.init()` | OK |
| `Search.focus()` | PRD-03 search.js | PRD-02 keyboard handler (`/` key) | OK |
| `Search.close()` | PRD-03 search.js | PRD-02 keyboard handler | OK |
| `Search.isOpen()` | PRD-03 search.js | PRD-02 keyboard handler (`Search.isOpen && Search.isOpen()`) | OK |

## Boundary 4: CSS Classes → HTML/JS
ISSUES(2) fixed

All class names used via `className` or inline HTML template literals in JS match CSS class definitions in PRD-02, with the following exceptions that were fixed:

- **PRD-05 `detail-panel.js` `renderHeader`**: Close button was `class="detail-close"`. No such class in CSS. CSS defines `.detail-panel__close`. **Fixed to `detail-panel__close`.**
- **PRD-05 `source-modal.js` `render`**: Close button was `class="modal-close"`. No such class in CSS. CSS defines `.source-modal__close`. **Fixed to `source-modal__close`.**

All other class usages verified clean:
- search.js: `search-dropdown`, `search-result`, `search-result--selected`, `search-result__name`, `search-result__kind`, `search-result__file`, `search-hint` — all defined in CSS.
- signal.js: `signal-hero__score/label/summary/stats/stat-value`, `top-bar__stat-value`, `risk-card`, `risk-card__header/name/badge/badge--{level}/description`, `module-card`, `module-card__name/stats/health-bar/health-fill/files/file-bar`, `coupling-row`, `coupling-row__files/arrow/strength/bar/bar-fill`, `dead-code-item`, `dead-code-item__icon/name/file`, `fade-in`, `fade-out`, `hidden` — all in CSS.
- void-renderer.js: `void-node`, `void-node__glow/name/stats/sparkline/spark-bar`, `void-node--selected`, `void-hud__btn--active`, `void-ambient__orb` — all in CSS.
- detail-panel.js (post-fix): `detail-panel--open`, `detail-panel__header/name/file/close/section/section-title/description/code/code-line-num/view-source/symbol-list/symbol-item/risk-bar/risk-bar-label/risk-bar-track/risk-bar-fill/risk-bar-value`, `risk-card__badge--{level}`, `syn-keyword/string/comment/number` — all in CSS.
- source-modal.js (post-fix): `source-modal__path`, `source-modal__close`, `detail-panel__code-line-num` — all in CSS.

## Hard Violations Fixed

1. **[PRD-05, detail-panel.js, `renderHeader`]** Close button class `detail-close` → `detail-panel__close`. The missing CSS class would have left the close button completely unstyled (no positioning, color, hover, or cursor). Fixed in PRD-05.

2. **[PRD-05, source-modal.js, `render`]** Close button class `modal-close` → `source-modal__close`. The missing CSS class would have left the source modal close button unstyled. Fixed in PRD-05.

## Soft Violations Fixed

1. **[PRD-02, App controller, `drillDown`]** `Signal.saveScrollPosition()` was defined as an interface contract in PRD-03 (Boundary 3 spec lists it as called by App), but `App.drillDown` bypassed it with `App.signalScrollY = window.scrollY` (duplicate inline logic) and the unused `static signalScrollY = 0` field. Fixed: replaced with `Signal.saveScrollPosition()` call and removed `App.signalScrollY`. `App.goBack()` already correctly calls `Signal.restoreScrollPosition()`, which reads from `Signal._scrollY` — the same field `saveScrollPosition()` writes to, so the round-trip is now consistent.
