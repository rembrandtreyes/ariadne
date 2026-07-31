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
