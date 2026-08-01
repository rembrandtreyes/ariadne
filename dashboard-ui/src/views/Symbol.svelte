<script lang="ts">
  import {
    loadSymbolCore,
    loadSource,
    type NeighborNode,
    type Resource,
    type SourceData,
    type SymbolCore,
  } from '../lib/api';
  import { formatCount } from '../lib/format';
  import Skeleton from '../lib/components/Skeleton.svelte';
  import ErrorState from '../lib/components/ErrorState.svelte';

  let { id }: { id: number } = $props();

  let core = $state<Resource<SymbolCore>>({ status: 'loading' });
  let source = $state<Resource<SourceData>>({ status: 'loading' });

  async function loadCore() {
    core = { status: 'loading' };
    try {
      core = { status: 'ready', data: await loadSymbolCore(id) };
    } catch (e) {
      core = { status: 'error', message: e instanceof Error ? e.message : String(e) };
    }
  }
  async function loadSrc() {
    source = { status: 'loading' };
    try {
      source = { status: 'ready', data: await loadSource(id) };
    } catch (e) {
      source = { status: 'error', message: e instanceof Error ? e.message : String(e) };
    }
  }
  loadCore();
  loadSrc();

  /* The neighborhood includes the focal symbol; relations are only the edges
     that touch it (depth-1 fetch also returns neighbor↔neighbor edges). */
  const focal = $derived(
    core.status === 'ready'
      ? (core.data.hood.nodes.find((n) => n.id === String(id)) ?? null)
      : null
  );

  function relations(data: SymbolCore, direction: 'callers' | 'callees'): NeighborNode[] {
    const idStr = String(id);
    const byId = new Map(data.hood.nodes.map((n) => [n.id, n]));
    const ids =
      direction === 'callers'
        ? data.hood.edges.filter((e) => e.target === idStr).map((e) => e.source)
        : data.hood.edges.filter((e) => e.source === idStr).map((e) => e.target);
    const unique = [...new Set(ids)].filter((nid) => nid !== idStr);
    return unique
      .map((nid) => byId.get(nid))
      .filter((n): n is NeighborNode => n !== undefined)
      .sort((a, b) => a.name.localeCompare(b.name));
  }
  const callers = $derived(core.status === 'ready' ? relations(core.data, 'callers') : []);
  const callees = $derived(core.status === 'ready' ? relations(core.data, 'callees') : []);

  const RELATION_CAP = 30;

  /* Risk maps onto the shared threshold colors: low risk = healthy green. */
  const riskColors: Record<string, string> = {
    low: 'var(--threshold-healthy)',
    medium: 'var(--threshold-moderate)',
    high: 'var(--threshold-elevated)',
    critical: 'var(--threshold-critical)',
  };

  function sourceLines(src: SourceData): { line: number; text: string }[] {
    const first = Math.max(1, src.line_start - 3);
    return src.code.split('\n').map((text, i) => ({ line: first + i, text }));
  }
</script>

