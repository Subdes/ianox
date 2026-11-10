use tauri::{AppHandle, Manager};

/// Notifica al usuario sólo si la ventana principal no está en foco o está oculta/minimizada.
#[tauri::command]
pub async fn notify_if_unfocused(
    app: AppHandle,
    title: String,
    body: String,
) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window("main") {
        let is_focused = window.is_focused().unwrap_or(false);
        let is_visible = window.is_visible().unwrap_or(false);
        let is_minimized = window.is_minimized().unwrap_or(false);

        // Si la ventana está visible y tiene el foco activo, no molestamos
        if is_focused && is_visible && !is_minimized {
            return Ok(false);
        }
    }

    send_system_notification(&title, &body);
    Ok(true)
}

fn send_system_notification(title: &str, body: &str) {
    #[cfg(target_os = "linux")]
    {
        // En Linux, notify-send es el estándar presente en KDE, GNOME, XFCE
        let _ = std::process::Command::new("notify-send")
            .arg("-a")
            .arg("ianox")
            .arg("-i")
            .arg("io.github.subdes.ianox")
            .arg(title)
            .arg(body)
            .spawn();
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\" sound name \"default\"",
            body.replace('\"', "\\\""),
            title.replace('\"', "\\\"")
        );
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .spawn();
    }

    #[cfg(target_os = "windows")]
    {
        let ps_cmd = format!(
            "[reflection.assembly]::loadwithpartialname('System.Windows.Forms'); \
             $notify = new-object system.windows.forms.notifyicon; \
             $notify.icon = [system.drawing.systemicons]::Information; \
             $notify.visible = $true; \
             $notify.showballoontip(10, '{}', '{}', [system.windows.forms.tooltipicon]::Info)",
            title.replace('\'', "''"),
            body.replace('\'', "''")
        );
        let _ = std::process::Command::new("powershell")
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg(ps_cmd)
            .spawn();
    }
}

