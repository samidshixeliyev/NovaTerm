//! Golden-ish tests for the terminal model: feed escape streams, assert grid.

use nova_protocol::{CellAttrs, SessionId};
use nova_terminal::{TermEvent, Terminal};

fn row_text(term: &Terminal, row: u16) -> String {
    term.grid()
        .line(row)
        .iter()
        .map(|c| c.ch)
        .collect::<String>()
        .trim_end()
        .to_string()
}

#[test]
fn plain_text_prints() {
    let mut t = Terminal::new(20, 5, 1000);
    t.feed(b"hello");
    assert_eq!(row_text(&t, 0), "hello");
    assert_eq!(t.grid().cursor_col, 5);
}

#[test]
fn newline_and_carriage_return() {
    let mut t = Terminal::new(20, 5, 1000);
    t.feed(b"ab\r\ncd");
    assert_eq!(row_text(&t, 0), "ab");
    assert_eq!(row_text(&t, 1), "cd");
    assert_eq!(t.grid().cursor_row, 1);
}

#[test]
fn cursor_addressing_and_erase() {
    let mut t = Terminal::new(20, 5, 1000);
    t.feed(b"XXXXX");
    t.feed(b"\x1b[1;1H"); // home
    t.feed(b"\x1b[K"); // erase to end of line
    assert_eq!(row_text(&t, 0), "");
}

#[test]
fn sgr_sets_attributes_and_color() {
    let mut t = Terminal::new(20, 5, 1000);
    t.feed(b"\x1b[1;31mA\x1b[0mB");
    let line = t.grid().line(0);
    assert!(line[0].attrs.contains(CellAttrs::BOLD));
    // red (index 1) from the default palette
    assert_eq!(line[0].fg.r(), 0x80);
    // reset
    assert!(line[1].attrs.is_empty());
    assert!(line[1].fg.is_default());
}

#[test]
fn truecolor_sgr() {
    let mut t = Terminal::new(20, 5, 1000);
    t.feed(b"\x1b[38;2;10;20;30mZ");
    let c = t.grid().line(0)[0];
    assert_eq!((c.fg.r(), c.fg.g(), c.fg.b()), (10, 20, 30));
}

#[test]
fn scroll_pushes_to_scrollback() {
    let mut t = Terminal::new(10, 2, 1000);
    t.feed(b"line1\r\nline2\r\nline3");
    // Two visible rows; the first line scrolled into history.
    assert_eq!(t.grid().scrollback_len(), 1);
    assert_eq!(row_text(&t, 0), "line2");
    assert_eq!(row_text(&t, 1), "line3");
}

#[test]
fn wide_glyph_occupies_two_cells() {
    let mut t = Terminal::new(20, 5, 1000);
    t.feed("世".as_bytes());
    let line = t.grid().line(0);
    assert!(line[0].attrs.contains(CellAttrs::WIDE));
    assert!(line[1].attrs.contains(CellAttrs::WIDE_SPACER));
    assert_eq!(t.grid().cursor_col, 2);
}

#[test]
fn osc_title_event() {
    let mut t = Terminal::new(20, 5, 1000);
    t.feed(b"\x1b]0;My Title\x07");
    let events = t.drain_events();
    assert_eq!(events, vec![TermEvent::Title("My Title".to_string())]);
}

#[test]
fn private_modes_tracked() {
    let mut t = Terminal::new(20, 5, 1000);
    t.feed(b"\x1b[?25l"); // hide cursor
    assert!(!t.grid().cursor_state().visible);
    t.feed(b"\x1b[?1h"); // application cursor keys
    assert!(t.app_cursor_keys());
    t.feed(b"\x1b[?2004h"); // bracketed paste
    assert!(t.bracketed_paste());
}

#[test]
fn frame_diff_reports_dirty_rows() {
    let mut t = Terminal::new(20, 5, 1000);
    let sid = SessionId::new();
    let _ = t.take_frame(sid, true); // initial full frame clears dirty
    t.feed(b"\r\n\r\nhi"); // writes on row 2
    let diff = t.take_frame(sid, false);
    assert!(!diff.full);
    assert!(diff.runs.iter().any(|r| r.row == 2));
    // a subsequent frame with no input has no runs
    let diff2 = t.take_frame(sid, false);
    assert!(diff2.runs.is_empty());
}

#[test]
fn never_panics_on_arbitrary_bytes() {
    let mut t = Terminal::new(40, 10, 1000);
    // A torture stream of escapes, control bytes, and UTF-8.
    let mut data = Vec::new();
    for b in 0u8..=255 {
        data.push(b);
    }
    data.extend_from_slice(b"\x1b[1;2;3;4;5;6;7m\x1b[999;999H\x1b]0;\x07\x1b[38;5;200m");
    data.extend_from_slice("héllo 世界 🚀".as_bytes());
    t.feed(&data);
    // Just assert we still have a sane cursor and didn't panic.
    assert!(t.grid().cursor_row < 10);
}
