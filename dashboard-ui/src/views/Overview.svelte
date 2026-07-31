<script lang="ts">
  import { loadOverviewData, type OverviewData, type Resource } from '../lib/api';
  import { thresholdColor, thresholdBand, bandLabels } from '../lib/threshold';
  import { formatCount, formatPercent, formatTimestamp, formatIsoDate } from '../lib/format';
  import Meter from '../lib/components/Meter.svelte';
  import Skeleton from '../lib/components/Skeleton.svelte';
  import ErrorState from '../lib/components/ErrorState.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';

  let resource = $state<Resource<OverviewData>>({ status: 'loading' });
  let breakdownOpen = $state(false);

  async function load() {
    resource = { status: 'loading' };
    try {
      resource = { status: 'ready', data: await loadOverviewData() };
    } catch (e) {
      resource = { status: 'error', message: e instanceof Error ? e.message : String(e) };
    }
  }

  load();

  /* Server score drives everything; no client-side health formula (ISC-13). */
  const score = $derived(
    resource.status === 'ready' ? Math.round(resource.data.overview.health.score ?? 0) : 0
  );
  const heroColor = $derived(thresholdColor(score));
  const heroBand = $derived(bandLabels[thresholdBand(score)]);

  /* Adaptive risk framing (ISC-15): a healthy codebase gets calm framing,
     not a wall of LOW-risk cards. Grade comes from the server. */
  const calmRisks = $derived(
    resource.status === 'ready' &&
      ['A', 'B'].includes(resource.data.overview.health.grade) &&
      !resource.data.hotspots.some((h) => h.is_volatile)
  );

  interface MetricRow {
    label: string;
    score: number | null;
    detail: string;
  }

  const metricRows = $derived.by((): MetricRow[] => {
    if (resource.status !== 'ready') return [];
    const h = resource.data.overview.health;
    return [
      {
        label: 'Dead code',
        score: h.metric_scores.dead_code,
        detail:
          h.dead_code_ratio !== null
            ? `${formatPercent(h.dead_code_ratio)} of symbols unreachable`
            : 'unavailable',
      },
      {
        label: 'Cycles',
        score: h.metric_scores.cycles,
        detail:
          h.cycle_count !== null
            ? `${formatCount(h.cycle_count)} circular dependency ${h.cycle_count === 1 ? 'group' : 'groups'}`
            : 'unavailable',
      },
      {
        label: 'Coupling',
        score: h.metric_scores.coupling,
        detail:
          h.coupling_density !== null
            ? `${h.coupling_density.toFixed(2)} avg strength, top pairs`
            : 'unavailable',
      },
      {
        label: 'Modularity',
        score: h.metric_scores.modularity,
        detail:
          h.modularity_score !== null
            ? `${h.modularity_score.toFixed(2)} community modularity`
            : 'unavailable',
      },
    ];
  });

  function fileName(path: string): string {
    return path.split('/').pop() ?? path;
  }
</script>

