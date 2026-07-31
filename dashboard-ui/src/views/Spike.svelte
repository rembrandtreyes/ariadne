<script lang="ts">
  /* Renderer spike (P1, D-6): measure Canvas2D vs WebGL at 5,000 nodes /
     8,000 edges under a scripted pan+zoom, so Atlas (F4) picks its renderer
     on numbers instead of guesses. Synthetic clustered data, same camera
     path for both renderers, fps sampled over a fixed window. */

  const NODE_COUNT = 5000;
  const EDGE_COUNT = 8000;
  const CLUSTERS = 24;
  const WORLD = 4000;
  const SAMPLE_MS = 6000;

  type Mode = 'canvas' | 'webgl';

  interface SpikeResult {
    mode: Mode;
    avgFps: number;
    onePercentLowFps: number;
    frames: number;
  }

  let containerEl = $state<HTMLDivElement | null>(null);
  let mode = $state<Mode>('canvas');
  let running = $state(false);
  let results = $state<SpikeResult[]>([]);
  let liveFps = $state(0);

  /* Deterministic PRNG so both renderers draw the identical graph. */
  function mulberry32(seed: number) {
    return () => {
      seed |= 0;
      seed = (seed + 0x6d2b79f5) | 0;
      let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  function buildGraph() {
    const rand = mulberry32(1337);
    const xs = new Float32Array(NODE_COUNT);
    const ys = new Float32Array(NODE_COUNT);
    const group = new Uint8Array(NODE_COUNT);
    const cx: number[] = [];
    const cy: number[] = [];
    for (let c = 0; c < CLUSTERS; c++) {
      cx.push((rand() - 0.5) * WORLD);
      cy.push((rand() - 0.5) * WORLD);
    }
    for (let i = 0; i < NODE_COUNT; i++) {
      const c = Math.floor(rand() * CLUSTERS);
      group[i] = c % 5;
      const r = rand() * 220 * Math.sqrt(rand());
      const a = rand() * Math.PI * 2;
      xs[i] = (cx[c] ?? 0) + Math.cos(a) * r;
      ys[i] = (cy[c] ?? 0) + Math.sin(a) * r;
    }
    const src = new Uint32Array(EDGE_COUNT);
    const dst = new Uint32Array(EDGE_COUNT);
    for (let e = 0; e < EDGE_COUNT; e++) {
      const a = Math.floor(rand() * NODE_COUNT);
      /* 80% intra-cluster-ish: pick a nearby index. */
      const b =
        rand() < 0.8
          ? Math.min(NODE_COUNT - 1, a + Math.floor(rand() * 200))
          : Math.floor(rand() * NODE_COUNT);
      src[e] = a;
      dst[e] = b;
    }
    return { xs, ys, group, src, dst };
  }

  const graph = buildGraph();
  const GROUP_COLORS = ['#d4a853', '#4ade80', '#60a5fa', '#f87171', '#c084fc'];

  /* Scripted camera: slow pan sweep + zoom breathing, identical per mode. */
  function camera(tMs: number) {
    const t = tMs / 1000;
    return {
      x: Math.sin(t * 0.4) * WORLD * 0.25,
      y: Math.cos(t * 0.3) * WORLD * 0.2,
      scale: 0.35 + 0.15 * Math.sin(t * 0.5),
    };
  }

  let raf = 0;

  function stop() {
    cancelAnimationFrame(raf);
    running = false;
  }

  function runSample(m: Mode) {
    if (!containerEl) return;
    stop();
    mode = m;
    running = true;
    /* A canvas element can only ever vend ONE context kind — after a '2d'
       run, getContext('webgl2') returns null on the same element. Fresh
       element per sample keeps the two renderers honest. */
    containerEl.replaceChildren();
    const el = document.createElement('canvas');
    el.className = 'spike__canvas-el';
    containerEl.appendChild(el);
    const dpr = window.devicePixelRatio || 1;
    el.width = containerEl.clientWidth * dpr;
    el.height = containerEl.clientHeight * dpr;

    const frameTimes: number[] = [];
    let last = performance.now();
    const start = last;

    const draw =
      m === 'canvas' ? makeCanvasRenderer(el, dpr) : makeWebglRenderer(el, dpr);
    if (!draw) {
      results = [...results, { mode: m, avgFps: 0, onePercentLowFps: 0, frames: 0 }];
      running = false;
      return;
    }

    const tick = (now: number) => {
      const dt = now - last;
      last = now;
      if (dt > 0) frameTimes.push(dt);
      liveFps = Math.round(1000 / Math.max(dt, 0.001));
      draw(camera(now - start), el.width, el.height);
      if (now - start < SAMPLE_MS) {
        raf = requestAnimationFrame(tick);
      } else {
        /* Drop the first 10 frames (warm-up, JIT, buffer upload). */
        const settled = frameTimes.slice(10);
        settled.sort((a, b) => a - b);
        const avg = settled.reduce((s, v) => s + v, 0) / Math.max(settled.length, 1);
        const p99 = settled[Math.min(settled.length - 1, Math.floor(settled.length * 0.99))] ?? avg;
        results = [
          ...results.filter((r) => r.mode !== m),
          {
            mode: m,
            avgFps: Math.round(1000 / avg),
            onePercentLowFps: Math.round(1000 / p99),
            frames: settled.length,
          },
        ];
        running = false;
      }
    };
    raf = requestAnimationFrame(tick);
  }

  type DrawFn = (cam: { x: number; y: number; scale: number }, w: number, h: number) => void;

  function makeCanvasRenderer(el: HTMLCanvasElement, dpr: number): DrawFn | null {
    const ctx = el.getContext('2d');
    if (!ctx) return null;
    return (cam, w, h) => {
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.fillStyle = '#06080c';
      ctx.fillRect(0, 0, w, h);
      const s = cam.scale * dpr;
      ctx.setTransform(s, 0, 0, s, w / 2 - cam.x * s, h / 2 - cam.y * s);

      ctx.strokeStyle = 'rgba(212, 168, 83, 0.10)';
      ctx.lineWidth = 1 / cam.scale;
      ctx.beginPath();
      for (let e = 0; e < EDGE_COUNT; e++) {
        const a = graph.src[e] ?? 0;
        const b = graph.dst[e] ?? 0;
        ctx.moveTo(graph.xs[a] ?? 0, graph.ys[a] ?? 0);
        ctx.lineTo(graph.xs[b] ?? 0, graph.ys[b] ?? 0);
      }
      ctx.stroke();

      const r = 3.5;
      for (let g = 0; g < 5; g++) {
        ctx.fillStyle = GROUP_COLORS[g] ?? '#fff';
        ctx.beginPath();
        for (let i = 0; i < NODE_COUNT; i++) {
          if (graph.group[i] !== g) continue;
          const x = graph.xs[i] ?? 0;
          const y = graph.ys[i] ?? 0;
          ctx.moveTo(x + r, y);
          ctx.arc(x, y, r, 0, Math.PI * 2);
        }
        ctx.fill();
      }
    };
  }

  function makeWebglRenderer(el: HTMLCanvasElement, dpr: number): DrawFn | null {
    const gl = el.getContext('webgl2') ?? el.getContext('webgl');
    if (!gl) return null;

    const vsSrc = `
      attribute vec2 a_pos;
      attribute float a_group;
      uniform vec2 u_translate;
      uniform float u_scale;
      uniform vec2 u_resolution;
      uniform float u_pointSize;
      varying float v_group;
      void main() {
        vec2 world = (a_pos - u_translate) * u_scale;
        vec2 clip = world / (u_resolution * 0.5);
        gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
        gl_PointSize = u_pointSize;
        v_group = a_group;
      }`;
    const fsPoints = `
      precision mediump float;
      varying float v_group;
      void main() {
        vec2 c = gl_PointCoord - vec2(0.5);
        if (dot(c, c) > 0.25) discard;
        vec3 colors[5];
        colors[0] = vec3(0.831, 0.659, 0.325);
        colors[1] = vec3(0.290, 0.871, 0.502);
        colors[2] = vec3(0.376, 0.647, 0.980);
        colors[3] = vec3(0.973, 0.443, 0.443);
        colors[4] = vec3(0.753, 0.518, 0.988);
        int g = int(v_group + 0.5);
        vec3 col = colors[0];
        if (g == 1) col = colors[1];
        if (g == 2) col = colors[2];
        if (g == 3) col = colors[3];
        if (g == 4) col = colors[4];
        gl_FragColor = vec4(col, 1.0);
      }`;
    const fsLines = `
      precision mediump float;
      varying float v_group;
      void main() { gl_FragColor = vec4(0.831, 0.659, 0.325, 0.10); }`;

    function compile(type: number, src: string): WebGLShader | null {
      const sh = gl!.createShader(type);
      if (!sh) return null;
      gl!.shaderSource(sh, src);
      gl!.compileShader(sh);
      return gl!.getShaderParameter(sh, gl!.COMPILE_STATUS) ? sh : null;
    }
    function program(fs: string): WebGLProgram | null {
      const v = compile(gl!.VERTEX_SHADER, vsSrc);
      const f = compile(gl!.FRAGMENT_SHADER, fs);
      if (!v || !f) return null;
      const p = gl!.createProgram();
      if (!p) return null;
      gl!.attachShader(p, v);
      gl!.attachShader(p, f);
      gl!.linkProgram(p);
      return gl!.getProgramParameter(p, gl!.LINK_STATUS) ? p : null;
    }

    const pPoints = program(fsPoints);
    const pLines = program(fsLines);
    if (!pPoints || !pLines) return null;

    /* Node buffer: x, y, group per vertex. */
    const nodeData = new Float32Array(NODE_COUNT * 3);
    for (let i = 0; i < NODE_COUNT; i++) {
      nodeData[i * 3] = graph.xs[i] ?? 0;
      nodeData[i * 3 + 1] = graph.ys[i] ?? 0;
      nodeData[i * 3 + 2] = graph.group[i] ?? 0;
    }
    const nodeBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, nodeBuf);
    gl.bufferData(gl.ARRAY_BUFFER, nodeData, gl.STATIC_DRAW);

    /* Edge buffer: two endpoints per edge. */
    const edgeData = new Float32Array(EDGE_COUNT * 6);
    for (let e = 0; e < EDGE_COUNT; e++) {
      const a = graph.src[e] ?? 0;
      const b = graph.dst[e] ?? 0;
      edgeData[e * 6] = graph.xs[a] ?? 0;
      edgeData[e * 6 + 1] = graph.ys[a] ?? 0;
      edgeData[e * 6 + 2] = 0;
      edgeData[e * 6 + 3] = graph.xs[b] ?? 0;
      edgeData[e * 6 + 4] = graph.ys[b] ?? 0;
      edgeData[e * 6 + 5] = 0;
    }
    const edgeBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, edgeBuf);
    gl.bufferData(gl.ARRAY_BUFFER, edgeData, gl.STATIC_DRAW);

    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

    function bindAttribs(p: WebGLProgram, buf: WebGLBuffer | null) {
      gl!.bindBuffer(gl!.ARRAY_BUFFER, buf);
      const pos = gl!.getAttribLocation(p, 'a_pos');
      const grp = gl!.getAttribLocation(p, 'a_group');
      gl!.enableVertexAttribArray(pos);
      gl!.vertexAttribPointer(pos, 2, gl!.FLOAT, false, 12, 0);
      if (grp >= 0) {
        gl!.enableVertexAttribArray(grp);
        gl!.vertexAttribPointer(grp, 1, gl!.FLOAT, false, 12, 8);
      }
    }
    function setUniforms(
      p: WebGLProgram,
      cam: { x: number; y: number; scale: number },
      w: number,
      h: number
    ) {
      gl!.uniform2f(gl!.getUniformLocation(p, 'u_translate'), cam.x, cam.y);
      gl!.uniform1f(gl!.getUniformLocation(p, 'u_scale'), cam.scale * dpr);
      gl!.uniform2f(gl!.getUniformLocation(p, 'u_resolution'), w, h);
      gl!.uniform1f(gl!.getUniformLocation(p, 'u_pointSize'), 7 * dpr * cam.scale);
    }

    return (cam, w, h) => {
      gl.viewport(0, 0, w, h);
      gl.clearColor(0.024, 0.031, 0.047, 1);
      gl.clear(gl.COLOR_BUFFER_BIT);

      gl.useProgram(pLines);
      bindAttribs(pLines, edgeBuf);
      setUniforms(pLines, cam, w, h);
      gl.drawArrays(gl.LINES, 0, EDGE_COUNT * 2);

      gl.useProgram(pPoints);
      bindAttribs(pPoints, nodeBuf);
      setUniforms(pPoints, cam, w, h);
      gl.drawArrays(gl.POINTS, 0, NODE_COUNT);
    };
  }

  $effect(() => stop);
