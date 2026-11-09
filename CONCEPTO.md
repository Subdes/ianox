# Concepto de ianox — AI Gateway Local

> Documento vivo para alinear identidad visual, arquitectura y comunicación. Editado por ambos agentes.
>
> **v4 — 2026-11-02: IDENTIDAD VISUAL APLICADA.** Iconos regenerados y acento `#1E5FFF` en la app. Sustituye por completo la identidad anterior (hexágono + keyhole azul `#6d7cff`), **descartada**. Ver §11.
>
> **v3 — 2026-11-02: IDENTIDAD VISUAL DEFINIDA.** (Histórico)

---

## ✅ APLICADO — IDENTIDAD VISUAL EN LA APP (2026-11-02)

La identidad v3 está **aplicada** en el código. Resumen de lo hecho:

**1) Iconos (assets).** Maestros en `branding/`:
- `branding/ianox-icon.svg` — SVG maestro (chispa sobre tile grafito)
- `branding/ianox-icon.psd` — PSD por capas (Fondo + Chispa)
- `branding/ianox-icon.png` — PNG 512×512 RGBA

Aplicación:
- `src-tauri/icons/source.svg` ← copia de `branding/ianox-icon.svg`.
- `src-tauri/icons/source.png` ← render del SVG a **1024×1024** RGBA (librsvg+cairo/`scripts/gen-icon.mjs`).
- Set completo regenerado con `npx tauri icon src-tauri/icons/source.png` (`16x16`…`512x512`, `icon.ico`, `icon.icns`, `Square*Logo`, `StoreLogo`, `android/`, `ios/`).
- Restaurado `scripts/gen-icon.mjs` (faltaba): rasteriza el SVG y copia `source.svg`. `npm run icons` vuelve a funcionar.
- `ianox_icon_tile.svg` (tile azul antiguo) ya no existe en la raíz.
- Logo del sidebar (`src/components/Sidebar.tsx`) actualizado de hexágono+keyhole a la **chispa `#1E5FFF` sobre tile grafito `#26262E→#141418`** (colores exactos del maestro, sin reinterpretar).

> ⚠️ Se aplicó el SVG entregado **tal cual**: el tile va inset 44/512 (≈8,6 % de margen transparente). Si el owner entrega la variante a lienzo completo (tile 0,0,512,512), basta con reemplazar `branding/ianox-icon.svg` y re-ejecutar `npm run icons`.

**2) Color de acento en la UI.** Valores vigentes en **ambos** CSS:

| Variable | Valor |
|----------|-------|
| `--accent-primary` | `#1E5FFF` |
| `--accent-primary-hover` / `--accent-hover` | `#1249D6` |
| `--accent-secondary` | `#5C8DFF` |
| `--accent-glow` | `rgba(30, 95, 255, 0.30)` |
| `--border-focus` (main.css) | `#1E5FFF` |

- Aplicado en modo oscuro y claro de `src/styles.css` (el importado en `src/main.tsx`) y de `src/styles/main.css`.
- Neutros y semánticos de `src/styles.css` alineados a los valores facilitados: `#15171C`/`#1C1F26`, `#E8E9ED`/`#A0A4B0`/`#6B6F7C`, `#22C55E`/`#F59E0B`/`#EF4444` (antes usaba variantes más claras).
- `Chat.tsx`: sombra de burbuja de usuario actualizada a `rgba(30,95,255,0.3)`.
- `.badge-info` actualizado a `rgba(30,95,255,0.15)`.

**3) Verificación.** `cargo check` y `npm run build` limpios; pendiente comprobación visual en tray/dock/taskbar/instalador tras el próximo `tauri build`.

---

## 1. Nombre
- **Definitivo:** `ianox`
- **Candidatos alternativos:** —
- **Significado / origen:** Por definir (¿acrónimo? ¿inventado? ¿referencia a "janus" = puerta bifronte?)

---

## 2. Propósito (Elevator Pitch)
**Una frase:**  
> ianox es una puerta de enlace de IA local que enruta peticiones entre múltiples proveedores (OpenAI, Anthropic, Ollama, OpenRouter, Google, Azure, Custom) bajo una sola API compatible con OpenAI/Anthropic/Ollama, con enrutamiento inteligente, fallbacks y gestión de modelos — todo en tu máquina, sin datos en la nube.

