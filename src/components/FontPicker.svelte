<script lang="ts">
  import { t } from "../i18n";

  let {
    label,
    value = $bindable(""),
    fonts,
    fallback,
  }: { label: string; value: string; fonts: string[]; fallback: string } = $props();

  let open = $state(false);
  let query = $state("");
  let search: HTMLInputElement | undefined = $state();

  /** Rows rendered at once; the search narrows the rest. */
  const LIMIT = 80;

  const needle = $derived(query.trim().toLowerCase());
  const matches = $derived(
    needle ? fonts.filter((name) => name.toLowerCase().includes(needle)) : fonts
  );
  const shown = $derived(matches.slice(0, LIMIT));
  /** A name that is not in the list can still be used (the list may miss some fonts). */
  const custom = $derived(
    needle && !fonts.some((name) => name.toLowerCase() === needle) ? query.trim() : ""
  );

  function toggle(): void {
    open = !open;
    query = "";
    if (open) queueMicrotask(() => search?.focus());
  }

  function choose(name: string): void {
    value = name;
    open = false;
    query = "";
  }

  function onKey(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.stopPropagation();
      open = false;
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (shown[0]) choose(shown[0]);
      else if (custom) choose(custom);
    }
  }
</script>

<div class="field">
  <span class="label">{label}</span>
  <button type="button" class="current" onclick={toggle} aria-expanded={open}>
    <span class="name" style:font-family={value ? `"${value}"` : undefined}>{value || fallback}</span>
    <span class="chev">{open ? "▴" : "▾"}</span>
  </button>

  {#if open}
    <div class="menu">
      <input
        bind:this={search}
        bind:value={query}
        onkeydown={onKey}
        placeholder={t("Search fonts")}
        spellcheck="false"
      />
      <div class="list">
        <button type="button" class="row" class:on={!value} onclick={() => choose("")}>
          {t("Default")} <span class="hint">{fallback}</span>
        </button>
        {#if custom}
          <button type="button" class="row" onclick={() => choose(custom)}>
            {t("Use")} “{custom}”
          </button>
        {/if}
        {#each shown as name (name)}
          <button
            type="button"
            class="row"
            class:on={name === value}
            style:font-family={`"${name}"`}
            onclick={() => choose(name)}
          >
            {name}
          </button>
        {/each}
        {#if matches.length > LIMIT}
          <div class="more">{t("{n} more — keep typing to narrow", { n: String(matches.length - LIMIT) })}</div>
        {/if}
        {#if fonts.length === 0 && !custom}
          <div class="more">{t("Type a font name")}</div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .label {
    color: var(--text-secondary);
  }
  .current {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    cursor: pointer;
    font: inherit;
    text-align: left;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chev {
    color: var(--text-muted);
  }
  .menu {
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    background: var(--bg-primary);
    overflow: hidden;
  }
  input {
    width: 100%;
    box-sizing: border-box;
    border: none;
    border-bottom: 1px solid var(--border-primary);
    background: transparent;
    color: var(--text-primary);
    padding: 7px 8px;
    font: inherit;
    outline: none;
  }
  .list {
    max-height: 190px;
    overflow: auto;
    padding: 3px;
  }
  .row {
    display: flex;
    justify-content: space-between;
    width: 100%;
    border: none;
    background: transparent;
    color: var(--text-primary);
    padding: 5px 6px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
    text-align: left;
  }
  .row:hover {
    background: var(--bg-hover);
  }
  .row.on {
    background: rgba(30, 95, 255, 0.16);
  }
  .hint,
  .more {
    color: var(--text-muted);
    font-size: 11px;
  }
  .more {
    padding: 6px;
  }
</style>
