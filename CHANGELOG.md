# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project follows [Semantic Versioning](https://semver.org/). Versions below `0.1.0` are
**pre-alpha**: anything may change or break between releases.

## [Unreleased]

## [0.0.9] - 2026-11-24

### Added

- **Copy button on code blocks (`Markdown.svelte`):** each `<pre><code>` block in the chat shows a **copy icon** (two overlapping squares) in the top-right corner, **always visible**, that copies the text to the clipboard and briefly shows a checkmark for 1.5 s.
- **Ask and Code modes in the Floating Assistant:** Ask keeps a global conversation; Code lets the user choose a project. Each mode/project has its own conversation history.

### Changed

- **Version:** bump to `0.0.9` in `package.json`, `tauri.conf.json` and `Cargo.toml`.
- **Soplos themes:** selection, focus and highlight colors now use the theme accent instead of hard-coded blue, preserving blue in ianox themes and using orange in Soplos themes.

### Fixed

- **Chat queue:** queued messages are drained serially when a response finishes, fails or is stopped, instead of remaining reserved in the composer.
- **Voice input:** dictation stops when returning to the text field, preventing speech recognition from overwriting manually typed text.
- **Floating Assistant queue:** sent text is cleared from the composer; subsequent messages can be queued during a response, reviewed and cancelled.
- **Floating Assistant keyboard (`assistant.rs`, `WidgetAssistant.svelte`):** the window uses `Utility` instead of `Dock` (compositors may deny keyboard focus to docks), and no longer remounts the widget on each focus event. The close-request event now matches between the native shortcut and the panel, restoring Ctrl+Shift+Space show/hide behavior.
- **Floating Assistant composer:** vertically aligns the text and action button within the input.
- **Floating Assistant header and models (`WidgetAssistant.svelte`):** loads models and enabled providers on mount, panel show and menu toggle, restoring default/last-used model preferences and wiring live streaming and permission events; replaces the nonexistent `external-link` icon with `external`; uses the model picker with readable names, free-model badges, favorites and text filtering. The provider and model selectors share the same button styling, and `Escape` closes the picker.
- **Model picker navigation:** its add-provider action opens the main window on the Providers page.
- **Floating Assistant width and layout (`tauri.conf.json`, `assistant.rs`, `WidgetAssistant.svelte`):** maintains the 420 px panel width and prevents the project/model selectors from overlapping in Code mode.
- **Floating Assistant positioning (`assistant.rs`):** uses consistent GTK units on Linux, migrates the KWin placement rule, and respects desktop panels. Windows and macOS use the monitor work area so the panel avoids the taskbar, Dock and menu bar.
- **`WidgetAssistant.svelte`:** fixes TypeScript/Svelte compilation errors, including the missing `requestId` event field, the `Markdown` prop name and the missing keyed block.

## [0.0.7] - 2026-11-15

Dynamic voice input in Chat and Assistant, conversion of the Assistant to Dock/Widget type with immunity against tiling compositors (KWin/Plasma 6, Pop Shell), reactive synchronization of the tray menu (Show/Hide) and exclusion of symlinks in Vite.

### Added

- **Dynamic voice input (`Chat.svelte`, `WidgetAssistant.svelte`, `Icon.svelte`):**
  - Interactive button with microphone icon (`mic`) when the text field is empty.
  - On click, starts voice dictation via Web Speech API transcribing in real time to the text area with pulsing animation.
  - As soon as the field contains written or dictated text, it automatically mutates to the send arrow (`arrow-up`).
- **Floating Assistant as Desktop Widget type Dock (`assistant.rs`):**
  - Surface configured with `WindowTypeHint::Dock` and `set_accept_focus(true)`, exempting it from tiling algorithms and forced center placement.
  - Automatic injection of persistent rule in `kwinrulesrc` (`placement=0`, borderless, always visible on top) and KWin Wayland script for right-side anchoring.
- **Reactive synchronization of tray context menu (`tray.rs`, `main.rs`):**
  - Real visibility check (`visible && !minimized`), alternating precisely between "Show ianox" and "Hide ianox".
  - Dynamic recalculation on each tray interaction and on each window event.

### Fixed

- **Exclusion of symlinks in Vite watcher (`vite.config.ts`):**
  - Ignored `.flatpak-builder` and `dist-packages` in Vite watcher to prevent `ELOOP` recursion errors.

## [0.0.6] - 2026-11-10

Floating Assistant positioning fix in tiling managers (persistent right-side anchoring) and dynamic version unification across the packaging pipeline (.deb, .rpm, .AppImage, .flatpak, .msi, .dmg).