**Problema que resuelve:**  
- Fragmentación de APIs y claves entre proveedores  
- Dependencia de conectividad externa / privacidad  
- Cambiar de modelo/proveedor requiere tocar código del cliente  
- No hay forma sencilla de "fallback automático" si un proveedor falla o tiene rate-limit  

**Valor diferencial (USP):**  
- **Local-first & privacy-first** — cero telemetría, cero nube obligatoria  
- **9router-style routing** — patrones glob, fallback chain, weighted providers, prioridades  
- **Multi-protocol** — OpenAI + Anthropic + Ollama endpoints simultáneos  
- **Desktop app nativa** (Tauri/Rust) — ligera, bandeja del sistema, auto-start  
- **Extensible** — proveedores custom, rate-limiting, auth token opcional  

---

## 3. Público Objetivo
| Segmento | Descripción |
|----------|-------------|
| **Primario** | Desarrolladores que integran LLMs en apps/scripts y quieren un gateway local único |
| **Secundario** | Investigadores / power-users que comparan modelos locales (Ollama) y remotos |
| **Terciario** | Equipos que necesitan gateway on-premise con control de claves y auditoría |

**Contexto de uso:** Escritorio (Linux/macOS/Windows), banda de sistema, CLI opcional futura  
**Nivel técnico:** Medio-alto (configura JSON/YAML, entiende APIs, patrones glob)

---

## 4. Personalidad de Marca (3–5 adjetivos)
1. **Técnico** — honesto, sin marketing fluff, hecho por/para devs
2. **Confiable** — Rust, tipos estáticos, fallbacks deterministas
3. **Rápido** — binario nativo, sin Electron overhead, streaming SSE nativo
4. **Minimalista** — UI limpia, atajos de teclado, cero bloat
5. **Local-first / Soberano** — tus datos, tus claves, tu máquina

---

## 5. Paleta de Colores — **NUEVA (vigente)**

| Rol | HEX | Uso |
|-----|-----|-----|
| **Acento de marca (chispa)** | **`#1E5FFF`** | Chispa del icono + `--accent-primary` de la UI (botones, links, activos) |
| **Acento hover** | `#1249D6` | Hover/pressed |
| **Acento secundario** | `#5C8DFF` | Degradados de acento (sidebar), apoyos |
| **Glow de acento** | `rgba(30,95,255,0.30)` | Halos/focus |
| **Tile fondo — inicio** | `#26262E` | Gradiente del tile del icono |
| **Tile fondo — fin** | `#141418` | Gradiente del tile del icono |
| **Superficie app (dark)** | `#0F1115` / `#15171C` / `#1C1F26` | `--bg-primary/secondary/tertiary` (ya existentes) |
| **Texto** | `#E8E9ED` / `#A0A4B0` / `#6B6F7C` | `--text-primary/secondary/muted` (ya existentes) |
| **Semánticos** | `#22C55E` éxito · `#F59E0B` warning · `#EF4444` danger | Estados del servidor/proveedor (ya existentes) |

> **Nota:** La identidad cromática de acento pasa de azul periwinkle (`#6d7cff` / `#4a9eff`) a **azul eléctrico profundo `#1E5FFF`**. Es el único azul de marca; el resto son neutros grafito y semánticos.
> **Pendiente:** unificar estructuralmente las dos paletas CSS (`main.css` y `styles.css`): los valores de acento ya coinciden, pero el archivo `src/styles/main.css` sigue sin importarse (solo se usa `src/styles.css`).

---

## 6. Metáforas Visuales — **RESUELTAS**

La metáfora del icono es la **chispa** (destello de 4 puntas con lados cóncavos): inteligencia / IA de forma directa y reconocible. Se descartan hexágono, keyhole, agujero central, anillo, cerebro, circuito, escudo y candado.

---

## 7. Referencias de Iconos (nivel de acabado, NO copiar)

| Referencia | Qué observar (acabado, no símbolo) |
|------------|-----------------------------------|
| **VS Code / Docker / Raycast / Ollama / Postman** | Legibilidad a 16px, presencia en dock, color de marca fuerte |

---

