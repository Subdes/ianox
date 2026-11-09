<script lang="ts">
  import { t } from "../i18n";
  // ianox — PTY terminal host (no frame; the workbench provides it)
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import "@xterm/xterm/css/xterm.css";

  let {
    cwd,
    visible = true,
    onClose,
    onNewTab,
  }: {
    cwd: string | null;
    visible?: boolean;
    onClose?: () => void;
    onNewTab?: () => void;
  } = $props();

  let host: HTMLDivElement;
  let error = $state<string | null>(null);
  let termRef = $state<Terminal | null>(null);
  let fitRef = $state<FitAddon | null>(null);

  const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad|iPod/.test(navigator.platform);

  $effect(() => {
    const c = cwd;
    const el = host;
    if (!el) return;
    return untrack(() => {
      // The code font the user picked (Aa panel), then Nerd Font symbols so the icons and
      // separators of prompts such as powerline/p10k draw instead of showing empty boxes.
      const userFont = getComputedStyle(document.documentElement).getPropertyValue("--font-code").trim();
      const fontFamily = [
        userFont || '"JetBrains Mono Variable"',
        '"Symbols Nerd Font Mono"',
        '"JetBrainsMono Nerd Font Mono"',
        '"Symbols Nerd Font"',
        'ui-monospace, "SF Mono", "Cascadia Code", Consolas, "DejaVu Sans Mono", monospace',
      ].join(", ");
      const term = new Terminal({
        fontFamily,
        fontSize: 12.5,
        lineHeight: 1.2,
        cursorBlink: true,
        scrollback: 5000,
        theme: {
          background: "#191515",
          foreground: "#d4d4d4",
          cursor: "#d4d4d4",
          cursorAccent: "#191515",
          selectionBackground: "rgba(212,212,212,0.25)",
          black: "#1c1f26",
          red: "#ef4444",
          green: "#22c55e",
          yellow: "#f59e0b",
          blue: "#5c8dff",
          magenta: "#a78bfa",
          cyan: "#22d3ee",
          white: "#e8e9ed",
        },
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      term.open(el);

      // Manejo inteligente de atajos multiplataforma (Linux, macOS, Windows)
      term.attachCustomKeyEventHandler((e: KeyboardEvent) => {
        if (e.type !== "keydown") return true;

        const key = e.key.toLowerCase();

        // 1. Copiar selección:
        //    macOS: Cmd+C (o Ctrl+Shift+C)
        //    Linux / Windows: Ctrl+Shift+C (o Ctrl+C si hay selección de texto activa)
        const isCopy =
          (isMac && e.metaKey && !e.shiftKey && key === "c") ||
          (!isMac && e.ctrlKey && e.shiftKey && key === "c") ||
          (!isMac && e.ctrlKey && !e.shiftKey && key === "c" && term.hasSelection());

        if (isCopy && term.hasSelection()) {
          const text = term.getSelection();
          navigator.clipboard.writeText(text).catch(() => {});
          e.preventDefault();
          return false;
        }

        // 2. Pegar portapapeles:
        //    macOS: Cmd+V (o Ctrl+Shift+V)
        //    Linux / Windows: Ctrl+Shift+V
        const isPaste =
          (isMac && e.metaKey && !e.shiftKey && key === "v") ||
          (!isMac && e.ctrlKey && e.shiftKey && key === "v");

        if (isPaste) {
          navigator.clipboard
            .readText()
            .then((text) => {
              if (text) term.paste(text);
            })
            .catch(() => {});
          e.preventDefault();
          return false;
        }

        // 3. Seleccionar todo:
        //    macOS: Cmd+A (o Ctrl+Shift+A)
        //    Linux / Windows: Ctrl+Shift+A
        const isSelectAll =
          (isMac && e.metaKey && !e.shiftKey && key === "a") ||
          (!isMac && e.ctrlKey && e.shiftKey && key === "a");

        if (isSelectAll) {
          term.selectAll();
          e.preventDefault();
          return false;
        }

        // 4. Nueva pestaña de terminal (estándar de terminales):
        //    macOS: Cmd+T o Cmd+Shift+T
        //    Linux / Windows: Ctrl+Shift+T
        const isNewTab =
          (isMac && e.metaKey && (key === "t")) ||
          (!isMac && e.ctrlKey && e.shiftKey && key === "t");

        if (isNewTab) {
          e.preventDefault();
          onNewTab?.();
          return false;
        }

        // 5. Cerrar pestaña terminal:
        //    macOS: Cmd+W o Cmd+Shift+D
        //    Linux / Windows: Ctrl+Shift+D o Ctrl+Shift+W
        const isClose =
          (isMac && e.metaKey && !e.shiftKey && key === "w") ||
          (isMac && e.metaKey && e.shiftKey && key === "d") ||
          (!isMac && e.ctrlKey && e.shiftKey && (key === "d" || key === "w"));

        if (isClose) {
          e.preventDefault();
          onClose?.();
          return false;
        }

        return true;
      });

      fit.fit();
      term.focus();
      termRef = term;
      fitRef = fit;

      let disposed = false;
      let unlistenOutput: UnlistenFn | undefined;
      let unlistenExit: UnlistenFn | undefined;
      let resizeObserver: ResizeObserver | undefined;
      let termId: string | null = null;

      const start = async () => {
        try {
          const id = await invoke<string>("term_start", { cwd: c, rows: term.rows, cols: term.cols });
          termId = id;
          if (disposed) {
            invoke("term_kill", { id }).catch(() => {});
            return;
          }
          unlistenOutput = await listen<string>(`term://output/${id}`, (event) => term.write(event.payload));
          unlistenExit = await listen(`term://exit/${id}`, () => {
            // Si el shell finaliza (ej. 'exit' o Ctrl+D), cerramos la pestaña
            onClose?.();
          });
          term.onData((data) => invoke("term_write", { id, data }).catch(() => {}));
          resizeObserver = new ResizeObserver(() => {
            try {
              fit.fit();
              invoke("term_resize", { id, rows: term.rows, cols: term.cols }).catch(() => {});
            } catch {
              /* no size yet */
            }
          });
          resizeObserver.observe(el);
        } catch (e) {
          error = String(e);
        }
      };

      start();

      return () => {
        disposed = true;
        unlistenOutput?.();
        unlistenExit?.();
        resizeObserver?.disconnect();
        if (termId) invoke("term_kill", { id: termId }).catch(() => {});
        term.dispose();
        termRef = null;
        fitRef = null;
      };
    });
  });

  $effect(() => {
    if (!visible) return;
    const frame = requestAnimationFrame(() => {
      try {
        fitRef?.fit();
        termRef?.focus();
      } catch {
        /* oculto */
      }
    });
    return () => cancelAnimationFrame(frame);
  });
</script>

{#if error}<div class="file-note error">{error}</div>{/if}
<div class="terminal-host" bind:this={host}></div>
