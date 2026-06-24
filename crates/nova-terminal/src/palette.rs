//! The default 256-color xterm palette. The model emits concrete RGB colors;
//! the first 16 entries can be overridden from a theme via [`Palette::set`].

use nova_protocol::Color;

/// A 256-entry color lookup table plus default fg/bg sentinels.
#[derive(Debug, Clone)]
pub struct Palette {
    colors: [Color; 256],
}

impl Palette {
    /// Build the standard xterm 256-color palette.
    #[must_use]
    pub fn xterm() -> Self {
        let mut colors = [Color::BLACK; 256];

        // 0..16 — the standard ANSI colors.
        const BASE: [(u8, u8, u8); 16] = [
            (0x00, 0x00, 0x00), // black
            (0x80, 0x00, 0x00), // red
            (0x00, 0x80, 0x00), // green
            (0x80, 0x80, 0x00), // yellow
            (0x00, 0x00, 0x80), // blue
            (0x80, 0x00, 0x80), // magenta
            (0x00, 0x80, 0x80), // cyan
            (0xc0, 0xc0, 0xc0), // white
            (0x80, 0x80, 0x80), // bright black
            (0xff, 0x00, 0x00), // bright red
            (0x00, 0xff, 0x00), // bright green
            (0xff, 0xff, 0x00), // bright yellow
            (0x00, 0x00, 0xff), // bright blue
            (0xff, 0x00, 0xff), // bright magenta
            (0x00, 0xff, 0xff), // bright cyan
            (0xff, 0xff, 0xff), // bright white
        ];
        for (i, &(r, g, b)) in BASE.iter().enumerate() {
            colors[i] = Color::rgb(r, g, b);
        }

        // 16..232 — 6x6x6 color cube.
        let steps = [0u8, 95, 135, 175, 215, 255];
        let mut idx = 16;
        for r in 0..6 {
            for g in 0..6 {
                for b in 0..6 {
                    colors[idx] = Color::rgb(steps[r], steps[g], steps[b]);
                    idx += 1;
                }
            }
        }

        // 232..256 — 24-step grayscale ramp.
        for i in 0..24 {
            let v = 8 + i as u8 * 10;
            colors[232 + i] = Color::rgb(v, v, v);
        }

        Palette { colors }
    }

    /// Look up an indexed color (0..=255).
    #[must_use]
    pub fn get(&self, index: u8) -> Color {
        self.colors[index as usize]
    }

    /// Override an entry (used to apply theme ANSI colors).
    pub fn set(&mut self, index: u8, color: Color) {
        self.colors[index as usize] = color;
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::xterm()
    }
}
