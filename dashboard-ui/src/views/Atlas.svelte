<script lang="ts">
  import { loadAtlasData, type AtlasData, type Resource } from '../lib/api';
  import {
    computeLayout,
    visibleGraph,
    PALETTE_SIZE,
    type AtlasLayout,
    type VisibleGraph,
    type VisibleNode,
  } from '../lib/atlas/layout';
  import {
    createRenderer,
    readAtlasPalette,
    type Camera,
    type Renderer,
  } from '../lib/atlas/renderer';
  import { navigate } from '../lib/router';
  import { formatCount } from '../lib/format';
  import Skeleton from '../lib/components/Skeleton.svelte';
  import ErrorState from '../lib/components/ErrorState.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';

  let resource = $state<Resource<AtlasData>>({ status: 'loading' });
  let webglFailed = $state(false);
  let openCount = $state(0);
  let stageEl = $state<HTMLDivElement | null>(null);

  interface Tooltip {
    x: number;
    y: number;
    title: string;
    sub: string;
    hint: string;
  }
  let tooltip = $state<Tooltip | null>(null);

  async function load() {
    resource = { status: 'loading' };
    try {
      resource = { status: 'ready', data: await loadAtlasData() };
    } catch (e) {
      resource = { status: 'error', message: e instanceof Error ? e.message : String(e) };
    }
  }
  load();

  /* ── Imperative scene state (not reactive: the rAF loop owns redraws) ── */
  let canvasEl: HTMLCanvasElement | null = null;
  let labelLayer = $state<HTMLDivElement | null>(null);
  let renderer: Renderer | null = null;
  let layout: AtlasLayout | null = null;
  let visible: VisibleGraph | null = null;
  let data: AtlasData | null = null;
  const expanded = new Set<number>();
  const camera: Camera = { x: 0, y: 0, scale: 1 };
  let dirty = false;
  let raf = 0;
  let cssWidth = 0;
  let cssHeight = 0;
  let hoverKey: string | null = null;
  let labelEls = new Map<string, HTMLDivElement>();
  let fitBounds = { w: 1000, h: 1000 };
  let resizeObserver: ResizeObserver | null = null;

  /* Ariadne communities are call-graph components (zero cross-community
     edges by construction), so a fully-collapsed view can never show an
     edge. Default: open the largest community so first paint shows real
     directed structure. A bare #/atlas gets the default; `open=` (empty)
     means the user explicitly collapsed everything. */
  function restoreExpandedFromUrl() {
    const query = window.location.hash.split('?')[1] ?? '';
    const open = new URLSearchParams(query).get('open');
    expanded.clear();
    if (open === null) {
      const largest = layout?.groups.find((g) => g.key !== -1) ?? layout?.groups[0];
      if (largest) expanded.add(largest.key);
    } else if (open !== '') {
      for (const part of open.split(',')) {
        const id = Number(part);
        if (Number.isFinite(id)) expanded.add(id);
      }
    }
  }

  function syncUrl() {
    const open = [...expanded].sort((a, b) => a - b).join(',');
    history.replaceState(null, '', `#/atlas?open=${open}`);
  }

  function rebuild() {
    if (!data || !layout || !renderer) return;
    visible = visibleGraph(data, layout, expanded);
    renderer.setGraph(visible.nodes, visible.edges);
    hoverKey = null;
    tooltip = null;
    applyHoverStates();
    rebuildLabels();
    openCount = expanded.size;
    dirty = true;
  }

  function applyHoverStates() {
    if (!visible || !renderer) return;
    const nodeStates = new Float32Array(visible.nodes.length);
    const edgeStates = new Float32Array(visible.edges.length);
    if (hoverKey) {
      const neighbors = visible.adjacency.get(hoverKey) ?? new Set();
      visible.nodes.forEach((n, i) => {
        nodeStates[i] = n.key === hoverKey ? 1 : neighbors.has(n.key) ? 2 : 3;
      });
      visible.edges.forEach((e, i) => {
        edgeStates[i] = e.sourceKey === hoverKey || e.targetKey === hoverKey ? 2 : 3;
      });
    }
    renderer.setNodeStates(nodeStates);
    renderer.setEdgeStates(edgeStates);
  }

  /* ── Camera math: camera.scale is CSS px per world unit; the renderer
        multiplies by dpr. Mouse coordinates are CSS px in stage space. ── */
  function worldFromScreen(cssX: number, cssY: number): { x: number; y: number } {
    return {
      x: camera.x + (cssX - cssWidth / 2) / camera.scale,
      y: camera.y + (cssY - cssHeight / 2) / camera.scale,
    };
  }

  function fit() {
    if (!visible || visible.nodes.length === 0) return;
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const n of visible.nodes) {
      minX = Math.min(minX, n.x - n.radius);
      minY = Math.min(minY, n.y - n.radius);
      maxX = Math.max(maxX, n.x + n.radius);
      maxY = Math.max(maxY, n.y + n.radius);
    }
    const w = Math.max(maxX - minX, 1);
    const h = Math.max(maxY - minY, 1);
    fitBounds = { w, h };
    camera.x = (minX + maxX) / 2;
    camera.y = (minY + maxY) / 2;
    camera.scale = Math.min(cssWidth / w, cssHeight / h) * 0.92;
    camera.scale = Math.min(20, Math.max(0.005, camera.scale));
    dirty = true;
  }

  function collapseAll() {
    expanded.clear();
    syncUrl();
    rebuild();
  }

  /* ── Hit testing: linear scan, best = smallest signed surface distance. ── */
  function hitTest(cssX: number, cssY: number): VisibleNode | null {
    if (!visible) return null;
    const p = worldFromScreen(cssX, cssY);
    const slop = 4 / camera.scale;
    let best: VisibleNode | null = null;
    let bestScore = Infinity;
    for (const n of visible.nodes) {
      const d = Math.hypot(n.x - p.x, n.y - p.y) - n.radius;
      if (d < slop && d < bestScore) {
        best = n;
        bestScore = d;
      }
    }
    return best;
  }

  function setHover(node: VisibleNode | null, cssX: number, cssY: number) {
    const key = node?.key ?? null;
    if (key !== hoverKey) {
      hoverKey = key;
      applyHoverStates();
      dirty = true;
    }
    if (node) {
      tooltip = {
        x: Math.min(cssX + 14, cssWidth - 240),
        y: cssY + 14,
        title: node.label,
        sub:
          node.kind === 'community'
            ? `${formatCount(node.memberCount)} symbols`
            : `${node.node?.kind ?? ''} · ${node.node?.module ?? ''} · ${node.node?.in_degree ?? 0} in / ${node.node?.out_degree ?? 0} out${node.node?.is_dead ? ' · dead' : ''}`,
        hint: node.kind === 'community' ? 'click to expand' : 'click for dossier',
      };
    } else {
      tooltip = null;
    }
  }

  /* ── Pointer interaction ── */
  let panning = false;
  let panMoved = false;
  let panStart = { x: 0, y: 0, camX: 0, camY: 0 };

  function stagePoint(e: PointerEvent | WheelEvent): { x: number; y: number } {
    const rect = stageEl?.getBoundingClientRect();
    return { x: e.clientX - (rect?.left ?? 0), y: e.clientY - (rect?.top ?? 0) };
  }

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    const p = stagePoint(e);
    panning = true;
    panMoved = false;
    panStart = { x: p.x, y: p.y, camX: camera.x, camY: camera.y };
    try {
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    } catch {
      /* Synthetic pointer events carry no active pointer to capture. */
    }
  }

  function onPointerMove(e: PointerEvent) {
    const p = stagePoint(e);
    if (panning) {
      const dx = p.x - panStart.x;
      const dy = p.y - panStart.y;
      if (Math.hypot(dx, dy) > 4) panMoved = true;
      if (panMoved) {
        camera.x = panStart.camX - dx / camera.scale;
        camera.y = panStart.camY - dy / camera.scale;
        tooltip = null;
        dirty = true;
        return;
      }
    }
    setHover(hitTest(p.x, p.y), p.x, p.y);
  }

  function onPointerUp(e: PointerEvent) {
    if (!panning) return;
    panning = false;
    if (panMoved) return;
    const p = stagePoint(e);
    const node = hitTest(p.x, p.y);
    if (!node) return;
    if (node.kind === 'community') {
      if (expanded.has(node.communityKey)) expanded.delete(node.communityKey);
      else expanded.add(node.communityKey);
      syncUrl();
      rebuild();
    } else if (node.symbolId !== null) {
      navigate(`/symbol/${node.symbolId}`);
    }
  }

  function onPointerLeave() {
    panning = false;
    setHover(null, 0, 0);
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const p = stagePoint(e);
    const before = worldFromScreen(p.x, p.y);
    const factor = Math.exp(-e.deltaY * 0.0015);
    camera.scale = Math.min(20, Math.max(0.005, camera.scale * factor));
    const after = worldFromScreen(p.x, p.y);
    camera.x += before.x - after.x;
    camera.y += before.y - after.y;
    tooltip = null;
    dirty = true;
  }

  /* ── Community labels: imperative DOM so pan stays off Svelte's render
        path — 60fps panning must not schedule component updates. ── */
  function rebuildLabels() {
    if (!labelLayer || !visible) return;
    labelLayer.replaceChildren();
    labelEls = new Map();
    for (const n of visible.nodes) {
      if (n.kind !== 'community') continue;
      const el = document.createElement('div');
      el.className = 'atlas__label mono';
      el.textContent = `${n.label} · ${formatCount(n.memberCount)}`;
      labelLayer.appendChild(el);
      labelEls.set(n.key, el);
    }
    /* Expanded groups keep a caption at their disc center. */
    if (layout) {
      for (const g of layout.groups) {
        if (!expanded.has(g.key)) continue;
        const el = document.createElement('div');
        el.className = 'atlas__label atlas__label--open mono';
        el.textContent = g.label;
        labelLayer.appendChild(el);
        labelEls.set(`open:${g.key}`, el);
      }
    }
    updateLabels();
  }

  function updateLabels() {
    if (!visible || !layout) return;
    const place = (el: HTMLDivElement, wx: number, wy: number, dy: number) => {
      const sx = cssWidth / 2 + (wx - camera.x) * camera.scale;
      const sy = cssHeight / 2 + (wy - camera.y) * camera.scale + dy;
      const off = sx < -160 || sx > cssWidth + 160 || sy < -40 || sy > cssHeight + 40;
      el.style.transform = `translate(-50%, 0) translate(${sx}px, ${sy}px)`;
      el.style.opacity = off ? '0' : '1';
    };
    for (const n of visible.nodes) {
      if (n.kind !== 'community') continue;
      const el = labelEls.get(n.key);
      if (el) place(el, n.x, n.y, n.radius * camera.scale + 6);
    }
    for (const g of layout.groups) {
      if (!expanded.has(g.key)) continue;
      const el = labelEls.get(`open:${g.key}`);
      if (el) place(el, g.cx, g.cy, -g.discRadius * camera.scale - 24);
    }
  }

  /* ── Render loop: continuous while mounted, draws only when dirty.
        Cancelled on unmount — ISC-22's no-leak claim rests here. ── */
  function tick() {
    if (dirty && renderer && canvasEl) {
      dirty = false;
      renderer.render(camera, canvasEl.width, canvasEl.height);
      updateLabels();
    }
    raf = requestAnimationFrame(tick);
  }

  function resizeCanvas() {
    if (!stageEl || !canvasEl) return;
    const dpr = window.devicePixelRatio || 1;
    cssWidth = stageEl.clientWidth;
    cssHeight = stageEl.clientHeight;
    canvasEl.width = Math.max(1, Math.round(cssWidth * dpr));
    canvasEl.height = Math.max(1, Math.round(cssHeight * dpr));
    dirty = true;
  }

  /* ── Verification handles (ISA test strategy 18–22): a scripted pan
        benchmark and an overlap probe, driven from the browser console. ── */
  function panBench(sampleMs = 5000): Promise<{
    avgFps: number;
    onePercentLowFps: number;
    frames: number;
  }> {
    return new Promise((resolve) => {
      const cam0 = { x: camera.x, y: camera.y, scale: camera.scale };
      const start = performance.now();
      let last = start;
      const dts: number[] = [];
      const bench = (now: number) => {
        const dt = now - last;
        last = now;
        if (dt > 0) dts.push(dt);
        const t = (now - start) / 1000;
        camera.x = cam0.x + Math.sin(t * 0.6) * fitBounds.w * 0.2;
        camera.y = cam0.y + Math.cos(t * 0.45) * fitBounds.h * 0.15;
        camera.scale = cam0.scale * (1 + 0.25 * Math.sin(t * 0.5));
        if (renderer && canvasEl) {
          renderer.render(camera, canvasEl.width, canvasEl.height);
          updateLabels();
        }
        if (now - start < sampleMs) {
          requestAnimationFrame(bench);
        } else {
          camera.x = cam0.x;
          camera.y = cam0.y;
          camera.scale = cam0.scale;
          dirty = true;
          const settled = dts.slice(10).sort((a, b) => a - b);
          const avg = settled.reduce((s, v) => s + v, 0) / Math.max(settled.length, 1);
          const p99 =
            settled[Math.min(settled.length - 1, Math.floor(settled.length * 0.99))] ?? avg;
          resolve({
            avgFps: Math.round(1000 / avg),
            onePercentLowFps: Math.round(1000 / p99),
            frames: settled.length,
          });
        }
      };
      requestAnimationFrame(bench);
    });
  }

  function overlapCount(): number {
    if (!visible) return -1;
    const supers = visible.nodes.filter((n) => n.kind === 'community');
    let overlaps = 0;
    for (let i = 0; i < supers.length; i++) {
      for (let j = i + 1; j < supers.length; j++) {
        const a = supers[i];
        const b = supers[j];
        if (!a || !b) continue;
        if (Math.hypot(a.x - b.x, a.y - b.y) < a.radius + b.radius) overlaps++;
      }
    }
    return overlaps;
  }

  function stats() {
    return {
      nodes: visible?.nodes.length ?? 0,
      edges: visible?.edges.length ?? 0,
      supernodes: visible?.nodes.filter((n) => n.kind === 'community').length ?? 0,
      expanded: [...expanded],
      scale: camera.scale,
      overlaps: overlapCount(),
    };
  }

  /** Visible nodes projected to stage-space CSS px — verification drives real
   * pointer events at these coordinates. */
  function screenPositions() {
    return (visible?.nodes ?? []).map((n) => ({
      key: n.key,
      kind: n.kind,
      label: n.label,
      symbolId: n.symbolId,
      x: cssWidth / 2 + (n.x - camera.x) * camera.scale,
      y: cssHeight / 2 + (n.y - camera.y) * camera.scale,
      r: n.radius * camera.scale,
    }));
  }

  /* ── Mount: initialize once the data and the stage both exist. ── */
  let initialized = false;
  $effect(() => {
    if (initialized || resource.status !== 'ready' || !stageEl) return;
    if (resource.data.nodes.length === 0) return;
    initialized = true;

    data = resource.data;
    layout = computeLayout(data);
    restoreExpandedFromUrl();

    canvasEl = document.createElement('canvas');
    canvasEl.className = 'atlas__canvas';
    canvasEl.setAttribute('role', 'img');
    canvasEl.setAttribute(
      'aria-label',
      `Dependency atlas: ${formatCount(data.nodes.length)} symbols in ${formatCount(layout.groups.length)} communities`
    );
    stageEl.prepend(canvasEl);

    renderer = createRenderer(canvasEl, readAtlasPalette(PALETTE_SIZE));
    if (!renderer) {
      webglFailed = true;
      return;
    }

    resizeCanvas();
    resizeObserver = new ResizeObserver(() => {
      resizeCanvas();
    });
    resizeObserver.observe(stageEl);

    rebuild();
    fit();
    raf = requestAnimationFrame(tick);

    (window as unknown as Record<string, unknown>).__atlas = {
      stats,
      panBench,
      screenPositions,
    };

    return () => {
      cancelAnimationFrame(raf);
      resizeObserver?.disconnect();
      renderer?.dispose();
      renderer = null;
      delete (window as unknown as Record<string, unknown>).__atlas;
    };
  });
