/** Atlas WebGL renderer — the D-8 spike's structure (static buffers, uniform
 * camera, round points via gl_PointCoord discard) extended with per-vertex
 * color/state and arrowhead triangles so edge direction is visible (ISC-18).
 * Three draw calls per frame regardless of graph size: LINES (edges),
 * TRIANGLES (arrowheads), POINTS (nodes). Hover updates only the small
 * dynamic state buffers. */

import type { VisibleEdge, VisibleNode } from './layout';

export interface Camera {
  x: number;
  y: number;
  scale: number;
}

export type Rgb = [number, number, number];

export interface Renderer {
  setGraph(nodes: VisibleNode[], edges: VisibleEdge[]): void;
  /** One float per node: 0 normal · 1 hovered · 2 neighbor · 3 dimmed. */
  setNodeStates(states: Float32Array): void;
  /** One float per edge (replicated across its vertices internally). */
  setEdgeStates(states: Float32Array): void;
  render(cam: Camera, deviceWidth: number, deviceHeight: number): void;
  dispose(): void;
}

const CAMERA_VS = `
  attribute vec2 a_pos;
  uniform vec2 u_translate;
  uniform float u_scale;
  uniform vec2 u_resolution;
  vec4 project() {
    vec2 world = (a_pos - u_translate) * u_scale;
    vec2 clip = world / (u_resolution * 0.5);
    return vec4(clip.x, -clip.y, 0.0, 1.0);
  }
`;

const POINT_VS = `${CAMERA_VS}
  attribute float a_size;
  attribute vec3 a_color;
  attribute float a_state;
  uniform float u_minPoint;
  uniform float u_maxPoint;
  varying vec3 v_color;
  varying float v_state;
  void main() {
    gl_Position = project();
    gl_PointSize = clamp(a_size * 2.0 * u_scale, u_minPoint, u_maxPoint);
    v_color = a_color;
    v_state = a_state;
  }`;

const POINT_FS = `
  precision mediump float;
  varying vec3 v_color;
  varying float v_state;
  void main() {
    vec2 c = gl_PointCoord - vec2(0.5);
    if (dot(c, c) > 0.25) discard;
    vec3 col = v_color;
    float alpha = 0.92;
    if (v_state > 2.5) { alpha = 0.10; }
    else if (v_state > 1.5) { alpha = 1.0; }
    else if (v_state > 0.5) { col = mix(col, vec3(1.0), 0.35); alpha = 1.0; }
    gl_FragColor = vec4(col, alpha);
  }`;

const LINE_VS = `${CAMERA_VS}
  attribute vec4 a_color;
  attribute float a_t;
  attribute float a_state;
  varying vec4 v_color;
  varying float v_t;
  varying float v_state;
  void main() {
    gl_Position = project();
    v_color = a_color;
    v_t = a_t;
    v_state = a_state;
  }`;

const LINE_FS = `
  precision mediump float;
  varying vec4 v_color;
  varying float v_t;
  varying float v_state;
  void main() {
    float alpha = v_color.a * mix(0.45, 1.0, v_t);
    if (v_state > 2.5) { alpha *= 0.12; }
    else if (v_state > 1.5) { alpha = max(alpha, 0.85); }
    gl_FragColor = vec4(v_color.rgb, alpha);
  }`;

