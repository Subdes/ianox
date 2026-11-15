//! ianox — Asistente lateral derecho (Widget omnipresente)
//!
//! Multiplataforma nativa:
//! - Linux (X11 y Wayland): consulta `gdk::Monitor::workarea()` directamente
//!   para obtener el área útil real (descontando panel superior de GNOME, dock, etc.).
//! - macOS y Windows: posicionamiento en monitor activo con fallback dinámico.
//! - Atajo global del sistema y toggling instantáneo.

#[cfg(target_os = "linux")]
use tauri::{LogicalPosition, LogicalSize};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow,
};

pub const ASSISTANT_WINDOW: &str = "assistant";

const MARGIN_RIGHT: f64 = 0.0;
const DEFAULT_WIDTH: f64 = 420.0;

// ─── Comandos Tauri ───────────────────────────────────────────────────────────

/// Cierra u oculta el asistente.
#[tauri::command]
pub fn hide_assistant<R: Runtime>(app: AppHandle<R>) {
    if let Some(w) = app.get_webview_window(ASSISTANT_WINDOW) {
        let _ = w.hide();
    }
}

/// Muestra y enfoca la ventana principal de ianox.
#[tauri::command]
pub fn open_main_window<R: Runtime>(app: AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Reajusta el tamaño y posición de la ventana al área real disponible
/// si el frontend detecta una geometría más restringida (ej. Windows/macOS).
#[tauri::command]
pub fn resize_assistant<R: Runtime>(app: AppHandle<R>, avail_height: u32, avail_top: i32) {
    let Some(window) = app.get_webview_window(ASSISTANT_WINDOW) else {
        return;
    };

    if avail_height == 0 {
        return;
    }

    let scale = window.scale_factor().unwrap_or(1.0);
    let phys_h = (avail_height as f64 * scale).round() as u32;
    let phys_w = (DEFAULT_WIDTH * scale).round() as u32;
    let phys_y = (avail_top as f64 * scale).round() as i32;

    let pos_x = window
        .current_monitor()
        .ok()
        .flatten()
        .map(|m| m.position().x + m.size().width as i32 - phys_w as i32)
        .unwrap_or_else(|| window.outer_position().map(|p| p.x).unwrap_or(0));

    let _ = window.set_size(PhysicalSize::new(phys_w, phys_h));
    let _ = window.set_position(PhysicalPosition::new(pos_x, phys_y));
}

// ─── Lógica interna de Posicionamiento ────────────────────────────────────────

#[cfg(target_os = "linux")]
fn get_workarea_linux<R: Runtime>(window: &WebviewWindow<R>) -> Option<(f64, f64, f64, f64)> {
    use gtk::gdk::prelude::MonitorExt;
    let display = gtk::gdk::Display::default()?;

    // Busca el monitor donde se encuentra la ventana o cursor, fallback al primario
    let monitor = if let Ok(pos) = window.outer_position() {
        display.monitor_at_point(pos.x, pos.y)
    } else {
        None
    }
    .or_else(|| display.primary_monitor())
    .or_else(|| display.monitor(0))?;

    let rect = monitor.workarea();
    Some((
        rect.x() as f64,
        rect.y() as f64,
        rect.width() as f64,
        rect.height() as f64,
    ))
}

#[cfg(target_os = "linux")]
fn apply_linux_window_hints<R: Runtime>(window: &WebviewWindow<R>) {
    use gtk::prelude::*;
    if let Ok(gtk_win) = window.gtk_window() {
        // En gestores tiling (KWin, Pop Shell, i3/sway, Bismuth, Polonium):
        // Dock indica al compositor y al script de tiling que se trata de un panel/widget lateral,
        // eximiéndolo incondicionalmente del auto-mosaico y del centrado forzado.
        gtk_win.set_type_hint(gtk::gdk::WindowTypeHint::Dock);
        gtk_win.set_accept_focus(true);
        gtk_win.set_role("ianox-assistant");
        gtk_win.set_keep_above(true);
        gtk_win.set_skip_taskbar_hint(true);
        gtk_win.set_skip_pager_hint(true);
        // Desactiva explícitamente cualquier centrado automático del gestor de ventanas
        gtk_win.set_position(gtk::WindowPosition::None);
        gtk_win.set_gravity(gtk::gdk::Gravity::NorthEast);
    }
}

#[cfg(target_os = "linux")]
fn ensure_kwin_rules() {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            std::path::PathBuf::from(home).join(".config")
        });

    let kwinrules = config_dir.join("kwinrulesrc");
    if !config_dir.exists() {
        return;
    }

    let content = std::fs::read_to_string(&kwinrules).unwrap_or_default();
    if content.contains("[ianox-assistant]") {
        return;
    }

    let rule = r#"

