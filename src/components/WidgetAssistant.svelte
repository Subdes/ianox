<script lang="ts">
  import { t } from "../i18n";
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import LogoMark from "../components/LogoMark.svelte";
  import Icon from "../components/Icon.svelte";
  import Markdown from "../components/Markdown.svelte";
  import { app, applyTheme } from "../app.svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import type { Config, ModelInfo } from "../types";

  interface ProviderInfo {
    id: string;
    name: string;
    enabled: boolean;
    models: string[];
  }

  let query = $state("");
  let sending = $state(false);
  let responseText = $state("");
  let models = $state<ModelInfo[]>([]);
  let selectedModel = $state<string>("");
  let providers = $state<ProviderInfo[]>([]);
  let selectedProvider = $state<string>("");
  let agentMode = $state<"ask" | "code">("ask");
  let inputEl: HTMLTextAreaElement | undefined = $state();
  let copied = $state(false);
  let isClosing = $state(false);
  let currentRequestId = $state<string | null>(null);
  let activeListeners: UnlistenFn[] = [];
  let animKey = $state(0);

  onMount(() => {
    let unlistenTheme: UnlistenFn | undefined;
    let unlistenShown: UnlistenFn | undefined;
    let unlistenClose: UnlistenFn | undefined;

    // 1. Cargar tema inmediato desde almacenamiento local
    try {
      const cached = localStorage.getItem("ianox.theme");
      if (cached) applyTheme(cached);
    } catch {}

    // Sincronizar idioma y tema de la configuración
    invoke<Config>("get_config")
      .then((cfg) => {
        if (cfg?.ui?.language) {
          app.language = cfg.ui.language === "es" ? "es" : "en";
        }
        if (cfg?.ui?.theme) {
          applyTheme(cfg.ui.theme);
        }
      })
      .catch(() => {});

    // Escuchar cambios de tema en vivo desde la ventana principal
    listen<string>("app://theme-changed", (event) => {
      if (event.payload) {
        applyTheme(event.payload);
      }
    }).then((fn) => {
      unlistenTheme = fn;
    });

    // Escuchar cuando el asistente se muestra para disparar la animación de entrada
    listen("assistant://shown", () => {
      isClosing = false;
      animKey++;
      setTimeout(() => inputEl?.focus(), 50);
    }).then((fn) => {
      unlistenShown = fn;
    });

    // Escuchar solicitud de cierre del atajo global o bandeja para deslizar hacia fuera
    listen("assistant://close-requested", () => {
      closeWidget();
    }).then((fn) => {
      unlistenClose = fn;
    });

    // Cargar modelos disponibles
    invoke<ModelInfo[]>("list_models")
      .then((data) => {
        models = data;
        if (data.length > 0 && !selectedModel) {
          selectedModel = data[0].id;
        }
      })
      .catch(() => {});

    // Cargar proveedores habilitados
    invoke<ProviderInfo[]>("list_providers")
      .then((data) => {
        providers = data.filter((p) => p.enabled);
        if (providers.length > 0 && !selectedProvider) {
          selectedProvider = providers[0].id;
          // Si el proveedor tiene modelos propios, preseleccionar el primero
          if (providers[0].models.length > 0) {
            selectedModel = providers[0].models[0];
          }
        }
      })
      .catch(() => {});

    // Autofoco inmediato y al recuperar el foco de ventana (también re-dispara la animación si se volvió a mostrar)
    inputEl?.focus();
    const handleFocus = () => {
      inputEl?.focus();
      if (!isClosing) {
        animKey++;
      }
    };
    window.addEventListener("focus", handleFocus);

    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        closeWidget();
      }
    };
    window.addEventListener("keydown", handleKey);

    return () => {
      unlistenTheme?.();
      unlistenShown?.();
      unlistenClose?.();
      window.removeEventListener("focus", handleFocus);
      window.removeEventListener("keydown", handleKey);
      cleanupListeners();
      if (currentRequestId) {
        invoke("chat_cancel", { requestId: currentRequestId }).catch(() => {});
      }
    };
  });

  async function closeWidget(): Promise<void> {
    if (isClosing) return;
    isClosing = true;
    // Permitir que la animación CSS de salida (slide out a la derecha) se complete
    setTimeout(async () => {
      try {
        await invoke("hide_assistant");
      } catch {
        /* ignore */
      } finally {
        isClosing = false;
      }
    }, 180);
  }

  async function openMainWindow(): Promise<void> {
    try {
      // Abre y enfoca la ventana principal de ianox
      await invoke("open_main_window");
    } catch {
      /* fallback */
    }
  }

  function cleanupListeners(): void {
    activeListeners.forEach((fn) => fn());
    activeListeners = [];
  }

  function handleStop(): void {
    if (currentRequestId) {
      invoke("chat_cancel", { requestId: currentRequestId }).catch(() => {});
    }
    sending = false;
    currentRequestId = null;
    cleanupListeners();
  }

  function handleClear(): void {
    responseText = "";
    query = "";
    inputEl?.focus();
  }

  async function handleCopy(): Promise<void> {
    if (!responseText) return;
    try {
      await navigator.clipboard.writeText(responseText);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch {
      /* ignore */
    }
  }

  async function handleSend(): Promise<void> {
    const text = query.trim();
    if (!text || sending) return;

    const model = selectedModel;
    if (!model) { responseText = t("No model selected"); return; }

    sending = true;
    responseText = "";
    cleanupListeners();

    const requestId = crypto.randomUUID();
    currentRequestId = requestId;
    const add = (fn: UnlistenFn) => activeListeners.push(fn);

    const systemPrompt =
      agentMode === "code"
        ? "Eres el Asistente de Sistema de ianox especializado en código, scripts y comandos técnicos. Ofrece soluciones directas, snippets limpios y explicaciones concisas sin rodeos."
        : "Eres el Asistente de Sistema de ianox. Ayudas al usuario con cualquier consulta, duda, redacción, análisis o tarea en su ordenador de forma rápida, clara y precisa.";

    try {
      add(await listen<string>(`chat://delta/${requestId}`, (event) => {
        responseText += event.payload;
      }));
      add(await listen(`chat://done/${requestId}`, () => {
        sending = false;
        currentRequestId = null;
        cleanupListeners();
        query = "";
        invoke("notify_if_unfocused", {
          title: "ianox Assistant",
          body: t("Response completed"),
        }).catch(() => {});
      }));
      add(await listen<string>(`chat://error/${requestId}`, (event) => {
        responseText = `${t("Error")}: ${event.payload}`;
        sending = false;
        currentRequestId = null;
        cleanupListeners();
      }));

      await invoke("chat_stream", {
        requestId,
        model,
        messages: [
          { role: "system", content: systemPrompt },
          { role: "user", content: text }
        ],
        temperature: null,
        maxTokens: null,
        sessionId: null,
        agentMode: agentMode === "code" ? "build" : "chat",
        providerId: selectedProvider || null
      });
    } catch (err) {
      responseText = `${t("Error")}: ${err}`;
      sending = false;
      currentRequestId = null;
      cleanupListeners();
    }
  }
</script>

{#key animKey}
  <div class={`widget-shell ${isClosing ? "slide-out" : "slide-in"}`}>
    <!-- Cabecera principal -->
    <header class="widget-header">
    <button
      class="widget-brand"
      type="button"
      onclick={openMainWindow}
      title={t("Open ianox main window")}
    >
      <LogoMark size={20} radius={5} />
      <span class="widget-title">ianox Assistant</span>
    </button>

    <div class="widget-actions">
      <!-- Botón Nueva consulta -->
      <button class="btn btn-ghost btn-icon btn-sm" onclick={handleClear} title={t("New conversation")}>
        <Icon name="plus" size={14} />
      </button>

      <!-- Abrir en ventana completa -->
      <button class="btn btn-ghost btn-icon btn-sm" onclick={openMainWindow} title={t("Open main window")}>
        <Icon name="external-link" size={14} />
      </button>

      <!-- Cerrar / Ocultar -->
      <button class="btn btn-ghost btn-icon btn-sm btn-close" onclick={closeWidget} title="Esc">
        <Icon name="x" size={14} />
      </button>
    </div>
  </header>

  <!-- Barra de contexto y opciones (Modo, Modelo) -->
  <div class="widget-subbar">
    <!-- Selector de Modo: Ask / Code -->
    <div class="mode-toggles">
      <button
        class={`mode-btn ${agentMode === "ask" ? "active" : ""}`}
        onclick={() => (agentMode = "ask")}
        title={t("General queries and chat")}
      >
        <Icon name="chat" size={12} />
        <span>Ask</span>
      </button>
      <button
        class={`mode-btn ${agentMode === "code" ? "active" : ""}`}
        onclick={() => (agentMode = "code")}
        title={t("Code and technical commands")}
      >
        <Icon name="terminal" size={12} />
        <span>Code</span>
      </button>
    </div>

    <!-- Selector de Proveedor + Modelo -->
    <div class="widget-selects">
      {#if providers.length > 0}
        <select
          bind:value={selectedProvider}
          class="widget-select"
          title={t("Active provider")}
          onchange={() => {
            const prov = providers.find((p) => p.id === selectedProvider);
            if (prov && prov.models.length > 0) selectedModel = prov.models[0];
          }}
        >
          {#each providers as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>
      {/if}
      {#if models.length > 0}
        {@const providerModels = selectedProvider
          ? (providers.find((p) => p.id === selectedProvider)?.models ?? [])
          : []}
        {@const displayModels = providerModels.length > 0
          ? models.filter((m) => providerModels.includes(m.id))
          : models}
        <select bind:value={selectedModel} class="widget-select" title={t("Active model")}>
          {#each displayModels as m (m.id)}
            <option value={m.id}>{m.id}</option>
          {/each}
        </select>
      {/if}
    </div>
  </div>

  <!-- Área de contenido / respuesta -->
  <main class="widget-content">
    {#if responseText}
      <div class="widget-response-card">
        <div class="response-header">
          <span class="response-tag">
            <Icon name={agentMode === "code" ? "terminal" : "sparkles"} size={12} />
            {agentMode === "code" ? "Code" : "Answer"}
          </span>
          <button class="btn btn-ghost btn-icon btn-xs" onclick={handleCopy} title={t("Copy response")}>
            <Icon name={copied ? "check" : "copy"} size={13} />
          </button>
        </div>
        <div class="response-markdown">
          <Markdown source={responseText} />
        </div>
      </div>
    {:else}
      <div class="widget-placeholder">
        <div class="placeholder-icon">
          <LogoMark size={36} radius={9} />
        </div>
        <p class="placeholder-title">{t("System Assistant")}</p>
        <p class="placeholder-desc">
          {agentMode === "code"
            ? t("Ask for commands, bash scripts, debugging or technical solutions.")
            : t("Ask anything, translate text, draft ideas or summarize information.")}
        </p>
        <div class="placeholder-hints">
          <span class="hint-badge"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Space</kbd></span>
          <span class="hint-badge"><kbd>Esc</kbd></span>
        </div>
      </div>
    {/if}
  </main>

  <!-- Pie con Input -->
  <footer class="widget-footer">
    <div class="widget-input-wrapper">
      <textarea
        bind:this={inputEl}
        bind:value={query}
        class="widget-input"
        placeholder={agentMode === "code" ? t("Ask for code, scripts or commands…") : t("Ask ianox anything…")}
        rows={2}
        onkeydown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            handleSend();
          }
        }}
      ></textarea>
      {#if sending}
        <button
          type="button"
          class="composer-send stop"
          onclick={handleStop}
          title={t("Stop")}
        >
          <Icon name="stop" size={14} />
        </button>
      {:else}
        <button
          type="button"
          class="composer-send"
          disabled={!query.trim()}
          onclick={handleSend}
          title={t("Send")}
        >
          <Icon name="arrow-up" size={16} />
        </button>
      {/if}
    </div>
  </footer>
</div>
{/key}

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    overflow: hidden;
    background: transparent !important;
  }

  .widget-shell {
    width: 100vw;
    height: 100vh;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    background: var(--bg-primary);
    backdrop-filter: blur(28px);
    -webkit-backdrop-filter: blur(28px);
    border-left: 1px solid var(--border-primary);
    border-top: none;
    border-bottom: none;
    border-right: none;
    border-radius: 14px 0 0 14px;
    box-shadow: -12px 0 36px rgba(0, 0, 0, 0.4);
    color: var(--text-primary);
    user-select: none;
    will-change: transform, opacity;
  }

  .widget-shell.slide-in {
    animation: assistantSlideIn 0.22s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }

  .widget-shell.slide-out {
    animation: assistantSlideOut 0.18s cubic-bezier(0.7, 0, 0.84, 0) forwards;
  }

  @keyframes assistantSlideIn {
    0% {
      transform: translateX(100%);
      opacity: 0;
    }
    100% {
      transform: translateX(0);
      opacity: 1;
    }
  }

  @keyframes assistantSlideOut {
    0% {
      transform: translateX(0);
      opacity: 1;
    }
    100% {
      transform: translateX(100%);
      opacity: 0;
    }
  }

  /* Cabecera */
  .widget-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-primary);
  }

  .widget-brand {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: inherit;
    transition: background 0.15s ease;
  }

  .widget-brand:hover {
    background: var(--bg-hover);
  }

  .widget-title {
    font-size: 13px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .widget-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .btn-xs {
    padding: 4px;
    border-radius: 4px;
  }

  /* Subbarra */
  .widget-subbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 14px;
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border-primary);
    gap: 8px;
  }

  .mode-toggles {
    display: flex;
    background: var(--bg-primary);
    padding: 2px;
    border-radius: 6px;
    gap: 2px;
    border: 1px solid var(--border-primary);
  }

  .mode-btn {
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 500;
    padding: 3px 8px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .mode-btn.active {
    background: var(--accent-primary);
    color: #fff;
    box-shadow: 0 1px 4px var(--accent-glow);
  }

  .subbar-selects {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .widget-selects {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 1;
    min-width: 0;
  }

  .widget-select {
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    font-size: 11px;
    padding: 3px 6px;
    outline: none;
    cursor: pointer;
    max-width: 140px;
  }

  .widget-select:focus {
    border-color: var(--accent-primary);
  }

  /* Contenido */
  .widget-content {
    flex: 1;
    overflow-y: auto;
    padding: 14px;
    display: flex;
    flex-direction: column;
  }

  .widget-placeholder {
    margin: auto;
    text-align: center;
    color: var(--text-secondary);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 20px;
  }

  .placeholder-icon {
    margin-bottom: 6px;
    opacity: 0.9;
  }

  .placeholder-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .placeholder-desc {
    margin: 0;
    font-size: 12px;
    max-width: 260px;
    line-height: 1.4;
    color: var(--text-muted);
  }

  .placeholder-hints {
    display: flex;
    gap: 8px;
    margin-top: 14px;
  }

  .hint-badge {
    font-size: 11px;
    color: var(--text-muted);
    background: var(--bg-secondary);
    padding: 3px 8px;
    border-radius: 12px;
    border: 1px solid var(--border-primary);
  }

  .hint-badge kbd {
    background: var(--bg-hover);
    color: var(--text-primary);
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 10px;
  }

  /* Tarjeta de respuesta */
  .widget-response-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: 10px;
    overflow: hidden;
  }

  .response-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border-primary);
  }

  .response-tag {
    font-size: 11px;
    font-weight: 600;
    color: var(--accent-primary);
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .response-markdown {
    padding: 12px 14px;
    font-size: 13px;
    line-height: 1.5;
    user-select: text;
  }

  /* Pie & Input */
  .widget-footer {
    padding: 10px 14px 14px;
    background: var(--bg-secondary);
    border-top: 1px solid var(--border-primary);
  }

  .widget-input-wrapper {
    position: relative;
    display: flex;
    align-items: flex-end;
  }

  .widget-input {
    width: 100%;
    background: var(--bg-primary);
    border: 1px solid var(--border-primary);
    border-radius: 12px;
    color: var(--text-primary);
    padding: 10px 48px 10px 12px;
    font-size: 13px;
    font-family: inherit;
    resize: none;
    outline: none;
    box-sizing: border-box;
    transition: border-color 0.15s ease;
  }

  .widget-input:focus {
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px var(--accent-glow);
  }

  .widget-input-wrapper :global(.composer-send) {
    position: absolute;
    right: 8px;
    bottom: 8px;
  }
</style>
