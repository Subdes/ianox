<script lang="ts">
  import { t } from "../i18n";
  // ianox — Extensions: MCP, plugins, skills and LSP servers
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { Config, McpServerInfo } from "../types";
  import { app } from "../app.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";
  import Mcp from "./Mcp.svelte";

  interface SkillInfo {
    id: string;
    name: string;
    description: string;
    path: string;
    scope: string;
  }
  interface PluginInfo {
    id: string;
    name: string;
    version: string;
    description: string;
    path: string;
    scope: string;
  }
  interface LspInfo {
    id: string;
    label: string;
    languages: string;
    path: string;
  }

  let loading = $state(true);
  let projectPath = $state<string | null>(null);
  let mcp = $state<McpServerInfo[]>([]);
  let plugins = $state<PluginInfo[]>([]);
  let skills = $state<SkillInfo[]>([]);
  let lsp = $state<LspInfo[]>([]);

  const tabs = $derived([
    { id: "mcp", label: "MCP", count: mcp.length },
    { id: "plugins", label: t("Plugins"), count: plugins.length },
    { id: "skills", label: t("Skills"), count: skills.length },
    { id: "lsp", label: "LSP", count: lsp.length },
  ] as const);

  async function load(): Promise<void> {
    loading = true;
    try {
      const config = await invoke<Config>("get_config");
      const activeId = config.active_project ?? null;
      projectPath = (config.projects ?? []).find((p) => p.id === activeId)?.path ?? null;

      const [m, p, s, l] = await Promise.all([
        invoke<McpServerInfo[]>("list_mcp_servers").catch(() => []),
        invoke<PluginInfo[]>("list_plugins", { project: projectPath }).catch(() => []),
        invoke<SkillInfo[]>("list_skills", { project: projectPath }).catch(() => []),
        invoke<LspInfo[]>("detect_lsp_servers").catch(() => []),
      ]);
      mcp = m;
      plugins = p;
      skills = s;
      lsp = l;
    } catch (e) {
      console.error("Error cargando extensiones:", e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    load();
  });

  const scopeLabel = (scope: string) => (scope === "proyecto" ? "proyecto" : "usuario");
</script>

<div class="ui-page">
  <PageHeader
    title={t("Extensions")}
    subtitle={t("MCP servers, plugins, skills and language servers available.")}
  >
    <button class="btn btn-secondary btn-sm" onclick={load} disabled={loading}>
      <Icon name="refresh" size={14} />{t("Refresh")}</button>
  </PageHeader>

  <div class="segmented" style="align-self:flex-start">
    {#each tabs as tab (tab.id)}
      <button class={app.extensionsTab === tab.id ? "active" : ""} onclick={() => (app.extensionsTab = tab.id)}>
        {tab.label}
        <span class="tab-count">{tab.count}</span>
      </button>
    {/each}
  </div>

  {#if app.extensionsTab === "mcp"}
    <Mcp compact />
  {:else}
    <div class="panel-card">
    <div class="panel-card-bar">
      <span class="panel-card-title">{tabs.find((t) => t.id === app.extensionsTab)?.label}</span>
      <span style="font-size:11px;color:var(--text-muted)">
        {tabs.find((t) => t.id === app.extensionsTab)?.count}
      </span>
      <div style="flex:1"></div>
    </div>

    <div class="panel-card-body">
      {#if loading}
        <div class="page-loading">{t("Loading…")}</div>
      {:else if app.extensionsTab === "plugins"}
        {#if plugins.length === 0}
          <EmptyState
            icon="extensions"
            title={t("No plugins installed")}
            desc={t("Create ~/.config/ianox/plugins/<id>/plugin.json or <project>/.ianox/plugins/<id>/plugin.json.")}
          />
        {:else}
          <div class="list">
            {#each plugins as plugin (plugin.path)}
              <div class="list-item">
                <div class="list-row">
                  <div class="row-icon"><Icon name="extensions" size={15} /></div>
                  <div class="row-main">
                    <div class="row-title">
                      <span class="truncate">{plugin.name}</span>
                      <span class="badge badge-neutral" style="font-size:10px">v{plugin.version}</span>
                      <span class="badge badge-info" style="font-size:10px">{scopeLabel(plugin.scope)}</span>
                    </div>
                    <div class="row-sub">{plugin.description || plugin.path}</div>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      {:else if app.extensionsTab === "skills"}
        {#if skills.length === 0}
          <EmptyState
            icon="agents"
            title={t("No skills")}
            desc={t("Create ~/.config/ianox/skills/<id>/SKILL.md or <project>/.ianox/skills/<id>/SKILL.md with frontmatter (name, description).")}
          />
        {:else}
          <div class="list">
            {#each skills as skill (skill.path)}
              <div class="list-item">
                <div class="list-row">
                  <div class="row-icon"><Icon name="agents" size={15} /></div>
                  <div class="row-main">
                    <div class="row-title">
                      <span class="truncate">{skill.name}</span>
                      <code style="font-size:10px;color:var(--text-muted)">{skill.id}</code>
                      <span class="badge badge-info" style="font-size:10px">{scopeLabel(skill.scope)}</span>
                    </div>
                    <div class="row-sub">{skill.description || skill.path}</div>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      {:else}
        {#if lsp.length === 0}
          <EmptyState
            icon="code"
            title={t("No LSP servers")}
            desc="Instala, por ejemplo, rust-analyzer, typescript-language-server o pyright."
          />
        {:else}
          <div class="list">
            {#each lsp as server (server.id)}
              <div class="list-item">
                <div class="list-row">
                  <div class="row-icon"><Icon name="code" size={15} /></div>
                  <div class="row-main">
                    <div class="row-title"><span class="truncate">{server.label}</span></div>
                    <div class="row-sub" style="font-family:monospace">{server.path}</div>
                  </div>
                  <div class="row-meta"><span>{server.languages}</span></div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      {/if}
    </div>
  </div>
  {/if}
</div>
