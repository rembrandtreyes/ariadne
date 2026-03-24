/* ═══════════════════════════════════════════════════════════
   CANVAS-BASED GRAPH RENDERER (Phase 2)
   Replaces SVG rendering with Canvas 2D for performance.
   Web Worker handles force simulation off main thread.
   ═══════════════════════════════════════════════════════════ */
class GraphRenderer {
    constructor() {
        this.container = document.getElementById('graph-container');
        this.tooltip = document.getElementById('tooltip');
        this.canvas = null;
        this.ctx = null;
        this.data = null;
        this.transform = d3.zoomIdentity;
        this.zoomBehavior = null;
        this.quadtree = null;
        this.hoveredNode = null;
        this.worker = null;
        this.simulation = null; // Fallback main-thread simulation
        this.animating = false;
        this.dirColorScale = d3.scaleOrdinal(d3.schemeTableau10);
        this.colors = {};
        this.visibleNodes = [];
        this.visibleEdges = [];
        this.dagEdges = [];
        this.backEdges = [];
        this.nodeMap = {};
        this.labelThreshold = 3;
        this.d3dag = null;
        this._dragNode = null;
        this._dragOffset = null;
        this._isDragging = false;
        this._mouseDownPos = null;
        this._dpr = window.devicePixelRatio || 1;
    }

    resolveColors() {
        const cs = getComputedStyle(document.documentElement);
        this.colors = {
            nodeFunction: cs.getPropertyValue('--node-function').trim(),
            nodeClass: cs.getPropertyValue('--node-class').trim(),
            nodeMethod: cs.getPropertyValue('--node-method').trim(),
            nodeInterface: cs.getPropertyValue('--node-interface').trim(),
            nodeOther: cs.getPropertyValue('--node-other').trim(),
            edgeColor: cs.getPropertyValue('--edge-color').trim(),
            edgeHover: cs.getPropertyValue('--edge-hover').trim(),
            warningColor: cs.getPropertyValue('--warning-color').trim(),
            textMuted: cs.getPropertyValue('--text-muted').trim(),
            accentPrimary: cs.getPropertyValue('--accent-primary').trim(),
            bgVoid: cs.getPropertyValue('--bg-void').trim(),
            fontBody: cs.getPropertyValue('--font-body').trim(),
        };
    }

    async init() {
        // Load d3-dag for Sugiyama hierarchical layout
        try {
            this.d3dag = await import('https://esm.sh/d3-dag@1');
        } catch (e) {
            console.warn('d3-dag unavailable, hierarchy will use fallback layout');
            this.d3dag = null;
        }

        const [graphData, statsData, insights] = await Promise.all([
            fetch('/api/graph').then(r => r.json()).catch(() => ({ nodes: [], edges: [] })),
            fetch('/api/stats').then(r => r.json()).catch(() => ({})),
            fetch('/api/graph/insights').then(r => r.json()).catch(() => null)
        ]);

        allGraphData = graphData;
        insightsData = insights;
        this.updateStats(statsData);
        if (insights) renderInsights(insights);

        this.resolveColors();

        const w = this.container.clientWidth;
        const h = this.container.clientHeight;

        // Create Canvas element
        this.canvas = document.createElement('canvas');
        this.canvas.width = w * this._dpr;
        this.canvas.height = h * this._dpr;
        this.canvas.style.width = w + 'px';
        this.canvas.style.height = h + 'px';
        this.ctx = this.canvas.getContext('2d');
        this.ctx.scale(this._dpr, this._dpr);
        this.container.appendChild(this.canvas);

        // Set up zoom on canvas
        this.zoomBehavior = d3.zoom()
            .scaleExtent([0.05, 10])
            .filter((e) => {
                // Disable zoom panning when dragging a node
                if (this._dragNode) return false;
                // Allow wheel zoom always, only filter mousedown
                if (e.type === 'mousedown') {
                    const node = this.findNodeAt(e.offsetX, e.offsetY);
                    if (node) {
                        return false; // Let drag handler take this
                    }
                }
                return !e.ctrlKey && !e.button;
            })
            .on('zoom', (e) => {
                this.transform = e.transform;
                this.draw();
            });
        d3.select(this.canvas).call(this.zoomBehavior);

        // Set up mouse interactions on canvas
        this.setupCanvasInteractions();

        // Handle window resize
        window.addEventListener('resize', () => {
            const rw = this.container.clientWidth;
            const rh = this.container.clientHeight;
            this.canvas.width = rw * this._dpr;
            this.canvas.height = rh * this._dpr;
            this.canvas.style.width = rw + 'px';
            this.canvas.style.height = rh + 'px';
            this.ctx = this.canvas.getContext('2d');
            this.ctx.scale(this._dpr, this._dpr);
            this.draw();
        });

        if (!graphData.nodes || graphData.nodes.length === 0) {
            document.querySelector('.stats-bar').style.display = 'none';
            const empty = document.createElement('div');
            empty.className = 'empty-state';
            empty.innerHTML = '<div class="empty-state-text">No graph data yet</div><div class="empty-state-cmd">ariadne index .</div>';
            this.container.appendChild(empty);
            document.body.classList.add('loaded');
            return;
        }

        // Compute in-degree, out-degree, and directory for each node
        graphData.nodes.forEach(n => {
            n.inDegree = n.in_degree || 0;
            n.outDegree = n.out_degree || 0;
            n.degree = n.inDegree + n.outDegree;
            n.dir = n.file ? n.file.split('/').slice(0, -1).join('/') || '/' : '/';
        });

        this.data = graphData;

        // Find the most-connected node for default ego graph
        const mostConnected = graphData.nodes.reduce((best, n) => n.degree > best.degree ? n : best, graphData.nodes[0]);
        filterState.focusNodeId = mostConnected.id;

        // Initialize Web Worker for force simulation
        this.initWorker();

        this.render();
        document.body.classList.add('loaded');
    }

