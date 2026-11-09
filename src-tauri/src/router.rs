//! Internal router: resolves the provider and streams without going through the
//! HTTP server. The server (port 20128) is only for external clients
//! (IDEs, CLIs, curl).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::config::{Config, ProviderConfig, ProviderKind};

#[derive(Debug, Clone, Copy, Default)]
pub struct StreamStats {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

pub fn glob_match(pattern: &str, text: &str) -> bool {
    if pattern == "*" || pattern == text {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return text.starts_with(prefix);
    }
    false
}

/// Same logic as the server: routes with priority and fallback.
pub fn resolve_provider(config: &Config, model: &str) -> Option<ProviderConfig> {
    let providers = &config.providers;

    for route in config.routes.iter().filter(|r| r.enabled) {
        if !glob_match(&route.model_pattern, model) {
            continue;
        }
        for provider_id in &route.provider_ids {
            if let Some(provider) = providers.get(provider_id) {
                if provider.enabled && provider.models.iter().any(|m| glob_match(m, model)) {
                    return Some(provider.clone());
                }
            }
        }
        if route.fallback {
            if let Some(provider) = providers
                .values()
                .find(|p| p.enabled && p.models.iter().any(|m| glob_match(m, model)))
            {
                return Some(provider.clone());
            }
        }
    }

    providers
        .values()
        .find(|p| p.enabled && p.models.iter().any(|m| glob_match(m, model)))
        .cloned()
}

fn build_body(
    provider: &ProviderConfig,
    model: &str,
    messages: &[Value],
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    tools: &[Value],
) -> (String, Value, bool) {
    let is_anthropic = matches!(provider.kind, ProviderKind::Anthropic);
    let base = provider.base_url.trim_end_matches('/');

    if is_anthropic {
        let (system, chat_messages) = crate::provider::openai_to_anthropic(messages);

        let mut body = json!({
            "model": model,
            "max_tokens": max_tokens.unwrap_or(16_384),
            "stream": true,
            "messages": chat_messages,
        });
        if let Some(system) = system {
            body["system"] = Value::String(system);
        }
        if let Some(temp) = temperature {
            body["temperature"] = json!(temp);
        }
        if !tools.is_empty() {
            body["tools"] = json!(crate::provider::tools_to_anthropic(tools));
        }
        (format!("{base}/messages"), body, true)
    } else {
        let url = if base.contains("/v1") {
            format!("{base}/chat/completions")
        } else if matches!(provider.kind, ProviderKind::Ollama) {
            format!("{base}/v1/chat/completions")
        } else {
            format!("{base}/chat/completions")
        };
        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": true,
        });
        if let Some(temp) = temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(tokens) = max_tokens {
            body["max_tokens"] = json!(tokens);
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools);
            body["tool_choice"] = json!("auto");
        }
        // OpenAI only sends token usage when asked; other servers may reject the field.
        if matches!(provider.kind, ProviderKind::OpenAI) {
            body["stream_options"] = json!({ "include_usage": true });
        }
        (url, body, false)
    }
}

fn estimated_stats(
    messages: &[Value],
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    output_len: usize,
) -> StreamStats {
    let prompt_estimate = (serde_json::to_string(messages).map(|s| s.len()).unwrap_or(0) / 4) as u64;
    StreamStats {
        prompt_tokens: prompt_tokens.unwrap_or(prompt_estimate),
        completion_tokens: completion_tokens.unwrap_or((output_len / 4) as u64),
    }
}

/// Resolves once the stop button has been pressed, polling the shared flag.
async fn wait_for_cancel(cancel: &Arc<AtomicBool>) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

/// Runs the request and emits `chat://delta/{id}`, `chat://done/{id}`.
/// Result of one streaming round.
pub struct StreamOutcome {
    pub stats: StreamStats,
    /// Tool calls requested by the model; empty when it answered normally.
    pub calls: Vec<ToolCall>,
    /// Assistant text streamed in this round (kept so the history stays faithful).
    pub text: String,
    /// Reasoning text streamed in this round.
    pub reasoning: String,
    /// The user pressed stop.
    pub cancelled: bool,
}

/// Una llamada a herramienta emitida por el modelo.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Acumula los trozos de `tool_calls` que llegan por streaming.
#[derive(Default, Debug)]
pub struct ToolAccumulator {
    calls: Vec<ToolCall>,
}

impl ToolAccumulator {
    fn ensure(&mut self, index: usize) -> &mut ToolCall {
        while self.calls.len() <= index {
            self.calls.push(ToolCall::default());
        }
        &mut self.calls[index]
    }

