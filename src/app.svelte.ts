// ianox — global application state (Svelte 5 runes)
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { t } from "./i18n";
import type { Config, ProjectInfo, ServerStatus, SessionSummary } from "./types";
import { detectProjectIcon, readFileAsDataUrl, shrinkIcon } from "./project-icons";

export type Page =
  | "chat"
  | "agents"
  | "gateway"
  | "dashboard"
  | "models"
  | "providers"
  | "routes"
  | "settings"
  | "extensions";

export type BootPhase = "connecting" | "error" | "ready";

export const app = $state({
  config: null as Config | null,
  serverStatus: { running: false, addr: null } as ServerStatus,
  page: "chat" as Page,
  sidebarCollapsed: false,
  sessions: [] as SessionSummary[],
  activeSessionId: null as string | null,
  projects: [] as ProjectInfo[],
  activeProjectId: null as string | null,
  projectRequest: 0,
  extensionsTab: "mcp" as "mcp" | "plugins" | "skills" | "lsp",
  sessionTabs: [] as Array<{ id: string }>,
  activeTab: null as string | null,
  theme: (typeof localStorage !== "undefined" && localStorage.getItem("ianox.theme")) || "system",
  language: "en" as "es" | "en",
  phase: "connecting" as BootPhase,
  maximized: false,
  /** Typography panel (temporary tuning tool). */
  tuner: false,
  /** "Quit or keep in the tray?" dialog (Ctrl+Q and the window's close button). */
  quitOpen: false,
  /** Conversation search dialog (the magnifier / Ctrl+K). */
  searchOpen: false,
  /** Conversations with a model answer in progress, by session id. */
  busy: {} as Record<string, boolean>,
});

// ── Persisted UI state: reopen the app exactly as it was left ──
const UI_KEY = "ianox.ui";

interface SavedUi {
  page?: Page;
  sidebarCollapsed?: boolean;
  tabs?: string[];
  activeTab?: string | null;
}

function restoreUi(): void {
  try {
    const saved = JSON.parse(localStorage.getItem(UI_KEY) ?? "null") as SavedUi | null;
    if (!saved) return;
    if (typeof saved.sidebarCollapsed === "boolean") app.sidebarCollapsed = saved.sidebarCollapsed;
    if (saved.page) app.page = saved.page;
    const tabs = (saved.tabs ?? []).filter((id) => typeof id === "string");
    if (tabs.length > 0) {
      app.sessionTabs = tabs.map((id) => ({ id }));
      app.activeTab = tabs.includes(saved.activeTab ?? "") ? (saved.activeTab ?? null) : tabs[0];
      app.activeSessionId = app.activeTab && app.activeTab !== DRAFT_TAB ? app.activeTab : null;
    }
  } catch {
    /* estado guardado ilegible: arranca limpio */
  }
}

/** Drops tabs of conversations that no longer exist (deleted while the app was closed). */
function pruneTabs(): void {
  const known = new Set(app.sessions.map((s) => s.id));
  const kept = app.sessionTabs.filter((t) => t.id === DRAFT_TAB || known.has(t.id));
  if (kept.length === app.sessionTabs.length) return;
  app.sessionTabs = kept;
  if (!kept.some((t) => t.id === app.activeTab)) {
    app.activeTab = kept[0]?.id ?? null;
    app.activeSessionId = app.activeTab && app.activeTab !== DRAFT_TAB ? app.activeTab : null;
  }
}

let uiRestored = false;
$effect.root(() => {
  $effect(() => {
    const snapshot: SavedUi = {
      page: app.page,
      sidebarCollapsed: app.sidebarCollapsed,
      tabs: app.sessionTabs.map((t) => t.id),
      activeTab: app.activeTab,
    };
    if (!uiRestored) return;
    try {
      localStorage.setItem(UI_KEY, JSON.stringify(snapshot));
    } catch {
      /* almacenamiento no disponible */
    }
  });
});

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function refreshSessions(): Promise<void> {
  try {
    app.sessions = await invoke<SessionSummary[]>("list_sessions");
  } catch (e) {
    console.error("Could not load sessions:", e);
  }
}

export async function refreshProjects(): Promise<void> {
  try {
    const config = await invoke<Config>("get_config");
    app.projects = config.projects ?? [];
    app.activeProjectId = config.active_project ?? null;
  } catch (e) {
    console.error("Could not load projects:", e);
  }
  void ensureProjectIcons();
}

