//! The terminal cell model: one character cell's glyph, colors, and attributes.

use crate::color::Color;
use serde::{Deserialize, Serialize};

/// Per-cell rendition attributes, packed into a `u16` bitset. A small hand-rolled
/// bitflags newtype keeps this crate dependency-light and `serde`-transparent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CellAttrs(pub u16);

impl CellAttrs {
    pub const BOLD: CellAttrs = CellAttrs(1 << 0);
    pub const ITALIC: CellAttrs = CellAttrs(1 << 1);
    pub const UNDERLINE: CellAttrs = CellAttrs(1 << 2);
    pub const STRIKETHROUGH: CellAttrs = CellAttrs(1 << 3);
    pub const INVERSE: CellAttrs = CellAttrs(1 << 4);
    pub const DIM: CellAttrs = CellAttrs(1 << 5);
    pub const BLINK: CellAttrs = CellAttrs(1 << 6);
    pub const HIDDEN: CellAttrs = CellAttrs(1 << 7);
    /// Leading cell of a double-width (CJK/emoji) glyph.
    pub const WIDE: CellAttrs = CellAttrs(1 << 8);
    /// Trailing spacer cell that belongs to the preceding `WIDE` glyph.
    pub const WIDE_SPACER: CellAttrs = CellAttrs(1 << 9);
    /// Cell participates in an OSC-8 hyperlink.
    pub const HYPERLINK: CellAttrs = CellAttrs(1 << 10);

    #[must_use]
    pub const fn empty() -> Self {
        CellAttrs(0)
    }
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    #[must_use]
    pub const fn contains(self, other: CellAttrs) -> bool {
        (self.0 & other.0) == other.0
    }
    pub fn insert(&mut self, other: CellAttrs) {
        self.0 |= other.0;
    }
    pub fn remove(&mut self, other: CellAttrs) {
        self.0 &= !other.0;
    }
    pub fn set(&mut self, other: CellAttrs, on: bool) {
        if on {
            self.insert(other);
        } else {
            self.remove(other);
        }
    }
}

impl std::ops::BitOr for CellAttrs {
    type Output = CellAttrs;
    fn bitor(self, rhs: CellAttrs) -> CellAttrs {
        CellAttrs(self.0 | rhs.0)
    }
}

/// A single character cell. Kept small and `Copy` for cheap grid storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    /// Unicode scalar value of the cell's character (`' '` when empty).
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttrs,
}

impl Cell {
    pub const EMPTY: Cell = Cell {
        ch: ' ',
        fg: Color::DEFAULT,
        bg: Color::DEFAULT,
        attrs: CellAttrs::empty(),
    };

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ch == ' ' && self.bg.is_default() && self.attrs.is_empty()
    }
}

impl Default for Cell {
    fn default() -> Self {
        Cell::EMPTY
    }
}

/// Cursor rendering shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorShape {
    #[default]
    Block,
    Bar,
    Underline,
}

/// Cursor position and presentation, sent with every frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorState {
    pub row: u16,
    pub col: u16,
    pub shape: CursorShape,
    pub visible: bool,
    pub blink: bool,
}

impl Default for CursorState {
    fn default() -> Self {
        CursorState {
            row: 0,
            col: 0,
            shape: CursorShape::Block,
            visible: true,
            blink: true,
        }
    }
}
