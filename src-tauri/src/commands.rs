//! Comandos Tauri expuestos al frontend

use tauri::{AppHandle, Manager, State};
use crate::config::{AppState, Config, ProviderConfig, ProviderKind, RateLimit};

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    let config = state.config.read().await;
    Ok(config.clone())
}

#[tauri::command]
pub async fn set_config(state: State<'_, AppState>, config: Config) -> Result<(), String> {
    if config.version == 0 {
        return Err("Invalid configuration version".into());
    }
    let mut current = state.config.write().await;
    *current = config.clone();

    persist_config(&config).await?;

    Ok(())
}

async fn persist_config(config: &Config) -> Result<(), String> {
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    let path = config_path().await?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| format!("Could not write {}: {e}", path.display()))
}

async fn config_path() -> Result<std::path::PathBuf, String> {
    let dir = std::env::var("APPDATA")
        .or_else(|_| std::env::var("XDG_CONFIG_HOME"))
        .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.config", h)))
        .map_err(|_| "Could not get the configuration directory")?;
    Ok(std::path::PathBuf::from(dir).join("ianox").join("config.json"))
}

pub async fn load_config_and_maybe_start(
    state: AppState,
    app: AppHandle,
) -> Result<(), String> {
    let path = config_path().await?;
    if path.exists() {
        let json = tokio::fs::read_to_string(&path).await.map_err(|e| e.to_string())?;
        let config: Config = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        let mut current = state.config.write().await;
        *current = config;
    }

    let config = state.config.read().await;
    if config.server.enabled && config.ui.auto_start_server {
        drop(config);
        start_server_inner(state).await?;
        crate::tray::update_api_status(&app);
    }
    Ok(())
}

