//! ianox configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    pub server: ServerConfig,
    pub providers: HashMap<String, ProviderConfig>,
    pub routes: Vec<RouteConfig>,
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    #[serde(default = "default_agents")]
    pub agents: Vec<AgentConfig>,
    #[serde(default)]
    pub projects: Vec<ProjectConfig>,
    #[serde(default)]
    pub active_project: Option<String>,
    pub ui: UiConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            server: ServerConfig::default(),
            providers: HashMap::new(),
            routes: Vec::new(),
            mcp_servers: HashMap::new(),
            agents: default_agents(),
            projects: Vec::new(),
            active_project: None,
            ui: UiConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub cors: bool,
    pub auth_token: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: "127.0.0.1".into(),
            port: 20128,
            cors: true,
            auth_token: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub api_key: Option<String>,
    pub models: Vec<String>,
    pub enabled: bool,
    pub priority: u8,
    pub rate_limit: Option<RateLimit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    OpenRouter,
    Ollama,
    Google,
    Azure,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub requests_per_minute: u32,
    pub tokens_per_minute: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub model_pattern: String,
    pub provider_ids: Vec<String>,
    pub fallback: bool,
    pub weight: u8,
}

/// Agente configurable (estilo OpenCode: build / plan).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    /// "build" (acceso total) o "plan" (solo lectura)
    pub mode: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub system_prompt: String,
}

pub fn default_agents() -> Vec<AgentConfig> {
    vec![
        AgentConfig {
            id: "build".into(),
            name: "Build".into(),
            mode: "build".into(),
            model: None,
            system_prompt: "You are a coding agent. You may read and modify files and run commands, asking for permission when needed.".into(),
        },
        AgentConfig {
            id: "plan".into(),
            name: "Plan".into(),
            mode: "plan".into(),
            model: None,
            system_prompt: "You are an analysis agent. You may read files and explore the project; do not modify anything or run commands.".into(),
        },
    ]
}

/// Carpeta de proyecto abierta en el workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub icon: Option<String>,
}

/// MCP server connected over stdio (same schema OpenCode uses).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub language: String,
    pub auto_start_server: bool,
    pub minimize_to_tray: bool,
    pub close_to_tray: bool,
    /// What closing the window or pressing Ctrl+Q does: "ask", "tray" or "quit".
    #[serde(default = "default_quit_action")]
    pub quit_action: String,
}

fn default_quit_action() -> String {
    "ask".into()
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            language: "en".into(),
            auto_start_server: false,
            minimize_to_tray: true,
            close_to_tray: true,
            quit_action: default_quit_action(),
        }
    }
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config: std::sync::Arc<tokio::sync::RwLock<Config>>,
    pub server_handle: std::sync::Arc<tokio::sync::Mutex<Option<crate::server::ServerHandle>>>,
    pub terminals: std::sync::Arc<tokio::sync::Mutex<HashMap<String, crate::terminal::TermSession>>>,
    pub chat_cancels: std::sync::Arc<
        tokio::sync::Mutex<HashMap<String, std::sync::Arc<std::sync::atomic::AtomicBool>>>,
    >,
    /// Questions the agent asked the user ("may I run this?"), waiting for an answer, by id.
    pub permission_waiters:
        std::sync::Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<String>>>>,
    /// "Allow for this conversation" answers already given, as `session:key`.
    pub allowances: std::sync::Arc<tokio::sync::Mutex<std::collections::HashSet<String>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            config: std::sync::Arc::new(tokio::sync::RwLock::new(Config::default())),
            server_handle: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            terminals: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            chat_cancels: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            permission_waiters: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            // Remembered across restarts: reopening ianox does not ask again.
            allowances: std::sync::Arc::new(tokio::sync::Mutex::new(crate::permissions::load())),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}