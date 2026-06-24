//! Events flowing core → UI (delivered as Tauri events).

use crate::frame::FrameDiff;
use crate::ids::SessionId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum CoreEvent {
    /// A session was successfully spawned and is ready for input.
    Spawned { session: SessionId, pid: u32 },
    /// Raw PTY output for a session, base64-encoded. The hot path when rendering
    /// with an external VT engine (xterm.js): the byte stream is shipped as-is
    /// and the frontend parses/renders it.
    Output { session: SessionId, base64: String },
    /// New frame for a session (used by the built-in grid renderer; unused when
    /// an external VT engine consumes `Output`).
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
