<script lang="ts">
  import { t } from "../i18n";
  import { app } from "../app.svelte";
  import Markdown from "./Markdown.svelte";
  import type { Message, MessagePart, ToolRun } from "../types";

  let {
    message,
    streaming,
    clean = (text: string) => text,
  }: { message: Message; streaming: boolean; clean?: (text: string) => string } = $props();

  /** One thing the model did inside a group: a thought or a tool call. */
  type Step = { kind: "reasoning"; text: string } | { kind: "tool"; tool: ToolRun };
  type Block = { kind: "text"; text: string } | { kind: "group"; steps: Step[] };

  /** Messages saved before parts existed are rebuilt in the same order: thinking, tools, answer. */
  function legacyParts(m: Message): MessagePart[] {
    const out: MessagePart[] = [];
    if (m.reasoning) out.push({ type: "reasoning", text: m.reasoning });
    for (const tool of m.tools ?? []) {
      out.push({ type: "tool-call", id: tool.id, name: tool.name, arguments: tool.arguments });
    }
    if (m.content) out.push({ type: "text", text: m.content });
    return out;
  }

  /**
   * Like OpenCode: text stays as it is, and every run of thoughts and tool calls
   * between two texts folds into a single group.
   */
  const blocks = $derived.by(() => {
    const byId = new Map((message.tools ?? []).map((tool) => [tool.id, tool]));
    const source = message.parts?.length ? message.parts : legacyParts(message);
    const out: Block[] = [];
    const group = (): Step[] => {
      const tail = out[out.length - 1];
      if (tail?.kind === "group") return tail.steps;
      const steps: Step[] = [];
      out.push({ kind: "group", steps });
      return steps;
    };
    for (const part of source) {
      if (part.type === "text") out.push({ kind: "text", text: clean(part.text) });
      else if (part.type === "reasoning") group().push({ kind: "reasoning", text: part.text });
      else if (part.type === "tool-call") {
        group().push({ kind: "tool", tool: byId.get(part.id) ?? { ...part, startedAt: 0 } });
      }
    }
    return out;
  });

  // ── Labels ──
  const TOOL_TITLES: Record<string, () => string> = {
    read: () => t("Read"),
    glob: () => t("Glob"),
    grep: () => t("Grep"),
    write: () => t("Write"),
    edit: () => t("Edit"),
    shell: () => t("Shell"),
  };
  const toolTitle = (name: string) => TOOL_TITLES[name]?.() ?? name;

  function list(items: string[]): string {
    try {
      return new Intl.ListFormat(app.language, { style: "long", type: "conjunction" }).format(items);
    } catch {
      return items.join(", ");
    }
  }

  const isPending = (tool: ToolRun) => streaming && tool.endedAt === undefined;

  function label(steps: Step[], live: boolean) {
    const tools = steps.flatMap((s) => (s.kind === "tool" ? [s.tool] : []));
    const thoughts = steps.length - tools.length;
    if (live && tools.length === 0) return { thinking: true, before: t("Thinking…"), count: "", after: "" };

    const names = [...new Set(tools.map((tool) => toolTitle(tool.name)))];
    if (names.length === 0 && thoughts === 0) return { thinking: false, before: t("Updates"), count: "", after: "" };

    const count = tools.length || thoughts;
    const title = names.length ? list(names) : thoughts === 1 ? t("Thought") : t("Thoughts");
    const text =
      count === 1
        ? t("Used {count} {tools}", { count: String(count), tools: title })
        : t("Used {count} {tools} [plural]", { count: String(count), tools: title });
    const at = text.indexOf(String(count));
    if (at === -1) return { thinking: false, before: text, count: "", after: "" };
    return {
      thinking: false,
      before: text.slice(0, at),
      count: String(count),
      after: text.slice(at + String(count).length),
    };
  }

  // Elapsed time while only thinking.
  let now = $state(Date.now());
  const thinkingNow = $derived(
    streaming && blocks.length > 0 && blocks[blocks.length - 1].kind === "group" &&
      (blocks[blocks.length - 1] as { steps: Step[] }).steps.every((s) => s.kind === "reasoning")
  );
  $effect(() => {
    if (!thinkingNow) return;
    now = Date.now();
    const timer = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(timer);
  });
  const elapsed = $derived.by(() => {
    if (!thinkingNow || !message.reasoningStartedAt) return "";
    const total = Math.max(0, Math.floor((now - message.reasoningStartedAt) / 1000));
    return total < 60 ? `${total}s` : `${Math.floor(total / 60)}m ${total % 60}s`;
  });

  // ── Tool rows ──
  const seconds = (tool: ToolRun) =>
    tool.endedAt !== undefined && tool.endedAt >= tool.startedAt
      ? Math.max(0.1, (tool.endedAt - tool.startedAt) / 1000)
      : null;
  const fmt = (s: number) => (s < 10 ? `${s.toFixed(1)}s` : `${Math.round(s)}s`);

  function args(tool: ToolRun): Record<string, unknown> {
    try {
      return JSON.parse(tool.arguments) as Record<string, unknown>;
    } catch {
      return {};
    }
  }

  function target(tool: ToolRun): string {
    const a = args(tool);
    const value = a.path ?? a.command ?? a.query ?? a.pattern;
    return typeof value === "string" ? value : "";
  }

  const lines = (text: unknown) => (typeof text === "string" ? text.split("\n") : []);

  /** Rows shown at once; the rest is behind "show more". */
  const VISIBLE_ROWS = 14;
  let expandedDiff = $state<Record<string, boolean>>({});

  /** Diff rows for a call: the backend's real one, or a plain fallback. */
  function diffRows(tool: ToolRun): { kind: string; text: string; old: number | null; new: number | null }[] {
    if (tool.diff) return tool.diff.lines;
    // Older sessions and tools without a backend diff: show the arguments as before.
    const a = args(tool);
    if (tool.name === "edit" && typeof a.old === "string") {
      return [
        ...lines(a.old).map((text) => ({ kind: "del", text, old: null, new: null })),
        ...lines(a.new).map((text) => ({ kind: "add", text, old: null, new: null })),
      ];
    }
    if (tool.name === "write" && typeof a.content === "string") {
      return lines(a.content).map((text) => ({ kind: "add", text, old: null, new: null }));
    }
    return [];
  }


