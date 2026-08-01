/** Atlas layout: deterministic clustered placement, no force simulation.
 * Members of each community sit on a phyllotaxis disc (hubs in the center);
 * community centers are packed greedily along a Vogel spiral with a
 * collision test, so no two groups overlap — ISC-19's "zero overlap at
 * default zoom" is a property of construction, not of a simulation settling.
 * Same input → same layout, every load. */

import type { AtlasData, AtlasNode } from '../api';

export const UNGROUPED = -1;
export const PALETTE_SIZE = 12;

const GOLDEN_ANGLE = 2.399963229728653;
/* Vogel-spiral nearest-neighbor distance ≈ the spacing constant, so spacing
   must exceed the largest member diameter (2×8) for overlap-free discs. */
const MEMBER_SPACING = 20;
const GROUP_MARGIN = 40;

export interface VisibleNode {
  key: string;
  kind: 'member' | 'community';
  /** DB symbol id for members; null for community super-nodes. */
  symbolId: number | null;
  communityKey: number;
  x: number;
  y: number;
  radius: number;
  colorIndex: number;
  label: string;
  memberCount: number;
  node: AtlasNode | null;
}

export interface VisibleEdge {
  sourceKey: string;
  targetKey: string;
  /** Number of underlying calls this (possibly aggregated) edge carries. */
  weight: number;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  sourceColorIndex: number;
  targetColorIndex: number;
  targetRadius: number;
}

export interface GroupLayout {
  key: number;
  label: string;
  colorIndex: number;
  members: AtlasNode[];
  memberOffsets: { x: number; y: number; radius: number }[];
  discRadius: number;
  superRadius: number;
  cx: number;
  cy: number;
}

export interface AtlasLayout {
  groups: GroupLayout[];
  groupByKey: Map<number, GroupLayout>;
  /** symbol id → its group (for edge port resolution). */
  groupOfSymbol: Map<number, GroupLayout>;
  /** symbol id → world position + radius (member placement). */
  memberPlacement: Map<number, { x: number; y: number; radius: number }>;
}

export interface VisibleGraph {
  nodes: VisibleNode[];
  edges: VisibleEdge[];
  byKey: Map<string, VisibleNode>;
  /** Undirected adjacency over visible node keys (for hover highlight). */
  adjacency: Map<string, Set<string>>;
}

function memberRadius(node: AtlasNode): number {
  const degree = node.in_degree + node.out_degree;
  return Math.min(8, 3 + Math.sqrt(degree) * 0.9);
}

function superRadius(memberCount: number): number {
  return Math.min(90, 14 + 3.2 * Math.sqrt(memberCount));
}

export function computeLayout(data: AtlasData): AtlasLayout {
  const communityNames = new Map<number, string>();
  for (const c of data.communities) communityNames.set(c.id, c.name);

  /* Group nodes by community; null membership pools into UNGROUPED. */
  const byCommunity = new Map<number, AtlasNode[]>();
  for (const node of data.nodes) {
    const key = node.community ?? UNGROUPED;
    const bucket = byCommunity.get(key);
    if (bucket) bucket.push(node);
    else byCommunity.set(key, [node]);
  }

  /* Largest first: stable color assignment and center-out packing. */
  const ordered = [...byCommunity.entries()].sort(
    (a, b) => b[1].length - a[1].length || a[0] - b[0]
  );

  const groups: GroupLayout[] = ordered.map(([key, members], index) => {
    /* Hubs in the disc center: sort members by degree, place on a spiral. */
    const sorted = [...members].sort(
      (a, b) => b.in_degree + b.out_degree - (a.in_degree + a.out_degree) || a.id - b.id
    );
    const memberOffsets = sorted.map((m, i) => {
      const r = MEMBER_SPACING * Math.sqrt(i) * 0.95;
      const angle = i * GOLDEN_ANGLE;
      return {
        x: Math.cos(angle) * r,
        y: Math.sin(angle) * r,
        radius: memberRadius(m),
      };
    });
    const discRadius = MEMBER_SPACING * (Math.sqrt(sorted.length) + 1);
    return {
      key,
      label: key === UNGROUPED ? 'ungrouped' : (communityNames.get(key) ?? `community ${key}`),
      colorIndex: index % PALETTE_SIZE,
      members: sorted,
      memberOffsets,
      discRadius,
      superRadius: superRadius(sorted.length),
      cx: 0,
      cy: 0,
    };
  });

  /* Greedy spiral packing: each group takes the first spiral position where
     its footprint (disc or super-node, whichever is larger, plus margin)
     clears every already-placed group. Deterministic and overlap-free. */
  const placed: { x: number; y: number; r: number }[] = [];
  const footprint = (g: GroupLayout) => Math.max(g.discRadius, g.superRadius) + GROUP_MARGIN;
  for (const group of groups) {
    const r = footprint(group);
    let position = { x: 0, y: 0 };
    for (let i = 0; ; i++) {
      const spiralR = i === 0 ? 0 : 26 * Math.sqrt(i);
      const angle = i * GOLDEN_ANGLE;
      const candidate = { x: Math.cos(angle) * spiralR, y: Math.sin(angle) * spiralR };
      const collides = placed.some((p) => {
        const dx = p.x - candidate.x;
        const dy = p.y - candidate.y;
        return Math.hypot(dx, dy) < p.r + r;
      });
      if (!collides) {
        position = candidate;
        break;
      }
    }
    group.cx = position.x;
    group.cy = position.y;
    placed.push({ x: position.x, y: position.y, r });
  }

  const groupByKey = new Map<number, GroupLayout>();
  const groupOfSymbol = new Map<number, GroupLayout>();
  const memberPlacement = new Map<number, { x: number; y: number; radius: number }>();
  for (const group of groups) {
    groupByKey.set(group.key, group);
    group.members.forEach((m, i) => {
      groupOfSymbol.set(m.id, group);
      const offset = group.memberOffsets[i];
      if (!offset) return;
      memberPlacement.set(m.id, {
        x: group.cx + offset.x,
        y: group.cy + offset.y,
        radius: offset.radius,
      });
    });
  }

  return { groups, groupByKey, groupOfSymbol, memberPlacement };
}