    initWorker() {
        try {
            const workerCode = `
                importScripts('https://d3js.org/d3.v7.min.js');
                var sim = null;
                var nodes = [];
                self.onmessage = function(e) {
                    var msg = e.data;
                    if (msg.type === 'start') {
                        nodes = msg.nodes;
                        var edges = msg.edges;
                        if (sim) sim.stop();
                        // Deterministic seeded initial positions
                        nodes.forEach(function(n, i) {
                            if (n.x === undefined || n.x === null || isNaN(n.x)) n.x = msg.width / 2 + Math.cos(i * 2.399) * Math.min(msg.width, msg.height) * 0.3;
                            if (n.y === undefined || n.y === null || isNaN(n.y)) n.y = msg.height / 2 + Math.sin(i * 2.399) * Math.min(msg.width, msg.height) * 0.3;
                        });
                        sim = d3.forceSimulation(nodes)
                            .force('link', d3.forceLink(edges).id(function(d) { return d.id; }).distance(msg.linkDistance || 70))
                            .force('charge', d3.forceManyBody().strength(msg.chargeStrength || -90))
                            .force('center', d3.forceCenter(msg.width / 2, msg.height / 2))
                            .force('collision', d3.forceCollide().radius(msg.collisionRadius || 8))
                            .alphaDecay(0.028);
                        sim.on('tick', function() {
                            var positions = new Float64Array(nodes.length * 2);
                            for (var i = 0; i < nodes.length; i++) {
                                positions[i * 2] = nodes[i].x;
                                positions[i * 2 + 1] = nodes[i].y;
                            }
                            self.postMessage({ type: 'tick', positions: positions }, [positions.buffer]);
                        });
                        sim.on('end', function() {
                            self.postMessage({ type: 'end' });
                        });
                    } else if (msg.type === 'stop') {
                        if (sim) sim.stop();
                    } else if (msg.type === 'reheat') {
                        if (sim) sim.alphaTarget(0.3).restart();
                    } else if (msg.type === 'cool') {
                        if (sim) sim.alphaTarget(0);
                    } else if (msg.type === 'pin') {
                        var pn = nodes.find(function(n) { return n.id === msg.nodeId; });
                        if (pn && sim) {
                            pn.fx = msg.x;
                            pn.fy = msg.y;
                            sim.alphaTarget(0.3).restart();
                        }
                    } else if (msg.type === 'unpin') {
                        var un = nodes.find(function(n) { return n.id === msg.nodeId; });
                        if (un) {
                            un.fx = null;
                            un.fy = null;
                            if (sim) sim.alphaTarget(0);
                        }
                    }
                };
            `;
            const blob = new Blob([workerCode], { type: 'application/javascript' });
            this.worker = new Worker(URL.createObjectURL(blob));

            this.worker.onmessage = (e) => {
                const msg = e.data;
                if (msg.type === 'tick') {
                    const positions = msg.positions;
                    for (let i = 0; i < this.visibleNodes.length; i++) {
                        this.visibleNodes[i].x = positions[i * 2];
                        this.visibleNodes[i].y = positions[i * 2 + 1];
                    }
                    this.updateQuadtree();
                    this.draw();
                } else if (msg.type === 'end') {
                    this.animating = false;
                }
            };

            this.worker.onerror = (e) => {
                console.warn('Web Worker error, falling back to main thread:', e.message);
                this.worker = null;
            };
        } catch (e) {
            console.warn('Web Worker unavailable, using main thread simulation');
            this.worker = null;
        }
    }

