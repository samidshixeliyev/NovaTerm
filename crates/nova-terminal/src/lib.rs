//! `nova-terminal` — the terminal model.
//!
//! Bytes from the PTY are fed through a [`vte`] parser into a [`Grid`]; the
//! result is a dirty-tracked screen that can emit minimal [`FrameDiff`]s for the
//! renderer and a stream of [`TermEvent`]s (title/cwd/bell) for the UI.
//!
//! The model is deliberately session-agnostic: the orchestrator attaches a
//! [`SessionId`] and sequence number when it calls [`Terminal::take_frame`].

#![forbid(unsafe_code)]

mod grid;
mod palette;

pub use grid::{Grid, Pen};
pub use palette::Palette;

use nova_protocol::{CellAttrs, Color, FrameDiff, RowRun, SessionId};
use unicode_width::UnicodeWidthChar;
use vte::{Params, Parser, Perform};

/// Side-channel events produced while parsing (not part of the visual frame).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermEvent {
    Title(String),
    Cwd(String),
    Bell,
    /// An OSC-8 hyperlink was opened with the given target URI.
    Hyperlink(String),
}

/// A terminal: parser + screen + parsed-out modes/events.
pub struct Terminal {
    parser: Parser,
    inner: Inner,
    seq: u64,
}

impl Terminal {
    #[must_use]
    pub fn new(cols: u16, rows: u16, scrollback_limit: usize) -> Self {
        Terminal {
            parser: Parser::new(),
            inner: Inner {
                grid: Grid::new(cols, rows, scrollback_limit),
                palette: Palette::xterm(),
                events: Vec::new(),
                app_cursor_keys: false,
                bracketed_paste: false,
                changed: true,
            },
            seq: 0,
        }
    }

    /// Override an ANSI palette entry (theme integration).
    pub fn set_palette_color(&mut self, index: u8, color: Color) {
        self.inner.palette.set(index, color);
    }

    /// Feed raw bytes from the PTY.
    pub fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.parser.advance(&mut self.inner, b);
        }
    }

    /// Whether anything changed since the last [`take_frame`].
    #[must_use]
    pub fn has_changes(&self) -> bool {
        self.inner.changed
    }

    /// True if the shell put the keypad/cursor keys into application mode (the
    /// orchestrator needs this to encode arrow keys correctly).
    #[must_use]
    pub fn app_cursor_keys(&self) -> bool {
        self.inner.app_cursor_keys
    }

    #[must_use]
    pub fn bracketed_paste(&self) -> bool {
        self.inner.bracketed_paste
    }

    /// Drain accumulated side-channel events.
    pub fn drain_events(&mut self) -> Vec<TermEvent> {
        std::mem::take(&mut self.inner.events)
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.inner.grid.resize(cols, rows);
        self.inner.changed = true;
    }

    /// Build a frame diff for the dirty rows. Pass `force_full` (e.g. after a
    /// resize or initial attach) to emit the whole screen.
    pub fn take_frame(&mut self, session: SessionId, force_full: bool) -> FrameDiff {
        self.seq += 1;
        let grid = &mut self.inner.grid;
        if force_full {
            // Drain dirty bookkeeping but emit everything.
            let _ = grid.take_dirty();
        }
        let (full, dirty_rows) = if force_full {
            (true, (0..grid.rows).collect::<Vec<_>>())
        } else {
            grid.take_dirty()
        };

        let runs = dirty_rows
            .iter()
            .map(|&row| RowRun {
                row,
                col: 0,
                cells: grid.line(row).to_vec(),
            })
            .collect();

        self.inner.changed = false;

        FrameDiff {
            session,
            seq: self.seq,
            cols: grid.cols,
            rows: grid.rows,
            full,
            scroll: None,
            runs,
            cursor: grid.cursor_state(),
            scrollback_len: grid.scrollback_len(),
        }
    }

    /// Read-only access to the grid (tests, snapshots).
    #[must_use]
    pub fn grid(&self) -> &Grid {
        &self.inner.grid
    }
}

/// Everything the `vte::Perform` implementation needs. Kept separate from the
/// `Parser` so both can be borrowed mutably at once.
struct Inner {
    grid: Grid,
    palette: Palette,
    events: Vec<TermEvent>,
    app_cursor_keys: bool,
    bracketed_paste: bool,
    changed: bool,
}

