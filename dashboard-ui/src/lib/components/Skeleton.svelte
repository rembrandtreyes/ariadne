<script lang="ts">
  interface Props {
    /** CSS height of the placeholder block. */
    height?: string;
    /** Number of stacked placeholder lines. */
    lines?: number;
  }

  let { height = '16px', lines = 1 }: Props = $props();
</script>

<!-- Loading placeholder (ISC-2): shown while a section's fetch is pending. -->
<div class="skeleton-group" aria-hidden="true">
  {#each Array(lines) as _, i (i)}
    <div class="skeleton" style="height: {height}"></div>
  {/each}
</div>

<style>
  .skeleton-group {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .skeleton {
    border-radius: var(--radius-sm);
    background: linear-gradient(
      100deg,
      var(--bg-elevated) 40%,
      var(--bg-hover) 50%,
      var(--bg-elevated) 60%
    );
    background-size: 200% 100%;
    animation: shimmer 1.4s infinite linear;
  }
  @keyframes shimmer {
    from {
      background-position: 120% 0;
    }
    to {
      background-position: -80% 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .skeleton {
      animation: none;
    }
  }
</style>
