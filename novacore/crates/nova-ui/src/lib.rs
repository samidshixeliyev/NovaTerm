//! `nova-ui` — NovaCore's native UI framework: a flexbox-style **layout
//! engine**, a retained **widget tree**, **value views** (table/cards/tree/
//! timeline), and a flat **draw list**. It has no GPU or windowing dependency —
//! it turns values into positioned draw commands that `nova-gpu` rasterizes.

#![forbid(unsafe_code)]

pub mod draw;
pub mod geometry;
pub mod layout;
pub mod node;
pub mod view;

pub use draw::{paint, DrawCmd, DrawList};
pub use geometry::{Color, Edges, Point, Rect, Size};
pub use layout::compute_layout;
pub use node::{Dim, Dir, Kind, Node, Style};
pub use view::value_to_node;
