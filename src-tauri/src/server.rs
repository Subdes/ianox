//! Servidor HTTP embebido (estilo 9router / DeepSeek Harness)
//!
//! Expone API compatible con OpenAI/Anthropic/Ollama en localhost:20128
//! Gestiona rutas, proveedores, fallbacks, streaming SSE

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use axum::{
    Router,
    extract::{State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{get, post},
    Json,
};
use futures_util::stream;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use crate::config::{Config, ProviderConfig, ProviderKind, RouteConfig};

/// Handle to manage the server lifecycle
#[derive(Clone)]
pub struct ServerHandle {
    pub addr: SocketAddr,
    shutdown_tx: Arc<tokio::sync::Mutex<Option<mpsc::Sender<()>>>>,
}

impl ServerHandle {
    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }
    }
}

/// Estado compartido del servidor
#[derive(Clone)]
#[allow(dead_code)]
pub struct ServerState {
    pub config: Arc<RwLock<Config>>,
    pub providers: Arc<RwLock<HashMap<String, ProviderConfig>>>,
    pub routes: Arc<RwLock<Vec<RouteConfig>>>,
}

/// Inicia el servidor HTTP en background
pub async fn start_server(
    config: Arc<RwLock<Config>>,
) -> Result<ServerHandle, Box<dyn std::error::Error + Send + Sync>> {
    let cfg = config.read().await;
    let server_cfg = cfg.server.clone();
    drop(cfg);

    if !server_cfg.enabled {
        return Err("Server disabled in configuration".into());
    }

    let addr: SocketAddr = format!("{}:{}", server_cfg.host, server_cfg.port).parse()?;

    let state = ServerState {
        config: config.clone(),
        providers: Arc::new(RwLock::new(HashMap::new())),
        routes: Arc::new(RwLock::new(Vec::new())),
    };

    // Cargar proveedores y rutas desde config
    {
        let cfg = config.read().await;
        let mut provs = state.providers.write().await;
        for (k, v) in &cfg.providers {
            provs.insert(k.clone(), v.clone());
        }
        *state.routes.write().await = cfg.routes.clone();
    }

    let app = router(state, server_cfg.cors, server_cfg.auth_token.clone());

    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

    let handle = ServerHandle {
        addr,
        shutdown_tx: Arc::new(tokio::sync::Mutex::new(Some(shutdown_tx))),
    };

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown_rx.recv().await;
            })
            .await
            .inspect_err(|e| eprintln!("Error en servidor HTTP: {}", e))
            .ok();
    });

    println!("🌐 Servidor ianox escuchando en http://{}", addr);
    Ok(handle)
}

/// Construye el router principal
fn router(
    state: ServerState,
    cors: bool,
    auth_token: Option<String>,
) -> Router {
    let mut app = Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(list_models_openai))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/completions", post(completions))
        .route("/v1/embeddings", post(embeddings))
        .route("/api/tags", get(ollama_list_models))
        .route("/api/chat", post(ollama_chat))
        .route("/api/generate", post(ollama_generate))
        .route("/api/show", post(ollama_show))
        .route("/api/pull", post(ollama_pull))
        .route("/api/delete", post(ollama_delete))
        .route("/v1/messages", post(anthropic_messages))
        .with_state(state);

    if cors {
        app = app.layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        );
    }

    if let Some(token) = auth_token {
        app = app.layer(axum::middleware::from_fn_with_state(
            token,
            auth_middleware,
        ));
    }

    app
}

/// Simple authentication middleware (Bearer token)
async fn auth_middleware(
    State(expected_token): State<String>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if let Some(auth) = headers.get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if auth_str == format!("Bearer {}", expected_token) {
                return next.run(request).await;
            }
        }
    }
    (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid or missing token"}))).into_response()
}

async fn health() -> &'static str {
    "ok"
}

// ── OpenAI-compatible endpoints ──

#[derive(Deserialize)]
#[allow(dead_code)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    max_tokens: Option<u32>,
}

#[derive(Deserialize, Serialize)]
struct Message {
    role: String,
    content: Value,
}

async fn list_models_openai(State(state): State<ServerState>) -> impl IntoResponse {
    let providers = state.providers.read().await;
    let mut models = Vec::new();
    for (id, prov) in providers.iter() {
        if prov.enabled {
            for model in &prov.models {
                models.push(json!({
                    "id": model,
                    "object": "model",
                    "created": chrono::Utc::now().timestamp(),
                    "owned_by": id,
                }));
            }
        }
    }
    Json(json!({ "object": "list", "data": models }))
}

async fn chat_completions(
    State(state): State<ServerState>,
    Json(req): Json<ChatCompletionRequest>,
) -> impl IntoResponse {
    let provider = resolve_provider(&state, &req.model).await;

    if req.stream {
        return stream_response(state, provider, req).await;
    }

    match provider {
        Some(prov) => forward_to_provider(prov, req).await,
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "Model not found"}))).into_response(),
    }
}

async fn completions(
    State(_state): State<ServerState>,
    Json(_req): Json<Value>,
) -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "Not implemented"})))
}

