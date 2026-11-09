//! Sesiones de chat persistentes (historial).
//!
//! Stored in `~/.config/ianox/sessions.json`. Each session belongs to a
//! proyecto (o a ninguno) y conserva sus mensajes.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Una parte de un mensaje, en el orden en que ocurrió.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum MessagePart {
    Text { text: String },
    Reasoning { text: String },
    ToolCall { id: String, name: String, arguments: String },
    ToolResult { id: String, output: String, #[serde(default)] is_error: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredTool {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub arguments: String,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredImage {
    pub name: String,
    pub mime: String,
    /// Base64, without the `data:` prefix.
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub role: String,
    pub content: String,
    /// Images that went with this message (pasted screenshots).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<StoredImage>,
    /// Texto de "pensamiento" de modelos con razonamiento (DeepSeek, o3…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Contenido estructurado (texto, razonamiento y herramientas en orden).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<MessagePart>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<StoredTool>,
    /// This message is the summary that replaced everything before it (context compaction).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub compaction: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub model: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub messages: Vec<StoredMessage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub project_id: Option<String>,
    pub agent_id: String,
    pub model: Option<String>,
    pub updated_at: i64,
    pub message_count: usize,
}

pub fn sessions_path() -> Option<PathBuf> {
    let dir = std::env::var("APPDATA")
        .or_else(|_| std::env::var("XDG_CONFIG_HOME"))
        .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.config", h)))
        .ok()?;
    Some(PathBuf::from(dir).join("ianox").join("sessions.json"))
}

/// Rellena `parts` en sesiones antiguas que solo tenían content/reasoning.
fn normalize(records: &mut [SessionRecord]) {
    for record in records.iter_mut() {
        for message in record.messages.iter_mut() {
            if !message.parts.is_empty() {
                continue;
            }
            if let Some(reasoning) = message.reasoning.take() {
                if !reasoning.is_empty() {
                    message.parts.push(MessagePart::Reasoning { text: reasoning });
                }
            }
            if !message.content.is_empty() {
                message
                    .parts
                    .push(MessagePart::Text { text: message.content.clone() });
            }
        }
    }
}

fn read_file(path: &Path) -> Vec<SessionRecord> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut records = serde_json::from_str::<Vec<SessionRecord>>(&content).unwrap_or_default();
    normalize(&mut records);
    records
}

fn write_file(path: &Path, records: &[SessionRecord]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(records).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Could not save {}: {e}", path.display()))
}

pub fn read_all() -> Vec<SessionRecord> {
    sessions_path().map(|p| read_file(&p)).unwrap_or_default()
}

pub fn list() -> Vec<SessionSummary> {
    let mut sessions = read_all();
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    sessions
        .into_iter()
        .map(|s| SessionSummary {
            id: s.id,
            title: s.title,
            project_id: s.project_id,
            agent_id: s.agent_id,
            model: s.model,
            updated_at: s.updated_at,
            message_count: s.messages.len(),
        })
        .collect()
}

/// A conversation whose messages contain the searched text.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SessionHit {
    pub id: String,
    pub title: String,
    pub project_id: Option<String>,
    pub updated_at: i64,
    /// Text around the first match, with ellipses when cut.
    pub snippet: String,
}

/// Lowercases and drops accents one char for one, so positions line up with the original.
fn fold_char(c: char) -> char {
    let lower = c.to_lowercase().next().unwrap_or(c);
    match lower {
        'á' | 'à' | 'ä' | 'â' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        other => other,
    }
}

fn snippet_around(text: &str, start: usize, len: usize) -> String {
    const BEFORE: usize = 30;
    const AFTER: usize = 70;
    let chars: Vec<char> = text.chars().collect();
    let from = start.saturating_sub(BEFORE);
    let to = (start + len + AFTER).min(chars.len());
    let body: String = chars[from..to].iter().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{}{}{}", if from > 0 { "…" } else { "" }, body, if to < chars.len() { "…" } else { "" })
}

/// Conversations (newest first) with a message containing `query`, ignoring case and accents.
pub fn search_in(records: &[SessionRecord], query: &str, limit: usize) -> Vec<SessionHit> {
    let needle: Vec<char> = query.trim().chars().map(fold_char).collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<&SessionRecord> = records.iter().collect();
    sorted.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    let mut hits = Vec::new();
    for record in sorted {
        for message in &record.messages {
            let folded: Vec<char> = message.content.chars().map(fold_char).collect();
            if let Some(at) = folded.windows(needle.len()).position(|w| w == needle.as_slice()) {
                hits.push(SessionHit {
                    id: record.id.clone(),
                    title: record.title.clone(),
                    project_id: record.project_id.clone(),
                    updated_at: record.updated_at,
                    snippet: snippet_around(&message.content, at, needle.len()),
                });
                break;
            }
        }
        if hits.len() >= limit {
            break;
        }
    }
    hits
}

pub fn search(query: &str, limit: usize) -> Vec<SessionHit> {
    search_in(&read_all(), query, limit)
}

pub fn get(id: &str) -> Option<SessionRecord> {
    read_all().into_iter().find(|s| s.id == id)
}

pub fn save(mut record: SessionRecord) -> Result<SessionSummary, String> {
    let path = sessions_path().ok_or("Could not resolve the configuration directory")?;
    let mut sessions = read_file(&path);

    record.updated_at = chrono::Utc::now().timestamp();
    if record.created_at == 0 {
        record.created_at = sessions
            .iter()
            .find(|s| s.id == record.id)
            .map(|s| s.created_at)
            .unwrap_or(record.updated_at);
    }

    if let Some(existing) = sessions.iter_mut().find(|s| s.id == record.id) {
        *existing = record.clone();
    } else {
        sessions.push(record.clone());
    }

    write_file(&path, &sessions)?;

    Ok(SessionSummary {
        id: record.id,
        title: record.title,
        project_id: record.project_id,
        agent_id: record.agent_id,
        model: record.model,
        updated_at: record.updated_at,
        message_count: record.messages.len(),
    })
}

