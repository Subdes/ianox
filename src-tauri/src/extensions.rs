//! Extensiones: servidores MCP, complementos, habilidades (skills) y servidores LSP.
//!
//! Discovers what is installed on the system and in the project:
//! - Complementos: `~/.config/ianox/plugins/<id>/plugin.json` y `<proyecto>/.ianox/plugins`
//! - Habilidades: `~/.config/ianox/skills/<id>/SKILL.md` y `<proyecto>/.ianox/skills`
//! - LSP: binarios de servidores de lenguaje presentes en el PATH

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct SkillInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub path: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LspInfo {
    pub id: String,
    pub label: String,
    pub languages: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ExtensionsSummary {
    pub mcp: usize,
    pub plugins: usize,
    pub skills: usize,
    pub lsp: usize,
}

fn config_dir() -> Option<PathBuf> {
    let dir = std::env::var("APPDATA")
        .or_else(|_| std::env::var("XDG_CONFIG_HOME"))
        .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.config", h)))
        .ok()?;
    Some(PathBuf::from(dir).join("ianox"))
}

fn roots(project: Option<&str>, kind: &str) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    if let Some(base) = config_dir() {
        out.push((base.join(kind), "usuario".to_string()));
    }
    if let Some(project) = project {
        out.push((Path::new(project).join(".ianox").join(kind), "proyecto".to_string()));
    }
    out
}

fn read_fence(text: &str, key: &str) -> Option<String> {
    let mut in_front = false;
    for line in text.lines() {
        let line = line.trim();
        if line == "---" {
            if in_front {
                break;
            }
            in_front = true;
            continue;
        }
        if !in_front {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case(key) {
                return Some(v.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

fn file_stem(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub fn list_skills(project: Option<&str>) -> Vec<SkillInfo> {
    let mut out = Vec::new();
    for (root, scope) in roots(project, "skills") {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            let file = if dir.is_dir() {
                dir.join("SKILL.md")
            } else {
                dir.clone()
            };
            if !file.is_file() || file.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let id = if dir.is_dir() {
                file_stem(&dir)
            } else {
                file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
            };
            out.push(SkillInfo {
                name: read_fence(&text, "name").unwrap_or_else(|| id.clone()),
                description: read_fence(&text, "description").unwrap_or_default(),
                id,
                path: file.to_string_lossy().to_string(),
                scope: scope.clone(),
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn list_plugins(project: Option<&str>) -> Vec<PluginInfo> {
    let mut out = Vec::new();
    for (root, scope) in roots(project, "plugins") {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            let manifest = if dir.is_dir() {
                dir.join("plugin.json")
            } else {
                dir.clone()
            };
            if !manifest.is_file() {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&manifest) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let id = if dir.is_dir() {
                file_stem(&dir)
            } else {
                manifest
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            };
            out.push(PluginInfo {
                name: json
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&id)
                    .to_string(),
                description: json
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                version: json
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0.0.0")
                    .to_string(),
                id,
                path: manifest.to_string_lossy().to_string(),
                scope: scope.clone(),
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

const LSP_SERVERS: &[(&str, &str, &str, &[&str])] = &[
    ("rust-analyzer", "rust-analyzer", "Rust", &["rust-analyzer"]),
    ("typescript", "TypeScript / JavaScript", "TypeScript, JavaScript", &["typescript-language-server"]),
    ("pyright", "Pyright", "Python", &["pyright-langserver", "pyright"]),
    ("gopls", "gopls", "Go", &["gopls"]),
    ("clangd", "clangd", "C, C++", &["clangd"]),
    ("lua", "Lua Language Server", "Lua", &["lua-language-server"]),
    ("jdtls", "Eclipse JDT", "Java", &["jdtls"]),
    ("intelephense", "Intelephense", "PHP", &["intelephense"]),
    ("bash", "Bash Language Server", "Shell", &["bash-language-server"]),
    ("yaml", "YAML Language Server", "YAML", &["yaml-language-server"]),
    ("json", "JSON Language Server", "JSON", &["vscode-json-language-server"]),
    ("html", "HTML Language Server", "HTML", &["vscode-html-language-server"]),
    ("css", "CSS Language Server", "CSS", &["vscode-css-language-server"]),
];

fn which(bin: &str) -> Option<String> {
    let out = std::process::Command::new("which").arg(bin).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!path.is_empty()).then_some(path)
}

pub fn detect_lsp() -> Vec<LspInfo> {
    let mut out = Vec::new();
    for (id, label, languages, bins) in LSP_SERVERS {
        if let Some(path) = bins.iter().find_map(|b| which(b)) {
            out.push(LspInfo {
                id: (*id).to_string(),
                label: (*label).to_string(),
                languages: (*languages).to_string(),
                path,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_frontmatter_de_una_habilidad() {
        let text = "---\nname: mi-skill\ndescription: Hace algo\n---\n\ncuerpo";
        assert_eq!(read_fence(text, "name").as_deref(), Some("mi-skill"));
        assert_eq!(read_fence(text, "description").as_deref(), Some("Hace algo"));
        assert!(read_fence(text, "otra").is_none());
    }

    #[test]
    fn detecta_lsp_sin_panico() {
        let list = detect_lsp();
        for item in &list {
            assert!(!item.label.is_empty());
        }
    }
}
