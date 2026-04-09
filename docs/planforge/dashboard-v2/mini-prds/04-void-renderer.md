# Mini PRD 04: Void Renderer -- Spatial Architecture Map

> **Dependency:** Requires PRD-02 (HTML shell with void-view containers must exist; `void-renderer.js` is a stub after PRD-02 and this PRD replaces it entirely)
> **Produces:** Functional void-renderer.js with module node layout, SVG connections, drag interaction, mode switching, and flow particle animation
> **Estimated steps:** 7

## Context

This PRD implements the Void spatial architecture map -- the second view of the dashboard that shows modules as glass-morphism nodes with SVG connections between them. Users drill into Void from Signal by clicking a module card, and can drag nodes, switch between Architecture/Risk/Coupling coloring modes, and click nodes to open the detail panel. The layout algorithm assigns modules to Interface/Core/Data layers based on dependency direction. Node positions persist in localStorage using individual per-module keys. Flow particles animate along SVG connection paths using `requestAnimationFrame` and `SVGPathElement.getPointAtLength()`.

## Files

| Action | Path | Purpose |
|--------|------|---------|
| MODIFY | `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/void-renderer.js` | Full Void renderer implementation (replaces stub created by PRD-02) |

## Existing Code Context

### API Endpoints (from PRD-01)

**GET /api/modules** — returns:
```json
{
  "modules": [
    {
      "name": "pipeline",
      "path": "src/pipeline",
      "symbol_count": 42,
      "file_count": 5,
      "health": 0.87,
      "risk": 0.13,
      "dead_count": 2,
      "cycle_count": 0,
      "god_objects": 0,
      "files": [
        { "name": "mod.rs", "symbol_count": 12, "dead_count": 1, "risk": 0.08, "health": 0.92 }
      ]
    }
  ]
}
```

**GET /api/coupling?limit=20** — returns:
```json
{
  "pairs": [
    {
      "from_module": "pipeline",
      "to_module": "db",
      "from_file": "src/pipeline/foo.rs",
      "to_file": "src/db/bar.rs",
      "strength": 0.75,
      "co_changes": 12,
      "is_cycle": false
    }
  ]
}
```

### DOM Structure (from PRD-02)

The `#void-view` container has two children created by PRD-02's HTML:
- `<svg id="void-connections">` — SVG layer for bezier path connections
- `<div id="void-nodes">` — container for `.void-node` divs

Additional containers also created by PRD-02:
- `<div id="void-ambient">` — background gradient orbs
- `<div id="void-layers">` — layer labels (Interface / Core / Data)
- `.void-hud__btn[data-mode]` buttons — Architecture / Risk / Coupling mode toggles

### Global Dependencies

- `esc(s)` — defined in `index.html`. Escapes a string for safe innerHTML insertion. Use for every `${}` interpolation in template literals assigned to `.innerHTML`.
- `DetailPanel.open(symbolId)` — defined in `detail-panel.js` (PRD-05), globally available at runtime. Called when a module node is clicked.
- `App.drillDown()` — defined in `index.html`, globally available.

### localStorage Keys

Node positions are stored per-module using the key pattern `ariadne_void_pos_{moduleName}` (e.g., `ariadne_void_pos_pipeline`). Each value is a JSON string `{"x": 120, "y": 340}`.

## Steps

### Step 1: Implement void-renderer.js

**File:** `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/void-renderer.js`
**Location:** Replace entire file