## 8. Restricciones Técnicas del Icono
| Requisito | Detalle |
|-----------|---------|
| **Tamaños** | 16, 32, 48, 64, 128, 256, 512 |
| **Formatos** | `.svg` (master), `.png` (set), `.ico` (Windows), `.icns` (macOS) |
| **Monocromático** | Debe leerse en 1 color (blanco/negro) |
| **Fondo** | Transparente. El **tile** (squircle grafito) forma parte del icono |
| **Generación** | `tauri icon src-tauri/icons/source.png` (el `scripts/gen-icon.mjs` de `npm run icons` falta; restaurar o usar `tauri icon` directo) |

---

## 9. Elementos Obligatorios / Prohibidos (vigente)

| Tipo | Detalle |
|------|---------|
| **Obligatorios** | - Icono = **tile squircle + símbolo bold** (no glifo suelto)<br>- Legible a 16px<br>- Monocromo OK<br>- Fondo transparente |
| **Prohibidos** | - Hexágonos / panales<br>- Keyholes / agujeros de llave / recortes circulares / anillos<br>- Cerebros, circuitos decorativos, escudos, candados<br>- Azul **pastel** (`#6d7cff`, `#4a9eff`, periwinkle)<br>- Sombras/3D/biseles anticuados |
| **Permitido ahora** | - Azul de marca **saturado** `#1E5FFF` (no pastel) |

---

## 10. Estado del Proyecto
| Campo | Valor |
|-------|-------|
| **Fase actual** | Identidad visual **aplicada** (iconos + acento). App: MVP funcional (Svelte 5 + Rust). |
| **Repo real** | `/home/soplos/Proyectos/ianox` — Tauri 2 + Rust + Svelte 5 |
| **Identidad** | Chispa eléctrico profundo `#1E5FFF` sobre tile grafito `#26262E→#141418` |
| **Assets** | `branding/ianox-icon.{svg,psd,png}` |
| **No tocar** | Lógica Svelte/Rust; solo assets + CSS de acento |
| **Aplicado** | `source.svg`/`source.png` + set `tauri icon`; acento `#1E5FFF` en ambos CSS; logo sidebar = chispa |

---

## 11. Decisiones de Diseño del Icono — **NUEVA IDENTIDAD**

| Decisión | Elección | Detalle |
|----------|----------|---------|
| **Formato** | Tile squircle | Cuadrado redondeado con fondo, se lee como app en dock/taskbar |
| **Símbolo** | Chispa de 4 puntas | Destello cóncavo; lee "IA" al instante |
| **Fondo del tile** | Gradiente grafito | `#26262E → #141418` diagonal 45° |
| **Color de la chispa** | **`#1E5FFF`** | Azul eléctrico profundo (no pastel) |
| **Radio del tile** | 104 sobre 424 (proporcional ≈24,5 %) | Ajuste final a lienzo completo por el owner |
| **Geometría chispa** | Anclas N/E/S/W con lados cóncavos | Ver `branding/ianox-icon.svg` |

> **NOTA:** el owner ajustará el PSD a **lienzo completo** (tile a 0,0,512,512). Al aplicarlo en la app, regenerar también el SVG/PNG coherentes.

### Identidad anterior — DESCARTADA
| Decisión (ANULADA) | Estado |
|--------------------|--------|
| ~~Hexágono~~ | ❌ |
| ~~Keyhole / portal con agujero~~ | ❌ |
| ~~Azul periwinkle `#6d7cff` + blanco~~ | ❌ |

---

## 12. Checklist de Entrega
- [x] Identidad y paleta definidas
- [x] SVG maestro (`branding/ianox-icon.svg`)
- [x] PSD por capas (`branding/ianox-icon.psd`: Fondo + Chispa)
- [x] PNG 512 (`branding/ianox-icon.png`)
- [x] **App:** `src-tauri/icons/source.svg` + `source.png` (1024) y set regenerado (`tauri icon`)
- [x] **App:** acento `#1E5FFF` en `main.css` y `styles.css` (valores unificados; archivos aún duplicados)
- [x] **App:** `ianox_icon_tile.svg` eliminado (no existe) y logo del sidebar actualizado a la chispa
- [x] `scripts/gen-icon.mjs` restaurado
- [ ] Verificar tray/dock/taskbar/instalador tras `tauri build`

---

> **Nota:** Este doc vive en la raíz del repo. Cualquier cambio en identidad → actualizar aquí primero.