<div class="dossier">
  {#if core.status === 'loading'}
    <Skeleton height="48px" />
    <Skeleton height="18px" lines={3} />
    <Skeleton height="200px" />
  {:else if core.status === 'error'}
    <ErrorState message={core.message} onretry={loadCore} />
  {:else}
    {@const d = core.data.describe}

    <header class="dossier__header">
      <div class="dossier__title-row">
        <h2 class="dossier__name serif">{focal?.name ?? `symbol ${id}`}</h2>
        {#if focal}
          <span class="chip">{focal.kind}</span>
        {/if}
        <span class="chip" style="color: {riskColors[d.risk_level] ?? 'var(--text-secondary)'}; border-color: currentcolor">
          {d.risk_level} risk
        </span>
        {#if d.metrics.is_volatile}
          <span class="chip chip--critical">volatile</span>
        {/if}
        {#if focal?.is_dead}
          <span class="chip chip--critical">dead</span>
        {/if}
      </div>
      {#if focal}
        <p class="dossier__path mono">
          {focal.file}:{focal.line_start}–{focal.line_end}
        </p>
        {#if focal.signature}
          <p class="dossier__signature mono">{focal.signature}</p>
        {/if}
      {/if}
    </header>

    <section class="panel" aria-labelledby="describe-heading">
      <h3 id="describe-heading" class="panel__title serif">What this is</h3>
      <p class="dossier__narrative">{d.description}</p>
      <dl class="stat-strip">
        <div class="stat"><dt>callers in</dt><dd class="mono">{formatCount(d.metrics.fan_in)}</dd></div>
        <div class="stat"><dt>calls out</dt><dd class="mono">{formatCount(d.metrics.fan_out)}</dd></div>
        <div class="stat"><dt>blast radius</dt><dd class="mono">{formatCount(d.metrics.blast_radius)} symbols</dd></div>
        <div class="stat"><dt>modifications</dt><dd class="mono">{formatCount(d.metrics.modification_count)}</dd></div>
        <div class="stat"><dt>authors</dt><dd class="mono">{formatCount(d.metrics.author_count)}</dd></div>
        <div class="stat"><dt>coupled files</dt><dd class="mono">{formatCount(d.metrics.coupled_file_count)}</dd></div>
        <div class="stat"><dt>role</dt><dd>{d.role.replaceAll('_', ' ')}</dd></div>
      </dl>
    </section>

    <div class="dossier__relations">
      {#each [{ label: 'Called by', items: callers, empty: 'No known callers — entry point or unused.' }, { label: 'Depends on', items: callees, empty: 'Calls nothing the index resolves.' }] as column (column.label)}
        <section class="panel" aria-label={column.label}>
          <h3 class="panel__title serif">
            {column.label} <span class="dossier__count mono">{formatCount(column.items.length)}</span>
          </h3>
          {#if column.items.length === 0}
            <p class="calm">{column.empty}</p>
          {:else}
            <ul class="relation-list">
              {#each column.items.slice(0, RELATION_CAP) as n (n.id)}
                <li>
                  <a class="relation" href="#/symbol/{n.id}">
                    <span class="relation__name mono">{n.name}</span>
                    <span class="relation__meta">{n.kind} · {n.file}:{n.line_start}</span>
                  </a>
                </li>
              {/each}
            </ul>
            {#if column.items.length > RELATION_CAP}
              <p class="calm">+ {formatCount(column.items.length - RELATION_CAP)} more</p>
            {/if}
          {/if}
        </section>
      {/each}
    </div>

    <section class="panel" aria-labelledby="source-heading">
      <h3 id="source-heading" class="panel__title serif">Source</h3>
      {#if source.status === 'loading'}
        <Skeleton height="18px" lines={6} />
      {:else if source.status === 'error'}
        <p class="calm">Source unavailable: {source.message}</p>
      {:else}
        {@const src = source.data}
        <p class="dossier__source-meta mono">
          {src.file} · lines {src.line_start}–{src.line_end} · {src.language}
        </p>
        <div class="source-scroll">
          <pre class="source mono"><code>{#each sourceLines(src) as row (row.line)}<span
                class="source__line"
                class:source__line--focus={row.line >= src.line_start && row.line <= src.line_end}
              ><span class="source__num">{row.line}</span>{row.text}
</span>{/each}</code></pre>
        </div>
      {/if}
    </section>
  {/if}
</div>

<style>
  .dossier {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }
  .dossier__header {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .dossier__title-row {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    flex-wrap: wrap;
  }
  .dossier__name {
    font-family: var(--font-serif);
    font-size: var(--text-3xl);
    color: var(--accent);
    overflow-wrap: anywhere;
  }
  .dossier__path,
  .dossier__signature {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    overflow-wrap: anywhere;
  }
  .dossier__narrative {
    color: var(--text-primary);
    max-width: 80ch;
    margin-bottom: var(--space-4);
  }
  .dossier__count {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

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

  .stat-strip {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
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
    font-size: var(--text-base);
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
  .calm {
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .dossier__relations {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(360px, 100%), 1fr));
    gap: var(--space-5);
  }
  .relation-list {
    list-style: none;
    display: flex;
    flex-direction: column;
  }
  .relation {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--border-subtle);
    text-decoration: none;
  }
  .relation:hover {
    text-decoration: none;
    background: var(--bg-hover);
  }
  .relation__name {
    font-size: var(--text-sm);
    color: var(--accent);
  }
  .relation__meta {
    font-size: var(--text-xs);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }

  .dossier__source-meta {
    font-size: var(--text-xs);
    color: var(--text-muted);
    margin-bottom: var(--space-3);
  }
  .source-scroll {
    overflow-x: auto;
    background: var(--bg-void);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
  }
  .source {
    font-size: var(--text-sm);
    line-height: 1.6;
    padding: var(--space-4);
    min-width: max-content;
  }
  .source__line {
    display: block;
    white-space: pre;
    color: var(--text-secondary);
  }
  .source__line--focus {
    color: var(--text-primary);
    background: rgba(212, 168, 83, 0.05);
  }
  .source__num {
    display: inline-block;
    width: 4ch;
    margin-right: var(--space-4);
    text-align: right;
    color: var(--text-muted);
    user-select: none;
  }
</style>
