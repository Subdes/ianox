// ianox — punto de entrada de la interfaz (Svelte 5)
import { mount } from "svelte";
import App from "./App.svelte";
import "@fontsource-variable/inter/wght.css";
import "@fontsource-variable/jetbrains-mono/wght.css";
import "./styles.css";
import { invoke } from "@tauri-apps/api/core";

// Compiled builds have no browser right-click menu ("Reload", "Inspect"…): it is not part of the
// app. Development keeps it so the inspector stays reachable.
if (!import.meta.env.DEV) {
  window.addEventListener("contextmenu", (event) => event.preventDefault());
}

// Without a compositing manager (common in virtual machines) a see-through window shows black
// corners, so the interface then draws an opaque, square window.
invoke<boolean>("window_alpha")
  .then((alpha) => {
    if (!alpha) document.documentElement.dataset.opaque = "1";
  })
  .catch(() => {
    /* outside Tauri (plain browser): nothing to adapt */
  });

// Aplicar tema en el HTML antes de renderizar para prevenir cualquier parpadeo visual
try {
  const saved = localStorage.getItem("ianox.theme");
  if (saved) {
    let eff = saved;
    if (saved === "system") {
      eff = window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
    }
    document.documentElement.setAttribute("data-theme", eff);
  }
} catch {
  /* ignore */
}

const app = mount(App, { target: document.getElementById("root")! });

export default app;
