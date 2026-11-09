/**
 * Project icons: found in the project folder or picked by the user, shrunk to a
 * small PNG so the config file stays light.
 */
import { invoke } from "@tauri-apps/api/core";

const SIZE = 64;

/** Draws any browser-readable image into a SIZE×SIZE PNG data URL (keeping its proportions). */
export function shrinkIcon(source: string, size = SIZE): Promise<string> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => {
      const canvas = document.createElement("canvas");
      canvas.width = size;
      canvas.height = size;
      const context = canvas.getContext("2d");
      if (!context) return reject(new Error("canvas unavailable"));
      const width = image.naturalWidth || size;
      const height = image.naturalHeight || size;
      const scale = Math.min(size / width, size / height);
      const w = width * scale;
      const h = height * scale;
      context.drawImage(image, (size - w) / 2, (size - h) / 2, w, h);
      resolve(canvas.toDataURL("image/png"));
    };
    image.onerror = () => reject(new Error("unreadable image"));
    image.src = source;
  });
}

/** Reads a file chosen with an <input type="file"> as a data URL. */
export function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

/** Icon found inside the project folder, already shrunk; null when there is none. */
export async function detectProjectIcon(path: string): Promise<string | null> {
  const found = await invoke<string | null>("find_project_icon", { path });
  if (!found) return null;
  try {
    return await shrinkIcon(found);
  } catch {
    return null;
  }
}
