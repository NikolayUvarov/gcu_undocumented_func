// The shell's console: a scrollback of text lines drawn with the 8x16 font through mind::tui, mirrored to the serial line (COM1 or the PL011).
use core::fmt::Write;
use mind::dev::Uart;
use mind::gfx::Screen;
use mind::mem::Pages;
use mind::tui::{Style, Terminal};

const BACKGROUND: u32 = 0x001E1E2E;
const FOREGROUND: u32 = 0x00A6E3A1;
const SCROLLBACK: usize = 400; // lines kept for Shift+PgUp
const MAX_COLS: usize = 256;

/// A position in the console: an absolute line number (it keeps counting as old lines drop out) and a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position { pub line: u64, pub col: usize }

pub struct Console {
    term: Option<Terminal>, cols: usize, rows: usize,
    text: Option<Pages>, // SCROLLBACK lines of `cols` characters (u32), a ring
    first: u64, total: u64, // absolute numbers: oldest kept line, lines so far (the last one is being written)
    cx: usize, back: usize, dirty: bool,
    utf8: u32, need: u8, // UTF-8 being decoded for the screen
    pub serial: Option<Uart>,
}

impl Console {
    pub fn new(screen: Option<Screen>, serial: Option<Uart>) -> Self {
        let term = screen.and_then(Terminal::new);
        let (cols, rows) = term.as_ref().map_or((80, 25), |t| (t.cols().min(MAX_COLS), t.rows()));
        let text = Pages::new(SCROLLBACK * cols * 4);
        let mut console = Self { term, cols, rows, text, first: 0, total: 1, cx: 0, back: 0, dirty: true, utf8: 0, need: 0, serial };
        console.clear_line(0);
        console
    }

    pub fn serial(&self, byte: u8) { if let Some(uart) = &self.serial { uart.write(byte); } }
    pub fn serial_str(&self, text: &str) { for byte in text.bytes() { if byte == b'\n' { self.serial(b'\r'); } self.serial(byte); } }

    fn row(&mut self, line: u64) -> &mut [u32] {
        let cols = self.cols;
        let start = (line as usize % SCROLLBACK) * cols;
        match self.text.as_mut() {
            Some(pages) => unsafe { core::slice::from_raw_parts_mut((pages.as_mut_slice().as_mut_ptr() as *mut u32).add(start), cols) },
            None => &mut [],
        }
    }
    fn clear_line(&mut self, line: u64) { for ch in self.row(line).iter_mut() { *ch = ' ' as u32; } }
    fn last(&self) -> u64 { self.total - 1 }

    pub fn position(&self) -> Position { Position { line: self.last(), col: self.cx } }
    /// Characters per line (80 without a screen).
    pub fn cols(&self) -> usize { self.cols }

    fn newline(&mut self) {
        self.total += 1;
        if self.total - self.first > SCROLLBACK as u64 { self.first += 1; }
        let last = self.last();
        self.clear_line(last);
        self.cx = 0; self.dirty = true;
    }
    /// A character on the screen only (no UART).
    pub fn put(&mut self, ch: char) {
        match ch {
            '\n' => self.newline(),
            '\r' => {}
            '\x08' => { if self.cx > 0 { self.cx -= 1; let (last, cx) = (self.last(), self.cx); self.row(last)[cx] = ' ' as u32; self.dirty = true; } }
            _ => {
                if self.cx >= self.cols { self.newline(); }
                let (last, cx) = (self.last(), self.cx);
                self.row(last)[cx] = if ch.is_control() { 0xFFFD } else { ch as u32 };
                self.cx += 1; self.dirty = true;
            }
        }
    }
    pub fn put_str(&mut self, text: &str) { for ch in text.chars() { self.put(ch); } }

    /// A byte of output: to the serial line as is (LF as CRLF), to the screen decoded as UTF-8.
    pub fn print_char(&mut self, byte: u8) {
        if byte == b'\n' { self.serial(b'\r'); }
        self.serial(byte);
        self.back = 0;
        match byte {
            0x80..=0xBF if self.need > 0 => {
                self.utf8 = self.utf8 << 6 | (byte & 0x3F) as u32; self.need -= 1;
                if self.need == 0 { self.put(char::from_u32(self.utf8).unwrap_or('\u{FFFD}')); }
            }
            0xC2..=0xDF => { self.utf8 = (byte & 0x1F) as u32; self.need = 1; }
            0xE0..=0xEF => { self.utf8 = (byte & 0x0F) as u32; self.need = 2; }
            0xF0..=0xF4 => { self.utf8 = (byte & 0x07) as u32; self.need = 3; }
            0x80..=0xFF => { self.need = 0; self.put('\u{FFFD}'); }
            _ => { self.need = 0; self.put(byte as char); }
        }
    }

    /// Drops everything after `at` on the screen (the input line is redrawn from there).
    pub fn truncate(&mut self, at: Position) {
        if at.line < self.first || at.line > self.last() { return; }
        while self.total - 1 > at.line { let last = self.last(); self.clear_line(last); self.total -= 1; }
        let cols = self.cols;
        let row = self.row(at.line);
        for ch in row[at.col.min(cols)..].iter_mut() { *ch = ' ' as u32; }
        self.cx = at.col; self.dirty = true;
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
        self.cx = 0; self.back = 0; self.dirty = true;
    }

    /// Shift+PgUp / Shift+PgDn: moves the view by a page.
    pub fn scroll(&mut self, up: bool) {
        let kept = (self.total - self.first) as usize;
        let page = self.rows.saturating_sub(1).max(1);
        let limit = kept.saturating_sub(self.rows);
        self.back = if up { (self.back + page).min(limit) } else { self.back.saturating_sub(page) };
        self.dirty = true;
    }
    pub fn unscroll(&mut self) { if self.back != 0 { self.back = 0; self.dirty = true; } }

    /// Draws the visible lines; `cursor` is where the input cursor is.
    pub fn render(&mut self, cursor: Option<Position>) {
        if !self.dirty { return; }
        self.dirty = false;
        let (rows, cols) = (self.rows, self.cols);
        let bottom = self.last().saturating_sub(self.back as u64);
        let top = (bottom + 1).saturating_sub(rows as u64).max(self.first);
        let style = Style::new(FOREGROUND, BACKGROUND);
        let Some(mut term) = self.term.take() else { return };
        {
            let mut grid = term.grid();
            grid.clear(style);
            for (y, line) in (top..=bottom).enumerate() {
                let row = self.row(line);
                for x in 0..cols.min(grid.cols) { grid.put(x, y, char::from_u32(row[x]).unwrap_or(' '), style); }
            }
            if self.back != 0 {
                let mut note = mind::util::FixedBuf::<48>::new();
                let _ = write!(note, " ↑ {} ", self.back);
                grid.text_right(cols, 0, core::str::from_utf8(note.as_bytes()).unwrap_or(""), style.inverse());
            }
        }
        let shown = cursor.filter(|_| self.back == 0).and_then(|c| (c.line >= top && c.line <= bottom).then(|| ((c.col).min(cols - 1), (c.line - top) as usize)));
        term.set_cursor(shown);
        term.present();
        self.term = Some(term);
    }
}

impl Write for Console {
    fn write_str(&mut self, text: &str) -> core::fmt::Result { for byte in text.bytes() { self.print_char(byte); } Ok(()) }
}
