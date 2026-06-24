//! The screen grid: visible rows, cursor, scrollback, and dirty tracking.

use std::collections::VecDeque;

use nova_protocol::{Cell, CellAttrs, Color, CursorShape, CursorState};

/// The current drawing "pen" — colors and attributes applied to printed cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pen {
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttrs,
}

impl Default for Pen {
    fn default() -> Self {
        Pen {
            fg: Color::DEFAULT,
            bg: Color::DEFAULT,
            attrs: CellAttrs::empty(),
        }
    }
}

impl Pen {
    fn cell(&self, ch: char) -> Cell {
        Cell {
            ch,
            fg: self.fg,
            bg: self.bg,
            attrs: self.attrs,
        }
    }
}

/// The terminal screen state.
pub struct Grid {
    pub cols: u16,
    pub rows: u16,
    /// Visible rows, `rows` entries each `cols` wide.
    lines: Vec<Vec<Cell>>,
    /// Off-screen history (oldest at front), capped at `scrollback_limit`.
    scrollback: VecDeque<Vec<Cell>>,
    scrollback_limit: usize,

    pub cursor_row: u16,
    pub cursor_col: u16,
    pub pen: Pen,
    pub cursor_visible: bool,
    pub cursor_shape: CursorShape,
    saved_cursor: Option<(u16, u16, Pen)>,

    /// Scroll region (top, bottom) inclusive; defaults to full screen.
    scroll_top: u16,
    scroll_bottom: u16,

    /// Per-row dirty flags + a "repaint everything" flag.
    dirty: Vec<bool>,
    all_dirty: bool,
    /// Deferred-wrap (pending) flag, like xterm's last-column behavior.
    wrap_pending: bool,
}

