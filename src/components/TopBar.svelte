<script lang="ts">
  import { t } from "../i18n";
  // ianox — application top bar: session tabs, API and theme
  import Icon from "./Icon.svelte";
  import {
    app,
    newSession,
    activateTab,
    closeSessionTab,
    applyTheme,
    startServer,
    stopServer,
    DRAFT_TAB,
    projectIcon,
  } from "../app.svelte";

  function titleFor(id: string): string {
    if (id === DRAFT_TAB) return t("New conversation");
    const session = app.sessions.find((s) => s.id === id);
    return session?.title || t("Untitled");
  }

  /** Project the conversation of a tab belongs to (a draft belongs to the open project). */
  function iconFor(id: string): string | null {
    const projectId =
      id === DRAFT_TAB ? app.activeProjectId : app.sessions.find((s) => s.id === id)?.project_id;
    return projectIcon(projectId);
  }

  function toggleApi(): void {
    if (app.serverStatus.running) stopServer();
    else startServer();
  }

  const themes = $derived([
    { id: "dark", label: t("ianox Dark"), icon: "moon" },
    { id: "light", label: t("ianox Light"), icon: "sun" },
    { id: "soplos-dark", label: t("Soplos Dark"), icon: "moon" },
    { id: "soplos-light", label: t("Soplos Light"), icon: "sun" },
    { id: "system", label: t("System"), icon: "monitor" },
  ]);

  const currentTheme = $derived(themes.find((t) => t.id === app.theme) ?? themes[0]);

  function cycleTheme(): void {
    const index = themes.findIndex((t) => t.id === app.theme);
    applyTheme(themes[(index + 1) % themes.length].id);
  }
</script>

<div class="topbar">
  <div class="titlebar-tabs">
    {#each app.sessionTabs as tab (tab.id)}
      <div class={`titlebar-tab${app.activeTab === tab.id ? " active" : ""}`}>
        {#if app.busy[tab.id]}
          <span class="tab-spin" title={t("Working…")} aria-label={t("Working…")}></span>
        {:else if iconFor(tab.id)}
          <img class="tab-icon" src={iconFor(tab.id)} alt="" />
        {/if}
        <button class="titlebar-tab-main" onclick={() => activateTab(tab.id)} title={titleFor(tab.id)}>
          {titleFor(tab.id)}
        </button>
        <button
          class="titlebar-tab-close"
          title={t("Close tab")}
          onclick={(e) => {
            e.stopPropagation();
            closeSessionTab(tab.id);
          }}
        >
          ×
        </button>
      </div>
    {/each}
    <button class="titlebar-tab-add" title={t("New conversation")} onclick={newSession}>
      <Icon name="plus" size={13} />
    </button>
  </div>

  <div class="topbar-spacer"></div>

  <button
    class={`titlebar-api${app.serverStatus.running ? " on" : ""}`}
    onclick={toggleApi}
    title={app.serverStatus.running
      ? t("External API on {addr} · click to stop", { addr: app.serverStatus.addr ?? "" })
      : t("External API is off · click to start")}
  >
    <span class="titlebar-api-dot"></span>
    {app.serverStatus.running ? t("API on") : t("API off")}
  </button>

  <button
    class="titlebar-icon"
    class:active={app.searchOpen}
    title={`${t("Search conversations")} (Ctrl+K)`}
    aria-label={t("Search conversations")}
    onclick={() => (app.searchOpen = true)}
  >
    <Icon name="search" size={16} />
  </button>

  <button
    class="titlebar-icon"
    class:active={app.tuner}
    title={t("Typography")}
    aria-label={t("Typography")}
    onclick={() => (app.tuner = !app.tuner)}
    style="font-weight:600;font-size:12px"
  >
    Aa
  </button>

  <button
    class="titlebar-icon"
    title={t("Theme: {label} · click to change", { label: currentTheme.label })}
    onclick={cycleTheme}
  >
    <Icon name={currentTheme.icon} size={16} />
  </button>
</div>
