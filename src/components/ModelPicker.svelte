<script lang="ts">
  import { t } from "../i18n";
  import { navigate } from "../app.svelte";
  import Icon from "./Icon.svelte";
  import ManageModels from "./ManageModels.svelte";
  import type { ModelInfo } from "../types";

  let {
    models,
    selected,
    onselect,
    onclose,
    onchanged,
  }: {
    models: ModelInfo[];
    selected: { provider: string; id: string } | null;
    onselect: (model: ModelInfo) => void;
    onclose: () => void;
    /** The set of available models changed (manage view). */
    onchanged: () => void;
  } = $props();

  let mode = $state<"pick" | "manage">("pick");

  type Ref = { provider: string; id: string };
  const FAV_KEY = "ianox.model.favorites";
  const DEFAULT_KEY = "ianox.model.default";
  const RECENT_KEY = "ianox.model.recent";

  function load(key: string): Ref[] {
    try {
      const raw = JSON.parse(localStorage.getItem(key) ?? "[]");
      return Array.isArray(raw) ? raw : [];
    } catch {
      return [];
    }
  }

  const same = (a: Ref, b: Ref) => a.provider === b.provider && a.id === b.id;

  let query = $state("");
  let active = $state(0);
  let favorites = $state<Ref[]>(load(FAV_KEY));
  const recent = load(RECENT_KEY);
  let defaultModel = $state<Ref | null>(load(DEFAULT_KEY)[0] ?? null);
  let input: HTMLInputElement | undefined = $state();

  $effect(() => {
    input?.focus();
  });

  function toggleFavorite(model: ModelInfo, event: Event): void {
    event.stopPropagation();
    const ref = { provider: model.provider, id: model.id };
    favorites = favorites.some((f) => same(f, ref))
      ? favorites.filter((f) => !same(f, ref))
      : [...favorites, ref];
    try {
      localStorage.setItem(FAV_KEY, JSON.stringify(favorites));
    } catch {}
  }

  const isDefault = (model: ModelInfo) => !!defaultModel && same(defaultModel, model);

  /** The model new conversations start with. */
  function setDefault(model: ModelInfo, event: Event): void {
    event.stopPropagation();
    defaultModel = { provider: model.provider, id: model.id };
    try {
      localStorage.setItem(DEFAULT_KEY, JSON.stringify([defaultModel]));
    } catch {}
  }

  function openProviders(): void {
    onclose();
    navigate("providers");
  }

  function pick(model: ModelInfo): void {
    const ref = { provider: model.provider, id: model.id };
    try {
      const next = [ref, ...recent.filter((r) => !same(r, ref))].slice(0, 5);
      localStorage.setItem(RECENT_KEY, JSON.stringify(next));
    } catch {}
    onselect(model);
  }

  function formatContext(tokens: number | null): string {
    if (!tokens) return "";
    if (tokens >= 1_000_000) return `${+(tokens / 1_000_000).toFixed(1)}M`;
    return `${Math.round(tokens / 1000)}K`;
  }

  const MAX_RESULTS = 200;
  const find = (ref: Ref) => models.find((m) => same(m, ref));

  /** Flat, ordered list as rendered: Favorites, Recent, then one group per provider. */
  const groups = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    const matches = (m: ModelInfo) =>
      !needle ||
      m.name.toLowerCase().includes(needle) ||
      m.id.toLowerCase().includes(needle) ||
      m.provider_name.toLowerCase().includes(needle);

    const seen = new Set<string>();
    const key = (m: ModelInfo) => `${m.provider}/${m.id}`;
    const out: { title: string; items: ModelInfo[]; inline: boolean }[] = [];
    const take = (title: string, list: (ModelInfo | undefined)[], inline = false) => {
      const items = list
        .filter((m): m is ModelInfo => !!m && matches(m) && !seen.has(key(m)))
        .slice(0, MAX_RESULTS);
      items.forEach((m) => seen.add(key(m)));
      if (items.length) out.push({ title, items, inline });
    };

    if (!needle) {
      take(t("Favorites"), favorites.map(find), true);
      take(t("Recent"), recent.map(find), true);
    }
    const byProvider = new Map<string, ModelInfo[]>();
    for (const m of models) {
      const list = byProvider.get(m.provider_name) ?? [];
      list.push(m);
      byProvider.set(m.provider_name, list);
    }
    for (const [name, list] of byProvider) take(name, list);
    return out;
  });

  const flat = $derived(groups.flatMap((g) => g.items));
  $effect(() => {
    query;
    active = 0;
  });

  function onKey(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      active = Math.min(active + 1, flat.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      active = Math.max(active - 1, 0);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (flat[active]) pick(flat[active]);
    }
  }

  $effect(() => {
    active;
    document.querySelector(".mp-row.active")?.scrollIntoView({ block: "nearest" });
  });
</script>