    updateQuadtree() {
        this.quadtree = d3.quadtree()
            .x(d => d.x)
            .y(d => d.y)
            .addAll(this.visibleNodes.filter(n => n.x !== undefined && !isNaN(n.x)));
    }

    setupCanvasInteractions() {
        const canvas = this.canvas;
        const self = this;
        const DRAG_THRESHOLD = 4;

        canvas.addEventListener('mousedown', (e) => {
            if (e.button !== 0) return;
            const node = self.findNodeAt(e.offsetX, e.offsetY);
            if (node) {
                self._mouseDownPos = { x: e.offsetX, y: e.offsetY };
                self._dragNode = node;
                self._isDragging = false;
            }
        });

        canvas.addEventListener('mousemove', (e) => {
            // Handle active drag
            if (self._dragNode && self._mouseDownPos) {
                const dx = e.offsetX - self._mouseDownPos.x;
                const dy = e.offsetY - self._mouseDownPos.y;
                if (!self._isDragging && (dx * dx + dy * dy) > DRAG_THRESHOLD * DRAG_THRESHOLD) {
                    self._isDragging = true;
                    if (filterState.layout === 'network' && self.worker) {
                        self.worker.postMessage({ type: 'reheat' });
                    }
                }
                if (self._isDragging) {
                    const [gx, gy] = self.screenToGraph(e.offsetX, e.offsetY);
                    self._dragNode.x = gx;
                    self._dragNode.y = gy;
                    if (filterState.layout === 'network' && self.worker) {
                        self.worker.postMessage({ type: 'pin', nodeId: self._dragNode.id, x: gx, y: gy });
                    } else {
                        self.updateQuadtree();
                        self.draw();
                    }
                    return;
                }
            }

            // Hover detection
            const node = self.findNodeAt(e.offsetX, e.offsetY);
            if (node !== self.hoveredNode) {
                self.hoveredNode = node;
                canvas.style.cursor = node ? 'pointer' : 'default';
                self.draw();
            }
            if (node) {
                self.tooltip.innerHTML = '<div class="tt-name">' + esc(node.name) + (node.is_dead ? ' <span style="color:var(--warning-color);font-size:0.7rem;font-weight:normal">⊘ dead</span>' : '') + '</div><div class="tt-detail">' + esc(node.kind) + ' &middot; ' + esc(node.file) + '</div><div class="tt-detail">In: ' + node.inDegree + ' &middot; Out: ' + node.outDegree + ' &middot; Total: ' + node.degree + '</div>';
                self.tooltip.style.display = 'block';
                self.tooltip.style.left = (e.clientX + 12) + 'px';
                self.tooltip.style.top = (e.clientY - 10) + 'px';
            } else {
                self.tooltip.style.display = 'none';
            }
        });

        canvas.addEventListener('mouseup', (e) => {
            if (self._dragNode) {
                if (self._isDragging) {
                    // End drag
                    if (filterState.layout === 'network' && self.worker) {
                        self.worker.postMessage({ type: 'unpin', nodeId: self._dragNode.id });
                        self.worker.postMessage({ type: 'cool' });
                    }
                } else {
                    // It was a click (no drag)
                    const node = self._dragNode;
                    filterState.focusNodeId = node.id;
                    filterState.showAll = false;
                    document.getElementById('showAllBtn').classList.remove('active');
                    self.render();
                    self.panToNode(node.id);
                }
                self._dragNode = null;
                self._mouseDownPos = null;
                self._isDragging = false;
            }
        });

        canvas.addEventListener('contextmenu', (e) => {
            const node = self.findNodeAt(e.offsetX, e.offsetY);
            if (node) {
                e.preventDefault();
                e.stopPropagation();
                showContextMenu(e.clientX, e.clientY, node);
            }
        });

        canvas.addEventListener('mouseleave', () => {
            self.hoveredNode = null;
            self.tooltip.style.display = 'none';
            if (self._dragNode) {
                if (self._isDragging && filterState.layout === 'network' && self.worker) {
                    self.worker.postMessage({ type: 'unpin', nodeId: self._dragNode.id });
                    self.worker.postMessage({ type: 'cool' });
                }
                self._dragNode = null;
                self._mouseDownPos = null;
                self._isDragging = false;
            }
            self.draw();
        });
    }

    screenToGraph(sx, sy) {
        return this.transform.invert([sx, sy]);
    }

    findNodeAt(sx, sy) {
        if (!this.quadtree) return null;
        const [gx, gy] = this.screenToGraph(sx, sy);
        const radius = 20 / this.transform.k;
        return this.quadtree.find(gx, gy, radius) || null;
    }

