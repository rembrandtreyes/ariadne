# Ariadne Dashboard v2 — Design Spec

## Overview

Redesign the Ariadne web dashboard from a raw graph visualization into a two-view intelligence product: **Signal** (intelligence report landing page) and **Void** (spatial architecture explorer). Navigation is drill-down: Signal is home, clicking any item triggers an animated transition into Void focused on that item, with a back button to return.

**Target user**: Vibe coders who need to understand codebases they didn't write. Descriptions should be educational — explaining what symbols do, how they fit in the system, and why they matter.

**Stack**: Vanilla HTML/CSS/JS embedded in the Rust binary via `include_str!`. No framework, no build step, no npm/bun dependencies. Zero frontend toolchain — just Rust developers writing JS that ships inside `cargo build`. Served by the existing Axum server on port 1337.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Views | Signal + Void | Signal gives instant understanding; Void gives spatial exploration |
| Navigation | Drill-down (Nav B) | Signal is home, click → animated morph into Void, back button returns |
| Descriptions | Level C (full narrative) | Vibe coders need architectural context, not just numbers |
| Description tiers | Template (always) + LLM (MCP) | Template-based Rust descriptions baseline; LLM via MCP when available |
| Source preview | Full function body | Short (<25 lines) inline, long (25+) in modal |
| Module layout | Auto-layout + manual drag | Auto-layout by dependency depth; drag to reposition, saved to localStorage |
| Re-indexing | Poll-based | Check `/api/health` every 30s, toast on change |
| Visual style | Void aesthetic | Dark (#000), glass morphism, ambient orbs, JetBrains Mono + Outfit fonts |

## Signal View (Landing Page)

The intelligence report. User lands here when opening the dashboard.

### Layout (top to bottom)

#### 1. Top Bar
- Logo "Ariadne" (left)
- Search input (center) — searches all symbols/function signatures
- Stats: file count, symbol count, resolution rate (right)

#### 2. Hero Section
- Large serif heading: "Your codebase scores {health_score}"
- Health score number in green gradient
- One-line summary: "{files} files · {languages} languages · {issue_count} issues found"
- Stats row: {dead} dead (red), {cycles} cycles (orange), {gods} gods (yellow), {risky} risky (purple)

#### 3. Critical Risks Section
- Header: "Critical Risks" with count badge
- Top 3-5 risk cards, each containing:
  - Symbol/file name (monospace, large)
  - Risk score (large number, color-coded)
  - File path
  - **Level C description** — full narrative explaining role, architecture position, risk factors
  - Metrics row: fan-in, fan-out, churn, coupling, blast radius
  - Issue tags (critical risk, bottleneck, cycle, god object)
  - "View in map →" link
- Clicking any card → drill-down transition to Void

#### 4. Modules Section
- Header: "Modules" with count
- 2-column grid of module cards, each containing:
  - Module name with health dot (green/yellow/orange/red)
  - Symbol count and file count
  - Health bar (colored segments representing file-level health)
  - Issue tags (cycles, dead code, god objects)
- Clicking any card → drill-down transition to Void

#### 5. Coupling Section
- Header: "Strongest Coupling Pairs"
- Top 5 coupling pairs as rows:
  - Module pair names with arrow (→ for dependency, ↔ for cycle)
  - Strength bar (color-coded: red >0.7, orange >0.4, green <0.4)
  - Strength value
  - Cycle indicator badge if applicable
- Clicking any row → drill-down to Void with both modules highlighted

#### 6. Dead Code Section
- Header: "Dead Code" with count
- 3-column grid of dead function entries:
  - Function name (monospace)
  - File path with line number
- Show first 9, "Show all" expands

### Search Behavior
- Floating search bar in top nav, focused with `/` key
- Searches symbols and function signatures via `/api/search`
- Results dropdown (max 10) shows: name, kind badge, file path, signature preview
- Clicking a result → drill-down to Void focused on that symbol's module, detail panel open showing that symbol
- Escape closes search dropdown

## Void View (Spatial Architecture Explorer)

The living architecture map. Full-screen dark canvas.

### Visual Design
- Background: pure black (#000) with ambient gradient orbs (indigo, pink, green) drifting slowly
- Dot grid: subtle radial gradient dots at 20px spacing
- Layer labels: "Interface" / "Core" / "Data" in small uppercase along the left edge

### Module Nodes
- Glass morphism cards: rgba background + backdrop-filter blur
- Content: module name, symbol count, health status dot (with glow), file sparkline bars
- File sparklines: horizontal bars inside each node representing individual files. Color = file health. Hover a bar → tooltip shows filename
- Issue tags below sparklines (cycles, god object, dead code count)
- Hover: node lifts (translateY -3px), border glows with module's accent color, shadow deepens
- Click: detail panel slides in from right

### Module Layout
- **Auto-layout (default)**: 3 horizontal layers based on dependency depth:
  - Interface layer (top): entry points — main.rs, MCP server, dashboard, LSP
  - Core layer (middle): pipeline, parse, graph, analysis
  - Data layer (bottom): db, search, config, output
  - Within each layer, modules spread horizontally with even spacing
- **Manual positioning**: Drag any node to reposition. New position saved to `localStorage` keyed by module name. Persists across page refreshes.
- **Reset layout**: Button in bottom HUD restores auto-layout and clears localStorage positions.

### SVG Connections
- Curved bezier paths between dependent modules
- Color encoding:
  - Blue (low opacity): normal dependency
  - Red: high coupling (strength > 0.7)
  - Orange dashed: circular dependency (cycle)
- Opacity scales with coupling strength (stronger = more visible)
- Animated flow particles: tiny glowing dots travel along high-coupling edges to show dependency direction

### Detail Panel (right slide-in, 400px)
Triggered by clicking a module node or a symbol within a module.

#### Panel Sections (top to bottom):

**1. Header**
- Module/symbol name (large)
- File path (monospace, muted)
- Close button (×)

**2. Level C Description**
- Full narrative paragraph explaining:
  - What the symbol does (role inferred from name, file path, module, callers/callees)
  - How it fits in the system (which modules call it, what it depends on)
  - Why it matters (risk factors, blast radius, coupling)
- Two-tier generation:
  - **Tier 1 (always)**: Template-based Rust composition from structural signals
  - **Tier 2 (MCP)**: When user has Ariadne MCP connected to Claude, `why_symbol` provides data for Claude to narrate with genuine code understanding

**3. Source Code Preview**
- Full function body with line numbers and lightweight syntax highlighting (keywords, strings, comments colored by language)
- **Short functions (<25 lines)**: Full code shown inline in a scrollable code block
- **Long functions (25+ lines)**: Signature + first 15 lines shown inline, with "View full source" button
- **Source modal**: Full-screen glass-morphism overlay with complete function code, file path header, line numbers. Escape or click outside to close.

**4. Callers**
- "Called by" section with list of calling symbols
- Each entry: name, kind badge (fn/method/class), file path
- Clickable → panel updates to show that symbol (navigation within Void)

**5. Callees**
- "Calls" section with list of called symbols
- Same format as callers, clickable

**6. Risk Factors**
- Labeled horizontal bars for: fan-in, fan-out, churn, coupling strength
- Color-coded (green → yellow → orange → red based on severity)
- Numeric values displayed

**7. Blast Radius**
- "Changes here affect {N} downstream symbols"
- Count of direct and transitive dependents

**8. Issues**
- Active issues for this symbol/module:
  - Dead code indicator
  - Cycle involvement (which modules form the cycle)
  - God object warning (connection count)
  - High volatility warning

### Bottom HUD
- Floating pill bar, centered at bottom
- Mode buttons: Architecture (default) | Risk | Coupling
  - **Architecture**: Nodes colored by type (entry=cyan, core=purple, data=green). Connections show dependency direction.
  - **Risk**: Nodes colored by risk score (green→red gradient). High-risk nodes glow.
  - **Coupling**: Connections colored and thickened by coupling strength. Cycle edges pulse.
- Reset layout button (right side of HUD)

### Keyboard Shortcuts
- `/` — Focus search
- `Escape` — Cascading close: modal → panel → back to Signal
- `← Signal` back button — Returns to Signal with reverse animation

## Drill-Down Transition Animation

### Signal → Void
1. User clicks a module card, risk card, or coupling pair in Signal
2. The clicked card visually expands (CSS transform scale + opacity, 400ms ease-out)
3. Signal content fades out (200ms)
4. Void fades in (300ms) with:
   - The target module pre-selected (glow border)
   - Detail panel auto-open showing that module/symbol
   - "← Signal" back button visible in top-left
5. Total transition: ~400ms

### Void → Signal
1. User clicks "← Signal" or presses Escape (when no panel/modal open)
2. Void fades out (200ms)
3. Signal fades in (300ms)
4. Scroll position in Signal restored

## New API Endpoints

### `GET /api/describe?id={symbol_id}`
Returns a Level C description for a symbol.

```json
{
  "description": "resolve_calls is the call resolution phase of Ariadne's indexing pipeline...",
  "role": "core_pipeline",
  "risk_level": "critical",
  "risk_score": 0.91,
  "metrics": {
    "fan_in": 34,
    "fan_out": 18,
    "modification_count": 42,
    "author_count": 5,
    "is_volatile": true,
    "blast_radius": 34,
    "coupled_file_count": 8,
    "max_coupling_strength": 0.85
  }
}
```

Implementation: New `describe` module in `src/dashboard/` with a `fn describe_symbol(db, graph, symbol_id) -> DescribeResult` that:
1. Fetches health data (`get_symbol_health_data`)
2. Fetches callers/callees (`get_dependents`, `get_dependencies`)
3. Fetches blast radius (`analyze_blast_radius`)
4. Fetches coupling (`get_file_couplings`)
5. Pattern-matches on signal combinations to compose narrative text

### `GET /api/modules`
Returns module-level aggregation grouped by top-level directory.

```json
{
  "modules": [
    {
      "name": "pipeline",
      "path": "src/pipeline",
      "symbol_count": 387,
      "file_count": 14,
      "health": 0.78,
      "risk": 0.71,
      "dead_count": 0,
      "cycle_count": 2,
      "god_objects": 0,
      "files": [
        { "name": "call_resolution.rs", "symbols": 34, "risk": 0.91, "health": 0.45, "dead_count": 0 },
        { "name": "parsing.rs", "symbols": 48, "risk": 0.67, "health": 0.85, "dead_count": 0 }
      ]
    }
  ]
}
```

Implementation: New query function in `src/db/query.rs` that groups symbols by directory, aggregates health/risk per module, and returns file-level breakdowns.

### `GET /api/coupling?limit={n}`
Returns top N coupled file/module pairs.

```json
{
  "pairs": [
    {
      "from_module": "pipeline",
      "to_module": "db",
      "from_file": "src/pipeline/call_resolution.rs",
      "to_file": "src/db/query.rs",
      "strength": 0.85,
      "co_changes": 42,
      "is_cycle": true
    }
  ]
}
```

Implementation: Query the existing `coupling` table, join with `files` for paths, aggregate to module level, cross-reference with circular dependency data.

## Modified Existing Endpoints

### `GET /api/source?id={symbol_id}`
**Change**: Remove the 3-line context limit. Return the full function body from `line_start` to `line_end`.

Add optional `context` query parameter: `/api/source?id=123&context=0` (default 0, was 3).

Response gains `line_count` field:
```json
{
  "code": "fn resolve_calls(...) { ... }",
  "line_start": 23,
  "line_end": 89,
  "line_count": 67,
  "language": "rust",
  "file": "src/pipeline/call_resolution.rs"
}
```

## Re-indexing / Live Updates

**v1**: Poll-based. Frontend checks `/api/health` every 30 seconds. The health endpoint already returns version info; add a `last_indexed` timestamp. If it changes since last check, show a toast: "Codebase re-indexed — Refresh to see changes" with a refresh button. Clicking reloads all data.

**Future**: When running `ariadne dash --watch`, the file watcher triggers incremental re-index, and a `last_indexed` field update signals the frontend to auto-refresh.

## File Structure

```
src/dashboard/
  mod.rs              — Axum server setup, routes (add new routes for static files)
  api.rs              — Existing API handlers (modify source handler, add new endpoints)
  describe.rs         — NEW: Level C description generator (template-based narrative)
  static/
    index.html        — NEW: Complete rewrite — shell + Signal markup + Void markup
    signal.js         — NEW: Signal view logic (hero, risk cards, modules, coupling, dead code)
    void-renderer.js  — NEW: Void canvas (module nodes, SVG connections, ambient effects, layout)
    detail-panel.js   — NEW: Detail panel (descriptions, source preview, callers, callees, metrics)
    search.js         — NEW: Search overlay (symbol search, results dropdown, navigation)
    source-modal.js   — NEW: Full-screen source code viewer modal
    style.css         — NEW: All styles (design tokens, Signal styles, Void styles, animations)
```

No build step. No npm/bun. Each JS file is embedded via `include_str!` and served on its own route. Class-based components with clear separation of concerns. Contributors only need `cargo build`.

## Visual Design Tokens

```
Background:     #000000 (pure black)
Surface:        #09090B, #111113, #19191C
Border:         #1E1E22
Text:           #FAFAF9, #D4D4D8, #A1A1AA, #71717A, #52525B, #3F3F46
Blue:           #60A5FA
Green:          #4ADE80
Yellow:         #FACC15
Orange:         #FB923C
Red:            #F87171
Purple:         #C084FC
Cyan:           #22D3EE

Fonts:          JetBrains Mono (code/data), Outfit (UI text), Instrument Serif (hero)
Border radius:  8px (small), 12px (medium), 14px (large)
Transitions:    150ms (micro), 300ms (panels), 400ms (view transitions)
Glass:          rgba(12,12,14,0.5-0.75) + backdrop-filter: blur(16-24px)
```

## Computed Metrics

### Codebase Health Score (0-100)
Weighted composite of:
- **Resolution rate** (30%): `calls_resolved / total_calls * 100`
- **Dead code ratio** (25%): `(1 - dead_symbols / total_symbols) * 100`
- **Cycle penalty** (20%): `max(0, 100 - cycle_count * 15)`
- **God object penalty** (15%): `max(0, 100 - god_object_count * 20)`
- **Coupling health** (10%): `(1 - avg_coupling_strength) * 100`

### Module Risk Score (0.0-1.0)
Maximum file-level risk score within the module. If no file-level risk data exists, computed as: `(god_object_count * 0.3 + cycle_count * 0.2 + dead_ratio * 0.2 + max_fan_in_normalized * 0.3)`.

### Module Health (0.0-1.0)
`1.0 - module_risk_score`, clamped to [0, 1].

## Font Strategy

Fonts are loaded from Google Fonts CDN for simplicity in v1. The dashboard works without them — CSS specifies fallback stacks:
- `'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace`
- `'Outfit', 'Inter', system-ui, sans-serif`
- `'Instrument Serif', Georgia, serif`

Future: Bundle font files as embedded static assets in the binary for fully offline operation.

## Out of Scope (v1)

- WebSocket/SSE for live updates (use polling instead)
- Orbit view (ego-centric symbol explorer) — future view option
- LLM-generated descriptions in the dashboard itself (only via MCP + Claude)
- Multi-service/monorepo topology view
- User accounts or saved configurations (beyond localStorage layout)
