//! Agent loop bookkeeping.
//!
//! The provider streams tool calls; the loop appends the assistant message that
//! requested them plus one `tool` message per result, then asks the model again
//! until it answers without tools.

#![allow(dead_code)]

use crate::router::ToolCall;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Outcome of running one tool.
#[derive(serde::Serialize)]
pub struct ToolOutcome {
    pub call_id: String,
    pub output: String,
    pub is_error: bool,
    /// Diff of what the call changed in a file, when it changed one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<crate::diff::FileDiff>,
}

impl ToolOutcome {
    fn ok(call_id: String, output: String, diff: Option<crate::diff::FileDiff>) -> Self {
        Self { call_id, output, is_error: false, diff }
    }

    fn failed(call_id: String, output: String) -> Self {
        Self { call_id, output, is_error: true, diff: None }
    }
}

/// Text of the file a `write`/`edit` call is about to touch, read before it runs
/// so the diff can show what actually changed. A file that does not exist yet counts
/// as empty, so creating one shows every line as added. Binary and huge files are skipped.
fn file_before(project: Option<&str>, name: &str, args: &Value) -> Option<(String, String)> {
    if name != "write" && name != "edit" {
        return None;
    }
    let path = args.get("path").and_then(Value::as_str)?.to_string();
    let Ok(bytes) = std::fs::read(crate::tools::resolve(project, &path)) else {
        // Not there (yet): the whole file will be new.
        return Some((path, String::new()));
    };
    if bytes.len() > 512 * 1024 || bytes.iter().take(8192).any(|b| *b == 0) {
        return None;
    }
    Some((path, String::from_utf8_lossy(&bytes).to_string()))
}

/// Diff between [`file_before`] and the file as it is now.
fn file_diff(
    project: Option<&str>,
    before: Option<(String, String)>,
) -> Option<crate::diff::FileDiff> {
    let (path, text) = before?;
    let after = std::fs::read(crate::tools::resolve(project, &path)).ok()?;
    if after.iter().take(8192).any(|b| *b == 0) {
        return None;
    }
    crate::diff::diff(&path, &text, &String::from_utf8_lossy(&after))
}

/// OpenAI-compatible assistant message carrying the requested calls.
pub fn assistant_tool_message(calls: &[ToolCall], text: &str, reasoning: &str) -> Value {
    let calls: Vec<Value> = calls
        .iter()
        .map(|call| {
            json!({
                "id": call.id,
                "type": "function",
                "function": { "name": call.name, "arguments": call.arguments }
            })
        })
        .collect();

    let content = if text.trim().is_empty() { Value::Null } else { Value::String(text.to_string()) };
    let mut message = json!({ "role": "assistant", "content": content, "tool_calls": calls });
    if !reasoning.is_empty() {
        message["reasoning_content"] = Value::String(reasoning.to_string());
    }
    message
}

/// OpenAI-compatible tool result message.
pub fn tool_result_message(outcome: &ToolOutcome) -> Value {
    json!({
        "role": "tool",
        "tool_call_id": outcome.call_id,
        "content": outcome.output,
    })
}

/// Appends a full round: assistant request + every result.
pub fn append_tool_round(
    messages: &mut Vec<Value>,
    calls: &[ToolCall],
    outcomes: &[ToolOutcome],
    text: &str,
    reasoning: &str,
) {
    messages.push(assistant_tool_message(calls, text, reasoning));
    for outcome in outcomes {
        messages.push(tool_result_message(outcome));
    }
}

/// Runs the advertised tools for a set of calls, collecting outcomes.
pub fn run_tools(project: Option<&str>, mode: &str, calls: &[ToolCall]) -> Vec<ToolOutcome> {
    calls
        .iter()
        .map(|call| {
            let args: Value = match serde_json::from_str(&call.arguments) {
                Ok(value) => value,
                Err(_) => {
                    // Usually the response hit the output limit mid-call.
                    return ToolOutcome::failed(
                        call.id.clone(),
                        format!(
                            "{}: the arguments are not valid JSON (the output was probably cut off). Retry with a shorter payload, e.g. write the file in smaller pieces.",
                            call.name
                        ),
                    );
                }
            };
            let before = file_before(project, &call.name, &args);
            match crate::tools::execute(project, mode, &call.name, &args, false) {
                Ok(output) => {
                    let diff = file_diff(project, before);
                    ToolOutcome::ok(call.id.clone(), output, diff)
                }
                Err(error) => ToolOutcome::failed(call.id.clone(), error),
            }
        })
        .collect()
}