### Fixed

- **Positioning and right-side anchoring in Tiling (`assistant.rs`):**
  - Explicit deactivation of automatic window manager centering (`gtk::WindowPosition::None`, gravity `NorthEast`).
  - Direct low-level positioning with `gtk_win.move_()` and `resize()`.
  - Post-map re-anchoring with 60ms verification to prevent KWin or Mutter forcing the window to center during entry animation.
- **Version unification in packaging scripts:**
  - `bundle-msi.sh`, `bundle-dmg.sh`, `bundle-flatpak.sh` and `collect-packages.sh` now extract the version dynamically from `package.json`, ensuring all artifacts carry the exact release version.

## [0.0.5] - 2026-11-10

System notifications when the window is not focused and direct update searcher from GitHub Releases with download and installation by platform.

### Added

- **Smart system notifications (`notify.rs`, `notify_if_unfocused`):**
  - Tauri command that verifies the real state of the main window (`is_focused()`, `is_visible()`, `is_minimized()`).
  - If the window is in the foreground and focused, it does not interrupt the user.
  - If the window is hidden, minimized in the tray or in the background, it launches a native system notification (Linux `notify-send`, macOS `osascript`, Windows PowerShell notification).
  - Integrated at the end of streaming responses both in the main Chat view and in the Floating Assistant.
- **Automatic update searcher and downloader (`updates.rs`):**
  - Native checker that queries `https://api.github.com/repos/Subdes/ianox/releases/latest`.
  - Smart detection of the user's operating system and execution environment (Flatpak, AppImage, Debian/Ubuntu `.deb`, Fedora/RHEL `.rpm`, Windows `.msi` or macOS `.dmg`).
  - Package download by streaming with live percentage events (`update://progress`).
  - Opening/execution of the installer at the end of the download.
  - Visual section in `Settings > General` with "Check for updates" button, progress bar and status.

### Fixed

- **Floating Assistant in tiling window managers (`assistant.rs`, `tauri.conf.json`):**
  - Window configured with type `WindowTypeHint::Utility`, role `"ianox-assistant"`, `keep_above(true)` and `resizable` disabled to prevent compositors like KWin Tiling, Pop Shell, PaperWM or Forge from capturing it and forcing it to full screen or inside the tiling. Maintains its geometry and right-side anchoring as an omnipresent floating widget.

## [0.0.4] - 2026-11-10

Minimized startup fix in tray, floating assistant with streaming and provider selector, scroll and flex layout fixes in chat, and multiplatform packaging scripts (.msi, .dmg, .flatpak).

### Added

- **Provider Selector in Floating Assistant (`WidgetAssistant.svelte`):**
  - Dynamic selector of enabled providers in the options bar next to the model selector.
  - Reactive model filtering according to the selected provider and sending the `providerId` parameter to `chat_stream`.
- **Multi-distribution and multiplatform packaging support:**
  - WiX script and manifest (`packaging/windows/ianox.wxs`, `scripts/bundle-msi.sh`) compatible with `wixl` to generate native `.msi` installer on Windows.
  - `scripts/bundle-dmg.sh` script with dual `.dmg` and `.app.tar.gz` generation for macOS.
  - Script and manifest `scripts/bundle-flatpak.sh` with GNOME 51 runtime and high-resolution icons.
  - Unifier script `scripts/collect-packages.sh` and master command `npm run build:all`.

### Fixed

- **Minimized startup on autostart (`main.rs`):**
  - Reading `--minimized` and `--hidden` arguments at startup. If the system tray is initialized and available, the main window remains hidden, avoiding invasive openings at login.
- **Floating Assistant with Streaming and Cancellation (`WidgetAssistant.svelte`):**
  - Full migration from stub command `chat_completion` to reactive engine `chat_stream` with events `chat://delta`, `chat://done` and `chat://error`.
  - Immediate cancellation button (`handleStop`) connected to `chat_cancel` and proper cleanup of listeners on component unmount.
- **Double scroll and message compression in Chat (`styles.css`, `Chat.svelte`):**
  - Removed double scroll container via `overflow: hidden` in `.page-content.chat-content`.
  - Fixed collapse and crushing of messages in flex containers with `flex-shrink: 0` and `min-height: 0` in `.app-main` and `.chat-scroll`.
  - Replaced `overflow: hidden` with `clip-path: inset(0 round var(--radius-md))` in `.workspace-chat` to avoid flickers and white repaints under WebKitGTK.
  - Anti-bounce on auto-scroll with flag `_programmaticScroll` so programmatic downward movement does not erroneously disable anchoring (`stickToBottom`).

