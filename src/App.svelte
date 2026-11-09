<script lang="ts">
  import { t } from "./i18n";
  // ianox — main application
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import type { Config, ServerStatus } from "./types";
  import { app, boot, requestQuit, toggleSidebar } from "./app.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Header from "./components/Header.svelte";
  import TitleBar from "./components/TitleBar.svelte";
  import TopBar from "./components/TopBar.svelte";
  import LogoMark from "./components/LogoMark.svelte";
  import WidgetAssistant from "./components/WidgetAssistant.svelte";
  import Dashboard from "./pages/Dashboard.svelte";
  import Models from "./pages/Models.svelte";
  import Providers from "./pages/Providers.svelte";
  import Routes from "./pages/Routes.svelte";
  import Chat from "./pages/Chat.svelte";
  import Agents from "./pages/Agents.svelte";
  import Gateway from "./pages/Gateway.svelte";
  import Settings from "./pages/Settings.svelte";
  import Extensions from "./pages/Extensions.svelte";
  import TypeTuner from "./components/TypeTuner.svelte";
  import SessionSearch from "./components/SessionSearch.svelte";
  import QuitDialog from "./components/QuitDialog.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { applyTuning, loadTuning } from "./type-tuner";

  const pageTitles: Record<string, string> = {
    chat: "Chat",
    agents: t("Agents"),
    gateway: t("Gateway"),
    dashboard: t("Dashboard"),
    models: t("Models"),
    providers: t("Providers"),
    routes: t("Routing rules"),
    settings: t("Configuration"),
    extensions: t("Extensions"),
  };

  function checkIsAssistant(): boolean {
    try {
      return getCurrentWindow().label === "assistant";
    } catch {
      return false;
    }
  }

  const isAssistantWindow = checkIsAssistant();

  onMount(() => {
    if (isAssistantWindow) {
      return;
    }
    // The typography the user picked is applied on every start.
    applyTuning(loadTuning());
    const onKey = (event: KeyboardEvent) => {
      const mod = event.ctrlKey || event.metaKey;
      const key = event.key.toLowerCase();

      if (mod && !event.altKey && !event.shiftKey && key === "b") {
        event.preventDefault();
        toggleSidebar();
      } else if (mod && !event.altKey && !event.shiftKey && key === "k") {
        event.preventDefault();
        app.searchOpen = !app.searchOpen;
      } else if (mod && !event.altKey && !event.shiftKey && key === "q") {
        event.preventDefault();
        void requestQuit();
      }
    };
    window.addEventListener("keydown", onKey);

    // The window's close button: the backend holds the close and asks us what to do.
    let unlistenQuit: UnlistenFn | undefined;
    listen("app://quit-requested", () => void requestQuit())
      .then((fn) => (unlistenQuit = fn))
      .catch(() => {});
    // From now on the backend may hand close requests to the interface.
    invoke("frontend_ready").catch(() => {});

    boot();

    // Estado maximizado -> esquinas rectas
    let unlisten: UnlistenFn | undefined;
    let win: ReturnType<typeof getCurrentWindow> | null = null;
    try {
      win = getCurrentWindow();
    } catch {
      /* fuera de Tauri */
    }
    const sync = () => {
      win
        ?.isMaximized()
        .then((value) => (app.maximized = value))
        .catch(() => {
          /* sin permisos o fuera de Tauri */
        });
    };
    sync();
    win?.onResized(sync).then((fn) => (unlisten = fn)).catch(() => {});

    return () => {
      unlisten?.();
      unlistenQuit?.();
      window.removeEventListener("keydown", onKey);
    };
  });

  // Backend events (not emitted yet; prepared for later)
  $effect(() => {
    if (app.phase !== "ready") return;
    let unlistenServer: UnlistenFn | null = null;
    let unlistenConfig: UnlistenFn | null = null;

    listen<ServerStatus>("server://status", (e) => {
      app.serverStatus = e.payload;
    }).then((fn) => (unlistenServer = fn)).catch(() => {});

    listen<Config>("config://changed", (e) => {
      app.config = e.payload;
    }).then((fn) => (unlistenConfig = fn)).catch(() => {});

    return () => {
      unlistenServer?.();
      unlistenConfig?.();
    };
  });
</script>

{#if isAssistantWindow}
  <WidgetAssistant />
{:else}
  <div class={`app-shell${app.maximized ? " maximized" : ""}`}>
    <TitleBar />

    {#if app.phase === "ready"}
      <div class="app-body">
        <Sidebar />

        <div class="app-main">
          <TopBar />

          {#if app.page !== "chat"}
            <Header title={pageTitles[app.page]} />
          {/if}

          <main class={`page-content${app.page === "chat" ? " chat-content" : ""}`}>
            {#if app.page === "chat"}
              {#key app.activeProjectId}
                <Chat />
              {/key}
            {:else if app.page === "agents"}
              <Agents />
            {:else if app.page === "gateway"}
              <Gateway />
            {:else if app.page === "dashboard"}
              <Dashboard />
            {:else if app.page === "models"}
              <Models />
            {:else if app.page === "providers"}
              <Providers />
            {:else if app.page === "routes"}
              <Routes />
            {:else if app.page === "settings"}
              <Settings />
            {:else if app.page === "extensions"}
              <Extensions />
            {/if}
          </main>
        </div>
      </div>
    {:else}
      <div class="boot">
        <div class="boot-logo">
          <LogoMark size={72} radius={18} />
        </div>
        <div class="boot-title">ianox</div>
        <div class="boot-sub">{t("Local AI gateway")}</div>
        {#if app.phase === "connecting"}
          <div class="boot-status">
            <span class="spinner"></span>
            {t("Preparing the workspace…")}
          </div>
        {/if}
      </div>
    {/if}
  </div>

  {#if app.tuner}<TypeTuner onclose={() => (app.tuner = false)} />{/if}
  {#if app.searchOpen}<SessionSearch onclose={() => (app.searchOpen = false)} />{/if}
  {#if app.quitOpen}<QuitDialog />{/if}
{/if}
