<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { OpenApp } from "../open-apps.svelte";

  let { app, size = 16 }: { app: OpenApp; size?: number } = $props();
</script>

<span class="app-icon" style={`width:${size}px;height:${size}px`}>
  {#if app.icon}
    <img src={app.icon} alt="" />
  {:else}
    <Icon name={app.id === "finder" ? "folder" : "external"} size={Math.round(size * 0.8)} />
  {/if}
</span>
