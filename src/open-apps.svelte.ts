import { t } from "./i18n";
// ianox — apps externas detectadas para "Abrir en" (estado compartido)
import { invoke } from "@tauri-apps/api/core";

export interface OpenApp {
  id: string;
  label: string;
  /** Program icon as a data URL (resolved by the backend). */
  icon?: string | null;
  kind?: string;
}

const FALLBACK: OpenApp[] = [{ id: "finder", label: t("File manager"), icon: null, kind: "system" }];

export const openApps = $state({
  items: FALLBACK as OpenApp[],
  preferred: (typeof localStorage !== "undefined" && localStorage.getItem("ianox.openApp")) || "finder",
});

export async function loadOpenApps(path?: string | null): Promise<void> {
  try {
    const list = await invoke<OpenApp[]>("detect_open_apps", { path: path ?? null });
    if (list && list.length > 0) {
      openApps.items = list;
    }
  } catch {
    /* fuera de Tauri o error */
  }
}

export function setPreferredApp(id: string): void {
  openApps.preferred = id;
  try {
    localStorage.setItem("ianox.openApp", id);
  } catch {
    /* almacenamiento no disponible */
  }
}

export function preferredApp(): OpenApp | undefined {
  return openApps.items.find((a) => a.id === openApps.preferred) ?? openApps.items[0];
}
