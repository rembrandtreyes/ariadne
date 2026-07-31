<script lang="ts">
  import { thresholdColor, thresholdBand, bandLabels } from '../threshold';

  interface Props {
    /** 0–100 score; color always derives from the shared threshold scale. */
    value: number;
    /** Accessible name for the meter. */
    label: string;
    /** Show the numeric value next to the bar. */
    showValue?: boolean;
    size?: 'sm' | 'md';
  }

  let { value, label, showValue = false, size = 'md' }: Props = $props();

  const clamped = $derived(Math.max(0, Math.min(100, value)));
  const color = $derived(thresholdColor(clamped));
  const band = $derived(bandLabels[thresholdBand(clamped)]);
</script>

<!-- The single bar/meter family (ISC-16). Semantics via role="meter" so the
     value is announced, not just painted. -->
<div class="meter meter--{size}">
  <div
    class="meter__track"
    role="meter"
    aria-label={label}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={clamped}
    aria-valuetext="{clamped} of 100 — {band}"
  >
    <div class="meter__fill" style="width: {clamped}%; background: {color}"></div>
  </div>
  {#if showValue}
    <span class="meter__value mono" style="color: {color}">{clamped}</span>
  {/if}
</div>

<style>
  .meter {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .meter__track {
    flex: 1;
    background: var(--bg-elevated);
    border-radius: 999px;
    overflow: hidden;
  }
  .meter--md .meter__track {
    height: 6px;
  }
  .meter--sm .meter__track {
    height: 4px;
  }
  .meter__fill {
    height: 100%;
    border-radius: 999px;
    transition: width var(--transition-normal);
  }
  .meter__value {
    font-size: var(--text-xs);
    min-width: 3ch;
    text-align: right;
  }
</style>
