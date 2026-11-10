//! Model catalog (names and limits) from models.dev.
//!
//! The provider config only stores raw ids such as `deepseek-flash`; the catalog
//! maps them to the names users see (`DeepSeek V4.1 Flash 1017`). It is cached on
//! disk and refreshed in the background, and lookups fall back to nothing when
//! offline so callers can prettify the id instead.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

const URL: &str = "https://models.dev/api.json";
const MAX_AGE_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub name: String,
    pub context: Option<u64>,
    /// Max output tokens per response.
    #[serde(default)]
    pub output: Option<u64>,
    /// USD per 1M input tokens.
    #[serde(default)]
    pub input_cost: Option<f64>,
    /// USD per 1M output tokens.
    #[serde(default)]
    pub output_cost: Option<f64>,
}

/// provider id -> model id -> entry
type Catalog = HashMap<String, HashMap<String, Entry>>;

static CATALOG: OnceLock<RwLock<Catalog>> = OnceLock::new();

fn store() -> &'static RwLock<Catalog> {
    CATALOG.get_or_init(|| RwLock::new(Catalog::new()))
}

fn cache_path() -> Option<std::path::PathBuf> {
    let dir = std::env::var("APPDATA")
        .or_else(|_| std::env::var("XDG_CONFIG_HOME"))
        .or_else(|_| std::env::var("HOME").map(|h| format!("{h}/.config")))
        .ok()?;
    Some(std::path::PathBuf::from(dir).join("ianox").join("models-cache-v2.json"))
}

/// Reduces the models.dev payload to what the UI needs.
pub fn slim(raw: &Value) -> Catalog {
    let mut out = Catalog::new();
    let Some(providers) = raw.as_object() else {
        return out;
    };
    for (provider_id, provider) in providers {
        let Some(models) = provider.get("models").and_then(Value::as_object) else {
            continue;
        };
        let entries = models
            .iter()
            .filter_map(|(id, model)| {
                let name = model.get("name").and_then(Value::as_str)?;
                let context = model.pointer("/limit/context").and_then(Value::as_u64);
                let output = model.pointer("/limit/output").and_then(Value::as_u64);
                let input_cost = model.pointer("/cost/input").and_then(Value::as_f64);
                let output_cost = model.pointer("/cost/output").and_then(Value::as_f64);
                Some((id.clone(), Entry { name: name.to_string(), context, output, input_cost, output_cost }))
            })
            .collect();
        out.insert(provider_id.clone(), entries);
    }
    out
}

/// Looks a model up: the provider's own entry first, then any provider with that id.
pub fn lookup(provider_id: &str, model_id: &str) -> Option<Entry> {
    let catalog = store().read().ok()?;
    if let Some(entry) = catalog.get(provider_id).and_then(|models| models.get(model_id)) {
        return Some(entry.clone());
    }
    let mut hits: Vec<_> = catalog
        .iter()
        .filter_map(|(provider, models)| models.get(model_id).map(|e| (provider, e)))
        .collect();
    // Deterministic when several providers share the id.
    hits.sort_by(|a, b| a.0.cmp(b.0));
    hits.first().map(|(_, e)| (*e).clone())
}

/// Every model the catalog lists for a provider, sorted by display name.
pub fn models_of(provider_key: &str) -> Vec<(String, Entry)> {
    let Ok(catalog) = store().read() else {
        return Vec::new();
    };
    let mut models: Vec<(String, Entry)> = catalog
        .get(provider_key)
        .map(|models| models.iter().map(|(id, entry)| (id.clone(), entry.clone())).collect())
        .unwrap_or_default();
    models.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));
    models
}

/// A model with a catalog price of zero in both directions.
pub fn is_free(entry: &Entry) -> bool {
    entry.input_cost == Some(0.0) && entry.output_cost == Some(0.0)
}

/// Estimated cost in USD from the catalog price (no cache discounts), if it lists one.
pub fn cost(provider_id: &str, model_id: &str, prompt: u64, completion: u64) -> Option<f64> {
    let entry = lookup(provider_id, model_id)?;
    let (input, output) = (entry.input_cost?, entry.output_cost?);
    Some(prompt as f64 * input / 1e6 + completion as f64 * output / 1e6)
}

/// Loads the disk cache and refreshes it in the background when stale or missing.
pub fn init() {
    let path = cache_path();
    let mut stale = true;
    if let Some(path) = &path {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(catalog) = serde_json::from_str::<Catalog>(&text) {
                if let Ok(mut guard) = store().write() {
                    *guard = catalog;
                }
                stale = std::fs::metadata(path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .map_or(true, |age| age.as_secs() > MAX_AGE_SECS);
            }
        }
    }
    if stale {
        tauri::async_runtime::spawn(async move {
            if let Err(error) = refresh().await {
                eprintln!("[ianox] models catalog: {error}");
            }
        });
    }
}

async fn refresh() -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let raw: Value = client
        .get(URL)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let catalog = slim(&raw);
    if catalog.is_empty() {
        return Err("empty catalog".into());
    }
    if let Some(path) = cache_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string(&catalog) {
            let _ = std::fs::write(path, text);
        }
    }
    if let Ok(mut guard) = store().write() {
        *guard = catalog;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slims_and_looks_up_models() {
        let raw = serde_json::json!({
            "deepseek": { "models": { "deepseek-flash": { "name": "DeepSeek V4.1 Flash 1017", "limit": { "context": 1048576, "output": 393216 }, "cost": { "input": 0.15, "output": 0.6 } } } },
            "other": { "models": { "x": { "name": "X" } } }
        });
        let catalog = slim(&raw);
        *store().write().unwrap() = catalog;
        let hit = lookup("deepseek", "deepseek-flash").unwrap();
        assert_eq!(hit.name, "DeepSeek V4.1 Flash 1017");
        assert_eq!(hit.context, Some(1048576));
        assert_eq!(hit.output, Some(393216));
        let cost = cost("deepseek", "deepseek-flash", 1_000_000, 1_000_000).unwrap();
        assert!((cost - 0.75).abs() < 1e-9);
        let listed = models_of("deepseek");
        assert_eq!(listed.len(), 1);
        assert!(!is_free(&listed[0].1));
        assert!(models_of("nope").is_empty());
        // Unknown provider id falls back to any provider with that model id.
        assert_eq!(lookup("my-router", "x").unwrap().name, "X");
        assert!(lookup("deepseek", "nope").is_none());
    }
}
