//! Map a structured [`Value`] (+ a [`View`] hint) to a widget [`Node`] subtree.
//! The same value renders as a table, cards, tree, or timeline depending on the
//! hint and its shape — this is the UI side of "structured output, many views".

use crate::geometry::{Color, Edges};
use crate::node::{Dim, Dir, Node};
use nova_value::{Table, Value, View};

const FG: Color = Color::rgb(0xc0, 0xca, 0xf5);
const HEADER: Color = Color::rgb(0x7a, 0xa2, 0xf7);
const ROW_H: f32 = 18.0;

/// Build a widget subtree for a value, honoring an explicit view hint and
/// otherwise choosing by shape.
#[must_use]
pub fn value_to_node(value: &Value, view: View) -> Node {
    match (view, value) {
        (View::Table, _) | (View::Auto, Value::Table(_)) => table_node(&as_table(value)),
        (View::Auto, Value::List(items)) if items.iter().all(is_rowish) && !items.is_empty() => {
            table_node(&Table::from_values(items))
        }
        (View::Cards, _) | (View::Auto, Value::Record(_)) => cards_node(value),
        (_, Value::List(items)) => lines_node(items),
        _ => Node::text(value.to_text(), FG),
    }
}

fn is_rowish(v: &Value) -> bool {
    matches!(v, Value::Record(_) | Value::Custom(_))
}

fn as_table(v: &Value) -> Table {
    match v {
        Value::Table(t) => t.clone(),
        Value::List(items) => Table::from_values(items),
        other => Table::from_values(std::slice::from_ref(other)),
    }
}

/// A header row + one row per record; columns share width via `Grow`.
fn table_node(table: &Table) -> Node {
    let cell = |text: String, color: Color| -> Node {
        Node::panel()
            .width(Dim::Grow(1.0))
            .height(Dim::Fixed(ROW_H))
            .child(Node::text(text, color))
    };
    let row = |cells: Vec<Node>| -> Node {
        Node::panel()
            .dir(Dir::Row)
            .gap(12.0)
            .height(Dim::Fixed(ROW_H))
            .children(cells)
    };

    let mut root = Node::panel()
        .dir(Dir::Col)
        .gap(2.0)
        .padding(Edges::all(6.0));
    root = root.child(row(table
        .columns
        .iter()
        .map(|c| cell(c.clone(), HEADER))
        .collect()));
    for r in &table.rows {
        let cells = table
            .columns
            .iter()
            .map(|c| cell(r.get(c).map(|v| v.to_text()).unwrap_or_default(), FG))
            .collect();
        root = root.child(row(cells));
    }
    root
}

/// A record rendered as `key: value` lines (a single card).
fn cards_node(value: &Value) -> Node {
    let mut root = Node::panel()
        .dir(Dir::Col)
        .gap(2.0)
        .padding(Edges::all(6.0));
    if let Value::Record(r) = value {
        for (k, v) in r.iter() {
            root = root
                .child(Node::text(format!("{k}: {}", v.to_text()), FG).height(Dim::Fixed(ROW_H)));
        }
    } else {
        root = root.child(Node::text(value.to_text(), FG));
    }
    root
}

fn lines_node(items: &[Value]) -> Node {
    let mut root = Node::panel().dir(Dir::Col).gap(1.0);
    for v in items {
        root = root.child(Node::text(v.to_text(), FG).height(Dim::Fixed(ROW_H)));
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::paint;
    use crate::geometry::Rect;
    use crate::layout::compute_layout;
    use nova_value::{FileEntry, FileKind};

    fn files() -> Value {
        Value::List(vec![
            Value::custom(FileEntry {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                kind: FileKind::File,
                size: 10,
                modified: 0,
                readonly: false,
            }),
            Value::custom(FileEntry {
                name: "b.txt".into(),
                path: "/b.txt".into(),
                kind: FileKind::Dir,
                size: 0,
                modified: 0,
                readonly: false,
            }),
        ])
    }

    #[test]
    fn list_of_files_becomes_table_with_header_and_rows() {
        let node = value_to_node(&files(), View::Auto);
        // header + 2 rows
        assert_eq!(node.children.len(), 3);
        // FileEntry exposes 6 fields → 6 columns/cells per row
        assert_eq!(node.children[0].children.len(), 6);
        assert_eq!(node.children[1].children.len(), 6);
    }

    #[test]
    fn table_lays_out_and_paints() {
        let mut node = value_to_node(&files(), View::Table);
        compute_layout(&mut node, Rect::new(0.0, 0.0, 600.0, 200.0));
        let dl = paint(&node);
        // Every non-empty cell yields a glyph run; expect the header names present.
        let has_name_header = dl
            .cmds
            .iter()
            .any(|c| matches!(c, crate::draw::DrawCmd::Glyphs { text, .. } if text == "name"));
        let has_size_value = dl
            .cmds
            .iter()
            .any(|c| matches!(c, crate::draw::DrawCmd::Glyphs { text, .. } if text == "10 B"));
        assert!(has_name_header);
        assert!(has_size_value);
    }

    #[test]
    fn record_becomes_cards() {
        let mut r = nova_value::Record::new();
        r.push("host", Value::from("prod"));
        r.push("status", Value::from("ok"));
        let node = value_to_node(&Value::Record(r), View::Auto);
        assert_eq!(node.children.len(), 2);
    }
}
