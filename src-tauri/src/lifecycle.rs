//! Quitting versus keeping ianox running in the system tray.
//!
//! The window's close button and `Ctrl+Q` both ask the frontend what to do (or apply the
//! choice the user saved). Without a tray there is nothing to keep it in, so closing just quits.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager};

/// The interface is loaded and can answer a close request. Before that (or if it hangs) the
/// window closes normally, so the user can never get stuck with a window that will not close.
static FRONTEND_READY: AtomicBool = AtomicBool::new(false);
/// The user chose to quit: stop intercepting close requests.
static QUITTING: AtomicBool = AtomicBool::new(false);

/// Whether a close request of the main window should be handed to the frontend instead.
pub fn should_intercept_close() -> bool {
    FRONTEND_READY.load(Ordering::Relaxed) && !QUITTING.load(Ordering::Relaxed) && crate::tray::available()
}

#[tauri::command]
pub fn frontend_ready() {
    FRONTEND_READY.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub fn tray_available() -> bool {
    crate::tray::available()
}

/// Quits ianox for real (stops the gateway too).
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    QUITTING.store(true, Ordering::Relaxed);
    app.exit(0);
}

/// Hides the window and leaves ianox running in the tray.
#[tauri::command]
pub fn hide_to_tray(app: AppHandle) -> Result<(), String> {
    if !crate::tray::available() {
        return Err("There is no tray icon to keep ianox in".into());
    }
    match app.get_webview_window("main") {
        Some(window) => {
            window.hide().map_err(|e| e.to_string())?;
            crate::tray::update_show_status(&app);
            Ok(())
        }
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_requests_are_only_intercepted_when_it_is_safe() {
        // No frontend yet, no tray: the window just closes.
        assert!(!should_intercept_close());
        frontend_ready();
        // Still no tray in the test environment.
        assert!(!should_intercept_close());
    }
}