// Internal logic (uses AppState directly, callable from spawned tasks)
async fn start_server_inner(state: AppState) -> Result<String, String> {
    let mut handle_guard = state.server_handle.lock().await;
    if handle_guard.is_some() {
        return Err("The server is already running".into());
    }

    let config_arc = state.config.clone();

    match crate::server::start_server(config_arc).await {
        Ok(handle) => {
            let addr = format!("http://{}", handle.addr);
            *handle_guard = Some(handle);
            Ok(addr)
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn start_server(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    let res = start_server_inner(state.inner().clone()).await;
    if res.is_ok() {
        crate::tray::update_api_status(&app);
    }
    res
}

#[tauri::command]
pub async fn stop_server(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let mut handle_guard = state.server_handle.lock().await;
    if let Some(mut handle) = handle_guard.take() {
        handle.stop().await;
    }
    drop(handle_guard);
    crate::tray::update_api_status(&app);
    Ok(())
}

#[tauri::command]
pub async fn server_status(state: State<'_, AppState>) -> Result<ServerStatus, String> {
    let handle_guard = state.server_handle.lock().await;
    let running = handle_guard.is_some();
    let addr = handle_guard.as_ref().map(|h| format!("http://{}", h.addr));
    Ok(ServerStatus { running, addr })
}

#[derive(serde::Serialize)]
pub struct ServerStatus {
    pub running: bool,
    pub addr: Option<String>,
}

// ── Modelos (Ollama-style) ──

#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let providers = state.config.read().await.providers.clone();
    let mut models = Vec::new();
    for (id, prov) in providers {
        if prov.enabled {
            for model in &prov.models {
                let entry = crate::catalog::lookup(&id, model);
                models.push(ModelInfo {
                    id: model.clone(),
                    name: entry
                        .as_ref()
                        .map(|e| e.name.clone())
                        .unwrap_or_else(|| display_name(model)),
                    context: entry.as_ref().and_then(|e| e.context),
                    free: entry.as_ref().map_or(false, crate::catalog::is_free),
                    provider_name: prov.name.clone(),
                    provider: id.clone(),
                    provider_kind: format!("{:?}", prov.kind),
                    size: None,
                });
            }
        }
    }
    Ok(models)
}

/// Human-readable model name: "deepseek-v4-pro" -> "DeepSeek V4 Pro".
pub fn display_name(id: &str) -> String {
    let id = id.rsplit('/').next().unwrap_or(id);
    id.split(|c: char| c == '-' || c == '_' || c == ':')
        .filter(|w| !w.is_empty())
        .map(|word| {
            let lower = word.to_lowercase();
            match lower.as_str() {
                "gpt" => "GPT".to_string(),
                "deepseek" => "DeepSeek".to_string(),
                "llama" => "Llama".to_string(),
                "qwen" => "Qwen".to_string(),
                "gemini" => "Gemini".to_string(),
                "claude" => "Claude".to_string(),
                _ if lower.chars().next().map_or(false, |c| c.is_ascii_digit() || c == 'v')
                    && lower.chars().skip(1).all(|c| c.is_ascii_digit() || c == '.' || c == 'b' || c == 'k') =>
                {
                    word.to_uppercase()
                }
                _ => {
                    let mut chars = word.chars();
                    chars.next().map(|f| f.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(serde::Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub context: Option<u64>,
    /// The catalog lists it at no cost.
    pub free: bool,
    pub provider_name: String,
    pub provider: String,
    pub provider_kind: String,
    pub size: Option<u64>,
}

#[tauri::command]
pub async fn pull_model(_state: State<'_, AppState>, _model: String) -> Result<(), String> {
    Err("Not implemented yet - requires an Ollama client".into())
}

#[tauri::command]
pub async fn delete_model(_state: State<'_, AppState>, _model: String) -> Result<(), String> {
    Err("Not implemented yet".into())
}

// ── Chat completion (OpenAI-compatible) ──

#[tauri::command]
pub async fn chat_completion(
    _state: State<'_, AppState>,
    _req: ChatRequest,
) -> Result<ChatResponse, String> {
    Err("Not implemented yet - use the server HTTP endpoint".into())
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(serde::Serialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(serde::Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: String,
}

#[derive(serde::Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ── Proveedores ──

#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderInfo>, String> {
    let providers = state.config.read().await.providers.clone();
    let mut result = Vec::new();
    for (id, prov) in providers {
        result.push(ProviderInfo {
            id,
            name: prov.name,
            kind: format!("{:?}", prov.kind),
            base_url: prov.base_url,
            models: prov.models,
            enabled: prov.enabled,
            priority: prov.priority,
            has_key: prov.api_key.as_deref().is_some_and(|key| !key.trim().is_empty()),
        });
    }
    Ok(result)
}

#[derive(serde::Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub models: Vec<String>,
    pub enabled: bool,
    pub priority: u8,
    /// A key is saved (the key itself is never sent to the interface).
    pub has_key: bool,
}

#[tauri::command]
pub async fn add_provider(
    state: State<'_, AppState>,
    provider: ProviderConfigInput,
) -> Result<(), String> {
    let mut config = state.config.write().await;
    // Editing keeps the saved key: the form sends `null` when the field is left empty.
    let kept_key = config.providers.get(&provider.id).and_then(|old| old.api_key.clone());
    let api_key = provider
        .api_key
        .filter(|key| !key.trim().is_empty())
        .or(kept_key);
    let prov = ProviderConfig {
        id: provider.id.clone(),
        name: provider.name,
        kind: provider.kind,
        base_url: provider.base_url,
        api_key,
        models: provider.models,
        enabled: provider.enabled,
        priority: provider.priority,
        rate_limit: provider.rate_limit.map(|rl| RateLimit {
            requests_per_minute: rl.requests_per_minute,
            tokens_per_minute: rl.tokens_per_minute,
        }),
    };
    config.providers.insert(provider.id, prov);
    drop(config);
    let cfg = state.config.read().await.clone();
    persist_config(&cfg).await
}

#[tauri::command]
pub async fn remove_provider(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let mut config = state.config.write().await;
    config.providers.remove(&id);
    drop(config);
    let cfg = state.config.read().await.clone();
    persist_config(&cfg).await
}

#[derive(serde::Serialize)]
pub struct ProviderTestResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub models: Vec<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn test_provider(provider: ProviderConfigInput) -> Result<ProviderTestResult, String> {
    use std::time::{Duration, Instant};

    let started = Instant::now();
    let base = provider.base_url.trim_end_matches('/');
    let is_anthropic = matches!(provider.kind, ProviderKind::Anthropic);
    let is_ollama = matches!(provider.kind, ProviderKind::Ollama);

    let url = if is_ollama && !base.contains("/v1") {
        format!("{base}/api/tags")
    } else {
        format!("{base}/models")
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let mut request = client.get(&url);
    if is_anthropic {
        if let Some(key) = provider.api_key.as_deref().filter(|k| !k.is_empty()) {
            request = request
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01");
        }
    } else if let Some(key) = provider.api_key.as_deref().filter(|k| !k.is_empty()) {
        request = request.header("authorization", format!("Bearer {key}"));
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(e) => {
            return Ok(ProviderTestResult {
                ok: false,
                latency_ms: started.elapsed().as_millis() as u64,
                models: Vec::new(),
                error: Some(format!("Could not connect: {e}")),
            })
        }
    };

    let status = response.status();
    let latency_ms = started.elapsed().as_millis() as u64;
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Ok(ProviderTestResult {
            ok: false,
            latency_ms,
            models: Vec::new(),
            error: Some(format!(
                "HTTP {}: {}",
                status.as_u16(),
                body.chars().take(300).collect::<String>()
            )),
        });
    }

    let json: serde_json::Value = match response.json().await {
        Ok(value) => value,
        Err(e) => {
            return Ok(ProviderTestResult {
                ok: false,
                latency_ms,
                models: Vec::new(),
                error: Some(format!("Invalid response: {e}")),
            })
        }
    };

    let mut models = Vec::new();
    if is_ollama && url.ends_with("/api/tags") {
        if let Some(items) = json.get("models").and_then(|v| v.as_array()) {
            for item in items {
                if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                    models.push(name.to_string());
                }
            }
        }
    } else if let Some(items) = json.get("data").and_then(|v| v.as_array()) {
        for item in items {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                models.push(id.to_string());
            }
        }
    }
    models.sort();
    models.dedup();

    Ok(ProviderTestResult {
        ok: true,
        latency_ms,
        models,
        error: None,
    })
}

#[tauri::command]
pub async fn get_usage(days: Option<u32>) -> Result<crate::usage::UsageSummary, String> {
    crate::pricing::ensure_file();
    Ok(crate::usage::summarize(days.unwrap_or(7)))
}

#[tauri::command]
pub async fn get_session_usage(session_id: String) -> Result<crate::usage::SessionUsage, String> {
    crate::pricing::ensure_file();
    Ok(crate::usage::session_usage(&session_id))
}

#[derive(serde::Deserialize)]
pub struct ProviderConfigInput {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub api_key: Option<String>,
    pub models: Vec<String>,
    pub enabled: bool,
    pub priority: u8,
    pub rate_limit: Option<RateLimitInput>,
}

#[derive(serde::Deserialize)]
pub struct RateLimitInput {
    pub requests_per_minute: u32,
    pub tokens_per_minute: u32,
}

// ── Rutas (gateway router) ──

#[tauri::command]
pub async fn get_routes(state: State<'_, AppState>) -> Result<Vec<RouteInfo>, String> {
    let routes = state.config.read().await.routes.clone();
    let mut result = Vec::new();
    for route in routes {
        result.push(RouteInfo {
            id: route.id,
            name: route.name,
            enabled: route.enabled,
            model_pattern: route.model_pattern,
            provider_ids: route.provider_ids,
            fallback: route.fallback,
            weight: route.weight,
        });
    }
    Ok(result)
}

#[derive(serde::Serialize)]
pub struct RouteInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub model_pattern: String,
    pub provider_ids: Vec<String>,
    pub fallback: bool,
    pub weight: u8,
}

#[tauri::command]
pub async fn add_route(
    state: State<'_, AppState>,
    route: RouteConfigInput,
) -> Result<(), String> {
    let mut config = state.config.write().await;
    if config.routes.iter().any(|r| r.id == route.id) {
        return Err("Ruta ya existe".into());
    }
    config.routes.push(crate::config::RouteConfig {
        id: route.id,
        name: route.name,
        enabled: route.enabled,
        model_pattern: route.model_pattern,
        provider_ids: route.provider_ids,
        fallback: route.fallback,
        weight: route.weight,
    });
    drop(config);
    let cfg = state.config.read().await.clone();
    persist_config(&cfg).await
}

#[tauri::command]
pub async fn remove_route(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let mut config = state.config.write().await;
    config.routes.retain(|r| r.id != id);
    drop(config);
    let cfg = state.config.read().await.clone();
    persist_config(&cfg).await
}

#[tauri::command]
pub async fn toggle_route(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let mut config = state.config.write().await;
    if let Some(route) = config.routes.iter_mut().find(|r| r.id == id) {
        route.enabled = enabled;
    }
    drop(config);
    let cfg = state.config.read().await.clone();
    persist_config(&cfg).await
}

#[derive(serde::Deserialize)]
pub struct RouteConfigInput {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub model_pattern: String,
    pub provider_ids: Vec<String>,
    pub fallback: bool,
    pub weight: u8,
}
// ── Servidores MCP (stdio) ──

#[derive(serde::Serialize)]
pub struct McpServerInfo {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: std::collections::HashMap<String, String>,
    pub enabled: bool,
}

#[derive(serde::Deserialize)]
pub struct McpServerInput {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[tauri::command]
pub async fn list_mcp_servers(state: State<'_, AppState>) -> Result<Vec<McpServerInfo>, String> {
    let config = state.config.read().await;
    let mut servers: Vec<McpServerInfo> = config
        .mcp_servers
        .values()
        .map(|s| McpServerInfo {
            id: s.id.clone(),
            name: s.name.clone(),
            command: s.command.clone(),
            args: s.args.clone(),
            env: s.env.clone(),
            enabled: s.enabled,
        })
        .collect();
    servers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(servers)
}

#[tauri::command]
pub async fn save_mcp_server(
    state: State<'_, AppState>,
    server: McpServerInput,
) -> Result<(), String> {
    let id = server.id.trim().to_string();
    if id.is_empty() {
        return Err("The ID cannot be empty".into());
    }
    if server.command.trim().is_empty() {
        return Err("The command cannot be empty".into());
    }

    {
        let mut config = state.config.write().await;
        config.mcp_servers.insert(
            id.clone(),
            crate::config::McpServerConfig {
                id,
                name: if server.name.trim().is_empty() {
                    server.command.clone()
                } else {
                    server.name
                },
                command: server.command,
                args: server.args,
                env: server.env,
                enabled: server.enabled,
            },
        );
    }

    let snapshot = state.config.read().await.clone();
    persist_config(&snapshot).await
}

#[tauri::command]
pub async fn remove_mcp_server(state: State<'_, AppState>, id: String) -> Result<(), String> {
    {
        let mut config = state.config.write().await;
        config.mcp_servers.remove(&id);
    }
    let snapshot = state.config.read().await.clone();
    persist_config(&snapshot).await
}

#[tauri::command]
pub async fn toggle_mcp_server(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    {
        let mut config = state.config.write().await;
        if let Some(server) = config.mcp_servers.get_mut(&id) {
            server.enabled = enabled;
        } else {
            return Err("Servidor MCP no encontrado".into());
        }
    }
    let snapshot = state.config.read().await.clone();
    persist_config(&snapshot).await
}

#[tauri::command]
pub async fn test_mcp_server(server: McpServerInput) -> Result<crate::mcp::McpTestResult, String> {
    Ok(crate::mcp::probe(&server.command, &server.args, &server.env).await)
}

// ── Proyectos (workspace) ──

#[derive(serde::Serialize)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    /// Small image as a data URL (detected from the project or chosen by the user).
    pub icon: Option<String>,
}

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectInfo>, String> {
    let config = state.config.read().await;
    Ok(config
        .projects
        .iter()
        .map(|p| ProjectInfo {
            id: p.id.clone(),
            name: p.name.clone(),
            path: p.path.clone(),
            icon: p.icon.clone(),
        })
        .collect())
}

#[tauri::command]
pub async fn add_project(
    state: State<'_, AppState>,
    name: String,
    path: String,
) -> Result<ProjectInfo, String> {
    add_project_inner(state.inner(), &name, &path).await
}

async fn add_project_inner(
    state: &AppState,
    name: &str,
    path: &str,
) -> Result<ProjectInfo, String> {
    let path = path.trim().to_string();
    if path.is_empty() {
        return Err("The path is empty".into());
    }
    let canonical = std::fs::canonicalize(&path).map_err(|e| format!("Invalid path: {e}"))?;
    if !canonical.is_dir() {
        return Err("The path is not a folder".into());
    }
    let path_str = canonical.to_string_lossy().to_string();

    let project = {
        let mut config = state.config.write().await;
        if let Some(existing) = config.projects.iter().find(|p| p.path == path_str) {
            let info = ProjectInfo {
                id: existing.id.clone(),
                name: existing.name.clone(),
                path: existing.path.clone(),
                icon: existing.icon.clone(),
            };
            config.active_project = Some(existing.id.clone());
            drop(config);
            let snapshot = state.config.read().await.clone();
            persist_config(&snapshot).await?;
            return Ok(info);
        }

        let id = uuid::Uuid::new_v4().to_string();
        let final_name = if name.trim().is_empty() {
            canonical
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "proyecto".to_string())
        } else {
            name.trim().to_string()
        };
        config.projects.push(crate::config::ProjectConfig {
            id: id.clone(),
            name: final_name.clone(),
            path: path_str.clone(),
            icon: None,
        });
        config.active_project = Some(id.clone());
        ProjectInfo { id, name: final_name, path: path_str, icon: None }
    };

    let snapshot = state.config.read().await.clone();
    persist_config(&snapshot).await?;
    Ok(project)
}

#[tauri::command]
pub async fn remove_project(state: State<'_, AppState>, id: String) -> Result<(), String> {
    {
        let mut config = state.config.write().await;
        config.projects.retain(|p| p.id != id);
        if config.active_project.as_deref() == Some(id.as_str()) {
            config.active_project = config.projects.first().map(|p| p.id.clone());
        }
    }
    let snapshot = state.config.read().await.clone();
    persist_config(&snapshot).await
}

#[tauri::command]
pub async fn set_active_project(
    state: State<'_, AppState>,
    id: Option<String>,
) -> Result<(), String> {
    {
        let mut config = state.config.write().await;
        config.active_project = id;
    }
    let snapshot = state.config.read().await.clone();
    persist_config(&snapshot).await
}

// ── Sistema de archivos (workspace) ──

#[derive(serde::Serialize)]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[tauri::command]
pub async fn list_dir(path: String) -> Result<Vec<FsEntry>, String> {
    let mut entries = Vec::new();
    let read = std::fs::read_dir(&path).map_err(|e| format!("Could not read {path}: {e}"))?;
    for entry in read.flatten() {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" || name == "node_modules" || name == "target" {
            continue;
        }
        entries.push(FsEntry {
            name,
            path: entry.path().to_string_lossy().to_string(),
            is_dir: meta.is_dir(),
        });
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

#[derive(serde::Serialize)]
pub struct FileContent {
    pub content: String,
    pub truncated: bool,
}

#[tauri::command]
pub async fn read_file(path: String) -> Result<FileContent, String> {
    use std::io::Read;

    const MAX_BYTES: usize = 384 * 1024;
    let mut file = std::fs::File::open(&path).map_err(|e| format!("Could not open {path}: {e}"))?;
    let mut buffer = Vec::new();
    file.by_ref()
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut buffer)
        .map_err(|e| format!("Could not read {path}: {e}"))?;

    let truncated = buffer.len() > MAX_BYTES;
    if truncated {
        buffer.truncate(MAX_BYTES);
    }

    if buffer.iter().take(8192).any(|b| *b == 0) {
        return Err("El archivo parece binario".into());
    }

    Ok(FileContent {
        content: String::from_utf8_lossy(&buffer).to_string(),
        truncated,
    })
}

// ── Internal router (chat without going through the HTTP server) ──

/// The picker sends the exact provider; routing only decides when none is given.
fn pick_provider(
    config: &crate::config::Config,
    model: &str,
    provider_id: Option<&str>,
) -> Option<crate::config::ProviderConfig> {
    provider_id
        .and_then(|id| config.providers.get(id))
        .filter(|p| p.enabled && p.models.iter().any(|m| m == model))
        .cloned()
        .or_else(|| crate::router::resolve_provider(config, model))
}

#[tauri::command]
pub async fn chat_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    request_id: String,
    model: String,
    messages: Vec<serde_json::Value>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    session_id: Option<String>,
    agent_mode: Option<String>,
    provider_id: Option<String>,
) -> Result<(), String> {
    let config = state.config.read().await.clone();
    let provider = pick_provider(&config, &model, provider_id.as_deref())
        .ok_or_else(|| format!("No provider configured for model '{model}'"))?;
    let provider_kind = format!("{:?}", provider.kind);

    // A round that is cut by the output limit leaves a half-written tool call, and
    // reasoning models spend that budget thinking. Default to the model's real limit.
    let max_tokens = max_tokens.or_else(|| {
        let from_catalog = crate::catalog::lookup(&provider.id, &model)
            .and_then(|entry| entry.output)
            .map(|output| output.min(32_768) as u32);
        Some(from_catalog.unwrap_or(16_384))
    });

    let project_path = config
        .projects
        .iter()
        .find(|p| config.active_project.as_deref() == Some(p.id.as_str()))
        .map(|p| p.path.clone());

    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    state
        .chat_cancels
        .lock()
        .await
        .insert(request_id.clone(), cancel.clone());

    let app_task = app.clone();
    let task_session_id = session_id.clone();
    let agent_mode = agent_mode.unwrap_or_else(|| "build".into());
    let task_provider_kind = Some(provider_kind);
    let task_id = request_id.clone();
    tauri::async_runtime::spawn(async move {
        let started = std::time::Instant::now();
        let provider_id = provider.id.clone();
        let model_name = model.clone();

        use tauri::Emitter;

        // Tools are off until the agent harness is built properly: the thin loop
        // made the model explore forever and leak DSML text.
        const TOOLS_ENABLED: bool = true;
        let tools = if TOOLS_ENABLED {
            crate::tools::definitions(&agent_mode)
        } else {
            Vec::new()
        };
        eprintln!(
            "[ianox] agent_mode={agent_mode} tools={} model={model}",
            tools.len()
        );
        // Paths the user wrote themselves may be read without asking, wherever they are.
        let user_paths = {
            let texts: Vec<&str> = messages
                .iter()
                .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
                .filter_map(|m| m.get("content").and_then(|v| v.as_str()))
                .collect();
            crate::agent::mentioned_paths(&texts)
        };
        // "Allow for this conversation" is remembered per conversation (or per request without one).
        let session_key = task_session_id.clone().unwrap_or_else(|| task_id.clone());
        let mut messages = messages;
        let mut prompt_tokens = 0u64;
        let mut completion_tokens = 0u64;
        let mut failure: Option<String> = None;
        let mut exhausted = true;
        let mut last_context: Option<u64> = None;
        let mut written: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Agent loop: ask, run the requested tools, append the round, ask again.
        for round in 0..16 {
            // At round 10, stop the exploration and push for the deliverable.
            if round == 6 {
                messages.push(serde_json::json!({
                    "role": "user",
                    "content": "Stop exploring. You already have enough context. Perform the requested action now with the write tool and then answer the user."
                }));
            }
            let outcome = match crate::router::stream_chat(
                app_task.clone(),
                task_id.clone(),
                provider.clone(),
                model.clone(),
                messages.clone(),
                temperature,
                max_tokens,
                cancel.clone(),
                &tools,
            )
            .await
            {
                Ok(outcome) => outcome,
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            };

            prompt_tokens += outcome.stats.prompt_tokens;
            completion_tokens += outcome.stats.completion_tokens;
            last_context = Some(outcome.stats.prompt_tokens + outcome.stats.completion_tokens);

            // Stop pressed: do not run half-streamed tool calls or start another round.
            if outcome.cancelled {
                exhausted = false;
                break;
            }

            eprintln!("[ianox] round: calls={}", outcome.calls.len());
            for call in &outcome.calls {
                eprintln!("[ianox]   {}({})", call.name, call.arguments);
            }
            if outcome.calls.is_empty() {
                exhausted = false;
                break;
            }

            // Do not write the same file twice in one answer.
            let calls: Vec<_> = outcome
                .calls
                .iter()
                .filter(|call| {
                    if call.name != "write" {
                        return true;
                    }
                    let path = serde_json::from_str::<serde_json::Value>(&call.arguments)
                        .ok()
                        .and_then(|args| args.get("path").and_then(|v| v.as_str()).map(str::to_string))
                        .unwrap_or_default();
                    written.insert(path)
                })
                .cloned()
                .collect();

            // Text from consecutive rounds shares one stream: keep paragraphs apart.
            if !outcome.text.trim().is_empty() {
                let _ = app_task.emit(&format!("chat://delta/{task_id}"), "\n\n");
            }
            // Announce the calls before running them so the UI can show them in progress.
            let _ = app_task.emit(&format!("chat://tool-call/{task_id}"), &calls);
            let results = crate::agent::run_tools_checked(
                &app_task,
                &task_id,
                &session_key,
                project_path.as_deref(),
                &agent_mode,
                &calls,
                &user_paths,
                &cancel,
            )
            .await;
            let _ = app_task.emit(&format!("chat://tool-result/{task_id}"), &results);
            let touched_files = calls
                .iter()
                .any(|c| matches!(c.name.as_str(), "write" | "edit" | "shell"))
                || results.iter().any(|r| r.diff.is_some());
            if touched_files {
                let _ = app_task.emit("app://workspace-changed", ());
            }
            crate::agent::append_tool_round(
                &mut messages,
                &calls,
                &results,
                &outcome.text,
                &outcome.reasoning,
            );
        }

        // If the loop ended on pending tools, ask once more without tools so the
        // model has to answer in text instead of going silent.
        if failure.is_none() && exhausted {
            messages.push(serde_json::json!({
                "role": "user",
                "content": "Now answer the user: a short summary in the user's language. Do not repeat your analysis and do not call more tools."
            }));
            match crate::router::stream_chat(
                app_task.clone(),
                task_id.clone(),
                provider.clone(),
                model.clone(),
                messages.clone(),
                temperature,
                max_tokens,
                cancel.clone(),
                &tools,
            )
            .await
            {
                Ok(outcome) => {
                    prompt_tokens += outcome.stats.prompt_tokens;
                    completion_tokens += outcome.stats.completion_tokens;
                    last_context = Some(outcome.stats.prompt_tokens + outcome.stats.completion_tokens);
                }
                Err(error) => failure = Some(error),
            }
        }

        let result: Result<(), String> = match failure {
            Some(error) => Err(error),
            None => Ok(()),
        };

        // Tell the UI how the answer ended. The error must go out BEFORE anything that makes the
        // frontend drop its listeners (it does so on `done`), or the message is never shown.
        match &result {
            Err(error) => {
                let _ = app_task.emit(&format!("chat://error/{task_id}"), error);
            }
            Ok(()) => {
                let _ = app_task.emit(&format!("chat://done/{task_id}"), ());
            }
        }
        crate::usage::record(&crate::usage::UsageRecord {
            ts: chrono::Utc::now().timestamp(),
            provider: provider_id,
            model: model_name,
            prompt_tokens,
            completion_tokens,
            latency_ms: started.elapsed().as_millis() as u64,
            ok: result.is_ok(),
            session_id: task_session_id,
            provider_kind: task_provider_kind,
            context_tokens: last_context,
        });

        let state_handle = app_task.state::<crate::config::AppState>();
        state_handle.chat_cancels.lock().await.remove(&task_id);
    });

    Ok(())
}

/// The user's answer to a question from the agent: "once", "always" or "deny".
#[tauri::command]
pub async fn permission_reply(
    state: State<'_, AppState>,
    id: String,
    decision: String,
) -> Result<(), String> {
    let decision = match decision.as_str() {
        "once" | "always" => decision,
        _ => "deny".to_string(),
    };
    if let Some(waiter) = state.permission_waiters.lock().await.remove(&id) {
        let _ = waiter.send(decision);
    }
    Ok(())
}

#[tauri::command]
pub async fn chat_cancel(state: State<'_, AppState>, request_id: String) -> Result<(), String> {
    if let Some(flag) = state.chat_cancels.lock().await.get(&request_id) {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}


// ── Agentes ──

#[derive(serde::Deserialize)]
pub struct AgentInput {
    pub id: String,
    pub name: String,
    pub mode: String,
    pub model: Option<String>,
    #[serde(default)]
    pub system_prompt: String,
}

#[tauri::command]
pub async fn list_agents(state: State<'_, AppState>) -> Result<Vec<crate::config::AgentConfig>, String> {
    Ok(state.config.read().await.agents.clone())
}

#[tauri::command]
pub async fn save_agent(state: State<'_, AppState>, agent: AgentInput) -> Result<(), String> {
    let id = agent.id.trim().to_string();
    if id.is_empty() {
        return Err("The agent ID cannot be empty".into());
    }
    if agent.name.trim().is_empty() {
        return Err("The agent name cannot be empty".into());
    }
    if agent.mode != "build" && agent.mode != "plan" {
        return Err("Invalid mode: use build or plan".into());
    }

    {
        let mut config = state.config.write().await;
        let entry = crate::config::AgentConfig {
            id: id.clone(),
            name: agent.name.trim().to_string(),
            mode: agent.mode,
            model: agent.model.filter(|m| !m.trim().is_empty()),
            system_prompt: agent.system_prompt,
        };
        if let Some(existing) = config.agents.iter_mut().find(|a| a.id == id) {
            *existing = entry;
        } else {
            config.agents.push(entry);
        }
    }

    let snapshot = state.config.read().await.clone();
    persist_config(&snapshot).await
}

#[tauri::command]
pub async fn remove_agent(state: State<'_, AppState>, id: String) -> Result<(), String> {
    {
        let mut config = state.config.write().await;
        config.agents.retain(|a| a.id != id);
    }
    let snapshot = state.config.read().await.clone();
    persist_config(&snapshot).await
}

#[cfg(test)]
mod project_tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[tokio::test]
    async fn add_project_works_on_first_run() {
        let _guard = ENV_LOCK.lock().unwrap();
        let base = std::env::temp_dir().join(format!("ianox-proj-{}", uuid::Uuid::new_v4()));
        let project_dir = base.join("proj");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", base.join("config"));

        let state = AppState::new();
        let info = add_project_inner(
            &state,
            "demo",
            project_dir.to_str().unwrap(),
        )
        .await
        .expect("add_project must work on first launch");

        assert!(base.join("config").join("ianox").join("config.json").exists());
        let config = state.config.read().await;
        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.active_project.as_deref(), Some(info.id.as_str()));

        std::fs::remove_dir_all(&base).ok();
    }

    #[tokio::test]
    async fn persist_creates_missing_config_dir() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = std::env::temp_dir().join(format!("ianox-persist-{}", uuid::Uuid::new_v4()));
        std::env::set_var("XDG_CONFIG_HOME", &tmp);

        persist_config(&Config::default())
            .await
            .expect("persist_config debe crear la carpeta");

        assert!(tmp.join("ianox").join("config.json").exists());
        std::fs::remove_dir_all(&tmp).ok();
    }
}

#[cfg(test)]
mod provider_input_tests {
    use super::*;

    #[test]
    fn deserializes_deepseek_form_payload() {
        let payload = serde_json::json!({
            "id": "deepseek",
            "name": "DeepSeek",
            "kind": "Custom",
            "base_url": "https://api.deepseek.com/v1",
            "api_key": "sk-test",
            "models": [],
            "enabled": true,
            "priority": 1,
            "rate_limit": null
        });
        let parsed: ProviderConfigInput =
            serde_json::from_value(payload).expect("the form payload must deserialize");
        assert_eq!(parsed.id, "deepseek");
        assert_eq!(parsed.base_url, "https://api.deepseek.com/v1");
    }
}

// ── Sesiones (historial de chat) ──

#[derive(serde::Deserialize)]
pub struct SessionInput {
    pub id: String,
    pub title: String,
    pub project_id: Option<String>,
    pub agent_id: String,
    pub model: Option<String>,
    pub messages: Vec<crate::sessions::StoredMessage>,
}

/// Finds conversations by the text of their messages.
#[tauri::command]
pub async fn search_sessions(query: String) -> Result<Vec<crate::sessions::SessionHit>, String> {
    tokio::task::spawn_blocking(move || crate::sessions::search(&query, 30))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_sessions() -> Result<Vec<crate::sessions::SessionSummary>, String> {
    Ok(crate::sessions::list())
}

#[tauri::command]
pub async fn get_session(id: String) -> Result<crate::sessions::SessionRecord, String> {
    crate::sessions::get(&id).ok_or_else(|| "Session not found".to_string())
}

#[tauri::command]
pub async fn save_session(session: SessionInput) -> Result<crate::sessions::SessionSummary, String> {
    crate::sessions::save(crate::sessions::SessionRecord {
        id: session.id,
        title: session.title,
        project_id: session.project_id,
        agent_id: session.agent_id,
        model: session.model,
        created_at: 0,
        updated_at: 0,
        messages: session.messages,
    })
}

#[tauri::command]
pub async fn delete_session(id: String) -> Result<(), String> {
    crate::sessions::delete(&id)
}

#[tauri::command]
pub async fn rename_session(id: String, title: String) -> Result<(), String> {
    crate::sessions::rename(&id, &title)
}

// ── Binary files (images, PDF) ──

#[derive(serde::Serialize)]
pub struct BinaryFile {
    pub mime: String,
    pub data: String,
}

fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        "pdf" => "application/pdf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Reads an image from the system clipboard.
///
/// WebKitGTK does not hand the image to the page's `paste` event, so the composer
/// asks the window's GTK clipboard instead. `None` means the clipboard holds no
/// image and the paste is plain text.
/// Reads an image from the system clipboard.
///
/// Works on X11 and Wayland. WebKitGTK never hands the clipboard image to the
/// page's `paste` event, so the composer asks here instead:
///
/// 1. GTK, through the window: GDK picks the backend (X11 or Wayland), and this is
///    the path that keeps working when the clipboard belongs to another app.
/// 2. `wl-paste` / `xclip`: some Wayland compositors (and bare X sessions without a
///    clipboard manager) only answer through these, so they are tried second.
///
/// `None` means the clipboard holds no image, and the paste is plain text.
#[tauri::command]
pub async fn read_clipboard_image(app: AppHandle) -> Result<Option<BinaryFile>, String> {
    #[cfg(target_os = "linux")]
    {
        if let Some(image) = read_clipboard_with_gtk(&app).await {
            return Ok(Some(image));
        }
        Ok(read_clipboard_with_tools())
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
        Ok(None)
    }
}

#[cfg(target_os = "linux")]
fn encode_png(bytes: Vec<u8>) -> Option<BinaryFile> {
    use base64::Engine;
    // A PNG signature, in case the tool answered with something else.
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return None;
    }
    Some(BinaryFile {
        mime: "image/png".to_string(),
        data: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

/// GTK clipboard, read on the thread that owns the main loop.
#[cfg(target_os = "linux")]
async fn read_clipboard_with_gtk(app: &AppHandle) -> Option<BinaryFile> {
    let (sender, receiver) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        #[allow(deprecated)]
        let pixbuf = {
            let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
            clipboard.wait_for_image()
        };
        let encoded = pixbuf
            .and_then(|pixbuf| pixbuf.save_to_bufferv("png", &[]).ok())
            .and_then(encode_png);
        let _ = sender.send(encoded);
    })
    .ok()?;

    receiver
        .recv_timeout(std::time::Duration::from_millis(2500))
        .ok()
        .flatten()
}

/// Wayland (`wl-paste`) and X11 (`xclip`) helpers, used when GTK has nothing.
#[cfg(target_os = "linux")]
fn read_clipboard_with_tools() -> Option<BinaryFile> {
    use std::process::{Command, Stdio};

    let attempts: [(&str, &[&str]); 2] = [
        ("wl-paste", &["--no-newline", "--type", "image/png"]),
        ("xclip", &["-selection", "clipboard", "-t", "image/png", "-o"]),
    ];

    for (program, args) in attempts {
        let Ok(child) = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
        else {
            // Not installed: try the next one.
            continue;
        };
        if !child.status.success() || child.stdout.is_empty() {
            continue;
        }
        if let Some(image) = encode_png(child.stdout) {
            return Some(image);
        }
    }
    None
}

#[cfg(all(test, target_os = "linux"))]
mod clipboard_tests {
    use super::*;

    /// Needs a display and an image already on the clipboard.
    #[test]
    #[ignore]
    fn gtk_reads_an_image_from_the_clipboard() {
        gtk::init().ok();
        let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
        #[allow(deprecated)]
        let image = clipboard.wait_for_image();
        assert!(image.is_some(), "no hay imagen en el portapapeles");
        let bytes = image.unwrap().save_to_bufferv("png", &[]).unwrap();
        assert!(encode_png(bytes).is_some());
    }

    /// The helper path used when GTK has nothing (Wayland compositors, bare X).
    #[test]
    #[ignore]
    fn the_tools_fall_back_reads_the_clipboard() {
        let image = read_clipboard_with_tools().expect("ni wl-paste ni xclip devolvieron una imagen");
        assert_eq!(image.mime, "image/png");
        assert!(!image.data.is_empty());
    }

    #[test]
    fn a_non_png_answer_is_refused() {
        assert!(encode_png(b"not an image".to_vec()).is_none());
        assert!(encode_png(vec![0x89, b'P', b'N', b'G', 0]).is_some());
    }
}

#[tauri::command]
pub async fn read_file_binary(path: String) -> Result<BinaryFile, String> {
    use base64::Engine;
    use std::io::Read;

    const MAX_BYTES: u64 = 12 * 1024 * 1024;
    let meta = std::fs::metadata(&path).map_err(|e| format!("Could not open {path}: {e}"))?;
    if meta.len() > MAX_BYTES {
        return Err("El archivo supera 12 MB".into());
    }

    let mut file = std::fs::File::open(&path).map_err(|e| format!("Could not open {path}: {e}"))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Could not read {path}: {e}"))?;

    Ok(BinaryFile {
        mime: mime_for(&path).to_string(),
        data: base64::engine::general_purpose::STANDARD.encode(buffer),
    })
}

// ── Abrir en app externa ──

#[derive(serde::Serialize, Clone)]
pub struct OpenAppInfo {
    pub id: String,
    pub label: String,
    /// Program icon as a data URL (PNG/SVG in base64).
    pub icon: Option<String>,
    pub kind: String,
}

#[derive(Clone)]
struct AppEntry {
    id: String,
    label: String,
    /// .desktop entry argv, with the %f/%F/%u/%U placeholders unsubstituted.
    argv: Vec<String>,
    /// Executable that receives the path (entries without a file placeholder).
    bin: Option<String>,
    icon: Option<String>,
    priority: u8,
    kind: String,
}

fn resolve_bin(token: &str) -> Option<String> {
    if token.is_empty() {
        return None;
    }
    if token.starts_with('/') {
        return std::path::Path::new(token).is_file().then(|| token.to_string());
    }
    if token.contains('/') {
        return None;
    }
    let out = std::process::Command::new("which").arg(token).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!path.is_empty()).then_some(path)
}

fn bin_key(token: &str) -> Option<String> {
    let path = if token.starts_with('/') {
        std::path::PathBuf::from(token)
    } else {
        std::path::PathBuf::from(resolve_bin(token)?)
    };
    Some(
        std::fs::canonicalize(&path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string(),
    )
}

fn slug(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Icon directories ordered by preferred size (best first).
fn icon_dirs() -> Vec<std::path::PathBuf> {
    let mut roots: Vec<std::path::PathBuf> = vec![
        "/usr/share/icons".into(),
        "/usr/local/share/icons".into(),
        "/var/lib/flatpak/exports/share/icons".into(),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        let home = std::path::PathBuf::from(home);
        roots.push(home.join(".local/share/icons"));
        roots.push(home.join(".local/share/flatpak/exports/share/icons"));
    }

    let mut dirs = Vec::new();
    for root in &roots {
        let Ok(themes) = std::fs::read_dir(root) else {
            continue;
        };
        for theme in themes.flatten() {
            let theme_path = theme.path();
            for size in ["256x256", "128x128", "scalable", "96x96", "64x64", "48x48", "32x32"] {
                let dir = theme_path.join(size).join("apps");
                if dir.is_dir() {
                    dirs.push(dir);
                }
            }
        }
    }
    for pix in ["/usr/share/pixmaps", "/usr/local/share/pixmaps"] {
        let path = std::path::Path::new(pix);
        if path.is_dir() {
            dirs.push(path.to_path_buf());
        }
    }
    dirs
}

fn find_icon(dirs: &[std::path::PathBuf], names: &[&str]) -> Option<std::path::PathBuf> {
    for name in names {
        for dir in dirs {
            for ext in ["svg", "png"] {
                let candidate = dir.join(format!("{name}.{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn icon_data_url(path: &std::path::Path) -> Option<String> {
    use base64::Engine;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mime = match ext.as_str() {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        _ => return None,
    };

    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > 512 * 1024 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// The Icon= value can be a theme name or an absolute path.
fn icon_from_value(dirs: &[std::path::PathBuf], value: &str) -> Option<String> {
    let path = std::path::Path::new(value);
    if path.is_absolute() {
        return path.is_file().then(|| icon_data_url(path)).flatten();
    }
    // Ojo: muchos nombres de icono llevan puntos ("com.example.app", "org.gnome.Nautilus").
    // Only treated as a file if the extension is an image one.
    let is_file = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_lowercase().as_str(), "svg" | "png" | "xpm" | "jpg" | "jpeg"))
        .unwrap_or(false);

    if is_file {
        for dir in dirs {
            let candidate = dir.join(value);
            if candidate.is_file() {
                return icon_data_url(&candidate);
            }
        }
        return None;
    }
    find_icon(dirs, &[value]).and_then(|p| icon_data_url(&p))
}

fn split_exec(exec: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = exec.chars().peekable();

    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else if c == '\\' && q == '"' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else {
                    current.push(c);
                }
            }
            None => match c {
                '\'' | '"' => quote = Some(c),
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                c if c.is_whitespace() => {
                    if !current.is_empty() {
                        out.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(c),
            },
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn has_placeholder(arg: &str) -> bool {
    arg.contains("%f") || arg.contains("%F") || arg.contains("%u") || arg.contains("%U")
}

fn substitute(arg: &str, path: &str) -> Option<String> {
    let mut out = String::new();
    let mut inserted = false;
    let mut chars = arg.chars();

    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('f') | Some('F') | Some('u') | Some('U') => {
                out.push_str(path);
                inserted = true;
            }
            // %i %c %k %d %D %n %N %v %m no aportan nada al lanzar
            Some(_) => {}
            None => out.push('%'),
        }
    }

    if out.is_empty() && !inserted {
        None
    } else {
        Some(out)
    }
}

struct DesktopEntry {
    id: String,
    name: String,
    exec: String,
    icon: Option<String>,
    categories: String,
}

fn parse_desktop(path: &std::path::Path) -> Option<DesktopEntry> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let mut name: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut icon: Option<String> = None;
    let mut categories = String::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Name" => {
                if name.is_none() {
                    name = Some(value.to_string())
                }
            }
            "Exec" => {
                if exec.is_none() {
                    exec = Some(value.to_string())
                }
            }
            "Icon" => {
                if icon.is_none() {
                    icon = Some(value.to_string())
                }
            }
            "Categories" => categories = value.to_string(),
            "NoDisplay" | "Hidden" | "Terminal" => {
                if value == "true" {
                    return None;
                }
            }
            _ => {}
        }
    }

    Some(DesktopEntry {
        id: path.file_stem()?.to_string_lossy().to_string(),
        name: name?,
        exec: exec?,
        icon,
        categories,
    })
}

fn desktop_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home = std::path::PathBuf::from(home);
        dirs.push(home.join(".local/share/applications"));
        dirs.push(home.join(".local/share/flatpak/exports/share/applications"));
    }
    dirs.push("/usr/local/share/applications".into());
    dirs.push("/usr/share/applications".into());
    dirs.push("/var/lib/flatpak/exports/share/applications".into());
    dirs
}

fn desktop_files() -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for dir in desktop_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if seen.insert(name) {
                files.push(path);
            }
        }
    }
    files
}

