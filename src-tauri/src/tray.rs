//! Bandeja del sistema (system tray) — menú enriquecido con estado dinámico

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, Runtime};
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem};

/// El icono de tray pudo crearse (hay entornos sin bandeja, como algunas VMs).
static AVAILABLE: AtomicBool = AtomicBool::new(false);

pub fn available() -> bool {
    AVAILABLE.load(Ordering::Relaxed)
}

/// Estado interno del tray para actualización dinámica de etiquetas y checkboxes.
pub struct TrayState<R: Runtime> {
    pub show_item: tauri::menu::MenuItem<R>,
    pub api_item: tauri::menu::MenuItem<R>,
    pub autostart_item: tauri::menu::CheckMenuItem<R>,
}

/// Idioma configurado por el usuario («en» o «es»); inglés si no se puede leer.
fn configured_language() -> String {
    let base = std::env::var("XDG_CONFIG_HOME")
        .or_else(|_| std::env::var("HOME").map(|h| format!("{h}/.config")))
        .unwrap_or_default();
    std::fs::read_to_string(std::path::Path::new(&base).join("ianox").join("config.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| json.pointer("/ui/language").and_then(|v| v.as_str().map(str::to_string)))
        .unwrap_or_else(|| "en".into())
}

/// Estado del servidor API leído sincrónicamente desde el AppState.
/// Devuelve `None` si el servidor no está activo, `Some(addr)` si está activo.
fn read_server_addr<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    let state = app.state::<crate::config::AppState>();
    tauri::async_runtime::block_on(async {
        let guard = state.server_handle.lock().await;
        guard.as_ref().map(|h| h.addr.to_string())
    })
}

/// Actualiza dinámicamente el texto de mostrar/ocultar ventana según su estado de visibilidad.
pub fn update_show_status<R: Runtime>(app: &AppHandle<R>) {
    if let Some(tray_state) = app.try_state::<TrayState<R>>() {
        let lang = configured_language();
        if let Some(window) = app.get_webview_window("main") {
            let visible = window.is_visible().unwrap_or(false);
            let text = if visible {
                crate::i18n::t(&lang, "Hide ianox")
            } else {
                crate::i18n::t(&lang, "Show ianox")
            };
            let _ = tray_state.show_item.set_text(text);
        }
    }
}

/// Actualiza dinámicamente el indicador de estado del servidor API en la bandeja.
pub fn update_api_status<R: Runtime>(app: &AppHandle<R>) {
    if let Some(tray_state) = app.try_state::<TrayState<R>>() {
        let server_addr = read_server_addr(app);
        let api_label = match &server_addr {
            Some(addr) => format!("🟢 API activa ({})", addr),
            None       => "🔴 API desactivada".to_string(),
        };
        let _ = tray_state.api_item.set_text(api_label);
    }
}

/// Sincroniza el checkbox de autostart con el estado real del sistema.
pub fn update_autostart_status<R: Runtime>(app: &AppHandle<R>) {
    if let Some(tray_state) = app.try_state::<TrayState<R>>() {
        let active = crate::autostart::get_autostart_status();
        let _ = tray_state.autostart_item.set_checked(active);
    }
}

pub fn init<R: Runtime>(app: AppHandle<R>) -> tauri::Result<()> {
    let lang = configured_language();

    // ── Grupo 1: navegación de ventanas ─────────────────────────────────────
    let show_item = MenuItemBuilder::new(crate::i18n::t(&lang, "Show ianox"))
        .id("show")
        .accelerator("Ctrl+Shift+I")
        .build(&app)?;

    let assistant_item = MenuItemBuilder::new(crate::i18n::t(&lang, "Toggle Assistant Widget"))
        .id("assistant")
        .accelerator(crate::assistant::default_shortcut())
        .build(&app)?;

    let sep1 = PredefinedMenuItem::separator(&app)?;

    // ── Grupo 2: estado del servidor API ────────────────────────────────────
    let server_addr = read_server_addr(&app);
    let api_label = match &server_addr {
        Some(addr) => format!("🟢 API activa ({})", addr),
        None       => "🔴 API desactivada".to_string(),
    };
    let api_item = MenuItemBuilder::new(&api_label)
        .id("api_status")
        .enabled(false) // puramente informativo
        .build(&app)?;

    // ── Grupo 3: autostart ──────────────────────────────────────────────────
    let autostart_active = crate::autostart::get_autostart_status();
    let autostart_item = CheckMenuItemBuilder::new(crate::i18n::t(&lang, "Start with system"))
        .id("autostart")
        .checked(autostart_active)
        .build(&app)?;

    let sep2 = PredefinedMenuItem::separator(&app)?;

    // ── Grupo 4: salir ──────────────────────────────────────────────────────
    let quit_item = MenuItemBuilder::new(crate::i18n::t(&lang, "Quit"))
        .id("quit")
        .accelerator("Ctrl+Q")
        .build(&app)?;

    let menu = MenuBuilder::new(&app)
        .items(&[
            &show_item,
            &assistant_item,
            &sep1,
            &api_item,
            &autostart_item,
            &sep2,
            &quit_item,
        ])
        .build()?;

    let tray = tauri::tray::TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().unwrap())
        .menu(&menu)
        // Clic derecho → menú contextual; clic izquierdo togglea la ventana principal.
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => toggle_main_window(app),
            "assistant" => crate::assistant::toggle_assistant(app),
            "autostart" => {
                let current = crate::autostart::get_autostart_status();
                let _ = crate::autostart::set_autostart(!current);
                update_autostart_status(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click { button, .. } = event {
                if button == tauri::tray::MouseButton::Left {
                    toggle_main_window(tray.app_handle());
                }
            }
        })
        .build(&app)?;

    // Guardar handles para actualizaciones dinámicas posteriores
    app.manage(TrayState {
        show_item,
        api_item,
        autostart_item,
    });

    app.manage(tray);
    AVAILABLE.store(true, Ordering::Relaxed);
    Ok(())
}

/// Muestra u oculta la ventana principal según su estado actual y actualiza el texto del menú.
fn toggle_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let visible = window.is_visible().unwrap_or(false);
        if visible {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
            let _ = window.unminimize();
        }
        update_show_status(app);
    }
}
