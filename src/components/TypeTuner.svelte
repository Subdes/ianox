<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { t } from "../i18n";
  import FontPicker from "./FontPicker.svelte";
  import { applyTuning, DEFAULT_TUNING, loadTuning, type Tuning } from "../type-tuner";

  let { onclose }: { onclose: () => void } = $props();

  let value = $state<Tuning>(loadTuning());
  let fonts = $state<string[]>([]);
  let advanced = $state(false);

  $effect(() => applyTuning($state.snapshot(value) as Tuning));

  onMount(() => {
    invoke<string[]>("list_fonts")
      .then((list) => (fonts = list))
      .catch(() => {
        /* without the list the user can still type a family name */
      });
  });

  const colors = $derived([
    ["", t("Default")],
    ["#ffffff", t("White")],
    ["#d4d6dc", t("Light gray")],
    ["#bfc2ca", t("Medium gray")],
  ]);
  const smoothing = $derived([
    ["", t("Automatic")],
    ["antialiased", "antialiased"],
    ["subpixel-antialiased", "subpixel-antialiased"],
    ["none", t("None")],
  ]);
  const rendering = ["", "optimizeSpeed", "optimizeLegibility", "geometricPrecision"];
</script>

<div class="tuner" role="dialog" aria-label={t("Typography")}>
  <div class="head">
    <strong>{t("Typography")}</strong>
    <button type="button" onclick={onclose} aria-label={t("Close")}>×</button>
  </div>

  <FontPicker label={t("Interface font")} bind:value={value.family} {fonts} fallback="Inter" />
  <FontPicker label={t("Code font")} bind:value={value.codeFamily} {fonts} fallback="JetBrains Mono" />

  <label>
    {t("Weight")}: <b>{value.weight}</b>
    <input type="range" min="300" max="600" step="10" bind:value={value.weight} />
  </label>
  <label>
    {t("Size")}: <b>{value.size}px</b>
    <input type="range" min="12" max="17" step="0.5" bind:value={value.size} />
  </label>

  <button type="button" class="link" onclick={() => (advanced = !advanced)}>
    {advanced ? "▾" : "▸"} {t("Advanced")}
  </button>
  {#if advanced}
    <label>
      {t("Text color")}
      <select bind:value={value.color}>{#each colors as [v, n]}<option value={v}>{n}</option>{/each}</select>
    </label>
    <label>
      {t("Font smoothing")}
      <select bind:value={value.smoothing}>{#each smoothing as [v, n]}<option value={v}>{n}</option>{/each}</select>
    </label>
    <label>
      text-rendering
      <select bind:value={value.rendering}>{#each rendering as v}<option value={v}>{v || t("Automatic")}</option>{/each}</select>
    </label>
  {/if}

  <div class="foot">
    <button type="button" onclick={() => (value = { ...DEFAULT_TUNING })}>{t("Reset")}</button>
  </div>
</div>

<style>
  .tuner {
    position: fixed;
    top: 52px;
    right: 14px;
    z-index: 2000;
    width: 290px;
    max-height: calc(100vh - 80px);
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-secondary);
    border-radius: 10px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
    font-size: 12px;
    color: var(--text-secondary);
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    color: var(--text-primary);
  }
  .head button {
    border: none;
    background: none;
    color: inherit;
    font-size: 18px;
    cursor: pointer;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  b {
    color: var(--text-primary);
  }
  input:not([type="range"]),
  select {
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    padding: 5px 7px;
    font: inherit;
  }
  .link {
    align-self: flex-start;
    border: none;
    background: none;
    color: var(--text-secondary);
    cursor: pointer;
    padding: 0;
    font: inherit;
  }
  .foot button {
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    padding: 4px 10px;
    cursor: pointer;
  }
</style>
