<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    subtitle,
    children,
  }: { title: string; subtitle?: string; children?: Snippet } = $props();
</script>

<header class="page-head">
  <div class="page-head-text">
    <h2>{title}</h2>
    {#if subtitle}<p>{subtitle}</p>{/if}
  </div>
  {#if children}
    <div class="page-head-actions">{@render children()}</div>
  {/if}
</header>
