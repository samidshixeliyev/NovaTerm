//! The retained widget tree: a `Node` with layout `Style`, a `Kind`, and
//! children. The layout engine fills in each node's computed `rect`.

use crate::geometry::{Color, Edges, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Row,
    Col,
}

/// A length along an axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dim {
    /// Sized to the cross-axis of its parent (stretch) on the cross axis, or 0
    /// basis on the main axis.
    Auto,
    /// A fixed logical-pixel length.
    Fixed(f32),
    /// Flexible: takes a share of free main-axis space proportional to the factor.
    Grow(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub direction: Dir,
    pub width: Dim,
    pub height: Dim,
    pub padding: Edges,
    pub gap: f32,
    pub bg: Color,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            direction: Dir::Col,
            width: Dim::Grow(1.0),
            height: Dim::Grow(1.0),
            padding: Edges::default(),
            gap: 0.0,
            bg: Color::TRANSPARENT,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// A container/background panel.
    Panel,
    /// A run of text.
    Text { content: String, color: Color },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub style: Style,
    pub kind: Kind,
    pub children: Vec<Node>,
    /// Computed by the layout engine.
    pub rect: Rect,
}

impl Node {
    #[must_use]
    pub fn panel() -> Node {
        Node {
            style: Style::default(),
            kind: Kind::Panel,
            children: Vec::new(),
            rect: Rect::default(),
        }
    }

    #[must_use]
    pub fn text(content: impl Into<String>, color: Color) -> Node {
        Node {
            style: Style {
                width: Dim::Auto,
                height: Dim::Fixed(18.0),
                ..Style::default()
            },
            kind: Kind::Text {
                content: content.into(),
                color,
            },
            children: Vec::new(),
            rect: Rect::default(),
        }
    }

    #[must_use]
    pub fn dir(mut self, d: Dir) -> Self {
        self.style.direction = d;
        self
    }
    #[must_use]
    pub fn width(mut self, w: Dim) -> Self {
        self.style.width = w;
        self
    }
    #[must_use]
    pub fn height(mut self, h: Dim) -> Self {
        self.style.height = h;
        self
    }
    #[must_use]
    pub fn padding(mut self, e: Edges) -> Self {
        self.style.padding = e;
        self
    }
    #[must_use]
    pub fn gap(mut self, g: f32) -> Self {
        self.style.gap = g;
        self
    }
    #[must_use]
    pub fn bg(mut self, c: Color) -> Self {
        self.style.bg = c;
        self
    }
    #[must_use]
    pub fn child(mut self, c: Node) -> Self {
        self.children.push(c);
        self
    }
    #[must_use]
    pub fn children(mut self, cs: impl IntoIterator<Item = Node>) -> Self {
        self.children.extend(cs);
        self
    }
}