const iconChecked = new Set<string>();

/** Gives every project without an icon the one found in its folder (once per session of the app). */
async function ensureProjectIcons(): Promise<void> {
  let changed = false;
  for (const project of app.projects) {
    // `null`/missing: never decided. An empty string means the user removed it on purpose.
    if (project.icon !== undefined && project.icon !== null) continue;
    if (iconChecked.has(project.id)) continue;
    iconChecked.add(project.id);
    try {
      const icon = await detectProjectIcon(project.path);
      if (!icon) continue;
      await invoke("set_project_icon", { id: project.id, icon });
      project.icon = icon;
      changed = true;
    } catch (e) {
      console.error("Could not detect the project icon:", e);
    }
  }
  if (changed) app.projects = [...app.projects];
}

/** Icon of a project, or null (also for the "removed on purpose" empty value). */
export function projectIcon(projectId: string | null | undefined): string | null {
  if (!projectId) return null;
  return app.projects.find((p) => p.id === projectId)?.icon || null;
}

/** Saves the image the user picked as the project's icon. */
export async function chooseProjectIcon(projectId: string, file: File): Promise<void> {
  try {
    const icon = await shrinkIcon(await readFileAsDataUrl(file));
    await invoke("set_project_icon", { id: projectId, icon });
    const project = app.projects.find((p) => p.id === projectId);
    if (project) project.icon = icon;
    app.projects = [...app.projects];
  } catch (e) {
    console.error("Could not set the project icon:", e);
  }
}

/** Removes the icon and stops looking for one automatically. */
export async function clearProjectIcon(projectId: string): Promise<void> {
  try {
    await invoke("set_project_icon", { id: projectId, icon: "" });
    const project = app.projects.find((p) => p.id === projectId);
    if (project) project.icon = "";
    app.projects = [...app.projects];
  } catch (e) {
    console.error("Could not remove the project icon:", e);
  }
}

// Arranque: config -> esperar al servidor local -> mostrar app
export async function boot(): Promise<void> {
  app.phase = "connecting";
  applyTheme(app.theme);

  try {
    app.config = await invoke<Config>("get_config");
    app.language = app.config?.ui?.language === "es" ? "es" : "en";
    if (app.config?.ui?.theme) {
      applyTheme(app.config.ui.theme);
    }
  } catch (e) {
    console.error("Could not load config:", e);
  }

  try {
    app.serverStatus = await invoke<ServerStatus>("server_status");
  } catch {
    /* la API externa es opcional: no bloquea el arranque */
  }

  // The UI does not depend on the HTTP server; we just take a moment
  // a moment so the connecting screen does not flicker.
  await sleep(450);
  restoreUi();
  uiRestored = true;
  if (app.sessionTabs.length === 0) {
    app.sessionTabs = [{ id: DRAFT_TAB }];
    app.activeTab = DRAFT_TAB;
  }
  app.phase = "ready";
  await Promise.all([refreshSessions(), refreshProjects()]);
  pruneTabs();
}

/** Ctrl+Q and the close button: apply the saved choice, or ask. */
export async function requestQuit(): Promise<void> {
  let tray = false;
  try {
    tray = await invoke<boolean>("tray_available");
  } catch {
    /* outside Tauri */
  }
  // Without a tray there is nothing to keep it in: no question to ask.
  if (!tray) return quitApp();
  const action = app.config?.ui?.quit_action ?? "ask";
  if (action === "quit") return quitApp();
  if (action === "tray") return keepInTray();
  app.quitOpen = true;
}

export async function quitApp(): Promise<void> {
  await invoke("quit_app").catch(() => {});
}

export async function keepInTray(): Promise<void> {
  await invoke("hide_to_tray").catch((e) => console.error("Could not hide to the tray:", e));
}

/** Saves "don't ask again" so Settings shows it and can change it. */
export async function rememberQuitAction(action: "quit" | "tray"): Promise<void> {
  if (!app.config?.ui) return;
  const next = { ...app.config, ui: { ...app.config.ui, quit_action: action } };
  try {
    await invoke("set_config", { config: next });
    app.config = next;
  } catch (e) {
    console.error("Could not save the choice:", e);
  }
}

export function navigate(page: string): void {
  app.page = page as Page;
}