fn classify_entry(name: &str, id: &str, categories: &str) -> Option<(&'static str, u8)> {
    let lower_name = name.to_lowercase();
    let lower_id = id.to_lowercase();
    let cats = categories.to_lowercase();

    // 1. Navegadores Web
    if cats.contains("webbrowser")
        || ["chrome", "firefox", "brave", "chromium", "edge", "vivaldi", "epiphany", "zen", "safari"]
            .iter().any(|b| lower_name.contains(b) || lower_id.contains(b))
    {
        return Some(("browser", 1));
    }

    // 2. Visores y editores de Imagen / Gráficos
    if cats.contains("rastergraphics") || cats.contains("vectorgraphics") || cats.contains("photography") || cats.contains("2dgraphics")
        || ["photocraft", "vectorcraft", "gimp", "inkscape", "photoshop", "krita", "loupe", "eog", "gwenview", "kolourpaint", "preview"]
            .iter().any(|b| lower_name.contains(b) || lower_id.contains(b))
    {
        return Some(("image", 1));
    }

    // 3. Vídeo / Audio
    if cats.contains("audiovideo") || cats.contains("video") || cats.contains("player")
        || ["filmcraft", "drift", "effectcraft", "vlc", "mpv", "totem", "kdenlive", "shotcut"]
            .iter().any(|b| lower_name.contains(b) || lower_id.contains(b))
    {
        return Some(("video", 1));
    }

    // 4. Editores de código / IDEs
    if cats.contains("texteditor") || cats.contains("ide")
        || ["cursor", "windsurf", "code", "vscode", "vscodium", "zed", "sublime", "antigravity", "kate", "gedit", "kwrite", "geany", "emacs", "neovim", "intellij", "pycharm", "clion"]
            .iter().any(|b| lower_name.contains(b) || lower_id.contains(b))
    {
        return Some(("editor", 1));
    }

    None
}

