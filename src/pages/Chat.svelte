<script lang="ts">
  import { t } from "../i18n";
  // ianox — workspace: proyecto + chat + archivos + terminal
  import { onMount, tick, untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import type { AgentConfig, Config, FileDiff, ImageAttachment, ModelInfo, Message, ProjectInfo, StoredSession } from "../types";
  import { app, refreshSessions, refreshProjects, navigate, adoptSession, closeSessionTab, projectIcon, newSession } from "../app.svelte";
  import LogoMark from "../components/LogoMark.svelte";
  import Icon from "../components/Icon.svelte";
  import ModelPicker from "../components/ModelPicker.svelte";
  import { toApiMessages, buildUserContent, splitAttachments, buildTranscript } from "../chat-history";
  import Markdown from "../components/Markdown.svelte";
  import { detectTrigger } from "../composer-suggest";
  import AssistantParts from "../components/AssistantParts.svelte";
  import ContextMeter from "../components/ContextMeter.svelte";
  import Kbd from "../components/Kbd.svelte";
  import ReviewPanel from "../components/ReviewPanel.svelte";
  import ContextPanel from "../components/ContextPanel.svelte";
  import QuickSetup from "../components/QuickSetup.svelte";
  import FileExplorer from "../components/FileExplorer.svelte";
  import FileViewer from "../components/FileViewer.svelte";
  import OpenInMenu from "../components/OpenInMenu.svelte";
  import BrowserPanel from "../components/BrowserPanel.svelte";
  import TerminalPanel from "../components/TerminalPanel.svelte";

  type WorkTab = {
    id: string;
    kind: "files" | "terminal" | "file" | "browser" | "review" | "context" | "launcher";
    title: string;
    path?: string;
    url?: string;
  };

  type Attachment = { path: string; name: string; content: string; truncated: boolean };

  let models = $state<ModelInfo[]>([]);
  let selectedModel = $state("");
  /** Provider that owns the selected model (the same id can exist in several). */
  let selectedProvider = $state("");
  let messages = $state<Message[]>([]);
  let input = $state("");
  let streaming = $state(false);
  const temperature = 0.7;
  // null: the backend uses the model's own output limit (a small cap truncates tool calls).
  const maxTokens: number | null = null;
  let systemPrompt = $state(t("You are a helpful, concise assistant."));
  let loadingModels = $state(true);
  let error = $state<string | null>(null);
  let sessionTitle = $state("");
  let titleMenu = $state(false);
  let extMenu = $state(false);
  let msgMenu = $state<number | null>(null);
  let extCounts = $state<{ mcp: number; plugins: number; skills: number; lsp: number } | null>(null);
  let usage = $state<{
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
    requests: number;
    last_ts: number;
    cost_usd: number | null;
    context_tokens: number | null;
  } | null>(null);

  let activeProject = $state<ProjectInfo | null>(null);
  let tabs = $state<WorkTab[]>([]);
  let activeTab = $state<string | null>(null);
  let terminalCount = $state(0);
  let modelMenu = $state(false);
  let attachments = $state<Attachment[]>([]);
  /** Screenshots pasted into the composer; they travel as base64 to the model. */
  let images = $state<ImageAttachment[]>([]);
  let agentMenu = $state(false);
  let treeOpen = $state(true);
  let sideOpen = $state(false);
  let closedTabs = $state<WorkTab[]>([]);
  let expandedClosed = $state(false);
  let chatWidth = $state(420);

  let agents = $state<AgentConfig[]>([]);
  let agentId = $state("build");

  let projectModal = $state(false);
  let projectName = $state("");
  let projectPath = $state("");
  let projectError = $state<string | null>(null);

  let textarea: HTMLTextAreaElement;

  let activeListeners: UnlistenFn[] = [];
  let currentRequestId: string | null = null;
  const sessionRef: { id: string | null; title: string } = { id: null, title: "" };

  async function loadUsage(): Promise<void> {
    const id = sessionRef.id;
    if (!id) {
      usage = null;
      return;
    }
    try {
      usage = await invoke("get_session_usage", { sessionId: id });
    } catch {
      usage = null;
    }
    // The model catalog (context limits) can finish downloading after the list was first loaded.
    const known = models.find((m) => m.id === selectedModel && m.provider === currentProvider);
    if (known && known.context === null) {
      invoke<ModelInfo[]>("list_models").then((list) => (models = list)).catch(() => {});
    }
  }

  async function loadExtensions(): Promise<void> {
    try {
      extCounts = await invoke("extensions_summary", { project: activeProject?.path ?? null });
    } catch {
      extCounts = null;
    }
  }

  function toggleExtMenu(): void {
    extMenu = !extMenu;
    if (extMenu) loadExtensions();
  }

  function openExtensions(tab: string): void {
    extMenu = false;
    app.extensionsTab = tab as "mcp";
    navigate("extensions");
  }

  async function loadProjects(): Promise<void> {
    try {
      const config = await invoke<Config>("get_config");
      const activeId = config.active_project ?? null;
      activeProject = (config.projects ?? []).find((p) => p.id === activeId) ?? null;
    } catch (e) {
      console.error("Error cargando proyectos:", e);
    }
  }

  onMount(() => {
    let cancelled = false;

    const load = async () => {
      try {
        const data = await invoke<ModelInfo[]>("list_models");
        if (cancelled) return;
        models = data;
        if (data.length > 0) selectedModel = selectedModel || data[0].id;
      } catch (e) {
        console.error("Error cargando modelos:", e);
      } finally {
        if (!cancelled) loadingModels = false;
      }
      try {
        const agentList = await invoke<AgentConfig[]>("list_agents");
        if (!cancelled && agentList.length > 0) {
          agents = agentList;
          const first = agentList[0];
          agentId = first.id;
          if (first.model) selectedModel = first.model;
          if (first.system_prompt) systemPrompt = first.system_prompt;
          restoreLast();
        }
      } catch (e) {
        console.error("Error cargando agentes:", e);
      }
      if (!cancelled) {
        await loadProjects();
        loadExtensions();
      }
    };

    load();

    const openPanel = () => {
      sideOpen = !sideOpen;
      if (sideOpen && tabs.length === 0) openLauncher();
    };
    const insideTerminal = (target: EventTarget | null): boolean => {
      const el = target as HTMLElement | null;
      return !!el?.closest?.(".terminal-host, .xterm");
    };
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();

      // Ctrl/Cmd + E: Toggle del panel lateral / lanzador
      if (mod && !e.altKey && !e.shiftKey && key === "e") {
        e.preventDefault();
        openPanel();
        return;
      }

      // Ctrl/Cmd + J: Conmutar la terminal (abrir / enfocar / cerrar panel)
      if (mod && !e.altKey && !e.shiftKey && key === "j") {
        e.preventDefault();
        toggleTerminal();
        return;
      }

      // Ctrl/Cmd + P: Abrir archivos en el panel de trabajo (estándar editor)
      if (mod && !e.altKey && !e.shiftKey && key === "p") {
        if (activeProject) {
          e.preventDefault();
          openFiles();
          return;
        }
      }

      // Ctrl/Cmd + I: Abrir navegador en el panel
      if (mod && !e.altKey && !e.shiftKey && key === "i") {
        e.preventDefault();
        openBrowser();
        return;
      }

      // Ctrl/Cmd + G: Abrir revisión (diff/cambios Git) en el panel
      if (mod && !e.altKey && !e.shiftKey && key === "g") {
        if (activeProject) {
          e.preventDefault();
          openReview();
          return;
        }
      }

      // Ctrl/Cmd + Shift + T:
      // Si el foco está en la terminal, añade una nueva pestaña de terminal (estándar terminal)
      // Si no, reabre la última pestaña cerrada (estándar navegador)
      if (mod && e.shiftKey && key === "t") {
        e.preventDefault();
        if (insideTerminal(e.target)) {
          openTerminal();
        } else {
          reopenLastClosed();
        }
        return;
      }

      // Ctrl/Cmd + T: Nueva pestaña en el panel según contexto
      if (mod && !e.altKey && !e.shiftKey && key === "t") {
        if (insideTerminal(e.target)) {
          // Si estamos en terminal, Ctrl+T abre nueva terminal
          e.preventDefault();
          openTerminal();
          return;
        }
        const activeT = tabs.find((t) => t.id === activeTab);
        if (activeT?.kind === "browser") {
          e.preventDefault();
          openBrowser();
          return;
        }
      }

      // Ctrl/Cmd + W: Cierra la pestaña activa del panel
      // No se intercepta si estamos tecleando en la terminal (el shell usa Ctrl+W para borrar una palabra).
      if (mod && !e.shiftKey && key === "w") {
        if (!sideOpen || !activeTab) return;
        if (insideTerminal(e.target)) return;
        e.preventDefault();
        closeTab(activeTab);
      }
    };
    window.addEventListener("ianox:toggle-panel", openPanel);
    window.addEventListener("keydown", onKey);

    return () => {
      cancelled = true;
      window.removeEventListener("ianox:toggle-panel", openPanel);
      window.removeEventListener("keydown", onKey);
      if (currentRequestId) invoke("chat_cancel", { requestId: currentRequestId }).catch(() => {});
      cleanupListeners();
    };
  });

  // ── Load the selected session ──
  let loadToken = 0;
  $effect(() => {
    const id = app.activeSessionId;
    if (id === sessionRef.id) return;
    sessionRef.id = id;
    const token = ++loadToken;

    untrack(() => {
      if (!id) {
        sessionRef.title = "";
        sessionTitle = "";
        messages = [];
        usage = null;
        return;
      }
      (async () => {
        try {
          const stored = await invoke<StoredSession>("get_session", { id });
          if (token !== loadToken) return;
          sessionRef.title = stored.title;
          sessionTitle = stored.title;
          stickToBottom = true;
          messages = stored.messages.map((message) => ({
            ...message,
            content: stripDsml(message.content),
            // Saved tools only keep their duration; rebuild the timestamps from it.
            tools: message.tools?.map((tool) => {
              const saved = tool as typeof tool & { duration_ms?: number };
              return { ...tool, startedAt: 0, endedAt: saved.duration_ms ?? undefined };
            }),
          }));
          loadUsage();
          if (stored.agent_id) agentId = stored.agent_id;
          if (stored.model) selectedModel = stored.model;
        } catch (e) {
          if (token === loadToken) error = String(e);
        }
      })();
    });
  });

  /** Whether the streamed answer should keep the view pinned to the newest line. */
  let stickToBottom = $state(true);
  let chatScroll: HTMLDivElement;
  let _programmaticScroll = false;

  function followStream(): void {
    const el = chatScroll;
    if (!el) return;
    // Si el scroll fue programático, ignoramos este evento y no cambiamos stickToBottom
    if (_programmaticScroll) { _programmaticScroll = false; return; }
    // Stop following as soon as the user scrolls up to read something.
    stickToBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
  }

  // Auto-scroll mientras llega el stream: se mueve el contenedor, no la ventana.
  $effect(() => {
    void messages.length;
    void (messages[messages.length - 1]?.content.length ?? 0);
    void compacting;
    if (!stickToBottom) return;
    tick().then(() => {
      const el = chatScroll;
      if (!el || !stickToBottom) return;
      _programmaticScroll = true;
      el.scrollTop = el.scrollHeight;
    });
  });

  // Auto-alto del textarea
  $effect(() => {
    void input;
    const el = textarea;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 220)}px`;
  });

  // External "open project" request
  let lastProjectRequest = -1;
  $effect(() => {
    const req = app.projectRequest;
    if (req === lastProjectRequest) return;
    lastProjectRequest = req;
    if (req > 0) {
      // Consume the request: if the chat remounts (e.g. when creating the
      // proyecto) no debe reabrir el modal.
      app.projectRequest = 0;
      untrack(() => {
        projectName = "";
        projectPath = "";
        projectError = null;
        projectModal = true;
      });
    }
  });

  // ── Per-session workbench persistence ──
  let restored = false;
  let restoredKey: string | null = null;

  $effect(() => {
    const sessionId = app.activeSessionId;
    const key = sessionId ? `ianox.workbench.session.${sessionId}` : "ianox.workbench.draft";
    if (key === restoredKey) return;
    restoredKey = key;

    untrack(() => {
      restored = false;

      // A new session inherits the draft tools.
      if (sessionId) {
        try {
          if (!localStorage.getItem(key)) {
            const draft = localStorage.getItem("ianox.workbench.draft");
            if (draft) {
              localStorage.setItem(key, draft);
              localStorage.removeItem("ianox.workbench.draft");
            }
          }
        } catch {
          /* almacenamiento no disponible */
        }
      }

      try {
        const raw = localStorage.getItem(key);
        if (raw) {
          const data = JSON.parse(raw) as {
            tabs?: WorkTab[];
            activeTab?: string | null;
            closedTabs?: WorkTab[];
            terminalCount?: number;
            chatWidth?: number;
            treeOpen?: boolean;
            sideOpen?: boolean;
          };
          const nextTabs = (data.tabs ?? []).filter((t) => t.kind !== "launcher");
          tabs = nextTabs;
          activeTab = nextTabs.some((t) => t.id === data.activeTab)
            ? (data.activeTab ?? null)
            : (nextTabs[0]?.id ?? null);
          closedTabs = (data.closedTabs ?? []).slice(0, 30);
          terminalCount = data.terminalCount ?? nextTabs.filter((t) => t.kind === "terminal").length;
          if (typeof data.chatWidth === "number") chatWidth = data.chatWidth;
          if (typeof data.treeOpen === "boolean") treeOpen = data.treeOpen;
          // Only reopen the panel if there is something in it.
          sideOpen = !!data.sideOpen && nextTabs.length > 0;
        } else {
          tabs = [];
          closedTabs = [];
          activeTab = null;
        }
      } catch {
        tabs = [];
        closedTabs = [];
        activeTab = null;
      }
      restored = true;
    });
  });

  $effect(() => {
    const sessionId = app.activeSessionId;
    const snapshot = JSON.stringify({
      tabs: tabs.filter((t) => t.kind !== "launcher"),
      activeTab,
      closedTabs: closedTabs.slice(0, 30),
      terminalCount,
      chatWidth,
      treeOpen,
      sideOpen,
    });
    if (!restored) return;
    const key = sessionId ? `ianox.workbench.session.${sessionId}` : "ianox.workbench.draft";
    try {
      localStorage.setItem(key, snapshot);
    } catch {
      /* almacenamiento lleno */
    }
  });

  // ── Chat ──
  function cleanupListeners(): void {
    activeListeners.forEach((fn) => fn());
    activeListeners = [];
  }

  function applyDelta(text: string): void {
    const last = messages[messages.length - 1];
    if (!last || last.role !== "assistant") return;
    last.content += text;
    const parts = (last.parts ??= []);
    const tail = parts[parts.length - 1];
    if (tail?.type === "text") tail.text += text;
    // A whitespace-only delta is the separator between rounds; parts already separate.
    else if (text.trim()) parts.push({ type: "text", text });
  }

  function addToolCalls(calls: Array<{ id: string; name: string; arguments: string }>): void {
    const last = messages[messages.length - 1];
    if (!last || last.role !== "assistant") return;
    const now = Date.now();
    last.tools = [
      ...(last.tools ?? []),
      ...calls.map((call) => ({ ...call, startedAt: now })),
    ];
    const parts = (last.parts ??= []);
    for (const call of calls) {
      parts.push({ type: "tool-call", id: call.id, name: call.name, arguments: call.arguments });
    }
  }

  function addToolResults(
    results: Array<{ call_id: string; output: string; is_error: boolean; diff?: FileDiff }>
  ): void {
    const last = messages[messages.length - 1];
    if (!last || last.role !== "assistant" || !last.tools) return;
    const now = Date.now();
    for (const result of results) {
      const tool = last.tools.find((t) => t.id === result.call_id);
      if (tool) {
        tool.output = result.output;
        tool.is_error = result.is_error;
        if (result.diff) tool.diff = result.diff;
        tool.endedAt = now;
      }
    }
  }

  function applyReasoning(text: string): void {
    const last = messages[messages.length - 1];
    if (!last || last.role !== "assistant") return;
    if (last.reasoning === undefined) last.reasoningStartedAt = Date.now();
    last.reasoning = (last.reasoning ?? "") + text;
    const parts = (last.parts ??= []);
    const tail = parts[parts.length - 1];
    if (tail?.type === "reasoning") tail.text += text;
    else parts.push({ type: "reasoning", text });
  }

  function finishAnswer(): void {
    const last = messages[messages.length - 1];
    if (last && last.role === "assistant") last.content = stripDsml(last.content);
  }

  function finishReasoning(): void {
    const last = messages[messages.length - 1];
    if (!last || last.role !== "assistant" || last.reasoningMs !== undefined) return;
    if (last.reasoningStartedAt) last.reasoningMs = Date.now() - last.reasoningStartedAt;
  }

  function persistSession(): void {
    const id = sessionRef.id;
    if (!id) return;
    const msgs = ($state.snapshot(messages) as Message[]).map((m) =>
      m.tools
        ? {
            ...m,
            tools: m.tools.map((tool) => ({
              ...tool,
              duration_ms: tool.endedAt ? tool.endedAt - tool.startedAt : undefined,
            })),
          }
        : m
    );
    const firstUser = placeholderTitle(msgs.find((m) => m.role === "user")?.content ?? "");
    const title = sessionRef.title || firstUser || t("New conversation");
    sessionRef.title = title;
    sessionTitle = title;

    invoke("save_session", {
      session: {
        id,
        title,
        project_id: activeProject?.id ?? null,
        agent_id: agentId,
        model: selectedModel,
        messages: msgs,
      },
    })
      .then(() => refreshSessions())
      .catch((e) => console.error("Could not save the session:", e));
  }

  /** Messages written while an answer was streaming; they go out one by one. */
  type QueuedMessage = { text: string; attachments: Attachment[]; images: ImageAttachment[] };
  let queued = $state<QueuedMessage[]>([]);

  /** Removes a queued message the user no longer wants to send. */
  function dequeue(index: number): void {
    queued = queued.filter((_, i) => i !== index);
  }

  async function handleSend(e?: Event): Promise<void> {
    e?.preventDefault();
    if (!selectedModel || compacting) return;
    const text = input.trim();
    if (!text && attachments.length === 0 && images.length === 0) return;

    // While an answer is streaming the message is queued and sent when it ends.
    if (streaming) {
      queued = [...queued, { text, attachments: [...attachments], images: [...images] }];
      input = "";
      attachments = [];
      images = [];
      return;
    }

    const outgoing = { text, attachments: [...attachments], images: [...images] };
    input = "";
    attachments = [];
    images = [];
    await deliver(outgoing.text, outgoing.attachments, outgoing.images);
  }

  /** Sends the first queued message, if the interface is free. */
  async function sendNextQueued(): Promise<void> {
    if (streaming || compacting) return;
    const next = queued[0];
    if (!next) return;
    queued = queued.slice(1);
    await deliver(next.text, next.attachments, next.images);
  }

  async function deliver(text: string, attach: Attachment[], imgs: ImageAttachment[]): Promise<void> {
    // Close to the model's limit: replace the history with a summary first, so this message fits.
    if (needsCompaction()) await compactConversation(true);

    const userMessage: Message = {
      role: "user",
      content: buildUserContent(text, attach),
      agent: agentId,
      model: selectedModel,
    };
    if (imgs.length > 0) userMessage.images = [...imgs];

    if (!sessionRef.id) {
      const newId = crypto.randomUUID();
      sessionRef.id = newId;
      sessionRef.title = placeholderTitle(userMessage.content);
      sessionTitle = sessionRef.title;
      adoptSession(newId);
    }

    const history = [
      ...messages.map((message) => ({ ...message, content: stripDsml(message.content) })),
      userMessage,
    ];
    messages = [...history, { role: "assistant", content: "" }];
    error = null;
    streaming = true;
    // A new turn always brings the view back to the newest message.
    stickToBottom = true;
    const busyId = sessionRef.id;
    if (busyId) app.busy[busyId] = true;
    const idle = () => {
      if (busyId) app.busy[busyId] = false;
    };

    cleanupListeners();
    const requestId = crypto.randomUUID();
    currentRequestId = requestId;
    const add = (fn: UnlistenFn) => activeListeners.push(fn);

    const fail = (message: string) => {
      error = message;
      const last = messages[messages.length - 1];
      if (last && last.role === "assistant" && last.content === "") {
        last.content = `⚠ ${message}`;
      }
      streaming = false;
      idle();
      permission = null;
      currentRequestId = null;
      cleanupListeners();
      persistSession();
      loadUsage();
      // A message written while this answer was streaming goes out now.
      void sendNextQueued();
    };

    add(
      await listen<string>(`chat://delta/${requestId}`, (event) => {
        applyDelta(event.payload);
      })
    );
    add(
      await listen<string>(`chat://reasoning/${requestId}`, (event) => {
        applyReasoning(event.payload);
      })
    );
    add(
      await listen<Array<{ id: string; name: string; arguments: string }>>(
        `chat://tool-call/${requestId}`,
        (event) => addToolCalls(event.payload)
      )
    );
    add(
      await listen<Array<{ call_id: string; output: string; is_error: boolean; diff?: FileDiff }>>(
        `chat://tool-result/${requestId}`,
        (event) => addToolResults(event.payload)
      )
    );
    add(
      await listen<PermissionAsk>(`chat://permission/${requestId}`, (event) => {
        permission = event.payload;
      })
    );
    add(
      await listen(`chat://done/${requestId}`, () => {
        streaming = false;
        idle();
        permission = null;
        currentRequestId = null;
        finishReasoning();
        finishAnswer();
        // An answer with nothing in it would look like the app did nothing; say so.
        const tail = messages[messages.length - 1];
        if (tail && tail.role === "assistant" && !tail.content.trim() && !tail.tools?.length && !tail.reasoning) {
          tail.content = `⚠ ${t("The model returned an empty answer.")}`;
        }
        cleanupListeners();
        persistSession();
        loadUsage();
        void maybeGenerateTitle();
        // Notificar al sistema sólo si la ventana no está a foco
        invoke("notify_if_unfocused", {
          title: "ianox",
          body: t("Response completed"),
        }).catch(() => {});
        // Pick up a message the user wrote while this answer was being generated.
        void sendNextQueued();
      })
    );
    add(
      await listen<string>(`chat://error/${requestId}`, (event) => {
        fail(event.payload);
      })
    );

    const activeAgent = agents.find((a) => a.id === agentId);
    const agentPrompt = activeAgent?.system_prompt?.trim() || systemPrompt.trim();
    const isPlan = (activeAgent?.mode ?? "build") === "plan";
    const modeInstructions = isPlan
      ? "# Mode: Plan (Read-Only Architect)\n- You are in PLAN mode. You MUST NOT execute commands that modify, write, or delete files, nor run shell commands that mutate the environment.\n- Analyze the codebase, explain architectural decisions, and outline plans step-by-step for the user."
      : "# Mode: Build (Full Access)\n- You are in BUILD mode. You have full access to edit, write, and run tools to accomplish the task.";

    const lastUser =
      [...history].reverse().find((m) => m.role === "user")?.content ?? "";
    const isSpanish = detectSpanish(lastUser);
    const systemText = [
      agentPrompt,
      modeInstructions,
      "You are an AI agent running in ianox, a coding agent harness. Help the user accomplish their goals using the tools you have available.",
      "# Harness\n- Responses are rendered as GitHub-flavored Markdown.\n- Prefer parallelizing independent tool calls.",
      isSpanish
        ? "# Comunicación\n- Responde siempre en español, con claridad y concisión, sin jerga innecesaria.\n- Usa rutas de archivo claras."
        : "# Communication\n- Keep responses clear and concise, and avoid unnecessary technical jargon.\n- Use clear file paths when referring to files.",
      `# Environment\n- Working directory: ${activeProject?.path ?? "(no project open)"}\n- Platform: ${navigator.platform}\n- Date: ${new Date().toISOString().slice(0, 10)}`,
      "# Doing the task\n- Act on what the user asked. Do not explore the codebase when the request does not need it: a simple request (write some text, answer a question, create a small file) is done directly, in one step.\n- Only read or search files when the answer depends on them, and stop exploring as soon as you have what you need.\n- When you create a file, write it once with the full content, then tell the user what you did.",
      "# Working in codebases\n- Keep changes consistent with the structure, naming, style, and patterns of the surrounding code.\n- Treat unfamiliar files or changes as potential user work and investigate before deleting or overwriting them.",
    ]
      .filter(Boolean)
      .join("\n\n");

    // Providers reject unknown keys: send only role/content, never UI state
    // (reasoning, parts, tools, agent, model…).
    const payload = [
      { role: "system", content: systemText },
      ...toApiMessages(history, stripDsml),
    ];

    try {
      await invoke("chat_stream", {
        requestId,
        model: selectedModel,
        messages: payload,
        temperature,
        maxTokens,
        sessionId: sessionRef.id,
        agentMode: agents.find((a) => a.id === agentId)?.mode ?? "build",
        providerId: currentProvider || null,
      });
    } catch (err) {
      fail(err instanceof Error ? err.message : String(err));
    }
  }

  /** What the user typed, without the files that travelled with the message. */
  const plain = (content: string) => splitAttachments(content).text;

  /** Provisional title: the first words typed (or the first file name when there is no text). */
  function placeholderTitle(content: string): string {
    const parts = splitAttachments(content);
    return (parts.text || parts.files[0]?.name || "").slice(0, 60);
  }

  /**
   * The reply only helps when the message says nothing by itself ("hola"); otherwise it
   * leaks details the user never asked about into the title.
   */
  function contextFor(userText: string, reply: string): string {
    return userText.trim().length < 20 ? reply : "";
  }

  /** Conversations already titled (or being titled) so a title is asked for only once. */
  const titled = new Set<string>();

  /**
   * After the first exchange, asks the model for a short title instead of showing the
   * first words the user typed. A title the user chose is never replaced.
   */
  async function maybeGenerateTitle(): Promise<void> {
    const id = sessionRef.id;
    if (!id || titled.has(id)) return;
    const users = messages.filter((m) => m.role === "user");
    const reply = messages.find((m) => m.role === "assistant")?.content ?? "";
    if (users.length !== 1 || !reply.trim() || reply.startsWith("⚠")) return;
    if (sessionRef.title !== placeholderTitle(users[0].content)) return;
    titled.add(id);
    try {
      const title = await invoke<string>("generate_session_title", {
        model: selectedModel,
        providerId: currentProvider || null,
        userText: plain(users[0].content),
        assistantText: contextFor(plain(users[0].content), reply),
      });
      // The user may have opened another conversation or renamed this one meanwhile.
      if (sessionRef.id !== id || sessionRef.title !== placeholderTitle(users[0].content)) return;
      sessionRef.title = title;
      sessionTitle = title;
      persistSession();
    } catch (e) {
      console.warn("Could not generate a title:", e);
    }
  }

  /** Names the conversation again from its first exchange (for when the first title missed). */
  async function regenerateTitle(): Promise<void> {
    titleMenu = false;
    const id = sessionRef.id;
    const first = messages.find((m) => m.role === "user");
    if (!id || !first) return;
    const reply = messages.find((m) => m.role === "assistant")?.content ?? "";
    try {
      const title = await invoke<string>("generate_session_title", {
        model: selectedModel,
        providerId: currentProvider || null,
        userText: plain(first.content),
        assistantText: contextFor(plain(first.content), reply),
      });
      if (sessionRef.id !== id) return;
      titled.add(id);
      sessionRef.title = title;
      sessionTitle = title;
      persistSession();
    } catch (e) {
      error = String(e);
    }
  }

  function handleStop(): void {
    const id = currentRequestId;
    if (id) invoke("chat_cancel", { requestId: id }).catch(() => {});
    // The provider may take a moment to notice; the interface stops right away.
    streaming = false;
    permission = null;
    currentRequestId = null;
    if (sessionRef.id) app.busy[sessionRef.id] = false;
    finishReasoning();
    finishAnswer();
    cleanupListeners();
    // An answer that never arrived leaves an empty bubble behind.
    const tail = messages[messages.length - 1];
    if (tail && tail.role === "assistant" && !tail.content.trim() && !tail.tools?.length && !tail.reasoning) {
      messages = messages.slice(0, -1);
    }
    persistSession();
    loadUsage();
  }

  function copyMessage(text: string): void {
    msgMenu = null;
    navigator.clipboard.writeText(plain(text)).catch(() => {});
  }

  function editMessage(index: number): void {
    msgMenu = null;
    const msg = messages[index];
    if (!msg) return;
    input = plain(msg.content);
    images = msg.images ? [...msg.images] : [];
    messages = messages.slice(0, index);
    persistSession();
    tick().then(() => textarea?.focus());
  }

  function deleteMessage(index: number): void {
    msgMenu = null;
    const next = messages.slice();
    next.splice(index, 1);
    if (next[index] && next[index].role === "assistant") next.splice(index, 1);
    messages = next;
    persistSession();
  }

  // ── Context compaction: replace the history with a summary when it gets too long ──
  let compacting = $state(false);
  /** Share of the model's context window at which a conversation is compacted before sending. */
  const COMPACT_AT = 0.85;

  function needsCompaction(): boolean {
    const used = usage?.context_tokens;
    const limit = models.find((m) => m.id === selectedModel && m.provider === currentProvider)?.context;
    // Right after a compaction the usage figure is still the old one until the next answer arrives.
    if (messages[messages.length - 1]?.compaction) return false;
    return !!used && !!limit && used / limit >= COMPACT_AT && messages.length >= 4;
  }

  /** Summarises the conversation so far and continues from the summary. Returns whether it did. */
  async function compactConversation(auto: boolean): Promise<boolean> {
    if (compacting || streaming) return false;
    const lastCut = messages.map((m) => !!m.compaction).lastIndexOf(true);
    const slice = messages.slice(Math.max(lastCut, 0));
    if (slice.filter((m) => !m.compaction).length < 2) {
      if (!auto) error = t("There is not enough conversation to compact yet.");
      return false;
    }
    const busyId = sessionRef.id;
    compacting = true;
    if (busyId) app.busy[busyId] = true;
    try {
      const summary = await invoke<string>("summarize_conversation", {
        model: selectedModel,
        providerId: currentProvider || null,
        transcript: buildTranscript(slice),
      });
      messages = [...messages, { role: "assistant", content: summary, compaction: true, model: selectedModel }];
      persistSession();
      return true;
    } catch (e) {
      error = `${t("Could not compact the conversation")}: ${e}`;
      return false;
    } finally {
      compacting = false;
      if (busyId) app.busy[busyId] = false;
    }
  }

  // ── "/" commands and "@" context in the message box ──
  interface Suggestion {
    id: string;
    title: string;
    detail?: string;
    hint?: string;
    run: () => void;
  }
  let suggest = $state<{
    kind: "slash" | "mention";
    start: number;
    end: number;
    items: Suggestion[];
    active: number;
    note?: string;
  } | null>(null);
  let mentionRequest = 0;

  // ── Questions from the agent ("may I run this?") ──
  interface PermissionAsk {
    id: string;
    tool: string;
    summary: string;
    detail: string;
    sandboxed: boolean;
    can_remember: boolean;
    warning: string | null;
  }
  let permission = $state<PermissionAsk | null>(null);

  function answerPermission(decision: "once" | "always" | "deny"): void {
    const ask = permission;
    permission = null;
    if (ask) invoke("permission_reply", { id: ask.id, decision }).catch(() => {});
  }

  function commandList(): Suggestion[] {
    const byMode = (mode: string) => agents.find((a) => a.mode === mode)?.id;
    return [
      { id: "new", title: t("New conversation"), run: () => newSession() },
      { id: "models", title: t("Select model"), run: () => { modelMenu = true; agentMenu = false; } },
      { id: "agent", title: t("Switch agent"), run: () => { agentMenu = true; modelMenu = false; } },
      { id: "plan", title: t("Plan mode: read-only"), run: () => { const id = byMode("plan"); if (id) handleSelectAgent(id); } },
      { id: "build", title: t("Build mode: can edit files"), run: () => { const id = byMode("build"); if (id) handleSelectAgent(id); } },
      { id: "compact", title: t("Compact the conversation"), run: () => void compactConversation(false) },
      { id: "title", title: t("Generate new title"), run: () => void regenerateTitle() },
      { id: "rename", title: t("Rename conversation"), run: () => void renameSession() },
      { id: "search", title: t("Search conversations"), hint: "Ctrl+K", run: () => (app.searchOpen = true) },
      { id: "terminal", title: t("Open terminal"), hint: "Ctrl+Alt+T", run: openTerminal },
      { id: "files", title: t("Open files"), run: openFiles },
      { id: "review", title: t("Review changes"), run: openReview },
      { id: "browser", title: t("Open browser"), run: openBrowser },
      { id: "typography", title: t("Typography"), run: () => (app.tuner = true) },
    ];
  }

  function updateSuggest(): void {
    const trigger = streaming ? null : detectTrigger(input, textarea?.selectionStart ?? input.length);
    if (!trigger) {
      suggest = null;
      mentionRequest++;
      return;
    }
    if (trigger.kind === "slash") {
      const q = trigger.query.toLowerCase();
      const items = commandList().filter((c) => c.id.startsWith(q) || c.title.toLowerCase().includes(q));
      suggest = items.length ? { kind: "slash", start: trigger.start, end: trigger.end, items, active: 0 } : null;
      return;
    }
    const keep = suggest?.kind === "mention" ? suggest.items : [];
    suggest = { kind: "mention", start: trigger.start, end: trigger.end, items: keep, active: 0 };
    void loadMentions(trigger.query, trigger.start, trigger.end);
  }

  async function loadMentions(query: string, start: number, end: number): Promise<void> {
    const mine = ++mentionRequest;
    try {
      const found = await invoke<string[]>("find_project_files", { query, limit: 12 });
      if (mine !== mentionRequest) return;
      const items = found.map((rel) => {
        const slash = rel.lastIndexOf("/");
        return {
          id: rel,
          title: slash >= 0 ? rel.slice(slash + 1) : rel,
          detail: slash >= 0 ? rel.slice(0, slash) : "",
          run: () => void attachProjectFile(rel),
        };
      });
      suggest = {
        kind: "mention",
        start,
        end,
        items,
        active: 0,
        note: items.length === 0 ? t("No matching files") : undefined,
      };
    } catch {
      if (mine !== mentionRequest) return;
      suggest = { kind: "mention", start, end, items: [], active: 0, note: t("Open a project to mention its files") };
    }
  }

  /** Adds a project file as context for the next message (the same chip the "+" button makes). */
  async function attachProjectFile(relative: string): Promise<void> {
    const root = activeProject?.path;
    if (!root) return;
    const path = `${root.replace(/\/$/, "")}/${relative}`;
    if (attachments.some((a) => a.path === path)) return;
    try {
      const file = await invoke<{ content: string; truncated: boolean }>("read_file", { path });
      const name = relative.split("/").pop() ?? relative;
      attachments = [...attachments, { path, name, content: file.content, truncated: file.truncated }];
    } catch (e) {
      error = `${relative}: ${e}`;
    }
  }

  function acceptSuggestion(item: Suggestion): void {
    const current = suggest;
    if (!current) return;
    // The typed "/cmd" or "@query" is replaced by its effect, not left in the message.
    const after = input.slice(current.end);
    input = input.slice(0, current.start) + (current.kind === "mention" ? after.replace(/^ /, "") : after);
    suggest = null;
    mentionRequest++;
    item.run();
    tick().then(() => {
      textarea?.focus();
      textarea?.setSelectionRange(current.start, current.start);
    });
  }

  let pasteTimeout: number | null = null;

  function handleKeyDown(e: KeyboardEvent): void {
    // Ctrl/Cmd+V: WebKit does not always emit `paste` when the clipboard holds an
    // image only. If the paste event has not dealt with it shortly after, ask the
    // window's clipboard directly.
    const isPaste =
      (e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "v";
    if (isPaste) {
      pasteHandled = false;
      if (pasteTimeout) window.clearTimeout(pasteTimeout);
      pasteTimeout = window.setTimeout(() => {
        if (!pasteHandled && !streaming) void addClipboardImage().then((added) => {
          if (added) pasteHandled = true;
        });
      }, 150);
    }

    if (suggest && suggest.items.length > 0) {
      const last = suggest.items.length - 1;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        suggest.active = Math.min(suggest.active + 1, last);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        suggest.active = Math.max(suggest.active - 1, 0);
        return;
      }
      if ((e.key === "Enter" && !e.shiftKey) || e.key === "Tab") {
        e.preventDefault();
        acceptSuggestion(suggest.items[suggest.active]);
        return;
      }
    }
    if (e.key === "Escape" && suggest) {
      e.preventDefault();
      suggest = null;
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  async function handleSetupDone(model: string): Promise<void> {
    try {
      const data = await invoke<ModelInfo[]>("list_models");
      models = data;
      if (model && data.some((m) => m.id === model)) selectedModel = model;
      else if (data.length > 0) selectedModel = data[0].id;
    } catch (e) {
      error = String(e);
    }
  }

  /** Remembers the last model and agent so a new chat starts where the user left off. */
  function saveLast(): void {
    try {
      localStorage.setItem(
        "ianox.chat.last",
        JSON.stringify({ model: selectedModel, provider: currentProvider, agent: agentId })
      );
    } catch {
      /* almacenamiento no disponible */
    }
  }

  /** The model the user marked as default in the picker, if any. */
  function defaultModel(): { id: string; provider: string } | null {
    try {
      const saved = JSON.parse(localStorage.getItem("ianox.model.default") ?? "[]") as { id: string; provider: string }[];
      return saved[0] ?? null;
    } catch {
      return null;
    }
  }

  function restoreLast(): void {
    try {
      const last = JSON.parse(localStorage.getItem("ianox.chat.last") ?? "null") as {
        model?: string;
        provider?: string;
        agent?: string;
      } | null;
      if (!last) return;
      if (last.agent && agents.some((a) => a.id === last.agent)) handleSelectAgent(last.agent);
      const preferred = defaultModel() ?? { id: last.model, provider: last.provider };
      const model = models.find((m) => m.id === preferred.id && m.provider === preferred.provider);
      if (model) {
        selectedModel = model.id;
        selectedProvider = model.provider;
      }
    } catch {
      /* estado guardado ilegible: se ignora */
    }
  }

  function handleSelectAgent(id: string): void {
    agentId = id;
    const agent = agents.find((a) => a.id === id);
    if (!agent) return;
    if (agent.model && models.some((m) => m.id === agent.model)) selectedModel = agent.model;
    systemPrompt = agent.system_prompt || "";
    saveLast();
  }

  // ── Workbench tabs ──
  function openLauncher(): void {
    sideOpen = true;
    const tab: WorkTab = { id: crypto.randomUUID(), kind: "launcher", title: t("New tab") };
    tabs = [...tabs, tab];
    activeTab = tab.id;
  }

  function consumeLauncher(): void {
    tabs = tabs.filter((t) => !(t.kind === "launcher" && t.id === activeTab));
  }

  function openFiles(): void {
    sideOpen = true;
    consumeLauncher();
    treeOpen = true;
    if (!tabs.some((t) => t.kind === "files")) {
      tabs = [...tabs, { id: "files", kind: "files", title: t("Open file") }];
    }
    activeTab = "files";
  }

  function toggleTerminal(): void {
    const activeTabData = tabs.find((t) => t.id === activeTab);
    if (sideOpen && activeTabData?.kind === "terminal") {
      sideOpen = false;
      return;
    }
    const termTab = tabs.find((t) => t.kind === "terminal");
    if (termTab) {
      sideOpen = true;
      activeTab = termTab.id;
    } else {
      openTerminal();
    }
  }

  function openTerminal(): void {
    sideOpen = true;
    consumeLauncher();
    const existing = new Set(
      tabs
        .filter((t) => t.kind === "terminal")
        .map((t) => {
          const match = t.title.match(/(\d+)$/);
          return match ? parseInt(match[1], 10) : 1;
        })
    );
    let num = 1;
    while (existing.has(num)) num++;
    const title = num === 1 && existing.size === 0 ? t("Terminal") : `${t("Terminal")} ${num}`;
    const tab: WorkTab = { id: crypto.randomUUID(), kind: "terminal", title };
    tabs = [...tabs, tab];
    activeTab = tab.id;
  }

  function openContext(): void {
    sideOpen = true;
    consumeLauncher();
    if (!tabs.some((t) => t.kind === "context")) {
      tabs = [...tabs, { id: "context", kind: "context", title: t("Context") }];
    }
    activeTab = "context";
  }

  function openReview(): void {
    sideOpen = true;
    consumeLauncher();
    if (!tabs.some((t) => t.kind === "review")) {
      tabs = [...tabs, { id: "review", kind: "review", title: t("Review") }];
    }
    activeTab = "review";
  }

  function openBrowser(): void {
    sideOpen = true;
    consumeLauncher();
    const tab: WorkTab = { id: crypto.randomUUID(), kind: "browser", title: t("Browser"), url: "" };
    tabs = [...tabs, tab];
    activeTab = tab.id;
  }

  function openFileTab(path: string, name: string): void {
    const id = `file:${path}`;
    if (!tabs.some((t) => t.id === id)) tabs = [...tabs, { id, kind: "file", title: name, path }];
    activeTab = id;
  }

  function closeTab(id: string): void {
    const index = tabs.findIndex((t) => t.id === id);
    const closing = tabs[index];
    const next = tabs.filter((t) => t.id !== id);
    if (closing && closing.kind !== "launcher") {
      closedTabs = [closing, ...closedTabs.filter((t) => t.title !== closing.title)].slice(0, 10);
    }
    if (activeTab === id) activeTab = next[Math.min(index, next.length - 1)]?.id ?? null;
    tabs = next;
    if (!next.some((t) => t.kind === "terminal")) {
      terminalCount = 0;
    }
    // With no tabs the panel closes, so Ctrl+` reopens it clean.
    if (next.length === 0) sideOpen = false;
  }

  function reopenTab(tab: WorkTab): void {
    closedTabs = closedTabs.filter((t) => t.title !== tab.title);
    if (!tabs.some((t) => t.id === tab.id)) tabs = [...tabs, tab];
    activeTab = tab.id;
  }

  function reopenLastClosed(): void {
    const tab = closedTabs[0];
    if (!tab) return;
    sideOpen = true;
    reopenTab(tab);
  }

  function stripDsml(text: string): string {
    let out = text;
    // Remove whole DSML blocks first (they can nest).
    for (let i = 0; i < 5; i++) {
      const next = out.replace(/<\|DSML\|[\s\S]*?<\/\|DSML\|[^>]*>/g, "");
      if (next === out) break;
      out = next;
    }
    return out.replace(/<\/?\|DSML\|[^>]*>/g, "").trim();
  }

  function detectSpanish(text: string): boolean {
    const sample = text.slice(0, 600).toLowerCase();
    const spanish = (sample.match(/\b(el|la|los|las|de|que|para|con|una|por|como|es|del|al|se|su|más|pero|esto|esta)\b/g) ?? []).length;
    const english = (sample.match(/\b(the|and|for|with|this|that|from|you|are|was|have|not|but|can)\b/g) ?? []).length;
    return spanish >= english;
  }

  const currentProvider = $derived(
    models.find((m) => m.id === selectedModel && m.provider === selectedProvider)?.provider ??
      models.find((m) => m.id === selectedModel)?.provider ??
      ""
  );

  function modelLabel(id: string): string {
    if (!id) return t("Model");
    const info =
      models.find((m) => m.id === id && m.provider === currentProvider) ?? models.find((m) => m.id === id);
    return info ? `${info.provider_name} / ${info.name}` : id;
  }

  function toolArg(argumentsJson: string): string {
    try {
      const args = JSON.parse(argumentsJson) as Record<string, unknown>;
      const value = args.path ?? args.command ?? args.query ?? args.pattern;
      return typeof value === "string" ? value : argumentsJson.slice(0, 80);
    } catch {
      return argumentsJson.slice(0, 80);
    }
  }

  function fmtCost(usd: number): string {
    const decimals = usd > 0 && usd < 0.01 ? 4 : 2;
    return `${usd.toFixed(decimals).replace(".", ",")} US$`;
  }

  function fmtTokens(n: number): string {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1).replace(".", ",")} M`;
    if (n >= 10_000) return `${Math.round(n / 1000)} mil`;
    if (n >= 1000) return `${(n / 1000).toFixed(1).replace(".", ",")} mil`;
    return String(n);
  }

  function iconForTab(tab: WorkTab): string {
    if (tab.kind === "terminal") return "terminal";
    if (tab.kind === "browser") return "browser";
    if (tab.kind === "file") return "file";
    return "folder";
  }

  function dirOf(path?: string): string {
    if (!path) return "";
    const parts = path.split("/");
    parts.pop();
    return parts.join("/") || "/";
  }

  function startChatResize(e: MouseEvent): void {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = chatWidth;
    const onMove = (event: MouseEvent) => {
      chatWidth = Math.min(
        Math.max(startWidth + (event.clientX - startX), 340),
        Math.max(window.innerWidth - 420, 380)
      );
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  // ── Proyecto ──
  async function renameSession(): Promise<void> {
    titleMenu = false;
    const id = sessionRef.id;
    if (!id) return;
    const next = prompt(t("New conversation title"), sessionTitle || sessionRef.title);
    if (!next || !next.trim()) return;
    try {
      const stored = await invoke<StoredSession>("get_session", { id });
      await invoke("save_session", { session: { ...stored, title: next.trim() } });
      sessionRef.title = next.trim();
      sessionTitle = next.trim();
      refreshSessions();
    } catch (e) {
      error = String(e);
    }
  }

  async function deleteCurrentSession(): Promise<void> {
    titleMenu = false;
    const id = sessionRef.id;
    if (!id) return;
    if (!confirm(t("Delete this conversation?"))) return;
    try {
      await invoke("delete_session", { id });
      closeSessionTab(id);
      sessionRef.id = null;
      sessionRef.title = "";
      sessionTitle = "";
      messages = [];
      usage = null;
      refreshSessions();
    } catch (e) {
      error = String(e);
    }
  }

  function handleNewProject(): void {
    projectName = "";
    projectPath = "";
    projectError = null;
    projectModal = true;
  }

  async function chooseFolder(): Promise<void> {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t("Select a folder as the project"),
      });
      if (!selected || Array.isArray(selected)) return;
      projectPath = selected;
      if (!projectName) {
        const parts = selected.split("/").filter(Boolean);
        projectName = parts[parts.length - 1] ?? "";
      }
    } catch (e) {
      projectError = String(e);
    }
  }

  async function handleCreateProject(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    projectError = null;
    try {
      const project = await invoke<ProjectInfo>("add_project", {
        name: projectName,
        path: projectPath,
      });
      await loadProjects();
      activeProject = project;
      messages = [];
      projectModal = false;
      refreshProjects();
    } catch (err) {
      projectError = String(err);
    }
  }

  async function handleAttach(): Promise<void> {
    try {
      const selected = await open({ multiple: true, title: t("Attach files") });
      const paths = (Array.isArray(selected) ? selected : selected ? [selected] : []).filter(
        (value): value is string => typeof value === "string"
      );
      if (paths.length === 0) return;

      const loaded = await Promise.all(
        paths.map(async (path) => {
          try {
            const file = await invoke<{ content: string; truncated: boolean }>("read_file", { path });
            const name = path.split("/").filter(Boolean).pop() ?? path;
            return { path, name, content: file.content, truncated: file.truncated };
          } catch (e) {
            error = `${path}: ${e}`;
            return null;
          }
        })
      );

      const next = [...attachments];
      for (const file of loaded) {
        if (file && !next.some((a) => a.path === file.path)) next.push(file);
      }
      attachments = next;
    } catch (e) {
      error = String(e);
    }
  }

  function removeAttachment(path: string): void {
    attachments = attachments.filter((a) => a.path !== path);
  }

  function removeImage(index: number): void {
    images = images.filter((_, i) => i !== index);
  }

  /** Longest side of a pasted image; bigger ones are scaled down before they reach the model. */
  const MAX_IMAGE_SIDE = 1600;
  /** Above this, the PNG is re-encoded as JPEG so one screenshot does not blow up the request. */
  const MAX_DATA_URL = 1_600_000;

  function readAsDataUrl(blob: Blob): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = () => reject(new Error("No se pudo leer la imagen"));
      reader.readAsDataURL(blob);
    });
  }

  function loadPicture(dataUrl: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
      const picture = new Image();
      picture.onload = () => resolve(picture);
      picture.onerror = () => reject(new Error("No se pudo decodificar la imagen"));
      picture.src = dataUrl;
    });
  }

  /** Scales a picture down and re-encodes it so one screenshot stays within request limits. */
  async function imageFromDataUrl(dataUrl: string, name: string): Promise<ImageAttachment> {
    const picture = await loadPicture(dataUrl);
    const scale = Math.min(1, MAX_IMAGE_SIDE / Math.max(picture.naturalWidth, picture.naturalHeight));
    const width = Math.max(1, Math.round(picture.naturalWidth * scale));
    const height = Math.max(1, Math.round(picture.naturalHeight * scale));
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("No se pudo preparar la imagen");
    context.drawImage(picture, 0, 0, width, height);

    let mime = "image/png";
    let scaled = canvas.toDataURL(mime);
    if (scaled.length > MAX_DATA_URL) {
      mime = "image/jpeg";
      scaled = canvas.toDataURL(mime, 0.85);
    }
    return { name, mime, data: scaled.slice(scaled.indexOf(",") + 1) };
  }

  async function addImages(files: File[]): Promise<void> {
    for (const file of files) {
      try {
        const extension = file.type === "image/jpeg" ? "jpg" : "png";
        const name =
          file.name && file.name !== "image.png"
            ? file.name
            : `Captura ${images.length + 1}.${extension}`;
        images = [...images, await imageFromDataUrl(await readAsDataUrl(file), name)];
      } catch (e) {
        error = String(e);
      }
    }
  }

  /** Files copied in a file manager arrive as `text/uri-list`. */
  async function addPastedPaths(list: string): Promise<boolean> {
    const paths = list
      .split(/\r?\n/)
      .filter((line) => line.startsWith("file://"))
      .map((line) => {
        try {
          return decodeURIComponent(new URL(line).pathname);
        } catch {
          return null;
        }
      })
      .filter((path): path is string => !!path);
    if (paths.length === 0) return false;

    for (const path of paths) {
      const name = path.split("/").filter(Boolean).pop() ?? path;
      if (attachments.some((attached) => attached.path === path) || images.some((image) => image.name === name && name !== "")) {
        continue;
      }
      try {
        if (/\.(png|jpe?g|gif|webp|bmp|avif)$/i.test(path)) {
          const file = await invoke<{ mime: string; data: string }>("read_file_binary", { path });
          images = [...images, await imageFromDataUrl(`data:${file.mime};base64,${file.data}`, name)];
        } else {
          const file = await invoke<{ content: string; truncated: boolean }>("read_file", { path });
          attachments = [...attachments, { path, name, content: file.content, truncated: file.truncated }];
        }
      } catch (e) {
        error = `${name}: ${e}`;
      }
    }
    return true;
  }

  /** A paste event already took care of the clipboard (set by `handlePaste`). */
  let pasteHandled = false;
  let clipboardReading = false;

  /** Adds whatever image the window's own clipboard holds. `true` if there was one. */
  async function addClipboardImage(): Promise<boolean> {
    if (clipboardReading) return false;
    clipboardReading = true;
    try {
      const clipboard = await invoke<{ mime: string; data: string } | null>("read_clipboard_image").catch(
        () => null
      );
      if (!clipboard) return false;
      const dataUrl = `data:${clipboard.mime};base64,${clipboard.data}`;
      images = [...images, await imageFromDataUrl(dataUrl, `Captura ${images.length + 1}.png`)];
      return true;
    } catch (e) {
      error = String(e);
      return false;
    } finally {
      clipboardReading = false;
    }
  }

  /**
   * Pasted pictures become attachments. WebKitGTK does not put the clipboard image in
   * the event, so when the event brings nothing usable the window's own clipboard is
   * asked instead. Plain text is left to the browser's normal paste.
   */
  async function handlePaste(event: ClipboardEvent): Promise<void> {
    if (pasteTimeout) {
      window.clearTimeout(pasteTimeout);
      pasteTimeout = null;
    }
    pasteHandled = true;
    const data = event.clipboardData;
    if (!data) return;

    const files: File[] = [];
    for (const item of data.items) {
      if (item.kind === "file" && item.type.startsWith("image/")) {
        const file = item.getAsFile();
        if (file) files.push(file);
      }
    }
    for (const file of data.files) {
      if (file.type.startsWith("image/") && !files.includes(file)) files.push(file);
    }
    if (files.length > 0) {
      event.preventDefault();
      pasteHandled = true;
      await addImages(files);
      return;
    }

    const uris = data.getData("text/uri-list");
    if (uris && (await addPastedPaths(uris))) {
      event.preventDefault();
      pasteHandled = true;
      return;
    }

    if (data.getData("text/plain")) {
      // A normal text paste: nothing to do here.
      pasteHandled = true;
      return;
    }

    if (await addClipboardImage()) {
      event.preventDefault();
      pasteHandled = true;
    }
  }

  const activeTabData = $derived(tabs.find((t) => t.id === activeTab));
  const activeKind = $derived(activeTabData?.kind);
  const showFileSidebar = $derived(
    treeOpen && !!activeProject && (activeKind === "file" || activeKind === "files")
  );
  const workbenchVisible = $derived(sideOpen && tabs.length > 0);
  const openSide = (): void => {
    sideOpen = true;
    if (tabs.length === 0) openLauncher();
  };
  const launcherItems = $derived([
    { id: "file", title: t("Open file"), icon: "folder", keys: ["Ctrl", "P"], disabled: !activeProject, run: openFiles },
    { id: "terminal", title: "Terminal", icon: "terminal", keys: ["Ctrl", "J"], disabled: false, run: openTerminal },
    { id: "browser", title: t("Browser"), icon: "browser", keys: ["Ctrl", "I"], disabled: false, run: openBrowser },
    { id: "review", title: t("Review"), icon: "diff", keys: ["Ctrl", "G"], disabled: !activeProject, run: openReview },
  ]);

  const visibleClosed = $derived(expandedClosed ? closedTabs : closedTabs.slice(0, 5));

  const extRows = $derived([
    { id: "mcp", label: "MCP", icon: "mcp", count: extCounts?.mcp ?? 0 },
    { id: "plugins", label: t("Plugins"), icon: "extensions", count: extCounts?.plugins ?? 0 },
    { id: "skills", label: t("Skills"), icon: "agents", count: extCounts?.skills ?? 0 },
    { id: "lsp", label: "LSP", icon: "code", count: extCounts?.lsp ?? 0 },
  ]);

  const noModels = $derived(!loadingModels && models.length === 0);
  const currentAgent = $derived(agents.find((a) => a.id === agentId));
  const currentAgentMode = $derived(currentAgent?.mode ?? "build");
  const currentAgentName = $derived(currentAgent?.name ?? t("Agent"));
</script>

{#snippet messageBubble(message: Message, isStreaming: boolean, index: number)}
  {#if message.compaction}
    <div class="compaction">
      <div class="c-rule"><span>{t("Conversation compacted")}</span></div>
      <details>
        <summary>{t("Show summary")}</summary>
        <div class="c-body"><Markdown source={message.content} /></div>
      </details>
    </div>
  {:else}
  {@const isUser = message.role === "user"}
  <div style={`display:flex;justify-content:${isUser ? "flex-end" : "flex-start"}`}>
    <div style={`max-width:${isUser ? "78%" : "100%"};display:flex;flex-direction:column;gap:4px`}>
      <div
        style={`padding:${isUser ? "10px 14px" : "2px 0"};border-radius:${isUser ? "12px 12px 4px 12px" : 0};background:${isUser ? "var(--bg-tertiary)" : "transparent"};color:var(--text-primary);border:${isUser ? "1px solid var(--border-primary)" : "none"};white-space:${isUser ? "pre-wrap" : "normal"};word-break:break-word`}
      >
        {#if isUser}
          {@const parts = splitAttachments(message.content)}
          {parts.text}
          {#if parts.files.length > 0}
            <div class="msg-files">
              {#each parts.files as file (file.path)}
                <span class="attachment-chip" title={file.path}>{file.name}</span>
              {/each}
            </div>
          {/if}
          {#if message.images?.length}
            <div class="msg-images">
              {#each message.images as image, i (i)}
                <img src={`data:${image.mime};base64,${image.data}`} alt={image.name} title={image.name} />
              {/each}
            </div>
          {/if}
        {:else}
          <AssistantParts {message} streaming={isStreaming} clean={stripDsml} />
        {/if}
      </div>
      {#if isUser}
        <div class="msg-meta">
          <span class="msg-model">
            {agents.find((a) => a.id === message.agent)?.name ?? currentAgentName}
            {#if message.model}· {models.find((m) => m.id === message.model)?.name ?? message.model}{/if}
          </span>
          <div class="msg-menu-wrap">
            <button class="msg-more" title={t("Message actions")} onclick={() => (msgMenu = msgMenu === index ? null : index)}>
              ⋯
            </button>
            {#if msgMenu === index}
              <div class="menu-backdrop" role="presentation" onclick={() => (msgMenu = null)}></div>
              <div class="msg-menu">
                <button class="menu-item" onclick={() => editMessage(index)}><span>{t("Edit message")}</span></button>
                <button class="menu-item" onclick={() => copyMessage(message.content)}><span>{t("Copy text")}</span></button>
                <div class="menu-divider"></div>
                <button class="menu-item danger" onclick={() => deleteMessage(index)}><span>{t("Delete message")}</span></button>
              </div>
            {/if}
          </div>
        </div>
      {/if}
    </div>
  </div>
  {/if}
{/snippet}

<div class="workspace">
  <div class="workspace-main">
    <div
      class="workspace-chat"
      style={`flex:0 0 auto;width:${workbenchVisible ? `${chatWidth}px` : "100%"};transition:width 380ms cubic-bezier(0.22, 1, 0.36, 1)`}
    >
      {#if error}
        <div class="error-banner">
          <span>⚠ {error}</span>
          <button class="btn btn-ghost btn-sm" onclick={() => (error = null)}>{t("Close")}</button>
        </div>
      {/if}

      <div class="session-header">
        <span class="session-header-mark">
          {#if projectIcon(activeProject?.id)}
            <img class="project-icon" src={projectIcon(activeProject?.id)} alt="" />
          {:else}
            <LogoMark size={16} radius={4} />
          {/if}
        </span>
        <span class="session-header-project">{activeProject?.name ?? "ianox"}</span>
        {#if sessionTitle}
          <span class="session-header-sep">/</span>
          <span class="session-header-name" title={sessionTitle}>{sessionTitle}</span>
        {/if}

        <button
          type="button"
          class={`session-agent-badge ${currentAgentMode}`}
          title={currentAgentMode === "build" ? t("Build mode: can edit files") : t("Plan mode: read-only")}
          onclick={() => {
            const nextMode = currentAgentMode === "build" ? "plan" : "build";
            const target = agents.find((a) => a.mode === nextMode);
            if (target) handleSelectAgent(target.id);
          }}
        >
          <span class="badge-dot"></span>
          <span>{currentAgentName}</span>
          <span class="badge-mode">({currentAgentMode === "build" ? t("full access") : t("read-only")})</span>
        </button>

        {#if app.activeSessionId}
          <div class="session-header-menu">
            <button class="session-header-dots" title={t("Session options")} onclick={() => (titleMenu = !titleMenu)}>
              ⋯
            </button>
            {#if titleMenu}
              <div class="menu-backdrop" role="presentation" onclick={() => (titleMenu = false)}></div>
              <div class="session-menu">
                <button class="menu-item" onclick={renameSession}><span>{t("Rename conversation")}</span></button>
                <button class="menu-item" onclick={regenerateTitle}><span>{t("Generate new title")}</span></button>
                <button class="menu-item danger" onclick={deleteCurrentSession}><span>{t("Delete conversation")}</span></button>
              </div>
            {/if}
          </div>
        {/if}

        <div class="session-header-spacer"></div>

        {#if usage && usage.total_tokens > 0}
          <ContextMeter
            context={usage.context_tokens ?? null}
            limit={models.find((m) => m.id === selectedModel && m.provider === currentProvider)?.context ?? null}
            total={usage.total_tokens}
            requests={usage.requests}
            cost={usage.cost_usd}
            formatTokens={fmtTokens}
            formatCost={fmtCost}
          />
        {/if}

        <div class="session-header-menu">
          <button class="session-header-action" title={t("Extensions")} onclick={toggleExtMenu}>
            <Icon name="extensions" size={15} />
          </button>
          {#if extMenu}
            <div class="menu-backdrop" role="presentation" onclick={() => (extMenu = false)}></div>
            <div class="session-menu ext-menu">
              <div class="menu-group-label">{t("Extensions")}</div>
              {#each extRows as row (row.id)}
                <button class="menu-item" onclick={() => openExtensions(row.id)}>
                  <span class="ext-row"><Icon name={row.icon} size={14} />{row.label}</span>
                  <span class="ext-count">{row.count}</span>
                </button>
              {/each}
              <div class="menu-divider"></div>
              <button class="menu-item" onclick={() => openExtensions("mcp")}>
                <span class="ext-row"><Icon name="extensions" size={14} />{t("View all")}</span>
              </button>
            </div>
          {/if}
        </div>

        <button class="session-header-action" title={t("Session context")} onclick={openContext}>
          <Icon name="info" size={15} />
        </button>

        {#if !workbenchVisible}
          <button class="session-header-action" onclick={openSide} title={`${t("Open panel")} (Ctrl+E)`}>
            <Icon name="panel-right" size={15} />
          </button>
        {/if}
      </div>

      <div class="chat-scroll" bind:this={chatScroll} onscroll={followStream}>
        {#if messages.length === 0}
          {#if noModels}
            <QuickSetup onDone={handleSetupDone} onAdvanced={() => navigate("providers")} />
          {:else}
            <div class="chat-empty">
              <LogoMark size={160} radius={36} />
              <h2>{t("How can I help?")}</h2>
              {#if !activeProject}
                <p>{t("Open a folder to work with files and terminal, or chat without a project.")}</p>
                <div class="chat-empty-actions">
                  <button class="btn btn-primary" onclick={handleNewProject}>{t("Open project")}</button>
                </div>
              {:else}
                <p>{t("Project")} <strong>{activeProject.name}</strong> · {activeProject.path}
                </p>
              {/if}
            </div>
          {/if}
        {:else}
          {#each messages as msg, i (i)}
            {@render messageBubble(msg, streaming && i === messages.length - 1 && msg.role === "assistant", i)}
          {/each}
        {/if}
        {#if compacting}
          <div class="compacting"><span class="tab-spin"></span>{t("Compacting the conversation…")}</div>
        {/if}
      </div>

      <div class="composer">
        {#if permission}
          <div class="permission" role="alertdialog" aria-label={t("The agent is asking permission")}>
            <div class="p-title">
              {permission.tool === "shell"
                ? t("The agent wants to run a command")
                : t("The agent wants to read a file outside the project")}
            </div>
            <code class="p-code">{permission.summary}</code>
            {#if permission.tool === "shell"}
              <div class="p-note">
                {permission.sandboxed
                  ? t("It runs isolated: it can only write inside the project.")
                  : t("This system cannot isolate commands: it could write outside the project.")}
              </div>
            {/if}
            {#if permission.warning === "sensitive"}
              <div class="p-warn">{t("This looks like a private file (keys or credentials). Its content would be sent to the model provider.")}</div>
            {/if}
            <div class="p-actions">
              <button class="btn btn-primary btn-sm" onclick={() => answerPermission("once")}>{t("Allow once")}</button>
              {#if permission.can_remember}
                <button class="btn btn-sm" onclick={() => answerPermission("always")}>
                  {permission.tool === "shell" ? t("Allow commands in this conversation") : t("Allow this folder in this conversation")}
                </button>
              {/if}
              <button class="btn btn-ghost btn-sm" onclick={() => answerPermission("deny")}>{t("Deny")}</button>
            </div>
          </div>
        {/if}
        <div class="composer-box">
          {#if images.length > 0}
            <div class="composer-attachments">
              {#each images as image, i (i)}
                <div class="composer-image-card" title={`${image.name} · ${image.mime}`}>
                  <img src={`data:${image.mime};base64,${image.data}`} alt={image.name} />
                  <button type="button" class="composer-image-del" onclick={() => removeImage(i)} title={t("Remove image")}>×</button>
                </div>
              {/each}
            </div>
          {/if}
          {#if attachments.length > 0}
            <div class="composer-attachments">
              {#each attachments as file (file.path)}
                <span class="attachment-chip" title={file.path}>
                  {file.name}
                  <button type="button" onclick={() => removeAttachment(file.path)}>×</button>
                </span>
              {/each}
            </div>
          {/if}

          {#if queued.length > 0}
            <div class="composer-attachments">
              {#each queued as item, i (i)}
                <span class="attachment-chip queued-chip" title={t("Sent when the current answer finishes")}>
                  <span class="queued-count">{i + 1}</span>
                  {item.text || item.attachments[0]?.name || item.images[0]?.name || t("Message")}
                  <button type="button" onclick={() => dequeue(i)}>×</button>
                </span>
              {/each}
            </div>
          {/if}

          {#if suggest && (suggest.items.length > 0 || suggest.note)}
            <div class="suggest" role="listbox">
              {#each suggest.items as item, i (item.id)}
                <button
                  type="button"
                  class="suggest-row"
                  class:active={i === suggest.active}
                  onmousedown={(e) => e.preventDefault()}
                  onclick={() => acceptSuggestion(item)}
                  onmousemove={() => suggest && (suggest.active = i)}
                >
                  {#if suggest.kind === "slash"}
                    <span class="s-cmd">/{item.id}</span>
                    <span class="s-desc">{item.title}</span>
                    {#if item.hint}<span class="s-hint">{item.hint}</span>{/if}
                  {:else}
                    <Icon name="file" size={13} />
                    <span class="s-name">{item.title}</span>
                    <span class="s-dir">{item.detail}</span>
                  {/if}
                </button>
              {/each}
              {#if suggest.note}<div class="s-note">{suggest.note}</div>{/if}
            </div>
          {/if}

          <textarea
            bind:this={textarea}
            class="composer-textarea"
            rows={1}
            bind:value={input}
            onkeydown={handleKeyDown}
            onpaste={handlePaste}
            oninput={updateSuggest}
            onclick={updateSuggest}
            onkeyup={(e) => {
              if (!["ArrowUp", "ArrowDown", "Enter", "Tab", "Escape"].includes(e.key)) updateSuggest();
            }}
            onblur={() => setTimeout(() => (suggest = null), 120)}
            placeholder={noModels
              ? t("Add a provider to start chatting")
              : streaming
                ? t("Write now: it goes out when this answer ends")
                : t("Type a message. / for actions, @ for files")}
            disabled={compacting || noModels}
          ></textarea>

          <div class="composer-bar">
            <button type="button" class="composer-icon" onclick={handleAttach} title={t("Attach file")}>
              <Icon name="plus" size={16} />
            </button>
            <button
              type="button"
              class="composer-chip"
              onclick={() => {
                modelMenu = !modelMenu;
                agentMenu = false;
                // Names come from a catalog that may finish downloading after startup.
                if (modelMenu) invoke<ModelInfo[]>("list_models").then((d) => (models = d)).catch(() => {});
              }}
              disabled={loadingModels || models.length === 0}
              title={t("Model")}
            >
              {modelLabel(selectedModel)}
              <Icon name="chevron-down" size={10} />
            </button>

            <button
              type="button"
              class="composer-chip"
              onclick={() => {
                agentMenu = !agentMenu;
                modelMenu = false;
              }}
              disabled={agents.length === 0}
              title={t("Agent")}
            >
              {currentAgentName}
              <Icon name="chevron-down" size={10} />
            </button>

            <div class="composer-spacer"></div>

            {#if streaming}
              {#if input.trim() || attachments.length > 0 || images.length > 0}
                <button
                  type="button"
                  class="composer-send queue"
                  onclick={() => handleSend()}
                  title={t("Send when this answer ends")}
                >
                  <Icon name="arrow-up" size={16} />
                </button>
              {/if}
              <button type="button" class="composer-send stop" onclick={handleStop} title={t("Stop")}>
                <Icon name="stop" size={14} />
              </button>
            {:else}
              <button
                type="button"
                class="composer-send"
                onclick={() => handleSend()}
                disabled={(!input.trim() && attachments.length === 0 && images.length === 0) || !selectedModel || noModels || compacting}
                title={t("Send")}
              >
                <Icon name="arrow-up" size={16} />
              </button>
            {/if}
          </div>

          {#if modelMenu}
            <ModelPicker
              {models}
              selected={selectedModel ? { provider: currentProvider, id: selectedModel } : null}
              onselect={(m) => {
                selectedModel = m.id;
                selectedProvider = m.provider;
                modelMenu = false;
                saveLast();
              }}
              onclose={() => (modelMenu = false)}
              onchanged={() => invoke<ModelInfo[]>("list_models").then((list) => (models = list)).catch(() => {})}
            />
          {/if}

          {#if agentMenu}
            <div class="menu-backdrop" role="presentation" onclick={() => (agentMenu = false)}></div>
            <div class="composer-menu">
              {#each agents as a (a.id)}
                <button
                  type="button"
                  class={`menu-item${a.id === agentId ? " active" : ""}`}
                  onclick={() => {
                    handleSelectAgent(a.id);
                    agentMenu = false;
                  }}
                >
                  <span>{a.name}</span>
                  <span class="menu-sub">{a.mode === "build" ? t("full access") : t("read-only")}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    </div>

    {#if sideOpen && tabs.length > 0}
      <div class="side-resizer" role="presentation" onmousedown={startChatResize} title={t("Drag to resize")}></div>
      <div class="workbench open" style={`flex:0 0 auto;width:calc(100% - ${chatWidth}px - 8px)`}>
        <div class="workbench-main">
          <div class="workbench-head">
            {#each tabs as tab (tab.id)}
              <div
                class={`workbench-tab${activeTab === tab.id ? " active" : ""}`}
                role="button"
                tabindex="0"
                onclick={() => (activeTab = tab.id)}
                onkeydown={(e) => {
                  if (e.key === "Enter") activeTab = tab.id;
                }}
              >
                <span>{tab.title}</span>
                <button
                  class="workbench-tab-close"
                  title={t("Close")}
                  onclick={(e) => {
                    e.stopPropagation();
                    closeTab(tab.id);
                  }}
                >
                  ×
                </button>
              </div>
            {/each}

            <button
              class={`workbench-action${activeKind === "launcher" ? " active" : ""}`}
              onclick={openLauncher}
              title={t("New tab")}
            >
              <Icon name="plus" size={14} />
            </button>

            <div class="workbench-spacer"></div>

            {#if activeTabData?.kind === "file" && activeTabData.path}
              <OpenInMenu path={activeTabData.path} />
            {/if}

            <button class="workbench-action" onclick={() => (sideOpen = false)} title={t("Close panel")}>
              <Icon name="panel" size={14} />
            </button>
          </div>

          <div class="workbench-body">
            {#if showFileSidebar && activeProject}
              <aside class="file-inner-sidebar">
                <FileExplorer
                  root={activeProject.path}
                  onOpenFile={openFileTab}
                  onClose={() => (treeOpen = false)}
                />
              </aside>
            {/if}

            <div class="workbench-view-area">
              {#if activeKind === "launcher"}
                <div class="newtab-view">
                  <div class="launcher-column">
                    {#each launcherItems as item (item.id)}
                      <button class="launcher-row" disabled={item.disabled} onclick={item.run}>
                        <span class="launcher-main">
                          <Icon name={item.icon} size={15} />
                          <span class="launcher-title">{item.title}</span>
                        </span>
                        {#if item.keys.length > 0}<Kbd keys={item.keys} />{/if}
                      </button>
                    {/each}

                    {#if closedTabs.length > 0}
                      <section class="launcher-recent">
                        <h2>{t("Recently closed")}</h2>
                        {#each visibleClosed as tab, i (`${tab.kind}-${tab.title}`)}
                          <button class="launcher-row recent" onclick={() => reopenTab(tab)}>
                            <span class="launcher-main">
                              <Icon name={iconForTab(tab)} size={15} />
                              <span class="launcher-title">{tab.title}</span>
                              {#if tab.path}<span class="launcher-desc">{dirOf(tab.path)}</span>{/if}
                            </span>
                            {#if i === 0}<Kbd keys={["Ctrl", t("Shift"), "T"]} />{/if}
                          </button>
                        {/each}
                        {#if !expandedClosed && closedTabs.length > 5}
                          <button class="launcher-more" onclick={() => (expandedClosed = true)}>
                            {t("Show more")}
                            <Icon name="chevron-down" size={12} />
                          </button>
                        {/if}
                      </section>
                    {/if}
                  </div>
                </div>
              {:else}
                {#each tabs as tab (tab.id)}
                  <div
                    class="workbench-view"
                    style={`display:${activeTab === tab.id ? "flex" : "none"}`}
                  >
                    {#if tab.kind === "files"}
                      <div class="files-view-empty">
                        <Icon name="file" size={22} />
                        <strong style="color:var(--text-primary);font-weight:500">{t("Open file")}</strong>
                        <span>{t("Select a file to open")}</span>
                      </div>
                    {:else if tab.kind === "file" && tab.path}
                      <FileViewer path={tab.path} />
                    {:else if tab.kind === "context"}
                      <ContextPanel
                        title={sessionTitle}
                        projectName={activeProject?.name ?? null}
                        projectPath={activeProject?.path ?? null}
                        model={selectedModel}
                        agent={currentAgentName}
                        messages={messages.length}
                        userMessages={messages.filter((m) => m.role === "user").length}
                        assistantMessages={messages.filter((m) => m.role === "assistant").length}
                        systemPrompt={systemPrompt}
                        {usage}
                      />
                    {:else if tab.kind === "review"}
                      <ReviewPanel project={activeProject?.path ?? null} />
                    {:else if tab.kind === "browser"}
                      <BrowserPanel
                        initialUrl={tab.url}
                        onUrlChange={(url) => {
                          tabs = tabs.map((t) => (t.id === tab.id ? { ...t, url } : t));
                        }}
                      />
                    {:else}
                      <TerminalPanel
                        cwd={activeProject?.path ?? null}
                        visible={activeTab === tab.id}
                        onClose={() => closeTab(tab.id)}
                        onNewTab={openTerminal}
                      />
                    {/if}
                  </div>
                {/each}
              {/if}
            </div>
          </div>
        </div>
      </div>
    {/if}
  </div>

  {#if projectModal}
    <div class="modal-overlay" role="presentation" onclick={() => (projectModal = false)}>
      <div
        class="card"
        role="dialog"
        aria-modal="true"
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.stopPropagation()}
        style="width:100%;max-width:540px"
      >
        <div class="panel-head">
          <span class="panel-title">{t("Open project")}</span>
          <div class="panel-actions">
            <button class="icon-btn" onclick={() => (projectModal = false)}>✕</button>
          </div>
        </div>
        <form
          onsubmit={handleCreateProject}
          style="padding:20px;display:flex;flex-direction:column;gap:14px"
        >
          <label class="form-field">
            <span>{t("Name")}</span>
            <input class="input" bind:value={projectName} placeholder="ianox" />
          </label>
          <label class="form-field">
            <span>{t("Folder path *")}</span>
            <div style="display:flex;gap:8px">
              <input class="input" bind:value={projectPath} placeholder="/path/to/project" />
              <button type="button" class="btn btn-secondary" onclick={chooseFolder}>{t("Choose…")}</button>
            </div>
          </label>
          {#if projectError}<div style="color:var(--danger);font-size:12px">⚠ {projectError}</div>{/if}
          <div style="display:flex;justify-content:flex-end;gap:8px">
            <button type="button" class="btn btn-secondary" onclick={() => (projectModal = false)}>{t("Cancel")}</button>
            <button type="submit" class="btn btn-primary" disabled={!projectPath.trim()}>{t("Open")}</button>
          </div>
        </form>
      </div>
    </div>
  {/if}
</div>
