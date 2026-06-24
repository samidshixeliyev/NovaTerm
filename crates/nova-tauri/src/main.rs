//! NovaTerm desktop shell (Tauri v2).
//!
//! Owns the [`Core`] engine, exposes a small allow-listed command surface to the
//! webview, and forwards every [`nova_protocol::CoreEvent`] (frames, title/cwd,
//! bell, exit) to the front-end over a single `core-event` Tauri event.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use nova_config::{builtin_themes, Config, Theme};
use nova_core::Core;
use nova_protocol::{InputEvent, ResizeEvent, SessionId, SpawnParams};
use tauri::{Emitter, Manager, State};

#[tauri::command]
fn spawn_session(
    state: State<Core>,
    profile_id: Option<String>,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<SessionId, String> {
    state
        .spawn(SpawnParams {
            profile_id,
            cwd,
            cols,
            rows,
            startup_cmds: Vec::new(),
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn send_input(state: State<Core>, session: SessionId, event: InputEvent) -> Result<(), String> {
    state.input(session, event).map_err(|e| e.to_string())
}

#[tauri::command]
fn resize_session(
    state: State<Core>,
    session: SessionId,
    cols: u16,
    rows: u16,
    pixel_width: u16,
    pixel_height: u16,
) -> Result<(), String> {
    state
        .resize(
            session,
            ResizeEvent {
                cols,
                rows,
                pixel_width,
                pixel_height,
            },
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn close_session(state: State<Core>, session: SessionId) -> Result<(), String> {
    state.close(session).map_err(|e| e.to_string())
}

#[tauri::command]
fn request_full_frame(state: State<Core>, session: SessionId) -> Result<(), String> {
    state.request_full_frame(session).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_themes() -> Vec<Theme> {
    builtin_themes()
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nova=info,warn".into()),
        )
        .init();

    // TODO(P3): load `config.json` from %APPDATA% and watch for changes.
    let config = Config::default();
    let (core, _rx) = Core::new(config);

    tauri::Builder::default()
        .manage(core.clone())
        .setup(move |app| {
            // Forward core events to the webview as a single stream.
            let handle = app.handle().clone();
            let mut events = core.subscribe();
            tauri::async_runtime::spawn(async move {
                loop {
                    match events.recv().await {
                        Ok(ev) => {
                            let _ = handle.emit("core-event", ev);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });

            // Windows 11 Mica backdrop for the translucent chrome.
            #[cfg(windows)]
            if let Some(win) = app.get_webview_window("main") {
                let _ = window_vibrancy::apply_mica(&win, None);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            spawn_session,
            send_input,
            resize_session,
            close_session,
            request_full_frame,
            list_themes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NovaTerm");
}