/// Escanea aplicaciones instaladas clasificadas por categoría (.desktop en Linux)
fn scan_desktop_apps(
    dirs: &[std::path::PathBuf],
    seen_bins: &mut std::collections::HashSet<String>,
    seen_labels: &mut std::collections::HashSet<String>,
) -> Vec<AppEntry> {
    let mut out = Vec::new();

    for path in desktop_files() {
        let Some(entry) = parse_desktop(&path) else {
            continue;
        };
        let Some((kind, priority)) = classify_entry(&entry.name, &entry.id, &entry.categories) else {
            continue;
        };

        let argv = split_exec(&entry.exec);
        if argv.is_empty() {
            continue;
        }

        let placeholder = argv.iter().any(|a| has_placeholder(a));

        let cli = {
            let mut candidates = vec![entry.id.clone()];
            if let Some(stem) = std::path::Path::new(&argv[0]).file_stem() {
                candidates.push(stem.to_string_lossy().to_string());
            }
            candidates.into_iter().find(|c| resolve_bin(c).is_some())
        };

        let bin = match cli {
            Some(bin) => Some(bin),
            None if placeholder => None,
            None => continue,
        };

        if bin.is_none() {
            let first = &argv[0];
            if resolve_bin(first).is_none() && !std::path::Path::new(first).is_file() {
                continue;
            }
        }

        let key = match &bin {
            Some(bin) => bin_key(bin),
            None => bin_key(&argv[0]),
        };
        if let Some(key) = key {
            if !seen_bins.insert(key) {
                continue;
            }
        }
        if !seen_labels.insert(slug(&entry.name)) {
            continue;
        }

        out.push(AppEntry {
            id: entry.id.clone(),
            label: entry.name.clone(),
            argv,
            bin,
            icon: entry.icon.as_deref().and_then(|v| icon_from_value(dirs, v)),
            priority,
            kind: kind.to_string(),
        });
    }

    out
}

