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

            el.addEventListener('click', async () => {
                if (Void._dragState && Void._dragState.moved) return;
                Void.selectModule(m.name);
                // Find a representative symbol for this module and open the detail panel
                if (typeof DetailPanel === 'undefined') return;
                try {
                    const res = await fetch('/api/search?q=' + encodeURIComponent(m.name));
                    const results = await res.json();
                    if (Array.isArray(results) && results.length > 0) {
                        const ranked = results
                            .filter(r => !r.file.startsWith('tests/') && !r.name.startsWith('test_'))
                            .sort((a, b) => (b.in_degree + b.out_degree) - (a.in_degree + a.out_degree));
                        const best = ranked.length > 0 ? ranked[0] : results[0];
                        await DetailPanel.open(parseInt(best.id, 10));
                    }
                } catch (_) {}
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

        // When coupling data is absent or sparse, all modules fall into "interface"
        // (0 incoming for everyone). Redistribute evenly across all three layers.
        if (layers.interface.length === modules.length && modules.length > 3) {
            const third = Math.ceil(modules.length / 3);
            layers.interface = modules.slice(0, third);
            layers.core      = modules.slice(third, third * 2);
            layers.data      = modules.slice(third * 2);
        }

        const layerX = {
            interface: width * 0.15,
            core: width * 0.50,
            data: width * 0.85,
        };

        const NODE_H = 96; // approximate node card height in px
        const GAP    = 10;

        for (const [layerName, layerModules] of Object.entries(layers)) {
            if (layerModules.length === 0) continue;
            const x = layerX[layerName];

            // How many nodes can fit in one column without overlap?
            const maxPerCol = Math.max(1, Math.floor((height - 80) / (NODE_H + GAP)));
            const cols      = Math.ceil(layerModules.length / maxPerCol);
            const perCol    = Math.ceil(layerModules.length / cols);
            const spacing   = Math.max(NODE_H + GAP, (height - 80) / perCol);
            const startY    = Math.max(20, (height - perCol * spacing) / 2 + 40);

            for (let i = 0; i < layerModules.length; i++) {
                const m = layerModules[i];
                const saved = Void._positions[m.name];
                if (saved) {
                    Void._setNodePosition(m.name, saved.x, saved.y);
                    continue;
                }
                const col  = Math.floor(i / perCol);
                const row  = i % perCol;
                // Sub-columns are centred around the layer's x anchor
                const colOffset = cols > 1 ? (col - (cols - 1) / 2) * 110 : 0;
                const posX = x - 80 + colOffset;
                const posY = startY + row * spacing;
                Void._positions[m.name] = { x: posX, y: posY };
                Void._setNodePosition(m.name, posX, posY);
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
