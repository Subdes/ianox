<script lang="ts">
  import { t } from "../i18n";
  // ianox — Gateway: usage, providers and models
  import { invoke } from "@tauri-apps/api/core";
  import type { UsageSummary } from "../types";
  import Providers from "./Providers.svelte";
  import Routes from "./Routes.svelte";
  import AreaChart from "../components/charts/AreaChart.svelte";
  import BarChart from "../components/charts/BarChart.svelte";
  import PageHeader from "../components/PageHeader.svelte";

  const COLORS = ["#1e5fff", "#14b8a6", "#f59e0b", "#ef4444", "#8b5cf6", "#06b6d4", "#10b981", "#f97316"];

  const fmtTokens = (n: number) => {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
    return String(n || 0);
  };

  const truncate = (s: string, max = 22) => (s.length > max ? `${s.slice(0, max)}…` : s);

  const fmtCost = (usd: number | null) =>
    usd === null ? "—" : `${usd.toFixed(usd > 0 && usd < 0.01 ? 4 : 2).replace(".", ",")} US$`;

  type ViewMode = "tokens" | "requests";

  let tab = $state<"uso" | "proveedores" | "rutas">("uso");

  let days = $state(7);
  let usage = $state<UsageSummary | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let mode = $state<ViewMode>("tokens");
  let providerMode = $state<ViewMode>("tokens");
  let modelMode = $state<ViewMode>("tokens");

  async function load(period: number): Promise<void> {
    loading = true;
    error = null;
    try {
      usage = await invoke<UsageSummary>("get_usage", { days: period });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  let loadedDays: number | null = null;
  $effect(() => {
    const d = days;
    if (d === loadedDays) return;
    loadedDays = d;
    load(d);
  });

  const hasData = $derived((usage?.total_requests ?? 0) > 0);

  const areaData = $derived((usage?.days ?? []).map((d) => ({ label: d.label, value: d[mode] })));
  const areaFormatter = $derived(mode === "tokens" ? fmtTokens : (n: number) => String(n || 0));
  const areaColor = $derived(mode === "tokens" ? "#1e5fff" : "#14b8a6");
  const areaGradId = $derived(mode === "tokens" ? "gradUsageTokens" : "gradUsageRequests");
  const areaHasData = $derived(areaData.some((d) => d.value > 0));

  const providerData = $derived(
    (usage?.by_provider ?? [])
      .map((p) => ({ label: truncate(p.id, 10), value: p[providerMode] }))
      .filter((p) => p.value > 0)
  );

  const modelData = $derived(
    (usage?.by_model ?? [])
      .map((m) => ({ label: truncate(m.model), value: m[modelMode] }))
      .filter((m) => m.value > 0)
      .slice(0, 5)
  );

  const segments = (current: ViewMode) =>
    (["tokens", "requests"] as ViewMode[]).map((value) => ({
      value,
      label: value === "tokens" ? t("Tokens") : t("Requests"),
      active: current === value,
    }));
</script>

<div style="display:flex;flex-direction:column;gap:16px;height:100%;min-height:0">
  <PageHeader title={t("Gateway")} subtitle={t("Internal router, providers and routes in one place.")}>
    <div class="segmented">
      <button class={tab === "uso" ? "active" : ""} onclick={() => (tab = "uso")}>{t("Usage")}</button>
      <button class={tab === "proveedores" ? "active" : ""} onclick={() => (tab = "proveedores")}>{t("Providers")}</button>
      <button class={tab === "rutas" ? "active" : ""} onclick={() => (tab = "rutas")}>{t("Routes")}</button>
    </div>
  </PageHeader>

  <div style="flex:1;min-height:0;display:flex;flex-direction:column">
    {#if tab === "uso"}
      <div style="display:flex;flex-direction:column;gap:16px;height:100%;overflow:auto">
        <div style="display:flex;align-items:center;justify-content:flex-end;flex-wrap:wrap;gap:12px">
          <div style="display:flex;gap:6px">
            {#each [7, 30] as period (period)}
              <button
                class={`btn btn-sm ${days === period ? "btn-primary" : "btn-secondary"}`}
                onclick={() => (days = period)}
              >
                {period} {t("days")}
              </button>
            {/each}
            <button class="btn btn-secondary btn-sm" onclick={() => load(days)} disabled={loading}>
              {loading ? t("Loading…") : t("Refresh")}
            </button>
          </div>
        </div>

        {#if error}<div class="error-banner"><span>⚠ {error}</span></div>{/if}

        <div class="gateway-stats">
          <div class="card gateway-stat">
            <div class="gateway-stat-value">{usage ? String(usage.total_requests) : "—"}</div>
            <div class="gateway-stat-label">{t("Requests")}</div>
          </div>
          <div class="card gateway-stat">
            <div class="gateway-stat-value">{usage ? fmtTokens(usage.total_tokens) : "—"}</div>
            <div class="gateway-stat-label">{t(t("Tokens"))}</div>
          </div>
          <div class="card gateway-stat">
            <div class="gateway-stat-value">{usage ? String(usage.by_provider.length) : "—"}</div>
            <div class="gateway-stat-label">{t("Providers")}</div>
          </div>
          <div class="card gateway-stat">
            <div class="gateway-stat-value">{usage ? String(usage.by_model.length) : "—"}</div>
            <div class="gateway-stat-label">{t("Models")}</div>
          </div>
          <div class="card gateway-stat">
            <div class="gateway-stat-value">{usage ? fmtCost(usage.total_cost_usd) : "—"}</div>
            <div class="gateway-stat-label">{t("Cost")}</div>
          </div>
        </div>

        {#if !hasData}
          <div
            class="card"
            style="flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;color:var(--text-muted);gap:8px;padding:40px"
          >
            <p style="font-size:14px">{t("No usage recorded yet")}</p>
            <p style="font-size:12px">{t("Every chat request is logged here automatically.")}</p>
          </div>
        {:else}
          <div class="card" style="padding:16px">
            <div style="display:flex;flex-direction:column;gap:12px">
              <div style="display:flex;align-items:center;justify-content:space-between;gap:12px">
                <span
                  style="font-size:12px;font-weight:600;color:var(--text-secondary);text-transform:uppercase;letter-spacing:0.05em"
                >
                  {t("Usage over time")}
                </span>
                <div class="segmented">
                  {#each segments(mode) as seg (seg.value)}
                    <button class={seg.active ? "active" : ""} onclick={() => (mode = seg.value)}>{seg.label}</button>
                  {/each}
                </div>
              </div>

              {#if !areaHasData}
                <div
                  style="height:220px;display:flex;align-items:center;justify-content:center;color:var(--text-muted);font-size:13px"
                >
                  {t("No activity in this period")}
                </div>
              {:else}
                <AreaChart data={areaData} color={areaColor} format={areaFormatter} gradId={areaGradId} />
              {/if}
            </div>
          </div>

          <div class="gateway-charts">
            <div class="card" style="padding:16px">
              <div style="display:flex;flex-direction:column;gap:12px">
                <div style="display:flex;align-items:center;justify-content:space-between;gap:8px">
                  <span
                    style="font-size:12px;font-weight:600;color:var(--text-secondary);text-transform:uppercase;letter-spacing:0.05em"
                  >
                    {t("By provider")}
                  </span>
                  <div class="segmented">
                    {#each segments(providerMode) as seg (seg.value)}
                      <button class={seg.active ? "active" : ""} onclick={() => (providerMode = seg.value)}>
                        {seg.label}
                      </button>
                    {/each}
                  </div>
                </div>
                {#if providerData.length === 0}
                  <div
                    style="height:180px;display:flex;align-items:center;justify-content:center;color:var(--text-muted);font-size:13px"
                  >
                    {t("No usage by provider")}
                  </div>
                {:else}
                  <BarChart data={providerData} colors={COLORS} format={providerMode === "tokens" ? fmtTokens : (n) => String(n || 0)} height={180} />
                {/if}
              </div>
            </div>

            <div class="card" style="padding:16px">
              <div style="display:flex;flex-direction:column;gap:12px">
                <div style="display:flex;align-items:center;justify-content:space-between;gap:8px">
                  <span
                    style="font-size:12px;font-weight:600;color:var(--text-secondary);text-transform:uppercase;letter-spacing:0.05em"
                  >
                    {t("Top models")}
                  </span>
                  <div class="segmented">
                    {#each segments(modelMode) as seg (seg.value)}
                      <button class={seg.active ? "active" : ""} onclick={() => (modelMode = seg.value)}>{seg.label}</button>
                    {/each}
                  </div>
                </div>
                {#if modelData.length === 0}
                  <div
                    style="height:180px;display:flex;align-items:center;justify-content:center;color:var(--text-muted);font-size:13px"
                  >
                    {t("No model usage")}
                  </div>
                {:else}
                  <BarChart
                    data={modelData}
                    colors={[COLORS[2], COLORS[3], COLORS[4], COLORS[5], COLORS[6]]}
                    format={modelMode === "tokens" ? fmtTokens : (n) => String(n || 0)}
                    height={180}
                    horizontal
                  />
                {/if}
              </div>
            </div>
          </div>
        {/if}
      </div>
    {:else if tab === "proveedores"}
      <Providers compact />
    {:else}
      <Routes compact />
    {/if}
  </div>
</div>
