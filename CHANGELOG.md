# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project follows [Semantic Versioning](https://semver.org/). Versions below `0.1.0` are
**pre-alpha**: anything may change or break between releases.

## [Unreleased]

## [0.0.4] - 2026-11-10

Corrección de inicio minimizado en bandeja, asistente flotante con streaming y selector de proveedor, correcciones de scroll y layouts flex en chat, y scripts multiplataforma de empaquetado (.msi, .dmg, .flatpak).

### Added

- **Selector de Proveedor en Asistente Flotante (`WidgetAssistant.svelte`):**
  - Selector dinámico de proveedores habilitados en la barra de opciones junto al selector de modelo.
  - Filtrado reactivo de modelos según el proveedor seleccionado y envío del parámetro `providerId` a `chat_stream`.
- **Soporte de empaquetado multi-distribución y multiplataforma:**
  - Script y manifiesto WiX (`packaging/windows/ianox.wxs`, `scripts/bundle-msi.sh`) compatibles con `wixl` para generar instalador nativo `.msi` en Windows.
  - Script `scripts/bundle-dmg.sh` con generación dual de `.dmg` y `.app.tar.gz` para macOS.
  - Script y manifiesto `scripts/bundle-flatpak.sh` con runtime GNOME 51 e iconos de alta resolución.
  - Script unificador `scripts/collect-packages.sh` y comando maestro `npm run build:all`.

### Fixed

- **Arranque minimizado en inicio automático (`main.rs`):**
  - Lectura de argumentos `--minimized` y `--hidden` al arrancar. Si la bandeja del sistema está inicializada y disponible, la ventana principal se mantiene oculta, evitando aperturas invasivas al inicio de sesión.
- **Asistente Flotante con Streaming y Cancelación (`WidgetAssistant.svelte`):**
  - Migración completa del comando stub `chat_completion` al motor reactivo `chat_stream` con eventos `chat://delta`, `chat://done` y `chat://error`.
  - Botón de cancelación inmediata (`handleStop`) conectado a `chat_cancel` y limpieza adecuada de listeners al desmontar el componente.
- **Doble scroll y compresión de mensajes en Chat (`styles.css`, `Chat.svelte`):**
  - Eliminado el doble contenedor con scroll mediante `overflow: hidden` en `.page-content.chat-content`.
  - Solucionado el colapso y aplastamiento de mensajes en contenedores flex con `flex-shrink: 0` y `min-height: 0` en `.app-main` y `.chat-scroll`.
  - Sustituido `overflow: hidden` por `clip-path: inset(0 round var(--radius-md))` en `.workspace-chat` para evitar parpadeos y repaints en blanco bajo WebKitGTK.
  - Anti-rebote en auto-scroll con flag `_programmaticScroll` para que el movimiento programático hacia abajo no desactive erróneamente el anclaje (`stickToBottom`).

Tray enriquecido, autostart, terminal multiplataforma con xterm, refresco reactivo de workspace, detección inteligente de aplicaciones y rediseño de atajos estándar.

### Added

- **Bandeja de sistema (Tray) enriquecida:**
  - Menú dinámico contextual con toggle Mostrar/Ocultar, acceso directo al Asistente Flotante con su acelerador de teclado.
  - Indicador de estado del servidor de la API en vivo (🟢 Activo / 🔴 Detenido).
  - Checkbox interactivo para configurar el inicio automático con el sistema operativo (autostart).
  - Manejo unificado de clics: clic izquierdo alterna la visibilidad de la ventana principal y clic derecho despliega el menú contextual.
- **Autostart multiplataforma (`autostart.rs`):**
  - Soporte nativo para autoinicio en Linux (`~/.config/autostart/ianox.desktop`), macOS (`LaunchAgents`) y Windows (registro de usuario).
  - Comandos Tauri `get_autostart_status` y `set_autostart`.
- **Detección contextual e inteligente de aplicaciones ("Abrir en..."):**
  - El backend analiza la extensión y tipo de archivo para ofrecer las mejores herramientas instaladas en el sistema (navegadores para HTML/web, editores gráficos para PNG/SVG/PSD como PhotoCraft y VectorCraft, suites de vídeo para MP4/clips como FilmCraft y Drift, o editores de código).
  - Soporte multiplataforma para ejecutar con la aplicación por defecto del SO (`open::that`) o con un binario específico.
