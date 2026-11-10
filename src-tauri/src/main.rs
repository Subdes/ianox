// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! ianox — AI gateway, model manager and coding agent
//!
//! Fusiona lo mejor de:
//! - Gateway: AI router with fallback and key management
//! - Ollama: gestor de modelos locales
//! - Server: servidor embebido con UI propia
//! - Agent: coding agent with terminal and tools

mod agent;
mod assistant;
mod catalog;
mod commands;
mod compat;
mod config;
mod diff;
mod extensions;
mod i18n;
mod lifecycle;
mod mcp;
mod permissions;
mod pricing;
mod provider;
mod router;
mod server;
mod sessions;
mod terminal;
mod tools;
mod usage;
mod autostart;
mod tray;
mod window;

use tauri::{Manager, RunEvent};

const MAIN_WINDOW: &str = "main";

pub fn run() {
    let app = tauri::Builder::default()
        // Only one ianox at a time: a second launch focuses the running window.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
                crate::tray::update_show_status(app);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(crate::config::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::start_server,
            commands::stop_server,
            commands::server_status,
            commands::list_models,
            commands::pull_model,
            commands::delete_model,
            commands::chat_completion,
            commands::list_providers,
            commands::add_provider,
            commands::remove_provider,
            commands::test_provider,
            commands::get_routes,
            commands::add_route,
            commands::remove_route,
            commands::toggle_route,
            commands::list_mcp_servers,
            commands::save_mcp_server,
            commands::remove_mcp_server,
            commands::toggle_mcp_server,
            commands::test_mcp_server,
            commands::list_projects,
            commands::add_project,
            commands::remove_project,
            commands::set_active_project,
            commands::list_dir,
            commands::read_file,
            commands::read_file_binary,
            commands::read_clipboard_image,
            commands::file_info,
            commands::write_text_file,
            commands::git_status,
            commands::git_diff,
            commands::git_init,
            commands::git_discard,
            commands::detect_open_apps,
            commands::open_path,
            terminal::term_start,
            terminal::term_write,
            terminal::term_resize,
            terminal::term_kill,
            commands::chat_stream,
            commands::chat_cancel,
            commands::list_agents,
            commands::save_agent,
            commands::remove_agent,
            commands::get_usage,
            commands::get_session_usage,
            commands::list_skills,
            commands::list_plugins,
            commands::detect_lsp_servers,
            commands::extensions_summary,
            commands::list_sessions,
            commands::get_session,
            commands::save_session,
            commands::delete_session,
            commands::list_fonts,
            window::window_alpha,
            lifecycle::frontend_ready,
            lifecycle::tray_available,
            lifecycle::quit_app,
            lifecycle::hide_to_tray,
            commands::summarize_conversation,
            commands::permission_reply,
            commands::find_project_files,
            commands::list_catalog_models,
            commands::set_provider_models,
            commands::search_sessions,
            commands::generate_session_title,
            commands::find_project_icon,
            commands::set_project_icon,
            commands::rename_session,
            assistant::hide_assistant,
            assistant::open_main_window,
            assistant::resize_assistant,
            autostart::get_autostart_status,
            autostart::set_autostart,
        ])
        .setup(|app| {
            crate::catalog::init();
            // The "main" window is already defined in tauri.conf.json.
            // La obtenemos y la configuramos (barra propia, esquinas redondeadas, etc.)
            let window = app
                .webview_windows()
                .get(MAIN_WINDOW)
                .cloned()
                .expect("Window 'main' not found in tauri.conf.json");

            // None of these may stop the app from starting: a desktop without a tray, or without
            // a compositor (common in virtual machines), still has to get a window.
            if let Err(e) = window::setup(&window) {
                eprintln!("[ianox] window setup: {e}");
            }
            if let Err(e) = tray::init(app.handle().clone()) {
                eprintln!("[ianox] tray icon unavailable: {e}");
            }
            if let Err(e) = assistant::setup_global_shortcut(app.handle()) {
                eprintln!("[ianox] global shortcut setup error: {e}");
            }

            // Load config and auto-start the server if configured
            let app_state = app.state::<crate::config::AppState>().inner().clone();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = commands::load_config_and_maybe_start(app_state, handle).await {
                    eprintln!("Failed to load the initial configuration: {}", e);
                }
            });

            // Show the window when ready — unless launched with --minimized and the
            // tray is available (i.e. autostart from the desktop file). Without a tray
            // the window must always appear or the app becomes unrecoverable.
            let start_hidden = std::env::args()
                .skip(1)
                .any(|a| a == "--minimized" || a == "--hidden");
            if start_hidden && crate::tray::available() {
                let _ = window.hide();
            } else {
                window.show()?;
            }
            crate::tray::update_show_status(app.handle());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Failed to build the Tauri application");

    app.run(|handle, event| {
        // The close button asks the interface what to do (quit, or stay in the tray).
        if let RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } = &event
        {
            if label == MAIN_WINDOW && lifecycle::should_intercept_close() {
                api.prevent_close();
                use tauri::Emitter;
                let _ = handle.emit_to(MAIN_WINDOW, "app://quit-requested", ());
            }
        }

        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            // Limpiar recursos al salir
            let state = handle.state::<crate::config::AppState>();
            tauri::async_runtime::block_on(async {
                if let Some(mut server) = state.server_handle.lock().await.take() {
                    server.stop().await;
                }
            });
        }

        // Guardar estado de la ventana
        if let RunEvent::WindowEvent { event: ref e, .. } = event {
            if let tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_) = e {
                if let Some(_window) = handle.webview_windows().get(MAIN_WINDOW) {
                    window::save_state(handle);
                }
            }
        }
    });
}

fn main() {
    // Before GTK and the web view exist: virtual machines need WebKit's software paths.
    compat::silence_known_system_notices();
    compat::apply_rendering_workarounds();
    run();
}