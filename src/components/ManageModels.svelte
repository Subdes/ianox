<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { t } from "../i18n";

  let { onchanged }: { onchanged: () => void } = $props();

  interface CatalogModel {
    id: string;
    name: string;
    context: number | null;
    free: boolean;
  }
  interface ProviderInfo {
    id: string;
    name: string;
    models: string[];
    enabled: boolean;
  }
  interface Group {
    provider: ProviderInfo;
    enabled: Set<string>;
    catalog: CatalogModel[];
  }

  const LIMIT = 100;

  let groups = $state<Group[]>([]);
  let loading = $state(true);
  let query = $state("");
  let input: HTMLInputElement | undefined = $state();

  onMount(async () => {
    input?.focus();
    try {
      const providers = (await invoke<ProviderInfo[]>("list_providers")).filter((p) => p.enabled);
      const loaded: Group[] = [];
      for (const provider of providers) {
        const catalog = await invoke<CatalogModel[]>("list_catalog_models", { providerId: provider.id }).catch(
          () => [] as CatalogModel[]
        );
        loaded.push({ provider, enabled: new Set(provider.models), catalog });
      }
      groups = loaded;
    } finally {
      loading = false;
    }
  });

  const needle = $derived(query.trim().toLowerCase());
  const matches = (name: string, id: string) =>
    !needle || name.toLowerCase().includes(needle) || id.toLowerCase().includes(needle);

  /** Configured models the catalog does not know (custom or renamed) stay manageable. */
  function rows(group: Group): CatalogModel[] {
    const known = new Set(group.catalog.map((m) => m.id));
    const custom = [...group.enabled]
      .filter((id) => !known.has(id))
      .map((id) => ({ id, name: id, context: null, free: false }));
    return [...custom, ...group.catalog].filter((m) => matches(m.name, m.id));
  }

  async function toggle(group: Group, id: string): Promise<void> {
    const next = new Set(group.enabled);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    group.enabled = next;
    groups = [...groups];
    try {
      await invoke("set_provider_models", { providerId: group.provider.id, models: [...next] });
      onchanged();
    } catch (e) {
      console.error("Could not save the models:", e);
    }
  }

  const fmtContext = (n: number | null) =>
    !n ? "" : n >= 1_000_000 ? `${+(n / 1_000_000).toFixed(1)}M` : `${Math.round(n / 1000)}K`;
</script>

<div class="mm-search">
  <input bind:this={input} bind:value={query} placeholder={t("Search models")} spellcheck="false" />
</div>
<div class="mm-list">
  {#if loading}
    <div class="mm-empty">{t("Loading…")}</div>
  {:else if groups.length === 0}
    <div class="mm-empty">{t("No providers enabled")}</div>
  {/if}
  {#each groups as group (group.provider.id)}
    {@const list = rows(group)}
    <div class="mm-group">{group.provider.name}</div>
    {#if group.catalog.length === 0}
      <div class="mm-note">{t("The catalog has no list for this provider; add its models by hand in Providers.")}</div>
    {/if}
    {#each list.slice(0, LIMIT) as model (model.id)}
      <label class="mm-row">
        <input type="checkbox" checked={group.enabled.has(model.id)} onchange={() => toggle(group, model.id)} />
        <span class="mm-name">{model.name}</span>
        {#if model.free}<span class="mm-free">{t("Free")}</span>{/if}
        <span class="mm-ctx">{fmtContext(model.context)}</span>
      </label>
    {/each}
    {#if list.length > LIMIT}
      <div class="mm-note">{t("{n} more — keep typing to narrow", { n: String(list.length - LIMIT) })}</div>
    {/if}
  {/each}
</div>

<style>
  .mm-search input {
    width: 100%;
    box-sizing: border-box;
    border: none;
    border-bottom: 1px solid var(--border-primary);
    background: transparent;
    color: var(--text-primary);
    padding: 12px 16px;
    font-size: 14px;
    outline: none;
  }
  .mm-list {
    max-height: min(56vh, 480px);
    overflow: auto;
    padding: 4px;
  }
  .mm-group {
    position: sticky;
    top: -4px;
    z-index: 1;
    background: var(--bg-secondary);
    padding: 10px 10px 4px;
    font-size: 11px;
    color: var(--text-muted);
  }
  .mm-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13.5px;
    color: var(--text-primary);
  }
  .mm-row:hover {
    background: var(--bg-hover);
  }
  .mm-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mm-ctx,
  .mm-note,
  .mm-empty {
    color: var(--text-muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .mm-note,
  .mm-empty {
    padding: 8px 10px;
  }
  .mm-free {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    border: 1px solid var(--border-primary);
  }
</style>