    /// OpenAI / DeepSeek / compatible.
    pub fn ingest_openai(&mut self, json: &Value) {
        for pointer in [
            "/choices/0/delta/tool_calls",
            "/choices/0/message/tool_calls",
        ] {
            let Some(list) = json.pointer(pointer).and_then(Value::as_array) else {
                continue;
            };
            for (position, item) in list.iter().enumerate() {
                let index = item
                    .get("index")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize)
                    .unwrap_or(position);
                let call = self.ensure(index);
                if let Some(id) = item.get("id").and_then(Value::as_str) {
                    call.id = id.to_string();
                }
                if let Some(name) = item.pointer("/function/name").and_then(Value::as_str) {
                    call.name = name.to_string();
                }
                if let Some(args) = item.pointer("/function/arguments").and_then(Value::as_str) {
                    call.arguments.push_str(args);
                }
            }
        }

        for pointer in [
            "/choices/0/delta/function_call",
            "/choices/0/message/function_call",
        ] {
            let Some(fc) = json.pointer(pointer) else {
                continue;
            };
            let call = self.ensure(0);
            if let Some(name) = fc.get("name").and_then(Value::as_str) {
                call.name = name.to_string();
            }
            if let Some(args) = fc.get("arguments").and_then(Value::as_str) {
                call.arguments.push_str(args);
            }
        }
    }

    /// Anthropic: content_block_start con type=tool_use.
    pub fn ingest_anthropic_start(&mut self, json: &Value) {
        let block = json.get("content_block");
        if block.and_then(|b| b.get("type")).and_then(Value::as_str) != Some("tool_use") {
            return;
        }
        let index = json.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
        let call = self.ensure(index);
        if let Some(id) = block.and_then(|b| b.get("id")).and_then(Value::as_str) {
            call.id = id.to_string();
        }
        if let Some(name) = block.and_then(|b| b.get("name")).and_then(Value::as_str) {
            call.name = name.to_string();
        }
    }

    /// Anthropic: content_block_delta con input_json_delta.
    pub fn ingest_anthropic_delta(&mut self, json: &Value) {
        let delta = json.get("delta");
        if delta.and_then(|d| d.get("type")).and_then(Value::as_str) != Some("input_json_delta") {
            return;
        }
        let index = json.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
        if let Some(partial) = delta.and_then(|d| d.get("partial_json")).and_then(Value::as_str) {
            self.ensure(index).arguments.push_str(partial);
        }
    }

    pub fn take(&mut self) -> Vec<ToolCall> {
        std::mem::take(&mut self.calls)
            .into_iter()
            .filter(|call| !call.name.is_empty())
            .collect()
    }
}

fn finish_round(
    _app: &AppHandle,
    _done_event: &str,
    tools: &mut ToolAccumulator,
    stats: StreamStats,
    text: String,
    reasoning: String,
    cancelled: bool,
) -> StreamOutcome {
    let calls = if cancelled { Vec::new() } else { tools.take() };
    StreamOutcome { stats, calls, text, reasoning, cancelled }
}

/// Message of an error reported inside the stream (Anthropic `type: "error"`, or an OpenAI-style
/// `{"error": {...}}` object), if this event is one.
fn stream_error(json: &Value) -> Option<String> {
    let error = if json.get("type").and_then(Value::as_str) == Some("error") {
        json.get("error")?
    } else {
        json.get("error").filter(|e| e.is_object() || e.is_string())?
    };
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| error.as_str())
        .unwrap_or("unknown error");
    Some(message.to_string())
}

/// Adds the provider's credentials to a request.
fn with_auth(
    mut request: reqwest::RequestBuilder,
    provider: &ProviderConfig,
    is_anthropic: bool,
) -> reqwest::RequestBuilder {
    let key = provider.api_key.as_deref().filter(|k| !k.is_empty());
    if let Some(key) = key {
        request = if is_anthropic {
            request.header("x-api-key", key).header("anthropic-version", "2023-06-01")
        } else {
            request.header("authorization", format!("Bearer {key}"))
        };
    }
    request
}