    render() {
        if (!this.data) return;

        // Stop any running simulation
        if (this.worker) this.worker.postMessage({ type: 'stop' });
        if (this.simulation) { this.simulation.stop(); this.simulation = null; }
        this.animating = false;

        // Apply filters to get visible nodes and edges
        const { visibleNodes, visibleEdges } = this.applyFilters();
        this.visibleNodes = visibleNodes;
        this.visibleEdges = visibleEdges;

        if (visibleNodes.length === 0) {
            this.quadtree = null;
            this.dagEdges = [];
            this.backEdges = [];
            this.nodeMap = {};
            this.draw();
            return;
        }

        this.nodeMap = {};
        visibleNodes.forEach(n => { this.nodeMap[n.id] = n; });

        this.labelThreshold = Math.max(3, d3.quantile(visibleNodes.map(n => n.degree).sort(d3.ascending), 0.8) || 3);

        const w = this.container.clientWidth;
        const h = this.container.clientHeight;

        if (filterState.layout === 'hierarchy') {
            this.layoutHierarchy(visibleNodes, visibleEdges, w, h);
        } else {
            this.layoutForce(visibleNodes, visibleEdges, w, h);
        }
    }

    layoutForce(nodes, edges, w, h) {
        this.animating = true;

        // Deterministic initial positions using golden angle
        nodes.forEach((n, i) => {
            if (n.x === undefined || isNaN(n.x)) n.x = w / 2 + Math.cos(i * 2.399) * Math.min(w, h) * 0.3;
            if (n.y === undefined || isNaN(n.y)) n.y = h / 2 + Math.sin(i * 2.399) * Math.min(w, h) * 0.3;
        });

        // Force mode: all edges are drawn the same (no cycle distinction needed for visual)
        this.dagEdges = edges;
        this.backEdges = [];

        this.updateQuadtree();

        const chargeStrength = nodes.length > 500 ? -30 : -90;

        if (this.worker) {
            // Send to Web Worker
            this.worker.postMessage({
                type: 'start',
                nodes: nodes.map(n => ({ id: n.id, x: n.x, y: n.y })),
                edges: edges.map(e => ({ source: e.source, target: e.target })),
                width: w,
                height: h,
                chargeStrength: chargeStrength,
                linkDistance: 70,
                collisionRadius: 8
            });
        } else {
            // Fallback: main thread simulation
            this.simulation = d3.forceSimulation(nodes)
                .force('link', d3.forceLink(edges).id(d => d.id).distance(70))
                .force('charge', d3.forceManyBody().strength(chargeStrength))
                .force('center', d3.forceCenter(w / 2, h / 2))
                .force('collision', d3.forceCollide().radius(8));

            this.simulation.on('tick', () => {
                this.updateQuadtree();
                this.draw();
            });
        }
    }

    layoutHierarchy(nodes, edges, w, h) {
        const { dagEdges, backEdges } = this.breakCycles(nodes, edges);
        this.dagEdges = dagEdges;
        this.backEdges = backEdges;

        let usedSugiyama = false;
        if (this.d3dag && nodes.length > 0) {
            try {
                const { graphStratify, sugiyama } = this.d3dag;

                const pMap = {};
                nodes.forEach(n => { pMap[String(n.id)] = new Set(); });
                dagEdges.forEach(e => {
                    const tid = String(e.target), sid = String(e.source);
                    if (pMap[tid]) pMap[tid].add(sid);
                });

                const stratData = nodes.map(n => ({
                    id: String(n.id),
                    parentIds: [...(pMap[String(n.id)] || [])]
                }));

                const dag = graphStratify()(stratData);
                const { width: dw, height: dh } = sugiyama()(dag);

                const margin = 60;
                const sx = dw > 0 ? (w - margin * 2) / dw : 1;
                const sy = dh > 0 ? (h - margin * 2) / dh : 1;

                for (const dn of dag.nodes()) {
                    const orig = this.nodeMap[dn.data.id];
                    if (orig) {
                        orig.x = margin + dn.x * sx;
                        orig.y = margin + dn.y * sy;
                    }
                }
                usedSugiyama = true;
            } catch (e) {
                console.warn('d3-dag Sugiyama layout failed, using fallback:', e);
            }
        }

        // Fallback: Kahn's topological sort layering
        if (!usedSugiyama) {
            const adj = {};
            const inCount = {};
            nodes.forEach(n => { adj[n.id] = []; inCount[n.id] = 0; });
            dagEdges.forEach(e => {
                if (adj[e.source] && inCount[e.target] !== undefined) {
                    adj[e.source].push(e.target);
                    inCount[e.target]++;
                }
            });
            const queue = [];
            const layers = {};
            Object.entries(inCount).forEach(([id, count]) => {
                if (count === 0) { queue.push(id); layers[id] = 0; }
            });
            let maxLayer = 0;
            while (queue.length > 0) {
                const nid = queue.shift();
                (adj[nid] || []).forEach(child => {
                    inCount[child]--;
                    const nextLayer = (layers[nid] || 0) + 1;
                    layers[child] = Math.max(layers[child] || 0, nextLayer);
                    maxLayer = Math.max(maxLayer, layers[child]);
                    if (inCount[child] === 0) queue.push(child);
                });
            }
            nodes.forEach(n => { if (layers[n.id] === undefined) layers[n.id] = 0; });

            const layerNodes = {};
            nodes.forEach(n => {
                const l = layers[n.id];
                if (!layerNodes[l]) layerNodes[l] = [];
                layerNodes[l].push(n);
            });
            const layerHeight = h / (maxLayer + 2);
            Object.entries(layerNodes).forEach(([layer, lnodes]) => {
                const layerWidth = w / (lnodes.length + 1);
                lnodes.forEach((n, i) => {
                    n.x = layerWidth * (i + 1);
                    n.y = layerHeight * (parseInt(layer) + 1);
                });
            });
        }

        this.updateQuadtree();
        this.draw();
    }

