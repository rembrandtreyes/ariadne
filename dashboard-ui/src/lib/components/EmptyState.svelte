<script lang="ts">
  /* The single guided empty state (ISC-4): a fresh/empty index gets one clear
     instruction, not five headed empty sections. Stateless by design. */
</script>

<div class="empty-state">
  <p class="empty-state__title serif">Nothing indexed yet</p>
  <p class="empty-state__body">
    Ariadne hasn't analyzed a codebase. Run this in the repository you want to map:
  </p>
  <code class="empty-state__command mono">ariadne index</code>
  <p class="empty-state__hint">
    Then reload this page — the overview fills in as soon as the index exists.
  </p>
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-8) var(--space-5);
    text-align: center;
  }
  .empty-state__title {
    font-family: var(--font-serif);
    font-size: var(--text-3xl);
    color: var(--accent);
  }
  .empty-state__body {
    color: var(--text-secondary);
    max-width: 44ch;
  }
  .empty-state__command {
    padding: var(--space-2) var(--space-4);
    background: var(--bg-card);
    border: 1px solid var(--border-active);
    border-radius: var(--radius-sm);
    color: var(--accent-bright);
    font-size: var(--text-lg);
  }
  .empty-state__hint {
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
</style>
