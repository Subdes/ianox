<script lang="ts">
  import { t } from "../i18n";
  // ianox — MCP servers (stdio)
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { McpServerInfo, McpServerInput, McpTestResult } from "../types";
  import Modal from "../components/Modal.svelte";
  import FormField from "../components/FormField.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";

  const emptyForm = (): McpServerInput => ({
    id: "",
    name: "",
    command: "",
    args: [],
    env: {},
    enabled: true,
  });

  let { compact = false }: { compact?: boolean } = $props();

  let servers = $state<McpServerInfo[]>([]);
  let loading = $state(true);
  let showModal = $state(false);
  let editing = $state<McpServerInfo | null>(null);
  let form = $state<McpServerInput>(emptyForm());
  let argsText = $state("");
  let envText = $state("");
  let testingId = $state<string | null>(null);
  let results = $state<Record<string, McpTestResult>>({});

  async function load(): Promise<void> {
    try {
      servers = await invoke<McpServerInfo[]>("list_mcp_servers");
    } catch (e) {
      console.error("Error cargando servidores MCP:", e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    load();
  });

  function openNew(): void {
    editing = null;
    form = emptyForm();
    argsText = "";
    envText = "";
    showModal = true;
  }

  function openEdit(server: McpServerInfo): void {
    editing = server;
    form = { ...server };
    argsText = server.args.join("\n");
    envText = Object.entries(server.env)
      .map(([k, v]) => `${k}=${v}`)
      .join("\n");
    showModal = true;
  }

  function buildInput(): McpServerInput {
    return {
      ...form,
      id: form.id.trim(),
      name: form.name.trim(),
      command: form.command.trim(),
      args: argsText
        .split("\n")
        .map((s) => s.trim())
        .filter(Boolean),
      env: Object.fromEntries(
        envText
          .split("\n")
          .map((line) => line.trim())
          .filter(Boolean)
          .map((line) => {
            const i = line.indexOf("=");
            return i === -1 ? [line, ""] : [line.slice(0, i).trim(), line.slice(i + 1).trim()];
          })
      ),
      enabled: form.enabled,
    };
  }

  async function handleSave(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    const input = buildInput();
    if (!input.id || !input.command) {
      alert(t("ID and command are required"));
      return;
    }
    try {
      await invoke("save_mcp_server", { server: input });
      showModal = false;
      await load();
    } catch (err) {
      alert(`${t("Error")}: ${err}`);
    }
  }

  async function handleRemove(id: string): Promise<void> {
    if (!confirm(t("Delete MCP server {id}?", { id }))) return;
    try {
      await invoke("remove_mcp_server", { id });
      await load();
    } catch (err) {
      alert(`${t("Error")}: ${err}`);
    }
  }

  async function handleToggle(server: McpServerInfo, enabled: boolean): Promise<void> {
    servers = servers.map((s) => (s.id === server.id ? { ...s, enabled } : s));
    try {
      await invoke("toggle_mcp_server", { id: server.id, enabled });
    } catch (err) {
      alert(`${t("Error")}: ${err}`);
      await load();
    }
  }

  async function handleTest(server: McpServerInfo): Promise<void> {
    testingId = server.id;
    try {
      const result = await invoke<McpTestResult>("test_mcp_server", { server });
      results = { ...results, [server.id]: result };
    } catch (err) {
      results = {
        ...results,
        [server.id]: {
          ok: false,
          server_name: null,
          server_version: null,
          protocol_version: null,
          tools: [],
          error: String(err),
        },
      };
    } finally {
      testingId = null;
    }
  }

  function applyPreset(preset: "filesystem" | "photocraft"): void {
    if (preset === "filesystem") {
      form = { ...form, id: form.id || "filesystem", name: form.name || "Filesystem", command: "npx" };
      argsText = "-y\n@modelcontextprotocol/server-filesystem\n/path/to/project";
    } else {
      form = { ...form, id: form.id || "photocraft", name: form.name || "PhotoCraft", command: "/usr/bin/photocraft-cli" };
      argsText = "mcp";
    }
  }
</script>

{#if loading}
  <div class="page-loading">{t("Loading MCP servers…")}</div>
{:else}
  <div class={compact ? "ui-page-contents" : "ui-page"}>
    {#if !compact}
      <PageHeader
        title={t("MCP servers")}
        subtitle={t("Stdio connections, the same format as OpenCode. Add servers and test their tools.")}
      >
        <button class="btn btn-primary" onclick={openNew}>
          <Icon name="plus" size={15} />{t("Add server")}</button>
      </PageHeader>
    {/if}

    <div class="panel-card">
      <div class="panel-card-bar">
        <span class="panel-card-title">{t("Servers")}</span>
        <span class="panel-card-count">{servers.length}</span>
        {#if compact}
          <div style="flex:1"></div>
          <button class="btn btn-primary btn-sm" onclick={openNew}>
            <Icon name="plus" size={13} />{t("Add server")}</button>
        {/if}
      </div>
      <div class="panel-card-body">
        {#if servers.length === 0}
          <EmptyState
            icon="mcp"
            title={t("No MCP servers")}
            desc={t("Add the filesystem server, PhotoCraft or any binary that speaks MCP over stdio.")}
          >
            <button class="btn btn-primary btn-sm" onclick={openNew}>{t("Add server")}</button>
          </EmptyState>
        {:else}
          <div class="list">
            {#each servers as server (server.id)}
              {@const result = results[server.id]}
              <div class="list-item">
                <div class="list-row">
                  <div class="row-icon" style={server.enabled ? "color:var(--success)" : "color:var(--text-muted)"}>
                    <Icon name="mcp" size={15} />
                  </div>
                  <div class="row-main">
                    <div class="row-title">
                      <span class="truncate">{server.name}</span>
                      <code style="font-size:10px;color:var(--text-muted);flex-shrink:0">{server.id}</code>
                      {#if server.enabled}
                        <span class="badge badge-success" style="font-size:10px;flex-shrink:0">{t("active")}</span>
                      {:else}
                        <span class="badge badge-neutral" style="font-size:10px;flex-shrink:0">{t("inactive")}</span>
                      {/if}
                      {#if result?.ok}
                        <span class="badge badge-info" style="font-size:10px;flex-shrink:0">
                          {t("{n} tools", { n: String(result.tools.length) })}
                        </span>
                      {/if}
                    </div>
                    <div class="row-sub" style="font-family:monospace">
                      {server.command} {server.args.join(" ")}
                    </div>
                  </div>
                  <div class="row-actions">
                    <button
                      class={`switch${server.enabled ? " on" : ""}`}
                      title={server.enabled ? "Deshabilitar" : "Habilitar"}
                      onclick={() => handleToggle(server, !server.enabled)}
                    >
                      <span></span>
                    </button>
                    <button
                      class="icon-btn"
                      title={t("Test connection")}
                      onclick={() => handleTest(server)}
                      disabled={testingId === server.id}
                    >
                      <Icon name={testingId === server.id ? "refresh" : "play"} size={15} />
                    </button>
                    <button class="icon-btn" title={t("Edit")} onclick={() => openEdit(server)}>
                      <Icon name="edit" size={15} />
                    </button>
                    <button class="icon-btn danger" title={t("Delete")} onclick={() => handleRemove(server.id)}>
                      <Icon name="trash" size={15} />
                    </button>
                  </div>
                </div>

                {#if result}
                  <div class="row-detail">
                    {#if result.ok}
                      <div style="display:flex;flex-direction:column;gap:6px">
                        <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap;font-size:12px">
                          <span class="badge badge-success" style="font-size:10px">{t("Connected")}</span>
                          <span style="color:var(--text-secondary)">
                            {result.server_name ?? t("server")}
                            {result.server_version ? `v${result.server_version}` : ""} · {t("protocol {version}", { version: result.protocol_version ?? "?" })}
                          </span>
                        </div>
                        <div style="display:flex;gap:6px;flex-wrap:wrap">
                          {#if result.tools.length === 0}
                            <span style="font-size:11px;color:var(--text-muted)">{t("No tools")}</span>
                          {/if}
                          {#each result.tools.slice(0, 12) as t (t.name)}
                            <code
                              title={t.description ?? ""}
                              style="font-size:10px;padding:2px 6px;background:var(--bg-primary);border:1px solid var(--border-primary);border-radius:4px"
                            >
                              {t.name}
                            </code>
                          {/each}
                          {#if result.tools.length > 12}
                            <span style="font-size:11px;color:var(--text-muted)">+{result.tools.length - 12} {t("more")}</span>
                          {/if}
                        </div>
                        {#if result.error}<span style="font-size:11px;color:var(--warning)">{result.error}</span>{/if}
                      </div>
                    {:else}
                      <span style="font-size:11.5px;color:var(--danger)">✖ {result.error ?? t("Unknown error")}</span>
                    {/if}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    {#if showModal}
      <Modal
        title={editing ? t("Edit MCP server") : t("Add MCP server")}
        maxWidth={620}
        onclose={() => (showModal = false)}
      >
        <form onsubmit={handleSave} style="display:flex;flex-direction:column;gap:16px">
          {#if !editing}
            <div style="display:flex;gap:8px;flex-wrap:wrap">
              <button type="button" class="btn btn-ghost btn-sm" onclick={() => applyPreset("filesystem")}>
                {t("Preset")}: Filesystem
              </button>
              <button type="button" class="btn btn-ghost btn-sm" onclick={() => applyPreset("photocraft")}>
                {t("Preset")}: PhotoCraft
              </button>
            </div>
          {/if}

          <div style="display:grid;grid-template-columns:1fr 1fr;gap:16px">
            <FormField label="ID" required hint={t("Unique identifier (no spaces)")}>
              <input class="input" bind:value={form.id} disabled={!!editing} placeholder="filesystem" />
            </FormField>
            <FormField label={t("Name")}>
              <input class="input" bind:value={form.name} placeholder="Filesystem" />
            </FormField>
          </div>

          <FormField label="Comando" required hint="Ejecutable que habla MCP por stdin/stdout">
            <input class="input" bind:value={form.command} placeholder="/usr/bin/photocraft-cli" />
          </FormField>

          <FormField label="Argumentos" hint={t("One per line")}>
            <textarea
              class="input"
              rows={4}
              bind:value={argsText}
              placeholder={"mcp\n--bridge\n127.0.0.1:7777"}
              style="font-family:monospace;font-size:12px"
            ></textarea>
          </FormField>

          <FormField label="Variables de entorno" hint={t("KEY=VALUE, one per line")}>
            <textarea
              class="input"
              rows={3}
              bind:value={envText}
              placeholder={"TOKEN=abc123"}
              style="font-family:monospace;font-size:12px"
            ></textarea>
          </FormField>

          <label style="display:flex;align-items:center;gap:8px;cursor:pointer;font-size:13px">
            <input type="checkbox" bind:checked={form.enabled} />
            {t("Enabled")}
          </label>

          <div style="display:flex;justify-content:flex-end;gap:8px">
            <button type="button" class="btn btn-secondary" onclick={() => (showModal = false)}>{t("Cancel")}</button>
            <button type="submit" class="btn btn-primary">{editing ? t("Save changes") : t("Add")}</button>
          </div>
        </form>
      </Modal>
    {/if}
  </div>
{/if}
