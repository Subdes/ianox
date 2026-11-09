<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    label,
    required = false,
    hint,
    children,
  }: {
    label: string;
    required?: boolean;
    hint?: string;
    children: Snippet;
  } = $props();
</script>

<div style="display:flex;flex-direction:column;gap:6px">
  <span style="font-size:13px;font-weight:500;color:var(--text-secondary);display:flex;align-items:center;gap:4px">
    {label}
    {#if required}<span style="color:var(--danger)">*</span>{/if}
  </span>
  {@render children()}
  {#if hint}<span style="font-size:11px;color:var(--text-muted)">{hint}</span>{/if}
</div>
