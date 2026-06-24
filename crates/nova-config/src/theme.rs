//! Theme model and the five built-in themes.

use serde::{Deserialize, Serialize};

/// UI chrome colors (hex strings, e.g. `#1a1b26`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeColors {
    pub bg: String,
    pub fg: String,
    pub accent: String,
    pub border: String,
    pub tab_active: String,
    pub tab_inactive: String,
}

/// The 16 ANSI colors used by the terminal palette.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnsiColors {
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    pub bright_magenta: String,
    pub bright_cyan: String,
    pub bright_white: String,
}

impl AnsiColors {
    /// Return the 16 colors in ANSI index order (0..16).
    #[must_use]
    pub fn indexed(&self) -> [&str; 16] {
        [
            &self.black,
            &self.red,
            &self.green,
            &self.yellow,
            &self.blue,
            &self.magenta,
            &self.cyan,
            &self.white,
            &self.bright_black,
            &self.bright_red,
            &self.bright_green,
            &self.bright_yellow,
            &self.bright_blue,
            &self.bright_magenta,
            &self.bright_cyan,
            &self.bright_white,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub builtin: bool,
    pub ui: ThemeColors,
    pub ansi: AnsiColors,
    pub cursor: String,
    pub selection: String,
}

/// Parse `#rrggbb` (or `#rgb`) into packed `0xRRGGBBAA`. Returns `None` on a
/// malformed string.
#[must_use]
pub fn parse_hex_rgba(s: &str) -> Option<u32> {
    let s = s.trim().strip_prefix('#')?;
    let (r, g, b) = match s.len() {
        6 => (
            u8::from_str_radix(&s[0..2], 16).ok()?,
            u8::from_str_radix(&s[2..4], 16).ok()?,
            u8::from_str_radix(&s[4..6], 16).ok()?,
        ),
        3 => {
            let r = u8::from_str_radix(&s[0..1], 16).ok()?;
            let g = u8::from_str_radix(&s[1..2], 16).ok()?;
            let b = u8::from_str_radix(&s[2..3], 16).ok()?;
            (r * 17, g * 17, b * 17)
        }
        _ => return None,
    };
    Some((r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | 0xff)
}

macro_rules! ansi {
    ($k:literal,$r:literal,$g:literal,$y:literal,$bl:literal,$m:literal,$c:literal,$w:literal,
     $bk:literal,$br:literal,$bg:literal,$by:literal,$bb:literal,$bm:literal,$bc:literal,$bw:literal) => {
        AnsiColors {
            black: $k.into(),
            red: $r.into(),
            green: $g.into(),
            yellow: $y.into(),
            blue: $bl.into(),
            magenta: $m.into(),
            cyan: $c.into(),
            white: $w.into(),
            bright_black: $bk.into(),
            bright_red: $br.into(),
            bright_green: $bg.into(),
            bright_yellow: $by.into(),
            bright_blue: $bb.into(),
            bright_magenta: $bm.into(),
            bright_cyan: $bc.into(),
            bright_white: $bw.into(),
        }
    };
}

fn theme(
    id: &str,
    name: &str,
    ui: ThemeColors,
    ansi: AnsiColors,
    cursor: &str,
    selection: &str,
) -> Theme {
    Theme {
        id: id.into(),
        name: name.into(),
        builtin: true,
        ui,
        ansi,
        cursor: cursor.into(),
        selection: selection.into(),
    }
}

/// The five built-in themes seeded on first run.
#[must_use]
pub fn builtin_themes() -> Vec<Theme> {
    vec![
        theme(
            "tokyo-night",
            "Tokyo Night",
            ThemeColors {
                bg: "#1a1b26".into(),
                fg: "#c0caf5".into(),
                accent: "#7aa2f7".into(),
                border: "#2a2e42".into(),
                tab_active: "#24283b".into(),
                tab_inactive: "#16161e".into(),
            },
            ansi!(
                "#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff",
                "#a9b1d6", "#414868", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7",
                "#7dcfff", "#c0caf5"
            ),
            "#c0caf5",
            "#283457",
        ),
        theme(
            "nord",
            "Nord",
            ThemeColors {
                bg: "#2e3440".into(),
                fg: "#d8dee9".into(),
                accent: "#88c0d0".into(),
                border: "#3b4252".into(),
                tab_active: "#3b4252".into(),
                tab_inactive: "#2e3440".into(),
            },
            ansi!(
                "#3b4252", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead", "#88c0d0",
                "#e5e9f0", "#4c566a", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead",
                "#8fbcbb", "#eceff4"
            ),
            "#d8dee9",
            "#434c5e",
        ),
        theme(
            "dracula",
            "Dracula",
            ThemeColors {
                bg: "#282a36".into(),
                fg: "#f8f8f2".into(),
                accent: "#bd93f9".into(),
                border: "#44475a".into(),
                tab_active: "#44475a".into(),
                tab_inactive: "#282a36".into(),
            },
            ansi!(
                "#21222c", "#ff5555", "#50fa7b", "#f1fa8c", "#bd93f9", "#ff79c6", "#8be9fd",
                "#f8f8f2", "#6272a4", "#ff6e6e", "#69ff94", "#ffffa5", "#d6acff", "#ff92df",
                "#a4ffff", "#ffffff"
            ),
            "#f8f8f2",
            "#44475a",
        ),
        theme(
            "catppuccin",
            "Catppuccin Mocha",
            ThemeColors {
                bg: "#1e1e2e".into(),
                fg: "#cdd6f4".into(),
                accent: "#89b4fa".into(),
                border: "#313244".into(),
                tab_active: "#313244".into(),
                tab_inactive: "#1e1e2e".into(),
            },
            ansi!(
                "#45475a", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5",
                "#bac2de", "#585b70", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7",
                "#94e2d5", "#a6adc8"
            ),
            "#f5e0dc",
            "#313244",
        ),
        theme(
            "fluent",
            "Fluent",
            ThemeColors {
                bg: "#202020".into(),
                fg: "#e6e6e6".into(),
                accent: "#60cdff".into(),
                border: "#2d2d2d".into(),
                tab_active: "#2d2d2d".into(),
                tab_inactive: "#202020".into(),
            },
            ansi!(
                "#0c0c0c", "#e74856", "#16c60c", "#f9f1a5", "#3b78ff", "#b4009e", "#61d6d6",
                "#cccccc", "#767676", "#e74856", "#16c60c", "#f9f1a5", "#3b78ff", "#b4009e",
                "#61d6d6", "#f2f2f2"
            ),
            "#e6e6e6",
            "#264f78",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing() {
        assert_eq!(parse_hex_rgba("#ff0000"), Some(0xff0000ff));
        assert_eq!(parse_hex_rgba("#0f0"), Some(0x00ff00ff));
        assert_eq!(parse_hex_rgba("nope"), None);
    }

    #[test]
    fn all_builtin_themes_parse_to_colors() {
        for t in builtin_themes() {
            assert!(parse_hex_rgba(&t.ui.bg).is_some(), "{} bg", t.id);
            for c in t.ansi.indexed() {
                assert!(parse_hex_rgba(c).is_some(), "{} ansi {c}", t.id);
            }
        }
    }
}
