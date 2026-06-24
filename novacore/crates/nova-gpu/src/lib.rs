//! `nova-gpu` — the GPU renderer core.
//!
//! It turns a [`nova_ui::DrawList`] into a flat array of [`Instance`]s (one per
//! rect, one per glyph) that a wgpu pipeline draws in a single instanced call,
//! and it carries the [`SHADER`] used by that pipeline. The wgpu device/surface
//! wiring lives in the application (`nova-shell`); this crate is the pure,
//! testable transform from UI draw commands to GPU instance data.
//!
//! Pipeline summary (see `ARCHITECTURE.md §7`): glyphs are rasterized into an
//! R8 coverage atlas; each instance references an atlas sub-rect via `uv` and a
//! `kind` discriminating solid rects (`0`) from glyph quads (`1`).

#![forbid(unsafe_code)]

use nova_ui::{Color, DrawCmd, DrawList};

/// One GPU instance — expanded into a quad by the vertex shader. `#[repr(C)]`
/// so it can be uploaded directly to a vertex/instance buffer (with `bytemuck`
/// in the application).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    /// `[x, y, w, h]` in logical pixels.
    pub rect: [f32; 4],
    /// Atlas sub-rect `[u0, v0, u1, v1]` (unused for solid rects).
    pub uv: [f32; 4],
    /// Linear RGBA.
    pub color: [f32; 4],
    /// `0` = solid rect, `1` = glyph (sample atlas coverage as alpha).
    pub kind: u32,
    pub _pad: [u32; 3],
}

/// Maps a character to its atlas sub-rect `[u0,v0,u1,v1]`, or `None` if the glyph
/// is absent (e.g. whitespace). The application's glyph atlas implements this.
pub trait GlyphSource {
    fn glyph_uv(&self, ch: char) -> Option<[f32; 4]>;
    /// Monospace advance width and line height in logical pixels.
    fn cell(&self) -> (f32, f32);
}

fn color_to_rgba(c: Color) -> [f32; 4] {
    let v = c.0;
    [
        ((v >> 24) & 0xff) as f32 / 255.0,
        ((v >> 16) & 0xff) as f32 / 255.0,
        ((v >> 8) & 0xff) as f32 / 255.0,
        (v & 0xff) as f32 / 255.0,
    ]
}

/// Build the instance buffer for a draw list. Solid rects become one instance;
/// each glyph in a text run becomes one instance advanced along the run.
#[must_use]
pub fn build_instances(list: &DrawList, glyphs: &dyn GlyphSource) -> Vec<Instance> {
    let (cell_w, line_h) = glyphs.cell();
    let mut out = Vec::with_capacity(list.cmds.len());
    for cmd in &list.cmds {
        match cmd {
            DrawCmd::Rect { rect, color } => out.push(Instance {
                rect: [rect.x, rect.y, rect.w, rect.h],
                uv: [0.0; 4],
                color: color_to_rgba(*color),
                kind: 0,
                _pad: [0; 3],
            }),
            DrawCmd::Glyphs { rect, text, color } => {
                let mut x = rect.x;
                for ch in text.chars() {
                    if let Some(uv) = glyphs.glyph_uv(ch) {
                        out.push(Instance {
                            rect: [x, rect.y, cell_w, line_h],
                            uv,
                            color: color_to_rgba(*color),
                            kind: 1,
                            _pad: [0; 3],
                        });
                    }
                    x += cell_w;
                }
            }
        }
    }
    out
}

/// The WGSL shader for the instanced rect/glyph pipeline. The vertex stage
/// expands a unit quad per instance into screen space; the fragment stage emits
/// a solid color (`kind==0`) or modulates by atlas coverage (`kind==1`).
pub const SHADER: &str = r#"
struct Globals { screen: vec2<f32>, _pad: vec2<f32> };
@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var atlas_smp: sampler;

struct Instance {
    @location(0) rect: vec4<f32>,
    @location(1) uv:   vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) kind: u32,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) kind: u32,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Instance) -> VsOut {
    // Unit quad (two triangles) corners.
    var corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    let c = corners[vi];
    let px = inst.rect.xy + c * inst.rect.zw;
    // Pixel space -> clip space (y down).
    let ndc = vec2(px.x / globals.screen.x * 2.0 - 1.0,
                   1.0 - px.y / globals.screen.y * 2.0);
    var out: VsOut;
    out.pos = vec4(ndc, 0.0, 1.0);
    out.uv = mix(inst.uv.xy, inst.uv.zw, c);
    out.color = inst.color;
    out.kind = inst.kind;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (in.kind == 1u) {
        let coverage = textureSample(atlas_tex, atlas_smp, in.uv).r;
        return vec4(in.color.rgb, in.color.a * coverage);
    }
    return in.color;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use nova_ui::{Color, Rect};

    struct AsciiGrid;
    impl GlyphSource for AsciiGrid {
        fn glyph_uv(&self, ch: char) -> Option<[f32; 4]> {
            if ch == ' ' {
                return None;
            }
            let i = ch as u32;
            let col = (i % 16) as f32 / 16.0;
            let row = (i / 16) as f32 / 8.0;
            Some([col, row, col + 1.0 / 16.0, row + 1.0 / 8.0])
        }
        fn cell(&self) -> (f32, f32) {
            (8.0, 16.0)
        }
    }

    #[test]
    fn rect_makes_one_solid_instance() {
        let mut dl = DrawList::default();
        dl.cmds.push(DrawCmd::Rect {
            rect: Rect::new(1.0, 2.0, 3.0, 4.0),
            color: Color::rgb(10, 20, 30),
        });
        let inst = build_instances(&dl, &AsciiGrid);
        assert_eq!(inst.len(), 1);
        assert_eq!(inst[0].kind, 0);
        assert_eq!(inst[0].rect, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn glyph_run_advances_and_skips_spaces() {
        let mut dl = DrawList::default();
        dl.cmds.push(DrawCmd::Glyphs {
            rect: Rect::new(0.0, 0.0, 100.0, 16.0),
            text: "a b".into(),
            color: Color::rgb(255, 255, 255),
        });
        let inst = build_instances(&dl, &AsciiGrid);
        // 'a' and 'b' produce glyphs; the space is skipped.
        assert_eq!(inst.len(), 2);
        assert!(inst.iter().all(|i| i.kind == 1));
        assert_eq!(inst[0].rect[0], 0.0); // 'a' at x=0
        assert_eq!(inst[1].rect[0], 16.0); // 'b' at x=2*cell_w
    }

    #[test]
    fn color_conversion() {
        assert_eq!(color_to_rgba(Color::rgb(255, 0, 0)), [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn shader_has_entrypoints() {
        assert!(SHADER.contains("fn vs_main"));
        assert!(SHADER.contains("fn fs_main"));
    }
}
