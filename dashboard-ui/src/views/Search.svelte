<script lang="ts">
  import {
    loadSearchFacets,
    searchSymbols,
    type Resource,
    type SearchFacets,
    type SearchHit,
  } from '../lib/api';
  import { formatCount } from '../lib/format';
  import Skeleton from '../lib/components/Skeleton.svelte';
  import ErrorState from '../lib/components/ErrorState.svelte';

  /* Query state seeds from the URL so a search deep-link reloads intact
     (ISC-10); edits sync back via replaceState so typing doesn't spam
     history. */
  const initial = new URLSearchParams(window.location.hash.split('?')[1] ?? '');
  let q = $state(initial.get('q') ?? '');
  let kind = $state(initial.get('kind') ?? '');
  let lang = $state(initial.get('lang') ?? '');

  let facets = $state<Resource<SearchFacets>>({ status: 'loading' });
  let results = $state<Resource<SearchHit[]>>({ status: 'ready', data: [] });

  async function loadFacets() {
    facets = { status: 'loading' };
    try {
      facets = { status: 'ready', data: await loadSearchFacets() };
    } catch (e) {
      facets = { status: 'error', message: e instanceof Error ? e.message : String(e) };
    }
  }
  loadFacets();

  function syncUrl() {
    const params = new URLSearchParams();
    if (q) params.set('q', q);
    if (kind) params.set('kind', kind);
    if (lang) params.set('lang', lang);
    const query = params.toString();
    history.replaceState(null, '', query ? `#/search?${query}` : '#/search');
  }

  let requestSeq = 0;
  async function runSearch() {
    syncUrl();
    if (!q.trim()) {
      results = { status: 'ready', data: [] };
      return;
    }
    const seq = ++requestSeq;
    results = { status: 'loading' };
    try {
      const hits = await searchSymbols(q.trim(), kind, lang);
      if (seq === requestSeq) results = { status: 'ready', data: hits };
    } catch (e) {
      if (seq === requestSeq)
        results = { status: 'error', message: e instanceof Error ? e.message : String(e) };
    }
  }

  let debounceTimer: ReturnType<typeof setTimeout> | undefined;
  function onInput() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(runSearch, 250);
  }
  $effect(() => () => clearTimeout(debounceTimer));

  if ((initial.get('q') ?? '').trim()) runSearch();
</script>

<div class="search">
  <header class="search__bar">
    <h2 class="search__title serif">Search</h2>
    <div class="search__controls">
      <input
        class="search__input mono"
        type="search"
        placeholder="Symbol name…"
        aria-label="Search symbols"
        bind:value={q}
        oninput={onInput}
      />
      {#if facets.status === 'ready'}
        <label class="search__filter">
          <span>kind</span>
          <select bind:value={kind} onchange={runSearch} aria-label="Filter by kind">
            <option value="">any</option>
            {#each facets.data.kinds as k (k)}
              <option value={k}>{k}</option>
            {/each}
          </select>
        </label>
        <label class="search__filter">
          <span>language</span>
          <select bind:value={lang} onchange={runSearch} aria-label="Filter by language">
            <option value="">any</option>
            {#each facets.data.languages as l (l)}
              <option value={l}>{l}</option>
            {/each}
          </select>
        </label>
      {:else if facets.status === 'error'}
        <span class="search__facet-error">filters unavailable</span>
      {/if}
    </div>
  </header>

  {#if results.status === 'loading'}
    <Skeleton height="52px" lines={5} />
  {:else if results.status === 'error'}
    <ErrorState message={results.message} onretry={runSearch} />
  {:else if !q.trim()}
    <p class="search__hint">
      Full-text search over every indexed symbol. Results link straight to the symbol's
      dossier.
    </p>
  {:else if results.data.length === 0}
    <p class="search__hint">No symbols match “{q.trim()}”{kind ? ` with kind ${kind}` : ''}{lang ? ` in ${lang}` : ''}.</p>
  {:else}
    <p class="search__count">{formatCount(results.data.length)} result{results.data.length === 1 ? '' : 's'}</p>
    <ol class="results">
      {#each results.data as hit (hit.symbol_id ?? `${hit.file}:${hit.line}`)}
        <li>
          {#if hit.symbol_id !== null}
            <a class="result" href="#/symbol/{hit.symbol_id}">
              <span class="result__head">
                <span class="result__name mono">{hit.name}</span>
                <span class="chip">{hit.kind}</span>
              </span>
              <span class="result__path">{hit.module} · {hit.file}:{hit.line}</span>
              {#if hit.snippet}
                <code class="result__snippet mono">{hit.snippet}</code>
              {/if}
            </a>
          {:else}
            <div class="result">
              <span class="result__head">
                <span class="result__name mono">{hit.name}</span>
                <span class="chip">{hit.kind}</span>
              </span>
              <span class="result__path">{hit.module} · {hit.file}:{hit.line}</span>
            </div>
          {/if}
        </li>
      {/each}
    </ol>
  {/if}
</div>

<style>
  .search {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .search__title {
    font-family: var(--font-serif);
    font-size: var(--text-xl);
    color: var(--accent);
    margin-bottom: var(--space-3);
  }
  .search__controls {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    flex-wrap: wrap;
  }
  .search__input {
    flex: 1;
    min-width: 260px;
    padding: var(--space-3) var(--space-4);
    font-size: var(--text-base);
    color: var(--text-primary);
    background: var(--bg-surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
  }
  .search__input::placeholder {
    color: var(--text-muted);
  }
  .search__filter {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }
  .search__filter select {
    font: inherit;
    text-transform: none;
    letter-spacing: normal;
    font-size: var(--text-sm);
    color: var(--text-primary);
    background: var(--bg-elevated);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    padding: var(--space-1) var(--space-2);
  }
  .search__facet-error {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .search__hint {
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }
  .search__count {
    color: var(--text-muted);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .results {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .result {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-4);
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    text-decoration: none;
  }
  a.result:hover {
    text-decoration: none;
    border-color: var(--border-active);
    background: var(--bg-hover);
  }
  .result__head {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
  }
  .result__name {
    font-size: var(--text-base);
    color: var(--text-primary);
  }
  .result__path {
    font-size: var(--text-xs);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
  .result__snippet {
    margin-top: var(--space-1);
    font-size: var(--text-xs);
    color: var(--text-secondary);
    background: var(--bg-void);
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-sm);
    overflow-wrap: anywhere;
  }
  .chip {
    font-size: var(--text-xs);
    padding: 2px var(--space-2);
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 999px;
    color: var(--text-secondary);
  }
</style>