<div class="overview">
  {#if resource.status === 'loading'}
    <section class="hero" aria-busy="true">
      <Skeleton height="120px" />
      <Skeleton height="16px" lines={2} />
    </section>
    <section class="panel"><Skeleton height="72px" /></section>
    <section class="panel"><Skeleton height="18px" lines={5} /></section>
    <section class="panel"><Skeleton height="18px" lines={4} /></section>
  {:else if resource.status === 'error'}
    <ErrorState message={resource.message} onretry={load} />
  {:else if resource.data.overview.symbols === 0}
    <EmptyState />
  {:else}
    {@const data = resource.data}
    {@const ov = data.overview}

    <!-- ── Hero: server-computed health ── -->
    <section class="hero" aria-labelledby="health-heading">
      <h2 id="health-heading" class="visually-hidden">Codebase health</h2>
      <div class="hero__score-block">
        <span class="hero__score serif" style="color: {heroColor}">{score}</span>
        <span class="hero__scale">health / 100 · grade {ov.health.grade}</span>
        <span class="hero__band" style="color: {heroColor}">{heroBand}</span>
      </div>
      <p class="hero__summary">{ov.health.summary}</p>

      <!-- Breakdown on interaction (ISC-12) -->
      <button
        class="hero__breakdown-toggle"
        aria-expanded={breakdownOpen}
        onclick={() => (breakdownOpen = !breakdownOpen)}
      >
        {breakdownOpen ? 'Hide' : 'Show'} score breakdown
      </button>
      {#if breakdownOpen}
        <dl class="hero__metrics">
          {#each metricRows as row (row.label)}
            <div class="hero__metric">
              <dt>{row.label}</dt>
              <dd>
                {#if row.score !== null}
                  <Meter value={row.score} label="{row.label} score" showValue size="sm" />
                {/if}
                <span class="hero__metric-detail">{row.detail}</span>
              </dd>
            </div>
          {/each}
        </dl>
      {/if}
    </section>

    <!-- ── Index health (ISC-29) ── -->
    <section class="panel" aria-labelledby="index-heading">
      <h2 id="index-heading" class="panel__title serif">Index</h2>
      <dl class="stat-strip">
        <div class="stat"><dt>files</dt><dd class="mono">{formatCount(ov.files)}</dd></div>
        <div class="stat"><dt>symbols</dt><dd class="mono">{formatCount(ov.symbols)}</dd></div>
        <div class="stat"><dt>calls</dt><dd class="mono">{formatCount(ov.calls)}</dd></div>
        <div class="stat">
          <dt>resolution</dt>
          <dd class="mono">{formatPercent(ov.resolution_rate)}</dd>
        </div>
        <div class="stat">
          <dt>parse errors</dt>
          <dd class="mono" class:stat--warn={ov.parse_error_files > 0}>
            {formatCount(ov.parse_error_files)} files
          </dd>
        </div>
        <div class="stat">
          <dt>dead symbols</dt>
          <dd class="mono">{formatCount(ov.dead_functions)}</dd>
        </div>
        <div class="stat"><dt>languages</dt><dd>{ov.languages.join(', ') || 'none'}</dd></div>
        <div class="stat"><dt>last indexed</dt><dd>{formatIsoDate(ov.last_indexed)}</dd></div>
      </dl>
    </section>

    <!-- ── Top risks (ISC-14/15) ── -->
    <section class="panel" aria-labelledby="risks-heading">
      <h2 id="risks-heading" class="panel__title serif">Where the risk lives</h2>
      {#if calmRisks}
        <p class="calm">
          No volatile hotspots. The most connected symbols are listed below — worth knowing,
          not worth worrying about.
        </p>
      {/if}
      {#if data.hotspots.length === 0}
        <p class="calm">No complexity hotspots detected.</p>
      {:else}
        <ol class="risk-list">
          {#each data.hotspots as h (h.id)}
            <li class="risk-card">
              <div class="risk-card__head">
                <span class="risk-card__name mono">{h.name}</span>
                <span class="risk-card__kind">{h.kind}</span>
                {#if h.is_volatile}
                  <span class="chip chip--critical">volatile</span>
                {/if}
              </div>
              <div class="risk-card__path mono">{h.file_path}:{h.line_start}</div>
              <div class="risk-card__chips">
                <span class="chip">{h.fan_in} callers in</span>
                <span class="chip">{h.fan_out} calls out</span>
                {#if h.has_history}
                  <span class="chip">{h.modification_count} modifications</span>
                  <span class="chip">{h.author_count} authors</span>
                {/if}
              </div>
            </li>
          {/each}
        </ol>
      {/if}
    </section>

    <!-- ── Modules (server-derived identity, ISC-17) ── -->
    <section class="panel" aria-labelledby="modules-heading">
      <h2 id="modules-heading" class="panel__title serif">Modules</h2>
      <div class="module-grid">
        {#each data.modules as m (m.name)}
          <article class="module-card" aria-label="module {m.name}">
            <div class="module-card__head">
              <span class="module-card__name mono">{m.name}</span>
              <span class="module-card__files">{formatCount(m.file_count)} files</span>
            </div>
            <div class="module-card__stats">
              <span>{formatCount(m.symbol_count)} symbols</span>
              <span>{formatCount(m.dead_count)} dead</span>
              {#if m.cycle_count > 0}<span>{formatCount(m.cycle_count)} cycles</span>{/if}
              {#if m.god_objects > 0}<span>{formatCount(m.god_objects)} god objects</span>{/if}
            </div>
            <Meter value={Math.round(m.health * 100)} label="{m.name} health" showValue />
          </article>
        {/each}
      </div>
    </section>

    <!-- ── Churn ranking (ISC-26 endpoint, compact surface) ── -->
    <section class="panel" aria-labelledby="churn-heading">
      <h2 id="churn-heading" class="panel__title serif">Most modified</h2>
      {#if data.churn.length === 0}
        <p class="calm">No git history captured — index with git available to see churn.</p>
      {:else}
        <ol class="churn-list">
          {#each data.churn as c (c.id)}
            <li class="churn-row">
              <span class="churn-row__name mono">{c.name}</span>
              <span class="churn-row__file">{c.module} · {fileName(c.file)}</span>
              <span class="churn-row__mods mono">
                {formatCount(c.modification_count)} mods · {c.author_count}
                {c.author_count === 1 ? 'author' : 'authors'}
                {#if c.last_modified_at}· {formatTimestamp(c.last_modified_at)}{/if}
              </span>
              {#if c.is_volatile}<span class="chip chip--critical">volatile</span>{/if}
            </li>
          {/each}
        </ol>
      {/if}
    </section>
  {/if}
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }

  /* ── Hero ── */
  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-7) var(--space-5) var(--space-5);
    text-align: center;
  }
  .hero__score-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-1);
  }
  .hero__score {
    font-family: var(--font-serif);
    font-size: var(--text-display);
    line-height: var(--leading-tight);
  }
  .hero__scale {
    color: var(--text-muted);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    white-space: nowrap;
  }
  .hero__band {
    font-size: var(--text-lg);
  }
  .hero__summary {
    color: var(--text-secondary);
    max-width: 60ch;
  }
  .hero__breakdown-toggle {
    color: var(--accent);
    font-size: var(--text-sm);
    padding: var(--space-1) var(--space-2);
    white-space: nowrap;
  }
  .hero__breakdown-toggle:hover {
    color: var(--accent-bright);
  }
  .hero__metrics {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: var(--space-4);
    width: 100%;
    max-width: 900px;
    text-align: left;
    padding: var(--space-4);
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
  }
  .hero__metric dt {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    margin-bottom: var(--space-2);
  }
  .hero__metric-detail {
    display: block;
    margin-top: var(--space-2);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  /* ── Panels ── */
  .panel {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    padding: var(--space-5);
  }
  .panel__title {
    font-family: var(--font-serif);
    font-size: var(--text-xl);
    color: var(--accent);
    margin-bottom: var(--space-4);
  }

  /* ── Index strip ── */
  .stat-strip {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    gap: var(--space-4);
  }
  .stat dt {
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    margin-bottom: var(--space-1);
  }
  .stat dd {
    font-size: var(--text-lg);
  }
  .stat--warn {
    color: var(--threshold-elevated);
  }

  /* ── Risks ── */
  .calm {
    color: var(--text-secondary);
    font-size: var(--text-sm);
    margin-bottom: var(--space-3);
  }
  .risk-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .risk-card {
    padding: var(--space-4);
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .risk-card__head {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    flex-wrap: wrap;
  }
  .risk-card__name {
    font-size: var(--text-base);
    color: var(--text-primary);
  }
  .risk-card__kind {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .risk-card__path {
    font-size: var(--text-xs);
    color: var(--text-secondary);
    overflow-wrap: anywhere;
  }
  .risk-card__chips {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
  }
  .chip {
    font-size: var(--text-xs);
    padding: 2px var(--space-2);
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 999px;
    color: var(--text-secondary);
  }
  .chip--critical {
    color: var(--threshold-critical);
    border-color: var(--threshold-critical);
  }

  /* ── Modules ── */
  .module-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: var(--space-4);
  }
  .module-card {
    padding: var(--space-4);
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .module-card__head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: var(--space-2);
  }
  .module-card__name {
    font-size: var(--text-base);
  }
  .module-card__files {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .module-card__stats {
    display: flex;
    gap: var(--space-3);
    flex-wrap: wrap;
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }

  /* ── Churn ── */
  .churn-list {
    list-style: none;
    display: flex;
    flex-direction: column;
  }
  .churn-row {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--border-subtle);
    flex-wrap: wrap;
  }
  .churn-row:last-child {
    border-bottom: none;
  }
  .churn-row__name {
    font-size: var(--text-sm);
  }
  .churn-row__file {
    font-size: var(--text-xs);
    color: var(--text-muted);
    flex: 1;
  }
  .churn-row__mods {
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }
</style>