/// Fallback multiplataforma para aplicaciones populares sin .desktop o en otras rutas
fn fallback_apps(
    dirs: &[std::path::PathBuf],
    seen_bins: &mut std::collections::HashSet<String>,
    seen_labels: &mut std::collections::HashSet<String>,
    seen_names: &std::collections::HashSet<String>,
) -> Vec<AppEntry> {
    const FALLBACK: &[(&str, &str, &[&str], &[&str], &str)] = &[
        // Navegadores
        ("chrome", "Google Chrome", &["google-chrome", "google-chrome-stable", "com.google.Chrome"], &["google-chrome", "chrome"], "browser"),
        ("firefox", "Firefox", &["firefox", "org.mozilla.firefox"], &["firefox", "firefox-esr"], "browser"),
        ("brave", "Brave", &["brave-browser", "brave"], &["brave-browser", "brave"], "browser"),
        ("chromium", "Chromium", &["chromium", "chromium-browser"], &["chromium", "chromium-browser"], "browser"),
        ("edge", "Microsoft Edge", &["microsoft-edge", "microsoft-edge-stable"], &["microsoft-edge"], "browser"),
        ("epiphany", "GNOME Web", &["epiphany", "org.gnome.Epiphany"], &["org.gnome.Epiphany", "epiphany"], "browser"),
        // Editores de imagen / gráficos (incluye creativas de AGENTS.md)
        ("photocraft", "PhotoCraft", &["/usr/bin/photocraft", "photocraft"], &["photocraft"], "image"),
        ("vectorcraft", "VectorCraft", &["/usr/bin/vectorcraft", "vectorcraft"], &["vectorcraft"], "image"),
        ("gimp", "GIMP", &["gimp"], &["gimp"], "image"),
        ("inkscape", "Inkscape", &["inkscape"], &["inkscape"], "image"),
        ("loupe", "Visor de imágenes", &["loupe", "eog"], &["org.gnome.Loupe", "eog"], "image"),
        // Video / Audio (incluye creativas de AGENTS.md)
        ("filmcraft", "FilmCraft", &["/usr/bin/filmcraft", "filmcraft"], &["filmcraft"], "video"),
        ("drift", "Drift", &["/usr/bin/drift", "drift"], &["drift"], "video"),
        ("effectcraft", "EffectCraft", &["/usr/bin/effectcraft", "effectcraft"], &["effectcraft"], "video"),
        ("vlc", "VLC media player", &["vlc"], &["vlc"], "video"),
        ("mpv", "mpv", &["mpv"], &["mpv"], "video"),
        // Editores de código / texto
        ("cursor", "Cursor", &["cursor"], &["cursor"], "editor"),
        ("windsurf", "Windsurf", &["windsurf"], &["windsurf"], "editor"),
        ("code", "Visual Studio Code", &["code", "codium"], &["code", "vscode", "vscodium"], "editor"),
        ("zed", "Zed", &["zed"], &["zed"], "editor"),
        ("sublime", "Sublime Text", &["subl"], &["sublime-text"], "editor"),
        ("antigravity", "Antigravity", &["antigravity"], &["antigravity"], "editor"),
        ("kate", "Kate", &["kate"], &["kate"], "editor"),
        ("gedit", "Editor de texto", &["gnome-text-editor", "gedit"], &["org.gnome.TextEditor", "gedit"], "editor"),
        ("geany", "Geany", &["geany"], &["geany"], "editor"),
        ("emacs", "Emacs", &["emacs"], &["emacs"], "editor"),
        ("meld", "Meld", &["meld"], &["meld"], "editor"),
    ];

    let mut out = Vec::new();
    for (id, label, bins, icons, kind) in FALLBACK {
        let Some(bin) = bins.iter().find_map(|b| resolve_bin(b)) else {
            continue;
        };
        if bins.iter().any(|b| seen_names.contains(*b)) {
            continue;
        }
        if let Some(key) = bin_key(&bin) {
            if !seen_bins.insert(key) {
                continue;
            }
        }
        if !seen_labels.insert(slug(label)) {
            continue;
        }
        out.push(AppEntry {
            id: (*id).to_string(),
            label: (*label).to_string(),
            argv: vec![bin.clone()],
            bin: Some(bin),
            icon: find_icon(dirs, icons).and_then(|p| icon_data_url(&p)),
            priority: 1,
            kind: (*kind).to_string(),
        });
    }
    out
}

