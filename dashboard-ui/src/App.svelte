<script lang="ts">
  import { currentRoute, onRouteChange } from './lib/router';
  import Overview from './views/Overview.svelte';
  import Spike from './views/Spike.svelte';

  let route = $state(currentRoute());

  $effect(() => onRouteChange((r) => (route = r)));
</script>

<div class="shell">
  <header class="top-bar">
    <a href="#/" class="top-bar__wordmark serif">Ariadne</a>
    <nav class="top-bar__nav" aria-label="views">
      <a href="#/" class="top-bar__link" aria-current={route === '/' ? 'page' : undefined}>
        Overview
      </a>
      <a
        href="#/spike"
        class="top-bar__link"
        aria-current={route === '/spike' ? 'page' : undefined}
      >
        Renderer spike
      </a>
    </nav>
  </header>

  <main class="content">
    {#if route === '/spike'}
      <Spike />
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
</style>
