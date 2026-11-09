<script lang="ts">
  import { t } from "../i18n";
  // ianox — Browser tab (URL + iframe)
  import { untrack } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    initialUrl = "",
    onUrlChange,
  }: {
    initialUrl?: string;
    onUrlChange?: (url: string) => void;
  } = $props();

  let address = $state(untrack(() => initialUrl) ?? "");
  let url = $state(untrack(() => initialUrl) ?? "");

  function go(e: SubmitEvent): void {
    e.preventDefault();
    const raw = address.trim();
    if (!raw) return;
    const next = /^https?:\/\//i.test(raw) ? raw : `https://${raw}`;
    url = next;
    address = next;
    onUrlChange?.(next);
  }
</script>

<div class="browser-panel">
  <form class="browser-bar" onsubmit={go}>
    <button type="submit" class="icon-btn" title={t("Go")}>
      <Icon name="arrow-right" size={14} />
    </button>
    <input class="input" bind:value={address} placeholder="https://…" spellcheck="false" />
  </form>
  {#if url}
    <iframe class="browser-frame" title={t("Browser")} src={url}></iframe>
  {:else}
    <div class="file-view-center">{t("Type a URL to browse. Some sites block iframes.")}</div>
  {/if}
</div>
