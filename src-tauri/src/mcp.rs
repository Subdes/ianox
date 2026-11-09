//! Minimal MCP client over stdio (JSON-RPC 2.0, one line per message).
//!
//! Used to validate servers from the UI: it starts the process, performs the
//! `initialize` → `tools/list` handshake and closes it. It keeps no live session
//! yet; that will come with the agent loop.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStdin, ChildStdout, Command};
use tokio::time::{timeout, Duration};

const PROTOCOL_VERSION: &str = "2024-11-05";
const HANDSHAKE_TIMEOUT_SECS: u64 = 12;

#[derive(Debug, Clone, Serialize)]
pub struct McpToolInfo {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpTestResult {
    pub ok: bool,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
    pub protocol_version: Option<String>,
    pub tools: Vec<McpToolInfo>,
    pub error: Option<String>,
}

impl McpTestResult {
    fn failure(error: impl Into<String>) -> Self {
        Self {
            ok: false,
            server_name: None,
            server_version: None,
            protocol_version: None,
            tools: Vec::new(),
            error: Some(error.into()),
        }
    }
}

async fn send(stdin: &mut ChildStdin, message: &Value) -> Result<(), String> {
    let mut line = serde_json::to_string(message).map_err(|e| e.to_string())?;
    line.push('\n');
    stdin
        .write_all(line.as_bytes())
        .await
        .map_err(|e| format!("Could not write to the server: {e}"))?;
    stdin
        .flush()
        .await
        .map_err(|e| format!("Could not flush: {e}"))
}

async fn read_response(
    lines: &mut Lines<BufReader<ChildStdout>>,
    id: u64,
) -> Result<Value, String> {
    let fut = async {
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    let value: Value = match serde_json::from_str(line) {
                        Ok(v) => v,
                        Err(_) => continue, // línea no JSON (logs del servidor)
                    };
                    if value.get("id").and_then(Value::as_u64) != Some(id) {
                        continue; // notificación u otra respuesta
                    }
                    if let Some(err) = value.get("error") {
                        let msg = err
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("error JSON-RPC sin mensaje");
                        return Err(msg.to_string());
                    }
                    return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                }
                Ok(None) => return Err("The server closed the connection".to_string()),
                Err(e) => return Err(format!("Error leyendo la salida: {e}")),
            }
        }
    };

    match timeout(Duration::from_secs(HANDSHAKE_TIMEOUT_SECS), fut).await {
        Ok(result) => result,
        Err(_) => Err(format!(
            "Timeout of {} s waiting for response id={id}",
            HANDSHAKE_TIMEOUT_SECS
        )),
    }
}

/// Arranca el servidor MCP, hace `initialize` + `tools/list` y lo termina.
pub async fn probe(
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
) -> McpTestResult {
    if command.trim().is_empty() {
        return McpTestResult::failure("The command is empty");
    }

    let mut cmd = Command::new(command);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    for (key, value) in env {
        cmd.env(key, value);
    }

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            return McpTestResult::failure(format!(
                "Could not run '{command}': {e}"
            ))
        }
    };

    let mut stdin = match child.stdin.take() {
        Some(stdin) => stdin,
        None => return McpTestResult::failure("No process stdin"),
    };
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => return McpTestResult::failure("No process stdout"),
    };
    let mut lines = BufReader::new(stdout).lines();

    let init = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "ianox", "version": env!("CARGO_PKG_VERSION") }
        }
    });

    if let Err(e) = send(&mut stdin, &init).await {
        let _ = child.kill().await;
        return McpTestResult::failure(e);
    }

    let init_result = match read_response(&mut lines, 1).await {
        Ok(result) => result,
        Err(e) => {
            let _ = child.kill().await;
            return McpTestResult::failure(format!("initialize: {e}"));
        }
    };

    let server_name = init_result
        .pointer("/serverInfo/name")
        .and_then(Value::as_str)
        .map(str::to_string);
    let server_version = init_result
        .pointer("/serverInfo/version")
        .and_then(Value::as_str)
        .map(str::to_string);
    let protocol_version = init_result
        .get("protocolVersion")
        .and_then(Value::as_str)
        .map(str::to_string);

    // Mandatory notification after initialize (no id, no reply)
    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    let _ = send(&mut stdin, &initialized).await;

    let list = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });

    let mut tools = Vec::new();
    let mut tool_error = None;

    match send(&mut stdin, &list).await {
        Ok(()) => match read_response(&mut lines, 2).await {
            Ok(result) => {
                if let Some(items) = result.get("tools").and_then(Value::as_array) {
                    for item in items {
                        let name = item
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("(sin nombre)")
                            .to_string();
                        let description = item
                            .get("description")
                            .and_then(Value::as_str)
                            .map(str::to_string);
                        tools.push(McpToolInfo { name, description });
                    }
                }
            }
            Err(e) => tool_error = Some(format!("tools/list: {e}")),
        },
        Err(e) => tool_error = Some(e),
    }

    let _ = child.kill().await;

    McpTestResult {
        ok: true,
        server_name,
        server_version,
        protocol_version,
        tools,
        error: tool_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn probe_detects_tools() {
        if std::process::Command::new("node").arg("--version").output().is_err() {
            eprintln!("node no disponible; test omitido");
            return;
        }

        let script = r#"
const readline = require('readline');
const rl = readline.createInterface({ input: process.stdin });
rl.on('line', (line) => {
  let msg;
  try { msg = JSON.parse(line); } catch { return; }
  if (msg.method === 'initialize') {
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0', id: msg.id,
      result: { protocolVersion: '2024-11-05', capabilities: { tools: {} },
                serverInfo: { name: 'test-server', version: '0.1.0' } }
    }) + '\n');
  } else if (msg.method === 'tools/list') {
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0', id: msg.id,
      result: { tools: [ { name: 'echo', description: 'Echo' },
                         { name: 'sum', description: 'Sum' } ] }
    }) + '\n');
  }
});
"#;

        let path = std::env::temp_dir().join("ianox-mcp-test-server.js");
        std::fs::write(&path, script).expect("could not write the test server");

        let args = vec![path.to_string_lossy().to_string()];
        let result = probe("node", &args, &HashMap::new()).await;

        assert!(result.ok, "probe failed: {:?}", result.error);
        assert_eq!(result.server_name.as_deref(), Some("test-server"));
        assert_eq!(result.server_version.as_deref(), Some("0.1.0"));
        assert_eq!(result.tools.len(), 2);
        assert_eq!(result.tools[0].name, "echo");
    }
}
