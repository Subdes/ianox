<script lang="ts">
  import { t } from "../i18n";
  // ianox — persistent workbench file tree
  import { onMount, untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import type { FsEntry } from "../types";
  import Icon from "./Icon.svelte";

  let {
    root,
    onOpenFile,
    onClose,
    active = true,
  }: {
    root: string;
    onOpenFile: (path: string, name: string) => void;
    onClose?: () => void;
    active?: boolean;
  } = $props();

  let entries = $state<Record<string, FsEntry[]>>({});
  let expanded = $state<Set<string>>(new Set());
  let loading = $state<Set<string>>(new Set());
  let error = $state<string | null>(null);
  let filter = $state("");

  async function loadDir(path: string): Promise<void> {
    loading = new Set(loading).add(path);
    try {
      const list = await invoke<FsEntry[]>("list_dir", { path });
      entries = { ...entries, [path]: list };
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      const next = new Set(loading);
      next.delete(path);
      loading = next;
    }
  }

  const projectName = $derived(root.split("/").filter(Boolean).pop() ?? root);

  function toggleDir(path: string): void {
    const next = new Set(expanded);
    if (next.has(path)) {
      next.delete(path);
    } else {
      next.add(path);
      if (!entries[path]) loadDir(path);
    }
    expanded = next;
  }

  // Reload on project change and when the tab becomes active again.
  let loadedRoot: string | null = null;
  $effect(() => {
    const r = root;
    if (r === loadedRoot) return;
    loadedRoot = r;
    untrack(() => {
      entries = {};
      expanded = new Set();
      loadDir(r);
    });
  });

  let wasActive = false;
  $effect(() => {
    const isActive = active;
    const r = root;
    if (isActive && wasActive && r) untrack(() => refreshTree());
    wasActive = isActive;
  });

  async function refreshTree(): Promise<void> {
    const r = root;
    if (!r) return;
    await loadDir(r);
    for (const dir of expanded) {
      if (dir !== r) {
        loadDir(dir);
      }
    }
  }

  onMount(() => {
    let unlisten: UnlistenFn | undefined;
    listen("app://workspace-changed", () => {
      refreshTree();
    }).then((u) => {
      unlisten = u;
    });
    return () => {
      unlisten?.();
    };
  });

  function visible(list: FsEntry[]): FsEntry[] {
    if (!filter) return list;
    const q = filter.toLowerCase();
    return list.filter((entry) => entry.name.toLowerCase().includes(q));
  }
</script>

{#snippet level(dirPath: string, depth: number)}
  {#each visible(entries[dirPath] ?? []) as entry (entry.path)}
    <div>
      <button
        class={`file-row${entry.is_dir ? " dir" : ""}`}
        style={`padding-left:${10 + depth * 14}px`}
        title={entry.path}
        onclick={() => (entry.is_dir ? toggleDir(entry.path) : onOpenFile(entry.path, entry.name))}
      >
        <span class="file-icon">
          {#if entry.is_dir}
            <Icon name={expanded.has(entry.path) ? "folder-open" : "folder"} size={14} />
          {:else}
            <Icon name="file" size={14} />
          {/if}
        </span>
        <span class="file-name">{entry.name}</span>
      </button>
      {#if entry.is_dir && expanded.has(entry.path)}
        {@render level(entry.path, depth + 1)}
      {/if}
    </div>
  {/each}
  {#if !entries[dirPath] && loading.has(dirPath)}
    <div class="file-note" style={`padding-left:${12 + depth * 14}px`}>{t("Loading…")}</div>
  {/if}
{/snippet}

<div class="file-tree-shell">
  <div class="panel-head">
    <span class="panel-title" style="display:inline-flex;align-items:center;gap:6px">
      <Icon name="folder" size={14} />
      {projectName}
    </span>
    <div class="panel-actions">
      <button class="icon-btn" title={t("Refresh")} onclick={refreshTree}>
        <Icon name="refresh" size={14} />
      </button>
      {#if onClose}
        <button class="icon-btn" title={t("Close file column")} onclick={onClose}>
          <Icon name="x" size={14} />
        </button>
      {/if}
    </div>
  </div>

  <div class="file-filter">
    <input class="input" type="search" bind:value={filter} placeholder={t("Filter files")} spellcheck="false" />
  </div>

  <div class="file-tree">
    {#if error}<div class="file-note error">{error}</div>{/if}
    {#if !entries[root] && loading.has(root)}<div class="file-note">{t("Loading…")}</div>{/if}
    {#if entries[root] && entries[root].length === 0 && !loading.has(root)}
      <div class="file-note">{t("Empty folder")}</div>
    {/if}
    {@render level(root, 0)}
  </div>
</div>
