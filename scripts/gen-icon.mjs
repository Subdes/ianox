#!/usr/bin/env node
/**
 * gen-icon.mjs — genera src-tauri/icons/source.png a partir del SVG maestro.
 *
 * Uso: node scripts/gen-icon.mjs
 * Orden de rasterizadores: rsvg-convert → Inkscape → Python (librsvg+cairo)
 * → copia de branding/ianox-icon.png como fallback.
 *
 * Note: ImageMagick is NOT used because its built-in SVG renderer ignores the
 * degradados y produce un tile negro.
 */
import { execFileSync } from "node:child_process";
import { existsSync, copyFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const svg = resolve(root, "branding/ianox-icon.svg");
const fallbackPng = resolve(root, "branding/ianox-icon.png");
const outDir = resolve(root, "src-tauri/icons");
const outPng = resolve(outDir, "source.png");
const outSvg = resolve(outDir, "source.svg");
const SIZE = 1024;

if (!existsSync(svg)) {
  console.error(`✖ No existe el SVG maestro: ${svg}`);
  process.exit(1);
}

mkdirSync(outDir, { recursive: true });

const has = (bin) => {
  try {
    execFileSync("which", [bin], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
};

const PY_RSVG = `
import cairo, gi, sys
gi.require_version('Rsvg', '2.0')
from gi.repository import Rsvg
svg_path, out_path, size = sys.argv[1], sys.argv[2], int(sys.argv[3])
handle = Rsvg.Handle.new_from_file(svg_path)
surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, size, size)
ctx = cairo.Context(surface)
ctx.scale(size / 512.0, size / 512.0)
handle.render_cairo(ctx)
surface.write_to_png(out_path)
`;

let rendered = false;

try {
  if (has("rsvg-convert")) {
    execFileSync("rsvg-convert", ["-w", String(SIZE), "-h", String(SIZE), "-o", outPng, svg], { stdio: "inherit" });
    rendered = true;
  } else if (has("inkscape")) {
    execFileSync("inkscape", [svg, "--export-type=png", `--export-filename=${outPng}`, `--export-width=${SIZE}`, `--export-height=${SIZE}`], { stdio: "inherit" });
    rendered = true;
  } else if (has("python3")) {
    execFileSync("python3", ["-c", PY_RSVG, svg, outPng, String(SIZE)], { stdio: "inherit" });
    rendered = true;
  }
} catch (err) {
  console.warn(`⚠ Falló el rasterizado del SVG: ${err.message}`);
  rendered = false;
}

if (!rendered) {
  if (!existsSync(fallbackPng)) {
    console.error("✖ No hay rasterizador disponible ni PNG de respaldo.");
    process.exit(1);
  }
  console.log("→ Copiando branding/ianox-icon.png como source.png (fallback).");
  copyFileSync(fallbackPng, outPng);
}

copyFileSync(svg, outSvg);
console.log(`✔ source.png listo (${SIZE}×${SIZE})`);
console.log("✔ source.svg copiado");
console.log("→ Siguiente paso: npx tauri icon src-tauri/icons/source.png");