impl Grid {
    #[must_use]
    pub fn new(cols: u16, rows: u16, scrollback_limit: usize) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        Grid {
            cols,
            rows,
            lines: vec![vec![Cell::EMPTY; cols as usize]; rows as usize],
            scrollback: VecDeque::new(),
            scrollback_limit,
            cursor_row: 0,
            cursor_col: 0,
            pen: Pen::default(),
            cursor_visible: true,
            cursor_shape: CursorShape::Block,
            saved_cursor: None,
            scroll_top: 0,
            scroll_bottom: rows - 1,
            dirty: vec![true; rows as usize],
            all_dirty: true,
            wrap_pending: false,
        }
    }

    #[must_use]
    pub fn scrollback_len(&self) -> u32 {
        self.scrollback.len() as u32
    }

    #[must_use]
    pub fn cursor_state(&self) -> CursorState {
        CursorState {
            row: self.cursor_row,
            col: self.cursor_col,
            shape: self.cursor_shape,
            visible: self.cursor_visible,
            blink: true,
        }
    }

    pub fn line(&self, row: u16) -> &[Cell] {
        &self.lines[row as usize]
    }

    pub fn take_dirty(&mut self) -> (bool, Vec<u16>) {
        if self.all_dirty {
            self.all_dirty = false;
            for d in &mut self.dirty {
                *d = false;
            }
            return (true, (0..self.rows).collect());
        }
        let mut rows = Vec::new();
        for (i, d) in self.dirty.iter_mut().enumerate() {
            if *d {
                rows.push(i as u16);
                *d = false;
            }
        }
        (false, rows)
    }

    fn mark(&mut self, row: u16) {
        if let Some(d) = self.dirty.get_mut(row as usize) {
            *d = true;
        }
    }

    fn mark_all(&mut self) {
        self.all_dirty = true;
    }

    // --- Printing --------------------------------------------------------

    /// Print a character of the given display width (1 or 2) at the cursor.
    pub fn print(&mut self, ch: char, width: usize) {
        if width == 0 {
            return; // combining marks: ignored in this MVP model
        }
        if self.wrap_pending {
            self.carriage_return();
            self.line_feed();
            self.wrap_pending = false;
        }
        // Wrap if a wide glyph won't fit at the last column.
        if self.cursor_col as usize + width > self.cols as usize {
            self.carriage_return();
            self.line_feed();
        }

        let row = self.cursor_row;
        let col = self.cursor_col as usize;
        let mut cell = self.pen.cell(ch);
        if width == 2 {
            cell.attrs.insert(CellAttrs::WIDE);
            self.lines[row as usize][col] = cell;
            if col + 1 < self.cols as usize {
                let mut spacer = self.pen.cell(' ');
                spacer.attrs.insert(CellAttrs::WIDE_SPACER);
                self.lines[row as usize][col + 1] = spacer;
            }
        } else {
            self.lines[row as usize][col] = cell;
        }
        self.mark(row);

        let new_col = self.cursor_col + width as u16;
        if new_col >= self.cols {
            // Stay in the last column and set the deferred-wrap flag.
            self.cursor_col = self.cols - 1;
            self.wrap_pending = true;
        } else {
            self.cursor_col = new_col;
        }
    }

    // --- Cursor movement -------------------------------------------------

    pub fn carriage_return(&mut self) {
        self.cursor_col = 0;
        self.wrap_pending = false;
    }

    pub fn line_feed(&mut self) {
        self.wrap_pending = false;
        if self.cursor_row == self.scroll_bottom {
            self.scroll_up(1);
        } else if self.cursor_row + 1 < self.rows {
            self.cursor_row += 1;
        }
    }

    pub fn backspace(&mut self) {
        self.wrap_pending = false;
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        }
    }

    pub fn tab(&mut self) {
        let next = ((self.cursor_col / 8) + 1) * 8;
        self.cursor_col = next.min(self.cols - 1);
    }

    pub fn move_to(&mut self, row: u16, col: u16) {
        self.cursor_row = row.min(self.rows - 1);
        self.cursor_col = col.min(self.cols - 1);
        self.wrap_pending = false;
    }

    pub fn move_up(&mut self, n: u16) {
        self.cursor_row = self.cursor_row.saturating_sub(n.max(1));
        self.wrap_pending = false;
    }
    pub fn move_down(&mut self, n: u16) {
        self.cursor_row = (self.cursor_row + n.max(1)).min(self.rows - 1);
        self.wrap_pending = false;
    }
    pub fn move_right(&mut self, n: u16) {
        self.cursor_col = (self.cursor_col + n.max(1)).min(self.cols - 1);
        self.wrap_pending = false;
    }
    pub fn move_left(&mut self, n: u16) {
        self.cursor_col = self.cursor_col.saturating_sub(n.max(1));
        self.wrap_pending = false;
    }
    pub fn set_col(&mut self, col: u16) {
        self.cursor_col = col.min(self.cols - 1);
        self.wrap_pending = false;
    }
    pub fn set_row(&mut self, row: u16) {
        self.cursor_row = row.min(self.rows - 1);
        self.wrap_pending = false;
    }

    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some((self.cursor_row, self.cursor_col, self.pen));
    }
    pub fn restore_cursor(&mut self) {
        if let Some((r, c, pen)) = self.saved_cursor {
            self.cursor_row = r;
            self.cursor_col = c;
            self.pen = pen;
        }
    }

    pub fn set_scroll_region(&mut self, top: u16, bottom: u16) {
        let top = top.min(self.rows - 1);
        let bottom = bottom.min(self.rows - 1);
        if top < bottom {
            self.scroll_top = top;
            self.scroll_bottom = bottom;
            self.move_to(0, 0);
        }
    }

    // --- Scrolling -------------------------------------------------------

    /// Scroll the scroll-region up by `n` lines, pushing the top line(s) into
    /// scrollback when the region spans the whole screen.
    pub fn scroll_up(&mut self, n: u16) {
        let n = n.max(1) as usize;
        let top = self.scroll_top as usize;
        let bottom = self.scroll_bottom as usize;
        let full_screen = self.scroll_top == 0 && self.scroll_bottom == self.rows - 1;

        for _ in 0..n {
            let line =
                std::mem::replace(&mut self.lines[top], vec![Cell::EMPTY; self.cols as usize]);
            if full_screen {
                self.push_scrollback(line);
            }
            // Shift region up by one.
            for r in top..bottom {
                self.lines.swap(r, r + 1);
            }
            self.lines[bottom] = vec![Cell::EMPTY; self.cols as usize];
        }
        self.mark_all();
    }

    pub fn scroll_down(&mut self, n: u16) {
        let n = n.max(1) as usize;
        let top = self.scroll_top as usize;
        let bottom = self.scroll_bottom as usize;
        for _ in 0..n {
            for r in (top..bottom).rev() {
                self.lines.swap(r, r + 1);
            }
            self.lines[top] = vec![Cell::EMPTY; self.cols as usize];
        }
        self.mark_all();
    }

    /// Insert `n` blank lines at the cursor row, scrolling the region below
    /// down (IL).
    pub fn insert_lines(&mut self, n: u16) {
        let top = self.cursor_row as usize;
        let bottom = self.scroll_bottom as usize;
        if top > bottom {
            return;
        }
        let n = (n.max(1) as usize).min(bottom - top + 1);
        for _ in 0..n {
            for r in (top..bottom).rev() {
                self.lines.swap(r, r + 1);
            }
            self.lines[top] = vec![Cell::EMPTY; self.cols as usize];
        }
        self.mark_all();
    }

    /// Delete `n` lines at the cursor row, scrolling the region below up (DL).
    pub fn delete_lines(&mut self, n: u16) {
        let top = self.cursor_row as usize;
        let bottom = self.scroll_bottom as usize;
        if top > bottom {
            return;
        }
        let n = (n.max(1) as usize).min(bottom - top + 1);
        for _ in 0..n {
            for r in top..bottom {
                self.lines.swap(r, r + 1);
            }
            self.lines[bottom] = vec![Cell::EMPTY; self.cols as usize];
        }
        self.mark_all();
    }

    fn push_scrollback(&mut self, line: Vec<Cell>) {
        if self.scrollback_limit == 0 {
            return;
        }
        self.scrollback.push_back(line);
        while self.scrollback.len() > self.scrollback_limit {
            self.scrollback.pop_front();
        }
    }

    // --- Erasing ---------------------------------------------------------

    /// Erase in line. mode: 0 = cursor→end, 1 = start→cursor, 2 = whole line.
    pub fn erase_in_line(&mut self, mode: u16) {
        let row = self.cursor_row as usize;
        let col = self.cursor_col as usize;
        let blank = self.pen.cell(' ');
        let line = &mut self.lines[row];
        match mode {
            0 => {
                for c in line.iter_mut().skip(col) {
                    *c = blank;
                }
            }
            1 => {
                for c in line.iter_mut().take(col + 1) {
                    *c = blank;
                }
            }
            2 => {
                for c in line.iter_mut() {
                    *c = blank;
                }
            }
            _ => {}
        }
        self.mark(self.cursor_row);
    }

    /// Erase in display. mode: 0 = cursor→end, 1 = start→cursor, 2 = all,
    /// 3 = all + scrollback.
    pub fn erase_in_display(&mut self, mode: u16) {
        let blank = self.pen.cell(' ');
        match mode {
            0 => {
                self.erase_in_line(0);
                for r in (self.cursor_row as usize + 1)..self.rows as usize {
                    for c in self.lines[r].iter_mut() {
                        *c = blank;
                    }
                }
            }
            1 => {
                self.erase_in_line(1);
                for r in 0..self.cursor_row as usize {
                    for c in self.lines[r].iter_mut() {
                        *c = blank;
                    }
                }
            }
            2 => {
                for line in &mut self.lines {
                    for c in line.iter_mut() {
                        *c = blank;
                    }
                }
            }
            3 => {
                self.scrollback.clear();
                for line in &mut self.lines {
                    for c in line.iter_mut() {
                        *c = blank;
                    }
                }
            }
            _ => {}
        }
        self.mark_all();
    }

    /// Insert/delete blank characters at the cursor (ICH/DCH).
    pub fn insert_chars(&mut self, n: u16) {
        let row = self.cursor_row as usize;
        let col = self.cursor_col as usize;
        let blank = self.pen.cell(' ');
        let n = (n.max(1) as usize).min(self.cols as usize - col);
        let line = &mut self.lines[row];
        for _ in 0..n {
            line.insert(col, blank);
            line.pop();
        }
        self.mark(self.cursor_row);
    }

    pub fn delete_chars(&mut self, n: u16) {
        let row = self.cursor_row as usize;
        let col = self.cursor_col as usize;
        let blank = self.pen.cell(' ');
        let n = (n.max(1) as usize).min(self.cols as usize - col);
        let line = &mut self.lines[row];
        for _ in 0..n {
            line.remove(col);
            line.push(blank);
        }
        self.mark(self.cursor_row);
    }

    // --- Resize ----------------------------------------------------------

    /// Resize the grid. This MVP implementation truncates/extends rather than
    /// reflowing wrapped lines (reflow is a P1 item).
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        for line in &mut self.lines {
            line.resize(cols as usize, Cell::EMPTY);
        }
        if rows as usize > self.lines.len() {
            let extra = rows as usize - self.lines.len();
            for _ in 0..extra {
                self.lines.push(vec![Cell::EMPTY; cols as usize]);
            }
        } else {
            self.lines.truncate(rows as usize);
        }
        self.cols = cols;
        self.rows = rows;
        self.scroll_top = 0;
        self.scroll_bottom = rows - 1;
        self.cursor_row = self.cursor_row.min(rows - 1);
        self.cursor_col = self.cursor_col.min(cols - 1);
        self.dirty = vec![true; rows as usize];
        self.mark_all();
        self.wrap_pending = false;
    }
}
