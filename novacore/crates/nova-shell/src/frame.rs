//! The render pipeline: a structured [`Value`] → widget tree → layout → draw
//! list → GPU instances. This is exactly what the windowed frontend uploads
//! each frame; here it is exercised headlessly so the whole path is testable.

use nova_gpu::{build_instances, GlyphSource, Instance};
use nova_ui::{compute_layout, paint, value_to_node, DrawList, Rect};
use nova_value::{Value, View};

/// A placeholder monospace atlas: maps printable ASCII onto a 16×8 grid. The
/// windowed frontend replaces this with a real rasterized glyph atlas; the
/// instance math is identical.
pub struct MonoAtlas {
    pub cell_w: f32,
    pub line_h: f32,
}

impl Default for MonoAtlas {
    fn default() -> Self {
        MonoAtlas {
            cell_w: 8.0,
            line_h: 16.0,
        }
    }
}

impl GlyphSource for MonoAtlas {
    fn glyph_uv(&self, ch: char) -> Option<[f32; 4]> {
        if ch == ' ' || (ch as u32) < 0x21 {
            return None;
        }
        let i = ch as u32 % 128;
        let col = (i % 16) as f32 / 16.0;
        let row = (i / 16) as f32 / 8.0;
        Some([col, row, col + 1.0 / 16.0, row + 1.0 / 8.0])
    }
    fn cell(&self) -> (f32, f32) {
        (self.cell_w, self.line_h)
    }
}

/// Lay out and rasterize a value into a draw list + GPU instances for an area.
#[must_use]
pub fn render_value(
    value: &Value,
    view: View,
    width: f32,
    height: f32,
) -> (DrawList, Vec<Instance>) {
    let mut node = value_to_node(value, view);
    compute_layout(&mut node, Rect::new(0.0, 0.0, width, height));
    let draw_list = paint(&node);
    let instances = build_instances(&draw_list, &MonoAtlas::default());
    (draw_list, instances)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nova_engine::Engine;

    #[test]
    fn record_renders_to_instances() {
        let mut eng = Engine::with_cwd(std::env::temp_dir());
        let v = eng.eval("{host: \"prod\", status: \"ok\"}").unwrap();
        let (dl, instances) = render_value(&v, View::Auto, 400.0, 200.0);
        assert!(!dl.is_empty());
        // glyphs for "host: prod" / "status: ok" produce many glyph instances
        assert!(instances.iter().filter(|i| i.kind == 1).count() > 5);
    }

    #[test]
    fn list_renders_as_table_instances() {
        let mut eng = Engine::with_cwd(std::env::temp_dir());
        let v = eng.eval("[1 2 3]").unwrap();
        let (_dl, instances) = render_value(&v, View::Auto, 400.0, 200.0);
        assert!(!instances.is_empty());
    }
}