    draw() {
        const ctx = this.ctx;
        const dpr = this._dpr;
        const w = parseInt(this.canvas.style.width);
        const h = parseInt(this.canvas.style.height);

        // Clear
        ctx.save();
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.clearRect(0, 0, w, h);

        // Apply zoom transform
        ctx.translate(this.transform.x, this.transform.y);
        ctx.scale(this.transform.k, this.transform.k);

        const isHierarchy = filterState.layout === 'hierarchy';
        const nodeMap = this.nodeMap;

        // ── Semantic zoom: collapse to file-level view when zoomed way out ──
        if (this.transform.k < 0.25 && this.visibleNodes.length > 0) {
            // Build file-level aggregates
            const fileGroups = {};
            this.visibleNodes.forEach(n => {
                const f = n.file || '(unknown)';
                if (!fileGroups[f]) fileGroups[f] = { file: f, nodes: [], x: 0, y: 0 };
                fileGroups[f].nodes.push(n);
            });
            Object.values(fileGroups).forEach(g => {
                const valid = g.nodes.filter(n => n.x !== undefined && !isNaN(n.x));
                if (valid.length === 0) return;
                g.x = valid.reduce((s, n) => s + n.x, 0) / valid.length;
                g.y = valid.reduce((s, n) => s + n.y, 0) / valid.length;
            });
            const groups = Object.values(fileGroups).filter(g => g.x !== 0 || g.y !== 0);
            // Draw inter-file edges
            const edgeSeen = new Set();
            this.visibleEdges.forEach(e => {
                const s = nodeMap[e.source], t = nodeMap[e.target];
                if (!s || !t) return;
                const sf = s.file || '(unknown)', tf = t.file || '(unknown)';
                if (sf === tf) return;
                const key = sf < tf ? sf + '||' + tf : tf + '||' + sf;
                if (edgeSeen.has(key)) return;
                edgeSeen.add(key);
                const sg = fileGroups[sf], tg = fileGroups[tf];
                if (!sg || !tg) return;
                ctx.strokeStyle = this.colors.edgeColor;
                ctx.globalAlpha = 0.4;
                ctx.lineWidth = 1;
                ctx.setLineDash([]);
                ctx.beginPath();
                ctx.moveTo(sg.x, sg.y);
                ctx.lineTo(tg.x, tg.y);
                ctx.stroke();
            });
            // Draw file bubbles
            ctx.globalAlpha = 1.0;
            groups.forEach(g => {
                const r = Math.max(12, Math.min(40, g.nodes.length * 2));
                ctx.beginPath();
                ctx.arc(g.x, g.y, r, 0, Math.PI * 2);
                ctx.fillStyle = this.colors.nodeOther || '#6B7080';
                ctx.globalAlpha = 0.7;
                ctx.fill();
                ctx.strokeStyle = this.colors.accentPrimary;
                ctx.lineWidth = 1;
                ctx.globalAlpha = 0.5;
                ctx.stroke();
                // Label
                ctx.globalAlpha = 0.9;
                ctx.fillStyle = this.colors.textMuted;
                ctx.font = '10px ' + (this.colors.fontBody || 'monospace');
                ctx.textBaseline = 'middle';
                const shortName = g.file.split('/').pop() || g.file;
                ctx.fillText(shortName, g.x + r + 4, g.y);
                // Count
                ctx.fillStyle = this.colors.accentPrimary;
                ctx.font = 'bold 9px ' + (this.colors.fontBody || 'monospace');
                ctx.textBaseline = 'middle';
                ctx.textAlign = 'center';
                ctx.fillText(g.nodes.length, g.x, g.y);
                ctx.textAlign = 'left';
                ctx.globalAlpha = 1.0;
            });
            ctx.restore();
            return;
        }

        // ── Draw edges ──
        if (isHierarchy) {
            // DAG edges with arrowheads (cubic bezier)
            this.dagEdges.forEach(e => {
                const s = nodeMap[e.source], t = nodeMap[e.target];
                if (!s || !t || s.x === undefined || t.x === undefined) return;
                ctx.strokeStyle = this.colors.edgeColor;
                ctx.globalAlpha = 0.6;
                ctx.lineWidth = Math.max(0.5, (e.confidence || 0.5) * 2);
                ctx.setLineDash([]);
                ctx.beginPath();
                const midY = (s.y + t.y) / 2;
                ctx.moveTo(s.x, s.y);
                ctx.bezierCurveTo(s.x, midY, t.x, midY, t.x, t.y);
                ctx.stroke();
                this.drawArrowhead(ctx, s.x, s.y, t.x, t.y, this.colors.edgeColor, this.nodeRadius(t));
            });

            // Back-edges (circular dependencies) - dashed, warning color
            if (this.backEdges.length > 0) {
                this.backEdges.forEach(e => {
                    const s = nodeMap[e.source], t = nodeMap[e.target];
                    if (!s || !t || s.x === undefined || t.x === undefined) return;
                    ctx.strokeStyle = this.colors.warningColor;
                    ctx.globalAlpha = 0.7;
                    ctx.lineWidth = 1.5;
                    ctx.setLineDash([6, 3]);
                    ctx.beginPath();
                    const dx = t.x - s.x, dy = t.y - s.y;
                    const cpX = (s.x + t.x) / 2 - dy * 0.3;
                    const cpY = (s.y + t.y) / 2 + dx * 0.3;
                    ctx.moveTo(s.x, s.y);
                    ctx.quadraticCurveTo(cpX, cpY, t.x, t.y);
                    ctx.stroke();
                    this.drawArrowhead(ctx, cpX, cpY, t.x, t.y, this.colors.warningColor, this.nodeRadius(t));
                });
                ctx.setLineDash([]);
            }
        } else {
            // Force mode: subtle curved edges (with optional bundling)
            ctx.strokeStyle = this.colors.edgeColor;
            ctx.globalAlpha = 0.6;
            ctx.setLineDash([]);

            // Precompute file centroids for edge bundling
            let fileCentroids = null;
            if (filterState.bundleEdges) {
                const fileAccum = {};
                this.visibleNodes.forEach(n => {
                    if (n.x === undefined || isNaN(n.x)) return;
                    const f = n.file || '(unknown)';
                    if (!fileAccum[f]) fileAccum[f] = { sx: 0, sy: 0, count: 0 };
                    fileAccum[f].sx += n.x; fileAccum[f].sy += n.y; fileAccum[f].count++;
                });
                fileCentroids = {};
                Object.entries(fileAccum).forEach(([f, a]) => {
                    fileCentroids[f] = { x: a.sx / a.count, y: a.sy / a.count };
                });
            }

            this.visibleEdges.forEach(e => {
                const s = nodeMap[e.source], t = nodeMap[e.target];
                if (!s || !t || s.x === undefined || t.x === undefined || isNaN(s.x) || isNaN(t.x)) return;
                ctx.lineWidth = Math.max(0.5, (e.confidence || 0.5) * 2);
                ctx.beginPath();
                if (filterState.bundleEdges && fileCentroids && s.file && t.file && s.file !== t.file) {
                    // Bundle: route edge through the midpoint between the two file centroids
                    const sc = fileCentroids[s.file || '(unknown)'];
                    const tc = fileCentroids[t.file || '(unknown)'];
                    if (sc && tc) {
                        const cpX = (sc.x + tc.x) / 2;
                        const cpY = (sc.y + tc.y) / 2;
                        ctx.moveTo(s.x, s.y);
                        ctx.quadraticCurveTo(cpX, cpY, t.x, t.y);
                    } else {
                        const dx = t.x - s.x, dy = t.y - s.y;
                        ctx.moveTo(s.x, s.y);
                        ctx.quadraticCurveTo((s.x + t.x) / 2 + dy * 0.08, (s.y + t.y) / 2 - dx * 0.08, t.x, t.y);
                    }
                } else {
                    const dx = t.x - s.x, dy = t.y - s.y;
                    const midX = (s.x + t.x) / 2 + dy * 0.08;
                    const midY = (s.y + t.y) / 2 - dx * 0.08;
                    ctx.moveTo(s.x, s.y);
                    ctx.quadraticCurveTo(midX, midY, t.x, t.y);
                }
                ctx.stroke();
            });
        }

        ctx.globalAlpha = 1.0;

        // ── Draw nodes ──
        this.visibleNodes.forEach(n => {
            if (n.x === undefined || n.y === undefined || isNaN(n.x)) return;
            const r = this.nodeRadius(n);
            const isHovered = n === this.hoveredNode;
            const isFocus = n.id === filterState.focusNodeId;
            const drawR = isHovered ? r * 1.5 : r;
            const color = this.nodeColor(n);

            // Glow effect on hover
            if (isHovered) {
                ctx.save();
                ctx.shadowColor = color;
                ctx.shadowBlur = 12;
                ctx.globalAlpha = n.is_dead ? 0.6 : 1.0;
                ctx.beginPath();
                ctx.arc(n.x, n.y, drawR, 0, Math.PI * 2);
                ctx.fillStyle = color;
                ctx.fill();
                ctx.restore();
            } else {
                ctx.globalAlpha = n.is_dead ? 0.45 : 1.0;
                ctx.beginPath();
                ctx.arc(n.x, n.y, drawR, 0, Math.PI * 2);
                ctx.fillStyle = color;
                ctx.fill();
                ctx.globalAlpha = 1.0;
            }

            // Dead code: dashed ring indicator
            if (n.is_dead) {
                ctx.save();
                ctx.setLineDash([2, 2]);
                ctx.beginPath();
                ctx.arc(n.x, n.y, drawR + 2, 0, Math.PI * 2);
                ctx.strokeStyle = this.colors.warningColor || '#C45B3E';
                ctx.lineWidth = 1;
                ctx.globalAlpha = 0.7;
                ctx.stroke();
                ctx.restore();
            }

            // Focus ring
            if (isFocus) {
                ctx.beginPath();
                ctx.arc(n.x, n.y, drawR + 1, 0, Math.PI * 2);
                ctx.strokeStyle = this.colors.accentPrimary;
                ctx.lineWidth = 2;
                ctx.stroke();
            }
        });

        // ── Draw labels ──
        ctx.fillStyle = this.colors.textMuted;
        ctx.font = '9px ' + (this.colors.fontBody || 'monospace');
        ctx.textBaseline = 'middle';
        this.visibleNodes.forEach(n => {
            if (n.x === undefined || n.y === undefined || isNaN(n.x)) return;
            if (n.degree >= this.labelThreshold || n.id === filterState.focusNodeId) {
                const r = this.nodeRadius(n);
                ctx.globalAlpha = n.id === filterState.focusNodeId ? 0.9 : 0.7;
                ctx.fillText(n.name, n.x + r + 4, n.y);
            }
        });
        ctx.globalAlpha = 1.0;

        ctx.restore();
    }