// ── Permissions: what the agent must ask the user before doing ──

/// A question for the user, sent to the UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PermissionAsk {
    pub id: String,
    pub tool: String,
    /// What is about to happen: the command, or the path to read.
    pub summary: String,
    /// Folder it concerns (for reads outside the project).
    pub detail: String,
    /// The command will run in a sandbox that can only write inside the project.
    pub sandboxed: bool,
    /// "Allow for this conversation" is offered.
    pub can_remember: bool,
    /// Machine-readable reason to warn about (the UI words it), e.g. "sensitive".
    pub warning: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Need {
    /// Runs without asking. `outside_read` is set when the user themselves named the outside path.
    Free { outside_read: bool },
    Ask {
        /// Allowance key: answering "for this conversation" remembers it.
        key: String,
        summary: String,
        detail: String,
        warning: Option<String>,
        can_remember: bool,
    },
}

/// Absolute paths the user wrote in their own messages (or attached), in canonical form.
/// File contents inside attachments are ignored: a file must not be able to grant itself access.
pub fn mentioned_paths(user_texts: &[&str]) -> Vec<PathBuf> {
    let home = std::env::var("HOME").ok();
    let mut found = Vec::new();
    for text in user_texts {
        // keep only what the user typed plus the path of each attachment
        let mut typed = String::new();
        let mut rest = *text;
        while let Some(at) = rest.find("<file path=\"") {
            typed.push_str(&rest[..at]);
            let after = &rest[at + "<file path=\"".len()..];
            let Some(end) = after.find('"') else { break };
            typed.push(' ');
            typed.push_str(&after[..end]);
            typed.push(' ');
            rest = match after.find("</file>") {
                Some(close) => &after[close + "</file>".len()..],
                None => "",
            };
        }
        typed.push_str(rest);

        let is_delim = |c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ',');
        for token in typed.split(is_delim) {
            let token = token.trim_end_matches(|c: char| matches!(c, '.' | ';' | ':' | '!' | '?'));
            let expanded = if let Some(tail) = token.strip_prefix("~/") {
                match &home {
                    Some(h) => format!("{h}/{tail}"),
                    None => continue,
                }
            } else if token.starts_with('/') && token.len() > 1 {
                token.to_string()
            } else {
                continue;
            };
            if let Ok(path) = crate::tools::canonical_lenient(Path::new(&expanded)) {
                found.push(path);
            }
        }
    }
    found
}

fn is_sensitive(target: &Path) -> bool {
    let secret_dir = |name: &str| target.components().any(|c| c.as_os_str() == name);
    if [".ssh", ".gnupg", ".aws", ".kube"].iter().any(|n| secret_dir(n)) {
        return true;
    }
    if target == Path::new("/etc/shadow") {
        return true;
    }
    // ianox's own config holds the API keys of every provider
    std::env::var("HOME")
        .map(|h| target.starts_with(Path::new(&h).join(".config/ianox")))
        .unwrap_or(false)
}

/// Decides whether a tool call may run on its own or the user has to be asked first.
/// Writing outside the project is not asked about: it is refused by the tools themselves.
pub fn needs_permission(project: Option<&str>, mode: &str, call: &ToolCall, user_paths: &[PathBuf]) -> Need {
    let args: Value = serde_json::from_str(&call.arguments).unwrap_or(Value::Null);
    match call.name.as_str() {
        // The read-only mode refuses the command by itself; there is nothing to ask.
        "shell" if mode != "plan" => Need::Ask {
            key: "shell".into(),
            summary: args.get("command").and_then(Value::as_str).unwrap_or("").to_string(),
            detail: String::new(),
            warning: None,
            can_remember: true,
        },
        "read" => {
            let Some(path) = args.get("path").and_then(Value::as_str) else {
                return Need::Free { outside_read: false };
            };
            if !crate::tools::is_outside_project(project, path) {
                return Need::Free { outside_read: false };
            }
            let target = crate::tools::canonical_lenient(&crate::tools::resolve(project, path))
                .unwrap_or_else(|_| crate::tools::resolve(project, path));
            let sensitive = is_sensitive(&target);
            if !sensitive && user_paths.iter().any(|m| target.starts_with(m)) {
                return Need::Free { outside_read: true };
            }
            let folder = target.parent().unwrap_or(&target).to_string_lossy().to_string();
            Need::Ask {
                key: format!("read:{folder}"),
                summary: target.to_string_lossy().to_string(),
                detail: folder,
                warning: sensitive.then(|| "sensitive".to_string()),
                can_remember: !sensitive,
            }
        }
        _ => Need::Free { outside_read: false },
    }
}