```javascript
// Ariadne Dashboard v2 -- Void Renderer
'use strict';

class Void {
    static _modules = [];
    static _coupling = [];
    static _nodeEls = {};
    static _positions = {};
    static _mode = 'architecture';
    static _focusModule = null;
    static _dragState = null;
    static _initialized = false;
    static _particleRafs = [];

    // Fetch /api/modules and /api/coupling?limit=20.
    // Stores results in Void._modules and Void._coupling.
    // Sets Void._initialized = true on success.
    // No-ops if already initialized.
    static async init() {
        if (Void._initialized) return;
        try {
            const [modulesRes, couplingRes] = await Promise.all([
                fetch('/api/modules'),
                fetch('/api/coupling?limit=20'),
            ]);
            const modulesData = await modulesRes.json();
            const couplingData = await couplingRes.json();
            Void._modules = modulesData.modules || [];
            Void._coupling = couplingData.pairs || [];
            Void._initialized = true;
        } catch (e) {
            console.error('Void init error:', e);
        }
    }

    // Show the void view. Calls init(), creates nodes, runs auto-layout,
    // draws connections, and starts flow particle animation.
    // focusModule: string|null — module name to highlight on show
    // focusSymbol: number|null — symbol id to pass to DetailPanel.open() after highlight
    static async show(focusModule, focusSymbol) {
        await Void.init();
        Void._focusModule = focusModule || null;

        const view = document.getElementById('void-view');
        if (!view) return;

        view.classList.remove('hidden', 'fade-out');
        view.classList.add('fade-in');

        Void.createAmbientBackground();
        Void.createNodes(Void._modules);
        Void.loadSavedPositions();
        Void.autoLayout(Void._modules);
        Void.drawConnections(Void._modules, Void._coupling);
        Void._renderLayerLabels();
        Void.animateFlowParticles();

        if (focusModule) {
            setTimeout(() => {
                Void.selectModule(focusModule);
                if (focusSymbol && typeof DetailPanel !== 'undefined') {
                    DetailPanel.open(focusSymbol);
                }
            }, 100);
        }
    }

    // Hide the void view with a fade-out transition (250ms).
    // Cancels all particle animation frames.
    // Clears #void-nodes and #void-connections innerHTML.
    // Resets Void._nodeEls to {}.
    static hide() {
        for (const raf of Void._particleRafs) {
            cancelAnimationFrame(raf);
        }
        Void._particleRafs = [];

        const view = document.getElementById('void-view');
        if (view) {
            view.classList.add('fade-out');
            setTimeout(() => view.classList.add('hidden'), 250);
        }

        const nodesContainer = document.getElementById('void-nodes');
        if (nodesContainer) nodesContainer.innerHTML = '';

        const connContainer = document.getElementById('void-connections');
        if (connContainer) connContainer.innerHTML = '';

        Void._nodeEls = {};
    }

    // Create and append background gradient orbs to #void-ambient.
    // No-ops if #void-ambient already has children.
    // Creates 3 orbs: gold at 20%/30%, green at 70%/60%, red at 50%/20%.
    static createAmbientBackground() {
        const container = document.getElementById('void-ambient');
        if (!container || container.children.length > 0) return;

        const orbs = [
            { color: 'rgba(212, 168, 83, 0.08)', size: 400, x: '20%', y: '30%' },
            { color: 'rgba(74, 222, 128, 0.05)', size: 300, x: '70%', y: '60%' },
            { color: 'rgba(248, 113, 113, 0.04)', size: 350, x: '50%', y: '20%' },
        ];

        for (const orb of orbs) {
            const el = document.createElement('div');
            el.className = 'void-ambient__orb';
            el.style.cssText = [
                'position:absolute',
                'border-radius:50%',
                'filter:blur(60px)',
                'pointer-events:none',
                `width:${orb.size}px`,
                `height:${orb.size}px`,
                `background:${orb.color}`,
                `left:${orb.x}`,
                `top:${orb.y}`,
                'transform:translate(-50%,-50%)',
            ].join(';');
            container.appendChild(el);
        }
    }

    // Create one .void-node div per module and append to #void-nodes.
    // Clears existing content first. Stores references in Void._nodeEls keyed by module name.
    // Each node: <div class="void-node" data-module="{name}"> with glow, name, stats, sparkline.
    // modules: array of module objects from /api/modules response
    static createNodes(modules) {
        const container = document.getElementById('void-nodes');
        if (!container) return;
        container.innerHTML = '';
        Void._nodeEls = {};

        for (const m of modules) {
            const el = document.createElement('div');
            el.className = 'void-node';
            el.dataset.module = m.name;

            const healthPct = Math.round((m.health || 0) * 100);
            const glowColor = Void._healthColor(healthPct);

            // Per-file sparkline bars (up to 15 files)
            let sparkline = '';
            const files = (m.files || []).slice(0, 15);
            for (const f of files) {
                const h = Math.round((f.health || 0) * 100);
                const barH = Math.max(2, Math.round(h / 100 * 16));
                const barColor = Void._healthColor(h);
                sparkline += `<div class="void-node__spark-bar" style="height:${esc(String(barH))}px;background:${esc(barColor)}"></div>`;
            }

            el.innerHTML = `
                <div class="void-node__glow" style="box-shadow: 0 0 15px ${esc(glowColor)}"></div>
                <div class="void-node__name">${esc(m.name)}</div>
                <div class="void-node__stats">${esc(String(m.symbol_count))} sym / ${esc(String(m.file_count))} files</div>
                <div class="void-node__sparkline">${sparkline}</div>
            `;

            el.addEventListener('click', () => {
                if (Void._dragState && Void._dragState.moved) return;
                Void.selectModule(m.name);
            });

            Void.enableDrag(el, m.name);
            container.appendChild(el);
            Void._nodeEls[m.name] = el;
        }
    }

    // Assign modules to Interface/Core/Data layers and position their .void-node elements.
    // Layer rules (applied in order; a module matches the first rule that fits):
    //   Interface: modules with 0 incoming module-level coupling entries (entry points — nothing calls them)
    //   Data:      modules with 0 outgoing module-level coupling entries (leaf nodes — they call nothing)
    //   Core:      all remaining modules
    // X positions: Interface = 15% of container width, Core = 50%, Data = 85%
    // Y positions: evenly spaced within each layer column, starting at (height - count*spacing)/2 + 40
    // Saved positions from localStorage override computed positions.
    // modules: array of module objects from /api/modules response
    static autoLayout(modules) {
        const container = document.getElementById('void-nodes');
        if (!container) return;

        const rect = container.getBoundingClientRect();
        const width = rect.width || window.innerWidth;
        const height = rect.height || (window.innerHeight - 56);

        // Classify modules into layers using coupling data
        const layers = { interface: [], core: [], data: [] };
        for (const m of modules) {
            const layer = Void._classifyLayer(m.name, Void._coupling);
            layers[layer].push(m);
        }

        const layerX = {
            interface: width * 0.15,
            core: width * 0.50,
            data: width * 0.85,
        };

        for (const [layerName, layerModules] of Object.entries(layers)) {
            const x = layerX[layerName];
            const spacing = Math.min(100, (height - 100) / Math.max(layerModules.length, 1));
            const startY = (height - layerModules.length * spacing) / 2 + 40;

            for (let i = 0; i < layerModules.length; i++) {
                const m = layerModules[i];
                const saved = Void._positions[m.name];
                if (saved) {
                    Void._setNodePosition(m.name, saved.x, saved.y);
                } else {
                    const posX = x - 80 + (Math.random() - 0.5) * 40;
                    const posY = startY + i * spacing;
                    Void._positions[m.name] = { x: posX, y: posY };
                    Void._setNodePosition(m.name, posX, posY);
                }
            }
        }
    }

    // Load per-module positions from localStorage.
    // Key pattern: ariadne_void_pos_{moduleName} (e.g., ariadne_void_pos_pipeline)
    // Each value is a JSON string {"x": number, "y": number}.
    // Populates Void._positions with all found entries.
    static loadSavedPositions() {
        Void._positions = {};
        for (const m of Void._modules) {
            try {
                const raw = localStorage.getItem('ariadne_void_pos_' + m.name);
                if (raw) {
                    const pos = JSON.parse(raw);
                    if (typeof pos.x === 'number' && typeof pos.y === 'number') {
                        Void._positions[m.name] = pos;
                    }
                }
            } catch (_) {}
        }
    }

    // Save a module's position to localStorage.
    // Key: ariadne_void_pos_{moduleName}
    // Value: JSON string {"x": x, "y": y}
    // moduleName: string — module name (e.g., "pipeline")
    // x: number — left offset in pixels
    // y: number — top offset in pixels
    static savePosition(moduleName, x, y) {
        Void._positions[moduleName] = { x, y };
        try {
            localStorage.setItem('ariadne_void_pos_' + moduleName, JSON.stringify({ x, y }));
        } catch (_) {}
    }

    // Clear all saved positions from Void._positions and localStorage.
    // Removes localStorage keys for all currently loaded modules.
    // Re-runs autoLayout and drawConnections to recompute from scratch.
    static resetLayout() {
        for (const m of Void._modules) {
            try {
                localStorage.removeItem('ariadne_void_pos_' + m.name);
            } catch (_) {}
        }
        Void._positions = {};
        Void.autoLayout(Void._modules);
        Void.drawConnections(Void._modules, Void._coupling);
    }

    // Draw SVG cubic bezier paths between coupled module nodes in #void-connections.
    // Clears existing SVG content first.
    // Skips self-coupling (from_module === to_module).
    // De-duplicates bidirectional pairs (sorts module names as key).
    // Path formula: M x1,y1 C cx1,cy1 cx2,cy2 x2,y2
    //   cx1 = x1 + (x2-x1)*0.4, cy1 = y1
    //   cx2 = x2 - (x2-x1)*0.4, cy2 = y2
    // Stroke color by coupling strength:
    //   strength >= 0.7: rgba(248,113,113,opacity) (red)
    //   strength >= 0.4: rgba(250,204,21,opacity)  (yellow)
    //   else:            rgba(138,143,156,opacity)  (grey)
    // opacity = 0.2 + strength * 0.6
    // stroke-width = 1 + strength * 2
    // Each path gets fill="none" and data-from / data-to attributes for particle animation.
    // modules: array of module objects (unused directly; positions come from Void._nodeEls)
    // coupling: array of coupling pair objects from /api/coupling response
    static drawConnections(modules, coupling) {
        const svg = document.getElementById('void-connections');
        if (!svg) return;
        svg.innerHTML = '';

        const drawn = new Set();
        for (const c of coupling) {
            if (c.from_module === c.to_module) continue;
            const key = [c.from_module, c.to_module].sort().join('|');
            if (drawn.has(key)) continue;
            drawn.add(key);

            const from = Void._getNodeCenter(c.from_module);
            const to = Void._getNodeCenter(c.to_module);
            if (!from || !to) continue;

            const strength = c.strength || 0;
            const opacity = 0.2 + strength * 0.6;
            const strokeWidth = 1 + strength * 2;
            const color = strength >= 0.7
                ? `rgba(248,113,113,${opacity})`
                : strength >= 0.4
                    ? `rgba(250,204,21,${opacity})`
                    : `rgba(138,143,156,${opacity})`;

            const dx = to.x - from.x;
            const cx1 = from.x + dx * 0.4;
            const cy1 = from.y;
            const cx2 = to.x - dx * 0.4;
            const cy2 = to.y;

            const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
            path.setAttribute('d', `M ${from.x} ${from.y} C ${cx1} ${cy1}, ${cx2} ${cy2}, ${to.x} ${to.y}`);
            path.setAttribute('stroke', color);
            path.setAttribute('stroke-width', String(strokeWidth));
            path.setAttribute('fill', 'none');
            path.setAttribute('data-from', c.from_module);
            path.setAttribute('data-to', c.to_module);
            svg.appendChild(path);
        }

        // Restart particles after redrawing paths
        for (const raf of Void._particleRafs) {
            cancelAnimationFrame(raf);
        }
        Void._particleRafs = [];
        Void.animateFlowParticles();
    }

    // Animate small circular particles traveling along each SVG connection path.
    // One particle per path. Uses requestAnimationFrame for smooth animation.
    // Each particle is a <circle> element (r=3) appended to #void-connections.
    // Animation: particle moves along path.getTotalLength() at 40px/frame speed (frame ~16ms).
    // Particle color matches path stroke color.
    // When a particle reaches the end of its path, it resets to the start.
    // Stores each requestAnimationFrame handle in Void._particleRafs for cancellation.
    static animateFlowParticles() {
        const svg = document.getElementById('void-connections');
        if (!svg) return;

        const paths = svg.querySelectorAll('path[data-from]');
        for (const pathEl of paths) {
            const totalLength = pathEl.getTotalLength();
            if (totalLength < 10) continue;

            const circle = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
            circle.setAttribute('r', '3');
            circle.setAttribute('fill', pathEl.getAttribute('stroke'));
            circle.setAttribute('opacity', '0.8');
            svg.appendChild(circle);

            let offset = Math.random() * totalLength; // stagger start positions
            const speed = 40; // pixels per second
            let lastTime = null;

            function step(timestamp) {
                if (lastTime === null) lastTime = timestamp;
                const dt = (timestamp - lastTime) / 1000; // seconds
                lastTime = timestamp;

                offset = (offset + speed * dt) % totalLength;
                const pt = pathEl.getPointAtLength(offset);
                circle.setAttribute('cx', String(pt.x));
                circle.setAttribute('cy', String(pt.y));

                const handle = requestAnimationFrame(step);
                Void._particleRafs.push(handle);
            }

            const handle = requestAnimationFrame(step);
            Void._particleRafs.push(handle);
        }
    }

    // Attach mousedown/mousemove/mouseup drag handlers to a node element.
    // Drag threshold: 3px movement before Void._dragState.moved = true.
    // Position is applied via nodeEl.style.left and nodeEl.style.top (absolute px).
    // On mouseup after a drag: calls Void.savePosition() and Void.drawConnections().
    // Sets Void._dragState = null after 10ms (allows click handler to detect moved state).
    // nodeEl: HTMLElement — the .void-node div
    // moduleName: string — used as key for Void.savePosition()
    static enableDrag(nodeEl, moduleName) {
        nodeEl.addEventListener('mousedown', (e) => {
            if (e.button !== 0) return;
            e.preventDefault();

            const startX = e.clientX;
            const startY = e.clientY;
            const startLeft = parseInt(nodeEl.style.left || '0', 10);
            const startTop = parseInt(nodeEl.style.top || '0', 10);

            Void._dragState = { moduleName, startX, startY, startLeft, startTop, moved: false };

            const onMove = (me) => {
                const dx = me.clientX - startX;
                const dy = me.clientY - startY;
                if (Math.abs(dx) > 3 || Math.abs(dy) > 3) {
                    Void._dragState.moved = true;
                }
                nodeEl.style.left = (startLeft + dx) + 'px';
                nodeEl.style.top = (startTop + dy) + 'px';
            };

            const onUp = (me) => {
                document.removeEventListener('mousemove', onMove);
                document.removeEventListener('mouseup', onUp);

                if (Void._dragState && Void._dragState.moved) {
                    const newX = startLeft + (me.clientX - startX);
                    const newY = startTop + (me.clientY - startY);
                    Void.savePosition(moduleName, newX, newY);
                    Void.drawConnections(Void._modules, Void._coupling);
                }

                setTimeout(() => { Void._dragState = null; }, 10);
            };

            document.addEventListener('mousemove', onMove);
            document.addEventListener('mouseup', onUp);
        });
    }

    // Highlight the named module node (adds .void-node--selected, removes from all others).
    // Scrolls the selected node into view.
    // Does NOT open DetailPanel -- callers (Void.show with focusSymbol, or App.drillDown with symbolId) are responsible.
    // moduleName: string — module name matching data-module attribute
    static selectModule(moduleName) {
        for (const el of Object.values(Void._nodeEls)) {
            el.classList.remove('void-node--selected');
        }

        const el = Void._nodeEls[moduleName];
        if (el) {
            el.classList.add('void-node--selected');
            el.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
        }

        // Detail panel is opened by Void.show() when focusSymbol is provided,
        // or by the caller (App.drillDown) passing a symbolId.
        // selectModule() itself does NOT call DetailPanel.open() -- it has no symbol ID.
    }

    // Switch the active visualization mode and recolor all node glows.
    // Modes: 'architecture' (default, color by health), 'risk' (color by risk), 'coupling' (color by coupling count).
    // Updates .void-hud__btn--active class on .void-hud__btn[data-mode] buttons.
    // Node health colors:
    //   green  #4ADE80 (health > 0.8)
    //   yellow #FACC15 (health 0.6-0.8)
    //   orange #FB923C (health 0.4-0.6)
    //   red    #F87171 (health < 0.4)
    // Risk mode: inverts health scale (high risk = red glow).
    // Coupling mode: normalizes coupling pair count to 0-1 over max 5 pairs; high count = red glow.
    // mode: string — one of 'architecture', 'risk', 'coupling'
    static setMode(mode) {
        Void._mode = mode;

        const btns = document.querySelectorAll('.void-hud__btn[data-mode]');
        btns.forEach(btn => {
            btn.classList.toggle('void-hud__btn--active', btn.dataset.mode === mode);
        });

        for (const m of Void._modules) {
            const el = Void._nodeEls[m.name];
            if (!el) continue;

            const glow = el.querySelector('.void-node__glow');
            if (!glow) continue;

            let color;
            if (mode === 'risk') {
                const riskPct = Math.round((m.risk || 0) * 100);
                color = Void._healthColor(100 - riskPct);
            } else if (mode === 'coupling') {
                const cCount = Void._coupling.filter(c =>
                    c.from_module === m.name || c.to_module === m.name
                ).length;
                const intensity = Math.min(cCount / 5, 1) * 100;
                color = Void._healthColor(100 - intensity);
            } else {
                const healthPct = Math.round((m.health || 0) * 100);
                color = Void._healthColor(healthPct);
            }

            glow.style.boxShadow = '0 0 15px ' + color;
        }
    }

    // --- Private helpers ---

    // Return RGBA glow color string for a given health percentage.
    // healthPct: number — integer 0-100
    // Returns: string — one of four rgba() colors:
    //   >= 80: rgba(74, 222, 128, 0.4)   (green  #4ADE80)
    //   >= 60: rgba(250, 204, 21, 0.4)   (yellow #FACC15)
    //   >= 40: rgba(251, 146, 60, 0.4)   (orange #FB923C)
    //   <  40: rgba(248, 113, 113, 0.4)  (red    #F87171)
    static _healthColor(healthPct) {
        if (healthPct >= 80) return 'rgba(74, 222, 128, 0.4)';
        if (healthPct >= 60) return 'rgba(250, 204, 21, 0.4)';
        if (healthPct >= 40) return 'rgba(251, 146, 60, 0.4)';
        return 'rgba(248, 113, 113, 0.4)';
    }

    // Classify a module into 'interface', 'core', or 'data' layer based on coupling direction.
    // Interface: 0 incoming coupling entries (nothing depends on this module — it is an entry point)
    // Data:      0 outgoing coupling entries (this module depends on nothing — it is a leaf node)
    // Core:      has both incoming and outgoing coupling entries
    // moduleName: string — module name
    // coupling: array of coupling pair objects from Void._coupling
    // Returns: string — 'interface', 'data', or 'core'
    static _classifyLayer(moduleName, coupling) {
        let outgoing = 0;
        let incoming = 0;

        for (const c of coupling) {
            if (c.from_module === moduleName && c.to_module !== moduleName) outgoing++;
            if (c.to_module === moduleName && c.from_module !== moduleName) incoming++;
        }

        if (incoming === 0) return 'interface';
        if (outgoing === 0) return 'data';
        return 'core';
    }

    // Apply left/top position to a node element by module name.
    // name: string — module name key in Void._nodeEls
    // x: number — left offset in pixels
    // y: number — top offset in pixels
    static _setNodePosition(name, x, y) {
        const el = Void._nodeEls[name];
        if (el) {
            el.style.left = x + 'px';
            el.style.top = y + 'px';
        }
    }

    // Return the center point of a node element relative to its parent container.
    // name: string — module name key in Void._nodeEls
    // Returns: {x: number, y: number} or null if element not found
    static _getNodeCenter(name) {
        const el = Void._nodeEls[name];
        if (!el) return null;
        const rect = el.getBoundingClientRect();
        const parent = el.parentElement.getBoundingClientRect();
        return {
            x: rect.left - parent.left + rect.width / 2,
            y: rect.top - parent.top + rect.height / 2,
        };
    }

    // Render layer labels (Interface / Core / Data) into #void-layers.
    static _renderLayerLabels() {
        const container = document.getElementById('void-layers');
        if (!container) return;
        container.innerHTML = '<span>Interface</span><span>Core</span><span>Data</span>';
    }
}
```

