import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// ianox usa Vite sólo para la interfaz. La salida va a `dist/`, que es lo que
// Tauri incrusta en el binario. No hay servidor de desarrollo en producción:
// la app final es un único ejecutable nativo con el frontend embebido.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 5183,
    strictPort: true,
    watch: {
      // El runtime de Rust no debe disparar recargas del frontend.
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "esnext",
    sourcemap: false,
    chunkSizeWarningLimit: 1500,
  },
});
