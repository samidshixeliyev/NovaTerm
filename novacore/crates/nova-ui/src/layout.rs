//! The layout engine: a single-pass flexbox-style solver. Given a root node and
//! an area, it assigns every node a computed `rect`.

use crate::geometry::Rect;
use crate::node::{Dim, Dir, Node, Style};

/// Lay out `root` within `area` (logical pixels).
pub fn compute_layout(root: &mut Node, area: Rect) {
    root.rect = area;
    layout_children(root);
}

fn main_dim(style: &Style, dir: Dir) -> Dim {
    match dir {
        Dir::Row => style.width,
        Dir::Col => style.height,
    }
}
fn cross_dim(style: &Style, dir: Dir) -> Dim {
    match dir {
        Dir::Row => style.height,
        Dir::Col => style.width,
    }
}

fn layout_children(node: &mut Node) {
    let n = node.children.len();
    if n == 0 {
        return;
    }
    let dir = node.style.direction;
    let gap = node.style.gap;
    let inner = node.rect.inset(node.style.padding);
    let gap_total = gap * (n - 1) as f32;
    let (main_avail, cross_avail) = match dir {
        Dir::Row => (inner.w, inner.h),
        Dir::Col => (inner.h, inner.w),
    };

    // First pass: fixed basis + total grow factor.
    let mut sum_basis = 0.0;
    let mut grow_total = 0.0;
    for c in &node.children {
        match main_dim(&c.style, dir) {
            Dim::Fixed(v) => sum_basis += v,
            Dim::Grow(g) => grow_total += g,
            Dim::Auto => {}
        }
    }
    let free = (main_avail - sum_basis - gap_total).max(0.0);

    // Second pass: position + size each child along the main axis.
    let mut pos = match dir {
        Dir::Row => inner.x,
        Dir::Col => inner.y,
    };
    for c in &mut node.children {
        let main_extent = match main_dim(&c.style, dir) {
            Dim::Fixed(v) => v,
            Dim::Grow(g) if grow_total > 0.0 => free * g / grow_total,
            _ => 0.0,
        };
        let cross_extent = match cross_dim(&c.style, dir) {
            Dim::Fixed(v) => v,
            _ => cross_avail, // Auto/Grow on the cross axis = stretch
        };
        c.rect = match dir {
            Dir::Row => Rect::new(pos, inner.y, main_extent, cross_extent),
            Dir::Col => Rect::new(inner.x, pos, cross_extent, main_extent),
        };
        pos += main_extent + gap;
        layout_children(c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Edges;

    #[test]
    fn row_splits_grow_children_evenly() {
        let mut root = Node::panel()
            .dir(Dir::Row)
            .child(Node::panel().width(Dim::Grow(1.0)))
            .child(Node::panel().width(Dim::Grow(1.0)));
        compute_layout(&mut root, Rect::new(0.0, 0.0, 100.0, 20.0));
        assert_eq!(root.children[0].rect, Rect::new(0.0, 0.0, 50.0, 20.0));
        assert_eq!(root.children[1].rect, Rect::new(50.0, 0.0, 50.0, 20.0));
    }

    #[test]
    fn gap_is_subtracted_before_distribution() {
        let mut root = Node::panel()
            .dir(Dir::Row)
            .gap(10.0)
            .child(Node::panel().width(Dim::Grow(1.0)))
            .child(Node::panel().width(Dim::Grow(1.0)));
        compute_layout(&mut root, Rect::new(0.0, 0.0, 100.0, 20.0));
        assert_eq!(root.children[0].rect, Rect::new(0.0, 0.0, 45.0, 20.0));
        assert_eq!(root.children[1].rect, Rect::new(55.0, 0.0, 45.0, 20.0));
    }

    #[test]
    fn fixed_and_grow_mix() {
        let mut root = Node::panel()
            .dir(Dir::Row)
            .child(Node::panel().width(Dim::Fixed(30.0)))
            .child(Node::panel().width(Dim::Grow(1.0)));
        compute_layout(&mut root, Rect::new(0.0, 0.0, 100.0, 10.0));
        assert_eq!(root.children[0].rect.w, 30.0);
        assert_eq!(root.children[1].rect.w, 70.0);
        assert_eq!(root.children[1].rect.x, 30.0);
    }

    #[test]
    fn padding_shrinks_inner_area() {
        let mut root = Node::panel().padding(Edges::all(5.0)).child(Node::panel());
        compute_layout(&mut root, Rect::new(0.0, 0.0, 100.0, 100.0));
        assert_eq!(root.children[0].rect, Rect::new(5.0, 5.0, 90.0, 90.0));
    }
}
