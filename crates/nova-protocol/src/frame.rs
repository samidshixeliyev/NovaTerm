//! Frame diffs: the minimal description of what changed since the last frame.
//!
//! The core never ships a full screen unless asked (`full`). Instead it sends
//! the set of changed rows plus cursor/scroll deltas. The renderer applies a
//! diff to its own cell buffer — the *same* code path used to replay recorded
//! sessions, guaranteeing live and playback are pixel-identical.

use crate::cell::{Cell, CursorState};
use crate::ids::SessionId;
use serde::{Deserialize, Serialize};

/// A run of cells starting at `col` within a single row. Diffs ship runs rather
/// than whole rows so a one-character change is a few bytes, not a full line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowRun {
    pub row: u16,
    pub col: u16,
    pub cells: Vec<Cell>,
}

/// A region that scrolled, letting the renderer blit instead of repaint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrollRegion {
    pub top: u16,
    pub bottom: u16,
    /// Positive = content moved up (new lines at bottom).
    pub delta: i16,
}

/// One frame's worth of changes for a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameDiff {
    pub session: SessionId,
    /// Monotonic frame counter; lets the UI detect dropped/out-of-order frames.
    pub seq: u64,
    /// Grid dimensions at the time of this frame.
    pub cols: u16,
    pub rows: u16,
    /// If set, the renderer should discard its buffer and treat `runs` as the
    /// full screen (used after resize, theme change, or initial attach).
    pub full: bool,
    /// Optional scroll hint applied *before* `runs`.
    pub scroll: Option<ScrollRegion>,
    /// Changed cell runs.
    pub runs: Vec<RowRun>,
    pub cursor: CursorState,
    /// New scrollback line count (so the UI can size the scrollbar).
    pub scrollback_len: u32,
}

impl FrameDiff {
    /// An empty diff carrying only an updated cursor (e.g. blink or move).
    #[must_use]
    pub fn cursor_only(
        session: SessionId,
        seq: u64,
        cols: u16,
        rows: u16,
        cursor: CursorState,
        scrollback_len: u32,
    ) -> Self {
        FrameDiff {
            session,
            seq,
            cols,
            rows,
            full: false,
            scroll: None,
            runs: Vec::new(),
            cursor,
            scrollback_len,
        }
    }

    /// True if nothing visual changed except possibly the cursor.
    #[must_use]
    pub fn is_cursor_only(&self) -> bool {
        self.runs.is_empty() && self.scroll.is_none() && !self.full
    }
}
