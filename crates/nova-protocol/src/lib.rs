//! `nova-protocol` — the typed wire contract shared by every NovaTerm layer.
//!
//! These types are the *only* thing the UI, the core, and the storage layer all
//! agree on. Keeping them in one small crate makes the boundary explicit and
//! versionable. All types are `serde`-serializable so they can cross the Tauri
//! IPC boundary or be persisted.

#![forbid(unsafe_code)]

pub mod cell;
pub mod color;
pub mod command;
pub mod event;
pub mod frame;
pub mod ids;
pub mod input;

pub use cell::{Cell, CellAttrs, CursorShape, CursorState};
pub use color::Color;
pub use command::{Command, SpawnParams};
pub use event::CoreEvent;
pub use frame::{FrameDiff, RowRun, ScrollRegion};
pub use ids::{PaneId, SessionId, TabId};
pub use input::{InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseEvent, ResizeEvent};

/// Wire-protocol version. Bumped on any breaking change to the types in this
/// crate so the UI and core can detect a mismatch at startup.
pub const PROTOCOL_VERSION: u32 = 1;
