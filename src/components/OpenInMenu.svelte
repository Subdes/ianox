<script lang="ts">
  import { t } from "../i18n";
  // ianox — t("Open in") (split button) para archivos, estilo OpenCode
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "./Icon.svelte";
  import AppIcon from "./AppIcon.svelte";
  import { openApps, loadOpenApps, setPreferredApp, preferredApp } from "../open-apps.svelte";

  let { path }: { path: string } = $props();

  let open = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    const p = path;
    loadOpenApps(p);
  });

  async function launch(app: string): Promise<void> {
    error = null;
    try {
      await invoke("open_path", { path, app });
    } catch (e) {
      error = String(e);
    }
  }

  function choose(app: string): void {
    setPreferredApp(app);
    open = false;
    launch(app);
  }

  async function copyPath(): Promise<void> {
    try {
      await navigator.clipboard.writeText(path);
    } catch (e) {
      error = String(e);
    }
    open = false;
  }

  const current = $derived(preferredApp());
</script>

<div class="openin">
  <button class="openin-main" onclick={() => launch(openApps.preferred)} title={t("Open in {app}", { app: current?.label ?? "…" })}>
    {#if current && current.id !== "finder"}
      <AppIcon app={current} size={14} />
    {:else}
      <Icon name="external" size={14} />
    {/if}
  </button>
  <button class="openin-toggle" onclick={() => (open = !open)} title={t("Open in…")}>
    <Icon name="chevron-down" size={10} />
  </button>

  {#if open}
    <div class="menu-backdrop" role="presentation" onclick={() => (open = false)}></div>
    <div class="openin-menu">
      <div class="menu-group-label">{t("Open in")}</div>
      {#each openApps.items as app (app.id)}
        <button class="menu-item" onclick={() => choose(app.id)}>
          <span class="app-menu-label"><AppIcon {app} size={15} />{app.label}</span>
          {#if app.id === openApps.preferred}<Icon name="check" size={12} />{/if}
        </button>
      {/each}
      <div class="menu-divider"></div>
      <button class="menu-item" onclick={copyPath}>
        <span>{t("Copy path")}</span>
      </button>
      {#if error}<div class="menu-empty" style="color:var(--danger)">{error}</div>{/if}
    </div>
  {/if}
</div>