/// Asks the UI and waits for the answer: "once", "always" or "deny" (also when stopped).
async fn ask_user<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    task_id: &str,
    ask: PermissionAsk,
    cancel: &Arc<AtomicBool>,
) -> String {
    use tauri::{Emitter, Manager};
    let state = app.state::<crate::config::AppState>();
    let (tx, mut rx) = tokio::sync::oneshot::channel::<String>();
    state.permission_waiters.lock().await.insert(ask.id.clone(), tx);
    let _ = app.emit(&format!("chat://permission/{task_id}"), &ask);

    let answer = loop {
        tokio::select! {
            reply = &mut rx => break reply.unwrap_or_else(|_| "deny".into()),
            _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {
                if cancel.load(Ordering::Relaxed) {
                    break "deny".into();
                }
            }
        }
    };
    state.permission_waiters.lock().await.remove(&ask.id);
    answer
}

/// Runs the calls of one round, asking the user wherever [`needs_permission`] says so.
pub async fn run_tools_checked<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    task_id: &str,
    session_key: &str,
    project: Option<&str>,
    mode: &str,
    calls: &[ToolCall],
    user_paths: &[PathBuf],
    cancel: &Arc<AtomicBool>,
) -> Vec<ToolOutcome> {
    use tauri::Manager;
    let state = app.state::<crate::config::AppState>();
    let mut outcomes = Vec::new();

    for call in calls {
        let outside_read = match needs_permission(project, mode, call, user_paths) {
            Need::Free { outside_read: allowed } => allowed,
            Need::Ask { key, summary, detail, warning, can_remember } => {
                let allowance = format!("{session_key}:{key}");
                let remembered = can_remember && state.allowances.lock().await.contains(&allowance);
                let answer = if remembered {
                    "once".to_string()
                } else {
                    let ask = PermissionAsk {
                        id: uuid::Uuid::new_v4().to_string(),
                        tool: call.name.clone(),
                        summary,
                        detail,
                        sandboxed: call.name == "shell" && crate::tools::sandbox_available(),
                        can_remember,
                        warning,
                    };
                    ask_user(app, task_id, ask, cancel).await
                };
                if answer == "deny" {
                    outcomes.push(ToolOutcome::failed(
                        call.id.clone(),
                        "The user denied this action. Do not retry it; continue without it or ask the user how to proceed.".into(),
                    ));
                    continue;
                }
                if answer == "always" && can_remember {
                    let snapshot = {
                        let mut allowances = state.allowances.lock().await;
                        allowances.insert(allowance);
                        allowances.clone()
                    };
                    crate::permissions::save(&snapshot);
                }
                // Allowed: a read outside the project may go ahead.
                call.name == "read"
            }
        };

        // Tools block (files, processes): keep them off the async threads.
        let (project, mode, call_owned) = (project.map(str::to_string), mode.to_string(), call.clone());
        let outcome = tokio::task::spawn_blocking(move || {
            let args: Value = match serde_json::from_str(&call_owned.arguments) {
                Ok(value) => value,
                Err(_) => {
                    return ToolOutcome::failed(
                        call_owned.id.clone(),
                        format!(
                            "{}: the arguments are not valid JSON (the output was probably cut off). Retry with a shorter payload, e.g. write the file in smaller pieces.",
                            call_owned.name
                        ),
                    )
                }
            };
            let before = file_before(project.as_deref(), &call_owned.name, &args);
            match crate::tools::execute(project.as_deref(), &mode, &call_owned.name, &args, outside_read) {
                Ok(output) => {
                    let diff = file_diff(project.as_deref(), before);
                    ToolOutcome::ok(call_owned.id.clone(), output, diff)
                }
                Err(error) => ToolOutcome::failed(call_owned.id.clone(), error),
            }
        })
        .await
        .unwrap_or_else(|e| ToolOutcome::failed(call.id.clone(), format!("the tool crashed: {e}")));
        outcomes.push(outcome);
    }
    outcomes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call() -> ToolCall {
        ToolCall {
            id: "call_1".into(),
            name: "read".into(),
            arguments: "{\"path\":\"a.txt\"}".into(),
        }
    }

    #[test]
    fn an_edit_reports_the_lines_it_changed() {
        let dir = std::env::temp_dir().join(format!("ianox-diff-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        let project = dir.to_str().unwrap();

        let edit = ToolCall {
            id: "c1".into(),
            name: "edit".into(),
            arguments: json!({ "path": "a.txt", "old": "two", "new": "TWO" }).to_string(),
        };
        let outcomes = run_tools(Some(project), "build", &[edit]);

        let diff = outcomes[0].diff.as_ref().expect("el diff debería venir del backend");
        assert_eq!(diff.path, "a.txt");
        assert_eq!(diff.added, 1);
        assert_eq!(diff.removed, 1);
        assert!(diff.lines.iter().any(|line| line.kind == "del" && line.text == "two"));
        assert!(diff.lines.iter().any(|line| line.kind == "add" && line.text == "TWO"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn overwriting_a_new_file_shows_every_line_as_added() {
        let dir = std::env::temp_dir().join(format!("ianox-diff-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let project = dir.to_str().unwrap();

        let write = ToolCall {
            id: "c2".into(),
            name: "write".into(),
            arguments: json!({ "path": "b.txt", "content": "hello\nworld\n" }).to_string(),
        };
        let outcomes = run_tools(Some(project), "build", &[write]);

        let diff = outcomes[0].diff.as_ref().expect("un archivo nuevo también tiene diff");
        assert_eq!(diff.added, 2);
        assert_eq!(diff.removed, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_read_has_no_diff() {
        let dir = std::env::temp_dir().join(format!("ianox-diff-read-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();

        let outcomes = run_tools(Some(dir.to_str().unwrap()), "build", &[call()]);
        assert!(outcomes[0].diff.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn builds_the_assistant_and_tool_messages() {
        let mut messages = vec![json!({ "role": "user", "content": "hi" })];
        let outcome = ToolOutcome::ok("call_1".into(), "file body".into(), None);
        append_tool_round(&mut messages, &[call()], &[outcome], "", "");

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["tool_calls"][0]["function"]["name"], "read");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "call_1");
        assert_eq!(messages[2]["content"], "file body");
    }

    #[test]
    fn truncated_arguments_report_the_real_cause() {
        let mut bad = call();
        bad.arguments = "{\"path\":\"a.txt\",\"content\":\"half".into();
        let outcomes = run_tools(None, "build", &[bad]);
        assert!(outcomes[0].is_error);
        assert!(outcomes[0].output.contains("not valid JSON"));
    }

    #[test]
    fn tool_errors_become_error_outcomes() {
        let outcomes = run_tools(None, "build", &[call()]);
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].is_error);
    }

    fn tool_call(name: &str, args: Value) -> ToolCall {
        ToolCall { id: "c".into(), name: name.into(), arguments: args.to_string() }
    }

    #[test]
    fn shell_always_asks_but_not_in_plan_mode() {
        let c = tool_call("shell", json!({"command":"ls"}));
        assert!(matches!(needs_permission(Some("/p"), "build", &c, &[]), Need::Ask { .. }));
        assert_eq!(needs_permission(Some("/p"), "plan", &c, &[]), Need::Free { outside_read: false });
    }

    #[test]
    fn reads_outside_ask_unless_the_user_named_the_path() {
        let proj = std::env::current_dir().unwrap();
        let p = proj.to_str().unwrap();
        // inside the project: free
        let inside = tool_call("read", json!({"path":"Cargo.toml"}));
        assert_eq!(needs_permission(Some(p), "build", &inside, &[]), Need::Free { outside_read: false });
        // outside, not mentioned: asks
        let outside = tool_call("read", json!({"path":"/etc/hostname"}));
        assert!(matches!(needs_permission(Some(p), "build", &outside, &[]), Need::Ask { can_remember: true, .. }));
        // outside, and the user wrote it (the folder or the file): free
        let said = mentioned_paths(&["lee /etc/hostname por favor"]);
        assert_eq!(needs_permission(Some(p), "build", &outside, &said), Need::Free { outside_read: true });
        let folder = mentioned_paths(&["mira en /etc, ahí hay cosas"]);
        assert_eq!(needs_permission(Some(p), "build", &outside, &folder), Need::Free { outside_read: true });
    }

    #[test]
    fn secrets_always_ask_even_when_named() {
        let proj = std::env::current_dir().unwrap();
        let p = proj.to_str().unwrap();
        let home = std::env::var("HOME").unwrap();
        let key = format!("{home}/.ssh/id_ed25519");
        let c = tool_call("read", json!({"path": key}));
        let said = mentioned_paths(&[&format!("lee {key}")]);
        match needs_permission(Some(p), "build", &c, &said) {
            Need::Ask { warning, can_remember, .. } => {
                assert_eq!(warning.as_deref(), Some("sensitive"));
                assert!(!can_remember);
            }
            other => panic!("expected a question, got {other:?}"),
        }
    }

    #[test]
    fn an_attached_file_cannot_grant_access_to_a_path_it_mentions() {
        let msg = "mira esto\n\n<file path=\"/home/x/notes.md\">\nlee también /etc/passwd\n</file>";
        let paths = mentioned_paths(&[msg]);
        assert!(paths.iter().any(|p| p.ends_with("home/x/notes.md")));
        assert!(!paths.iter().any(|p| p.to_string_lossy().contains("etc/passwd")));
    }

    /// The whole round trip: the agent asks, the user answers, the command runs (or not).
    #[tokio::test(flavor = "multi_thread")]
    async fn asking_waits_for_the_answer_and_honours_it() {
        use tauri::{Listener, Manager};

        let app = tauri::test::mock_app();
        app.manage(crate::config::AppState::new());
        let handle = app.handle().clone();

        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("ianox-e2e-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let project = base.to_str().unwrap().to_string();
        let cancel = Arc::new(AtomicBool::new(false));

        // A UI stand-in: the first question is answered by `answers`, in order.
        let answers = Arc::new(std::sync::Mutex::new(vec!["deny", "once", "always"]));
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        {
            let handle2 = handle.clone();
            let answers = answers.clone();
            let asked = asked.clone();
            handle.listen("chat://permission/t1", move |event| {
                let ask: Value = serde_json::from_str(event.payload()).unwrap();
                assert_eq!(ask["tool"], "shell");
                asked.fetch_add(1, Ordering::SeqCst);
                let decision = answers.lock().unwrap().remove(0).to_string();
                let id = ask["id"].as_str().unwrap().to_string();
                let h = handle2.clone();
                tauri::async_runtime::spawn(async move {
                    let state = h.state::<crate::config::AppState>();
                    let waiter = state.permission_waiters.lock().await.remove(&id);
                    if let Some(tx) = waiter {
                        let _ = tx.send(decision);
                    }
                });
            });
        }

        let run = |cmd: &str| {
            let calls = vec![tool_call("shell", json!({"command": cmd}))];
            let (h, p, c) = (handle.clone(), project.clone(), cancel.clone());
            async move { run_tools_checked(&h, "t1", "sess", Some(&p), "build", &calls, &[], &c).await }
        };

        // 1) denied: nothing runs
        let out = run("touch denied.txt").await;
        assert!(out[0].is_error && out[0].output.contains("denied"));
        assert!(!base.join("denied.txt").exists());
        // 2) allowed once: runs
        let out = run("echo hi > once.txt").await;
        assert!(!out[0].is_error, "{}", out[0].output);
        assert!(base.join("once.txt").exists());
        // 3) allowed for the conversation: runs, and the next one is not asked about
        let _ = run("echo a > always1.txt").await;
        let _ = run("echo b > always2.txt").await;
        assert!(base.join("always1.txt").exists() && base.join("always2.txt").exists());
        assert_eq!(asked.load(Ordering::SeqCst), 3, "the remembered answer must not ask again");

        // 4) stopping while a question is open denies it instead of hanging
        let calls = vec![tool_call("read", json!({"path": "/etc/hostname"}))];
        let stopper = cancel.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            stopper.store(true, Ordering::Relaxed);
        });
        let out = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            run_tools_checked(&handle, "t-none", "sess2", Some(&project), "build", &calls, &[], &cancel),
        )
        .await
        .expect("a pending question must not block forever once the user stops");
        assert!(out[0].is_error && out[0].output.contains("denied"));

        let _ = std::fs::remove_dir_all(&base);
    }
}