<div class="mp-overlay" role="presentation" onclick={onclose}></div>
<div class="model-picker" role="dialog" aria-label={t("Select model")}>
  {#if mode === "pick"}
    <div class="mp-bar">
      <Icon name="search" size={16} />
      <input
        bind:this={input}
        bind:value={query}
        onkeydown={onKey}
        placeholder={t("Search models")}
        spellcheck="false"
      />
      <button type="button" class="mp-tool" title={t("Connect provider")} onclick={openProviders}>
        <Icon name="plus" size={16} />
      </button>
      <button type="button" class="mp-tool" title={t("Manage models")} onclick={() => (mode = "manage")}>
        <Icon name="settings" size={16} />
      </button>
    </div>
    <div class="mp-list">
      {#if flat.length === 0}
        <div class="menu-empty">{models.length === 0 ? t("No models configured") : t("No results")}</div>
      {/if}
      {#each groups as group (group.title)}
        <div class="mp-group">{group.title}</div>
        {#each group.items as m (`${group.title}/${m.provider}/${m.id}`)}
          {@const index = flat.indexOf(m)}
          {@const isFav = favorites.some((f) => same(f, m))}
          <div
            class="mp-row"
            class:active={index === active}
            class:current={selected && same(selected, m)}
            role="option"
            tabindex="-1"
            aria-selected={!!selected && same(selected, m)}
            onclick={() => pick(m)}
            onkeydown={() => {}}
            onmousemove={() => (active = index)}
          >
            <span class="mp-name">{m.name}</span>
            {#if group.inline || query.trim()}<span class="menu-sub">{m.provider_name}</span>{/if}
            {#if m.free}<span class="mp-badge">{t("Free")}</span>{/if}
            <span class="mp-spacer"></span>
            {#if isDefault(m)}
              <span class="mp-default">{t("Default")}</span>
            {:else}
              <button type="button" class="mp-setdefault" onclick={(e) => setDefault(m, e)}>{t("Set as default")}</button>
            {/if}
            {#if m.context}<span class="menu-sub mp-ctx">{formatContext(m.context)}</span>{/if}
            <button
              type="button"
              class="mp-star"
              class:on={isFav}
              title={isFav ? t("Remove from favorites") : t("Add to favorites")}
              onclick={(e) => toggleFavorite(m, e)}
            >
              <Icon name="star" size={12} />
            </button>
          </div>
        {/each}
      {/each}
    </div>
  {:else}
    <div class="mp-bar">
      <button type="button" class="mp-tool" title={t("Back")} onclick={() => (mode = "pick")}>
        <Icon name="chevron-left" size={16} />
      </button>
      <strong class="mp-title">{t("Manage models")}</strong>
    </div>
    <ManageModels {onchanged} />
  {/if}
</div>

<style>
  .mp-overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    background: rgba(0, 0, 0, 0.45);
  }
  .model-picker {
    position: fixed;
    top: 14vh;
    left: 50%;
    transform: translateX(-50%);
    z-index: 1001;
    width: min(600px, calc(100vw - 32px));
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    overflow: hidden;
  }
  .mp-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px 0 16px;
    border-bottom: 1px solid var(--border-primary);
    color: var(--text-muted);
  }
  .mp-bar input {
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    color: var(--text-primary);
    padding: 14px 0;
    font-size: 14px;
    outline: none;
  }
  .mp-title {
    padding: 14px 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .mp-tool {
    display: inline-flex;
    padding: 6px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }
  .mp-tool:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .mp-spacer {
    flex: 1;
  }
  .mp-badge {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    border: 1px solid var(--border-primary);
  }
  .mp-default {
    font-size: 12px;
    color: var(--text-secondary);
  }
  .mp-setdefault {
    border: none;
    background: none;
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
    opacity: 0;
    padding: 0 2px;
  }
  .mp-row:hover .mp-setdefault,
  .mp-row.active .mp-setdefault {
    opacity: 1;
  }
  .mp-setdefault:hover {
    color: var(--text-primary);
    text-decoration: underline;
  }
  .mp-list {
    max-height: min(56vh, 480px);
    overflow: auto;
    padding: 4px;
  }
  .mp-group {
    position: sticky;
    top: -4px;
    z-index: 1;
    background: var(--bg-secondary);
    padding: 8px 8px 3px;
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }
  .mp-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 6px;
    font-size: 12.5px;
    color: var(--text-secondary);
    cursor: pointer;
  }
  .mp-row.active {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .mp-row.current .mp-name::after {
    content: " ✓";
    color: var(--text-muted);
  }
  .mp-ctx {
    font-variant-numeric: tabular-nums;
  }
  .mp-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mp-star {
    border: none;
    background: transparent;
    color: var(--text-muted);
    opacity: 0;
    cursor: pointer;
    padding: 2px;
  }
  .mp-row:hover .mp-star,
  .mp-row.active .mp-star,
  .mp-star.on {
    opacity: 1;
  }
  .mp-star.on {
    color: #e8b931;
  }
</style>