pub fn delete(id: &str) -> Result<(), String> {
    let path = sessions_path().ok_or("Could not resolve the configuration directory")?;
    let mut sessions = read_file(&path);
    let before = sessions.len();
    sessions.retain(|s| s.id != id);
    if sessions.len() == before {
        return Ok(());
    }
    write_file(&path, &sessions)
}

pub fn rename(id: &str, title: &str) -> Result<(), String> {
    let path = sessions_path().ok_or("Could not resolve the configuration directory")?;
    let mut sessions = read_file(&path);
    let Some(session) = sessions.iter_mut().find(|s| s.id == id) else {
        return Err("Session not found".into());
    };
    session.title = title.to_string();
    session.updated_at = chrono::Utc::now().timestamp();
    write_file(&path, &sessions)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str) -> SessionRecord {
        SessionRecord {
            id: id.into(),
            title: "Test".into(),
            project_id: Some("p1".into()),
            agent_id: "build".into(),
            model: Some("deepseek-chat".into()),
            created_at: 0,
            updated_at: 0,
            messages: vec![StoredMessage {
                reasoning: None,
                agent: None,
                model: None,
                parts: Vec::new(),
                tools: Vec::new(),
                images: Vec::new(),
                compaction: false,
                role: "user".into(),
                content: "hello".into(),
            }],
        }
    }

    #[test]
    fn read_write_roundtrip_and_delete() {
        let dir = std::env::temp_dir().join(format!("ianox-sessions-{}", uuid::Uuid::new_v4()));
        let path = dir.join("sessions.json");

        write_file(&path, &[record("a"), record("b")]).expect("write");
        let loaded = read_file(&path);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].messages.len(), 1);

        write_file(&path, &[record("b")]).expect("rewrite");
        assert_eq!(read_file(&path).len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(test)]
mod part_tests {
    use super::*;

    #[test]
    fn las_partes_sobreviven_al_json() {
        let message = StoredMessage {
            role: "assistant".into(),
            content: "done".into(),
            reasoning: None,
            agent: None,
            model: None,
            tools: Vec::new(),
            images: Vec::new(),
            compaction: false,
            parts: vec![
                MessagePart::Reasoning { text: "thinking".into() },
                MessagePart::Text { text: "done".into() },
                MessagePart::ToolCall {
                    id: "call_1".into(),
                    name: "read".into(),
                    arguments: "{\"path\":\"a.txt\"}".into(),
                },
                MessagePart::ToolResult {
                    id: "call_1".into(),
                    output: "hola".into(),
                    is_error: false,
                },
            ],
        };

        let json = serde_json::to_string(&message).unwrap();
        let back: StoredMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.parts.len(), 4);
        assert_eq!(back.parts[2], message.parts[2]);
    }

    #[test]
    fn migra_sesiones_sin_partes() {
        let mut message = StoredMessage {
            role: "assistant".into(),
            content: "answer".into(),
            reasoning: Some("why".into()),
            agent: None,
            model: None,
            parts: Vec::new(),
            tools: Vec::new(),
            images: Vec::new(),
            compaction: false,
        };
        let mut record = SessionRecord {
            id: "s1".into(),
            title: "T".into(),
            project_id: None,
            agent_id: "build".into(),
            model: None,
            created_at: 0,
            updated_at: 0,
            messages: vec![message.clone()],
        };
        normalize(std::slice::from_mut(&mut record));
        message = record.messages[0].clone();
        assert_eq!(
            message.parts[0],
            MessagePart::Reasoning { text: "why".into() }
        );
        assert_eq!(message.parts[1], MessagePart::Text { text: "answer".into() });
    }
}

#[cfg(test)]
mod search_tests {
    use super::*;

    fn record(id: &str, updated_at: i64, texts: &[&str]) -> SessionRecord {
        SessionRecord {
            id: id.into(),
            title: format!("title {id}"),
            project_id: None,
            agent_id: String::new(),
            model: None,
            created_at: 0,
            updated_at,
            messages: texts
                .iter()
                .map(|t| StoredMessage {
                    role: "user".into(),
                    content: (*t).into(),
                    reasoning: None,
                    agent: None,
                    model: None,
                    parts: vec![],
                    tools: vec![],
                    images: Vec::new(),
                    compaction: false,
                })
                .collect(),
        }
    }

    #[test]
    fn finds_text_ignoring_case_and_accents_newest_first() {
        let records = vec![
            record("old", 10, &["Cómo cambio la CONFIGURACIÓN del router"]),
            record("new", 20, &["hola", "la configuracion está en config.json"]),
            record("none", 30, &["nada que ver"]),
        ];
        let hits = search_in(&records, "configuracion", 10);
        assert_eq!(hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), ["new", "old"]);
        assert!(hits[0].snippet.contains("configuracion"));
        assert!(search_in(&records, "   ", 10).is_empty());
    }

    #[test]
    fn snippets_are_cut_around_the_match_without_breaking_characters() {
        let text = format!("{}ñandú objetivo ñoño{}", "á".repeat(80), "é".repeat(120));
        let hits = search_in(&[record("a", 1, &[&text])], "objetivo", 5);
        let snippet = &hits[0].snippet;
        assert!(snippet.starts_with('…') && snippet.ends_with('…'));
        assert!(snippet.contains("objetivo"));
    }
}
