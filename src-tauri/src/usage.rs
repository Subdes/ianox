//! Usage log and aggregation for the dashboard (9router style).

use chrono::{DateTime, Datelike, Duration as ChronoDuration, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub ts: i64,
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub latency_ms: u64,
    #[serde(default)]
    pub ok: bool,
    /// Chat session that originated the request (if it came from the internal chat).
    #[serde(default)]
    pub session_id: Option<String>,
    /// Provider kind (OpenAI, Ollama…); local ones are not charged.
    #[serde(default)]
    pub provider_kind: Option<String>,
    /// Size of the conversation after the last round (prompt + answer), i.e. context in use.
    #[serde(default)]
    pub context_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayPoint {
    pub label: String,
    pub tokens: u64,
    pub requests: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderUsage {
    pub id: String,
    pub tokens: u64,
    pub requests: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelUsage {
    pub model: String,
    pub tokens: u64,
    pub requests: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageSummary {
    pub days: Vec<DayPoint>,
    pub by_provider: Vec<ProviderUsage>,
    pub by_model: Vec<ModelUsage>,
    pub total_requests: u64,
    pub total_tokens: u64,
    pub total_cost_usd: Option<f64>,
}

pub fn usage_path() -> Option<PathBuf> {
    let dir = std::env::var("APPDATA")
        .or_else(|_| std::env::var("XDG_CONFIG_HOME"))
        .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.config", h)))
        .ok()?;
    Some(PathBuf::from(dir).join("ianox").join("usage.jsonl"))
}

pub fn record(record: &UsageRecord) {
    let Some(path) = usage_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        if let Ok(line) = serde_json::to_string(record) {
            let _ = writeln!(file, "{line}");
        }
    }
}

fn read_all() -> Vec<UsageRecord> {
    let Some(path) = usage_path() else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(|line| serde_json::from_str::<UsageRecord>(line).ok())
        .collect()
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SessionUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub requests: u64,
    pub last_ts: i64,
    /// Estimated cost in USD from the price table (`None` if the model has no price).
    pub cost_usd: Option<f64>,
    /// Context size after the most recent request.
    pub context_tokens: Option<u64>,
}

/// Accumulated usage for a given chat session.
pub fn session_usage(session_id: &str) -> SessionUsage {
    session_usage_from(&read_all(), session_id)
}

fn session_usage_from(records: &[UsageRecord], session_id: &str) -> SessionUsage {
    let mut out = SessionUsage::default();
    let mut latest_context: Option<(i64, u64)> = None;
    for record in records {
        if record.session_id.as_deref() != Some(session_id) {
            continue;
        }
        out.prompt_tokens += record.prompt_tokens;
        out.completion_tokens += record.completion_tokens;
        out.requests += 1;
        out.last_ts = out.last_ts.max(record.ts);
        if let Some(context) = record.context_tokens {
            if latest_context.map_or(true, |(ts, _)| record.ts >= ts) {
                latest_context = Some((record.ts, context));
            }
        }
        if crate::pricing::is_local(record.provider_kind.as_deref()) {
            out.cost_usd = Some(out.cost_usd.unwrap_or(0.0));
        } else if let Some(cost) = crate::pricing::cost(&record.model, record.prompt_tokens, record.completion_tokens)
            .or_else(|| crate::catalog::cost(&record.provider, &record.model, record.prompt_tokens, record.completion_tokens))
        {
            out.cost_usd = Some(out.cost_usd.unwrap_or(0.0) + cost);
        }
    }
    out.total_tokens = out.prompt_tokens + out.completion_tokens;
    out.context_tokens = latest_context.map(|(_, tokens)| tokens);
    out
}

pub fn summarize(days: u32) -> UsageSummary {
    summarize_records(&read_all(), days, Local::now())
}

fn summarize_records(records: &[UsageRecord], days: u32, now: DateTime<Local>) -> UsageSummary {
    let days = days.clamp(1, 90) as i64;
    let cutoff = (now - ChronoDuration::days(days - 1))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|naive| naive.and_local_timezone(Local).single())
        .flatten()
        .map(|start| start.timestamp())
        .unwrap_or(0);

    let mut per_day: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut per_provider: HashMap<String, (u64, u64)> = HashMap::new();
    let mut per_model: HashMap<String, (u64, u64)> = HashMap::new();
    let mut total_requests = 0u64;
    let mut total_tokens = 0u64;
    let mut total_cost: Option<f64> = None;

    for record in records.iter().filter(|r| r.ts >= cutoff) {
        let tokens = record.prompt_tokens + record.completion_tokens;
        total_requests += 1;
        total_tokens += tokens;
        if crate::pricing::is_local(record.provider_kind.as_deref()) {
            total_cost = Some(total_cost.unwrap_or(0.0));
        } else if let Some(cost) = crate::pricing::cost(
            &record.model,
            record.prompt_tokens,
            record.completion_tokens,
        ) {
            total_cost = Some(total_cost.unwrap_or(0.0) + cost);
        }

        let label = Local
            .timestamp_opt(record.ts, 0)
            .single()
            .map(|dt| format!("{:02}-{:02}", dt.day(), dt.month()))
            .unwrap_or_else(|| "--".to_string());

        let day = per_day.entry(label).or_insert((0, 0));
        day.0 += tokens;
        day.1 += 1;

        let provider = per_provider.entry(record.provider.clone()).or_insert((0, 0));
        provider.0 += tokens;
        provider.1 += 1;

        let model = per_model.entry(record.model.clone()).or_insert((0, 0));
        model.0 += tokens;
        model.1 += 1;
    }

    // Fills inactive days so the chart has a continuous axis.
    let mut day_series = Vec::new();
    for offset in (0..days).rev() {
        let day = (now - ChronoDuration::days(offset)).date_naive();
        let label = format!("{:02}-{:02}", day.day(), day.month());
        let (tokens, requests) = per_day.get(&label).copied().unwrap_or((0, 0));
        day_series.push(DayPoint { label, tokens, requests });
    }

    let mut by_provider: Vec<ProviderUsage> = per_provider
        .into_iter()
        .map(|(id, (tokens, requests))| ProviderUsage { id, tokens, requests })
        .collect();
    by_provider.sort_by(|a, b| b.tokens.cmp(&a.tokens));

    let mut by_model: Vec<ModelUsage> = per_model
        .into_iter()
        .map(|(model, (tokens, requests))| ModelUsage { model, tokens, requests })
        .collect();
    by_model.sort_by(|a, b| b.tokens.cmp(&a.tokens));
    by_model.truncate(5);

    UsageSummary {
        days: day_series,
        by_provider,
        by_model,
        total_requests,
        total_tokens,
        total_cost_usd: total_cost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(ts: i64, provider: &str, model: &str, tokens: u64) -> UsageRecord {
        UsageRecord {
            ts,
            provider: provider.into(),
            model: model.into(),
            prompt_tokens: tokens / 2,
            completion_tokens: tokens / 2,
            latency_ms: 10,
            ok: true,
            session_id: None,
            provider_kind: Some("Custom".into()),
            context_tokens: None,
        }
    }

    #[test]
    fn session_usage_reports_the_latest_context_size() {
        let mut first = rec(100, "p", "m", 1000);
        first.session_id = Some("s".into());
        first.context_tokens = Some(500);
        let mut second = rec(200, "p", "m", 4000);
        second.session_id = Some("s".into());
        second.context_tokens = Some(900);
        let mut other = rec(300, "p", "m", 10);
        other.session_id = Some("other".into());
        other.context_tokens = Some(1);
        let usage = session_usage_from(&[second, first, other], "s");
        assert_eq!(usage.requests, 2);
        assert_eq!(usage.context_tokens, Some(900));
    }

    #[test]
    fn aggregates_totals_by_provider_and_model() {
        let now = Local::now();
        let records = vec![
            rec(now.timestamp(), "ollama", "llama3.1", 100),
            rec(now.timestamp(), "ollama", "llama3.1", 50),
            rec((now - ChronoDuration::days(1)).timestamp(), "openai", "gpt-4o", 200),
        ];

        let summary = summarize_records(&records, 7, now);
        assert_eq!(summary.total_requests, 3);
        assert_eq!(summary.total_tokens, 350);
        assert_eq!(summary.days.len(), 7);
        assert_eq!(summary.by_provider[0].id, "openai");
        assert_eq!(summary.by_provider[0].tokens, 200);
        assert_eq!(summary.by_model[0].model, "gpt-4o");
    }
}