    drawArrowhead(ctx, sx, sy, tx, ty, color, targetRadius) {
        const angle = Math.atan2(ty - sy, tx - sx);
        const r = targetRadius || 5;
        const headLen = 6;
        const endX = tx - Math.cos(angle) * r;
        const endY = ty - Math.sin(angle) * r;

        ctx.fillStyle = color;
        ctx.beginPath();
        ctx.moveTo(endX, endY);
        ctx.lineTo(endX - headLen * Math.cos(angle - Math.PI / 6), endY - headLen * Math.sin(angle - Math.PI / 6));
        ctx.lineTo(endX - headLen * Math.cos(angle + Math.PI / 6), endY - headLen * Math.sin(angle + Math.PI / 6));
        ctx.closePath();
        ctx.fill();
    }

    applyFilters() {
        const data = this.data;
        let nodes = data.nodes;
        let edges = data.edges;

        // Ego graph: BFS from focus node
        if (!filterState.showAll && filterState.focusNodeId) {
            const reachable = new Set();
            const queue = [[filterState.focusNodeId, 0]];
            reachable.add(filterState.focusNodeId);

            const adj = {};
            edges.forEach(e => {
                const s = typeof e.source === 'object' ? e.source.id : e.source;
                const t = typeof e.target === 'object' ? e.target.id : e.target;
                if (!adj[s]) adj[s] = [];
                if (!adj[t]) adj[t] = [];
                adj[s].push(t);
                adj[t].push(s);
            });

            while (queue.length > 0) {
                const [nid, depth] = queue.shift();
                if (depth >= filterState.depth) continue;
                (adj[nid] || []).forEach(neighbor => {
                    if (!reachable.has(neighbor)) {
                        reachable.add(neighbor);
                        queue.push([neighbor, depth + 1]);
                    }
                });
            }

            nodes = nodes.filter(n => reachable.has(n.id));
        }

        // Filter by node type
        nodes = nodes.filter(n => {
            const kind = n.kind === 'function' || n.kind === 'class' || n.kind === 'method' || n.kind === 'interface' ? n.kind : 'other';
            return filterState.nodeTypes.has(kind);
        });

        // Filter by min connections
        if (filterState.minConnections > 0) {
            nodes = nodes.filter(n => n.degree >= filterState.minConnections);
        }

        // Filter by name
        if (filterState.nameFilter) {
            const q = filterState.nameFilter.toLowerCase();
            nodes = nodes.filter(n => n.name.toLowerCase().includes(q) || n.file.toLowerCase().includes(q));
        }

        // Filter orphans
        if (!filterState.showOrphans) {
            const nodeIds = new Set(nodes.map(n => n.id));
            const connected = new Set();
            edges.forEach(e => {
                const s = typeof e.source === 'object' ? e.source.id : e.source;
                const t = typeof e.target === 'object' ? e.target.id : e.target;
                if (nodeIds.has(s) && nodeIds.has(t)) { connected.add(s); connected.add(t); }
            });
            nodes = nodes.filter(n => connected.has(n.id) || n.id === filterState.focusNodeId);
        }

        // Filter edges to only include visible nodes
        const nodeIds = new Set(nodes.map(n => n.id));
        edges = edges.filter(e => {
            const s = typeof e.source === 'object' ? e.source.id : e.source;
            const t = typeof e.target === 'object' ? e.target.id : e.target;
            return nodeIds.has(s) && nodeIds.has(t);
        });

        // Deep copy nodes to avoid mutation
        const visibleNodes = nodes.map(n => ({...n}));
        const visibleEdges = edges.map(e => ({
            source: typeof e.source === 'object' ? e.source.id : e.source,
            target: typeof e.target === 'object' ? e.target.id : e.target,
            confidence: e.confidence
        }));

        return { visibleNodes, visibleEdges };
    }

