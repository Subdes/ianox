//! Main window: persistent state (position/size).
//!
//! Rounded corners, the title bar and transparency are handled
//! in the frontend (CSS + TitleBar component). Here we only save/restore the
//! estado de la ventana.

use tauri::{WebviewWindow, Manager, Position, Size, PhysicalPosition, PhysicalSize};
use std::fs;
use std::path::PathBuf;

const STATE_FILE: &str = "window_state.json";

/// The window has a real alpha channel (so its rounded corners are see-through).
static ALPHA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the frontend may draw rounded, see-through corners.
#[tauri::command]
pub fn window_alpha() -> bool {
    // Other platforms composite windows themselves.
    cfg!(not(target_os = "linux")) || ALPHA.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct WindowState {
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
    maximized: bool,
}

pub fn setup(window: &WebviewWindow) -> tauri::Result<()> {
    #[cfg(target_os = "linux")]
    enable_transparency(window)?;

    restore_state(window);
    Ok(())
}

/// Without an RGBA visual, WebKitGTK paints an opaque background and the rounded corners
/// from CSS are not visible (they look square). This enables the real alpha channel.
#[cfg(target_os = "linux")]
fn enable_transparency(window: &WebviewWindow) -> tauri::Result<()> {
    use gtk::prelude::*;

    let gtk_window = window.gtk_window()?;
    // Without a compositing manager an alpha channel is painted as solid black corners.
    // The frontend asks `window_alpha` and then draws a plain, square, opaque window instead.
    if let Some(screen) = gtk::gdk::Screen::default() {
        if screen.is_composited() {
            if let Some(visual) = screen.rgba_visual() {
                gtk_window.set_visual(Some(&visual));
                gtk_window.set_app_paintable(true);
                ALPHA.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
    Ok(())
}

/// Saves window position/size
pub fn save_state(app: &tauri::AppHandle) {
    if let Some(window) = app.webview_windows().get("main") {
        let state = WindowState {
            x: window.outer_position().ok().map(|p| p.x as f64),
            y: window.outer_position().ok().map(|p| p.y as f64),
            width: window.outer_size().ok().map(|s| s.width as f64),
            height: window.outer_size().ok().map(|s| s.height as f64),
            maximized: window.is_maximized().unwrap_or(false),
        };
        if let Ok(json) = serde_json::to_string(&state) {
            if let Some(path) = state_path(app) {
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(path, json);
            }
        }
    }
}

/// Restaura estado guardado
fn restore_state(window: &WebviewWindow) {
    let app = window.app_handle();
    if let Some(path) = state_path(app) {
        if let Ok(json) = fs::read_to_string(path) {
            if let Ok(state) = serde_json::from_str::<WindowState>(&json) {
                if let (Some(x), Some(y)) = (state.x, state.y) {
                    let _ = window.set_position(Position::Physical(PhysicalPosition { x: x as i32, y: y as i32 }));
                }
                if let (Some(w), Some(h)) = (state.width, state.height) {
                    let _ = window.set_size(Size::Physical(PhysicalSize { width: w as u32, height: h as u32 }));
                }
                if state.maximized {
                    let _ = window.maximize();
                }
            }
        }
    }
}

fn state_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|dir| dir.join(STATE_FILE))
}
