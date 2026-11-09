<script lang="ts">
  import { t } from "../i18n";
  // ianox — file viewer: preview / source, copy, download and "Open in"
  import { untrack } from "svelte";
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";
  import Markdown from "./Markdown.svelte";
  import Icon from "./Icon.svelte";
  import AppIcon from "./AppIcon.svelte";
  import { openApps, loadOpenApps, setPreferredApp } from "../open-apps.svelte";

  type Mode = "text" | "markdown" | "html" | "svg" | "image" | "pdf";

  const IMAGE_EXT = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "avif"];

  let { path }: { path: string } = $props();

  let mode = $state<Mode>("text");
  let text = $state("");
  let dataUrl = $state("");
  let loading = $state(true);
  let error = $state<string | null>(null);
  let truncated = $state(false);
  let size = $state<number | null>(null);
  let source = $state(false);
  let menu = $state(false);
  let toast = $state<string | null>(null);

  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  const name = $derived(path.split("/").filter(Boolean).pop() ?? path);
  const canToggle = $derived(mode === "markdown" || mode === "html" || mode === "svg");
  const canDownload = $derived(
    mode === "text" || mode === "markdown" || mode === "html" || mode === "svg"
  );

  function flash(message: string): void {
    toast = message;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 1600);
  }

  function fmtSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} kB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  $effect(() => {
    const p = path;
    loadOpenApps();

    return untrack(() => {
      let cancelled = false;
      loading = true;
      error = null;
      text = "";
      dataUrl = "";
      truncated = false;
      size = null;
      source = false;
      menu = false;

      const ext = p.split(".").pop()?.toLowerCase() ?? "";

      invoke<{ size: number }>("file_info", { path: p })
        .then((info) => {
          if (!cancelled) size = info.size;
        })
        .catch(() => {});

      (async () => {
        try {
          if (IMAGE_EXT.includes(ext) || ext === "pdf") {
            const file = await invoke<{ mime: string; data: string }>("read_file_binary", { path: p });
            if (cancelled) return;
            mode = ext === "pdf" ? "pdf" : "image";
            dataUrl = `data:${file.mime};base64,${file.data}`;
          } else if (ext === "svg") {
            const file = await invoke<{ content: string }>("read_file", { path: p });
            if (cancelled) return;
            mode = "svg";
            text = file.content;
            dataUrl = `data:image/svg+xml;utf8,${encodeURIComponent(file.content)}`;
          } else {
            const file = await invoke<{ content: string; truncated: boolean }>("read_file", { path: p });
            if (cancelled) return;
            mode =
              ext === "md" || ext === "markdown"
                ? "markdown"
                : ext === "html" || ext === "htm"
                  ? "html"
                  : "text";
            text = file.content;
            truncated = file.truncated;
          }
        } catch (e) {
          if (!cancelled) error = String(e);
        } finally {
          if (!cancelled) loading = false;
        }
      })();

      return () => {
        cancelled = true;
        clearTimeout(toastTimer);
      };
    });
  });

  async function copyContent(): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
      flash("Contenido copiado");
    } catch (e) {
      error = String(e);
    }
  }

  async function copyPath(): Promise<void> {
    menu = false;
    try {
      await navigator.clipboard.writeText(path);
      flash(t("Path copied"));
    } catch (e) {
      error = String(e);
    }
  }

  async function download(): Promise<void> {
    menu = false;
    try {
      const dest = await save({ defaultPath: name });
      if (!dest) return;
      await invoke("write_text_file", { path: dest, content: text });
      flash("Archivo guardado");
    } catch (e) {
      error = String(e);
    }
  }

  async function openWith(app: string): Promise<void> {
    menu = false;
    try {
      await invoke("open_path", { path, app });
    } catch (e) {
      error = String(e);
    }
  }

  function reveal(): void {
    openWith("finder");
  }

  function chooseApp(app: string): void {
    setPreferredApp(app);
    openWith(app);
  }
</script>

<div class="file-view">
  <div class="panel-head file-head">
    <span class="panel-title" title={path}>{name}</span>

    <div class="file-head-right">
      {#if size !== null}<span class="file-size">{fmtSize(size)}</span>{/if}

      {#if canToggle}
        <button
          class={`file-action${source ? " active" : ""}`}
          title={source ? t("View preview") : t("View source")}
          onclick={() => (source = !source)}
        >
          <Icon name={source ? "eye" : "code"} size={14} />
        </button>
      {/if}

      {#if canDownload}
        <button class="file-action" title={t("Copy content")} onclick={copyContent}>
          <Icon name="copy" size={14} />
        </button>
      {/if}

      <div class="file-actions">
        <button class="file-action" title={t("File actions")} onclick={() => (menu = !menu)}>
          <Icon name="more" size={14} />
        </button>

        {#if menu}
          <div class="menu-backdrop" role="presentation" onclick={() => (menu = false)}></div>
          <div class="file-menu">
            {#if canDownload}
              <button class="menu-item" onclick={download}><span>{t("Download")}</span></button>
            {/if}
            {#if canToggle}
              <button
                class="menu-item"
                onclick={() => {
                  source = !source;
                  menu = false;
                }}
              >
                <span>{source ? t("View preview") : t("View source")}</span>
              </button>
            {/if}
            <button class="menu-item" onclick={reveal}><span>{t("Reveal in File Manager")}</span></button>
            <button class="menu-item" onclick={copyPath}><span>{t("Copy path")}</span></button>

            <div class="menu-divider"></div>

            <div class="menu-item has-sub" role="menuitem" tabindex="0">
              <span>{t("Open in")}</span>
              <span class="submenu-arrow">›</span>
              <div class="submenu">
                {#each openApps.items as app (app.id)}
                  <button class="menu-item" onclick={() => chooseApp(app.id)}>
                    <span class="app-menu-label"><AppIcon {app} size={15} />{app.label}</span>
                    {#if app.id === openApps.preferred}<Icon name="check" size={12} />{/if}
                  </button>
                {/each}
              </div>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>

  {#if loading}
    <div class="file-view-center">{t("Loading…")}</div>
  {:else if error}
    <div class="file-view-center" style="color:var(--danger)">⚠ {error}</div>
  {:else if source && canToggle}
    <pre class="file-view-body">{text}</pre>
  {:else if mode === "markdown"}
    <div class="file-view-markdown">
      <Markdown source={text} />
    </div>
  {:else if mode === "html"}
    <iframe
      class="file-view-frame"
      title={name}
      sandbox="allow-scripts allow-forms allow-popups allow-same-origin"
      src={convertFileSrc(path)}
    ></iframe>
  {:else if mode === "svg" || mode === "image"}
    <div class="file-view-image-wrap">
      <img class="file-view-image" src={dataUrl} alt={name} />
    </div>
  {:else if mode === "pdf"}
    <iframe class="file-view-frame" title={name} src={dataUrl}></iframe>
  {:else}
    <pre class="file-view-body">{text}</pre>
    {#if truncated}<div class="file-note">{t("Truncated view (large file)")}</div>{/if}
  {/if}

  {#if toast}<div class="file-toast">{toast}</div>{/if}
</div>