**Verify:** `cargo build`
**Expected:** Compiles without errors

### Step 2: Verify XSS safety

Audit all innerHTML usage in void-renderer.js. Every `${...}` interpolation inside a `.innerHTML` assignment must use `esc()`.

Patterns present:
- `${esc(String(barH))}` — OK
- `${esc(barColor)}` — OK
- `${esc(glowColor)}` — OK
- `${esc(m.name)}` — OK
- `${esc(String(m.symbol_count))}` — OK
- `${esc(String(m.file_count))}` — OK

`_renderLayerLabels()` uses only hardcoded string literals — no interpolation needed.

**Verify:** Visual inspection
**Expected:** All interpolations use esc()

### Step 3: Verify XSS test passes

**Verify:** `cargo test test_xss -- --nocapture`
**Expected:** PASS

### Step 4: Run cargo build

**Verify:** `cargo build`
**Expected:** Compiles without errors

### Step 5: Run full test suite

**Verify:** `cargo test`
**Expected:** All tests PASS

### Step 6: Run clippy

**Verify:** `cargo clippy -- -D warnings`
**Expected:** No warnings

### Step 7: Manual smoke test

**Verify:** `cargo run -- index . && cargo run -- dash`
**Expected:**
1. Open http://localhost:1337
2. Signal view loads with module cards visible
3. Click any module card — `#void-view` becomes visible (loses `hidden` class), `#void-nodes` contains `.void-node` divs, each with `data-module` attribute
4. Module nodes appear at three horizontal columns: leftmost at ~15% width (Interface layer), center at ~50% (Core), rightmost at ~85% (Data)
5. `#void-connections` SVG contains `<path>` elements with `fill="none"` and colored strokes between module center points
6. Small `<circle>` elements visible in SVG traveling along path curves (flow particles)
7. Drag a node — it repositions; on mouseup, connections redraw with updated endpoint positions
8. Reload page — dragged node reappears at saved position (localStorage key `ariadne_void_pos_{moduleName}` persists)
9. Click "Reset" button — all nodes return to computed layer positions; localStorage keys for all modules are removed
10. Click "Risk" HUD button — node glow colors shift (high-risk modules glow red `rgba(248,113,113,...)`)
11. Click "Coupling" HUD button — heavily coupled modules glow red; isolated modules glow green
12. Click "Architecture" button — node glows return to health-based colors
13. Click a module node (without dragging) — node receives `void-node--selected` class; `DetailPanel` is NOT opened by `selectModule` (detail panel only opens when a symbolId is provided via `Void.show()` or `App.drillDown()`)
14. Click browser back or Signal nav — `Void.hide()` runs, `#void-view` gets `hidden` class, `#void-nodes` is empty

