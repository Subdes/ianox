<script lang="ts">
  import { t } from "../i18n";
  import { onMount, tick, untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import LogoMark from "../components/LogoMark.svelte";
  import Icon from "../components/Icon.svelte";
  import Markdown from "../components/Markdown.svelte";
  import ModelPicker from "../components/ModelPicker.svelte";
  import { app, applyTheme } from "../app.svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { toApiMessages } from "../chat-history";
  import type { Config, Message, ModelInfo, ProjectInfo } from "../types";

  interface PermissionAsk {
    id: string;
    tool: string;
    summary: string;
    detail: string;
    sandboxed: boolean;
    can_remember: boolean;
    warning: string | null;
  }

  interface ProviderInfo {
    id: string;
    name: string;
    enabled: boolean;
    models: string[];
  }

  /** Ask = system assistant (home folder), Code = the project picked here. */
  type Mode = "ask" | "code";

  let query = $state("");
  let mode = $state<Mode>("ask");
  let messages = $state<Message[]>([]);
  let streaming = $state(false);
  let error = $state<string | null>(null);
  let permission = $state<PermissionAsk | null>(null);
  let models = $state<ModelInfo[]>([]);
  let providers = $state<ProviderInfo[]>([]);
  let selectedModel = $state<string>("");
  let selectedProvider = $state<string>("");
  let projects = $state<ProjectInfo[]>([]);
  let projectId = $state<string>("");
  let modelMenu = $state(false);
  let projectMenu = $state(false);
  let inputEl: HTMLTextAreaElement | undefined = $state();
  let scrollEl: HTMLElement | undefined = $state();
  let copied = $state(false);
  let isClosing = $state(false);
  let currentRequestId = $state<string | null>(null);
  let activeListeners: UnlistenFn[] = [];
  let animKey = $state(0);
  let queued = $state<string[]>([]);
  let listening = $state(false);
  let stickToBottom = $state(true);
  let loadToken = 0;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let recognition: any = null;

  /** Every mode keeps its own conversation: Ask globally, Code per project. */
  const sessionId = $derived(
    mode === "ask" ? "assistant-ask" : `assistant-code-${projectId || "none"}`
  );

  const project = $derived(projects.find((p) => p.id === projectId) ?? null);
  const projectLabel = $derived(project?.name ?? t("Select project"));

  // Catalog models, or the providers' configured lists when the catalog is still empty.
  const providerModelInfos = $derived.by<ModelInfo[]>(() => {
    if (models.length > 0) return models;
    return providers.flatMap((provider) =>
      provider.models.map((id) => ({
        id,
        name: id,
        provider_name: provider.name,
        context: null,
        free: false,
        provider: provider.id,
        provider_kind: "",
        size: null,
      }))
    );
  });

  const currentModel = $derived(providerModelInfos.find((m) => m.id === selectedModel) ?? null);
  const currentModelLabel = $derived(currentModel?.name ?? selectedModel);

  async function reloadModels(): Promise<void> {
    try {
      const [modelList, providerList] = await Promise.all([
        invoke<ModelInfo[]>("list_models").catch(() => [] as ModelInfo[]),
        invoke<ProviderInfo[]>("list_providers").catch(() => [] as ProviderInfo[]),
      ]);
      models = modelList;
      providers = providerList.filter((p) => p.enabled);
      restoreModelPreference();
    } catch {
      /* ignore */
    }
  }

  function restoreModelPreference(): void {
    try {
      const available = providerModelInfos;
      if (available.length === 0) return;

      const def = JSON.parse(localStorage.getItem("ianox.model.default") ?? "[]") as { id: string; provider: string }[];
      if (def[0]?.id && available.some((m) => m.id === def[0].id)) {
        selectedModel = def[0].id;
        selectedProvider = def[0].provider;
        return;
      }
      const last = JSON.parse(localStorage.getItem("ianox.assistant.last") ?? "{}") as { model?: string; provider?: string };
      if (last.model && available.some((m) => m.id === last.model)) {
        selectedModel = last.model;
        selectedProvider = last.provider ?? "";
        return;
      }
      const chatLast = JSON.parse(localStorage.getItem("ianox.chat.last") ?? "{}") as { model?: string; provider?: string };
      if (chatLast.model && available.some((m) => m.id === chatLast.model)) {
        selectedModel = chatLast.model;
        selectedProvider = chatLast.provider ?? "";
        return;
      }
      if (!selectedModel || !available.some((m) => m.id === selectedModel)) {
        selectedModel = available[0].id;
        selectedProvider = available[0].provider;
      }
    } catch {
      /* ignore */
    }
  }

  function onSelectModel(model: ModelInfo): void {
    selectedModel = model.id;
    selectedProvider = model.provider;
    modelMenu = false;
    try {
      localStorage.setItem("ianox.assistant.last", JSON.stringify({ model: selectedModel, provider: selectedProvider }));
    } catch {}
  }

  $effect(() => {
    if (currentModel && currentModel.provider !== selectedProvider) {
      selectedProvider = currentModel.provider;
    }
  });

  $effect(() => {
    if (providerModelInfos.length > 0 && !providerModelInfos.some((m) => m.id === selectedModel)) {
      selectedModel = providerModelInfos[0].id;
    }
  });

  // Switching mode or project loads that conversation from disk.
  $effect(() => {
    const id = sessionId;
    const token = ++loadToken;
    untrack(() => {
      invoke<{ messages: Message[] }>("get_session", { id })
        .then((stored) => {
          if (token !== loadToken) return;
          messages = (stored.messages ?? []).map((m) => ({ ...m, content: m.content ?? "" }));
          stickToBottom = true;
        })
        .catch(() => {
          if (token === loadToken) messages = [];
        });
    });
  });

  $effect(() => {
    void messages.length;
    void (messages[messages.length - 1]?.content.length ?? 0);
    if (!stickToBottom) return;
    tick().then(() => {
      if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
    });
  });

  onMount(() => {
    let unlistenTheme: UnlistenFn | undefined;
    let unlistenShown: UnlistenFn | undefined;
    let unlistenClose: UnlistenFn | undefined;
    let unlistenModels: UnlistenFn | undefined;
    let unlistenProviders: UnlistenFn | undefined;
    let unlistenConfig: UnlistenFn | undefined;

    // 1. Cargar tema inmediato desde almacenamiento local
    try {
      const cached = localStorage.getItem("ianox.theme");
      if (cached) applyTheme(cached);
    } catch {}

    const syncConfig = () => {
      invoke<Config>("get_config")
        .then((cfg) => {
          if (cfg?.ui?.language) {
            app.language = cfg.ui.language === "es" ? "es" : "en";
          }
          if (cfg?.ui?.theme) {
            applyTheme(cfg.ui.theme);
          }
          projects = cfg?.projects ?? [];
          if (!projectId) {
            projectId = cfg?.active_project ?? projects[0]?.id ?? "";
          }
        })
        .catch(() => {});
    };
    syncConfig();

    // Cargar modelos y proveedores disponibles
    void reloadModels();

    // Listen to live theme changes from the main window
    listen<string>("app://theme-changed", (event) => {
      if (event.payload) applyTheme(event.payload);
    }).then((fn) => {
      unlistenTheme = fn;
    });

    // Listen when the assistant is shown to trigger the entrance animation and refresh data
    listen("assistant://shown", () => {
      isClosing = false;
      animKey++;
      void reloadModels();
      syncConfig();
      setTimeout(() => inputEl?.focus(), 50);
    }).then((fn) => {
      unlistenShown = fn;
    });

    // Listen for close request from global command (shortcut, tray, etc.)
    listen("assistant://close-requested", () => {
      closeWidget();
    }).then((fn) => {
      unlistenClose = fn;
    });

    listen("models://updated", () => {
      void reloadModels();
    }).then((fn) => {
      unlistenModels = fn;
    });

    listen("providers://updated", () => {
      void reloadModels();
    }).then((fn) => {
      unlistenProviders = fn;
    });

    listen("config://changed", () => {
      syncConfig();
      void reloadModels();
    }).then((fn) => {
      unlistenConfig = fn;
    });

    // Focus the textarea on show
    window.addEventListener("focus", handleFocus);

    const handleKey = (e: KeyboardEvent) => {
      // A dialog inside the widget (model picker, permission) handles Escape first and
      // marks the event as prevented; only then the whole assistant slides away.
      if (e.key === "Escape" && !e.defaultPrevented) {
        if (modelMenu) {
          modelMenu = false;
          return;
        }
        if (projectMenu) {
          projectMenu = false;
          return;
        }
        closeWidget();
      }
    };
    window.addEventListener("keydown", handleKey);

    return () => {
      unlistenTheme?.();
      unlistenShown?.();
      unlistenClose?.();
      unlistenModels?.();
      unlistenProviders?.();
      unlistenConfig?.();
      window.removeEventListener("focus", handleFocus);
      window.removeEventListener("keydown", handleKey);
      cleanupListeners();
      recognition?.stop();
      if (currentRequestId) {
        invoke("chat_cancel", { requestId: currentRequestId }).catch(() => {});
      }
    };
  });

  async function closeWidget(): Promise<void> {
    if (isClosing) return;
    isClosing = true;
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
      await invoke("open_main_window", { page: null });
    } catch {
      /* fallback */
    }
  }

  function handleFocus(): void {
    inputEl?.focus();
  }

  function onScroll(): void {
    const el = scrollEl;
    if (!el) return;
    stickToBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 60;
  }

  function cleanupListeners(): void {
    activeListeners.forEach((fn) => fn());
    activeListeners = [];
  }

  function lastMessage(): Message | undefined {
    return messages[messages.length - 1];
  }

  function handleStop(): void {
    if (currentRequestId) {
      invoke("chat_cancel", { requestId: currentRequestId }).catch(() => {});
    }
    streaming = false;
    permission = null;
    currentRequestId = null;
    cleanupListeners();
    const tail = lastMessage();
    if (tail && tail.role === "assistant" && !tail.content && !tail.tools?.length) {
      messages = messages.slice(0, -1);
    }
    persist();
    void sendNextQueued();
  }

  /** New conversation: the stored history of this mode/project goes away. */
  function handleClear(): void {
    const id = sessionId;
    messages = [];
    queued = [];
    error = null;
    query = "";
    stickToBottom = true;
    if (id) invoke("delete_session", { id }).catch(() => {});
    inputEl?.focus();
  }

  function setMode(next: Mode): void {
    if (mode === next) return;
    if (streaming) handleStop();
    mode = next;
    error = null;
    projectMenu = false;
    inputEl?.focus();
  }

  function selectProject(id: string): void {
    projectMenu = false;
    if (projectId === id) return;
    if (streaming) handleStop();
    projectId = id;
    error = null;
    inputEl?.focus();
  }

  function stopSpeechInput(): void {
    if (!listening) return;
    recognition?.stop();
    listening = false;
  }

  function toggleSpeechInput(): void {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const win = window as any;
    const SpeechRec = win.SpeechRecognition || win.webkitSpeechRecognition;
    if (!SpeechRec) {
      alert(t("Speech recognition is not supported in this environment"));
      return;
    }

    if (listening) {
      stopSpeechInput();
      return;
    }

    try {
      recognition = new SpeechRec();
      recognition.continuous = true;
      recognition.interimResults = true;
      recognition.lang = app.language === "es" ? "es-ES" : "en-US";

      let baseText = query;
      if (baseText && !baseText.endsWith(" ")) {
        baseText += " ";
      }

      recognition.onstart = () => {
        listening = true;
      };

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      recognition.onresult = (event: any) => {
        let transcript = "";
        for (let i = event.resultIndex; i < event.results.length; ++i) {
          transcript += event.results[i][0].transcript;
        }
        if (transcript) {
          query = baseText + transcript;
        }
      };

      recognition.onerror = () => {
        listening = false;
      };

      recognition.onend = () => {
        listening = false;
      };

      recognition.start();
    } catch {
      listening = false;
    }
  }

  async function handleCopy(): Promise<void> {
    const last = [...messages].reverse().find((m) => m.role === "assistant" && m.content);
    if (!last) return;
    try {
      await navigator.clipboard.writeText(last.content);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch {
      /* ignore */
    }
  }

  function answerPending(): boolean {
    return messages.some((m) => m.role === "assistant" && !m.content && !m.tools?.length);
  }

  async function handleSend(): Promise<void> {
    const text = query.trim();
    if (!text && !permission) return;
    if (!selectedModel) return;

    if (streaming) {
      queued = [...queued, text];
      query = "";
      return;
    }

    const userMsg: Message = { role: "user", content: text };
    messages = [...messages, userMsg];
    query = "";
    stickToBottom = true;
    await sendRequest();
  }

  async function sendRequest(): Promise<void> {
    streaming = true;
    error = null;

    const assistantMsg: Message = { role: "assistant", content: "" };
    messages = [...messages, assistantMsg];
    stickToBottom = true;

    try {
      const apiMessages = toApiMessages(messages.slice(0, -1));
      const requestId = crypto.randomUUID();
      currentRequestId = requestId;

      const systemPrompt = mode === "code"
        ? (project?.path
            ? t("You are an expert coding assistant working inside the project: {path}. Answer questions, write code, and use tools when helpful.", { path: project.path })
            : t("You are an expert coding assistant. Help the user with programming tasks and technical questions."))
        : t("You are a helpful, concise assistant. Answer questions clearly and concisely.");

      const addListener = (unlisten: UnlistenFn) => activeListeners.push(unlisten);

      addListener(
        await listen<string>(`chat://delta/${requestId}`, (event) => {
          const idx = messages.length - 1;
          if (idx >= 0 && messages[idx].role === "assistant") {
            messages = [
              ...messages.slice(0, idx),
              { ...messages[idx], content: messages[idx].content + event.payload },
              ...messages.slice(idx + 1),
            ];
            stickToBottom = true;
          }
        })
      );

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      addListener(
        await listen<any[]>(`chat://tool-call/${requestId}`, (event) => {
          const idx = messages.length - 1;
          if (idx >= 0 && messages[idx].role === "assistant") {
            const existing = messages[idx].tools ?? [];
            messages = [
              ...messages.slice(0, idx),
              { ...messages[idx], tools: [...existing, ...event.payload] },
              ...messages.slice(idx + 1),
            ];
            stickToBottom = true;
          }
        })
      );

      addListener(
        await listen<PermissionAsk>(`chat://permission/${requestId}`, (event) => {
          permission = event.payload;
        })
      );

      addListener(
        await listen(`chat://done/${requestId}`, () => {
          streaming = false;
          currentRequestId = null;
          cleanupListeners();
          persist();
          invoke("notify_if_unfocused", {
            title: "ianox Assistant",
            body: t("Response completed"),
          }).catch(() => {});
          void sendNextQueued();
        })
      );

      addListener(
        await listen<string>(`chat://error/${requestId}`, (event) => {
          error = event.payload;
          streaming = false;
          currentRequestId = null;
          cleanupListeners();
          const tail = lastMessage();
          if (tail && tail.role === "assistant" && !tail.content) {
            messages = messages.slice(0, -1);
          }
        })
      );

      await invoke("chat_stream", {
        requestId,
        model: selectedModel,
        messages: [
          { role: "system", content: systemPrompt },
          ...apiMessages,
        ],
        temperature: null,
        maxTokens: null,
        sessionId,
        agentMode: mode === "code" ? "build" : "chat",
        providerId: selectedProvider || null,
        projectPath: mode === "code" ? (project?.path ?? null) : null,
        systemScope: mode === "ask",
      });
    } catch (e) {
      streaming = false;
      currentRequestId = null;
      cleanupListeners();
      error = e instanceof Error ? e.message : String(e);
      const tail = lastMessage();
      if (tail && tail.role === "assistant" && !tail.content) {
        messages = messages.slice(0, -1);
      }
    }
  }

  async function sendNextQueued(): Promise<void> {
    if (queued.length === 0) return;
    const next = queued.shift();
    if (!next) return;
    query = next;
    await handleSend();
  }

  function persist(): void {
    const id = sessionId;
    if (!id) return;
    invoke("save_session", { id, messages }).catch(() => {});
  }

  function handlePermission(action: "allow" | "deny" | "allowAlways"): void {
    if (!permission) return;
    const { id, can_remember } = permission;
    permission = null;
    invoke("permission_reply", { id, action, remember: action === "allowAlways" && can_remember }).catch(() => {});
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
        <Icon name="external" size={14} />
      </button>

      <!-- Close / Hide -->
      <button class="btn btn-ghost btn-icon btn-sm btn-close" onclick={closeWidget} title="Esc">
        <Icon name="x" size={14} />
      </button>
    </div>
  </header>

  <!-- Context bar and options (Mode, Model) -->
  <div class="widget-subbar">
    <!-- Selector de Modo: Ask / Code -->
    <div class="mode-toggles">
      <button
        class={`mode-btn ${mode === "ask" ? "active" : ""}`}
        onclick={() => setMode("ask")}
        title={t("General queries and chat")}
      >
        <Icon name="chat" size={12} />
        <span>Ask</span>
      </button>
      <button
        class={`mode-btn ${mode === "code" ? "active" : ""}`}
        onclick={() => setMode("code")}
        title={t("Code and technical commands")}
      >
        <Icon name="terminal" size={12} />
        <span>Code</span>
      </button>
    </div>

    <!-- Project (Code) and model -->
    <div class="widget-selects">
      {#if mode === "code"}
        <button
          type="button"
          class="widget-picker"
          title={t("Select project")}
          onclick={() => {
            projectMenu = !projectMenu;
            modelMenu = false;
          }}
        >
          <span class="widget-picker-label">{projectLabel}</span>
          <Icon name="chevron-down" size={10} />
        </button>
      {/if}
      <button
        type="button"
        class="widget-picker"
        title={t("Active model")}
        onclick={() => {
          modelMenu = !modelMenu;
          projectMenu = false;
          if (modelMenu) void reloadModels();
        }}
      >
        <span class="widget-picker-label">{currentModelLabel || t("Select model")}</span>
        <Icon name="chevron-down" size={10} />
      </button>

      {#if projectMenu}
        <button
          type="button"
          class="widget-menu-overlay"
          aria-label={t("Close")}
          onclick={() => (projectMenu = false)}
        ></button>
        <div class="widget-provider-menu" role="listbox">
          {#if projects.length === 0}
            <span class="widget-menu-empty">{t("No projects yet")}</span>
          {/if}
          {#each projects as item (item.id)}
            <button
              type="button"
              role="option"
              class="widget-provider-row"
              class:active={item.id === projectId}
              aria-selected={item.id === projectId}
              onclick={() => selectProject(item.id)}
            >
              {item.name}
            </button>
          {/each}
          <button
            type="button"
            class="widget-provider-row"
            onclick={() => {
              projectMenu = false;
              invoke("open_main_window", { page: "chat" }).catch(() => {});
            }}
          >
            {t("Open projects")}
          </button>
        </div>
      {/if}
    </div>
  </div>

  {#if modelMenu}
    <ModelPicker
      models={providerModelInfos}
      selected={selectedModel ? { provider: selectedProvider, id: selectedModel } : null}
      onselect={onSelectModel}
      onclose={() => (modelMenu = false)}
      onchanged={() => void reloadModels()}
      onconnect={() => invoke("open_main_window", { page: "providers" }).catch(() => {})}
    />
  {/if}

  <!-- Área de mensajes -->
  <div class="widget-messages" bind:this={scrollEl} onscroll={onScroll}>
    {#if messages.length === 0}
      <div class="widget-empty">
        <Icon name={mode === "ask" ? "chat" : "terminal"} size={32} />
        <p>{mode === "ask" ? t("Start a conversation") : t("Select a project to start coding")}</p>
      </div>
    {:else}
      {#each messages as message (message)}
        <div class={`msg ${message.role}`}>
          {#if message.role === "assistant"}
            <div class="msg-avatar">
              <LogoMark size={14} radius={4} />
            </div>
          {/if}
          <div class="msg-content">
            {#if message.tools && message.tools.length > 0}
              {#each message.tools as tool}
                <div class="tool-call">
                  <span class="tool-name">{tool.name}</span>
                  <span class="tool-args">{JSON.stringify(tool.arguments, null, 2)}</span>
                </div>
              {/each}
            {/if}
            {#if message.content}
              <Markdown source={message.content} />
            {:else if message.role === "assistant" && streaming}
              <div class="typing-indicator">
                <span></span><span></span><span></span>
              </div>
            {/if}
            {#if message.role === "assistant" && !streaming && message.content}
              <button class="btn btn-ghost btn-icon btn-xs copy-btn" onclick={handleCopy} title={copied ? t("Copied") : t("Copy")}>
                <Icon name={copied ? "check" : "copy"} size={12} />
              </button>
            {/if}
          </div>
        </div>
      {/each}
    {/if}
  </div>

  {#if error}
    <div class="widget-error" role="alert">
      <Icon name="alert-triangle" size={14} />
      <span>{error}</span>
      <button class="btn btn-ghost btn-icon btn-xs" onclick={() => (error = null)}>
        <Icon name="x" size={12} />
      </button>
    </div>
  {/if}

  {#if permission}
    <div class="widget-permission" role="dialog" aria-modal="true">
      <div class="permission-content">
        <Icon name="shield-alert" size={20} />
        <div class="permission-text">
          <strong>{permission.tool}</strong>
          <span>{permission.summary}</span>
          {#if permission.detail}<span class="detail">{permission.detail}</span>{/if}
          {#if permission.warning}<span class="warning">⚠ {permission.warning}</span>{/if}
        </div>
        <div class="permission-actions">
          <button class="btn btn-primary" onclick={() => handlePermission("allow")}>
            {t("Allow")}
          </button>
          {#if permission.can_remember}
            <button class="btn btn-primary" onclick={() => handlePermission("allowAlways")}>
              {t("Always allow")}
            </button>
          {/if}
          <button class="btn btn-danger" onclick={() => handlePermission("deny")}>
            {t("Deny")}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Compositor -->
  <div class="widget-composer">
    <div class="composer-wrap">
      <textarea
        bind:this={inputEl}
        class="composer-textarea"
        rows={1}
        bind:value={query}
        placeholder={streaming
          ? t("Write now: it goes out when this answer ends")
          : t("Type a message...")}
        onkeydown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            handleSend();
          }
        }}
        onfocus={stopSpeechInput}
        oninput={stopSpeechInput}
        disabled={streaming && !query.trim() && queued.length === 0}
      ></textarea>

      {#if !query.trim() && !streaming}
        <button
          type="button"
          class="composer-btn mic"
          onclick={toggleSpeechInput}
          title={t("Voice input")}
        >
          <Icon name="mic" size={16} />
        </button>
      {/if}

      {#if query.trim() && !streaming}
        <button
          type="button"
          class="composer-btn send"
          onclick={() => handleSend()}
          title={t("Send")}
          disabled={!selectedModel}
        >
          <Icon name="arrow-up" size={16} />
        </button>
      {/if}

      {#if streaming}
        <button
          type="button"
          class="composer-btn stop"
          onclick={handleStop}
          title={t("Stop")}
        >
          <Icon name="stop" size={14} />
        </button>

        {#if query.trim() || queued.length > 0}
          <button
            type="button"
            class="composer-btn queue"
            onclick={() => handleSend()}
            title={t("Send when this answer ends")}
          >
            <Icon name="arrow-up" size={16} />
            {#if queued.length > 0}<span class="queue-badge">{queued.length}</span>{/if}
          </button>
        {/if}
      {/if}
    </div>
  </div>
</div>
{/key}

<style>
  .widget-shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-primary);
    border-radius: 12px 0 0 12px;
    box-shadow: -4px 0 24px rgba(0, 0, 0, 0.4);
    overflow: hidden;
    animation: slideIn 0.2s ease-out;
  }

  .widget-shell.slide-out {
    animation: slideOut 0.18s ease-in forwards;
  }

  @keyframes slideIn {
    from {
      transform: translateX(100%);
      opacity: 0;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }

  @keyframes slideOut {
    from {
      transform: translateX(0);
      opacity: 1;
    }
    to {
      transform: translateX(100%);
      opacity: 0;
    }
  }

  .widget-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 44px;
    padding: 0 12px;
    border-bottom: 1px solid var(--border-color);
    background: var(--bg-secondary);
    flex-shrink: 0;
  }

  .widget-brand {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-radius: 8px;
    background: transparent;
    border: none;
    color: var(--fg-primary);
    cursor: pointer;
    transition: background 0.15s;
  }

  .widget-brand:hover {
    background: var(--bg-tertiary);
  }

  .widget-title {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 160px;
  }

  .widget-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .widget-subbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    gap: 12px;
    border-bottom: 1px solid var(--border-color);
    background: var(--bg-secondary);
    flex-shrink: 0;
  }

  .mode-toggles {
    display: flex;
    gap: 4px;
    background: var(--bg-tertiary);
    border-radius: 8px;
    padding: 2px;
  }

  .mode-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: var(--fg-secondary);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s;
    white-space: nowrap;
  }

  .mode-btn:hover {
    color: var(--fg-primary);
    background: var(--bg-primary);
  }

  .mode-btn.active {
    background: var(--accent-primary);
    color: white;
  }

  .widget-selects {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 0 1 auto;
    justify-content: flex-end;
    min-width: auto;
  }

  .widget-picker {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-radius: 8px;
    border: 1px solid var(--border-color);
    background: var(--bg-primary);
    color: var(--fg-primary);
    font-size: 12px;
    cursor: pointer;
    transition: all 0.15s;
    min-width: 110px;
    max-width: 220px;
  }

  .widget-picker:hover {
    border-color: var(--accent-primary);
    background: var(--bg-secondary);
  }

  .widget-picker-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    text-align: left;
  }

  .widget-menu-overlay {
    position: fixed;
    inset: 0;
    background: transparent;
    z-index: 99;
  }

  .widget-provider-menu {
    position: fixed;
    right: 16px;
    top: 100px;
    min-width: 220px;
    max-height: 300px;
    overflow-y: auto;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 10px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
    padding: 6px;
    z-index: 100;
    animation: fadeIn 0.12s ease-out;
  }

  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .widget-provider-row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 10px 12px;
    border-radius: 8px;
    border: none;
    background: transparent;
    color: var(--fg-primary);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    transition: background 0.1s;
  }

  .widget-provider-row:hover {
    background: var(--bg-tertiary);
  }

  .widget-provider-row.active {
    background: var(--accent-primary);
    color: white;
  }

  .widget-menu-empty {
    display: block;
    padding: 16px;
    text-align: center;
    color: var(--fg-tertiary);
    font-size: 12px;
  }

  .widget-messages {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .widget-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--fg-tertiary);
    text-align: center;
    padding: 24px;
    gap: 12px;
  }

  .widget-empty p {
    font-size: 13px;
    margin: 0;
  }

  .msg {
    display: flex;
    gap: 10px;
    max-width: 100%;
  }

  .msg.user {
    flex-direction: row-reverse;
  }

  .msg-avatar {
    width: 28px;
    height: 28px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--accent-primary);
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    margin-top: 2px;
  }

  .msg-content {
    flex: 1;
    min-width: 0;
    padding: 10px 12px;
    border-radius: 12px;
    background: var(--bg-secondary);
    font-size: 13px;
    line-height: 1.55;
  }

  .msg.user .msg-content {
    background: var(--accent-primary);
    color: white;
    border-bottom-right-radius: 4px;
  }

  .msg.assistant .msg-content {
    border-bottom-left-radius: 4px;
  }

  .tool-call {
    margin: 8px 0;
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border-color);
    font-size: 11px;
    font-family: var(--font-mono);
  }

  .tool-name {
    font-weight: 600;
    color: var(--accent-primary);
  }

  .tool-args {
    display: block;
    margin-top: 4px;
    color: var(--fg-secondary);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .typing-indicator {
    display: flex;
    gap: 3px;
    padding: 4px 0;
  }

  .typing-indicator span {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--fg-tertiary);
    animation: bounce 1.4s infinite ease-in-out both;
  }

  .typing-indicator span:nth-child(1) { animation-delay: -0.32s; }
  .typing-indicator span:nth-child(2) { animation-delay: -0.16s; }

  @keyframes bounce {
    0%, 80%, 100% { transform: scale(0.8); opacity: 0.5; }
    40% { transform: scale(1); opacity: 1; }
  }

  .copy-btn {
    margin-top: 8px;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .msg-content:hover .copy-btn {
    opacity: 1;
  }

  .widget-error {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    margin: 0 16px 16px;
    border-radius: 8px;
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #ef4444;
    font-size: 12px;
  }

  .widget-permission {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
    z-index: 50;
    animation: fadeIn 0.12s ease-out;
  }

  .permission-content {
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 16px;
    max-width: 360px;
    width: 100%;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
  }

  .permission-text {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 16px;
  }

  .permission-text strong {
    color: var(--fg-primary);
  }

  .permission-text .detail {
    font-size: 12px;
    color: var(--fg-secondary);
  }

  .permission-text .warning {
    font-size: 11px;
    color: #f59e0b;
  }

  .permission-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .widget-composer {
    display: flex;
    flex-direction: column;
    padding: 12px;
    border-top: 1px solid var(--border-color);
    background: var(--bg-secondary);
    flex-shrink: 0;
  }

  .composer-wrap {
    position: relative;
    width: 100%;
  }

  .composer-textarea {
    display: block;
    width: 100%;
    padding: 13px 40px 13px 12px;
    border-radius: 10px;
    border: 1px solid var(--border-color);
    background: var(--bg-primary);
    color: var(--fg-primary);
    font: inherit;
    font-size: 13px;
    line-height: 20px;
    resize: none;
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
    font-family: var(--font-sans);
  }

  .composer-textarea:focus {
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px var(--accent-glow);
  }

  .composer-textarea::placeholder {
    color: var(--fg-tertiary);
  }

  .composer-textarea:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .composer-btn {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    width: 34px;
    height: 34px;
    border-radius: 10px;
    border: none;
    background: var(--bg-tertiary);
    color: var(--fg-secondary);
    cursor: pointer;
    transition: all 0.15s;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .composer-btn.mic,
  .composer-btn.send,
  .composer-btn.stop,
  .composer-btn.queue {
    right: 8px;
  }

  .composer-btn.send {
    background: var(--accent-primary);
    color: white;
  }

  .composer-btn.stop {
    background: var(--bg-tertiary);
    color: var(--fg-secondary);
  }

  .composer-btn.queue {
    background: var(--accent-secondary);
    color: white;
  }

  .composer-btn:hover:not(:disabled) {
    background: var(--accent-primary);
    color: white;
  }

  .composer-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .queue-badge {
    position: absolute;
    top: -4px;
    right: -4px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: #ef4444;
    color: white;
    font-size: 10px;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>