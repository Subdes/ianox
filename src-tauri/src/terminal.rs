//! Real PTY terminal (bash/zsh) for the workspace.
//!
//! Each terminal is a child process with its own PTY. Output is emitted to
//! frontend por eventos `term://output/{id}`.

use std::io::{Read, Write};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tauri::{AppHandle, Emitter, State};

use crate::config::AppState;

pub struct TermSession {
    pub writer: Box<dyn Write + Send>,
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
}

#[cfg(not(windows))]
fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
}

#[cfg(windows)]
fn default_shell() -> String {
    std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
}

#[tauri::command]
pub async fn term_start(
    app: AppHandle,
    state: State<'_, AppState>,
    cwd: Option<String>,
    rows: Option<u16>,
    cols: Option<u16>,
) -> Result<String, String> {
    let pty_system = native_pty_system();
    let size = PtySize {
        rows: rows.unwrap_or(24).max(4),
        cols: cols.unwrap_or(80).max(10),
        pixel_width: 0,
        pixel_height: 0,
    };

    let pair = pty_system
        .openpty(size)
        .map_err(|e| format!("Could not open the PTY: {e}"))?;

    let mut cmd = CommandBuilder::new(default_shell());
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    if let Some(dir) = cwd.as_ref() {
        if std::path::Path::new(dir).is_dir() {
            cmd.cwd(dir);
        }
    }

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Could not launch the shell: {e}"))?;
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Could not read from the PTY: {e}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Could not write to the PTY: {e}"))?;

    let id = uuid::Uuid::new_v4().to_string();

    // Reader thread: forwards output to the webview
    let app_out = app.clone();
    let out_event = format!("term://output/{id}");
    let exit_event = format!("term://exit/{id}");
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app_out.emit(&out_event, chunk);
                }
                Err(_) => break,
            }
        }
        let _ = app_out.emit(&exit_event, ());
    });

    state.terminals.lock().await.insert(
        id.clone(),
        TermSession {
            writer,
            master: pair.master,
            child,
        },
    );

    Ok(id)
}

#[tauri::command]
pub async fn term_write(
    state: State<'_, AppState>,
    id: String,
    data: String,
) -> Result<(), String> {
    let mut terms = state.terminals.lock().await;
    let session = terms.get_mut(&id).ok_or("Terminal no encontrada")?;
    session
        .writer
        .write_all(data.as_bytes())
        .map_err(|e| format!("Could not write to the terminal: {e}"))?;
    let _ = session.writer.flush();
    Ok(())
}

#[tauri::command]
pub async fn term_resize(
    state: State<'_, AppState>,
    id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let terms = state.terminals.lock().await;
    let session = terms.get(&id).ok_or("Terminal no encontrada")?;
    session
        .master
        .resize(PtySize {
            rows: rows.max(4),
            cols: cols.max(10),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Could not resize: {e}"))
}

#[tauri::command]
pub async fn term_kill(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut terms = state.terminals.lock().await;
    if let Some(mut session) = terms.remove(&id) {
        let _ = session.child.kill();
        let _ = session.child.wait();
    }
    Ok(())
}
