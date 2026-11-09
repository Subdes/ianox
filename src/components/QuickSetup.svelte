<script lang="ts">
  import { t } from "../i18n";
  // ianox — quick setup for the first provider, inside the chat
  import { invoke } from "@tauri-apps/api/core";
  import type { ProviderConfigInput, ProviderKindInput, ProviderTestResult } from "../types";
  import FormField from "./FormField.svelte";

  const PRESETS: Array<{
    id: string;
    label: string;
    kind: ProviderKindInput;
    base: string;
    needsKey: boolean;
  }> = [
    { id: "openai", label: "OpenAI", kind: "OpenAI", base: "https://api.openai.com/v1", needsKey: true },
    { id: "anthropic", label: "Anthropic", kind: "Anthropic", base: "https://api.anthropic.com/v1", needsKey: true },
    { id: "openrouter", label: "OpenRouter", kind: "OpenRouter", base: "https://openrouter.ai/api/v1", needsKey: true },
    { id: "deepseek", label: "DeepSeek", kind: "Custom", base: "https://api.deepseek.com/v1", needsKey: true },
    { id: "ollama", label: t("Ollama (local)"), kind: "Ollama", base: "http://127.0.0.1:11434/v1", needsKey: false },
    { id: "custom", label: t("Other (OpenAI-compatible)"), kind: "Custom", base: "", needsKey: true },
  ];

  let { onDone, onAdvanced }: { onDone: (model: string) => void; onAdvanced: () => void } = $props();

  let presetId = $state("");
  let baseUrl = $state("");
  let apiKey = $state("");
  let testing = $state(false);
  let saving = $state(false);
  let result = $state<ProviderTestResult | null>(null);
  let chosenModel = $state("");
  let error = $state<string | null>(null);

  const preset = $derived(PRESETS.find((p) => p.id === presetId) ?? null);

  function selectPreset(id: string): void {
    const next = PRESETS.find((p) => p.id === id) ?? PRESETS[0];
    presetId = id;
    baseUrl = next.base;
    result = null;
    chosenModel = "";
    error = null;
  }

  function buildInput(models: string[]): ProviderConfigInput {
    return {
      id: preset?.id ?? "custom",
      name: preset?.label ?? "Proveedor",
      kind: preset?.kind ?? "Custom",
      base_url: baseUrl.trim(),
      api_key: apiKey.trim() ? apiKey.trim() : null,
      models,
      enabled: true,
      priority: 1,
      rate_limit: null,
    };
  }

  async function test(): Promise<void> {
    testing = true;
    error = null;
    result = null;
    try {
      const res = await invoke<ProviderTestResult>("test_provider", { provider: buildInput([]) });
      result = res;
      if (res.ok && res.models.length > 0) chosenModel = res.models[0];
    } catch (e) {
      error = String(e);
    } finally {
      testing = false;
    }
  }

  async function save(): Promise<void> {
    if (!result?.ok || result.models.length === 0) return;
    saving = true;
    error = null;
    const model = chosenModel || result.models[0];
    try {
      const input = buildInput(result.models);
      try {
        await invoke("add_provider", { provider: input });
      } catch (e) {
        const message = String(e);
        if (message.toLowerCase().includes("ya existe")) {
          await invoke("remove_provider", { id: input.id });
          await invoke("add_provider", { provider: input });
        } else {
          throw e;
        }
      }
      onDone(model);
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="quick-setup">
  <div class="quick-setup-head">
    <h2>{t("Set up your first provider")}</h2>
    <p>{t("It's the only thing missing for the chat to reply. Takes under a minute.")}</p>
  </div>

  <div class="quick-presets">
    {#each PRESETS as p (p.id)}
      <button
        type="button"
        class={`quick-preset${presetId === p.id ? " active" : ""}`}
        onclick={() => selectPreset(p.id)}
      >
        {p.label}
      </button>
    {/each}
  </div>

  {#if !preset}
    <p style="font-size:12px;color:var(--text-muted)">
      {t("Pick a provider above. The URL and API key must belong to the same provider.")}
    </p>
  {/if}

  <FormField label="URL base">
    <input class="input" bind:value={baseUrl} placeholder={t("Filled in when you pick a provider")} />
  </FormField>

  {#if preset?.needsKey}
    <FormField label="API key">
      <input class="input" type="password" bind:value={apiKey} placeholder="sk-…" />
    </FormField>
  {/if}

  <div style="display:flex;align-items:center;gap:10px;flex-wrap:wrap">
    <button class="btn btn-primary" onclick={test} disabled={testing || !baseUrl.trim() || !preset}>
      {testing ? "Probando…" : t("Test connection")}
    </button>
    {#if result?.ok}
      <span style="font-size:12px;color:var(--success)">
        {t("Connected in {ms} ms · {n} models", { ms: String(result.latency_ms), n: String(result.models.length) })}
      </span>
    {/if}
    {#if result && !result.ok}
      <span style="font-size:12px;color:var(--danger);display:flex;flex-direction:column;gap:2px">
        <span>⚠ {result.error}</span>
        {#if result.error?.toLowerCase().includes("incorrect api key") && baseUrl.includes("openai.com")}
          <span style="color:var(--warning)">
            {t("That error comes from OpenAI. If your key is from DeepSeek, pick the DeepSeek preset (the URL must be https://api.deepseek.com/v1).")}
          </span>
        {/if}
      </span>
    {/if}
    {#if error}<span style="font-size:12px;color:var(--danger)">⚠ {error}</span>{/if}
  </div>

  {#if baseUrl}
    <p style="font-size:11px;color:var(--text-muted);font-family:monospace">
      {t("Will test:")} {baseUrl.replace(/\/$/, "")}/models
    </p>
  {/if}

  {#if result?.ok && result.models.length > 0}
    <FormField label={t("Default model")}>
      <select class="input" bind:value={chosenModel}>
        {#each result.models as m (m)}<option value={m}>{m}</option>{/each}
      </select>
    </FormField>
    <div style="display:flex;gap:8px;align-items:center">
      <button class="btn btn-primary" onclick={save} disabled={saving}>
        {saving ? t("Saving…") : t("Save and start")}
      </button>
      <button class="btn btn-ghost btn-sm" onclick={onAdvanced}>{t("Advanced settings")}</button>
    </div>
  {/if}

  {#if !result?.ok}
    <button class="btn btn-ghost btn-sm" style="align-self:flex-start" onclick={onAdvanced}>
      {t("I'd rather set it up manually")}
    </button>
  {/if}
</div>
