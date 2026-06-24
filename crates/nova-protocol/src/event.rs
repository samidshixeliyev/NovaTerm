//! Events flowing core → UI (delivered as Tauri events).

use crate::frame::FrameDiff;
use crate::ids::SessionId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum CoreEvent {
    /// A session was successfully spawned and is ready for input.
    Spawned { session: SessionId, pid: u32 },
    /// New frame for a session (the hot path event).
    Frame(FrameDiff),
    /// The shell program changed the window/tab title (sanitized).
    TitleChanged { session: SessionId, title: String },
    /// Working directory reported via OSC 7 / shell integration.
    CwdChanged { session: SessionId, cwd: String },
    /// Terminal bell.
    Bell { session: SessionId },
    /// The child process exited.
    Exited { session: SessionId, code: i32 },
    /// A non-fatal error scoped to a session (e.g. write failure).
    Error {
        session: Option<SessionId>,
        message: String,
    },
}