- **Renderizado de HTML con recursos locales (`FileViewer.svelte`):**
  - Soporte de `assetProtocol` en Tauri 2 y resolución con `convertFileSrc` para que los archivos HTML carguen correctamente imágenes, fuentes y hojas de estilo locales relativas.
- **Auto-refresco reactivo del explorador de archivos y revisión:**
  - Emisión de evento `app://workspace-changed` cada vez que el agente o el usuario modifica archivos (`write`, `edit`, `shell`, `git_discard`).
  - `FileExplorer` y `ReviewPanel` refrescan su árbol y estado de cambios automáticamente sin requerir recarga manual.
- **Deslizamiento animado de salida y entrada del Asistente:**
  - Animación fluida de deslizamiento lateral (`@keyframes assistantSlideIn` y `assistantSlideOut` con curvas cúbicas tipo SO).
  - El toggle de tray o atajo global emite `assistant://close-requested` para permitir la animación de salida antes de ocultar la ventana nativa.
  - Al recuperar foco (`focus`), la animación de entrada se re-ejecuta de forma consistente.

### Changed

- **Mapeo estándar de atajos de teclado:**
  - Eliminados todos los atajos `Ctrl+Alt+*` que colisionaban con el gestor de ventanas del sistema operativo en Linux.
  - **`Ctrl+B`** (Cmd+B): Alternar visibilidad de la barra lateral izquierda (Sidebar).
  - **`Ctrl+E`** (Cmd+E): Alternar visibilidad del panel lateral de trabajo (Lanzador).
  - **`Ctrl+P`** (Cmd+P): Abrir visor/explorador de archivos en el panel de trabajo.
  - **`Ctrl+J`** (Cmd+J): Abrir/conmutar la terminal integrada en el panel.
  - **`Ctrl+I`** (Cmd+I): Abrir pestaña de navegador web en el panel.
  - **`Ctrl+G`** (Cmd+G): Abrir pestaña de revisión de cambios (Git diff) en el panel.
- **Atajos contextuales en Terminal (`TerminalPanel.svelte`):**
  - **`Ctrl+Shift+T`** (Cmd+T en Mac): Nueva pestaña de terminal (estándar de emuladores de terminal).
  - **`Ctrl+Shift+W`** / **`Ctrl+Shift+D`** (Cmd+W en Mac): Cerrar la terminal actual.
  - **`Ctrl+Shift+C`** / **`Ctrl+Shift+V`** / **`Ctrl+Shift+A`**: Copiar, pegar y seleccionar todo en xterm.
- **Reordenación del panel Lanzador:**
  - Nuevo orden prioritario: 1º Abrir archivo, 2º Terminal, 3º Navegador, 4º Revisión.
- **Selector de modo de Agente en sesión de chat:**
  - Badge interactivo `Build` / `Plan` en el encabezado de la conversación.
  - En modo `Plan`, el prompt del sistema bloquea explícitamente escrituras y ejecuciones para diseñar planes antes de actuar.

### Fixed

- **Cierre inesperado de pestaña de terminal:** Se corrigió para que el shell no intercepte atajos de forma destructiva y se cierre adecuadamente al salir del proceso (`exit`).
- **Problema de teclas muertas en Linux:** Se eliminó el atajo con acento grave / backtick (`Ctrl+\``) que fallaba en teclados en español y distribuciones Linux.
- **Internacionalización (i18n):** Actualizadas y sincronizadas 518 cadenas en español (`es.po`) e inglés (`en.po`) sin entradas fuzzy ni faltantes.

## [0.0.2] - 2026-11-09

Icon, clipboard, chat queue, provider keys and a real diff.

### Added

- **Pasted images and files.** A screenshot pasted with `Ctrl+V` becomes an attachment and
  travels to the model as an image, on both OpenAI-compatible and Anthropic providers. It
  works on X11 and Wayland: the window's own clipboard is read directly, with `wl-paste`
  and `xclip` as fallbacks. Images are scaled to 1600 px and re-encoded when needed.
- **Messages written while the model answers** are queued and sent when the current answer
  ends, instead of locking the message box. The queue is visible and each entry can be
  removed before it goes out.
- **16 ready-made providers** in the provider form (NVIDIA NIM, Groq, Mistral, DeepSeek,
  xAI, Together, Fireworks, Cerebras, Perplexity, Cohere, LM Studio…), which fill in the URL
  and the model list. The API key is kept when a provider is edited.

### Fixed

