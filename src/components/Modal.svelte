<script lang="ts">
  import { t } from "../i18n";
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    onclose,
    children,
    maxWidth = 600,
  }: {
    title: string;
    onclose: () => void;
    children: Snippet;
    maxWidth?: number;
  } = $props();
</script>

<div
  class="modal-overlay"
  role="presentation"
  onclick={onclose}
  style="position:fixed;inset:0;background:rgba(0,0,0,0.6);display:flex;align-items:center;justify-content:center;z-index:1000;padding:20px"
>
  <div
    class="card"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
    style={`width:100%;max-width:${maxWidth}px;max-height:90vh;overflow:auto`}
  >
    <div
      style="display:flex;align-items:center;justify-content:space-between;padding:14px 20px;border-bottom:1px solid var(--border-primary)"
    >
      <h3 style="font-size:16px;font-weight:600">{title}</h3>
      <button class="btn btn-ghost btn-icon" onclick={onclose} aria-label={t("Close")}>
        <Icon name="x" size={18} />
      </button>
    </div>
    <div style="padding:20px">{@render children()}</div>
  </div>
</div>