async fn embeddings(
    State(_state): State<ServerState>,
    Json(_req): Json<Value>,
) -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "Not implemented"})))
}

// ── Ollama-compatible endpoints ──

async fn ollama_list_models(State(state): State<ServerState>) -> impl IntoResponse {
    let providers = state.providers.read().await;
    let mut models = Vec::new();
    for (_id, prov) in providers.iter() {
        if prov.enabled && matches!(prov.kind, ProviderKind::Ollama) {
            for model in &prov.models {
                models.push(json!({
                    "name": model,
                    "model": model,
                    "modified_at": chrono::Utc::now().to_rfc3339(),
                    "size": 0,
                    "digest": "",
                    "details": { "parent_model": "", "format": "gguf", "family": "", "families": [], "parameter_size": "", "quantization_level": "" }
                }));
            }
        }
    }
    Json(json!({ "models": models }))
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(default)]
    stream: bool,
    options: Option<Value>,
}

async fn ollama_chat(
    State(state): State<ServerState>,
    Json(req): Json<OllamaChatRequest>,
) -> impl IntoResponse {
    let provider = resolve_provider(&state, &req.model).await;
    match provider {
        Some(prov) => forward_ollama_chat(prov, req).await,
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "Model not found"}))).into_response(),
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    #[serde(default)]
    stream: bool,
    options: Option<Value>,
}

async fn ollama_generate(
    State(state): State<ServerState>,
    Json(req): Json<OllamaGenerateRequest>,
) -> impl IntoResponse {
    let provider = resolve_provider(&state, &req.model).await;
    match provider {
        Some(prov) => forward_ollama_generate(prov, req).await,
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "Model not found"}))).into_response(),
    }
}

async fn ollama_show(
    State(_state): State<ServerState>,
    Json(_req): Json<Value>,
) -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "Not implemented"})))
}

async fn ollama_pull(
    State(_state): State<ServerState>,
    Json(_req): Json<Value>,
) -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "Not implemented"})))
}

async fn ollama_delete(
    State(_state): State<ServerState>,
    Json(_req): Json<Value>,
) -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "Not implemented"})))
}

// ── Anthropic-compatible endpoints ──

#[derive(Deserialize)]
#[allow(dead_code)]
struct AnthropicRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(default)]
    stream: bool,
    max_tokens: u32,
    #[serde(default)]
    system: Option<String>,
}

async fn anthropic_messages(
    State(state): State<ServerState>,
    Json(req): Json<AnthropicRequest>,
) -> impl IntoResponse {
    let provider = resolve_provider(&state, &req.model).await;
    match provider {
        Some(prov) => forward_anthropic(prov, req).await,
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "Model not found"}))).into_response(),
    }
}

// ── Provider resolution by routes (9router style) ──

async fn resolve_provider(state: &ServerState, model: &str) -> Option<ProviderConfig> {
    let routes = state.routes.read().await;
    let providers = state.providers.read().await;

    for route in routes.iter() {
        if !route.enabled { continue; }
        if glob_match(&route.model_pattern, model) {
            for prov_id in &route.provider_ids {
                if let Some(prov) = providers.get(prov_id) {
                    if prov.enabled && prov.models.iter().any(|m| glob_match(m, model)) {
                        return Some(prov.clone());
                    }
                }
            }
            if route.fallback {
                for prov in providers.values() {
                    if prov.enabled && prov.models.iter().any(|m| glob_match(m, model)) {
                        return Some(prov.clone());
                    }
                }
            }
        }
    }

    for prov in providers.values() {
        if prov.enabled && prov.models.iter().any(|m| glob_match(m, model)) {
            return Some(prov.clone());
        }
    }

    None
}

fn glob_match(pattern: &str, text: &str) -> bool {
    if pattern == "*" || pattern == text { return true; }
    if pattern.ends_with('*') {
        return text.starts_with(&pattern[..pattern.len()-1]);
    }
    false
}

// ── Forwarding a proveedores (stubs - implementar con reqwest) ──

async fn forward_to_provider(
    _prov: ProviderConfig,
    _req: ChatCompletionRequest,
) -> Response {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "Provider forwarding not implemented yet"}))).into_response()
}

async fn forward_ollama_chat(
    _prov: ProviderConfig,
    _req: OllamaChatRequest,
) -> Response {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "Provider forwarding not implemented yet"}))).into_response()
}

async fn forward_ollama_generate(
    _prov: ProviderConfig,
    _req: OllamaGenerateRequest,
) -> Response {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "Provider forwarding not implemented yet"}))).into_response()
}

async fn forward_anthropic(
    _prov: ProviderConfig,
    _req: AnthropicRequest,
) -> Response {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "Provider forwarding not implemented yet"}))).into_response()
}

// ── Streaming SSE response ──

async fn stream_response(
    _state: ServerState,
    _provider: Option<ProviderConfig>,
    _req: ChatCompletionRequest,
) -> Response {
    let stream = stream::iter(vec![
        Ok::<_, std::io::Error>(Event::default().data(json!({"choices": [{"delta": {"content": "Hola"}}]}).to_string())),
        Ok(Event::default().data("[DONE]".to_string())),
    ]);

    Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::new())
        .into_response()
}