- **Stop** did nothing while waiting for the provider. It now cancels at once, and the
  backend stops waiting for the response.
- The chat **scrolled the whole window** instead of its own container, and did so while the
  user was reading earlier messages.
- **Permissions** granted "for this conversation" were lost when the app was closed. They
  are now saved in `permissions.json` and reloaded on start.
- **The diff of a changed file** is now computed by the backend from the file before and
  after, with line numbers and green/red rows. It is shown as soon as the tool group is
  expanded, and a newly created file is no longer shown without changes.

### Changed

- The app icon is a new concept: three traces converging on a single port, on the graphite
  tile, still in `#1E5FFF`. Regenerated for every platform, and the sidebar mark with it.

## [0.0.1] - 2026-11-04

First public pre-alpha. Linux is the only platform it has been tried on.

### Added

**Chat and agent**
- Streaming chat with Markdown rendering. Models that expose their reasoning show it in a
  folded block.
- An agent that can read, search, write and edit files and run shell commands, with a
  `build` mode (full access) and a read-only `plan` mode.
- Each turn is shown in order, with the tool calls grouped, timed and, for edits, shown as a
  diff. Tool calls and results are remembered in later turns of the same conversation.
- Permissions: writing and editing are limited to the project folder; shell commands ask
  for confirmation and run inside a `bubblewrap` sandbox when it is installed; reading
  outside the project needs permission unless you wrote the path yourself.
- Context meter in the header (a ring with the share of the model's context window, the
  tokens in use and the estimated cost).
- Context compaction: the conversation is summarized when it passes 85% of the model's
  context window, or on demand with `/compact`.
- Conversation titles generated by the model after the first answer, and regenerable.
- `/` commands and `@` file mentions in the message box; file attachments.
- Provider errors are shown in the conversation instead of being swallowed.

**Models and providers**
- Providers: OpenAI, Anthropic, OpenRouter, Ollama, Google, Azure and custom
  OpenAI-compatible endpoints, with a connection test that also loads the model list.
- Model picker with search, favorites, recents, a default model, a "Free" badge and the
  context window of each model.
- Model names, context windows and prices from the [models.dev](https://models.dev)
  catalogue, cached on disk, plus a view to switch each provider's models on or off.
- Routes with glob patterns, ordered provider chains and fallback.

**Workspace**
- Projects, with an icon detected from the project folder or chosen by hand.
- Conversation tabs with a working indicator, restored when the app is reopened.
- Search across conversations (`Ctrl+K`), including the text of the messages.
- Side panel with an integrated terminal, file explorer and viewer, git review and a
  browser. MCP servers and detection of skills, plugins and LSP servers.
- Usage dashboard with tokens, latency and estimated cost per provider and model.

**Interface**
- English and Spanish.
- Dark, light or system theme.
- Inter and JetBrains Mono bundled; any font installed on the system can be chosen for the
  interface and for code, with weight and size controls.

**Gateway**
- Embedded HTTP server (default `127.0.0.1:20128`) with optional bearer token and CORS.
  See *Known issues* for what it supports so far.

**Packaging**
- `.deb` (Debian, Ubuntu) and `.rpm` (Fedora) packages with AppStream metadata, a man page
  and license files.

### Known issues

- The external gateway is incomplete. `/health`, `/v1/models`, `/api/tags` and streaming
  `/v1/chat/completions` work; non-streaming chat, `/v1/messages`, `/api/chat`,
  `/api/generate`, `/api/show`, `/api/pull`, `/api/delete`, `/v1/completions` and
  `/v1/embeddings` answer `501 Not Implemented`.
- The chat handles one answer at a time. Switching conversation while one is being
  answered is not reliable.
- Compaction summarizes the whole conversation; it does not keep the latest messages
  verbatim. A conversation that already exceeds the model's limit cannot be summarized.
- The Anthropic provider with tools has only been tested against recorded formats, not
  against the live API.
- API keys are stored as plain text in `~/.config/ianox/config.json`.
- Inside the command sandbox `/tmp` stays writable, and tools that need a cache outside the
  project (such as `cargo` or `npm`) may fail because everything else is read-only.
- Without `bubblewrap` (or on macOS and Windows) commands run without a sandbox; they still
  ask for confirmation.
- macOS and Windows builds are untested.

[Unreleased]: https://github.com/Subdes/ianox/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/Subdes/ianox/releases/tag/v0.0.1