Enriched tray, autostart, multiplatform terminal with xterm, reactive workspace refresh, smart application detection and redesign of standard shortcuts.

### Added

- **Enriched system tray:**
  - Dynamic contextual menu with Show/Hide toggle, direct access to Floating Assistant with its keyboard accelerator.
  - Live API server status indicator (🟢 Active / 🔴 Stopped).
  - Interactive checkbox to configure autostart with the operating system.
  - Unified click handling: left click toggles main window visibility and right click displays the contextual menu.
- **Multiplatform autostart (`autostart.rs`):**
  - Native support for autostart on Linux (`~/.config/autostart/ianox.desktop`), macOS (`LaunchAgents`) and Windows (user registry).
  - Tauri commands `get_autostart_status` and `set_autostart`.
- **Contextual and smart application detection ("Open in..."):**
  - Backend analyzes file extension and type to offer the best tools installed on the system (browsers for HTML/web, graphic editors for PNG/SVG/PSD such as PhotoCraft and VectorCraft, video suites for MP4/clips such as FilmCraft and Drift, or code editors).
  - Multiplatform support to run with the OS default application (`open::that`) or with a specific binary.
- **HTML rendering with local resources (`FileViewer.svelte`):**
  - Support for `assetProtocol` in Tauri 2 and resolution with `convertFileSrc` so HTML files load images, fonts and local stylesheets correctly.
- **Reactive auto-refresh of file explorer and review:**
  - Emission of event `app://workspace-changed` whenever the agent or user modifies files (`write`, `edit`, `shell`, `git_discard`).
  - `FileExplorer` and `ReviewPanel` refresh their tree and change status automatically without requiring manual reload.
- **Animated slide-out and slide-in of the Assistant:**
  - Smooth lateral slide animation (`@keyframes assistantSlideIn` and `assistantSlideOut` with SO-type cubic curves).
  - Tray toggle or global shortcut emits `assistant://close-requested` to allow slide-out animation before hiding the native window.
  - On focus regain (`focus`), the entry animation is re-executed consistently.

### Changed

- **Standard keyboard shortcut mapping:**
  - Removed all `Ctrl+Alt+*` shortcuts that collided with the operating system window manager on Linux.
  - **`Ctrl+B`** (Cmd+B): Toggle left sidebar visibility.
  - **`Ctrl+E`** (Cmd+E): Toggle right work panel (Launcher) visibility.
  - **`Ctrl+P`** (Cmd+P): Open file viewer/explorer in the work panel.
  - **`Ctrl+J`** (Cmd+J): Open/toggle integrated terminal in the panel.
  - **`Ctrl+I`** (Cmd+I): Open web browser tab in the panel.
  - **`Ctrl+G`** (Cmd+G): Open git changes review tab in the panel.
- **Contextual shortcuts in Terminal (`TerminalPanel.svelte`):**
  - **`Ctrl+Shift+T`** (Cmd+T on Mac): New terminal tab (terminal emulator standard).
  - **`Ctrl+Shift+W`** / **`Ctrl+Shift+D`** (Cmd+W on Mac): Close current terminal.
  - **`Ctrl+Shift+C`** / **`Ctrl+Shift+V`** / **`Ctrl+Shift+A`**: Copy, paste and select all in xterm.
- **Launcher panel reordering:**
  - New priority order: 1º Open file, 2º Terminal, 3º Browser, 4º Review.
- **Agent mode selector in chat session:**
  - Interactive `Build` / `Plan` badge in conversation header.
  - In `Plan` mode, the system prompt explicitly blocks writes and executions to design plans before acting.

### Fixed

