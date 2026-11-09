<script lang="ts">
  import { t } from "../i18n";
  // ianox — Context tab: real session data and usage
  import Icon from "../components/Icon.svelte";

  interface SessionUsage {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
    requests: number;
    last_ts: number;
    cost_usd: number | null;
    context_tokens?: number | null;
  }

  let {
    title,
    projectName,
    projectPath,
    model,
    agent,
    messages,
    userMessages,
    assistantMessages,
    systemPrompt,
    usage,
  }: {
    title: string;
    projectName: string | null;
    projectPath: string | null;
    model: string;
    agent: string;
    messages: number;
    userMessages: number;
    assistantMessages: number;
    systemPrompt: string;
    usage: SessionUsage | null;
  } = $props();

  const fmt = (n: number) => n.toLocaleString("es-ES");

  const lastActivity = $derived(
    usage && usage.last_ts > 0
      ? new Date(usage.last_ts * 1000).toLocaleString("es-ES", {
          day: "2-digit",
          month: "short",
          hour: "2-digit",
          minute: "2-digit",
        })
      : "—"
  );
</script>

<div class="context-panel">
  <div class="context-inner">
    <h2 class="context-heading">{t("Session")}</h2>
    <div class="context-grid">
      <div class="context-item">
        <span class="context-label">{t("Title")}</span>
        <span class="context-value">{title || t("Untitled")}</span>
      </div>
      <div class="context-item">
        <span class="context-label">{t("Project")}</span>
        <span class="context-value">
          {projectName ?? "—"}
          {#if projectPath}<span class="context-muted">{projectPath}</span>{/if}
        </span>
      </div>
      <div class="context-item">
        <span class="context-label">{t("Model")}</span>
        <span class="context-value mono">{model || "—"}</span>
      </div>
      <div class="context-item">
        <span class="context-label">{t("Agent")}</span>
        <span class="context-value">{agent}</span>
      </div>
    </div>

    <h2 class="context-heading">{t("Messages")}</h2>
    <div class="context-grid">
      <div class="context-item">
        <span class="context-label">{t(t("Total"))}</span>
        <span class="context-value">{fmt(messages)}</span>
      </div>
      <div class="context-item">
        <span class="context-label">{t("User")}</span>
        <span class="context-value">{fmt(userMessages)}</span>
      </div>
      <div class="context-item">
        <span class="context-label">{t("Assistant")}</span>
        <span class="context-value">{fmt(assistantMessages)}</span>
      </div>
      <div class="context-item">
        <span class="context-label">{t("Model requests")}</span>
        <span class="context-value">{usage ? fmt(usage.requests) : "—"}</span>
      </div>
    </div>

    <h2 class="context-heading">{t(t("Tokens"))}</h2>
    <div class="context-grid">
      <div class="context-item">
        <span class="context-label">{t("Input")}</span>
        <span class="context-value">{usage ? fmt(usage.prompt_tokens) : "—"}</span>
      </div>
      <div class="context-item">
        <span class="context-label">{t("Output")}</span>
        <span class="context-value">{usage ? fmt(usage.completion_tokens) : "—"}</span>
      </div>
      <div class="context-item">
        <span class="context-label">{t(t("Total"))}</span>
        <span class="context-value">{usage ? fmt(usage.total_tokens) : "—"}</span>
      </div>
      <div class="context-item">
        <span class="context-label">{t("Cost")}</span>
        <span class="context-value">
          {usage && usage.cost_usd !== null
            ? `${usage.cost_usd.toFixed(usage.cost_usd < 0.01 ? 4 : 2).replace(".", ",")} US$`
            : "—"}
        </span>
      </div>
      <div class="context-item">
        <span class="context-label">{t("Last activity")}</span>
        <span class="context-value">{lastActivity}</span>
      </div>
    </div>

    <p class="context-note">
      <Icon name="info" size={12} />
      {t("Estimated cost from the price table in")} <code>~/.config/ianox/pricing.json</code>
      {t("(editable). If the model is not in the table it shows «—». The context limit is not shown because the provider does not return it.")}
    </p>

    <h2 class="context-heading">{t("System prompt")}</h2>
    <pre class="context-prompt">{systemPrompt || t("No system prompt")}</pre>
  </div>
</div>