## Acceptance Criteria

- [ ] `void-renderer.js` implements: `init`, `show`, `hide`, `createNodes`, `autoLayout`, `loadSavedPositions`, `savePosition`, `resetLayout`, `drawConnections`, `createAmbientBackground`, `enableDrag`, `selectModule`, `setMode`, `animateFlowParticles`
- [ ] Auto-layout classifies modules into Interface (0 incoming), Data (0 outgoing), Core (both) layers
- [ ] Node positions are saved individually: `localStorage.getItem('ariadne_void_pos_pipeline')` returns `{"x":number,"y":number}` after positioning
- [ ] Node positions survive a page reload: after dragging a node and reloading, the node reappears at its dragged position
- [ ] `resetLayout()` removes all `ariadne_void_pos_{name}` localStorage keys and recomputes positions
- [ ] Flow particles: `<circle>` elements visible inside `#void-connections` SVG, moving along path curves
- [ ] SVG paths have `fill="none"` — no black fill over connections
- [ ] All innerHTML interpolation uses `esc()`
- [ ] `cargo build` — compiles
- [ ] `cargo test` — ALL PASS

## Types and Signatures

No Rust types. JavaScript class:

```javascript
class Void {
    // Public API
    static async init()
        // Fetch /api/modules and /api/coupling?limit=20; populate Void._modules, Void._coupling

    static async show(focusModule, focusSymbol)
        // focusModule: string|null — module name to select on show
        // focusSymbol: number|null — symbol id passed to DetailPanel.open() if set

    static hide()
        // Cancel particles, fade out #void-view, clear #void-nodes and #void-connections

    static createNodes(modules)
        // modules: ModuleSummary[] — from /api/modules response

    static autoLayout(modules)
        // modules: ModuleSummary[] — classifies into Interface/Core/Data, positions nodes

    static loadSavedPositions()
        // Reads ariadne_void_pos_{name} keys from localStorage into Void._positions

    static savePosition(moduleName, x, y)
        // moduleName: string, x: number, y: number — writes ariadne_void_pos_{moduleName}

    static resetLayout()
        // Clears all ariadne_void_pos_{name} keys; recomputes layout

    static drawConnections(modules, coupling)
        // modules: ModuleSummary[], coupling: CouplingPairSummary[]
        // Draws SVG bezier curves; restarts flow particles

    static createAmbientBackground()
        // Creates 3 gradient orbs in #void-ambient

    static enableDrag(nodeEl, moduleName)
        // nodeEl: HTMLElement — the .void-node div
        // moduleName: string — key for savePosition

    static selectModule(moduleName)
        // moduleName: string — highlights node, scrolls into view; does NOT call DetailPanel.open() (no symbol ID available)

    static setMode(mode)
        // mode: 'architecture'|'risk'|'coupling' — recolors node glows

    static animateFlowParticles()
        // Spawns one <circle> per SVG path; animates via requestAnimationFrame + getPointAtLength()

    // Private helpers
    static _healthColor(healthPct)
        // healthPct: number (0-100) — returns rgba glow color string

    static _classifyLayer(moduleName, coupling)
        // moduleName: string, coupling: CouplingPairSummary[] — returns 'interface'|'core'|'data'

    static _setNodePosition(name, x, y)
        // name: string, x: number, y: number — sets el.style.left / el.style.top

    static _getNodeCenter(name)
        // name: string — returns {x, y} center relative to parent, or null

    static _renderLayerLabels()
        // Writes hardcoded Interface/Core/Data labels into #void-layers
}
```

## Imports

No imports — vanilla JS. References `esc()` from `index.html` (globally available). References `DetailPanel` from `detail-panel.js` (PRD-05, globally available; guarded with `typeof DetailPanel !== 'undefined'` check). Does not reference `App.drillDown()` directly (called by Signal, not Void).

## Completion Contract

**Tests that must pass before signaling done:**
- `cargo test` — exit 0
- `cargo clippy -- -D warnings` — exit 0
- `cargo build` — exit 0

**Files this mini PRD is permitted to touch:**
- `/Users/rembrandt/loremllc/ariadne/src/dashboard/static/void-renderer.js`

**Completion signal:**
PLANFORGE_COMPLETE: PRD-04 Void renderer with spatial architecture map, flow particles, and node interactions
