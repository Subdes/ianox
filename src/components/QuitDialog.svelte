<script lang="ts">
  import { t } from "../i18n";
  import Modal from "./Modal.svelte";
  import { app, quitApp, keepInTray, rememberQuitAction } from "../app.svelte";

  let remember = $state(false);

  async function choose(action: "quit" | "tray"): Promise<void> {
    app.quitOpen = false;
    if (remember) await rememberQuitAction(action);
    if (action === "quit") await quitApp();
    else await keepInTray();
  }
</script>

<Modal title={t("Close ianox?")} onclose={() => (app.quitOpen = false)} maxWidth={440}>
  <p style="margin:0 0 14px;color:var(--text-secondary);line-height:1.5">
    {t("Do you want to quit ianox, or keep it running in the system tray? The gateway stays available while it is in the tray.")}
  </p>
  <label style="display:flex;align-items:center;gap:8px;margin-bottom:18px;color:var(--text-secondary);font-size:13px">
    <input type="checkbox" bind:checked={remember} />
    {t("Don't ask again")}
  </label>
  <div style="display:flex;gap:8px;justify-content:flex-end;flex-wrap:wrap">
    <button class="btn btn-ghost" onclick={() => (app.quitOpen = false)}>{t("Cancel")}</button>
    <button class="btn" onclick={() => choose("tray")}>{t("Keep in tray")}</button>
    <button class="btn btn-primary" onclick={() => choose("quit")}>{t("Quit")}</button>
  </div>
</Modal>