- **Unexpected terminal tab closure:** Fixed so the shell does not intercept shortcuts destructively and closes properly when exiting the process (`exit`).
- **Dead key issue on Linux:** Removed the grave accent / backtick shortcut (`Ctrl+``) that failed on Spanish keyboards and Linux distributions.
- **Internationalization (i18n):** Updated and synchronized 518 strings in Spanish (`es.po`) and English (`en.po`) without fuzzy or missing entries.

## [0.0.2] - 2026-11-09

Icon, clipboard, chat queue, provider keys and a real diff.

### Added

- **Pasted images and files.** A screenshot pasted with `Ctrl+V` becomes an attachment and travels to the model as an image, on both OpenAI-compatible and Anthropic providers. It works on X11 and Wayland: the window's own clipboard is read directly, with `wl-paste` and `xclip` as fallbacks. Images are scaled to 1600 px and re-encoded when needed.
- **Messages written while the model answers** are queued and sent when the current answer ends, instead of locking the message box. The queue is visible and each entry can be removed before it goes out.
- **16 ready-made providers** in the provider form (NVIDIA NIM, Groq, Mistral, DeepSeek, xAI, Together, Fireworks, Cerebras, Perplexity, Cohere, LM Studio…), which fill in the URL and the model list. The API key is kept when a provider is edited.

### Fixed

- **Stop** did nothing while waiting for the provider. It now cancels at once, and the backend stops waiting for the response.
- The chat **scrolled the whole window** instead of its own container, and did so while the user was reading earlier messages.
- **Permissions** granted "for this conversation" were lost when the app was closed. They are now saved in `permissions.json` and reloaded on start.
- **The diff of a changed file** is now computed by the backend from the file before and after, with line numbers and green/red rows. It is shown as soon as the tool group is expanded, and a newly created file is no longer shown without changes.

### Changed

- The app icon is a new concept: three traces converging on a single port, on the graphite tile, still in `#1E5FFF`. Regenerated for every platform, and the sidebar mark with it.

## [0.0.1] - 2026-11-04

First public pre-alpha. Linux is the only platform it has been tried on.

### Added

**Chat and agent**
- Streaming chat with Markdown rendering. Models that expose their reasoning show it in a folded block.
- An agent that can read, search, write and edit files and run shell commands, with a `build` mode (full access) and a read-only `plan` mode.
- Each turn is shown in order, with the tool calls grouped, timed and, for edits, shown as a diff. Tool calls and results are remembered in later turns of the same conversation.
- Permissions: writing and editing are limited to the project folder; shell commands ask for confirmation and run inside a `bubblewrap` sandbox when it is installed; reading outside the project needs permission unless you wrote the path yourself.
- Context meter in the header (a ring with the share of the model's context window, the tokens in use and the estimated cost).
- Context compaction: the conversation is summarized when it passes 85% of the model's context window, or on demand with `/compact`.
- Conversation titles generated by the model after the first answer, and regenerable.
- `/` commands and `@` file mentions in the message box; file attachments.
- Provider errors are shown in the conversation instead of being swallowed.

**Models and providers**
- Providers: OpenAI, Anthropic, OpenRouter, Ollama, Google, Azure and custom OpenAI-compatible endpoints, with a connection test that also loads the model list.
- Model picker with search, favorites, recents, a default model, a "Free" badge and the context window of each model.
- Model names, context windows and prices from the [models.dev](https://models.dev) catalogue, cached on disk, plus a view to switch each provider's models on or off.
- Routes with glob patterns, ordered provider chains and fallback.

**Workspace**
- Projects, with an icon detected from the project folder or chosen by hand.
- Conversation tabs with a working indicator, restored when the app is reopened.
- Search across conversations (`Ctrl+K`), including the text of the messages.
- Side panel with an integrated terminal, file explorer and viewer, git review and a browser. MCP servers and detection of skills, plugins and LSP servers.
- Usage dashboard with tokens, latency and estimated cost per provider and model.

**Interface**
- English and Spanish.
- Dark, light or system theme.
- Inter and JetBrains Mono bundled; any font installed on the system can be chosen for the interface and for code, with weight and size controls.

**Gateway**
- Embedded HTTP server (default `127.0.0.1:20128`) with optional bearer token and CORS. See *Known issues* for what it supports so far.

**Packaging**
- `.deb` (Debian, Ubuntu) and `.rpm` (Fedora) packages with AppStream metadata, a man page and license files.

### Known issues

- The external gateway is incomplete. `/health`, `/v1/models`, `/api/tags` and streaming `/v1/chat/completions` work; non-streaming chat, `/v1/messages`, `/api/chat`, `/api/generate`, `/api/show`, `/api/pull`, `/api/delete`, `/v1/completions` and `/v1/embeddings` answer `501 Not Implemented`.
- The chat handles one answer at a time. Switching conversation while one is being answered is not reliable.
- Compaction summarizes the whole conversation; it does not keep the latest messages verbatim. A conversation that already exceeds the model's limit cannot be summarized.
- The Anthropic provider with tools has only been tested against recorded formats, not against the live API.
- API keys are stored as plain text in `~/.config/ianox/config.json`.
- Inside the command sandbox `/tmp` stays writable, and tools that need a cache outside the project (such as `cargo` or `npm`) may fail because everything else is read-only.
- Without `bubblewrap` (or on macOS and Windows) commands run without a sandbox; they still ask for confirmation.
- macOS and Windows builds are untested.

[Unreleased]: https://github.com/Subdes/ianox/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/Subdes/ianox/releases/tag/v0.0.1