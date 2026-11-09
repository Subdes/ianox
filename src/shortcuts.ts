// ianox — application keyboard shortcuts (single source of truth)

export interface Shortcut {
  id: string;
  label: string;
  keys: string[];
}

export const SHORTCUTS: Shortcut[] = [
  { id: "app.quit", label: "Quit ianox", keys: ["Ctrl", "Q"] },
  { id: "search.sessions", label: "Search conversations", keys: ["Ctrl", "K"] },
  { id: "panel.toggle", label: "Open or close the panel", keys: ["Ctrl", "`"] },
  { id: "tab.close", label: "Close the active tab", keys: ["Ctrl", "W"] },
  { id: "tab.reopen", label: "Reopen the last closed tab", keys: ["Ctrl", "Shift", "T"] },
  { id: "panel.terminal", label: "New terminal", keys: ["Ctrl", "Alt", "T"] },
  { id: "panel.file", label: "Open file", keys: ["Ctrl", "Alt", "P"] },
  { id: "panel.review", label: "Review (git changes)", keys: ["Ctrl", "Alt", "R"] },
  { id: "panel.browser", label: "Open browser", keys: ["Ctrl", "Alt", "B"] },
];

export function keysFor(id: string): string[] {
  return SHORTCUTS.find((s) => s.id === id)?.keys ?? [];
}
