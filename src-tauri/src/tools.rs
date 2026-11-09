//! Tool registry for the agent loop.
//!
//! Each tool declares a JSON schema and executes with the arguments the model
//! returned. Read-only tools come first; write/shell will need permissions.

#![allow(dead_code)]

use serde_json::{json, Value};

/// Tool definitions advertised to the provider.
pub fn definitions(mode: &str) -> Vec<Value> {
    let mut tools = vec![json!({
        "type": "function",
        "function": {
            "name": "read",
            "description": "Read a text file from the workspace.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute path or path relative to the project" },
                    "offset": { "type": "integer", "description": "First line to read (1-based)" },
                    "limit": { "type": "integer", "description": "Maximum number of lines" }
                },
                "required": ["path"]
            }
        }
    }), json!({
        "type": "function",
        "function": {
            "name": "glob",
            "description": "List files in the workspace matching a wildcard pattern.",
            "parameters": {
                "type": "object",
                "properties": { "pattern": { "type": "string", "description": "e.g. src/**/*.ts" } },
                "required": ["pattern"]
            }
        }
    }), json!({
        "type": "function",
        "function": {
            "name": "grep",
            "description": "Search text files in the workspace for a literal string.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "pattern": { "type": "string", "description": "Optional file wildcard, e.g. src/**/*.rs" }
                },
                "required": ["query"]
            }
        }
    })];

    if mode != "plan" {
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "write",
                "description": "Create or overwrite a text file in the workspace.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["path", "content"]
                }
            }
        }));
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "edit",
                "description": "Replace an exact string in a file.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "old": { "type": "string" },
                        "new": { "type": "string" }
                    },
                    "required": ["path", "old", "new"]
                }
            }
        }));
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "shell",
                "description": "Run a shell command in the workspace.",
                "parameters": {
                    "type": "object",
                    "properties": { "command": { "type": "string" } },
                    "required": ["command"]
                }
            }
        }));
    }

    tools
}

fn require_str(args: &Value, key: &str, tool: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("{tool}: missing '{key}'"))
}

/// Minimal wildcard matcher: `*` within a segment, `**` across segments, `?` one char.
fn wildcard(pattern: &str, text: &str) -> bool {
    fn matches(p: &[char], t: &[char]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        if p[0] == '*' && p.len() > 1 && p[1] == '*' {
            let rest = &p[2..];
            // `**/` may match zero directories.
            if rest.first() == Some(&'/') && matches(&rest[1..], t) {
                return true;
            }
            for i in 0..=t.len() {
                if matches(rest, &t[i..]) {
                    return true;
                }
            }
            return false;
        }
        if p[0] == '*' {
            for i in 0..=t.len() {
                if i > 0 && t[i - 1] == '/' {
                    break;
                }
                if matches(&p[1..], &t[i..]) {
                    return true;
                }
            }
            return false;
        }
        if t.is_empty() {
            return false;
        }
        if p[0] == '?' && t[0] != '/' || p[0] == t[0] {
            return matches(&p[1..], &t[1..]);
        }
        false
    }

    matches(
        &pattern.chars().collect::<Vec<_>>(),
        &text.chars().collect::<Vec<_>>(),
    )
}

const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", "dist", ".ianox", "build"];

pub fn walk(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>, depth: usize) {
    if depth > 12 || out.len() > 5000 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) {
                walk(&path, out, depth + 1);
            }
        } else {
            out.push(path);
        }
    }
}

const MAX_BYTES: usize = 200 * 1024;

pub fn resolve(project: Option<&str>, path: &str) -> std::path::PathBuf {
    let candidate = std::path::Path::new(path);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else if let Some(project) = project {
        std::path::Path::new(project).join(candidate)
    } else {
        candidate.to_path_buf()
    }
}

// ── Path policy: the agent may read from anywhere the user allows, but only write inside the project ──