fn detect_apps_sync() -> Vec<AppEntry> {
    let dirs = icon_dirs();
    let mut seen_bins = std::collections::HashSet::new();
    let mut seen_labels = std::collections::HashSet::new();

    let mut apps = scan_desktop_apps(&dirs, &mut seen_bins, &mut seen_labels);

    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for app in &apps {
        let token = app.bin.clone().or_else(|| app.argv.first().cloned());
        if let Some(token) = token {
            if let Some(name) = std::path::Path::new(&token).file_name() {
                seen_names.insert(name.to_string_lossy().to_string());
            }
        }
    }
    apps.extend(fallback_apps(&dirs, &mut seen_bins, &mut seen_labels, &seen_names));
    apps.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    apps
}

static APPS_CACHE: std::sync::OnceLock<Vec<AppEntry>> = std::sync::OnceLock::new();

fn cached_apps() -> &'static Vec<AppEntry> {
    APPS_CACHE.get_or_init(detect_apps_sync)
}

#[tauri::command]
pub async fn detect_open_apps(path: Option<String>) -> Result<Vec<OpenAppInfo>, String> {
    let dirs = icon_dirs();
    let mut apps = Vec::new();

    let ext = path
        .as_deref()
        .and_then(|p| std::path::Path::new(p).extension().and_then(|e| e.to_str()))
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    let is_web = matches!(ext.as_str(), "html" | "htm" | "xhtml" | "url");
    let is_image = matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "avif" | "tiff" | "tif" | "psd" | "ai" | "pcraft" | "vectorcraft"
    );
    let is_svg = ext == "svg";
    let is_video = matches!(ext.as_str(), "mp4" | "mkv" | "mov" | "avi" | "webm" | "flv" | "wmv" | "fcproj");
    let is_audio = matches!(ext.as_str(), "mp3" | "wav" | "ogg" | "flac" | "m4a" | "aac");

    let default_label = if is_web {
        "Navegador predeterminado"
    } else if is_image || is_svg {
        "Visor predeterminado"
    } else if is_video || is_audio {
        "Reproductor predeterminado"
    } else {
        "Aplicación predeterminada"
    };

    apps.push(OpenAppInfo {
        id: "system_default".into(),
        label: default_label.into(),
        icon: find_icon(&dirs, &["preferences-desktop-default-applications", "document-open"]).and_then(|p| icon_data_url(&p)),
        kind: "system".into(),
    });

    apps.push(OpenAppInfo {
        id: "finder".into(),
        label: "Gestor de archivos".into(),
        icon: find_icon(
            &dirs,
            &[
                "system-file-manager",
                "org.gnome.Nautilus",
                "nautilus",
                "dolphin",
                "thunar",
                "nemo",
                "caja",
                "pcmanfm",
            ],
        )
        .and_then(|p| icon_data_url(&p)),
        kind: "system".into(),
    });

    let all = cached_apps();

    let (primary_kind, secondary_kind): (Option<&str>, Option<&str>) = if is_web {
        (Some("browser"), Some("editor"))
    } else if is_svg {
        (Some("image"), Some("editor"))
    } else if is_image {
        (Some("image"), None)
    } else if is_video || is_audio {
        (Some("video"), None)
    } else {
        (Some("editor"), None)
    };

    if let Some(pk) = primary_kind {
        for app in all.iter().filter(|a| a.kind == pk) {
            apps.push(OpenAppInfo {
                id: app.id.clone(),
                label: app.label.clone(),
                icon: app.icon.clone(),
                kind: app.kind.clone(),
            });
        }
    }

    if let Some(sk) = secondary_kind {
        for app in all.iter().filter(|a| a.kind == sk) {
            apps.push(OpenAppInfo {
                id: app.id.clone(),
                label: app.label.clone(),
                icon: app.icon.clone(),
                kind: app.kind.clone(),
            });
        }
    }

    if primary_kind.is_none() {
        for app in all.iter() {
            apps.push(OpenAppInfo {
                id: app.id.clone(),
                label: app.label.clone(),
                icon: app.icon.clone(),
                kind: app.kind.clone(),
            });
        }
    }

    Ok(apps)
}

#[tauri::command]
pub fn open_path(path: String, app: Option<String>) -> Result<(), String> {
    let target = std::path::PathBuf::from(&path);
    let parent = target
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| target.clone());

    let Some(id) = app.as_deref() else {
        return open::that(&target).map_err(|e| format!("Could not open file: {e}"));
    };
    if id == "system_default" {
        return open::that(&target).map_err(|e| format!("Could not open file: {e}"));
    }
    if id == "finder" {
        return open::that(&parent).map_err(|e| format!("Could not open the folder: {e}"));
    }

    let entry = cached_apps()
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| format!("Application not available: {id}"))?;

    let path_str = target.to_string_lossy().to_string();

    if let Some(bin) = &entry.bin {
        return std::process::Command::new(bin)
            .arg(&target)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not open with {bin}: {e}"));
    }

    let Some(program) = entry.argv.first() else {
        return Err("The application has no command".into());
    };
    let mut command = std::process::Command::new(program);
    for arg in entry.argv.iter().skip(1) {
        if let Some(value) = substitute(arg, &path_str) {
            command.arg(value);
        }
    }

    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open with {program}: {e}"))
}

// ── Visor de archivos: metadatos y descarga ──

#[derive(serde::Serialize)]
pub struct FileInfo {
    pub size: u64,
}

#[tauri::command]
pub fn file_info(path: String) -> Result<FileInfo, String> {
    let meta = std::fs::metadata(&path).map_err(|e| format!("Could not read {path}: {e}"))?;
    Ok(FileInfo { size: meta.len() })
}

#[tauri::command]
pub fn write_text_file(app: AppHandle, path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| format!("Could not save {path}: {e}"))?;
    use tauri::Emitter;
    let _ = app.emit("app://workspace-changed", ());
    Ok(())
}

#[cfg(test)]
mod open_app_tests {
    use super::*;

    // Checks the real detector against the system it runs on.
    #[test]
    fn detecta_apps_instaladas() {
        for app in detect_apps_sync() {
            println!(
                "p{} {:<28} id={:<34} icono={} lanzamiento={}",
                app.priority,
                app.label,
                app.id,
                app.icon.as_ref().map(|i| format!("{} B", i.len())).unwrap_or_else(|| "—".into()),
                if app.bin.is_some() { "bin+ruta" } else { "exec" }
            );
        }

        for app in detect_apps_sync() {
            assert!(!app.label.is_empty());
            assert!(app.bin.is_some() || !app.argv.is_empty());
        }
    }
}

// ── Git review ──

#[derive(serde::Serialize)]
pub struct GitFile {
    pub path: String,
    /// Letra corta: M, A, D, R, ? …
    pub status: String,
    /// There are staged changes.
    pub staged: bool,
    pub untracked: bool,
}

#[derive(serde::Serialize)]
pub struct GitStatus {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub files: Vec<GitFile>,
}

fn git(project: &str, args: &[&str]) -> Result<std::process::Output, String> {
    std::process::Command::new("git")
        .arg("-C")
        .arg(project)
        .args(args)
        .output()
        .map_err(|e| format!("Could not run git: {e}"))
}

#[tauri::command]
pub fn git_status(project: String) -> Result<GitStatus, String> {
    let inside = git(&project, &["rev-parse", "--is-inside-work-tree"])?;
    if !inside.status.success() {
        return Ok(GitStatus {
            is_repo: false,
            branch: None,
            files: Vec::new(),
        });
    }

    let branch = git(&project, &["branch", "--show-current"])
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());

    let output = git(&project, &["status", "--porcelain=v1", "-uall", "--no-renames"])?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let mut files = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.len() < 4 {
            continue;
        }
        let x = line.as_bytes()[0] as char;
        let y = line.as_bytes()[1] as char;
        let path = line[3..].trim().to_string();
        let untracked = x == '?' && y == '?';
        let status = if untracked {
            "?".to_string()
        } else if y != ' ' {
            y.to_string()
        } else {
            x.to_string()
        };
        files.push(GitFile {
            path,
            status,
            staged: x != ' ' && x != '?',
            untracked,
        });
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(GitStatus {
        is_repo: true,
        branch,
        files,
    })
}

