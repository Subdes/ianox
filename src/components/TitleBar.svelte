<script lang="ts">
  import { t } from "../i18n";
  // ianox — custom title bar (the window has no native decorations)
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import LogoMark from "./LogoMark.svelte";

  const win = (() => {
    try {
      return getCurrentWindow();
    } catch {
      return null;
    }
  })();
</script>

<div class="titlebar" data-tauri-drag-region>
  <div class="titlebar-brand" data-tauri-drag-region>
    <LogoMark size={18} radius={5} />
    <span>ianox</span>
  </div>

  <div class="titlebar-spacer" data-tauri-drag-region></div>

  <div class="titlebar-controls">
    <button class="titlebar-btn" title={t("Minimize")} onclick={() => win?.minimize()}>
      <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
        <line x1="2" y1="6" x2="10" y2="6" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
      </svg>
    </button>
    <button class="titlebar-btn" title={t("Maximize")} onclick={() => win?.toggleMaximize()}>
      <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
        <rect x="2.2" y="2.2" width="7.6" height="7.6" rx="1.4" fill="none" stroke="currentColor" stroke-width="1.3" />
      </svg>
    </button>
    <button class="titlebar-btn close" title={t("Close")} onclick={() => win?.close()}>
      <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
        <line x1="2.5" y1="2.5" x2="9.5" y2="9.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
        <line x1="9.5" y1="2.5" x2="2.5" y2="9.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
      </svg>
    </button>
  </div>
</div>
