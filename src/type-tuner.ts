/**
 * Live typography tuning. Text rendering depends on the machine (fontconfig, GTK,
 * the WebKit build), so these values are picked by eye inside the real app and kept
 * in localStorage. The "Aa" button in the top bar opens the panel.
 */
export interface Tuning {
  weight: number;
  size: number;
  color: string;
  family: string;
  codeFamily: string;
  smoothing: string;
  rendering: string;
}

export const DEFAULT_TUNING: Tuning = {
  weight: 400,
  size: 14,
  color: "",
  family: "",
  codeFamily: "",
  smoothing: "",
  rendering: "",
};

const KEY = "ianox.tuning";

export function loadTuning(): Tuning {
  try {
    return { ...DEFAULT_TUNING, ...(JSON.parse(localStorage.getItem(KEY) ?? "{}") as Partial<Tuning>) };
  } catch {
    return { ...DEFAULT_TUNING };
  }
}

export function applyTuning(t: Tuning): void {
  const root = document.documentElement.style;
  const set = (name: string, value: string) => (value ? root.setProperty(name, value) : root.removeProperty(name));
  set("--chat-weight", String(t.weight));
  set("--chat-size", `${t.size}px`);
  set("--chat-color", t.color);
  set("--chat-smoothing", t.smoothing);
  set("--chat-rendering", t.rendering);
  // A family typed by the user may contain spaces: quote it. Fallbacks keep the UI usable if it vanishes.
  const quote = (name: string) => (/^[\w-]+$/.test(name) ? name : `"${name.replace(/"/g, "")}"`);
  set("--font-ui", t.family ? `${quote(t.family)}, "Inter Variable", system-ui, sans-serif` : "");
  set("--font-code", t.codeFamily ? `${quote(t.codeFamily)}, "JetBrains Mono Variable", ui-monospace, monospace` : "");
  try {
    localStorage.setItem(KEY, JSON.stringify(t));
  } catch {
    /* almacenamiento no disponible */
  }
}
