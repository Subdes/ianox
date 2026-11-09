<script lang="ts">
  import { t } from "../../i18n";
  // ianox — pure SVG area chart (no chart library)
  let {
    data,
    color = "#1e5fff",
    format = (n: number) => String(n),
    height = 220,
    gradId,
  }: {
    data: Array<{ label: string; value: number }>;
    color?: string;
    format?: (n: number) => string;
    height?: number;
    gradId: string;
  } = $props();

  let width = $state(640);
  let hover = $state<number | null>(null);

  const padL = 46;
  const padR = 12;
  const padT = 10;
  const padB = 22;

  const innerW = $derived(Math.max(width - padL - padR, 10));
  const innerH = $derived(Math.max(height - padT - padB, 10));
  const n = $derived(data.length);
  const maxVal = $derived(Math.max(1, ...data.map((d) => d.value)));
  const niceMax = $derived(niceCeil(maxVal));
  const step = $derived(n > 1 ? innerW / (n - 1) : 0);

  const ptX = (i: number) => padL + (n > 1 ? i * step : innerW / 2);
  const ptY = (v: number) => padT + innerH * (1 - v / niceMax);

  const linePath = $derived(
    data
      .map((d, i) => `${i === 0 ? "M" : "L"}${ptX(i).toFixed(2)} ${ptY(d.value).toFixed(2)}`)
      .join(" ")
  );
  const areaPath = $derived(
    n === 0
      ? ""
      : `${linePath} L${ptX(n - 1).toFixed(2)} ${(padT + innerH).toFixed(2)} L${ptX(0).toFixed(2)} ${(padT + innerH).toFixed(2)} Z`
  );

  const gridVals = $derived([0, 0.25, 0.5, 0.75, 1].map((f) => niceMax * f));
  const xIdx = $derived(pickIndices(n, 7));

  function niceCeil(v: number): number {
    if (v <= 0) return 1;
    const exp = Math.pow(10, Math.floor(Math.log10(v)));
    const f = v / exp;
    const nf = f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10;
    return nf * exp;
  }

  function pickIndices(count: number, max: number): number[] {
    if (count === 0) return [];
    if (count <= max) return Array.from({ length: count }, (_, i) => i);
    const out: number[] = [];
    for (let k = 0; k < max; k++) out.push(Math.round((k * (count - 1)) / (max - 1)));
    return [...new Set(out)];
  }
</script>

<div class="chart" bind:clientWidth={width} style={`height:${height}px`}>
  <svg {width} {height} role="img" aria-label={t("Usage chart")}>
    <defs>
      <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
        <stop offset="5%" stop-color={color} stop-opacity="0.25" />
        <stop offset="95%" stop-color={color} stop-opacity="0" />
      </linearGradient>
    </defs>

    {#each gridVals as g}
      <line
        x1={padL}
        x2={padL + innerW}
        y1={ptY(g)}
        y2={ptY(g)}
        stroke="currentColor"
        stroke-opacity="0.1"
        stroke-dasharray="3 3"
      />
      <text
        x={padL - 8}
        y={ptY(g) + 3}
        text-anchor="end"
        font-size="10"
        fill="currentColor"
        fill-opacity="0.5">{format(g)}</text
      >
    {/each}

    {#if n > 0}
      <path d={areaPath} fill={`url(#${gradId})`} />
      <path d={linePath} fill="none" stroke={color} stroke-width="2" />
    {/if}

    {#each data as d, i}
      {#if hover === i}
        <line
          x1={ptX(i)}
          x2={ptX(i)}
          y1={padT}
          y2={padT + innerH}
          stroke="currentColor"
          stroke-opacity="0.15"
        />
        <circle cx={ptX(i)} cy={ptY(d.value)} r="4" fill={color} />
      {/if}
    {/each}

    {#each xIdx as i}
      <text
        x={ptX(i)}
        y={height - 6}
        text-anchor="middle"
        font-size="10"
        fill="currentColor"
        fill-opacity="0.5">{data[i]?.label ?? ""}</text
      >
    {/each}

    {#each data as d, i}
      <rect
        x={ptX(i) - step / 2}
        y={padT}
        width={Math.max(step, 1)}
        height={innerH}
        fill="transparent"
        role="presentation"
        onmouseenter={() => (hover = i)}
        onmouseleave={() => (hover = null)}
      />
    {/each}
  </svg>

  {#if hover !== null && data[hover]}
    <div
      class="chart-tip"
      style={`left:${Math.min(Math.max(ptX(hover), 60), Math.max(width - 60, 60))}px`}
    >
      <span class="chart-tip-label">{data[hover].label}</span>
      <span class="chart-tip-value">{format(data[hover].value)}</span>
    </div>
  {/if}
</div>