/// Canonical form of a path that may not exist yet: the nearest existing ancestor is resolved
/// (following symlinks) and the missing tail is appended, applying `..` lexically.
pub fn canonical_lenient(path: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    use std::path::Component;
    let mut existing = path.to_path_buf();
    while !existing.exists() {
        match existing.parent() {
            Some(parent) => existing = parent.to_path_buf(),
            None => break,
        }
    }
    let mut out = existing.canonicalize()?;
    let tail = path.strip_prefix(&existing).unwrap_or_else(|_| std::path::Path::new(""));
    for component in tail.components() {
        match component {
            Component::Normal(name) => out.push(name),
            Component::ParentDir => {
                out.pop();
            }
            _ => {}
        }
    }
    Ok(out)
}

/// The project folder in canonical form.
pub fn project_root(project: Option<&str>) -> Result<std::path::PathBuf, String> {
    let project = project.ok_or_else(|| "open a project first".to_string())?;
    std::fs::canonicalize(project).map_err(|e| format!("the project folder is not available: {e}"))
}

/// Whether `path` (resolved against the project) lies outside the project folder.
/// With no project open everything counts as outside.
pub fn is_outside_project(project: Option<&str>, path: &str) -> bool {
    let Ok(root) = project_root(project) else {
        return true;
    };
    match canonical_lenient(&resolve(project, path)) {
        Ok(full) => !full.starts_with(&root),
        Err(_) => true,
    }
}

/// Resolves a path for writing; refuses anything that ends up outside the project folder,
/// whether through an absolute path, `..` or a symlink.
fn check_write(project: Option<&str>, tool: &str, path: &str) -> Result<std::path::PathBuf, String> {
    let root = project_root(project).map_err(|e| format!("{tool}: {e}"))?;
    let full = canonical_lenient(&resolve(project, path)).map_err(|e| format!("{tool}: invalid path {path}: {e}"))?;
    if !full.starts_with(&root) {
        return Err(format!(
            "{tool}: {path} is outside the project folder; writing is only allowed inside {}",
            root.display()
        ));
    }
    Ok(full)
}

/// Runs a shell command so that it can only write inside the project (and /tmp).
/// Uses bubblewrap when it is installed; returns whether the command was sandboxed.
pub fn shell_command(root: &std::path::Path, command: &str) -> (std::process::Command, bool) {
    if sandbox_available() {
        let mut cmd = std::process::Command::new("bwrap");
        cmd.args(["--ro-bind", "/", "/", "--bind"])
            .arg(root)
            .arg(root)
            .args(["--bind", "/tmp", "/tmp", "--dev", "/dev", "--proc", "/proc", "--die-with-parent", "--chdir"])
            .arg(root)
            .args(["sh", "-c", command]);
        (cmd, true)
    } else {
        #[cfg(not(windows))]
        let mut cmd = {
            let mut cmd = std::process::Command::new("sh");
            cmd.arg("-c").arg(command);
            cmd
        };
        #[cfg(windows)]
        let mut cmd = {
            let mut cmd = std::process::Command::new("cmd");
            cmd.arg("/C").arg(command);
            cmd
        };
        cmd.current_dir(root);
        (cmd, false)
    }
}

/// Whether commands can be sandboxed on this machine.
pub fn sandbox_available() -> bool {
    use std::sync::OnceLock;
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        cfg!(target_os = "linux")
            && std::process::Command::new("bwrap")
                .args(["--ro-bind", "/", "/", "true"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
    })
}