/// One non-streaming completion, for short helper calls such as conversation titles.
pub async fn complete_text(
    provider: &ProviderConfig,
    model: &str,
    messages: &[Value],
    max_tokens: u32,
) -> Result<String, String> {
    let (url, mut body, is_anthropic) =
        build_body(provider, model, messages, Some(0.3), Some(max_tokens), &[]);
    body["stream"] = json!(false);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    let response = with_auth(client.post(&url).json(&body), provider, is_anthropic)
        .send()
        .await
        .map_err(|e| format!("Could not connect to {}: {e}", provider.name))?;
    let status = response.status();
    let value: Value = response.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("{} responded {}", provider.name, status.as_u16()));
    }

    let text = if is_anthropic {
        value["content"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default()
    } else {
        value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    if text.trim().is_empty() {
        return Err("The model returned no text".into());
    }
    Ok(text)
}

pub async fn stream_chat(
    app: AppHandle,
    request_id: String,
    provider: ProviderConfig,
    model: String,
    messages: Vec<Value>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    cancel: Arc<AtomicBool>,
    tools: &[Value],
) -> Result<StreamOutcome, String> {
    let (url, body, is_anthropic) =
        build_body(&provider, &model, &messages, temperature, max_tokens, tools);
    let delta_event = format!("chat://delta/{request_id}");
    let reasoning_event = format!("chat://reasoning/{request_id}");
    let mut tools = ToolAccumulator::default();
    let done_event = format!("chat://done/{request_id}");

    let client = reqwest::Client::new();
    let request = with_auth(client.post(&url).json(&body), &provider, is_anthropic);

    let response = tokio::select! {
        response = request.send() => {
            response.map_err(|e| format!("Could not connect to {}: {e}", provider.name))?
        }
        // Stop pressed while the provider is still answering: do not wait for it.
        _ = wait_for_cancel(&cancel) => {
            return Ok(StreamOutcome {
                stats: StreamStats { prompt_tokens: 0, completion_tokens: 0 },
                calls: Vec::new(),
                text: String::new(),
                reasoning: String::new(),
                cancelled: true,
            });
        }
    };

    let status = response.status();
    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        return Err(format!(
            "{} responded {}: {}",
            provider.name,
            status.as_u16(),
            detail.chars().take(400).collect::<String>()
        ));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut output_len = 0usize;
    let mut text_buf = String::new();
    let mut reasoning_buf = String::new();
    let mut prompt_tokens: Option<u64> = None;
    let mut completion_tokens: Option<u64> = None;

    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            return Ok(finish_round(&app, &done_event, &mut tools, estimated_stats(&messages, prompt_tokens, completion_tokens, output_len), text_buf, reasoning_buf, true));
        }

        let bytes = chunk.map_err(|e| format!("Error leyendo el stream: {e}"))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(newline) = buffer.find('\n') {
            let line = buffer[..newline].trim().to_string();
            buffer.drain(..=newline);

            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() {
                continue;
            }
            if data == "[DONE]" {
                return Ok(finish_round(&app, &done_event, &mut tools, estimated_stats(&messages, prompt_tokens, completion_tokens, output_len), text_buf, reasoning_buf, false));
            }

            let Ok(json) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            if let Some(message) = stream_error(&json) {
                return Err(format!("{} error: {message}", provider.name));
            }

            if is_anthropic {
                match json.get("type").and_then(Value::as_str) {
                    Some("content_block_start") => tools.ingest_anthropic_start(&json),
                    Some("content_block_delta") => tools.ingest_anthropic_delta(&json),
                    _ => {}
                }
                if let Some(input) = json
                    .pointer("/message/usage/input_tokens")
                    .and_then(Value::as_u64)
                {
                    prompt_tokens = Some(input);
                }
                if let Some(output) = json.pointer("/usage/output_tokens").and_then(Value::as_u64) {
                    completion_tokens = Some(output);
                }
                if json.get("type").and_then(Value::as_str) == Some("message_stop") {
                    return Ok(finish_round(&app, &done_event, &mut tools, estimated_stats(&messages, prompt_tokens, completion_tokens, output_len), text_buf, reasoning_buf, false));
                }
                if let Some(thinking) = json
                    .pointer("/delta/thinking")
                    .and_then(Value::as_str)
                    .filter(|t| !t.is_empty())
                {
                    reasoning_buf.push_str(thinking);
                    let _ = app.emit(&reasoning_event, thinking.to_string());
                }
                if let Some(text) = json
                    .pointer("/delta/text")
                    .and_then(Value::as_str)
                    .filter(|t| !t.is_empty())
                {
                    output_len += text.len();
                    text_buf.push_str(text);
                    let _ = app.emit(&delta_event, text.to_string());
                }
            } else {
                tools.ingest_openai(&json);
                if let Some(usage) = json.get("usage") {
                    if let Some(prompt) = usage.get("prompt_tokens").and_then(Value::as_u64) {
                        prompt_tokens = Some(prompt);
                    }
                    if let Some(completion) = usage.get("completion_tokens").and_then(Value::as_u64) {
                        completion_tokens = Some(completion);
                    }
                }
                for pointer in ["/choices/0/delta/reasoning_content", "/choices/0/delta/reasoning"] {
                    if let Some(thinking) = json
                        .pointer(pointer)
                        .and_then(Value::as_str)
                        .filter(|t| !t.is_empty())
                    {
                        reasoning_buf.push_str(thinking);
                    let _ = app.emit(&reasoning_event, thinking.to_string());
                    }
                }
                if let Some(text) = json
                    .pointer("/choices/0/delta/content")
                    .and_then(Value::as_str)
                    .filter(|t| !t.is_empty())
                {
                    output_len += text.len();
                    text_buf.push_str(text);
                    let _ = app.emit(&delta_event, text.to_string());
                }
                // Keep reading after the finish reason: the usage chunk arrives after it,
                // and the stream ends with [DONE] or by closing.
            }
        }
    }

    Ok(finish_round(&app, &done_event, &mut tools, estimated_stats(&messages, prompt_tokens, completion_tokens, output_len), text_buf, reasoning_buf, false))
}


