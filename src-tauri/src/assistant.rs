//! ianox — Asistente lateral derecho (Widget omnipresente)
//!
//! Multiplataforma nativa:
//! - Linux (X11 y Wayland): consulta `gdk::Monitor::workarea()` directamente
//!   para obtener el área útil real (descontando panel superior de GNOME, dock, etc.).
//! - macOS y Windows: posicionamiento en monitor activo con fallback dinámico.
//! - Atajo global del sistema y toggling instantáneo.

use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize,
    Runtime, WebviewWindow,
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

/// Posiciona y muestra la ventana del asistente ajustada al área disponible del SO.
fn position_and_show<R: Runtime>(window: &WebviewWindow<R>) {
    #[cfg(target_os = "linux")]
    {
        if let Some((x, y, w, h)) = get_workarea_linux(window) {
            let width = DEFAULT_WIDTH.min(w);
            let pos_x = (x + w) - width - MARGIN_RIGHT;
            let pos_y = y;

            let _ = window.set_size(LogicalSize::new(width, h));
            let _ = window.set_position(LogicalPosition::new(pos_x, pos_y));
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
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
