//! Per-model price table (USD per 1M tokens).
//!
//! Reference prices live in `pricing.default.json` (embedded) and are
//! copian a `~/.config/ianox/pricing.json` la primera vez, para poder editarlos
//! without recompiling. An unknown model has no cost: it shows "—".

use serde::Deserialize;
use std::path::PathBuf;

const DEFAULTS: &str = include_str!("../pricing.default.json");

#[derive(Debug, Clone, Deserialize)]
pub struct PriceEntry {
    pub pattern: String,
    pub input: f64,
    pub output: f64,
}

#[derive(Debug, Deserialize)]
struct PriceTable {
    #[serde(default)]
    models: Vec<PriceEntry>,
}

pub fn pricing_path() -> Option<PathBuf> {
    let dir = std::env::var("APPDATA")
        .or_else(|_| std::env::var("XDG_CONFIG_HOME"))
        .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.config", h)))
        .ok()?;
    Some(PathBuf::from(dir).join("ianox").join("pricing.json"))
}

/// Writes the default table if it does not exist yet.
pub fn ensure_file() {
    let Some(path) = pricing_path() else {
        return;
    };
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, DEFAULTS);
}

fn load_table() -> Vec<PriceEntry> {
    let Some(path) = pricing_path() else {
        return parse(DEFAULTS);
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => parse(&text),
        Err(_) => parse(DEFAULTS),
    }
}

fn parse(text: &str) -> Vec<PriceEntry> {
    serde_json::from_str::<PriceTable>(text)
        .map(|table| table.models)
        .unwrap_or_default()
}

fn normalize(model: &str) -> String {
    // "deepseek-ai/DeepSeek-V3" → "deepseek-v3"
    model
        .rsplit('/')
        .next()
        .unwrap_or(model)
        .to_lowercase()
}

/// Cost in USD for a request. `None` if the model is not in the table.
pub fn cost(model: &str, prompt_tokens: u64, completion_tokens: u64) -> Option<f64> {
    let model = normalize(model);
    let table = load_table();

    let mut best: Option<(&PriceEntry, usize)> = None;
    for entry in &table {
        let pattern = entry.pattern.to_lowercase();
        if model == pattern || model.starts_with(&pattern) {
            if best.map(|(_, len)| pattern.len() > len).unwrap_or(true) {
                best = Some((entry, pattern.len()));
            }
        }
    }

    best.map(|(entry, _)| {
        (prompt_tokens as f64 / 1_000_000.0) * entry.input
            + (completion_tokens as f64 / 1_000_000.0) * entry.output
    })
}

/// `true` for local providers (not charged).
pub fn is_local(kind: Option<&str>) -> bool {
    matches!(kind, Some("Ollama"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empareja_por_prefijo_mas_largo() {
        let mini = cost("gpt-4o-mini-2024-07-18", 1_000_000, 1_000_000).unwrap();
        assert!((mini - (0.15 + 0.6)).abs() < 1e-9);

        let full = cost("openai/gpt-4o", 1_000_000, 0).unwrap();
        assert!((full - 2.5).abs() < 1e-9);
    }

    #[test]
    fn modelo_desconocido_sin_coste() {
        assert!(cost("modelo-inventado-xyz", 10, 10).is_none());
    }
}
