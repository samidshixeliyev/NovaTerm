//! Flatten a laid-out node tree into a `DrawList` — the flat command stream the
//! GPU backend (`nova-gpu`) turns into instanced rect/glyph draws.

use crate::geometry::{Color, Rect};
use crate::node::{Kind, Node};

#[derive(Debug, Clone, PartialEq)]
pub enum DrawCmd {
    Rect {
        rect: Rect,
        color: Color,
    },
    /// A run of text laid into `rect`. The GPU backend shapes it into glyph
    /// quads from the atlas.
    Glyphs {
        rect: Rect,
        text: String,
        color: Color,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrawList {
    pub cmds: Vec<DrawCmd>,
}

impl DrawList {
    #[must_use]
    pub fn len(&self) -> usize {
        self.cmds.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }
}

/// Produce the draw list for a laid-out tree (call `compute_layout` first).
#[must_use]
pub fn paint(root: &Node) -> DrawList {
    let mut list = DrawList::default();
    paint_into(root, &mut list);
    list
}

fn paint_into(node: &Node, list: &mut DrawList) {
    if node.style.bg != Color::TRANSPARENT {
        list.cmds.push(DrawCmd::Rect {
            rect: node.rect,
            color: node.style.bg,
        });
    }
    if let Kind::Text { content, color } = &node.kind {
        if !content.is_empty() {
            list.cmds.push(DrawCmd::Glyphs {
                rect: node.rect,
                text: content.clone(),
                color: *color,
            });
        }
    }
    for child in &node.children {
        paint_into(child, list);
    }
}
