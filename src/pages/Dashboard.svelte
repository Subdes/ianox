<script lang="ts">
  import { t } from "../i18n";
  // ianox — dashboard page
  import { app, navigate } from "../app.svelte";
  import ServerStatus from "../components/ServerStatus.svelte";
  import Icon from "../components/Icon.svelte";

  const running = $derived(app.serverStatus.running);

  const recentActivity = $derived(
    running
      ? [
          { type: "info", message: t("Server started successfully"), time: t("2 min ago") },
          { type: "success", message: t("3 providers loaded"), time: t("3 min ago") },
          { type: "info", message: t("12 models available"), time: t("3 min ago") },
        ]
      : [
          { type: "warning", message: t("Server stopped"), time: t("10 min ago") },
          { type: "info", message: t("Configuration saved"), time: t("1 hour ago") },
        ]
  );

  const activityColors: Record<string, string> = {
    success: "var(--success)",
    warning: "var(--warning)",
    danger: "var(--danger)",
    info: "var(--accent-primary)",
  };

  let hovered = $state<string | null>(null);

  const quickActions = $derived([
    { id: "providers", title: t("Add provider"), desc: t("Set up OpenAI, Anthropic, Ollama…"), icon: "plus" },
    { id: "routes", title: t("Create route"), desc: t("Define routing patterns"), icon: "gateway" },
    { id: "chat", title: t("Try the chat"), desc: t("A playground to try models"), icon: "chat" },
    {
      id: "dashboard",
      title: t("Open the web dashboard"),
      desc: t("A browser interface for the gateway"),
      icon: "external",
      external: true,
    },
  ]);
</script>

<div style="display:flex;flex-direction:column;gap:20px">
  <ServerStatus config={app.config} serverStatus={app.serverStatus} />

  <div class="card" style="padding:20px">
    <h2 style="font-size:16px;font-weight:600;margin-bottom:16px">{t("Quick actions")}</h2>
    <div style="display:flex;flex-wrap:wrap;gap:12px">
      {#each quickActions as action (action.id)}
        {@const disabled = !!action.external && !app.serverStatus.running}
        <button
          class="card"
          disabled={disabled}
          onclick={() => {
            if (action.external) {
              if (app.serverStatus.addr) window.open(`${app.serverStatus.addr}/dashboard`, "_blank");
            } else {
              navigate(action.id);
            }
          }}
          onmouseenter={() => (hovered = action.id)}
          onmouseleave={() => (hovered = null)}
          style={`flex:1;min-width:180px;max-width:220px;padding:16px;display:flex;flex-direction:column;gap:8px;text-align:left;border:1px solid ${hovered === action.id && !disabled ? "var(--accent-primary)" : "var(--border-primary)"};background:${disabled ? "var(--bg-tertiary)" : "var(--bg-secondary)"};opacity:${disabled ? 0.6 : 1};cursor:${disabled ? "not-allowed" : "pointer"};transition:all var(--transition-fast)`}
        >
          <div style="display:flex;align-items:center;justify-content:space-between">
            <div style="color:var(--accent-primary)"><Icon name={action.icon} size={20} /></div>
          </div>
          <div>
            <div style="font-weight:600;font-size:14px;color:var(--text-primary)">{action.title}</div>
            <div style="font-size:12px;color:var(--text-secondary);margin-top:2px">{action.desc}</div>
          </div>
        </button>
      {/each}
    </div>
  </div>

  <div style="display:grid;grid-template-columns:1fr 1fr;gap:20px">
    <div class="card" style="padding:20px">
      <h3 style="font-size:14px;font-weight:600;margin-bottom:16px">{t("Recent activity")}</h3>
      {#if recentActivity.length === 0}
        <p style="color:var(--text-muted);font-size:13px">{t("No recent activity")}</p>
      {:else}
        <ul style="list-style:none;display:flex;flex-direction:column;gap:8px">
          {#each recentActivity as activity, i (i)}
            <li style="display:flex;align-items:center;gap:10px;padding:8px 0">
              <div
                style={`width:8px;height:8px;border-radius:50%;background:${activityColors[activity.type] || "var(--text-muted)"};box-shadow:0 0 8px ${activityColors[activity.type] || "transparent"}`}
              ></div>
              <div style="flex:1;min-width:0">
                <div style="font-size:13px;color:var(--text-primary)">{activity.message}</div>
                <div style="font-size:11px;color:var(--text-muted)">{activity.time}</div>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <div class="card" style="padding:20px">
      <h3 style="font-size:14px;font-weight:600;margin-bottom:16px">{t("System information")}</h3>
      <dl style="display:grid;grid-template-columns:auto 1fr;gap:8px 16px;font-size:13px">
        <dt style="color:var(--text-secondary)">{t("App version")}</dt>
        <dd style="color:var(--text-primary);font-family:monospace">1.0.0</dd>
        <dt style="color:var(--text-secondary)">{t("Runtime")}</dt>
        <dd style="color:var(--text-primary)">Tauri 2 + Rust + Svelte 5</dd>
        <dt style="color:var(--text-secondary)">{t("Platform")}</dt>
        <dd style="color:var(--text-primary)">{typeof navigator !== "undefined" ? navigator.platform : t("Unknown")}</dd>
        <dt style="color:var(--text-secondary)">{t("Default port")}</dt>
        <dd style="color:var(--text-primary);font-family:monospace">20128</dd>
        <dt style="color:var(--text-secondary)">{t("Configuration")}</dt>
        <dd style="color:var(--text-primary)">
          {app.config
            ? t("{providers} providers, {routes} routes", { providers: String(Object.keys(app.config.providers).length), routes: String(app.config.routes.length) })
            : t("Not configured")}
        </dd>
      </dl>
    </div>
  </div>
</div>
