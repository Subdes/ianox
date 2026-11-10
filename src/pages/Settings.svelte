<script lang="ts">
  import { t } from "../i18n";
  // ianox — settings page
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import type { Config, ServerConfig, UiConfig } from "../types";
  import FormField from "../components/FormField.svelte";
  import Icon from "../components/Icon.svelte";
  import Kbd from "../components/Kbd.svelte";
  import { SHORTCUTS } from "../shortcuts";
  import { app, applyTheme } from "../app.svelte";

  let config = $state<Config | null>(null);
  let loading = $state(true);
  let saving = $state(false);
  let activeTab = $state<"general" | "server" | "ui" | "advanced">("general");
  let uiConfig = $state<UiConfig>({
    theme: "system",
    language: "es",
    auto_start_server: true,
    minimize_to_tray: true,
    close_to_tray: true,
    quit_action: "ask",
  });
  let serverConfig = $state<ServerConfig>({
    enabled: true,
    host: "127.0.0.1",
    port: 20128,
    cors: true,
    auth_token: null,
  });

  interface UpdateInfo {
    current_version: string;
    latest_version: string;
    has_update: boolean;
    release_notes: string;
    release_url: string;
    asset_name: string | null;
    asset_url: string | null;
    asset_size: number;
  }

  let checkingUpdate = $state(false);
  let updateInfo = $state<UpdateInfo | null>(null);
  let updateError = $state<string | null>(null);
  let downloading = $state(false);
  let downloadPercent = $state(0);
  let downloadStatus = $state<string | null>(null);

  async function handleCheckUpdate(): Promise<void> {
    checkingUpdate = true;
    updateError = null;
    try {
      const res = await invoke<UpdateInfo>("check_for_updates");
      updateInfo = res;
    } catch (err) {
      updateError = String(err);
    } finally {
      checkingUpdate = false;
    }
  }

  async function handleInstallUpdate(): Promise<void> {
    if (!updateInfo?.asset_url || !updateInfo?.asset_name) return;
    downloading = true;
    downloadPercent = 0;
    downloadStatus = t("Downloading update...");

    const unlistenProgress = await listen<{ downloaded: number; total: number; percent: number }>(
      "update://progress",
      (e) => {
        downloadPercent = Math.round(e.payload.percent);
      }
    );

    try {
      const targetPath = await invoke<string>("download_and_install_update", {
        assetUrl: updateInfo.asset_url,
        assetName: updateInfo.asset_name,
      });
      downloadStatus = `${t("Package downloaded and installer started:")} ${targetPath}`;
    } catch (err) {
      downloadStatus = `${t("Error during update:")} ${err}`;
    } finally {
      downloading = false;
      unlistenProgress();
    }
  }

  let unlisten: UnlistenFn | null = null;

  $effect(() => {
    let cancelled = false;
    (async () => {
      try {
        const data = await invoke<Config>("get_config");
        if (cancelled) return;
        config = data;
        if (data.ui) uiConfig = data.ui;
        if (data.server) serverConfig = data.server;
      } catch (e) {
        console.error("Error cargando config:", e);
      } finally {
        if (!cancelled) loading = false;
      }

      const fn = await listen<Config>("config://changed", (e) => {
        config = e.payload;
        if (e.payload.ui) uiConfig = e.payload.ui;
      });
      if (cancelled) fn();
      else unlisten = fn;
    })();

    return () => {
      cancelled = true;
      unlisten?.();
      unlisten = null;
    };
  });

  async function handleSave(): Promise<void> {
    if (!config) return;
    saving = true;
    try {
      const newConfig = { ...config, ui: uiConfig };
      await invoke("set_config", { config: newConfig });
      app.language = uiConfig.language === "en" ? "en" : "es";
    } catch (e) {
      console.error("Error guardando:", e);
      alert(`${t("Error")}: ${e}`);
    } finally {
      saving = false;
    }
  }

  function handleReset(): void {
    if (confirm(t("Reset settings to defaults?"))) {
      // TODO: invoke reset command
    }
  }

  const tabs = [
    { id: "general", label: "General", icon: "settings", desc: "Version, references and status" },
    { id: "server", label: "Server", icon: "server", desc: "External HTTP API" },
    { id: "ui", label: "Interface", icon: "monitor", desc: "Theme and window" },
    { id: "advanced", label: "Advanced", icon: "code", desc: "Data, packaging and development" },
  ] as const;

  const endpoints = [
    { method: "GET", path: "/health", desc: "Health check" },
    { method: "GET", path: "/v1/models", desc: "List models (OpenAI)" },
    { method: "POST", path: "/v1/chat/completions", desc: "Chat (OpenAI)" },
    { method: "POST", path: "/v1/messages", desc: "Messages (Anthropic)" },
    { method: "GET", path: "/api/tags", desc: "List models (Ollama)" },
    { method: "POST", path: "/api/chat", desc: "Chat (Ollama)" },
    { method: "POST", path: "/api/generate", desc: "Generate (Ollama)" },
    { method: "GET", path: "/dashboard", desc: "Web dashboard" },
  ];

  const methodColors: Record<string, string> = { GET: "var(--success)", POST: "var(--accent-primary)" };

  const references = [
    {
      name: "Gateway",
      desc: "AI router with fallback, key management, web dashboard",
      features: ["Fallback chains", "Multiple providers", "Dashboard /dashboard", "OpenAI-compatible proxy"],
    },
    {
      name: "Ollama",
      desc: "Local model manager, OpenAI-compatible API on :11434/v1",
      features: ["Local models", "Pull/Delete", "REST API", "Full privacy"],
    },
    {
      name: "Server",
      desc: "Embedded HTTP server with live status and CORS",
      features: ["Axum HTTP gateway", "Multi-protocol", "SSE streaming", "Zero latency"],
    },
    {
      name: "Agent",
      desc: "Coding agent with terminal, file review, tools and MCP",
      features: ["Plan/Build modes", "MCP servers", "Tools execution", "Workspace isolation"],
    },
  ];

  const packages = [
    { name: ".deb", desc: "Debian/Ubuntu/Mint", features: ["dpkg -i", "apt install ./", "Auto dependencies"] },
    { name: "AppImage", desc: "Portable, any distro", features: ["chmod +x", "No install", "FUSE required"] },
    { name: ".rpm", desc: "Fedora/RHEL", features: ["dnf install ./", "Auto dependencies"] },
    { name: "Flatpak", desc: "Sandbox, Flathub-ready", features: ["flatpak install", "Sandboxing", "Granular permissions"] },
  ];

  const devCommands = [
    { code: "npm run dev:app", desc: "Dev: Vite + Tauri (hot reload)" },
    { code: "npm run build:app", desc: "Production build" },
    { code: "npm run icons", desc: "Generate icons from SVG" },
    { code: "flatpak-builder --user --install --force-clean build-dir packaging/flatpak/io.github.subdes.ianox.yml", desc: "Local Flatpak build" },
  ];

