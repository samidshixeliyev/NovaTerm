//! Simple geometry primitives (logical pixels, f32).

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    #[must_use]
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }

    /// Shrink by per-side edges.
    #[must_use]
    pub fn inset(&self, e: Edges) -> Rect {
        Rect {
            x: self.x + e.left,
            y: self.y + e.top,
            w: (self.w - e.left - e.right).max(0.0),
            h: (self.h - e.top - e.bottom).max(0.0),
        }
    }

    #[must_use]
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    #[must_use]
    pub fn all(v: f32) -> Self {
        Edges {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }
    #[must_use]
    pub fn xy(x: f32, y: f32) -> Self {
        Edges {
            top: y,
            bottom: y,
            left: x,
            right: x,
        }
    }
}

/// Packed `0xRRGGBBAA` color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color(pub u32);

impl Color {
    pub const TRANSPARENT: Color = Color(0);
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color((r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | 0xff)
    }
}
