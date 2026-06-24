//! Packed RGBA color, compact on the wire (a single `u32`).

use serde::{Deserialize, Serialize};

/// 8-bit-per-channel RGBA, packed as `0xRRGGBBAA`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Color(pub u32);

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(0xff, 0xff, 0xff);
    /// Sentinel meaning "use the theme default fg/bg". Fully transparent.
    pub const DEFAULT: Color = Color(0);

    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color((r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | a as u32)
    }

    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color::rgba(r, g, b, 0xff)
    }

    #[must_use]
    pub const fn r(self) -> u8 {
        (self.0 >> 24) as u8
    }
    #[must_use]
    pub const fn g(self) -> u8 {
        (self.0 >> 16) as u8
    }
    #[must_use]
    pub const fn b(self) -> u8 {
        (self.0 >> 8) as u8
    }
    #[must_use]
    pub const fn a(self) -> u8 {
        self.0 as u8
    }

    /// True if this is the "use theme default" sentinel.
    #[must_use]
    pub const fn is_default(self) -> bool {
        self.0 == 0
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::DEFAULT
    }
}