/** Resolve the graph actually drawn for a given expansion state: expanded
 * communities contribute their members, collapsed ones a single super-node,
 * and every underlying call edge is re-ported onto those endpoints (parallel
 * edges aggregate; calls interior to a collapsed community disappear). */
export function visibleGraph(
  data: AtlasData,
  layout: AtlasLayout,
  expanded: ReadonlySet<number>
): VisibleGraph {
  const nodes: VisibleNode[] = [];
  const byKey = new Map<string, VisibleNode>();

  for (const group of layout.groups) {
    if (expanded.has(group.key)) {
      group.members.forEach((m) => {
        const placement = layout.memberPlacement.get(m.id);
        if (!placement) return;
        const visible: VisibleNode = {
          key: `n${m.id}`,
          kind: 'member',
          symbolId: m.id,
          communityKey: group.key,
          x: placement.x,
          y: placement.y,
          radius: placement.radius,
          colorIndex: group.colorIndex,
          label: m.name,
          memberCount: 1,
          node: m,
        };
        nodes.push(visible);
        byKey.set(visible.key, visible);
      });
    } else {
      const visible: VisibleNode = {
        key: `c${group.key}`,
        kind: 'community',
        symbolId: null,
        communityKey: group.key,
        x: group.cx,
        y: group.cy,
        radius: group.superRadius,
        colorIndex: group.colorIndex,
        label: group.label,
        memberCount: group.members.length,
        node: null,
      };
      nodes.push(visible);
      byKey.set(visible.key, visible);
    }
  }

  /* Port every call edge onto the visible endpoints and aggregate. */
  const portOf = (symbolId: number): string | null => {
    const group = layout.groupOfSymbol.get(symbolId);
    if (!group) return null;
    return expanded.has(group.key) ? `n${symbolId}` : `c${group.key}`;
  };

  const aggregated = new Map<string, { sourceKey: string; targetKey: string; weight: number }>();
  for (const edge of data.edges) {
    const sourceKey = portOf(edge.source);
    const targetKey = portOf(edge.target);
    if (!sourceKey || !targetKey || sourceKey === targetKey) continue;
    const id = `${sourceKey}→${targetKey}`;
    const existing = aggregated.get(id);
    if (existing) existing.weight += 1;
    else aggregated.set(id, { sourceKey, targetKey, weight: 1 });
  }

  const edges: VisibleEdge[] = [];
  const adjacency = new Map<string, Set<string>>();
  const link = (a: string, b: string) => {
    let set = adjacency.get(a);
    if (!set) {
      set = new Set();
      adjacency.set(a, set);
    }
    set.add(b);
  };
  for (const { sourceKey, targetKey, weight } of aggregated.values()) {
    const source = byKey.get(sourceKey);
    const target = byKey.get(targetKey);
    if (!source || !target) continue;
    edges.push({
      sourceKey,
      targetKey,
      weight,
      x1: source.x,
      y1: source.y,
      x2: target.x,
      y2: target.y,
      sourceColorIndex: source.colorIndex,
      targetColorIndex: target.colorIndex,
      targetRadius: target.radius,
    });
    link(sourceKey, targetKey);
    link(targetKey, sourceKey);
  }

  return { nodes, edges, byKey, adjacency };
}