#[tauri::command]
pub fn git_diff(project: String, file: String, untracked: bool) -> Result<String, String> {
    const MAX_BYTES: usize = 400 * 1024;

    if untracked {
        let full = std::path::Path::new(&project).join(&file);
        let bytes = std::fs::read(&full).map_err(|e| format!("Could not read {file}: {e}"))?;
        let truncated = bytes.len() > MAX_BYTES;
        let bytes = &bytes[..bytes.len().min(MAX_BYTES)];
        let text = String::from_utf8_lossy(bytes).to_string();
        let mut out = String::from("--- /dev/null\n+++ b/");
        out.push_str(&file);
        out.push_str("\n@@ archivo nuevo @@\n");
        for line in text.lines() {
            out.push('+');
            out.push_str(line);
            out.push('\n');
        }
        if truncated {
            out.push_str("\n… vista truncada\n");
        }
        return Ok(out);
    }

    // HEAD may not exist yet (fresh repo).
    let output = match git(&project, &["diff", "--no-color", "--no-ext-diff", "HEAD", "--", &file]) {
        Ok(out) if out.status.success() => out,
        _ => git(&project, &["diff", "--no-color", "--no-ext-diff", "--", &file])?,
    };

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.len() > MAX_BYTES {
        text.truncate(MAX_BYTES);
        text.push_str("\n… vista truncada\n");
    }
    Ok(text)
}

#[tauri::command]
pub fn git_init(project: String) -> Result<(), String> {
    let output = git(&project, &["init"])?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn git_discard_inner(project: &str, file: &str, untracked: bool) -> Result<(), String> {
    let full = std::path::Path::new(project).join(file);

    if untracked {
        std::fs::remove_file(&full).map_err(|e| format!("Could not delete {file}: {e}"))
    } else {
        let output = git(project, &["restore", "--staged", "--worktree", "--", file])?;
        if output.status.success() {
            Ok(())
        } else {
            let output = git(project, &["checkout", "--", file])?;
            if output.status.success() {
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
            }
        }
    }
}

#[tauri::command]
pub fn git_discard(app: AppHandle, project: String, file: String, untracked: bool) -> Result<(), String> {
    let res = git_discard_inner(&project, &file, untracked);
    if res.is_ok() {
        use tauri::Emitter;
        let _ = app.emit("app://workspace-changed", ());
    }
    res
}

#[cfg(test)]
mod git_tests {
    use super::*;

    #[test]
    fn estado_diff_e_init_de_un_repo_temporal() {
        let dir = std::env::temp_dir().join(format!("ianox-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let project = dir.to_string_lossy().to_string();

        let before = git_status(project.clone()).unwrap();
        assert!(!before.is_repo);

        git_init(project.clone()).unwrap();
        std::fs::write(dir.join("hola.txt"), "uno\ndos\n").unwrap();

        let after = git_status(project.clone()).unwrap();
        assert!(after.is_repo);
        assert_eq!(after.files.len(), 1);
        assert!(after.files[0].untracked);
        assert_eq!(after.files[0].status, "?");

        let diff = git_diff(project.clone(), "hola.txt".into(), true).unwrap();
        assert!(diff.contains("+uno"));

        git_discard_inner(&project, "hola.txt", true).unwrap();
        assert!(!dir.join("hola.txt").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}

// ── Extensiones ──

#[tauri::command]
pub async fn list_skills(project: Option<String>) -> Result<Vec<crate::extensions::SkillInfo>, String> {
    Ok(crate::extensions::list_skills(project.as_deref()))
}

#[tauri::command]
pub async fn list_plugins(project: Option<String>) -> Result<Vec<crate::extensions::PluginInfo>, String> {
    Ok(crate::extensions::list_plugins(project.as_deref()))
}

#[tauri::command]
pub async fn detect_lsp_servers() -> Result<Vec<crate::extensions::LspInfo>, String> {
    Ok(crate::extensions::detect_lsp())
}

#[tauri::command]
pub async fn extensions_summary(
    project: Option<String>,
    state: State<'_, AppState>,
) -> Result<crate::extensions::ExtensionsSummary, String> {
    let config = state.config.read().await;
    let mcp = config.mcp_servers.len();
    Ok(crate::extensions::ExtensionsSummary {
        mcp,
        plugins: crate::extensions::list_plugins(project.as_deref()).len(),
        skills: crate::extensions::list_skills(project.as_deref()).len(),
        lsp: crate::extensions::detect_lsp().len(),
    })
}

#[cfg(test)]
mod display_name_tests {
    use super::display_name;

    #[test]
    fn prettifies_model_ids() {
        assert_eq!(display_name("deepseek-v4-pro"), "DeepSeek V4 Pro");
        assert_eq!(display_name("gpt-4o"), "GPT 4o");
        assert_eq!(display_name("llama3:8b"), "Llama3 8B");
    }
}

// ── Fonts ──

/// Family names of the fonts installed on the system, for the typography picker.
fn installed_font_families() -> Vec<String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut families: Vec<String> = db
        .faces()
        .filter_map(|face| face.families.first().map(|(name, _)| name.trim().to_string()))
        .filter(|name| !name.is_empty() && !name.starts_with('.'))
        .collect();
    families.sort_by_key(|name| name.to_lowercase());
    families.dedup();
    families
}

#[tauri::command]
pub async fn list_fonts() -> Result<Vec<String>, String> {
    // Scanning the system fonts touches the disk; keep it off the async runtime threads.
    tokio::task::spawn_blocking(installed_font_families)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod font_tests {
    #[test]
    fn families_are_sorted_and_unique() {
        let families = super::installed_font_families();
        let mut sorted = families.clone();
        sorted.sort_by_key(|name| name.to_lowercase());
        sorted.dedup();
        assert_eq!(families, sorted);
    }
}

// ── Conversation titles ──

/// Turns the model's reply into a tidy one-line title.
fn clean_title(raw: &str) -> String {
    let line = raw.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let line = line
        .trim_start_matches(|c: char| c == '#' || c.is_whitespace())
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '*' | '“' | '”' | '«' | '»'));
    let lower = line.to_lowercase();
    let line = ["title:", "título:", "titulo:"]
        .iter()
        .find(|prefix| lower.starts_with(*prefix))
        .map(|prefix| line[prefix.len()..].trim())
        .unwrap_or(line);
    let line = line.trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '*' | '“' | '”' | '«' | '»'));
    let line = line.trim_end_matches(|c: char| matches!(c, '.' | ':' | ',' | ';' | '!'));
    line.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(60).collect()
}

const TITLE_SYSTEM: &str = "You name chat conversations. You receive the user's first message inside <message> tags, and sometimes the start of the assistant's reply inside <reply> tags, only as context.

Write a title that names the TOPIC or TASK of the conversation, in the same language as the user's message.

Rules:
- 2 to 5 words, as a noun phrase. Not a sentence and not a question.
- Never include greetings, and never use the words \"user\", \"assistant\" or \"conversation\".
- Do not answer or carry out the message: you are only naming it.
- Keep the user's own key terms and do not add names, places or details that are not in the message.\n- Use the reply only when the message says nothing by itself (for example just \"hola\"); then name what the reply is about.
- Reply with the title alone: no quotes, no final punctuation.

Examples:
<message>Hola, ¿puedes ayudarme a configurar nginx como proxy inverso?</message> -> Configurar nginx como proxy inverso
<message>arregla el error de compilación en main.rs</message> -> Error de compilación en main.rs
<message>how do I reverse a list in python</message> -> Reverse a list in Python";

/// Builds the request that names a conversation from its first exchange.
fn title_messages(user_text: &str, assistant_text: &str) -> Vec<serde_json::Value> {
    let clip = |text: &str, max: usize| text.trim().chars().take(max).collect::<String>();
    let mut prompt = format!("<message>\n{}\n</message>", clip(user_text, 600));
    let reply = clip(assistant_text, 250);
    if !reply.is_empty() {
        prompt.push_str(&format!("\n\n<reply>\n{reply}\n</reply>"));
    }
    prompt.push_str("\n\nTitle:");
    vec![
        serde_json::json!({ "role": "system", "content": TITLE_SYSTEM }),
        serde_json::json!({ "role": "user", "content": prompt }),
    ]
}

/// Asks the model for a short title once the first exchange of a conversation is done.
#[tauri::command]
pub async fn generate_session_title(
    state: State<'_, AppState>,
    model: String,
    provider_id: Option<String>,
    user_text: String,
    assistant_text: String,
) -> Result<String, String> {
    let config = state.config.read().await.clone();
    let provider = pick_provider(&config, &model, provider_id.as_deref())
        .ok_or_else(|| format!("No provider configured for model '{model}'"))?;

    let messages = title_messages(&user_text, &assistant_text);
    // Reasoning models spend part of the budget thinking; leave room for the title itself.
    let raw = crate::router::complete_text(&provider, &model, &messages, 1024).await?;
    let title = clean_title(&raw);
    if title.is_empty() {
        return Err("Empty title".into());
    }
    Ok(title)
}

// ── Project icons ──

/// Files that usually hold a project's icon, best first.
const ICON_CANDIDATES: &[&str] = &[
    "icon.svg", "icon.png", "logo.svg", "logo.png", "favicon.svg", "favicon.png", "favicon.ico",
    "public/favicon.svg", "public/icon.svg", "public/logo.svg", "public/favicon.png",
    "public/icon.png", "public/logo.png", "public/favicon.ico",
    "static/favicon.svg", "static/favicon.png", "static/favicon.ico",
    "src/assets/logo.svg", "src/assets/logo.png", "src/assets/icon.svg", "src/assets/icon.png",
    "assets/icon.svg", "assets/icon.png", "assets/logo.svg", "assets/logo.png",
    "branding/icon.svg", "branding/icon.png", "branding/logo.svg", "branding/logo.png",
    "src-tauri/icons/icon.png", "src-tauri/icons/128x128.png",
    "app/icon.svg", "app/icon.png", "app/favicon.ico", "build/icon.png", "resources/icon.png",
];

fn image_mime(path: &std::path::Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "png" => Some("image/png"),
        "svg" => Some("image/svg+xml"),
        "ico" => Some("image/x-icon"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn find_icon_data_url(root: &std::path::Path) -> Option<String> {
    use base64::Engine;
    const MAX_BYTES: u64 = 1024 * 1024;
    for candidate in ICON_CANDIDATES {
        let path = root.join(candidate);
        let Some(mime) = image_mime(&path) else { continue };
        let Ok(meta) = std::fs::metadata(&path) else { continue };
        if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_BYTES {
            continue;
        }
        if let Ok(bytes) = std::fs::read(&path) {
            let data = base64::engine::general_purpose::STANDARD.encode(bytes);
            return Some(format!("data:{mime};base64,{data}"));
        }
    }
    None
}

/// Looks for an icon inside a project folder; the frontend shrinks it before saving.
#[tauri::command]
pub async fn find_project_icon(path: String) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || find_icon_data_url(std::path::Path::new(&path)))
        .await
        .map_err(|e| e.to_string())
}

/// Saves a project's icon. An empty string means "no icon, do not detect one again".
#[tauri::command]
pub async fn set_project_icon(
    state: State<'_, AppState>,
    id: String,
    icon: Option<String>,
) -> Result<(), String> {
    if let Some(icon) = &icon {
        // The frontend sends a small image; refuse anything else so the config stays light.
        if !icon.is_empty() && (!icon.starts_with("data:image/") || icon.len() > 300_000) {
            return Err("The icon must be a small image".into());
        }
    }
    {
        let mut config = state.config.write().await;
        let project = config
            .projects
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| "Project not found".to_string())?;
        project.icon = icon;
    }
    let snapshot = state.config.read().await.clone();
    persist_config(&snapshot).await
}