    breakCycles(nodes, edges) {
        const WHITE = 0, GRAY = 1, BLACK = 2;
        const color = {};
        nodes.forEach(n => { color[n.id] = WHITE; });
        const adj = {};
        nodes.forEach(n => { adj[n.id] = []; });
        edges.forEach((e, i) => {
            if (adj[e.source]) adj[e.source].push({ target: e.target, idx: i });
        });
        const backIdx = new Set();
        const dfs = (u) => {
            color[u] = GRAY;
            for (const { target, idx } of (adj[u] || [])) {
                if (color[target] === GRAY) backIdx.add(idx);
                else if (color[target] === WHITE) dfs(target);
            }
            color[u] = BLACK;
        };
        nodes.forEach(n => { if (color[n.id] === WHITE) dfs(n.id); });
        return {
            dagEdges: edges.filter((_, i) => !backIdx.has(i)),
            backEdges: edges.filter((_, i) => backIdx.has(i))
        };
    }

    nodeRadius(d) {
        return Math.max(3, Math.min(14, 3 + Math.sqrt(d.inDegree || 0) * 2));
    }

    nodeColor(d) {
        if (d.is_dead) {
            return this.colors.warningColor || '#C45B3E';
        }
        if (filterState.colorByDir) {
            return this.dirColorScale(d.dir);
        }
        const map = {
            function: this.colors.nodeFunction,
            class: this.colors.nodeClass,
            method: this.colors.nodeMethod,
            interface: this.colors.nodeInterface
        };
        return map[d.kind] || this.colors.nodeOther;
    }