</script>

{#if loading}
  <div class="page-loading">{t("Loading settings…")}</div>
{:else}
  <div class="ui-page">
    <div class="panel-card">
      <div class="settings-layout">
        <nav class="settings-nav">
          {#each tabs as tab (tab.id)}
            <button class:active={activeTab === tab.id} onclick={() => (activeTab = tab.id)} title={t(tab.desc)}>
              <Icon name={tab.icon} size={15} />
              {t(tab.label)}
            </button>
          {/each}
        </nav>

        <div class="settings-body">
          {#if activeTab === "general"}
            <div style="display:flex;flex-direction:column;gap:26px;max-width:760px">
              <section>
                <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:12px">
                  <h3 class="section-title" style="margin:0">{t("About ianox")}</h3>
                  <button
                    class="btn btn-secondary btn-sm"
                    disabled={checkingUpdate || downloading}
                    onclick={handleCheckUpdate}
                  >
                    {#if checkingUpdate}
                      <span class="spin"><Icon name="loader" size={13} /></span>
                      <span>{t("Checking...")}</span>
                    {:else}
                      <Icon name="refresh-cw" size={13} />
                      <span>{t("Check for updates")}</span>
                    {/if}
                  </button>
                </div>

                <dl class="kv">
                  <dt>{t("Version")}</dt><dd class="mono">{updateInfo?.current_version ?? "0.0.5"}</dd>
                  <dt>{t("Runtime")}</dt><dd>Tauri 2 + Rust + Svelte 5</dd>
                  <dt>{t("Framework")}</dt><dd>Vite + TypeScript</dd>
                  <dt>{t("Architecture")}</dt><dd>{t("Cross-platform (Linux/macOS/Windows)")}</dd>
                </dl>

                {#if updateInfo}
                  <div style="margin-top:14px;padding:12px;border-radius:var(--radius-md);background:var(--bg-secondary);border:1px solid var(--border-primary)">
                    {#if updateInfo.has_update}
                      <div style="display:flex;align-items:center;justify-content:space-between;gap:12px;flex-wrap:wrap">
                        <div>
                          <div style="font-weight:600;font-size:13px;color:var(--accent-primary)">
                            ✨ {t("New version available:")} v{updateInfo.latest_version}
                          </div>
                          {#if updateInfo.asset_name}
                            <div style="font-size:11px;color:var(--text-muted);margin-top:2px">
                              {t("Package:")} <code>{updateInfo.asset_name}</code> ({(updateInfo.asset_size / (1024 * 1024)).toFixed(1)} MB)
                            </div>
                          {/if}
                        </div>
                        {#if updateInfo.asset_url}
                          <button
                            class="btn btn-primary btn-sm"
                            disabled={downloading}
                            onclick={handleInstallUpdate}
                          >
                            <Icon name="download" size={13} />
                            <span>{t("Download and Install")}</span>
                          </button>
                        {:else}
                          <a
                            href={updateInfo.release_url}
                            target="_blank"
                            rel="noreferrer"
                            class="btn btn-secondary btn-sm"
                          >
                            <Icon name="external-link" size={13} />
                            <span>{t("Open Release")}</span>
                          </a>
                        {/if}
                      </div>

                      {#if downloading}
                        <div style="margin-top:10px">
                          <div style="height:6px;width:100%;background:var(--bg-tertiary);border-radius:3px;overflow:hidden">
                            <div style="height:100%;background:var(--accent-primary);width:{downloadPercent}%;transition:width 0.2s"></div>
                          </div>
                          <div style="font-size:11px;color:var(--text-muted);margin-top:4px;text-align:right">
                            {downloadPercent}%
                          </div>
                        </div>
                      {/if}

                      {#if downloadStatus}
                        <div style="font-size:11px;color:var(--text-secondary);margin-top:6px">
                          {downloadStatus}
                        </div>
                      {/if}
                    {:else}
                      <div style="font-size:12px;color:var(--success);display:flex;align-items:center;gap:6px">
                        <Icon name="check" size={14} />
                        <span>{t("ianox is up to date")} (v{updateInfo.current_version})</span>
                      </div>
                    {/if}
                  </div>
                {:else if updateError}
                  <div style="margin-top:12px;font-size:12px;color:var(--danger)">
                    ⚠ {t("Update check error:")} {updateError}
                  </div>
                {/if}
              </section>

              <section>
                <h3 class="section-title">{t("Inspiration / References")}</h3>
                <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(210px,1fr));gap:10px">
                  {#each references as ref (ref.name)}
                    <div class="mini-card">
                      <h4>{ref.name}</h4>
                      <p>{t(ref.desc)}</p>
                      <ul>
                        {#each ref.features as f (f)}<li>{t(f)}</li>{/each}
                      </ul>
                    </div>
                  {/each}
                </div>
              </section>

              <section>
                <h3 class="section-title">{t("Configuration status")}</h3>
                <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:10px">
                  <div class="stat-box">
                    <div class="stat-box-value">{config?.providers ? Object.keys(config.providers).length : 0}</div>
                    <div class="stat-box-label">{t("Providers")}</div>
                  </div>
                  <div class="stat-box">
                    <div class="stat-box-value">{config?.routes?.length || 0}</div>
                    <div class="stat-box-label">{t("Routes")}</div>
                  </div>
                  <div class="stat-box">
                    <div class="stat-box-value">
                      {config?.providers ? Object.values(config.providers).reduce((a, p) => a + p.models.length, 0) : 0}
                    </div>
                    <div class="stat-box-label">{t("Total models")}</div>
                  </div>
                  <div class="stat-box">
                    <div class="stat-box-value">{config?.version || 0}</div>
                    <div class="stat-box-label">{t("Config version")}</div>
                  </div>
                </div>
              </section>
            </div>
          {:else if activeTab === "server"}
            <div style="display:flex;flex-direction:column;gap:26px;max-width:620px">
              <section>
                <h3 class="section-title">{t("HTTP server")}</h3>
                <div style="display:flex;flex-direction:column;gap:16px">
                  <FormField label={t("Enabled")}>
                    <label class="check-row">
                      <input type="checkbox" bind:checked={serverConfig.enabled} />
                      <span>{t("Server active on start")}</span>
                    </label>
                  </FormField>

                  <div style="display:grid;grid-template-columns:1fr 1fr;gap:16px">
                    <FormField label="Host" required>
                      <input class="input" bind:value={serverConfig.host} placeholder="127.0.0.1" />
                    </FormField>
                    <FormField label={t("Port")} required>
                      <input
                        class="input"
                        type="number"
                        min={1024}
                        max={65535}
                        value={serverConfig.port}
                        oninput={(e) => (serverConfig.port = parseInt(e.currentTarget.value))}
                      />
                    </FormField>
                  </div>

                  <FormField label="CORS">
                    <label class="check-row">
                      <input type="checkbox" bind:checked={serverConfig.cors} />
                      <span>{t("Allow cross-origin requests (needed for the web dashboard)")}</span>
                    </label>
                  </FormField>

                  <FormField label={t("Authentication token (Bearer)")}>
                    <input
                      class="input"
                      type="password"
                      value={serverConfig.auth_token || ""}
                      oninput={(e) => (serverConfig.auth_token = e.currentTarget.value || null)}
                      placeholder={t("Leave empty to disable")}
                    />
                    <p style="font-size:11px;color:var(--text-muted);margin-top:4px">
                      {t("If set, every request must send the header: Authorization: Bearer <token>")}
                    </p>
                  </FormField>
                </div>
              </section>

              <section>
                <h3 class="section-title">{t("Exposed endpoints")}</h3>
                <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(230px,1fr));gap:8px;font-size:12px">
                  {#each endpoints as ep (ep.path)}
                    <div class="endpoint-row">
                      <span
                        class="endpoint-method"
                        style={`background:${methodColors[ep.method] || "var(--text-muted)"}`}>{ep.method}</span
                      >
                      <code>{ep.path}</code>
                      <span class="endpoint-desc">{t(ep.desc)}</span>
                    </div>
                  {/each}
                </div>
              </section>
            </div>
          {:else if activeTab === "ui"}
            <div style="display:flex;flex-direction:column;gap:26px;max-width:620px">
              <section>
                <h3 class="section-title">{t("Appearance")}</h3>
                <div style="display:flex;flex-direction:column;gap:16px">
                  <FormField label={t("Theme")}>
                    <select
                      class="input"
                      bind:value={uiConfig.theme}
                      onchange={() => applyTheme(uiConfig.theme)}
                    >
                      <option value="dark">{t("ianox Dark")}</option>
                      <option value="light">{t("ianox Light")}</option>
                      <option value="soplos-dark">{t("Soplos Dark")}</option>
                      <option value="soplos-light">{t("Soplos Light")}</option>
                      <option value="system">{t("System")}</option>
                    </select>
                  </FormField>
                  <FormField label="Idioma">
                    <select class="input" bind:value={uiConfig.language}>
                      <option value="es">{t("Spanish")}</option>
                      <option value="en">English</option>
                    </select>
                  </FormField>
                </div>
              </section>

              <section>
                <h3 class="section-title">{t("Window behavior")}</h3>
                <div style="display:flex;flex-direction:column;gap:8px">
                  <label class="toggle-row">
                    <input type="checkbox" bind:checked={uiConfig.auto_start_server} />
                    <div>
                      <div class="toggle-row-title">{t("Start server automatically")}</div>
                      <div class="toggle-row-desc">{t("Starts the HTTP server when the app opens")}</div>
                    </div>
                  </label>
                  <label class="toggle-row">
                    <input type="checkbox" bind:checked={uiConfig.minimize_to_tray} />
                    <div>
                      <div class="toggle-row-title">{t("Minimize to tray")}</div>
                      <div class="toggle-row-desc">{t("When minimized, hides the window and leaves only the tray icon")}</div>
                    </div>
                  </label>
                  <label class="toggle-row" style="align-items:center">
                    <div style="flex:1">
                      <div class="toggle-row-title">{t("When closing ianox")}</div>
                      <div class="toggle-row-desc">{t("Choose what Ctrl+Q and the window's close button do")}</div>
                    </div>
                    <select class="input" style="width:auto" bind:value={uiConfig.quit_action}>
                      <option value="ask">{t("Ask every time")}</option>
                      <option value="tray">{t("Keep in the tray")}</option>
                      <option value="quit">{t("Quit")}</option>
                    </select>
                  </label>
                </div>
              </section>

              <section>
                <h3 class="section-title">{t("Keyboard shortcuts")}</h3>
                <div style="display:flex;flex-direction:column;gap:6px;font-size:12px">
                  {#each SHORTCUTS as sc (sc.id)}
                    <div class="shortcut-row">
                      <span>{t(sc.label)}</span>
                      <Kbd keys={sc.keys} />
                    </div>
                  {/each}
                </div>
              </section>
            </div>
          {:else}
            <div style="display:flex;flex-direction:column;gap:26px;max-width:760px">
              <section>
                <h3 class="section-title">{t("Data and storage")}</h3>
                <div style="display:flex;flex-direction:column;gap:12px">
                  <p style="font-size:12.5px;color:var(--text-secondary)">
                    {t("Settings are stored in the application data directory:")}
                  </p>
                  <code class="code-block">
                    {typeof window !== "undefined"
                      ? "~/.local/share/ianox/config.json (Linux)"
                      : "AppData/ianox/config.json"}
                  </code>
                  <div style="display:flex;gap:8px">
                    <button class="btn btn-secondary btn-sm">{t("Open folder")}</button>
                    <button class="btn btn-ghost btn-sm">{t("Export config")}</button>
                    <button class="btn btn-ghost btn-sm">{t("Import config")}</button>
                  </div>
                </div>
              </section>

              <section>
                <h3 class="section-title">{t("Linux packaging")}</h3>
                <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:10px">
                  {#each packages as pkg (pkg.name)}
                    <div class="mini-card">
                      <div style="display:flex;align-items:baseline;gap:8px">
                        <span class="pkg-name">{pkg.name}</span>
                        <span style="font-size:11.5px;color:var(--text-muted)">{t(pkg.desc)}</span>
                      </div>
                      <ul>
                        {#each pkg.features as f (f)}<li>{t(f)}</li>{/each}
                      </ul>
                    </div>
                  {/each}
                </div>
              </section>

              <section>
                <h3 class="section-title">{t("Development")}</h3>
                <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(250px,1fr));gap:8px">
                  {#each devCommands as cmd (cmd.code)}
                    <div class="code-block" style="font-size:12px;display:flex;flex-direction:column;gap:2px">
                      <code>{cmd.code}</code>
                      <span style="font-size:10px;color:var(--text-muted)">{t(cmd.desc)}</span>
                    </div>
                  {/each}
                </div>
              </section>

              <section style="border-top:1px solid var(--border-primary);padding-top:18px">
                <h3 class="section-title">{t("Danger zone")}</h3>
                <div style="display:flex;gap:12px;flex-wrap:wrap">
                  <button
                    class="btn btn-danger"
                    onclick={() => {
                      if (confirm(t("Delete ALL settings and data?"))) {
                        /* TODO */
                      }
                    }}>{t("Reset everything")}</button
                  >
                  <button class="btn btn-ghost" style="color:var(--danger)">{t("View logs")}</button>
                </div>
              </section>
            </div>
          {/if}
        </div>
      </div>
    </div>

    <div style="display:flex;justify-content:flex-end;gap:12px">
      <button class="btn btn-secondary" onclick={handleReset} disabled={saving}>{t("Reset")}</button>
      <button class="btn btn-primary" onclick={handleSave} disabled={saving}>
        {saving ? t("Saving…") : t("Save changes")}
      </button>
    </div>
  </div>
{/if}
