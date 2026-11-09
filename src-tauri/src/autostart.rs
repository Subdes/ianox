//! ianox — Gestión de arranque automático con el sistema (Autostart)
//!
//! Multiplataforma:
//! - Linux: maneja `~/.config/autostart/io.github.subdes.ianox.desktop`
//! - macOS: LaunchAgents plist
//! - Windows: registro de usuario Run

use std::path::PathBuf;

/// Ruta al archivo de autostart en Linux
#[cfg(target_os = "linux")]
fn linux_autostart_path() -> Option<PathBuf> {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
        .ok()?;
    Some(config_dir.join("autostart").join("io.github.subdes.ianox.desktop"))
}

/// Comprueba si el autostart está activo en el sistema
#[tauri::command]
pub fn get_autostart_status() -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Some(path) = linux_autostart_path() {
            return path.exists();
        }
    }
    false
}

/// Activa o desactiva el arranque automático con el sistema operativo
#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<bool, String> {
    #[cfg(target_os = "linux")]
    {
        let Some(desktop_path) = linux_autostart_path() else {
            return Err("No se pudo determinar la ruta de configuración del usuario".into());
        };

        if enabled {
            // Asegurar directorio ~/.config/autostart/
            if let Some(parent) = desktop_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            // Obtener ruta del ejecutable actual o usar "ianox" como comando
            let exec_cmd = std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "ianox".to_string());

            let content = format!(
                "[Desktop Entry]\n\
                 Type=Application\n\
                 Name=ianox\n\
                 Comment=AI gateway, model manager and coding agent\n\
                 Exec=\"{}\" --minimized\n\
                 Icon=ianox\n\
                 Terminal=false\n\
                 Categories=Development;Utility;\n\
                 StartupNotify=false\n\
                 X-GNOME-Autostart-enabled=true\n",
                exec_cmd
            );

            std::fs::write(&desktop_path, content)
                .map_err(|e| format!("Error al escribir archivo autostart: {e}"))?;
        } else if desktop_path.exists() {
            let _ = std::fs::remove_file(&desktop_path);
        }

        return Ok(get_autostart_status());
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = enabled;
        Ok(false)
    }
}

