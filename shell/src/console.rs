// The shell's console: a scrollback of text lines drawn with the 8x16 font through mind::tui, mirrored to the serial line (COM1 or the PL011).
// The shell keeps one per virtual console (issue 155): the one shown holds the screen, only the first the serial line.
// The lines themselves are a `ring::Ring`; the console in the shell's window follows the window's size (211-APP-0040).
use crate::ring::{Ring, LINES};
use alloc::vec::Vec;
use core::fmt::Write;
use mind::dev::Uart;
use mind::gfx::Screen;
use mind::mem::Pages;
use mind::tui::{Style, Terminal};

pub use crate::ring::Position;

const BACKGROUND: u32 = 0x001E1E2E;
const FOREGROUND: u32 = 0x00A6E3A1;
const MAX_COLS: usize = 256;

/// A ring's characters in pages of their own (none when there was no memory for them).
pub struct Text(Option<Pages>);

impl Text {
    fn new(stride: usize) -> Self { Self(Pages::new(LINES * stride * 4)) }
}
impl core::ops::Deref for Text {
    type Target = [u32];
    fn deref(&self) -> &[u32] { self.0.as_ref().map_or(&[], |pages| unsafe { core::slice::from_raw_parts(pages.as_slice().as_ptr() as *const u32, pages.as_slice().len() / 4) }) }
}
impl core::ops::DerefMut for Text {
    fn deref_mut(&mut self) -> &mut [u32] { self.0.as_mut().map_or(&mut [], |pages| { let len = pages.as_slice().len() / 4; unsafe { core::slice::from_raw_parts_mut(pages.as_mut_slice().as_mut_ptr() as *mut u32, len) } }) }
}

pub struct Console {
    term: Option<Terminal>,
    text: Ring<Text>,
    dirty: bool,
    utf8: u32, need: u8, // UTF-8 being decoded for the screen
    pub serial: Option<Uart>,
    /// Shown at the top right while more than one console is open: which one this is.
    pub label: &'static str,
    /// While a script captures a command's output (`capture`, issue 094): the bytes it printed, kept off the screen.
    pub capture: Option<Vec<u8>>,
    /// Drawn in a window: its lines and rows follow the window's size, last seen as `seen` (211-APP-0040).
    follows: bool, seen: (usize, usize),
}

impl Console {
    fn with(term: Option<Terminal>, text: Ring<Text>, serial: Option<Uart>, follows: bool) -> Self {
        let seen = (text.cols, text.rows);
        Self { term, text, dirty: true, utf8: 0, need: 0, serial, label: "", capture: None, follows, seen }
    }

    pub fn new(screen: Option<Screen>, serial: Option<Uart>) -> Self {
        let term = screen.and_then(Terminal::new);
        let (cols, rows) = term.as_ref().map_or((80, 25), |t| (t.cols().min(MAX_COLS), t.rows()));
        Self::with(term, Ring::new(Text::new(cols), cols, cols, rows), serial, false)
    }

    /// A console drawn in a window (the shell's window in `wm`, 211-APP-0040): its lines are as wide as the window can
    /// be (`room` cells), and wrap and show at the window's size.
    pub fn in_window(term: Terminal, room: usize) -> Option<Self> {
        let stride = room.clamp(1, MAX_COLS);
        let text = Text::new(stride);
        text.0.as_ref()?;
        let ring = Ring::new(text, stride, term.cols(), term.rows());
        Some(Self::with(Some(term), ring, None, true))
    }

    /// An empty console of the same size, without the screen or the serial line (None: no memory for its lines).
    pub fn sibling(&self) -> Option<Self> {
        let cols = self.text.cols;
        let text = Text::new(cols);
        text.0.as_ref()?;
        Some(Self::with(None, Ring::new(text, cols, cols, self.text.rows), None, false))
    }
    /// The screen goes to the console shown.
    pub fn take_screen(&mut self) -> Option<Terminal> { self.term.take() }
    pub fn give_screen(&mut self, term: Option<Terminal>) { self.term = term; self.dirty = true; }

    pub fn serial(&self, byte: u8) { if let Some(uart) = &self.serial { uart.write(byte); } }
    pub fn serial_str(&self, text: &str) { for byte in text.bytes() { if byte == b'\n' { self.serial(b'\r'); } self.serial(byte); } }

    pub fn position(&self) -> Position { self.text.position() }
    /// Characters per line (80 without a screen).
    pub fn cols(&self) -> usize { self.text.cols }

    /// A character on the screen only (no UART); `\r` goes back to the start of the line, and what comes next
    /// overwrites it (`clock --line`, issue u016).
    pub fn put(&mut self, ch: char) { self.text.put(ch); self.dirty = true; }
    pub fn put_str(&mut self, text: &str) { for ch in text.chars() { self.put(ch); } }

    /// A byte of output: to the serial line as is (LF as CRLF), to the screen decoded as UTF-8.
    pub fn print_char(&mut self, byte: u8) {
        if let Some(captured) = self.capture.as_mut() { if captured.len() < 64 * 1024 { captured.push(byte); } return; }
        if byte == b'\n' { self.serial(b'\r'); }
        self.serial(byte);
        self.text.follow();
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
    pub fn truncate(&mut self, at: Position) { self.text.truncate(at); self.dirty = true; }

    /// Where the character `index` of text written from `start` lands.
    pub fn offset(&self, start: Position, index: usize) -> Position { self.text.offset(start, index) }

    pub fn clear(&mut self) { self.text.clear(); self.dirty = true; }

    /// Shift+PgUp / Shift+PgDn: moves the view by a page.
    pub fn scroll(&mut self, up: bool) { self.text.scroll(up); self.dirty = true; }
    pub fn unscroll(&mut self) { if self.text.unscroll() { self.dirty = true; } }

    /// Draws the visible lines; `cursor` is where the input cursor is.
    pub fn render(&mut self, cursor: Option<Position>) {
        if self.follows {
            let size = self.term.as_mut().map(|term| { let grid = term.grid(); (grid.cols, grid.rows) });
            if let Some((cols, rows)) = size.filter(|&size| size != self.seen) { self.seen = (cols, rows); self.text.resize(cols, rows); self.dirty = true; }
        }
        if !self.dirty { return; }
        self.dirty = false;
        let (cols, back) = (self.text.cols, self.text.back());
        let (top, bottom) = self.text.view();
        let style = Style::new(FOREGROUND, BACKGROUND);
        let Some(mut term) = self.term.take() else { return };
        {
            let mut grid = term.grid();
            grid.clear(style);
            for (y, line) in (top..=bottom).enumerate() {
                let row = self.text.row(line);
                for x in 0..cols.min(grid.cols).min(row.len()) { grid.put(x, y, char::from_u32(row[x]).unwrap_or(' '), style); }
            }
            if back != 0 || !self.label.is_empty() {
                let mut note = mind::util::FixedBuf::<48>::new();
                if back != 0 { let _ = write!(note, " ↑ {} ", back); }
                let _ = write!(note, "{}", self.label);
                grid.text_right(cols, 0, core::str::from_utf8(note.as_bytes()).unwrap_or(""), style.inverse());
            }
        }
        let shown = cursor.filter(|_| back == 0).and_then(|c| (c.line >= top && c.line <= bottom).then(|| ((c.col).min(cols - 1), (c.line - top) as usize)));
        term.set_cursor(shown);
        term.present();
        self.term = Some(term);
    }
}

impl Write for Console {
    fn write_str(&mut self, text: &str) -> core::fmt::Result { for byte in text.bytes() { self.print_char(byte); } Ok(()) }
}