#[cfg(test)]
mod title_and_icon_tests {
    use super::*;

    #[test]
    fn cleans_model_titles() {
        assert_eq!(clean_title("\"Revisar el selector de modelos.\""), "Revisar el selector de modelos");
        assert_eq!(clean_title("Título: **Arreglar el chat**\nExplicación…"), "Arreglar el chat");
        assert_eq!(clean_title("  \n  Plan de migración  "), "Plan de migración");
        assert_eq!(clean_title(&"x".repeat(100)).chars().count(), 60);
    }

    #[test]
    fn title_prompt_frames_the_message_and_keeps_the_reply_short() {
        let long_reply = "r".repeat(1000);
        let messages = title_messages("  Hola, ¿qué proyecto hay abierto? ", &long_reply);
        let prompt = messages[1]["content"].as_str().unwrap();
        assert!(prompt.starts_with("<message>\nHola, ¿qué proyecto hay abierto?\n</message>"));
        let reply = prompt.split("<reply>\n").nth(1).unwrap().split("\n</reply>").next().unwrap();
        assert_eq!(reply.chars().count(), 250); // clipped
        assert!(prompt.ends_with("Title:"));
        // Without a reply there is no <reply> block at all.
        assert!(!title_messages("hola", "")[1]["content"].as_str().unwrap().contains("<reply>"));
    }

    #[test]
    fn finds_a_project_icon_in_known_places() {
        let dir = std::env::temp_dir().join(format!("ianox-icon-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("public")).unwrap();
        assert!(find_icon_data_url(&dir).is_none());
        std::fs::write(dir.join("public/favicon.svg"), "<svg xmlns='http://www.w3.org/2000/svg'/>").unwrap();
        let url = find_icon_data_url(&dir).unwrap();
        assert!(url.starts_with("data:image/svg+xml;base64,"));
        std::fs::remove_dir_all(&dir).ok();
    }
}

// ── Managing the models of a provider ──

#[derive(serde::Serialize)]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub context: Option<u64>,
    pub free: bool,
}

/// Name the models.dev catalog uses for a provider of ours.
fn catalog_key(provider: &crate::config::ProviderConfig) -> String {
    use crate::config::ProviderKind::*;
    match provider.kind {
        OpenAI => "openai".into(),
        Anthropic => "anthropic".into(),
        OpenRouter => "openrouter".into(),
        Google => "google".into(),
        // Custom, Ollama and Azure providers are named by the user: their id is the best guess.
        _ => provider.id.clone(),
    }
}

/// The catalog's models for each enabled provider, so the user can switch them on or off.
#[tauri::command]
pub async fn list_catalog_models(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<Vec<CatalogModel>, String> {
    let config = state.config.read().await;
    let provider = config
        .providers
        .get(&provider_id)
        .ok_or_else(|| "Provider not found".to_string())?;
    // An exact id match wins over the kind (a custom "deepseek" provider uses the deepseek list).
    let mut listed = crate::catalog::models_of(&provider_id);
    if listed.is_empty() {
        listed = crate::catalog::models_of(&catalog_key(provider));
    }
    Ok(listed
        .into_iter()
        .map(|(id, entry)| CatalogModel {
            free: crate::catalog::is_free(&entry),
            id,
            name: entry.name,
            context: entry.context,
        })
        .collect())
}

/// Replaces the models a provider serves (the checkboxes of the manage view).
#[tauri::command]
pub async fn set_provider_models(
    state: State<'_, AppState>,
    provider_id: String,
    models: Vec<String>,
) -> Result<(), String> {
    {
        let mut config = state.config.write().await;
        let provider = config
            .providers
            .get_mut(&provider_id)
            .ok_or_else(|| "Provider not found".to_string())?;
        let mut seen = std::collections::HashSet::new();
        provider.models = models.into_iter().filter(|m| !m.trim().is_empty() && seen.insert(m.clone())).collect();
    }
    let snapshot = state.config.read().await.clone();
    persist_config(&snapshot).await
}

// ── Project files for `@` mentions ──

/// Orders project files for a typed query: name matches first, then path matches, then loose
/// letter-by-letter matches. An empty query lists the shallowest files.
fn rank_files(files: &[String], query: &str, limit: usize) -> Vec<String> {
    let needle = query.trim().to_lowercase();
    let mut scored: Vec<(u8, usize, usize, &String)> = Vec::new();
    for file in files {
        let lower = file.to_lowercase();
        let name = lower.rsplit('/').next().unwrap_or(&lower);
        let score = if needle.is_empty() {
            0
        } else if name.starts_with(&needle) {
            0
        } else if name.contains(&needle) {
            1
        } else if lower.contains(&needle) {
            2
        } else {
            // every letter of the query appears in order somewhere in the path
            let mut rest = needle.chars();
            let mut next = rest.next();
            for ch in lower.chars() {
                if Some(ch) == next {
                    next = rest.next();
                }
            }
            if next.is_none() { 3 } else { continue }
        };
        let depth = file.matches('/').count();
        let tie = if needle.is_empty() { depth } else { name.len() };
        scored.push((score, tie, file.len(), file));
    }
    scored.sort_by(|a, b| (a.0, a.1, a.2, a.3).cmp(&(b.0, b.1, b.2, b.3)));
    scored.into_iter().take(limit).map(|(_, _, _, f)| f.clone()).collect()
}

/// Files of the active project matching `query`, as paths relative to the project.
#[tauri::command]
pub async fn find_project_files(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<String>, String> {
    let root = {
        let config = state.config.read().await;
        config
            .projects
            .iter()
            .find(|p| config.active_project.as_deref() == Some(p.id.as_str()))
            .map(|p| p.path.clone())
    }
    .ok_or_else(|| "No project is open".to_string())?;

    tokio::task::spawn_blocking(move || {
        let root = std::path::PathBuf::from(root);
        let mut files = Vec::new();
        crate::tools::walk(&root, &mut files, 0);
        let relative: Vec<String> = files
            .iter()
            .filter_map(|p| p.strip_prefix(&root).ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        rank_files(&relative, &query, limit.unwrap_or(30).min(100))
    })
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod mention_tests {
    use super::rank_files;

    fn files() -> Vec<String> {
        ["README.md", "src/main.rs", "src/pages/Chat.svelte", "src/components/ChatInput.svelte", "docs/chat-notes.md", "package.json"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn name_matches_come_before_path_matches() {
        let ranked = rank_files(&files(), "chat", 10);
        assert_eq!(ranked[0], "src/pages/Chat.svelte"); // name starts with it, shortest
        assert!(ranked.contains(&"docs/chat-notes.md".to_string()));
        assert!(!ranked.contains(&"package.json".to_string()));
    }

    #[test]
    fn loose_matches_and_empty_query() {
        assert!(rank_files(&files(), "smr", 10).contains(&"src/main.rs".to_string()));
        let first = rank_files(&files(), "", 2);
        assert_eq!(first, vec!["README.md".to_string(), "package.json".to_string()]); // shallowest first
        assert!(rank_files(&files(), "zzzz", 10).is_empty());
    }
}

// ── Context compaction ──

const SUMMARY_SYSTEM: &str = "You compress a long conversation between a user and a coding assistant so that work can continue from the summary alone. The assistant that reads it will have nothing else.

Write the summary in the language the user wrote in, as plain Markdown with these sections (skip a section only if it is empty):
## Goal
What the user is trying to achieve.
## Instructions and preferences
Rules, constraints and style the user asked for; things that must not change.
## Done so far
What was done and decided, with the real file paths, commands and names involved.
## Current state
Where things stand now, including anything broken, unverified or half-finished.
## Next steps
What remains, in order.
## Facts to keep
Exact values that would be costly to rediscover: paths, ids, versions, error messages.

Be concrete and complete but compact (under 700 words). Do not add commentary, and do not answer or continue the conversation.";

/// Summarises a conversation so the chat can replace its history with the summary.
#[tauri::command]
pub async fn summarize_conversation(
    state: State<'_, AppState>,
    model: String,
    provider_id: Option<String>,
    transcript: String,
) -> Result<String, String> {
    let config = state.config.read().await.clone();
    let provider = pick_provider(&config, &model, provider_id.as_deref())
        .ok_or_else(|| format!("No provider configured for model '{model}'"))?;
    let messages = vec![
        serde_json::json!({ "role": "system", "content": SUMMARY_SYSTEM }),
        serde_json::json!({
            "role": "user",
            "content": format!("<conversation>\n{transcript}\n</conversation>\n\nWrite the summary now.")
        }),
    ];
    // Reasoning models spend part of the budget thinking, and the summary itself is long.
    let summary = crate::router::complete_text(&provider, &model, &messages, 8192).await?;
    let summary = summary.trim().to_string();
    if summary.is_empty() {
        return Err("Empty summary".into());
    }
    Ok(summary)
}