[ianox-assistant]
Description=ianox Assistant Floating Widget
above=true
aboverule=2
noborder=true
noborderrule=2
placement=0
placementrule=2
skiptaskbar=true
skiptaskbarrule=2
skippager=true
skippagerrule=2
title=ianox Assistant
titlematch=1
wmclass=ianox
wmclassmatch=1
"#;

    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&kwinrules) {
        use std::io::Write;
        let _ = f.write_all(rule.as_bytes());
        // Pedir a KWin que recargue sus reglas en vivo
        let qdbus_bin = if std::path::Path::new("/usr/bin/qdbus6").exists() {
            "qdbus6"
        } else {
            "qdbus"
        };
        let _ = std::process::Command::new(qdbus_bin)
            .args(["org.kde.KWin", "/KWin", "reconfigure"])
            .status();
    }
}

#[cfg(target_os = "linux")]
fn reposition_kwin_wayland(width: f64) {
    let script_content = format!(
        r#"
        (function() {{
            var clients = workspace.windowList();
            for (var i = 0; i < clients.length; i++) {{
                var c = clients[i];
                var cls = (c.resourceClass || "").toLowerCase();
                var cap = (c.caption || "").toLowerCase();
                if ((cls.indexOf("ianox") !== -1 || cap.indexOf("ianox") !== -1) &&
                    (cap.indexOf("assistant") !== -1 || c.resourceName === "assistant")) {{
                    var area = workspace.clientArea(KWin.PlacementArea, c);
                    if (!area) area = workspace.clientArea(KWin.MaximizeArea, c);
                    try {{ c.tile = null; }} catch(e) {{}}
                    var targetW = {width};
                    var targetX = area.x + area.width - targetW;
                    var targetY = area.y;
                    var targetH = area.height;
                    if (c.frameGeometry !== undefined) {{
                        c.frameGeometry = {{
                            x: targetX,
                            y: targetY,
                            width: targetW,
                            height: targetH
                        }};
                    }} else {{
                        c.geometry = {{
                            x: targetX,
                            y: targetY,
                            width: targetW,
                            height: targetH
                        }};
                    }}
                    break;
                }}
            }}
        }})();
        "#
    );

    let temp_script = std::env::temp_dir().join("ianox_kwin_reposition.js");
    if std::fs::write(&temp_script, script_content).is_err() {
        return;
    }

    let script_path = temp_script.to_string_lossy().to_string();
    let qdbus_bin = if std::path::Path::new("/usr/bin/qdbus6").exists() {
        "qdbus6"
    } else {
        "qdbus"
    };

    let output = std::process::Command::new(qdbus_bin)
        .args(["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.loadScript", &script_path])
        .output();

    if let Ok(out) = output {
        let script_id = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !script_id.is_empty() {
            let service_path = format!("/Scripting/Script{script_id}");
            let _ = std::process::Command::new(qdbus_bin)
                .args(["org.kde.KWin", &service_path, "org.kde.kwin.Script.run"])
                .status();
            let _ = std::process::Command::new(qdbus_bin)
                .args(["org.kde.KWin", &service_path, "org.kde.kwin.Script.stop"])
                .status();
        }
    }
}

