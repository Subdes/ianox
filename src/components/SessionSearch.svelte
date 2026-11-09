<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { t } from "../i18n";
  import { app, openSession } from "../app.svelte";
  import Icon from "./Icon.svelte";
  import type { SessionSummary } from "../types";

  let { onclose }: { onclose: () => void } = $props();

  interface Hit {
    id: string;
    snippet: string;
  }

  const RECENT = 10;

  let query = $state("");
  let active = $state(0);
  let hits = $state<Hit[]>([]);
  let input: HTMLInputElement | undefined = $state();
  let token = 0;

  onMount(() => input?.focus());

  /** Lowercase without accents, so "configuracion" finds "configuración". */
  const fold = (text: string) =>
    text
      .toLowerCase()
      .normalize("NFD")
      .replace(/[̀-ͯ]/g, "");

  const projectName = (session: SessionSummary) =>
    app.projects.find((p) => p.id === session.project_id)?.name ?? "";

  function when(ts: number): string {
    const date = new Date(ts * 1000);
    return date.toDateString() === new Date().toDateString()
      ? date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      : date.toLocaleDateString([], { day: "2-digit", month: "short" });
  }

  const needle = $derived(fold(query.trim()));

  /** Conversations whose title matches (or the newest ones when nothing is typed). */
  const byTitle = $derived(
    needle
      ? app.sessions.filter((s) => fold(s.title || "").includes(needle))
      : app.sessions.slice(0, RECENT)
  );

  /** Conversations that only match inside their messages. */
  const byContent = $derived.by(() => {
    const seen = new Set(byTitle.map((s) => s.id));
    return hits
      .filter((hit) => !seen.has(hit.id))
      .flatMap((hit) => {
        const session = app.sessions.find((s) => s.id === hit.id);
        return session ? [{ session, snippet: hit.snippet }] : [];
      });
  });

  type Row = { session: SessionSummary; snippet?: string };
  const groups = $derived.by(() => {
    const out: { title: string; rows: Row[] }[] = [];
    if (byTitle.length) {
      out.push({ title: needle ? t("Conversations") : t("Recent conversations"), rows: byTitle.map((session) => ({ session })) });
    }
    if (byContent.length) out.push({ title: t("In messages"), rows: byContent });
    return out;
  });
  const flat = $derived(groups.flatMap((g) => g.rows));

  // Search inside messages once the user pauses typing.
  $effect(() => {
    const q = query.trim();
    active = 0;
    const mine = ++token;
    if (q.length < 2) {
      hits = [];
      return;
    }
    const timer = setTimeout(() => {
      invoke<{ id: string; snippet: string }[]>("search_sessions", { query: q })
        .then((found) => {
          if (mine === token) hits = found;
        })
        .catch(() => {
          if (mine === token) hits = [];
        });
    }, 180);
    return () => clearTimeout(timer);
  });

  function choose(row: Row | undefined): void {
    if (!row) return;
    onclose();
    openSession(row.session.id);
  }

  function onKey(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      active = Math.min(active + 1, flat.length - 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      active = Math.max(active - 1, 0);
    } else if (event.key === "Enter") {
      event.preventDefault();
      choose(flat[active]);
    }
  }

  $effect(() => {
    active;
    document.querySelector(".ss-row.active")?.scrollIntoView({ block: "nearest" });
  });
</script>

<div class="ss-overlay" role="presentation" onclick={onclose}></div>
<div class="ss-dialog" role="dialog" aria-label={t("Search conversations")}>
  <div class="ss-bar">
    <Icon name="search" size={16} />
    <input
      bind:this={input}
      bind:value={query}
      onkeydown={onKey}
      placeholder={t("Search conversations")}
      spellcheck="false"
    />
  </div>
  <div class="ss-list">
    {#if flat.length === 0}
      <div class="ss-empty">{app.sessions.length === 0 ? t("No conversations yet") : t("No results")}</div>
    {/if}
    {#each groups as group (group.title)}
      <div class="ss-group">{group.title}</div>
      {#each group.rows as row (row.session.id + (row.snippet ? ":m" : ""))}
        {@const index = flat.indexOf(row)}
        <div
          class="ss-row"
          class:active={index === active}
          role="option"
          tabindex="-1"
          aria-selected={index === active}
          onclick={() => choose(row)}
          onkeydown={() => {}}
          onmousemove={() => (active = index)}
        >
          <div class="ss-main">
            <span class="ss-title">{row.session.title || t("Untitled")}</span>
            {#if row.snippet}<span class="ss-snippet">{row.snippet}</span>{/if}
          </div>
          <span class="ss-meta">{[projectName(row.session), when(row.session.updated_at)].filter(Boolean).join(" · ")}</span>
        </div>
      {/each}
    {/each}
  </div>
</div>

<style>
  .ss-overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    background: rgba(0, 0, 0, 0.45);
  }
  .ss-dialog {
    position: fixed;
    top: 14vh;
    left: 50%;
    transform: translateX(-50%);
    z-index: 1001;
    width: min(620px, calc(100vw - 32px));
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    overflow: hidden;
  }
  .ss-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 16px;
    border-bottom: 1px solid var(--border-primary);
    color: var(--text-muted);
  }
  input {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--text-primary);
    padding: 15px 0;
    font-size: 14px;
    outline: none;
  }
  .ss-list {
    max-height: min(56vh, 480px);
    overflow: auto;
    padding: 4px;
  }
  .ss-group {
    padding: 10px 10px 4px;
    font-size: 11px;
    color: var(--text-muted);
  }
  .ss-row {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 14px;
    padding: 7px 10px;
    border-radius: 7px;
    cursor: pointer;
    color: var(--text-secondary);
  }
  .ss-row.active {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .ss-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .ss-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13.5px;
  }
  .ss-snippet {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--text-muted);
  }
  .ss-meta {
    flex: none;
    font-size: 12px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .ss-empty {
    padding: 18px;
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
