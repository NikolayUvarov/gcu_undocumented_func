//! The text of a shell console (shell/src/console.rs): a ring of `LINES` lines of characters, each `stride` wide,
//! written and shown `cols` wide on `rows` rows. A console in the shell's window takes the window's size as it changes
//! (211-APP-0040): its lines keep their characters beyond a narrower width, and show them again when it widens.
//! Host-tested in tests/shell_host.rs.
use core::ops::DerefMut;

pub const LINES: usize = 400; // lines kept for Shift+PgUp

/// A position in the console: an absolute line number (it keeps counting as old lines drop out) and a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position { pub line: u64, pub col: usize }

/// `cells`: LINES × `stride` characters (u32), or fewer when there was no memory for them (nothing is kept then).
pub struct Ring<S> {
    cells: S, stride: usize, pub cols: usize, pub rows: usize,
    first: u64, total: u64, // absolute numbers: oldest kept line, lines so far (the last one is being written)
    cx: usize, back: usize, // the column written next; how many lines the view is scrolled back
}

impl<S: DerefMut<Target = [u32]>> Ring<S> {
    pub fn new(cells: S, stride: usize, cols: usize, rows: usize) -> Self {
        let mut ring = Self { cells, stride, cols: cols.clamp(1, stride.max(1)), rows: rows.max(1), first: 0, total: 1, cx: 0, back: 0 };
        ring.clear_line(0);
        ring
    }

    /// Line `line`'s characters, `stride` of them.
    pub fn row(&mut self, line: u64) -> &mut [u32] {
        let start = (line as usize % LINES) * self.stride;
        self.cells.get_mut(start..start + self.stride).unwrap_or(&mut [])
    }
    fn clear_line(&mut self, line: u64) { for ch in self.row(line).iter_mut() { *ch = ' ' as u32; } }
    fn last(&self) -> u64 { self.total - 1 }
    pub fn position(&self) -> Position { Position { line: self.last(), col: self.cx } }

    fn newline(&mut self) {
        self.total += 1;
        if self.total - self.first > LINES as u64 { self.first += 1; }
        let last = self.last();
        self.clear_line(last);
        self.cx = 0;
    }

    /// A character: a new line, the start of the line again (`\r`), a step back, or one written (a full line wraps).
    pub fn put(&mut self, ch: char) {
        match ch {
            '\n' => self.newline(),
            '\r' => self.cx = 0,
            '\x08' => if self.cx > 0 { self.cx -= 1; let (last, cx) = (self.last(), self.cx); if let Some(c) = self.row(last).get_mut(cx) { *c = ' ' as u32; } },
            _ => {
                if self.cx >= self.cols { self.newline(); }
                let (last, cx) = (self.last(), self.cx);
                if let Some(c) = self.row(last).get_mut(cx) { *c = if ch.is_control() { 0xFFFD } else { ch as u32 }; }
                self.cx += 1;
            }
        }
    }

    /// Drops everything after `at` (the input line is redrawn from there).
    pub fn truncate(&mut self, at: Position) {
        if at.line < self.first || at.line > self.last() { return; }
        while self.total - 1 > at.line { let last = self.last(); self.clear_line(last); self.total -= 1; }
        let row = self.row(at.line);
        let from = at.col.min(row.len());
        for ch in row[from..].iter_mut() { *ch = ' ' as u32; }
        self.cx = at.col;
    }

    /// Where the character `index` of text written from `start` lands.
    pub fn offset(&self, start: Position, index: usize) -> Position {
        let col = start.col + index;
        // A full line wraps only when the next character is written, so the cursor may sit on column `cols`.
        let (extra, col) = if col > 0 && col % self.cols == 0 && col >= self.cols { (col / self.cols - 1, self.cols) } else { (col / self.cols, col % self.cols) };
        Position { line: start.line + extra as u64, col }
    }

    pub fn clear(&mut self) {
        self.first = self.total - 1;
        let last = self.last();
        self.clear_line(last);
        self.cx = 0; self.back = 0;
    }

    /// Shift+PgUp / Shift+PgDn: moves the view by a page.
    pub fn scroll(&mut self, up: bool) {
        let kept = (self.total - self.first) as usize;
        let page = self.rows.saturating_sub(1).max(1);
        let limit = kept.saturating_sub(self.rows);
        self.back = if up { (self.back + page).min(limit) } else { self.back.saturating_sub(page) };
    }
    /// Back to the newest lines; true if the view moved.
    pub fn unscroll(&mut self) -> bool { core::mem::replace(&mut self.back, 0) != 0 }
    /// What was written goes to the newest lines.
    pub fn follow(&mut self) { self.back = 0; }
    pub fn back(&self) -> usize { self.back }

    /// The lines on view, top and bottom (absolute numbers).
    pub fn view(&self) -> (u64, u64) {
        let bottom = self.last().saturating_sub(self.back as u64);
        let top = (bottom + 1).saturating_sub(self.rows as u64).max(self.first);
        (top, bottom)
    }

    /// A new size: lines wrap at `cols` (at most `stride`) from now on, `rows` show; the view goes to the newest lines.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols.clamp(1, self.stride.max(1));
        self.rows = rows.max(1);
        self.back = 0;
    }
}
