<script lang="ts">
  import { t } from "../i18n";
  // ianox — sidebar: chat history, projects and navigation
  import LogoMark from "./LogoMark.svelte";
  import Icon from "./Icon.svelte";
  import {
    app,
    navigate,
    newSession,
    openSession,
    removeSession,
    openProject,
    requestNewProject,
    deleteProject,
    toggleSidebar,
    chooseProjectIcon,
    clearProjectIcon,
  } from "../app.svelte";

  let iconMenu = $state<string | null>(null);
  let iconInput: HTMLInputElement | undefined = $state();
  let iconTarget: string | null = null;

  function pickIcon(projectId: string): void {
    iconMenu = null;
    iconTarget = projectId;
    iconInput?.click();
  }

  async function onIconFile(event: Event): Promise<void> {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (file && iconTarget) await chooseProjectIcon(iconTarget, file);
    iconTarget = null;
  }

  const navItems = [
    { id: "chat", label: "Chat", icon: "chat" },
    { id: "agents", label: t("Agents"), icon: "agents" },
    { id: "gateway", label: "Gateway", icon: "gateway" },
    { id: "models", label: t("Models"), icon: "models" },
    { id: "extensions", label: t("Extensions"), icon: "extensions" },
    { id: "settings", label: "Settings", icon: "settings" },
  ];

  function relativeDate(ts: number): string {
    const date = new Date(ts * 1000);
    const now = new Date();
    const sameDay = date.toDateString() === now.toDateString();
    if (sameDay) return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    return date.toLocaleDateString([], { day: "2-digit", month: "short" });
  }
</script>

<aside class={`panel sidebar${app.sidebarCollapsed ? " collapsed" : ""}`} style={`width:${app.sidebarCollapsed ? 64 : 260}px`}>
  <div class="sidebar-head">
    {#if !app.sidebarCollapsed}
      <button class="sidebar-brand" onclick={() => navigate("chat")}>
        <LogoMark size={26} radius={7} />
        <span>ianox</span>
      </button>
    {/if}
    <button
      class="btn btn-ghost btn-icon"
      onclick={toggleSidebar}
      aria-label={app.sidebarCollapsed ? t("Expand") : t("Collapse")}
    >
      <Icon name={app.sidebarCollapsed ? "chevron-right" : "chevron-left"} size={18} />
    </button>
  </div>

  <div class="sidebar-new">
    <button
      class={`btn btn-primary ${app.sidebarCollapsed ? "btn-icon" : ""}`}
      onclick={newSession}
      title={t("New chat")}
    >
      <Icon name="plus" size={16} />
      {#if !app.sidebarCollapsed}<span>{t("New chat")}</span>{/if}
    </button>
  </div>

  {#if !app.sidebarCollapsed}
    <div class="sidebar-scroll">
      <div class="sidebar-section-title">{t("Recent")}</div>
      {#if app.sessions.length === 0}
        <div class="sidebar-empty">{t("No conversations yet")}</div>
      {/if}
      {#each app.sessions as session (session.id)}
        <div
          class={`session-row${app.activeSessionId === session.id ? " active" : ""}`}
          role="button"
          tabindex="0"
          onclick={() => openSession(session.id)}
          onkeydown={(e) => e.key === "Enter" && openSession(session.id)}
        >
          <div class="session-text">
            <span class="session-title">{session.title || t("Untitled")}</span>
            <span class="session-meta">
              {relativeDate(session.updated_at)}{session.model ? ` · ${session.model}` : ""}
            </span>
          </div>
          <button
            class="session-delete"
            title={t("Delete session")}
            onclick={(e) => {
              e.stopPropagation();
              removeSession(session.id);
            }}
          >
            <Icon name="x" size={12} />
          </button>
        </div>
      {/each}

      <div
        class="sidebar-section-title"
        style="margin-top:14px;display:flex;align-items:center;justify-content:space-between"
      >
        <span>{t("Projects")}</span>
        <button class="icon-btn" title={t("New project")} onclick={requestNewProject}>
          <Icon name="plus" size={14} />
        </button>
      </div>
      {#if app.projects.length === 0}
        <div class="sidebar-empty">{t("No project open")}</div>
      {/if}
      {#each app.projects as project (project.id)}
        <div
          class={`project-row${app.activeProjectId === project.id ? " active" : ""}`}
          title={project.path}
          role="button"
          tabindex="0"
          onclick={() => openProject(project.id)}
          onkeydown={(e) => e.key === "Enter" && openProject(project.id)}
        >
          {#if project.icon}
            <img class="project-icon" src={project.icon} alt="" />
          {:else}
            <Icon name="folder" size={14} />
          {/if}
          <span>{project.name}</span>
          <button
            class="project-icon-btn"
            title={t("Project icon")}
            onclick={(e) => {
              e.stopPropagation();
              iconMenu = iconMenu === project.id ? null : project.id;
            }}
          >
            <Icon name="image" size={12} />
          </button>
          {#if iconMenu === project.id}
            <div class="menu-backdrop" role="presentation" onclick={(e) => { e.stopPropagation(); iconMenu = null; }}></div>
            <div class="project-icon-menu">
              <button class="menu-item" onclick={(e) => { e.stopPropagation(); pickIcon(project.id); }}>
                <span>{t("Change icon…")}</span>
              </button>
              {#if project.icon}
                <button
                  class="menu-item"
                  onclick={(e) => {
                    e.stopPropagation();
                    iconMenu = null;
                    clearProjectIcon(project.id);
                  }}
                >
                  <span>{t("Remove icon")}</span>
                </button>
              {/if}
            </div>
          {/if}
          <button
            class="project-delete"
            title={t("Remove project")}
            onclick={(e) => {
              e.stopPropagation();
              deleteProject(project.id);
            }}
          >
            ×
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <nav class={`sidebar-nav${app.sidebarCollapsed ? " collapsed" : ""}`}>
    {#each navItems as item (item.id)}
      <button
        class={`nav-item${app.page === item.id ? " active" : ""}`}
        onclick={() => navigate(item.id)}
        title={item.label}
      >
        <span class="nav-icon"><Icon name={item.icon} size={18} /></span>
        {#if !app.sidebarCollapsed}<span class="nav-label">{t(item.label)}</span>{/if}
      </button>
    {/each}
  </nav>

  <div class="sidebar-foot">
    <span
      class="status-dot"
      style={`background:${app.serverStatus.running ? "var(--success)" : "var(--danger)"};box-shadow:${app.serverStatus.running ? "0 0 8px var(--success)" : "none"}`}
      title={app.serverStatus.running ? `${t("External API on")}: ${app.serverStatus.addr}` : t("External API off")}
    ></span>
    {#if !app.sidebarCollapsed}
      <span class="sidebar-foot-text">
        {app.serverStatus.running ? t("External API on") : t("External API off")}
      </span>
    {/if}
  </div>
</aside>

<input bind:this={iconInput} type="file" accept="image/*" hidden onchange={onIconFile} />
