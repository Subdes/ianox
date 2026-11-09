<script lang="ts">
  import { t } from "../../i18n";
  // ianox — pure SVG bar chart, vertical or horizontal
  let {
    data,
    colors = ["#1e5fff"],
    format = (n: number) => String(n),
    height = 180,
    horizontal = false,
  }: {
    data: Array<{ label: string; value: number }>;
    colors?: string[];
    format?: (n: number) => string;
    height?: number;
    horizontal?: boolean;
  } = $props();

  let width = $state(640);
  let hover = $state<number | null>(null);

  const padT = 8;
  const padB = 24;
  const padL = $derived(horizontal ? 110 : 44);
  const padR = $derived(horizontal ? 30 : 10);

  const innerW = $derived(Math.max(width - padL - padR, 10));
  const innerH = $derived(Math.max(height - padT - padB, 10));
  const n = $derived(data.length);

  function niceCeil(v: number): number {
    if (v <= 0) return 1;
    const exp = Math.pow(10, Math.floor(Math.log10(v)));
    const f = v / exp;
    const nf = f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10;
    return nf * exp;
  }

  const niceMax = $derived(niceCeil(Math.max(1, ...data.map((d) => d.value))));
  const band = $derived(n > 0 ? innerH / n : innerH);
  const vband = $derived(n > 0 ? innerW / n : innerW);
  const gridVals = $derived([0, 0.5, 1].map((f) => niceMax * f));
</script>

<div class="chart" bind:clientWidth={width} style={`height:${height}px`}>
  <svg {width} {height} role="img" aria-label={t("Bar chart")}>
    {#if horizontal}
      {#each gridVals as g}
        <line
          x1={padL + (g / niceMax) * innerW}
          x2={padL + (g / niceMax) * innerW}
          y1={padT}
          y2={padT + innerH}
          stroke="currentColor"
          stroke-opacity="0.1"
          stroke-dasharray="3 3"
        />
        <text
          x={padL + (g / niceMax) * innerW}
          y={height - 8}
          text-anchor="middle"
          font-size="10"
          fill="currentColor"
          fill-opacity="0.5">{format(g)}</text
        >
      {/each}

      {#each data as d, i}
        <text
          x={padL - 10}
          y={padT + i * band + band / 2 + 3}
          text-anchor="end"
          font-size="10"
          fill="currentColor"
          fill-opacity="0.7">{d.label}</text
        >
        <rect
          x={padL}
          y={padT + i * band + band * 0.2}
          width={Math.max((d.value / niceMax) * innerW, 1)}
          height={band * 0.6}
          rx="4"
          fill={colors[i % colors.length]}
          fill-opacity={hover === i ? 1 : 0.85}
          role="presentation"
          onmouseenter={() => (hover = i)}
          onmouseleave={() => (hover = null)}
        />
      {/each}
    {:else}
      {#each gridVals as g}
        <line
          x1={padL}
          x2={padL + innerW}
          y1={padT + innerH * (1 - g / niceMax)}
          y2={padT + innerH * (1 - g / niceMax)}
          stroke="currentColor"
          stroke-opacity="0.1"
          stroke-dasharray="3 3"
        />
        <text
          x={padL - 8}
          y={padT + innerH * (1 - g / niceMax) + 3}
          text-anchor="end"
          font-size="10"
          fill="currentColor"
          fill-opacity="0.5">{format(g)}</text
        >
      {/each}

      {#each data as d, i}
        <rect
          x={padL + i * vband + vband * 0.2}
          y={padT + innerH * (1 - d.value / niceMax)}
          width={Math.max(vband * 0.6, 2)}
          height={Math.max((d.value / niceMax) * innerH, 0)}
          rx="4"
          fill={colors[i % colors.length]}
          fill-opacity={hover === i ? 1 : 0.85}
          role="presentation"
          onmouseenter={() => (hover = i)}
          onmouseleave={() => (hover = null)}
        />
        <text
          x={padL + i * vband + vband / 2}
          y={height - 8}
          text-anchor="middle"
          font-size="10"
          fill="currentColor"
          fill-opacity="0.6">{d.label}</text
        >
      {/each}
    {/if}
  </svg>

  {#if hover !== null && data[hover]}
    <div
      class="chart-tip"
      style={`left:${horizontal ? Math.min(padL + 140, Math.max(width - 70, 70)) : Math.min(padL + hover * vband + vband / 2, Math.max(width - 60, 60))}px`}
    >
      <span class="chart-tip-label">{data[hover].label}</span>
      <span class="chart-tip-value">{format(data[hover].value)}</span>
    </div>
  {/if}
</div>
