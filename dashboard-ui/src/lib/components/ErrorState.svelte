<script lang="ts">
  interface Props {
    message: string;
    onretry: () => void;
  }

  let { message, onretry }: Props = $props();
</script>

<!-- Visible failure with a retry control (ISC-3) — never a blank page. -->
<div class="error-state" role="alert">
  <p class="error-state__title serif">Couldn't reach the index</p>
  <p class="error-state__message">{message}</p>
  <button class="error-state__retry" onclick={onretry}>Retry</button>
</div>

<style>
  .error-state {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-5);
    background: var(--bg-card);
    border: 1px solid var(--border-strong);
    border-left: 3px solid var(--threshold-critical);
    border-radius: var(--radius-md);
  }
  .error-state__title {
    font-family: var(--font-serif);
    font-size: var(--text-xl);
  }
  .error-state__message {
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }
  .error-state__retry {
    padding: var(--space-2) var(--space-4);
    background: var(--bg-elevated);
    border: 1px solid var(--border-active);
    border-radius: var(--radius-sm);
    color: var(--accent);
    font-size: var(--text-sm);
    transition: background var(--transition-fast);
  }
  .error-state__retry:hover {
    background: var(--bg-hover);
  }
</style>