export function toggleSidebar(): void {
  app.sidebarCollapsed = !app.sidebarCollapsed;
}

export function newSession(): void {
  if (!app.sessionTabs.some((t) => t.id === DRAFT_TAB)) {
    app.sessionTabs = [{ id: DRAFT_TAB }, ...app.sessionTabs];
  }
  app.activeTab = DRAFT_TAB;
  app.activeSessionId = null;
  app.page = "chat";
}

/** Tab that represents a conversation not created yet. */
export const DRAFT_TAB = "new";

export function openSession(id: string): void {
  if (!app.sessionTabs.some((t) => t.id === id)) app.sessionTabs = [...app.sessionTabs, { id }];
  app.activeTab = id;
  app.activeSessionId = id;
  app.page = "chat";
}

export function activateTab(id: string): void {
  app.activeTab = id;
  app.activeSessionId = id === DRAFT_TAB ? null : id;
  app.page = "chat";
}

/** Turns the draft tab into the newly created session. */
export function adoptSession(id: string): void {
  const index = app.sessionTabs.findIndex((t) => t.id === DRAFT_TAB);
  if (index >= 0) app.sessionTabs[index] = { id };
  else if (!app.sessionTabs.some((t) => t.id === id)) app.sessionTabs = [...app.sessionTabs, { id }];
  app.activeTab = id;
  app.activeSessionId = id;
}

export function closeSessionTab(id: string): void {
  const index = app.sessionTabs.findIndex((t) => t.id === id);
  if (index === -1) return;
  const next = app.sessionTabs.filter((t) => t.id !== id);
  app.sessionTabs = next;

  if (app.activeTab !== id) return;

  const fallback = next[Math.min(index, next.length - 1)];
  if (fallback) {
    app.activeTab = fallback.id;
    app.activeSessionId = fallback.id === DRAFT_TAB ? null : fallback.id;
  } else {
    app.activeTab = null;
    app.activeSessionId = null;
  }
}

export function applyTheme(theme: string): void {
  let effective = theme;
  if (theme === "system") {
    effective = window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  }

  const root = document.documentElement;
  if (root.getAttribute("data-theme") !== effective) {
    root.setAttribute("data-theme", effective);
  }

  if (app.theme === theme) return;
  app.theme = theme;

  try {
    localStorage.setItem("ianox.theme", theme);
  } catch {
    /* almacenamiento no disponible */
  }

  // Notificar a todas las ventanas (main, assistant)
  emit("app://theme-changed", theme).catch(() => {});

  // Persistir en config de fondo si está cargada
  if (app.config && app.config.ui && app.config.ui.theme !== theme) {
    app.config.ui.theme = theme;
    invoke("set_config", { config: $state.snapshot(app.config) }).catch(() => {});
  }
}

export async function removeSession(id: string): Promise<void> {
  if (!confirm(t("Delete this conversation?"))) return;
  try {
    await invoke("delete_session", { id });
    closeSessionTab(id);
    if (app.activeSessionId === id) app.activeSessionId = null;
    await refreshSessions();
  } catch (e) {
    console.error("Could not delete the session:", e);
  }
}

export function requestNewProject(): void {
  app.projectRequest += 1;
  app.page = "chat";
}

export async function deleteProject(id: string): Promise<void> {
  if (!confirm(t("Remove this project from the sidebar? Files are not deleted from disk."))) return;
  try {
    await invoke("remove_project", { id });
    if (app.activeProjectId === id) app.activeProjectId = null;
    await refreshProjects();
  } catch (e) {
    console.error("Could not remove the project:", e);
  }
}

export async function openProject(id: string): Promise<void> {
  try {
    await invoke("set_active_project", { id });
    app.activeProjectId = id;
    await refreshProjects();
    app.activeSessionId = null;
    app.page = "chat";
  } catch (e) {
    console.error("Could not open the project:", e);
  }
}

export async function startServer(): Promise<void> {
  try {
    const addr = await invoke<string>("start_server");
    app.serverStatus = { running: true, addr };
  } catch (e) {
    console.error("Could not start the server:", e);
  }
}

export async function stopServer(): Promise<void> {
  try {
    await invoke("stop_server");
    app.serverStatus = { running: false, addr: null };
  } catch (e) {
    console.error("Could not stop the server:", e);
  }
}