</script>

<div class="turn">
  {#each blocks as block, i (i)}
    {#if block.kind === "text"}
      <Markdown source={block.text} />
    {:else}
      {@const last = i === blocks.length - 1}
      {@const pending = streaming && last && block.steps.some((s) => s.kind === "tool" && isPending(s.tool))}
      {@const head = label(block.steps, streaming && last)}
      <details class="group">
        <summary>
          {#if pending}<span class="spin" aria-hidden="true"></span>{/if}
          <span class="title" class:shimmer={head.thinking}>
            {head.before}{#if head.count}<span class="count">{head.count}</span>{/if}{head.after}
          </span>
          {#if head.thinking && elapsed}<span class="elapsed">{elapsed}</span>{/if}
        </summary>
        <div class="steps">
          {#each block.steps as step, s (s)}
            {#if step.kind === "reasoning"}
              <div class="thought">{step.text}</div>
            {:else}
              {@const tool = step.tool}
              {@const rows = diffRows(tool)}
              {@const open = expandedDiff[tool.id] ?? false}
              {@const shown = open ? rows : rows.slice(0, VISIBLE_ROWS)}
              {#if rows.length > 0}
                <!-- An edit or a write: the change is shown as soon as the group is open. -->
                <div class="row" class:err={tool.is_error}>
                  <div class="head">
                    <span class="name">{toolTitle(tool.name)}</span>
                    <span class="arg" title={tool.arguments}>{target(tool)}</span>
                    {#if tool.diff}
                      <span class="stat"><span class="plus">+{tool.diff.added}</span> <span class="minus">−{tool.diff.removed}</span></span>
                    {/if}
                    {#if isPending(tool)}
                      <span class="time">…</span>
                    {:else if seconds(tool) !== null}
                      <span class="time">{fmt(seconds(tool) ?? 0)}</span>
                    {/if}
                  </div>

                  <div class="diff">
                    {#each shown as line, index (index)}
                      <div class="line {line.kind}">
                        <span class="num">{line.old ?? ""}</span>
                        <span class="num">{line.new ?? ""}</span>
                        <span class="sign">{line.kind === "add" ? "+" : line.kind === "del" ? "−" : " "}</span>
                        <span class="code">{line.text}</span>
                      </div>
                    {/each}
                  </div>

                  {#if rows.length > VISIBLE_ROWS}
                    <button
                      type="button"
                      class="more"
                      onclick={() => (expandedDiff = { ...expandedDiff, [tool.id]: !open })}
                    >
                      {open
                        ? t("Show less")
                        : t("Show {count} more lines", { count: String(rows.length - VISIBLE_ROWS) })}
                    </button>
                  {/if}
                  {#if tool.diff?.truncated}
                    <div class="note">{t("File too large: the diff was shortened.")}</div>
                  {/if}
                  {#if tool.is_error && tool.output}
                    <pre class="out">{tool.output}</pre>
                  {/if}
                </div>
              {:else}
                <details class="row" class:err={tool.is_error}>
                  <summary>
                    <span class="name">{toolTitle(tool.name)}</span>
                    <span class="arg" title={tool.arguments}>{target(tool)}</span>
                    {#if isPending(tool)}
                      <span class="time">…</span>
                    {:else if seconds(tool) !== null}
                      <span class="time">{fmt(seconds(tool) ?? 0)}</span>
                    {/if}
                  </summary>
                  {#if tool.output}
                    <pre class="out">{tool.output}</pre>
                  {/if}
                </details>
              {/if}
            {/if}
          {/each}
        </div>
      </details>
    {/if}
  {/each}
  {#if streaming && blocks.length === 0}
    <div class="group"><span class="title shimmer">{t("Thinking…")}</span></div>
  {:else if streaming && blocks[blocks.length - 1]?.kind === "text"}
    <span class="chat-caret"></span>
  {/if}
</div>

<style>
  .turn {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .group {
    font-size: 13px;
    color: var(--text-secondary);
  }
  summary {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    list-style: none;
    padding: 2px 0;
  }
  summary::-webkit-details-marker {
    display: none;
  }
  summary:hover .title,
  summary:hover .name {
    color: var(--text-primary);
  }
  .count {
    color: var(--text-primary);
    font-variant-numeric: tabular-nums;
  }
  .elapsed,
  .time {
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
    font-size: 12px;
  }
  .shimmer {
    background: linear-gradient(90deg, var(--text-muted) 0%, var(--text-primary) 45%, var(--text-muted) 90%);
    background-size: 220% 100%;
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    animation: shimmer 1.6s linear infinite;
  }
  @keyframes shimmer {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: -100% 0;
    }
  }
  .spin {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    border: 1.5px solid var(--text-muted);
    border-top-color: transparent;
    animation: rot 0.8s linear infinite;
  }
  @keyframes rot {
    to {
      transform: rotate(360deg);
    }
  }
  .steps {
    margin: 6px 0 2px 5px;
    padding-left: 14px;
    border-left: 1px solid var(--border-primary);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .thought {
    color: var(--text-muted);
    font-size: 13px;
    line-height: 1.6;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .row {
    font-size: 13px;
    color: var(--text-secondary);
  }
  .row.err .name {
    color: var(--danger);
  }
  .name {
    color: var(--text-primary);
  }
  .arg {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-code);
    font-size: 12px;
    color: var(--text-muted);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px 0;
  }
  .out {
    margin: 4px 0 6px;
    padding: 8px 10px;
    max-height: 260px;
    overflow: auto;
    border-radius: 6px;
    background: var(--bg-tertiary);
    font-family: var(--font-code);
    font-size: 12px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .more {
    color: var(--text-muted);
  }

  /* ── Diff real: números de línea, verde y rojo ── */
  .stat {
    font-family: var(--font-code);
    font-size: 11px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }
  .stat .plus {
    color: #5fbf7a;
  }
  .stat .minus {
    color: #e06c75;
  }
  .diff {
    margin: 4px 0 6px;
    padding: 6px 0;
    max-height: 320px;
    overflow: auto;
    border-radius: 6px;
    background: var(--bg-tertiary);
    font-family: var(--font-code);
    font-size: 12px;
    line-height: 1.5;
  }
  .line {
    display: flex;
    gap: 8px;
    align-items: baseline;
    padding: 0 8px;
  }
  .line .num {
    flex-shrink: 0;
    width: 2.6em;
    text-align: right;
    color: var(--text-muted);
    opacity: 0.7;
    font-variant-numeric: tabular-nums;
    user-select: none;
  }
  .line .sign {
    flex-shrink: 0;
    width: 1em;
    user-select: none;
  }
  .line .code {
    white-space: pre;
  }
  .line .num,
  .line .code {
    flex-shrink: 0;
  }
  .line.add {
    background: rgba(95, 191, 122, 0.12);
  }
  .line.add .sign,
  .line.add .code {
    color: #5fbf7a;
  }
  .line.del {
    background: rgba(224, 108, 117, 0.12);
  }
  .line.del .sign,
  .line.del .code {
    color: #e06c75;
  }
  .line.ctx .code {
    color: var(--text-secondary);
  }
  .row button.more {
    display: block;
    margin: 4px 0 6px;
    padding: 2px 6px;
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    background: transparent;
    color: var(--text-secondary);
    font-size: 12px;
    cursor: pointer;
  }
  .row button.more:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .note {
    color: var(--text-muted);
    font-size: 12px;
    margin-bottom: 6px;
  }
</style>
