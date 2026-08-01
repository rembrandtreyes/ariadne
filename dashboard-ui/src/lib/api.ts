/** Typed fetch layer for the ariadne REST API.
 * Every view consumes endpoints through `Resource<T>` so pending, failed, and
 * ready states are impossible to skip (ISC-2/ISC-3). */

export type Resource<T> =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'ready'; data: T };

export async function fetchJson<T>(path: string): Promise<T> {
  const res = await fetch(path);
  if (!res.ok) {
    let message = `Request failed (${res.status})`;
    try {
      const body = await res.json();
      if (body && typeof body.message === 'string') message = body.message;
    } catch {
      /* non-JSON error body — keep the status message */
    }
    throw new Error(message);
  }
  return (await res.json()) as T;
}

/* ── Response shapes (mirror src/dashboard/api.rs) ── */

export interface MetricScores {
  dead_code: number | null;
  cycles: number | null;
  coupling: number | null;
  modularity: number | null;
}

export interface HealthReport {
  grade: string;
  score: number | null;
  dead_code_ratio: number | null;
  cycle_count: number | null;
  coupling_density: number | null;
  modularity_score: number | null;
  metric_scores: MetricScores;
  summary: string;
  degraded_fields: string[];
}

export interface Overview {
  health: HealthReport;
  files: number;
  symbols: number;
  calls: number;
  resolution_rate: number;
  dead_functions: number;
  languages: string[];
  parse_error_files: number;
  last_indexed: string | null;
}

export interface SymbolHealth {
  id: number;
  name: string;
  qualified_name: string;
  kind: string;
  file_path: string;
  line_start: number;
  line_end: number;
  is_dead: boolean;
  fan_in: number;
  fan_out: number;
  modification_count: number;
  author_count: number;
  is_volatile: boolean;
  has_history: boolean;
}

export interface ModuleFileSummary {
  name: string;
  symbol_count: number;
  dead_count: number;
  risk: number;
  health: number;
}

export interface ModuleSummary {
  name: string;
  path: string;
  symbol_count: number;
  file_count: number;
  health: number;
  risk: number;
  dead_count: number;
  cycle_count: number;
  god_objects: number;
  files: ModuleFileSummary[];
}

export interface ChurnEntry {
  id: number;
  name: string;
  qualified_name: string;
  kind: string;
  file: string;
  module: string;
  line_start: number;
  modification_count: number;
  author_count: number;
  is_volatile: boolean;
  last_modified_at: number | null;
}

export interface OverviewData {
  overview: Overview;
  hotspots: SymbolHealth[];
  modules: ModuleSummary[];
  churn: ChurnEntry[];
}

/* ── Atlas (P2) ── */

export interface AtlasNode {
  id: number;
  name: string;
  kind: string;
  module: string;
  community: number | null;
  in_degree: number;
  out_degree: number;
  is_dead: boolean;
}

export interface AtlasEdge {
  source: number;
  target: number;
}

export interface CommunityInfo {
  id: number;
  name: string;
  symbol_count: number;
  internal_edges: number;
  external_edges: number;
  modularity: number;
}

export interface AtlasData {
  nodes: AtlasNode[];
  edges: AtlasEdge[];
  communities: CommunityInfo[];
  total_symbols: number;
  truncated: boolean;
}

export function loadAtlasData(): Promise<AtlasData> {
  return fetchJson<AtlasData>('/api/atlas');
}

/* ── Symbol dossier (P2) ── */

export interface DescribeMetrics {
  fan_in: number;
  fan_out: number;
  modification_count: number;
  author_count: number;
  is_volatile: boolean;
  blast_radius: number;
  coupled_file_count: number;
  max_coupling_strength: number;
}

export interface DescribeResult {
  description: string;
  role: string;
  risk_level: string;
  risk_score: number;
  metrics: DescribeMetrics;
}

/** Node shape of the legacy graph endpoints (string ids). */
export interface NeighborNode {
  id: string;
  name: string;
  kind: string;
  file: string;
  in_degree: number;
  out_degree: number;
  is_dead: boolean;
  line_start: number;
  line_end: number;
  signature: string;
}

export interface NeighborhoodData {
  nodes: NeighborNode[];
  edges: { source: string; target: string; confidence: number }[];
}

export interface SourceData {
  code: string;
  line_start: number;
  line_end: number;
  line_count: number;
  language: string;
  file: string;
}

export interface SymbolCore {
  describe: DescribeResult;
  hood: NeighborhoodData;
}

/** Everything the dossier header/relations render, one round-trip. Source is
 * fetched separately so a moved file degrades one panel, not the dossier. */
export async function loadSymbolCore(id: number): Promise<SymbolCore> {
  const [describe, hood] = await Promise.all([
    fetchJson<DescribeResult>(`/api/describe?id=${id}`),
    fetchJson<NeighborhoodData>(`/api/graph/neighborhood?id=${id}&depth=1`),
  ]);
  return { describe, hood };
}

export function loadSource(id: number): Promise<SourceData> {
  return fetchJson<SourceData>(`/api/source?id=${id}&context=3`);
}

/* ── Search (P2) ── */

export interface SearchFacets {
  kinds: string[];
  languages: string[];
}

export interface SearchHit {
  name: string;
  qualified_name: string | null;
  symbol_id: number | null;
  kind: string;
  file: string;
  line: number;
  score: number;
  snippet: string | null;
  module: string;
}

export function loadSearchFacets(): Promise<SearchFacets> {
  return fetchJson<SearchFacets>('/api/search_facets');
}

export async function searchSymbols(
  q: string,
  kind: string,
  lang: string
): Promise<SearchHit[]> {
  const params = new URLSearchParams({ q, limit: '50' });
  if (kind) params.set('kind', kind);
  if (lang) params.set('lang', lang);
  const res = await fetchJson<{ results: SearchHit[] }>(`/api/symbol_search?${params}`);
  return res.results;
}

/** One round-trip for everything the Overview renders. */
export async function loadOverviewData(): Promise<OverviewData> {
  const [overview, hotspotsRes, modulesRes, churnRes] = await Promise.all([
    fetchJson<Overview>('/api/overview'),
    fetchJson<{ hotspots: SymbolHealth[] }>('/api/complexity_hotspots?limit=5'),
    fetchJson<{ modules: ModuleSummary[] }>('/api/modules'),
    fetchJson<{ entries: ChurnEntry[] }>('/api/churn?limit=8'),
  ]);
  return {
    overview,
    hotspots: hotspotsRes.hotspots,
    modules: modulesRes.modules,
    churn: churnRes.entries,
  };
}
