<script lang="ts">
  import { t } from "../i18n";
  // ianox — tarjeta de estado del servidor
  import type { Config, ServerStatus } from "../types";
  import Icon from "./Icon.svelte";

  let { config, serverStatus }: { config: Config | null; serverStatus: ServerStatus } = $props();

  const providersCount = $derived(config ? Object.keys(config.providers).filter((k) => config.providers[k].enabled).length : 0);
  const modelsCount = $derived(
    config ? Object.values(config.providers).reduce((acc, p) => acc + (p.enabled ? p.models.length : 0), 0) : 0
  );
  const routesCount = $derived(config ? config.routes.filter((r) => r.enabled).length : 0);

  const endpoints = [
    { method: "GET", path: "/health", desc: "Health check" },
    { method: "GET", path: "/v1/models", desc: "Lista modelos (OpenAI)" },
    { method: "POST", path: "/v1/chat/completions", desc: "Chat completions" },
    { method: "POST", path: "/v1/messages", desc: "Messages (Anthropic)" },
    { method: "GET", path: "/api/tags", desc: "Lista modelos (Ollama)" },
    { method: "POST", path: "/api/chat", desc: "Chat (Ollama)" },
    { method: "POST", path: "/api/generate", desc: "Generate (Ollama)" },
    { method: "GET", path: "/dashboard", desc: "Panel web" },
  ];

  const methodColors: Record<string, string> = {
    GET: "var(--success)",
    POST: "var(--accent-primary)",
    PUT: "var(--warning)",
    DELETE: "var(--danger)",
  };
</script>

<div class="card" style="padding:20px">
  <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:16px">
    <div>
      <h2 style="font-size:18px;font-weight:600">{t("Local server status")}</h2>
      <p style="font-size:13px;color:var(--text-secondary);margin-top:4px">
        {t("OpenAI, Anthropic and Ollama compatible API on localhost")}
      </p>
    </div>
    <div style="display:flex;align-items:center;gap:12px">
      <div
        style={`width:12px;height:12px;border-radius:50%;background:${serverStatus.running ? "var(--success)" : "var(--danger)"};box-shadow:${serverStatus.running ? "0 0 12px var(--success)" : "none"}`}
      ></div>
      <span style={`font-weight:500;color:${serverStatus.running ? "var(--success)" : "var(--danger)"}`}>
        {serverStatus.running ? t("Online") : t("Offline")}
      </span>
    </div>
  </div>

  {#if serverStatus.addr}
    <div style="margin-bottom:16px;padding:12px;background:var(--bg-tertiary);border-radius:var(--radius-sm)">
      <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:8px">
        <span style="font-size:12px;color:var(--text-secondary)">{t("Base endpoint")}</span>
        <button
          class="btn btn-ghost btn-sm btn-icon"
          title={t("Copy URL")}
          onclick={() => navigator.clipboard.writeText(serverStatus.addr ?? "")}
        >
          <Icon name="copy" size={16} />
        </button>
      </div>
      <code style="font-size:13px;font-family:monospace;color:var(--text-primary);word-break:break-all">
        {serverStatus.addr}
      </code>
    </div>
  {/if}

  <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:12px">
    <div class="card" style="padding:16px;display:flex;flex-direction:column;gap:8px">
      <div style="display:flex;align-items:center;justify-content:space-between">
        <span style="font-size:12px;color:var(--text-secondary);font-weight:500">{t("Providers")}</span>
        <div style="color:var(--accent-primary);opacity:0.8"><Icon name="server" size={24} /></div>
      </div>
      <div style="font-size:28px;font-weight:700;color:var(--text-primary);line-height:1">{providersCount}</div>
    </div>
    <div class="card" style="padding:16px;display:flex;flex-direction:column;gap:8px">
      <div style="display:flex;align-items:center;justify-content:space-between">
        <span style="font-size:12px;color:var(--text-secondary);font-weight:500">{t("Models")}</span>
        <div style="color:var(--accent-secondary);opacity:0.8"><Icon name="models" size={24} /></div>
      </div>
      <div style="font-size:28px;font-weight:700;color:var(--text-primary);line-height:1">{modelsCount}</div>
    </div>
    <div class="card" style="padding:16px;display:flex;flex-direction:column;gap:8px">
      <div style="display:flex;align-items:center;justify-content:space-between">
        <span style="font-size:12px;color:var(--text-secondary);font-weight:500">{t("Active routes")}</span>
        <div style="color:var(--success);opacity:0.8"><Icon name="gateway" size={24} /></div>
      </div>
      <div style="font-size:28px;font-weight:700;color:var(--text-primary);line-height:1">{routesCount}</div>
    </div>
    <div class="card" style="padding:16px;display:flex;flex-direction:column;gap:8px">
      <div style="display:flex;align-items:center;justify-content:space-between">
        <span style="font-size:12px;color:var(--text-secondary);font-weight:500">{t("Port")}</span>
        <div style="color:var(--warning);opacity:0.8"><Icon name="server" size={24} /></div>
      </div>
      <div style="font-size:28px;font-weight:700;color:var(--text-primary);line-height:1">
        {config?.server.port.toString() || "20128"}
      </div>
    </div>
  </div>

  {#if serverStatus.running}
    <div style="margin-top:20px;padding-top:16px;border-top:1px solid var(--border-primary)">
      <h3 style="font-size:14px;font-weight:600;margin-bottom:12px">{t("Available endpoints")}</h3>
      <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:8px;font-size:12px">
        {#each endpoints as ep (ep.path)}
          <div
            style="display:flex;align-items:center;gap:8px;padding:6px 8px;background:var(--bg-tertiary);border-radius:var(--radius-sm)"
          >
            <span
              style={`background:${methodColors[ep.method] || "var(--text-muted)"};color:white;padding:2px 6px;border-radius:4px;font-size:10px;font-weight:600`}
            >
              {ep.method}
            </span>
            <code style="font-family:monospace;color:var(--text-primary);flex:1">{ep.path}</code>
            <span style="color:var(--text-muted)">{ep.desc}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
