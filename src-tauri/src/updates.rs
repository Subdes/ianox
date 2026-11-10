use futures_util::StreamExt;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_notes: String,
    pub release_url: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub asset_size: u64,
}

#[derive(Debug, serde::Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Clone, serde::Serialize)]
struct DownloadProgress {
    downloaded: u64,
    total: u64,
    percent: f32,
}

/// Comprueba si hay una nueva versión en github.com/Subdes/ianox/releases
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateCheckResult, String> {
    let current_version = app.package_info().version.to_string();

    let client = reqwest::Client::builder()
        .user_agent(format!("ianox/{}", current_version))
        .build()
        .map_err(|e| format!("Error creando cliente HTTP: {e}"))?;

    let resp = client
        .get("https://api.github.com/repos/Subdes/ianox/releases/latest")
        .send()
        .await
        .map_err(|e| format!("No se pudo conectar a GitHub Releases: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Respuesta no satisfactoria de GitHub: HTTP {}", resp.status()));
    }

    let release: GitHubRelease = resp
        .json()
        .await
        .map_err(|e| format!("Error parseando respuesta de GitHub: {e}"))?;

    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let has_update = is_newer_version(&latest_version, &current_version);

    // Detectamos el tipo de paquete adecuado para este sistema
    let preferred_exts = get_preferred_package_extensions();
    let mut selected_asset: Option<&GitHubAsset> = None;

    for ext in &preferred_exts {
        if let Some(asset) = release.assets.iter().find(|a| a.name.ends_with(ext)) {
            selected_asset = Some(asset);
            break;
        }
    }

    Ok(UpdateCheckResult {
        current_version,
        latest_version,
        has_update,
        release_notes: release.body.unwrap_or_default(),
        release_url: release.html_url,
        asset_name: selected_asset.map(|a| a.name.clone()),
        asset_url: selected_asset.map(|a| a.browser_download_url.clone()),
        asset_size: selected_asset.map(|a| a.size).unwrap_or(0),
    })
}

/// Descarga el paquete de instalación mostrando progreso y procede a abrirlo
#[tauri::command]
pub async fn download_and_install_update(
    app: AppHandle,
    asset_url: String,
    asset_name: String,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent("ianox-updater")
        .build()
        .map_err(|e| format!("Error cliente HTTP: {e}"))?;

    let resp = client
        .get(&asset_url)
        .send()
        .await
        .map_err(|e| format!("Fallo al iniciar descarga de {asset_url}: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Descarga fallida: HTTP {}", resp.status()));
    }

    let total_size = resp.content_length().unwrap_or(0);
    let temp_dir = std::env::temp_dir();
    let target_path: PathBuf = temp_dir.join(&asset_name);

    let mut file = tokio::fs::File::create(&target_path)
        .await
        .map_err(|e| format!("No se pudo crear archivo temporal {}: {e}", target_path.display()))?;

    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;

    use tokio::io::AsyncWriteExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Error en flujo de descarga: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Error escribiendo en disco: {e}"))?;
        downloaded += chunk.len() as u64;

        let percent = if total_size > 0 {
            (downloaded as f32 / total_size as f32) * 100.0
        } else {
            0.0
        };

        let _ = app.emit(
            "update://progress",
            DownloadProgress {
                downloaded,
                total: total_size,
                percent,
            },
        );
    }

    file.flush()
        .await
        .map_err(|e| format!("Error finalizando archivo: {e}"))?;

    // Permisos de ejecución en Linux/macOS
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        let _ = std::fs::set_permissions(&target_path, perms);
    }

    // Abrir o ejecutar el instalador según el sistema
    let path_str = target_path.to_string_lossy().to_string();
    launch_installer(&target_path)?;

    Ok(path_str)
}

fn launch_installer(path: &PathBuf) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if filename.ends_with(".AppImage") {
            let _ = std::process::Command::new(path).spawn();
            return Ok(());
        }

        // Para .deb, intentamos abrir con xdg-open (centro de software)
        // o si no está disponible, abrir carpeta
        let _ = open::that(path);
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let _ = open::that(path);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let _ = open::that(path);
        Ok(())
    }
}

fn get_preferred_package_extensions() -> Vec<&'static str> {
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/.flatpak-info").exists() {
            return vec![".flatpak", ".AppImage", ".deb", ".rpm"];
        }
        if std::env::var("APPIMAGE").is_ok() {
            return vec![".AppImage", ".deb", ".rpm", ".flatpak"];
        }
        if std::path::Path::new("/etc/debian_version").exists() {
            return vec![".deb", ".AppImage", ".flatpak", ".rpm"];
        }
        if std::path::Path::new("/etc/fedora-release").exists()
            || std::path::Path::new("/etc/redhat-release").exists()
        {
            return vec![".rpm", ".AppImage", ".flatpak", ".deb"];
        }
        vec![".deb", ".AppImage", ".rpm", ".flatpak"]
    }

    #[cfg(target_os = "windows")]
    {
        vec![".msi", ".exe"]
    }

    #[cfg(target_os = "macos")]
    {
        vec![".dmg", ".app.tar.gz"]
    }
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_version = |v: &str| -> Vec<u64> {
        v.split('.')
            .filter_map(|part| part.split('-').next().and_then(|n| n.parse::<u64>().ok()))
            .collect()
    };

    let l_parts = parse_version(latest);
    let c_parts = parse_version(current);

    for (l, c) in l_parts.iter().zip(c_parts.iter()) {
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }

    l_parts.len() > c_parts.len()
}

