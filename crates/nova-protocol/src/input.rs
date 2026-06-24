//! Input events flowing UI → core.

use serde::{Deserialize, Serialize};

/// Keyboard modifier state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Windows / Super key.
    pub meta: bool,
}

/// A key press. `key` is a logical key name (e.g. "Enter", "ArrowUp", "a") as
/// produced by the browser `KeyboardEvent.key`; the core maps it to the right
/// escape sequence honoring application/cursor-key modes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvent {
    pub key: String,
    pub mods: KeyModifiers,
    /// Pre-composed text for printable input (IME / dead keys); when present the
    /// core writes it verbatim instead of synthesizing from `key`.
    pub text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseKind {
    Down,
    Up,
    Move,
    Scroll,
}

/// A mouse event in *cell* coordinates (the UI translates pixels → cells).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MouseEvent {
    pub kind: MouseKind,
    pub button: Option<MouseButton>,
    pub col: u16,
    pub row: u16,
    /// Scroll delta in lines (negative = up).
    pub scroll_lines: i16,
    pub mods: KeyModifiers,
}

/// PTY resize in cells (plus pixel size for apps that want it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResizeEvent {
    pub cols: u16,
    pub rows: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}

/// Union of input events the UI can send for a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputEvent {
    Key(KeyEvent),
    /// Text from a paste; written to the PTY (bracketed if the app enabled it).
    Paste {
        text: String,
    },
    Mouse(MouseEvent),
    Resize(ResizeEvent),
    /// Scroll the *viewport* (scrollback), not the PTY. Handled UI-side normally,
    /// but exposed here for keyboard-driven scroll.
    ScrollViewport {
        lines: i32,
    },
}
