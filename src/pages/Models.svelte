<script lang="ts">
  import { t } from "../i18n";
  // ianox — models page
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import type { ModelInfo } from "../types";
  import PageHeader from "../components/PageHeader.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";

  let models = $state<ModelInfo[]>([]);
  let loading = $state(true);
  let search = $state("");
  let selectedProvider = $state("all");

  onMount(() => {
    let unlisten: UnlistenFn | null = null;

    invoke<ModelInfo[]>("list_models")
      .then((data) => (models = data))
      .catch((e) => console.error("Error cargando modelos:", e))
      .finally(() => (loading = false));

    listen<ModelInfo[]>("models://updated", (e) => (models = e.payload))
      .then((fn) => (unlisten = fn))
      .catch(() => {});

    return () => unlisten?.();
  });

  const providers = $derived([...new Set(models.map((m) => m.provider))]);

  const filteredModels = $derived(
    models.filter((m) => {
      const q = search.toLowerCase();
      const matchesSearch = m.id.toLowerCase().includes(q) || m.provider.toLowerCase().includes(q);
      const matchesProvider = selectedProvider === "all" || m.provider === selectedProvider;
      return matchesSearch && matchesProvider;
    })
  );

  const kindColors: Record<string, string> = {
    openai: "var(--accent-primary)",
    anthropic: "var(--warning)",
    openrouter: "var(--accent-secondary)",
    ollama: "var(--success)",
    google: "var(--danger)",
    azure: "var(--accent-primary)",
    custom: "var(--text-muted)",
  };

  const fmtSize = (size: number | null) => {
    if (!size) return "—";
    if (size >= 1_000_000_000) return `${(size / 1_000_000_000).toFixed(1)} GB`;
    if (size >= 1_000_000) return `${(size / 1_000_000).toFixed(0)} MB`;
    return `${size} B`;
  };
</script>

<div class="ui-page">
  <PageHeader title={t("Models")} subtitle={t("All models exposed by your active providers.")}>
    {#if loading}
      <span class="animate-pulse" style="font-size:12px;color:var(--accent-primary)">{t("Loading…")}</span>
    {/if}
  </PageHeader>

  <div class="panel-card">
    <div class="panel-card-bar">
      <div class="toolbar" style="flex:1">
        <div class="searchbox">
          <span class="searchbox-icon"><Icon name="search" size={15} /></span>
          <input class="input" type="text" placeholder={t("Search models…")} bind:value={search} spellcheck="false" />
        </div>
        <select class="input" bind:value={selectedProvider} style="height:30px;min-width:170px">
          <option value="all">{t("All providers")}</option>
          {#each providers as p (p)}<option value={p}>{p}</option>{/each}
        </select>
      </div>
      <span class="panel-card-count">{filteredModels.length} {t("models")}</span>
    </div>

    <div class="panel-card-body">
      {#if filteredModels.length === 0}
        <EmptyState
          icon="models"
          title={models.length === 0 ? t("No models configured") : t("No matching models")}
          desc={models.length === 0
            ? t("Add a provider and its models will show up here.")
            : t("Try another search or change the provider filter.")}
        />
      {:else}
        <div class="list">
          {#each filteredModels as model (`${model.provider}-${model.id}`)}
            {@const color = kindColors[model.provider_kind.toLowerCase()] || "var(--text-muted)"}
            <div class="list-item">
              <div class="list-row">
                <div class="row-icon" style={`color:${color}`}><Icon name="models" size={15} /></div>
                <div class="row-main">
                  <div class="row-title"><span class="truncate">{model.id}</span></div>
                  <div class="row-sub">{model.provider_kind}</div>
                </div>
                <div class="row-meta">
                  <span class="badge" style={`background:${color}20;color:${color};font-size:10px`}>
                    {model.provider}
                  </span>
                  <span style="min-width:52px;text-align:right">{fmtSize(model.size)}</span>
                </div>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</div>