impl Inner {
    fn touch(&mut self) {
        self.changed = true;
    }
}

/// Fetch CSI parameter `idx` (flattened across sub-params), or `default`.
fn arg(ps: &[u16], idx: usize, default: u16) -> u16 {
    ps.get(idx).copied().filter(|&v| v != 0).unwrap_or(default)
}

impl Perform for Inner {
    fn print(&mut self, c: char) {
        let width = c.width().unwrap_or(0);
        self.grid.print(c, width);
        self.touch();
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x07 => self.events.push(TermEvent::Bell),
            0x08 => self.grid.backspace(),
            0x09 => self.grid.tab(),
            0x0A..=0x0C => self.grid.line_feed(),
            0x0D => self.grid.carriage_return(),
            _ => {}
        }
        self.touch();
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        // Flatten params (handles both `38;5;n` and `38:5:n` forms).
        let mut ps: Vec<u16> = Vec::with_capacity(8);
        for group in params.iter() {
            ps.extend_from_slice(group);
        }
        let private = intermediates.first() == Some(&b'?');

        match action {
            'A' => self.grid.move_up(arg(&ps, 0, 1)),
            'B' => self.grid.move_down(arg(&ps, 0, 1)),
            'C' => self.grid.move_right(arg(&ps, 0, 1)),
            'D' => self.grid.move_left(arg(&ps, 0, 1)),
            'E' => {
                self.grid.move_down(arg(&ps, 0, 1));
                self.grid.set_col(0);
            }
            'F' => {
                self.grid.move_up(arg(&ps, 0, 1));
                self.grid.set_col(0);
            }
            'G' | '`' => self.grid.set_col(arg(&ps, 0, 1) - 1),
            'd' => self.grid.set_row(arg(&ps, 0, 1) - 1),
            'H' | 'f' => {
                let row = arg(&ps, 0, 1) - 1;
                let col = arg(&ps, 1, 1) - 1;
                self.grid.move_to(row, col);
            }
            'J' => self.grid.erase_in_display(arg(&ps, 0, 0)),
            'K' => self.grid.erase_in_line(arg(&ps, 0, 0)),
            'L' => self.grid.insert_lines(arg(&ps, 0, 1)),
            'M' => self.grid.delete_lines(arg(&ps, 0, 1)),
            '@' => self.grid.insert_chars(arg(&ps, 0, 1)),
            'P' => self.grid.delete_chars(arg(&ps, 0, 1)),
            'S' => self.grid.scroll_up(arg(&ps, 0, 1)),
            'T' => self.grid.scroll_down(arg(&ps, 0, 1)),
            'r' => {
                let top = arg(&ps, 0, 1) - 1;
                let bottom = arg(&ps, 1, self.grid.rows) - 1;
                self.grid.set_scroll_region(top, bottom);
            }
            's' => self.grid.save_cursor(),
            'u' => self.grid.restore_cursor(),
            'm' => self.apply_sgr(&ps),
            'h' if private => self.set_private_mode(&ps, true),
            'l' if private => self.set_private_mode(&ps, false),
            _ => {}
        }
        self.touch();
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            b'M' => self.grid.scroll_down(1), // Reverse Index
            b'D' => self.grid.line_feed(),    // Index
            b'7' => self.grid.save_cursor(),
            b'8' => self.grid.restore_cursor(),
            b'c' => {
                // RIS — full reset of the visible screen.
                self.grid.erase_in_display(2);
                self.grid.move_to(0, 0);
            }
            _ => {}
        }
        self.touch();
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        let Some(&cmd) = params.first() else { return };
        match cmd {
            b"0" | b"2" => {
                if let Some(raw) = params.get(1) {
                    let title = sanitize(&String::from_utf8_lossy(raw));
                    self.events.push(TermEvent::Title(title));
                }
            }
            b"7" => {
                if let Some(raw) = params.get(1) {
                    let s = String::from_utf8_lossy(raw);
                    let cwd = s.strip_prefix("file://").map_or_else(
                        || s.to_string(),
                        |rest| {
                            // file://host/C:/path -> C:/path
                            rest.split_once('/').map_or(rest.to_string(), |(_h, p)| {
                                let p = p.trim_start_matches('/');
                                p.to_string()
                            })
                        },
                    );
                    self.events.push(TermEvent::Cwd(cwd));
                }
            }
            b"8" => {
                // OSC 8 ; params ; URI
                if let Some(uri) = params.get(2) {
                    let uri = String::from_utf8_lossy(uri).to_string();
                    if !uri.is_empty() {
                        self.events.push(TermEvent::Hyperlink(uri));
                    }
                }
            }
            _ => {}
        }
    }

    // DCS passthrough (e.g. sixel) is not yet modeled.
    fn hook(&mut self, _params: &Params, _inter: &[u8], _ignore: bool, _action: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
}