/// Posiciona y muestra la ventana del asistente ajustada al área disponible del SO.
fn position_and_show<R: Runtime>(window: &WebviewWindow<R>) {
    #[cfg(target_os = "linux")]
    {
        apply_linux_window_hints(window);
        if let Some((x, y, w, h)) = get_workarea_linux(window) {
            let width = DEFAULT_WIDTH.min(w);
            let pos_x = (x + w) - width - MARGIN_RIGHT;
            let pos_y = y;

            let _ = window.set_size(LogicalSize::new(width, h));
            let _ = window.set_position(LogicalPosition::new(pos_x, pos_y));

            // Forzar en GTK directamente antes y después de show
            if let Ok(gtk_win) = window.gtk_window() {
                use gtk::prelude::*;
                gtk_win.move_(pos_x as i32, pos_y as i32);
                gtk_win.resize(width as i32, h as i32);
            }

            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();

            // Re-aplicar posición tras mapeo para sobreescribir cualquier heurística de centrado de KWin/Mutter
            if let Ok(gtk_win) = window.gtk_window() {
                use gtk::prelude::*;
                gtk_win.move_(pos_x as i32, pos_y as i32);
            }
            let _ = window.set_position(LogicalPosition::new(pos_x, pos_y));

            // En Wayland bajo KDE Plasma 6 (Soplos Linux Tyson), KWin ignora gtk_window_move
            // y centra las ventanas flotantes; llamamos al scripting de KWin para posicionarla
            // exactamente en el borde derecho y sacarla del mosaico/tiling.
            reposition_kwin_wayland(width);

            let w_clone = window.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(60)).await;
                let _ = w_clone.set_position(LogicalPosition::new(pos_x, pos_y));
                #[cfg(target_os = "linux")]
                {
                    if let Ok(gtk_win) = w_clone.gtk_window() {
                        use gtk::prelude::*;
                        gtk_win.move_(pos_x as i32, pos_y as i32);
                    }
                    reposition_kwin_wayland(width);
                }
            });

            let _ = window.emit("assistant://shown", ());
            return;
        }
    }

    // Fallback estándar (Windows, macOS o sistemas sin GDK)
    if let Some(mon) = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
    {
        let pos = mon.position();
        let size = mon.size();
        let scale = mon.scale_factor();

        let phys_w = (DEFAULT_WIDTH * scale).round() as u32;
        let pos_x = pos.x + size.width as i32 - phys_w as i32 - (MARGIN_RIGHT * scale).round() as i32;
        let pos_y = pos.y;

        let _ = window.set_size(PhysicalSize::new(phys_w, size.height));
        let _ = window.set_position(PhysicalPosition::new(pos_x, pos_y));
    }

    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("assistant://shown", ());
}

/// Inicializa los hints nativos de la ventana del asistente en el arranque.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(ASSISTANT_WINDOW) {
        #[cfg(target_os = "linux")]
        {
            apply_linux_window_hints(&window);
            ensure_kwin_rules();
        }
    }
}

/// Alterna la visibilidad del asistente flotante.
pub fn toggle_assistant<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(ASSISTANT_WINDOW) else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        // Enviar evento de cierre para que anime la salida
        let _ = window.emit("assistant://close-requested", ());
    } else {
        position_and_show(&window);
    }
}

// ─── Atajo global ─────────────────────────────────────────────────────────────

pub fn default_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd+Shift+Space"
    } else {
        "Ctrl+Shift+Space"
    }
}

pub fn setup_global_shortcut(app: &AppHandle) -> tauri::Result<()> {
    use std::str::FromStr;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

    if let Ok(shortcut) = Shortcut::from_str(default_shortcut()) {
        let handle = app.clone();
        let _ = app.global_shortcut().on_shortcut(shortcut, move |_app, _sc, event| {
            if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                let h = handle.clone();
                let _ = handle.run_on_main_thread(move || {
                    toggle_assistant(&h);
                });
            }
        });
    }

    Ok(())
}