    updateColors() {
        this.resolveColors();
        this.draw();
    }

    updateStats(data) {
        document.getElementById('s-files').textContent = (data.files || 0).toLocaleString();
        document.getElementById('s-symbols').textContent = (data.symbols || 0).toLocaleString();
        document.getElementById('s-calls').textContent = (data.calls || 0).toLocaleString();
        document.getElementById('s-res').textContent = ((data.resolution_rate || 0) * 100).toFixed(0) + '%';
        document.getElementById('s-dead').textContent = (data.dead_functions || 0).toLocaleString();
        document.getElementById('s-langs').textContent = (data.languages || []).length;
    }

    panToNode(id) {
        const node = this.data.nodes.find(n => n.id === id);
        if (!node || !node.x) return;
        const w = this.container.clientWidth, h = this.container.clientHeight;
        d3.select(this.canvas).transition().duration(750).call(
            this.zoomBehavior.transform,
            d3.zoomIdentity.translate(w / 2 - node.x * 1.5, h / 2 - node.y * 1.5).scale(1.5)
        );
    }

    highlightNode(id) {
        filterState.focusNodeId = id;
        filterState.showAll = false;
        document.getElementById('showAllBtn').classList.remove('active');
        this.render();
        setTimeout(() => this.panToNode(id), 100);
    }
}