</script>

<div class="atlas">
  {#if resource.status === 'loading'}
    <Skeleton height="32px" />
    <Skeleton height="520px" />
  {:else if resource.status === 'error'}
    <ErrorState message={resource.message} onretry={load} />
  {:else if resource.data.nodes.length === 0}
    <EmptyState />
  {:else}
    {@const d = resource.data}
    <header class="atlas__bar">
      <div class="atlas__title-block">
        <h2 class="atlas__title serif">Atlas</h2>
        <p class="atlas__counts mono">
          {formatCount(d.nodes.length)} symbols · {formatCount(d.edges.length)} calls ·
          {formatCount(d.communities.length)} communities
          {#if d.truncated}
            · showing the {formatCount(d.nodes.length)} most connected of
            {formatCount(d.total_symbols)}
          {/if}
        </p>
      </div>
      <div class="atlas__actions">
        <button onclick={fit}>Fit</button>
        <button onclick={collapseAll} disabled={openCount === 0}>
          Collapse all{openCount > 0 ? ` (${openCount})` : ''}
        </button>
      </div>
    </header>

    {#if webglFailed}
      <div class="atlas__fallback" role="alert">
        <p class="serif">WebGL is unavailable in this browser.</p>
        <p>
          The atlas needs GPU rendering for large graphs. Symbol search and the dossier views
          still work.
        </p>
      </div>
    {:else}
      <div
        class="atlas__stage"
        bind:this={stageEl}
        role="application"
        aria-label="Dependency atlas — drag to pan, scroll to zoom, click to open"
        onpointerdown={onPointerDown}
        onpointermove={onPointerMove}
        onpointerup={onPointerUp}
        onpointerleave={onPointerLeave}
        onwheel={onWheel}
      >
        <div class="atlas__labels" bind:this={labelLayer}></div>
        {#if tooltip}
          <div class="atlas__tooltip" style="left: {tooltip.x}px; top: {tooltip.y}px">
            <span class="atlas__tooltip-title mono">{tooltip.title}</span>
            <span class="atlas__tooltip-sub">{tooltip.sub}</span>
            <span class="atlas__tooltip-hint">{tooltip.hint}</span>
          </div>
        {/if}
      </div>
      <p class="atlas__legend">
        Arrows point from caller to callee · brighter edges carry more calls · click a cluster
        to expand it, click a symbol for its dossier · drag to pan, scroll to zoom
      </p>
    {/if}
  {/if}
</div>

<style>
  .atlas {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .atlas__bar {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--space-4);
    flex-wrap: wrap;
  }
  .atlas__title {
    font-family: var(--font-serif);
    font-size: var(--text-xl);
    color: var(--accent);
  }
  .atlas__counts {
    color: var(--text-muted);
    font-size: var(--text-xs);
    margin-top: var(--space-1);
  }
  .atlas__actions {
    display: flex;
    gap: var(--space-3);
  }
  .atlas__actions button {
    padding: var(--space-2) var(--space-4);
    background: var(--bg-elevated);
    border: 1px solid var(--border-active);
    border-radius: var(--radius-sm);
    color: var(--accent);
    font-size: var(--text-sm);
  }
  .atlas__actions button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .atlas__stage {
    position: relative;
    width: 100%;
    height: max(480px, calc(100vh - 260px));
    background: var(--bg-void);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    overflow: hidden;
    touch-action: none;
    cursor: grab;
  }
  .atlas__stage:active {
    cursor: grabbing;
  }
  .atlas__stage :global(.atlas__canvas) {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  .atlas__labels {
    position: absolute;
    inset: 0;
    pointer-events: none;
    overflow: hidden;
  }
  .atlas__labels :global(.atlas__label) {
    position: absolute;
    top: 0;
    left: 0;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    background: rgba(6, 8, 12, 0.7);
    padding: 1px var(--space-2);
    border-radius: var(--radius-sm);
    white-space: nowrap;
    will-change: transform;
  }
  .atlas__labels :global(.atlas__label--open) {
    color: var(--accent);
    border: 1px solid var(--border-active);
  }
  .atlas__tooltip {
    position: absolute;
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-width: 320px;
    padding: var(--space-2) var(--space-3);
    background: var(--bg-elevated);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    pointer-events: none;
    z-index: 5;
  }
  .atlas__tooltip-title {
    font-size: var(--text-sm);
    color: var(--text-primary);
  }
  .atlas__tooltip-sub {
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }
  .atlas__tooltip-hint {
    font-size: var(--text-xs);
    color: var(--accent);
  }
  .atlas__legend {
    color: var(--text-muted);
    font-size: var(--text-xs);
  }
  .atlas__fallback {
    padding: var(--space-5);
    background: var(--bg-card);
    border: 1px solid var(--border-strong);
    border-left: 3px solid var(--threshold-elevated);
    border-radius: var(--radius-md);
    color: var(--text-secondary);
  }
  .atlas__fallback .serif {
    font-family: var(--font-serif);
    font-size: var(--text-lg);
    color: var(--text-primary);
    margin-bottom: var(--space-2);
  }
</style>
