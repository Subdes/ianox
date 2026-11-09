<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    icon = "info",
    title,
    desc,
    children,
  }: { icon?: string; title: string; desc?: string; children?: Snippet } = $props();
</script>

<div class="empty-state">
  <div class="empty-state-icon"><Icon name={icon} size={22} /></div>
  <div class="empty-state-title">{title}</div>
  {#if desc}<div class="empty-state-desc">{desc}</div>{/if}
  {#if children}
    <div class="empty-state-actions">{@render children()}</div>
  {/if}
</div>
