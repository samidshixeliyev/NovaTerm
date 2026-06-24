//! A monospace glyph atlas: rasterizes printable ASCII from the bundled Nerd
//! Font into a single R8 coverage texture and exposes per-glyph UVs. Implements
//! [`nova_gpu::GlyphSource`] so the renderer can place each glyph as a cell-sized
//! textured quad.

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};
use nova_gpu::GlyphSource;

const FONT: &[u8] = include_bytes!("../assets/font.ttf");
const FIRST: u32 = 32; // space
const LAST: u32 = 126; // ~
const COLS: u32 = 16;

pub struct GlyphAtlas {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // R8 coverage
    cell_w: u32,
    cell_h: u32,
    uv: Vec<[f32; 4]>, // indexed by codepoint - FIRST
}

impl GlyphAtlas {
    pub fn build(px: f32) -> Self {
        let font = FontRef::try_from_slice(FONT).expect("embedded font is valid");
        let scale = PxScale::from(px);
        let sf = font.as_scaled(scale);

        let cell_w = sf.h_advance(font.glyph_id('M')).ceil().max(1.0) as u32;
        let cell_h = (sf.ascent() - sf.descent() + sf.line_gap()).ceil().max(1.0) as u32;
        let baseline = sf.ascent();

        let count = LAST - FIRST + 1;
        let rows = count.div_ceil(COLS);
        let width = COLS * cell_w;
        let height = rows * cell_h;
        let mut data = vec![0u8; (width * height) as usize];
        let mut uv = vec![[0.0f32; 4]; count as usize];

        for i in 0..count {
            let ch = char::from_u32(FIRST + i).unwrap_or(' ');
            let col = i % COLS;
            let row = i / COLS;
            let ox = col * cell_w;
            let oy = row * cell_h;

            let glyph = font.glyph_id(ch).with_scale_and_position(scale, point(0.0, baseline));
            if let Some(outline) = font.outline_glyph(glyph) {
                let b = outline.px_bounds();
                outline.draw(|gx, gy, c| {
                    let px_x = b.min.x as i32 + gx as i32;
                    let px_y = b.min.y as i32 + gy as i32;
                    if px_x >= 0 && px_y >= 0 && (px_x as u32) < cell_w && (px_y as u32) < cell_h {
                        let idx = ((oy + px_y as u32) * width + (ox + px_x as u32)) as usize;
                        let v = (c * 255.0) as u8;
                        if v > data[idx] {
                            data[idx] = v;
                        }
                    }
                });
            }

            uv[i as usize] = [
                ox as f32 / width as f32,
                oy as f32 / height as f32,
                (ox + cell_w) as f32 / width as f32,
                (oy + cell_h) as f32 / height as f32,
            ];
        }

        GlyphAtlas { width, height, data, cell_w, cell_h, uv }
    }
}

impl GlyphSource for GlyphAtlas {
    fn glyph_uv(&self, ch: char) -> Option<[f32; 4]> {
        let c = ch as u32;
        if ch == ' ' || c < FIRST || c > LAST {
            return None;
        }
        Some(self.uv[(c - FIRST) as usize])
    }
    fn cell(&self) -> (f32, f32) {
        (self.cell_w as f32, self.cell_h as f32)
    }
}
