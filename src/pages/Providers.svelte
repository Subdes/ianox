<script lang="ts">
  import { t } from "../i18n";
  // ianox — providers page
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import type { ProviderInfo, ProviderConfigInput, ProviderKind, ProviderTestResult } from "../types";
  import Modal from "../components/Modal.svelte";
  import FormField from "../components/FormField.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";

  function defaultForm(): ProviderConfigInput {
    return {
      id: "",
      name: "",
      kind: "OpenAI",
      base_url: "https://api.openai.com/v1",
      api_key: null,
      models: [],
      enabled: true,
      priority: 1,
      rate_limit: { requests_per_minute: 60, tokens_per_minute: 150000 },
    };
  }

  const kindColors: Record<string, string> = {
    openai: "var(--accent-primary)",
    anthropic: "var(--warning)",
    openrouter: "var(--accent-secondary)",
    ollama: "var(--success)",
    google: "var(--danger)",
    azure: "var(--accent-primary)",
    custom: "var(--text-muted)",
  };

  let { compact = false }: { compact?: boolean } = $props();

  let providers = $state<ProviderInfo[]>([]);
  let loading = $state(true);
  let showAddModal = $state(false);
  let editingProvider = $state<ProviderInfo | null>(null);
  let testing = $state(false);
  let testResult = $state<ProviderTestResult | null>(null);
  let form = $state<ProviderConfigInput>(defaultForm());

  /**
   * Well-known providers, all of them OpenAI-compatible, so adding one is a
   * single pick and an API key instead of typing the URL and the model list.
   */
  type Preset = { id: string; name: string; kind: ProviderKind; base_url: string; models: string[] };
  const PRESETS: Preset[] = [
    { id: "openai", name: "OpenAI", kind: "OpenAI", base_url: "https://api.openai.com/v1", models: ["gpt-4o", "gpt-4o-mini", "gpt-4.1", "o3"] },
    { id: "anthropic", name: "Anthropic", kind: "Anthropic", base_url: "https://api.anthropic.com/v1", models: ["claude-sonnet-4-20250514", "claude-3-5-haiku-20241022"] },
    { id: "google", name: "Google (Gemini)", kind: "Google", base_url: "https://generativelanguage.googleapis.com/v1beta", models: ["gemini-2.0-flash", "gemini-2.5-pro"] },
    { id: "openrouter", name: "OpenRouter", kind: "OpenRouter", base_url: "https://openrouter.ai/api/v1", models: ["openai/gpt-4o", "anthropic/claude-sonnet-4", "meta-llama/llama-3.3-70b-instruct"] },
    { id: "nvidia", name: "NVIDIA NIM", kind: "OpenAI", base_url: "https://integrate.api.nvidia.com/v1", models: ["meta/llama-3.3-70b-instruct", "nvidia/llama-3.1-nemotron-70b-instruct", "deepseek-ai/deepseek-r1", "qwen/qwen2.5-coder-32b-instruct"] },
    { id: "groq", name: "Groq", kind: "OpenAI", base_url: "https://api.groq.com/openai/v1", models: ["llama-3.3-70b-versatile", "llama-3.1-8b-instant", "qwen-2.5-coder-32b"] },
    { id: "mistral", name: "Mistral", kind: "OpenAI", base_url: "https://api.mistral.ai/v1", models: ["mistral-large-latest", "mistral-small-latest", "codestral-latest"] },
    { id: "deepseek", name: "DeepSeek", kind: "OpenAI", base_url: "https://api.deepseek.com/v1", models: ["deepseek-chat", "deepseek-reasoner"] },
    { id: "xai", name: "xAI (Grok)", kind: "OpenAI", base_url: "https://api.x.ai/v1", models: ["grok-2-latest", "grok-beta"] },
    { id: "together", name: "Together AI", kind: "OpenAI", base_url: "https://api.together.xyz/v1", models: ["meta-llama/Llama-3.3-70B-Instruct-Turbo", "Qwen/Qwen2.5-Coder-32B-Instruct"] },
    { id: "fireworks", name: "Fireworks AI", kind: "OpenAI", base_url: "https://api.fireworks.ai/inference/v1", models: ["accounts/fireworks/models/llama-v3p3-70b-instruct"] },
    { id: "cerebras", name: "Cerebras", kind: "OpenAI", base_url: "https://api.cerebras.ai/v1", models: ["llama-3.3-70b", "llama3.1-8b"] },
    { id: "perplexity", name: "Perplexity", kind: "OpenAI", base_url: "https://api.perplexity.ai", models: ["sonar-pro", "sonar"] },
    { id: "cohere", name: "Cohere", kind: "OpenAI", base_url: "https://api.cohere.ai/compatibility/v1", models: ["command-r-plus", "command-r"] },
    { id: "ollama", name: "Ollama (local)", kind: "Ollama", base_url: "http://localhost:11434/v1", models: ["llama3.1", "qwen2.5-coder", "mistral"] },
    { id: "lmstudio", name: "LM Studio (local)", kind: "OpenAI", base_url: "http://localhost:1234/v1", models: [] },
  ];

  function applyPreset(id: string): void {
    const preset = PRESETS.find((item) => item.id === id);
    if (!preset) {
      form = { ...form, id: "", name: "", base_url: "", models: [] };
      return;
    }
    form = {
      ...form,
      id: preset.id,
      name: preset.name,
      kind: preset.kind,
      base_url: preset.base_url,
      models: [...preset.models],
    };
  }

  onMount(() => {
    let unlisten: UnlistenFn | null = null;

    invoke<ProviderInfo[]>("list_providers")
      .then((data) => (providers = data))
      .catch((e) => console.error("Error cargando proveedores:", e))
      .finally(() => (loading = false));

    listen<ProviderInfo[]>("providers://updated", (e) => (providers = e.payload))
      .then((fn) => (unlisten = fn))
      .catch(() => {});

    return () => unlisten?.();
  });

  async function handleTestProvider(): Promise<void> {
    testing = true;
    testResult = null;
    try {
      const result = await invoke<ProviderTestResult>("test_provider", { provider: form });
      testResult = result;
      if (result.ok && result.models.length > 0) form.models = result.models;
    } catch (e) {
      testResult = { ok: false, latency_ms: 0, models: [], error: String(e) };
    } finally {
      testing = false;
    }
  }

  async function handleSubmit(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    try {
      // An edit is an update in place: the backend keeps the saved key when the field is empty.
      await invoke("add_provider", { provider: form });
      showAddModal = false;
      editingProvider = null;
      resetForm();
      providers = await invoke<ProviderInfo[]>("list_providers");
    } catch (e) {
      console.error("Error guardando proveedor:", e);
      alert(`${t("Error")}: ${e}`);
    }
  }

  function handleEdit(provider: ProviderInfo): void {
    editingProvider = provider;
    form = {
      id: provider.id,
      name: provider.name,
      kind: provider.kind as ProviderKind,
      base_url: provider.base_url,
      api_key: null, // No mostrar la key guardada
      models: provider.models,
      enabled: provider.enabled,
      priority: provider.priority,
      rate_limit: { requests_per_minute: 60, tokens_per_minute: 150000 },
    };
    testResult = null;
    showAddModal = true;
  }
  async function handleDelete(id: string): Promise<void> {
    if (!confirm(t("Delete provider {id}?", { id }))) return;
    try {
      await invoke("remove_provider", { id });
    } catch (e) {
      console.error("Error eliminando proveedor:", e);
      alert(`${t("Error")}: ${e}`);
    }
  }

  function openAdd(): void {
    resetForm();
    editingProvider = null;
    showAddModal = true;
  }

  function resetForm(): void {
    testResult = null;
    form = defaultForm();
  }

  function handleKindChange(kind: ProviderKind): void {
    const defaults: Record<ProviderKind, Partial<ProviderConfigInput>> = {
      OpenAI: { base_url: "https://api.openai.com/v1", models: ["gpt-4o", "gpt-4o-mini", "gpt-4-turbo"] },
      Anthropic: {
        base_url: "https://api.anthropic.com/v1",
        models: ["claude-3-5-sonnet-20241022", "claude-3-5-haiku-20241022"],
      },
      OpenRouter: {
        base_url: "https://openrouter.ai/api/v1",
        models: ["openai/gpt-4o", "anthropic/claude-3.5-sonnet", "meta-llama/llama-3.1-405b"],
      },
      Ollama: { base_url: "http://localhost:11434/v1", models: ["llama3.1", "mistral", "codellama"] },
      Google: {
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        models: ["gemini-1.5-pro", "gemini-1.5-flash"],
      },
      Azure: { base_url: "https://{resource}.openai.azure.com", models: [] },
      Custom: { base_url: "", models: [] },
    };
    form = { ...form, ...defaults[kind], kind };
  }