</script>

<div class="spike">
  <header class="spike__header">
    <h2 class="serif">Renderer spike</h2>
    <p>
      {NODE_COUNT.toLocaleString()} nodes · {EDGE_COUNT.toLocaleString()} edges · scripted
      pan+zoom · {SAMPLE_MS / 1000}s sample per run. Atlas (F4) picks its renderer from these
      numbers (ISC-21 floor: ≥30fps during pan).
    </p>
    <div class="spike__controls">
      <button onclick={() => runSample('canvas')} disabled={running}>Run Canvas2D</button>
      <button onclick={() => runSample('webgl')} disabled={running}>Run WebGL</button>
      {#if running}
        <span class="spike__live mono">sampling {mode}… {liveFps} fps</span>
      {/if}
    </div>
    {#if results.length > 0}
      <table class="spike__results">
        <thead>
          <tr><th>renderer</th><th>avg fps</th><th>1% low fps</th><th>frames</th></tr>
        </thead>
        <tbody>
          {#each results as r (r.mode)}
            <tr>
              <td>{r.mode}</td>
              <td class="mono">{r.avgFps}</td>
              <td class="mono">{r.onePercentLowFps}</td>
              <td class="mono">{r.frames}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </header>
  <div bind:this={containerEl} class="spike__canvas"></div>
</div>

<style>
  .spike {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .spike__header h2 {
    font-family: var(--font-serif);
    font-size: var(--text-xl);
    color: var(--accent);
    margin-bottom: var(--space-2);
  }
  .spike__header p {
    color: var(--text-secondary);
    font-size: var(--text-sm);
    max-width: 70ch;
  }
  .spike__controls {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-3);
  }
  .spike__controls button {
    padding: var(--space-2) var(--space-4);
    background: var(--bg-elevated);
    border: 1px solid var(--border-active);
    border-radius: var(--radius-sm);
    color: var(--accent);
    font-size: var(--text-sm);
  }
  .spike__controls button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .spike__live {
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }
  .spike__results {
    margin-top: var(--space-3);
    border-collapse: collapse;
    font-size: var(--text-sm);
  }
  .spike__results th,
  .spike__results td {
    text-align: left;
    padding: var(--space-1) var(--space-4) var(--space-1) 0;
    color: var(--text-secondary);
  }
  .spike__results th {
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }
  .spike__canvas {
    width: 100%;
    height: 560px;
    background: var(--bg-void);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    overflow: hidden;
  }
  .spike__canvas :global(.spike__canvas-el) {
    width: 100%;
    height: 100%;
  }
</style>