impl Inner {
    fn set_private_mode(&mut self, ps: &[u16], on: bool) {
        for &mode in ps {
            match mode {
                1 => self.app_cursor_keys = on,
                25 => self.grid.cursor_visible = on,
                2004 => self.bracketed_paste = on,
                // 1049/47/1047: alternate screen buffer (modeled in P1).
                _ => {}
            }
        }
    }

    fn apply_sgr(&mut self, ps: &[u16]) {
        if ps.is_empty() {
            self.grid.pen = Pen::default();
            return;
        }
        let mut i = 0;
        while i < ps.len() {
            match ps[i] {
                0 => self.grid.pen = Pen::default(),
                1 => self.grid.pen.attrs.insert(CellAttrs::BOLD),
                2 => self.grid.pen.attrs.insert(CellAttrs::DIM),
                3 => self.grid.pen.attrs.insert(CellAttrs::ITALIC),
                4 => self.grid.pen.attrs.insert(CellAttrs::UNDERLINE),
                5 => self.grid.pen.attrs.insert(CellAttrs::BLINK),
                7 => self.grid.pen.attrs.insert(CellAttrs::INVERSE),
                8 => self.grid.pen.attrs.insert(CellAttrs::HIDDEN),
                9 => self.grid.pen.attrs.insert(CellAttrs::STRIKETHROUGH),
                22 => {
                    self.grid.pen.attrs.remove(CellAttrs::BOLD);
                    self.grid.pen.attrs.remove(CellAttrs::DIM);
                }
                23 => self.grid.pen.attrs.remove(CellAttrs::ITALIC),
                24 => self.grid.pen.attrs.remove(CellAttrs::UNDERLINE),
                25 => self.grid.pen.attrs.remove(CellAttrs::BLINK),
                27 => self.grid.pen.attrs.remove(CellAttrs::INVERSE),
                29 => self.grid.pen.attrs.remove(CellAttrs::STRIKETHROUGH),
                30..=37 => self.grid.pen.fg = self.palette.get((ps[i] - 30) as u8),
                39 => self.grid.pen.fg = Color::DEFAULT,
                40..=47 => self.grid.pen.bg = self.palette.get((ps[i] - 40) as u8),
                49 => self.grid.pen.bg = Color::DEFAULT,
                90..=97 => self.grid.pen.fg = self.palette.get((ps[i] - 90 + 8) as u8),
                100..=107 => self.grid.pen.bg = self.palette.get((ps[i] - 100 + 8) as u8),
                38 => {
                    if let Some((color, consumed)) = parse_extended_color(&ps[i..], &self.palette) {
                        self.grid.pen.fg = color;
                        i += consumed - 1;
                    }
                }
                48 => {
                    if let Some((color, consumed)) = parse_extended_color(&ps[i..], &self.palette) {
                        self.grid.pen.bg = color;
                        i += consumed - 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

/// Parse a `38;5;n` (indexed) or `38;2;r;g;b` (truecolor) sequence starting at
/// the `38`/`48` selector. Returns the color and how many params it consumed.
fn parse_extended_color(ps: &[u16], palette: &Palette) -> Option<(Color, usize)> {
    match ps.get(1) {
        Some(5) => {
            let idx = *ps.get(2)? as u8;
            Some((palette.get(idx), 3))
        }
        Some(2) => {
            let r = *ps.get(2)? as u8;
            let g = *ps.get(3)? as u8;
            let b = *ps.get(4)? as u8;
            Some((Color::rgb(r, g, b), 5))
        }
        _ => None,
    }
}

/// Strip control characters from window/tab titles (anti-injection).
fn sanitize(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(512).collect()
}