</script>

{#if loading}
  <div class="page-loading">{t("Loading providers…")}</div>
{:else}
  <div class="ui-page">
    {#if !compact}
      <PageHeader title={t("AI providers")} subtitle={t("OpenAI, Anthropic, Ollama, OpenRouter and any compatible API.")}>
        <button class="btn btn-primary" onclick={openAdd}>
          <Icon name="plus" size={15} />{t("Add provider")}</button>
      </PageHeader>
    {/if}

    <div class="panel-card">
      <div class="panel-card-bar">
        <span class="panel-card-title">{t("Configured")}</span>
        <span style="font-size:11px;color:var(--text-muted)">{providers.length}</span>
        <div style="flex:1"></div>
        {#if compact}
          <button class="btn btn-primary btn-sm" onclick={openAdd}>
            <Icon name="plus" size={13} />
            {t("Add")}
          </button>
        {/if}
      </div>
      <div class="panel-card-body">
        {#if providers.length === 0}
          <EmptyState
            icon="server"
            title={t("No providers configured")}
            desc={t("Add an API and its models to chat and route.")}
          >
            <button class="btn btn-primary btn-sm" onclick={openAdd}>{t("Add provider")}</button>
          </EmptyState>
        {:else}
          <div class="list">
            {#each providers as provider (provider.id)}
              {@const color = kindColors[provider.kind.toLowerCase()] || "var(--text-muted)"}
              <div class="list-item">
                <div class="list-row">
                  <div class="row-icon" style={`color:${color}`}><Icon name="server" size={15} /></div>
                  <div class="row-main">
                    <div class="row-title">
                      <span class="truncate">{provider.name}</span>
                      <span class="badge" style={`background:${color}20;color:${color};font-size:10px;flex-shrink:0`}>
                        {provider.kind}
                      </span>
                      {#if provider.enabled}
                        <span class="badge badge-success" style="font-size:10px;flex-shrink:0">{t("Active")}</span>
                      {:else}
                        <span class="badge badge-neutral" style="font-size:10px;flex-shrink:0">{t("Inactive")}</span>
                      {/if}
                    </div>
                    <div class="row-sub" style="font-family:monospace">{provider.base_url}</div>
                  </div>
                  <div class="row-meta">
                    <span><b>{provider.models.length}</b>{t("models")}</span>
                    <span>{t("prio.")} <b>{provider.priority}</b></span>
                  </div>
                  <div class="row-actions">
                    <button class="icon-btn" title={t("Edit")} onclick={() => handleEdit(provider)}>
                      <Icon name="edit" size={15} />
                    </button>
                    <button class="icon-btn danger" title={t("Delete")} onclick={() => handleDelete(provider.id)}>
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

    {#if showAddModal}
      <Modal
        title={editingProvider ? t("Edit provider") : t("Add provider")}
        maxWidth={600}
        onclose={() => {
          showAddModal = false;
          editingProvider = null;
          resetForm();
        }}
      >
        <form onsubmit={handleSubmit} style="display:flex;flex-direction:column;gap:16px">
          {#if !editingProvider}
            <FormField label={t("Provider")}>
              <select class="input" value={form.id} onchange={(e) => applyPreset(e.currentTarget.value)}>
                <option value="">{t("Choose a provider…")}</option>
                {#each PRESETS as preset (preset.id)}
                  <option value={preset.id}>{preset.name}</option>
                {/each}
              </select>
              <p style="font-size:11px;color:var(--text-muted);margin-top:4px">
                {t("Fills the URL and the models; you can change them afterwards.")}
              </p>
            </FormField>
          {/if}
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:16px">
            <FormField label="ID" required>
              <input class="input" bind:value={form.id} disabled={!!editingProvider} placeholder={t("my-provider")} />
            </FormField>
            <FormField label={t("Name")} required>
              <input class="input" bind:value={form.name} placeholder={t("My Provider")} />
            </FormField>
          </div>

          <FormField label="Tipo" required>
            <select class="input" value={form.kind} onchange={(e) => handleKindChange(e.currentTarget.value as ProviderKind)}>
              <option value="OpenAI">OpenAI</option>
              <option value="Anthropic">Anthropic</option>
              <option value="OpenRouter">OpenRouter</option>
              <option value="Ollama">{t("Ollama (local)")}</option>
              <option value="Google">Google (Gemini)</option>
              <option value="Azure">Azure OpenAI</option>
              <option value="Custom">{t("Custom")}</option>
            </select>
          </FormField>

          <FormField label="URL base" required>
            <input class="input" bind:value={form.base_url} placeholder="https://api.openai.com/v1" />
          </FormField>

          <FormField label="API Key" required={!editingProvider && !form.api_key}>
            <input
              class="input"
              type="password"
              value={form.api_key || ""}
              oninput={(e) => (form.api_key = e.currentTarget.value)}
              placeholder={editingProvider
                ? t("Leave empty to keep unchanged")
                : "sk-..."}
            />
            {#if editingProvider && editingProvider.has_key}
              <p style="font-size:11px;color:var(--success);margin-top:4px">{t("A key is already saved.")}</p>
            {/if}
          </FormField>

          <FormField label={t("Models (one per line)")}>
            <textarea
              class="input"
              rows={4}
              value={form.models.join("\n")}
              oninput={(e) =>
                (form.models = e.currentTarget.value
                  .split("\n")
                  .map((s) => s.trim())
                  .filter(Boolean))}
              placeholder={"gpt-4o\ngpt-4o-mini\ngpt-4-turbo"}
              style="font-family:monospace;font-size:12px"
            ></textarea>
            <p style="font-size:11px;color:var(--text-muted);margin-top:4px">
              {t("List of models available from this provider")}
            </p>
          </FormField>

          <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:16px">
            <FormField label={t("Enabled")}>
              <label style="display:flex;align-items:center;gap:8px;cursor:pointer">
                <input type="checkbox" bind:checked={form.enabled} />
                <span style="font-size:13px">{t("Yes")}</span>
              </label>
            </FormField>
            <FormField label="Prioridad (1-10)">
              <input
                class="input"
                type="number"
                min={1}
                max={10}
                value={form.priority}
                oninput={(e) => (form.priority = parseInt(e.currentTarget.value))}
              />
            </FormField>
            <FormField label="Rate limit (req/min)">
              <input
                class="input"
                type="number"
                value={form.rate_limit?.requests_per_minute || 60}
                oninput={(e) =>
                  (form.rate_limit = {
                    ...(form.rate_limit ?? { tokens_per_minute: 150000 }),
                    requests_per_minute: parseInt(e.currentTarget.value),
                  })}
              />
            </FormField>
          </div>

          <div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap">
            <button
              type="button"
              class="btn btn-secondary btn-sm"
              onclick={handleTestProvider}
              disabled={testing || !form.base_url.trim()}
            >
              {testing ? t("Testing…") : t("Test and load models")}
            </button>
            {#if testResult}
              {#if testResult.ok}
                <span style="font-size:12px;color:var(--success)">
                  {t("Connected in {ms} ms · {n} models", { ms: String(testResult.latency_ms), n: String(testResult.models.length) })}
                </span>
              {:else}
                <span style="font-size:12px;color:var(--danger)">⚠ {testResult.error}</span>
              {/if}
            {/if}
          </div>

          <div style="display:flex;justify-content:flex-end;gap:8px;margin-top:8px">
            <button
              type="button"
              class="btn btn-secondary"
              onclick={() => {
                showAddModal = false;
                editingProvider = null;
                resetForm();
              }}>{t("Cancel")}</button
            >
            <button type="submit" class="btn btn-primary">{editingProvider ? t("Update") : t("Save")}</button>
          </div>
        </form>
      </Modal>
    {/if}
  </div>
{/if}
