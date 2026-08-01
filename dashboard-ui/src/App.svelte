<script lang="ts">
  import { currentRoute, onRouteChange } from './lib/router';
  import Overview from './views/Overview.svelte';
  import Atlas from './views/Atlas.svelte';
  import Search from './views/Search.svelte';
  import Symbol from './views/Symbol.svelte';

  let route = $state(currentRoute());

  $effect(() => onRouteChange((r) => (route = r)));

  /* Views own their query params; the shell routes on the path alone so
     param edits (search text, expanded clusters) never remount a view. */
  const path = $derived(route.split('?')[0] ?? route);
  const symbolId = $derived(
    path.startsWith('/symbol/') ? Number(path.slice('/symbol/'.length)) : null
  );
</script>

<div class="shell">
  <header class="top-bar">
    <a href="#/" class="top-bar__wordmark serif">Ariadne</a>
    <nav class="top-bar__nav" aria-label="views">
      <a href="#/" class="top-bar__link" aria-current={path === '/' ? 'page' : undefined}>
        Overview
      </a>
      <a
        href="#/atlas"
        class="top-bar__link"
        aria-current={path === '/atlas' ? 'page' : undefined}
      >
        Atlas
      </a>
      <a
        href="#/search"
        class="top-bar__link"
        aria-current={path === '/search' ? 'page' : undefined}
      >
        Search
      </a>
    </nav>
  </header>

  <main class="content">
    {#if path === '/atlas'}
      <Atlas />
    {:else if path === '/search'}
      <Search />
    {:else if symbolId !== null && Number.isInteger(symbolId)}
      {#key route}
        <Symbol id={symbolId} />
      {/key}
    {:else if symbolId !== null}
      <p class="not-found">No symbol with id “{path.slice('/symbol/'.length)}”. <a href="#/search">Search instead.</a></p>
    {:else}
      <Overview />
    {/if}
  </main>
</div>

<style>
  .shell {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
  }
  .top-bar {
    position: sticky;
    top: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: var(--space-6);
    height: 56px;
    padding: 0 var(--space-5);
    background: rgba(6, 8, 12, 0.85);
    backdrop-filter: blur(12px);
    border-bottom: 1px solid var(--border-subtle);
  }
  .top-bar__wordmark {
    font-family: var(--font-serif);
    font-size: var(--text-xl);
    color: var(--accent);
  }
  .top-bar__wordmark:hover {
    text-decoration: none;
    color: var(--accent-bright);
  }
  .top-bar__nav {
    display: flex;
    gap: var(--space-4);
  }
  .top-bar__link {
    color: var(--text-secondary);
    font-size: var(--text-sm);
    padding: var(--space-1) var(--space-2);
  }
  .top-bar__link:hover {
    color: var(--text-primary);
    text-decoration: none;
  }
  .top-bar__link[aria-current='page'] {
    color: var(--accent);
  }
  .content {
    flex: 1;
    width: 100%;
    max-width: 1280px;
    margin: 0 auto;
    padding: var(--space-5);
  }
  .not-found {
    color: var(--text-secondary);
  }
</style>