export function createRenderer(canvas: HTMLCanvasElement, palette: Rgb[]): Renderer | null {
  const gl = canvas.getContext('webgl2') ?? canvas.getContext('webgl');
  if (!gl) return null;

  function compile(type: number, src: string): WebGLShader | null {
    const shader = gl!.createShader(type);
    if (!shader) return null;
    gl!.shaderSource(shader, src);
    gl!.compileShader(shader);
    return gl!.getShaderParameter(shader, gl!.COMPILE_STATUS) ? shader : null;
  }
  function program(vs: string, fs: string): WebGLProgram | null {
    const v = compile(gl!.VERTEX_SHADER, vs);
    const f = compile(gl!.FRAGMENT_SHADER, fs);
    if (!v || !f) return null;
    const p = gl!.createProgram();
    if (!p) return null;
    gl!.attachShader(p, v);
    gl!.attachShader(p, f);
    gl!.linkProgram(p);
    return gl!.getProgramParameter(p, gl!.LINK_STATUS) ? p : null;
  }

  const pointProgram = program(POINT_VS, POINT_FS);
  const lineProgram = program(LINE_VS, LINE_FS);
  if (!pointProgram || !lineProgram) return null;

  const dpr = window.devicePixelRatio || 1;
  const pointSizeRange = gl.getParameter(gl.ALIASED_POINT_SIZE_RANGE) as Float32Array;
  const maxPoint = pointSizeRange?.[1] ?? 64;

  const nodeGeomBuf = gl.createBuffer();
  const nodeStateBuf = gl.createBuffer();
  const edgeGeomBuf = gl.createBuffer();
  const edgeStateBuf = gl.createBuffer();
  const arrowGeomBuf = gl.createBuffer();
  const arrowStateBuf = gl.createBuffer();

  let nodeCount = 0;
  let edgeCount = 0;

  const color = (index: number): Rgb => palette[index % palette.length] ?? [1, 1, 1];

  function setGraph(nodes: VisibleNode[], edges: VisibleEdge[]): void {
    nodeCount = nodes.length;
    edgeCount = edges.length;

    /* Nodes: [x, y, size, r, g, b] per vertex. Dead symbols drawn muted. */
    const nodeGeom = new Float32Array(nodes.length * 6);
    nodes.forEach((n, i) => {
      const [r, g, b] = color(n.colorIndex);
      const mute = n.node?.is_dead ? 0.45 : 1;
      nodeGeom.set([n.x, n.y, n.radius, r * mute, g * mute, b * mute], i * 6);
    });
    gl!.bindBuffer(gl!.ARRAY_BUFFER, nodeGeomBuf);
    gl!.bufferData(gl!.ARRAY_BUFFER, nodeGeom, gl!.STATIC_DRAW);
    gl!.bindBuffer(gl!.ARRAY_BUFFER, nodeStateBuf);
    gl!.bufferData(gl!.ARRAY_BUFFER, new Float32Array(nodes.length), gl!.DYNAMIC_DRAW);

    /* Edges: 2 vertices × [x, y, t, r, g, b, baseAlpha]. The t gradient plus
       the arrowhead make direction legible; weight sets brightness. */
    const edgeGeom = new Float32Array(edges.length * 2 * 7);
    const arrowGeom = new Float32Array(edges.length * 3 * 7);
    edges.forEach((e, i) => {
      const alpha = Math.min(0.55, 0.1 + 0.03 * e.weight);
      const [sr, sg, sb] = color(e.sourceColorIndex);
      const [tr, tg, tb] = color(e.targetColorIndex);
      edgeGeom.set([e.x1, e.y1, 0, sr, sg, sb, alpha], i * 14);
      edgeGeom.set([e.x2, e.y2, 1, tr, tg, tb, alpha], i * 14 + 7);

      /* Arrowhead: an isoceles triangle whose tip rests on the target's rim.
         Zero-length edges get a degenerate (invisible) triangle so buffer
         alignment with edge indices never breaks. */
      const dx = e.x2 - e.x1;
      const dy = e.y2 - e.y1;
      const len = Math.hypot(dx, dy);
      const base = i * 21;
      if (len > 0.001) {
        const ux = dx / len;
        const uy = dy / len;
        /* Sized against the target node so hub fan-in reads as a fine ring of
           ticks, not a sunburst that swallows the node. */
        const size = Math.min(e.targetRadius * 0.8, 3 + e.weight * 0.25);
        const tipX = e.x2 - ux * (e.targetRadius + 1.5);
        const tipY = e.y2 - uy * (e.targetRadius + 1.5);
        const backX = tipX - ux * size;
        const backY = tipY - uy * size;
        const px = -uy * size * 0.4;
        const py = ux * size * 0.4;
        const arrowAlpha = Math.min(0.8, 0.35 + 0.03 * e.weight);
        arrowGeom.set([tipX, tipY, 1, tr, tg, tb, arrowAlpha], base);
        arrowGeom.set([backX + px, backY + py, 1, tr, tg, tb, arrowAlpha], base + 7);
        arrowGeom.set([backX - px, backY - py, 1, tr, tg, tb, arrowAlpha], base + 14);
      }
    });
    gl!.bindBuffer(gl!.ARRAY_BUFFER, edgeGeomBuf);
    gl!.bufferData(gl!.ARRAY_BUFFER, edgeGeom, gl!.STATIC_DRAW);
    gl!.bindBuffer(gl!.ARRAY_BUFFER, edgeStateBuf);
    gl!.bufferData(gl!.ARRAY_BUFFER, new Float32Array(edges.length * 2), gl!.DYNAMIC_DRAW);
    gl!.bindBuffer(gl!.ARRAY_BUFFER, arrowGeomBuf);
    gl!.bufferData(gl!.ARRAY_BUFFER, arrowGeom, gl!.STATIC_DRAW);
    gl!.bindBuffer(gl!.ARRAY_BUFFER, arrowStateBuf);
    gl!.bufferData(gl!.ARRAY_BUFFER, new Float32Array(edges.length * 3), gl!.DYNAMIC_DRAW);
  }

  function setNodeStates(states: Float32Array): void {
    gl!.bindBuffer(gl!.ARRAY_BUFFER, nodeStateBuf);
    gl!.bufferData(gl!.ARRAY_BUFFER, states, gl!.DYNAMIC_DRAW);
  }

  function setEdgeStates(states: Float32Array): void {
    const perVertex = new Float32Array(states.length * 2);
    const perArrow = new Float32Array(states.length * 3);
    for (let i = 0; i < states.length; i++) {
      const s = states[i] ?? 0;
      perVertex[i * 2] = s;
      perVertex[i * 2 + 1] = s;
      perArrow[i * 3] = s;
      perArrow[i * 3 + 1] = s;
      perArrow[i * 3 + 2] = s;
    }
    gl!.bindBuffer(gl!.ARRAY_BUFFER, edgeStateBuf);
    gl!.bufferData(gl!.ARRAY_BUFFER, perVertex, gl!.DYNAMIC_DRAW);
    gl!.bindBuffer(gl!.ARRAY_BUFFER, arrowStateBuf);
    gl!.bufferData(gl!.ARRAY_BUFFER, perArrow, gl!.DYNAMIC_DRAW);
  }

  function bindAttrib(
    p: WebGLProgram,
    name: string,
    buf: WebGLBuffer | null,
    size: number,
    stride: number,
    offset: number
  ): void {
    const loc = gl!.getAttribLocation(p, name);
    if (loc < 0) return;
    gl!.bindBuffer(gl!.ARRAY_BUFFER, buf);
    gl!.enableVertexAttribArray(loc);
    gl!.vertexAttribPointer(loc, size, gl!.FLOAT, false, stride, offset);
  }

  function setCamera(p: WebGLProgram, cam: Camera, w: number, h: number): void {
    gl!.uniform2f(gl!.getUniformLocation(p, 'u_translate'), cam.x, cam.y);
    gl!.uniform1f(gl!.getUniformLocation(p, 'u_scale'), cam.scale * dpr);
    gl!.uniform2f(gl!.getUniformLocation(p, 'u_resolution'), w, h);
  }

  gl.enable(gl.BLEND);
  gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

  function render(cam: Camera, w: number, h: number): void {
    gl!.viewport(0, 0, w, h);
    gl!.clearColor(0.024, 0.031, 0.047, 1);
    gl!.clear(gl!.COLOR_BUFFER_BIT);

    if (edgeCount > 0) {
      gl!.useProgram(lineProgram);
      setCamera(lineProgram!, cam, w, h);
      bindAttrib(lineProgram!, 'a_pos', edgeGeomBuf, 2, 28, 0);
      bindAttrib(lineProgram!, 'a_t', edgeGeomBuf, 1, 28, 8);
      bindAttrib(lineProgram!, 'a_color', edgeGeomBuf, 4, 28, 12);
      bindAttrib(lineProgram!, 'a_state', edgeStateBuf, 1, 4, 0);
      gl!.drawArrays(gl!.LINES, 0, edgeCount * 2);

      bindAttrib(lineProgram!, 'a_pos', arrowGeomBuf, 2, 28, 0);
      bindAttrib(lineProgram!, 'a_t', arrowGeomBuf, 1, 28, 8);
      bindAttrib(lineProgram!, 'a_color', arrowGeomBuf, 4, 28, 12);
      bindAttrib(lineProgram!, 'a_state', arrowStateBuf, 1, 4, 0);
      gl!.drawArrays(gl!.TRIANGLES, 0, edgeCount * 3);
    }

    if (nodeCount > 0) {
      gl!.useProgram(pointProgram);
      setCamera(pointProgram!, cam, w, h);
      gl!.uniform1f(gl!.getUniformLocation(pointProgram!, 'u_minPoint'), 3 * dpr);
      gl!.uniform1f(gl!.getUniformLocation(pointProgram!, 'u_maxPoint'), maxPoint);
      bindAttrib(pointProgram!, 'a_pos', nodeGeomBuf, 2, 24, 0);
      bindAttrib(pointProgram!, 'a_size', nodeGeomBuf, 1, 24, 8);
      bindAttrib(pointProgram!, 'a_color', nodeGeomBuf, 3, 24, 12);
      bindAttrib(pointProgram!, 'a_state', nodeStateBuf, 1, 4, 0);
      gl!.drawArrays(gl!.POINTS, 0, nodeCount);
    }
  }

  function dispose(): void {
    for (const buf of [
      nodeGeomBuf,
      nodeStateBuf,
      edgeGeomBuf,
      edgeStateBuf,
      arrowGeomBuf,
      arrowStateBuf,
    ]) {
      if (buf) gl!.deleteBuffer(buf);
    }
    if (pointProgram) gl!.deleteProgram(pointProgram);
    if (lineProgram) gl!.deleteProgram(lineProgram);
  }

  return { setGraph, setNodeStates, setEdgeStates, render, dispose };
}

/** Read the community palette out of the token system so even shader colors
 * stay token-sourced (tokens.css --atlas-1..N). */
export function readAtlasPalette(count: number): Rgb[] {
  const styles = getComputedStyle(document.documentElement);
  const palette: Rgb[] = [];
  for (let i = 1; i <= count; i++) {
    const raw = styles.getPropertyValue(`--atlas-${i}`).trim();
    const hex = raw.replace('#', '');
    if (hex.length === 6) {
      palette.push([
        parseInt(hex.slice(0, 2), 16) / 255,
        parseInt(hex.slice(2, 4), 16) / 255,
        parseInt(hex.slice(4, 6), 16) / 255,
      ]);
    }
  }
  return palette.length > 0 ? palette : [[0.83, 0.66, 0.33]];
}
