<script lang="ts">
  import { t } from "../i18n";

  let {
    context,
    limit,
    total,
    requests,
    cost,
    formatTokens,
    formatCost,
  }: {
    /** Tokens in the conversation after the last request. */
    context: number | null;
    /** Context window of the model, when known. */
    limit: number | null;
    /** Accumulated tokens of every request (used when the context size is unknown). */
    total: number;
    requests: number;
    cost: number | null;
    formatTokens: (n: number) => string;
    formatCost: (usd: number) => string;
  } = $props();

  const fraction = $derived(context && limit ? Math.min(1, context / limit) : null);
  const percent = $derived(fraction === null ? null : Math.round(fraction * 100));

  // Ring geometry
  const R = 6;
  const C = 2 * Math.PI * R;

  const shown = $derived(context ?? total);
  const title = $derived(
    context && limit
      ? t("{used} of {limit} tokens of context ({percent}%) · {n} requests", {
          used: context.toLocaleString(),
          limit: limit.toLocaleString(),
          percent: String(percent),
          n: String(requests),
        })
      : t("{used} tokens · {n} requests", { used: total.toLocaleString(), n: String(requests) })
  );
</script>

<span class="meter" {title}>
  {#if fraction !== null}
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="8" cy="8" r={R} class="track" />
      <circle
        cx="8"
        cy="8"
        r={R}
        class="arc"
        class:warn={fraction >= 0.8}
        stroke-dasharray={C}
        stroke-dashoffset={C * (1 - fraction)}
        transform="rotate(-90 8 8)"
      />
    </svg>
  {/if}
  <span class="text">
    {formatTokens(shown)}{#if cost !== null}<span class="sep">·</span>{formatCost(cost)}{/if}
  </span>
</span>

<style>
  .meter {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .track {
    fill: none;
    stroke: var(--border-secondary);
    stroke-width: 2;
  }
  .arc {
    fill: none;
    stroke: var(--text-secondary);
    stroke-width: 2;
    stroke-linecap: round;
    transition: stroke-dashoffset 0.4s ease;
  }
  .arc.warn {
    stroke: #e8a13a;
  }
  .sep {
    margin: 0 6px;
    color: var(--text-muted);
  }
</style>
