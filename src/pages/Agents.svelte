<script lang="ts">
  import { t } from "../i18n";
  // ianox — agents (build / plan)
  import { invoke } from "@tauri-apps/api/core";
  import type { AgentConfig, AgentInput, AgentMode, ModelInfo } from "../types";
  import Modal from "../components/Modal.svelte";
  import FormField from "../components/FormField.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";

  const emptyAgent = {
    id: "",
    name: "",
    mode: "build" as AgentMode,
    model: null,
    system_prompt: "",
  };

  let agents = $state<AgentConfig[]>([]);
  let models = $state<ModelInfo[]>([]);
  let loading = $state(true);
  let showModal = $state(false);
  let editing = $state<AgentConfig | null>(null);
  let form = $state<AgentInput>({ ...emptyAgent });
  let error = $state<string | null>(null);

  async function load(): Promise<void> {
    try {
      const [list, modelList] = await Promise.all([
        invoke<AgentConfig[]>("list_agents"),
        invoke<ModelInfo[]>("list_models").catch(() => [] as ModelInfo[]),
      ]);
      agents = list;
      models = modelList;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  load();

  function openNew(): void {
    editing = null;
    form = { ...emptyAgent };
    error = null;
    showModal = true;
  }

  function openEdit(agent: AgentConfig): void {
    editing = agent;
    form = {
      id: agent.id,
      name: agent.name,
      mode: agent.mode,
      model: agent.model,
      system_prompt: agent.system_prompt,
    };
    error = null;
    showModal = true;
  }

  async function handleSave(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    error = null;
    try {
      await invoke("save_agent", { agent: form });
      showModal = false;
      await load();
    } catch (err) {
      error = String(err);
    }
  }

  async function handleRemove(agent: AgentConfig): Promise<void> {
    if (!confirm(t("Delete agent {name}?", { name: agent.name }))) return;
    try {
      await invoke("remove_agent", { id: agent.id });
      await load();
    } catch (err) {
      error = String(err);
    }
  }

  const oneLine = (s: string) => s.replace(/\s+/g, " ").trim();
</script>

{#if loading}
  <div class="page-loading">{t("Loading agents…")}</div>
{:else}
  <div class="ui-page">
    <PageHeader
      title={t("Agents")}
      subtitle={t("Build with full access and plan read-only. Each agent sets its model and system prompt.")}
    >
      <button class="btn btn-primary" onclick={openNew}>
        <Icon name="plus" size={15} />
        {t("New agent")}
      </button>
    </PageHeader>

    {#if error}<div class="error-banner"><span>⚠ {error}</span></div>{/if}

    <div class="panel-card">
      <div class="panel-card-bar">
        <span class="panel-card-title">{t("Agents")}</span>
        <span class="panel-card-count">{agents.length}</span>
      </div>
      <div class="panel-card-body">
        {#if agents.length === 0}
          <EmptyState
            icon="agents"
            title={t("No agents")}
            desc="Crea un agente y fija su modo, modelo y prompt de sistema."
          >
            <button class="btn btn-primary btn-sm" onclick={openNew}>{t("Create agent")}</button>
          </EmptyState>
        {:else}
          <div class="list">
            {#each agents as agent (agent.id)}
              <div class="list-item">
                <div class="list-row">
                  <div class="row-icon"><Icon name="agents" size={15} /></div>
                  <div class="row-main">
                    <div class="row-title">
                      <span class="truncate">{agent.name}</span>
                      <span
                        class={`badge ${agent.mode === "build" ? "badge-success" : "badge-info"}`}
                        style="font-size:10px;flex-shrink:0"
                      >
                        {agent.mode === "build" ? t("full access") : t("read-only")}
                      </span>
                      <code style="font-size:10px;color:var(--text-muted);flex-shrink:0">{agent.id}</code>
                    </div>
                    <div class="row-sub">
                      {agent.system_prompt ? oneLine(agent.system_prompt) : t("No system prompt")}
                    </div>
                  </div>
                  <div class="row-meta">
                    <span>{agent.model || t("chat model")}</span>
                  </div>
                  <div class="row-actions">
                    <button class="icon-btn" title={t("Edit")} onclick={() => openEdit(agent)}>
                      <Icon name="edit" size={15} />
                    </button>
                    <button class="icon-btn danger" title={t("Delete")} onclick={() => handleRemove(agent)}>
                      <Icon name="trash" size={15} />
                    </button>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    {#if showModal}
      <Modal title={editing ? t("Edit agent") : t("New agent")} maxWidth={620} onclose={() => (showModal = false)}>
        <form onsubmit={handleSave} style="display:flex;flex-direction:column;gap:16px">
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:16px">
            <FormField label="ID" required>
              <input class="input" bind:value={form.id} disabled={!!editing} placeholder={t("my-agent")} />
            </FormField>
            <FormField label={t("Name")} required>
              <input class="input" bind:value={form.name} placeholder={t("My agent")} />
            </FormField>
          </div>

          <div style="display:grid;grid-template-columns:1fr 1fr;gap:16px">
            <FormField label="Modo">
              <select class="input" value={form.mode} onchange={(e) => (form.mode = e.currentTarget.value as AgentMode)}>
                <option value="build">{t("build — full access")}</option>
                <option value="plan">{t("plan — read-only")}</option>
              </select>
            </FormField>
            <FormField label={t("Model")}>
              <select class="input" value={form.model ?? ""} onchange={(e) => (form.model = e.currentTarget.value || null)}>
                <option value="">{t("Chat default")}</option>
                {#each models as m (`${m.provider}-${m.id}`)}
                  <option value={m.id}>{m.id} · {m.provider}</option>
                {/each}
              </select>
            </FormField>
          </div>

          <FormField label="Prompt de sistema">
            <textarea
              class="input"
              rows={5}
              bind:value={form.system_prompt}
              placeholder={t("Agent instructions…")}
              style="font-family:inherit;font-size:12px"
            ></textarea>
          </FormField>

          {#if error}<div style="color:var(--danger);font-size:12px">⚠ {error}</div>{/if}

          <div style="display:flex;justify-content:flex-end;gap:8px">
            <button type="button" class="btn btn-secondary" onclick={() => (showModal = false)}>{t("Cancel")}</button>
            <button type="submit" class="btn btn-primary">{editing ? t("Save") : t("Create")}</button>
          </div>
        </form>
      </Modal>
    {/if}
  </div>
{/if}