/// Runs a tool and returns the text shown to the model.
/// Reads outside the project only run when `allow_outside_read` is set (the user said yes).
pub fn execute(
    project: Option<&str>,
    mode: &str,
    name: &str,
    args: &Value,
    allow_outside_read: bool,
) -> Result<String, String> {
    match name {
        "read" => {
            let path = args
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| "read: missing 'path'".to_string())?;
            if !allow_outside_read && is_outside_project(project, path) {
                return Err(format!("read: {path} is outside the project folder and needs the user's permission"));
            }
            let full = resolve(project, path);
            let bytes = std::fs::read(&full)
                .map_err(|e| format!("read: could not open {}: {e}", full.display()))?;
            if bytes.iter().take(8192).any(|b| *b == 0) {
                return Err("read: binary file".into());
            }
            let text = String::from_utf8_lossy(&bytes);
            let offset = args
                .get("offset")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .max(1) as usize;
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(2000)
                .clamp(1, 5000) as usize;

            let mut out = String::new();
            for (index, line) in text.lines().enumerate().skip(offset - 1).take(limit) {
                out.push_str(&format!("{:>6}\t{}\n", index + 1, line));
                if out.len() > MAX_BYTES {
                    out.push_str("… truncated\n");
                    break;
                }
            }
            Ok(out)
        }
        "glob" => {
            let pattern = args
                .get("pattern")
                .and_then(Value::as_str)
                .ok_or_else(|| "glob: missing 'pattern'".to_string())?;
            let root = project
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            let mut files = Vec::new();
            walk(&root, &mut files, 0);

            let mut hits: Vec<String> = files
                .iter()
                .filter_map(|path| {
                    let relative = path.strip_prefix(&root).ok()?.to_string_lossy().to_string();
                    wildcard(pattern, &relative).then_some(relative)
                })
                .collect();
            hits.sort();
            hits.truncate(500);
            Ok(if hits.is_empty() {
                "No files match.".to_string()
            } else {
                hits.join("\n")
            })
        }
        "grep" => {
            let query = args
                .get("query")
                .and_then(Value::as_str)
                .ok_or_else(|| "grep: missing 'query'".to_string())?;
            let pattern = args.get("pattern").and_then(Value::as_str).unwrap_or("*");
            let root = project
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            let mut files = Vec::new();
            walk(&root, &mut files, 0);

            let mut out = String::new();
            for path in files {
                let Ok(relative) = path.strip_prefix(&root) else {
                    continue;
                };
                let relative = relative.to_string_lossy().to_string();
                if !wildcard(pattern, &relative) {
                    continue;
                }
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                if bytes.iter().take(4096).any(|b| *b == 0) {
                    continue;
                }
                for (index, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
                    if line.contains(query) {
                        out.push_str(&format!("{relative}:{}: {}\n", index + 1, line.trim()));
                        if out.len() > 60_000 {
                            out.push_str("… truncated\n");
                            return Ok(out);
                        }
                    }
                }
            }
            Ok(if out.is_empty() {
                "No matches.".to_string()
            } else {
                out
            })
        }
        "write" => {
            if mode == "plan" {
                return Err("write: read-only mode".into());
            }
            let path = require_str(args, "path", "write")?;
            let content = require_str(args, "content", "write")?;
            let full = check_write(project, "write", &path)?;
            if let Some(parent) = full.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&full, content)
                .map_err(|e| format!("write: could not save {}: {e}", full.display()))?;
            Ok(format!("Wrote {path}"))
        }
        "edit" => {
            if mode == "plan" {
                return Err("edit: read-only mode".into());
            }
            let path = require_str(args, "path", "edit")?;
            let old = require_str(args, "old", "edit")?;
            let new = require_str(args, "new", "edit")?;
            let full = check_write(project, "edit", &path)?;
            let text = std::fs::read_to_string(&full)
                .map_err(|e| format!("edit: could not read {path}: {e}"))?;
            let count = text.matches(&old).count();
            if count == 0 {
                return Err(format!("edit: '{old}' not found in {path}"));
            }
            std::fs::write(&full, text.replacen(&old, &new, 1))
                .map_err(|e| format!("edit: could not save {path}: {e}"))?;
            Ok(format!("Edited {path} ({count} match(es))"))
        }
        "shell" => {
            if mode == "plan" {
                return Err("shell: read-only mode".into());
            }
            let command = require_str(args, "command", "shell")?;
            let root = project_root(project).map_err(|e| format!("shell: {e}"))?;
            let (mut cmd, _sandboxed) = shell_command(&root, &command);
            let output = cmd
                .output()
                .map_err(|e| format!("shell: could not run: {e}"))?;
            let mut text = String::from_utf8_lossy(&output.stdout).to_string();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            if text.is_empty() {
                text = format!("(exit {})", output.status.code().unwrap_or(-1));
            }
            Ok(text.chars().take(20_000).collect())
        }
        other => Err(format!("unknown tool: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_file_with_line_numbers() {
        let dir = std::env::temp_dir().join(format!("ianox-tool-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();

        let out = execute(
            Some(dir.to_str().unwrap()),
            "build",
            "read",
            &json!({ "path": "a.txt", "offset": 2, "limit": 1 }),
            false,
        )
        .unwrap();

        assert!(out.contains("two"));
        assert!(!out.contains("one"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn matches_wildcards() {
        assert!(wildcard("src/**/*.rs", "src/a/b/c.rs"));
        assert!(wildcard("*.md", "README.md"));
        assert!(!wildcard("*.md", "src/README.md"));
        assert!(wildcard("src/?ain.rs", "src/main.rs"));
    }

    #[test]
    fn glob_and_grep_find_files() {
        let dir = std::env::temp_dir().join(format!("ianox-glob-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(dir.join("README.md"), "hello world\n").unwrap();

        let project = dir.to_str().unwrap();
        let files = execute(Some(project), "build", "glob", &json!({ "pattern": "**/*.rs" }), false).unwrap();
        assert!(files.contains("src/main.rs"));

        let hits = execute(Some(project), "build", "grep", &json!({ "query": "hello" }), false).unwrap();
        assert!(hits.contains("README.md:1"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_unknown_tools() {
        assert!(execute(None, "build", "nope", &json!({}), false).is_err());
    }

    #[test]
    fn requires_a_path() {
        assert!(execute(None, "build", "read", &json!({}), false).is_err());
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        // Not under /tmp: the sandbox leaves /tmp writable on purpose, which would hide a leak.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("ianox-policy-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("proj")).unwrap();
        dir
    }

    #[test]
    fn writes_stay_inside_the_project() {
        let base = scratch("w");
        let proj = base.join("proj");
        let p = proj.to_str().unwrap();
        // inside: fine, including new folders
        assert!(execute(Some(p), "build", "write", &json!({"path":"a/b.txt","content":"x"}), false).is_ok());
        assert!(proj.join("a/b.txt").exists());
        // outside: absolute path, `..`, and a not-yet-existing parent reached through `..`
        let outside = base.join("outside.txt");
        for path in [outside.to_str().unwrap(), "../outside.txt", "new/../../outside2.txt"] {
            let r = execute(Some(p), "build", "write", &json!({"path":path,"content":"x"}), false);
            assert!(r.is_err(), "{path} should be refused");
            assert!(r.unwrap_err().contains("outside the project"));
        }
        assert!(!outside.exists() && !base.join("outside2.txt").exists());
        // no project: nothing can be written
        assert!(execute(None, "build", "write", &json!({"path":"x.txt","content":"x"}), false).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_cannot_lead_a_write_outside() {
        let base = scratch("s");
        let proj = base.join("proj");
        std::fs::create_dir_all(base.join("elsewhere")).unwrap();
        std::os::unix::fs::symlink(base.join("elsewhere"), proj.join("link")).unwrap();
        let p = proj.to_str().unwrap();
        let r = execute(Some(p), "build", "write", &json!({"path":"link/evil.txt","content":"x"}), false);
        assert!(r.is_err());
        assert!(!base.join("elsewhere/evil.txt").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn reading_outside_needs_permission() {
        let base = scratch("r");
        let proj = base.join("proj");
        std::fs::write(base.join("secret.txt"), "hidden").unwrap();
        std::fs::write(proj.join("ok.txt"), "visible").unwrap();
        let p = proj.to_str().unwrap();
        let outside = base.join("secret.txt");
        let args = json!({"path": outside.to_str().unwrap()});
        assert!(execute(Some(p), "build", "read", &args, false).is_err());
        assert!(execute(Some(p), "build", "read", &args, true).unwrap().contains("hidden"));
        assert!(execute(Some(p), "build", "read", &json!({"path":"ok.txt"}), false).unwrap().contains("visible"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn shell_cannot_write_outside_when_sandboxed() {
        if !sandbox_available() {
            return; // nothing to assert on machines without bubblewrap
        }
        let base = scratch("sh");
        let proj = base.join("proj");
        let p = proj.to_str().unwrap();
        let inside = execute(Some(p), "build", "shell", &json!({"command":"echo hi > in.txt && cat in.txt"}), false).unwrap();
        assert!(inside.contains("hi"));
        let target = base.join("out.txt");
        let _ = execute(Some(p), "build", "shell", &json!({"command": format!("echo x > {}", target.display())}), false);
        assert!(!target.exists(), "the sandboxed command wrote outside the project");
        let _ = std::fs::remove_dir_all(&base);
    }
}