#[cfg(test)]
mod tool_tests {
    use super::*;

    #[test]
    fn ensambla_tool_calls_de_openai() {
        let mut acc = ToolAccumulator::default();
        acc.ingest_openai(&serde_json::json!({
            "choices": [{ "delta": { "tool_calls": [
                { "index": 0, "id": "call_1", "function": { "name": "shell", "arguments": "{\"cmd\":" } }
            ] } }]
        }));
        acc.ingest_openai(&serde_json::json!({
            "choices": [{ "delta": { "tool_calls": [
                { "index": 0, "function": { "arguments": "\"ls\"}" } }
            ] } }]
        }));

        let calls = acc.take();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].name, "shell");
        assert_eq!(calls[0].arguments, "{\"cmd\":\"ls\"}");
        assert!(acc.take().is_empty());
    }

    #[test]
    fn lee_tool_calls_del_mensaje_final() {
        let mut acc = ToolAccumulator::default();
        acc.ingest_openai(&serde_json::json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": { "role": "assistant", "content": null, "tool_calls": [
                    { "id": "call_9", "type": "function",
                      "function": { "name": "glob", "arguments": "{\"pattern\":\"**/*.md\"}" } }
                ] }
            }]
        }));
        let calls = acc.take();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "glob");
    }

    #[test]
    fn lee_function_call_antiguo() {
        let mut acc = ToolAccumulator::default();
        acc.ingest_openai(&serde_json::json!({
            "choices": [{ "delta": { "function_call": { "name": "read", "arguments": "{\"path\":\"a\"}" } } }]
        }));
        assert_eq!(acc.take()[0].name, "read");
    }

    #[test]
    fn ensambla_tool_calls_de_anthropic() {
        let mut acc = ToolAccumulator::default();
        acc.ingest_anthropic_start(&serde_json::json!({
            "type": "content_block_start",
            "index": 1,
            "content_block": { "type": "tool_use", "id": "toolu_1", "name": "read" }
        }));
        acc.ingest_anthropic_delta(&serde_json::json!({
            "type": "content_block_delta",
            "index": 1,
            "delta": { "type": "input_json_delta", "partial_json": "{\"path\":" }
        }));
        acc.ingest_anthropic_delta(&serde_json::json!({
            "type": "content_block_delta",
            "index": 1,
            "delta": { "type": "input_json_delta", "partial_json": "\"src/main.ts\"}" }
        }));

        let calls = acc.take();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read");
        assert_eq!(calls[0].arguments, "{\"path\":\"src/main.ts\"}");
    }
}

#[cfg(test)]
mod body_tests {
    use super::*;
    use crate::config::ProviderConfig;

    fn provider() -> ProviderConfig {
        ProviderConfig {
            id: "p".into(),
            name: "P".into(),
            kind: ProviderKind::Custom,
            base_url: "https://example.com/v1".into(),
            api_key: None,
            models: vec![],
            enabled: true,
            priority: 1,
            rate_limit: None,
        }
    }

    #[test]
    fn sends_tools_when_present() {
        let tools = crate::tools::definitions("build");
        let (_, body, _) = build_body(&provider(), "m", &[], None, None, &tools);
        assert!(body["tools"].as_array().is_some());
        assert_eq!(body["tool_choice"], "auto");
    }

    #[test]
    fn omits_tools_when_empty() {
        let (_, body, _) = build_body(&provider(), "m", &[], None, None, &[]);
        assert!(body.get("tools").is_none());
    }
}

#[cfg(test)]
mod stream_error_tests {
    use super::*;

    #[test]
    fn reads_errors_sent_inside_the_stream() {
        let anthropic = json!({"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}});
        assert_eq!(stream_error(&anthropic).as_deref(), Some("Overloaded"));
        let openai = json!({"error":{"message":"Rate limit reached","type":"rate_limit"}});
        assert_eq!(stream_error(&openai).as_deref(), Some("Rate limit reached"));
        assert!(stream_error(&json!({"choices":[{"delta":{"content":"hi"}}]})).is_none());
        assert!(stream_error(&json!({"type":"content_block_delta","delta":{"text":"x"}})).is_none());
    }
}
