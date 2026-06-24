//! Commands the UI invokes on the core (the allow-listed Tauri command surface,
//! mirrored as a typed enum for non-Tauri callers and tests).

use crate::ids::SessionId;
use crate::input::{InputEvent, ResizeEvent};
use serde::{Deserialize, Serialize};

/// Parameters for spawning a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnParams {
    /// Profile id from config (selects shell/args/env). `None` = default profile.
    pub profile_id: Option<String>,
    pub cwd: Option<String>,
    pub cols: u16,
    pub rows: u16,
    /// Commands to run immediately after spawn (workspace startup_cmds).
    #[serde(default)]
    pub startup_cmds: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    Spawn(SpawnParams),
    Input {
        session: SessionId,
        event: InputEvent,
    },
    Resize {
        session: SessionId,
        size: ResizeEvent,
    },
    /// Ask the core to resend the entire screen as a `full` frame.
    RequestFullFrame {
        session: SessionId,
    },
    Close {
        session: SessionId,
    },
    /// Drop live state but keep compact scrollback (tab hibernation).
    Hibernate {
        session: SessionId,
    },
    /// Re-spawn / re-attach a hibernated session.
    Restore {
        session: SessionId,
    },
}
