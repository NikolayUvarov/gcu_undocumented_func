//! Text UI: a grid of character cells drawn with the 8x16 font (MIND Mono 16), frames, bars, graphs and widgets
//! (`widgets`). `Grid` is plain memory and builds on the host for tests; `Terminal` puts it on the program's screen
//! and redraws only cells that changed.
pub mod digits;
pub mod syntax;
pub mod viewer;
pub mod widgets;
#[cfg(target_os = "none")]
mod term;
#[cfg(target_os = "none")]
pub use term::Terminal;

/// Foreground and background colour (0x00RRGGBB).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Style { pub fg: u32, pub bg: u32 }

impl Style {
    pub const fn new(fg: u32, bg: u32) -> Self { Self { fg, bg } }
    /// The same colours swapped (selection, cursor).
    pub const fn inverse(self) -> Self { Self { fg: self.bg, bg: self.fg } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell { pub ch: char, pub style: Style }

impl Cell {
    pub const BLANK: Cell = Cell { ch: ' ', style: Style { fg: 0, bg: 0 } };
}

/// A rectangle of cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect { pub x: usize, pub y: usize, pub w: usize, pub h: usize }

impl Rect {
    pub const fn new(x: usize, y: usize, w: usize, h: usize) -> Self { Self { x, y, w, h } }
    /// The inside of a frame drawn on this rectangle.
    pub fn inner(self) -> Self { Self { x: self.x + 1, y: self.y + 1, w: self.w.saturating_sub(2), h: self.h.saturating_sub(2) } }
    /// A rectangle of `w` x `h` centred in this one.
    pub fn centered(self, w: usize, h: usize) -> Self { let (w, h) = (w.min(self.w), h.min(self.h)); Self { x: self.x + (self.w - w) / 2, y: self.y + (self.h - h) / 2, w, h } }
    pub fn right(self) -> usize { self.x + self.w }
    pub fn bottom(self) -> usize { self.y + self.h }
    pub fn contains(self, x: usize, y: usize) -> bool { x >= self.x && x < self.right() && y >= self.y && y < self.bottom() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line { Single, Double }

/// Colours of the standard elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub panel: Style, pub frame: Style, pub header: Style, pub selected: Style, pub marked: Style, pub directory: Style,
    pub menu: Style, pub menu_selected: Style, pub dialog: Style, pub dialog_frame: Style, pub input: Style,
    pub fkey_number: Style, pub fkey_label: Style, pub status: Style, pub accent: Style, pub dim: Style, pub error: Style,
}

/// Norton Commander colours.
pub const CLASSIC: Theme = Theme {
    panel: Style::new(0x55FFFF, 0x0000AA), frame: Style::new(0x55FFFF, 0x0000AA), header: Style::new(0xFFFF55, 0x0000AA),
    selected: Style::new(0x000000, 0x00AAAA), marked: Style::new(0xFFFF55, 0x0000AA), directory: Style::new(0xFFFFFF, 0x0000AA),
    menu: Style::new(0x000000, 0x00AAAA), menu_selected: Style::new(0xFFFFFF, 0x000000), dialog: Style::new(0x000000, 0xAAAAAA),
    dialog_frame: Style::new(0xFFFFFF, 0xAAAAAA), input: Style::new(0x000000, 0x00AAAA), fkey_number: Style::new(0xFFFFFF, 0x000000),
    fkey_label: Style::new(0x000000, 0x00AAAA), status: Style::new(0x000000, 0x00AAAA), accent: Style::new(0xFFFF55, 0x0000AA),
    dim: Style::new(0x5555FF, 0x0000AA), error: Style::new(0xFFFFFF, 0xAA0000),
};

/// Dark colours (monitors, viewer).
pub const DARK: Theme = Theme {
    panel: Style::new(0xD0D0D0, 0x101820), frame: Style::new(0x5080A0, 0x101820), header: Style::new(0x80D0FF, 0x101820),
    selected: Style::new(0x101820, 0x80D0FF), marked: Style::new(0xFFD060, 0x101820), directory: Style::new(0xFFFFFF, 0x101820),
    menu: Style::new(0x101820, 0x80A0C0), menu_selected: Style::new(0xFFFFFF, 0x305070), dialog: Style::new(0xE0E0E0, 0x283848),
    dialog_frame: Style::new(0x80D0FF, 0x283848), input: Style::new(0xFFFFFF, 0x405060), fkey_number: Style::new(0xE0E0E0, 0x000000),
    fkey_label: Style::new(0x101820, 0x80A0C0), status: Style::new(0x101820, 0x80A0C0), accent: Style::new(0xA6E3A1, 0x101820),
    dim: Style::new(0x708090, 0x101820), error: Style::new(0xFFFFFF, 0xA03030),
};

// Box-drawing characters: [horizontal, vertical, top-left, top-right, bottom-left, bottom-right].
const SINGLE: [char; 6] = ['─', '│', '┌', '┐', '└', '┘'];
const DOUBLE: [char; 6] = ['═', '║', '╔', '╗', '╚', '╝'];
// Left-aligned eighths of a cell for bars.
const EIGHTHS: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];

/// A grid of cells over borrowed memory.
pub struct Grid<'a> { cells: &'a mut [Cell], pub cols: usize, pub rows: usize }

impl<'a> Grid<'a> {
    pub fn new(cells: &'a mut [Cell], cols: usize, rows: usize) -> Self {
        assert!(cells.len() >= cols * rows);
        Self { cells, cols, rows }
    }
    pub fn area(&self) -> Rect { Rect::new(0, 0, self.cols, self.rows) }
    pub fn get(&self, x: usize, y: usize) -> Cell { if x < self.cols && y < self.rows { self.cells[y * self.cols + x] } else { Cell::BLANK } }
    pub fn put(&mut self, x: usize, y: usize, ch: char, style: Style) {
        if x < self.cols && y < self.rows { self.cells[y * self.cols + x] = Cell { ch, style }; }
    }
    pub fn clear(&mut self, style: Style) { self.fill(self.area(), ' ', style); }
    pub fn fill(&mut self, area: Rect, ch: char, style: Style) {
        for y in area.y..area.bottom().min(self.rows) { for x in area.x..area.right().min(self.cols) { self.put(x, y, ch, style); } }
    }
    /// Changes the colours of cells without touching their characters.
    pub fn restyle(&mut self, area: Rect, style: Style) {
        for y in area.y..area.bottom().min(self.rows) { for x in area.x..area.right().min(self.cols) { self.cells[y * self.cols + x].style = style; } }
    }
    /// Text from (x, y), cut at `max` cells and at the right edge; control characters show as U+FFFD. Returns the
    /// number of cells written.
    pub fn text_max(&mut self, x: usize, y: usize, text: &str, max: usize, style: Style) -> usize {
        let mut count = 0;
        for ch in text.chars() {
            if count >= max || x + count >= self.cols { break; }
            self.put(x + count, y, if ch.is_control() { '\u{FFFD}' } else { ch }, style);
            count += 1;
        }
        count
    }
    pub fn text(&mut self, x: usize, y: usize, text: &str, style: Style) -> usize { self.text_max(x, y, text, usize::MAX, style) }
    /// Text padded with spaces (or cut) to exactly `width` cells.
    pub fn text_padded(&mut self, x: usize, y: usize, text: &str, width: usize, style: Style) {
        let used = self.text_max(x, y, text, width, style);
        for i in used..width { self.put(x + i, y, ' ', style); }
    }
    /// Text right-aligned so that it ends before column `right`.
    pub fn text_right(&mut self, right: usize, y: usize, text: &str, style: Style) {
        let len = text.chars().count();
        self.text(right.saturating_sub(len), y, text, style);
    }
    /// Text centred in `area` on row `y`.
    pub fn text_centered(&mut self, area: Rect, y: usize, text: &str, style: Style) {
        let len = text.chars().count().min(area.w);
        self.text_max(area.x + (area.w - len) / 2, y, text, len, style);
    }
    pub fn hline(&mut self, x: usize, y: usize, w: usize, line: Line, style: Style) {
        let ch = if line == Line::Single { SINGLE[0] } else { DOUBLE[0] };
        for i in 0..w { self.put(x + i, y, ch, style); }
    }
    pub fn vline(&mut self, x: usize, y: usize, h: usize, line: Line, style: Style) {
        let ch = if line == Line::Single { SINGLE[1] } else { DOUBLE[1] };
        for i in 0..h { self.put(x, y + i, ch, style); }
    }
    /// A frame on the edge of `area`; the inside is filled with spaces of `style`.
    pub fn frame(&mut self, area: Rect, line: Line, style: Style) {
        if area.w < 2 || area.h < 2 { return; }
        let c = if line == Line::Single { SINGLE } else { DOUBLE };
        self.fill(area.inner(), ' ', style);
        self.hline(area.x + 1, area.y, area.w - 2, line, style);
        self.hline(area.x + 1, area.bottom() - 1, area.w - 2, line, style);
        self.vline(area.x, area.y + 1, area.h - 2, line, style);
        self.vline(area.right() - 1, area.y + 1, area.h - 2, line, style);
        self.put(area.x, area.y, c[2], style); self.put(area.right() - 1, area.y, c[3], style);
        self.put(area.x, area.bottom() - 1, c[4], style); self.put(area.right() - 1, area.bottom() - 1, c[5], style);
    }
    /// A frame with a title centred in its top edge (` title `).
    pub fn frame_titled(&mut self, area: Rect, line: Line, title: &str, style: Style, title_style: Style) {
        self.frame(area, line, style);
        if area.w <= 4 { return; }
        let len = title.chars().count().min(area.w - 4);
        let x = area.x + (area.w - len - 2) / 2;
        self.put(x, area.y, ' ', title_style);
        self.text_max(x + 1, area.y, title, len, title_style);
        self.put(x + 1 + len, area.y, ' ', title_style);
    }
    /// Horizontal bar of `width` cells filled to `value / max` with 1/8-cell steps.
    pub fn bar(&mut self, x: usize, y: usize, width: usize, value: u64, max: u64, fill: Style, empty: Style) {
        let eighths = if max == 0 { 0 } else { (value.min(max) as u128 * width as u128 * 8 / max as u128) as usize };
        for i in 0..width {
            let filled = eighths.saturating_sub(i * 8).min(8);
            match filled { 8 => self.put(x + i, y, '█', fill), 0 => self.put(x + i, y, ' ', empty), n => self.put(x + i, y, EIGHTHS[n], Style::new(fill.fg, empty.bg)) }
        }
    }
    /// Time-series graph in braille dots: each cell holds 2 samples x 4 levels, so `area` shows `2 * w` samples (the
    /// latest at the right) with `4 * h` levels between 0 and `max`.
    pub fn graph(&mut self, area: Rect, samples: &[u64], max: u64, style: Style) {
        let levels = area.h * 4;
        let shown = samples.len().min(area.w * 2);
        let first = samples.len() - shown;
        let offset = area.w * 2 - shown; // samples are right-aligned
        // Dot bits of a braille cell from the bottom row up, for the left and the right column.
        const LEFT: [u32; 4] = [0x40, 0x04, 0x02, 0x01];
        const RIGHT: [u32; 4] = [0x80, 0x20, 0x10, 0x08];
        for row in 0..area.h {
            for col in 0..area.w {
                let mut bits = 0u32;
                for half in 0..2 {
                    let slot = col * 2 + half;
                    if slot < offset { continue; }
                    let value = samples[first + slot - offset];
                    let height = if max == 0 { 0 } else { ((value.min(max) as u128 * levels as u128 + max as u128 / 2) / max as u128) as usize };
                    // Rows count from the top of the area; levels from the bottom.
                    let base = (area.h - 1 - row) * 4;
                    for dot in 0..4 { if height > base + dot { bits |= if half == 0 { LEFT[dot] } else { RIGHT[dot] }; } }
                }
                self.put(area.x + col, area.y + row, char::from_u32(0x2800 + bits).unwrap_or(' '), style);
            }
        }
    }
}

/// Decimal formatting into a small buffer, with thousands grouped by spaces (for sizes in panels).
pub fn grouped(value: u64, out: &mut [u8; 32]) -> &str {
    let mut digits = [0u8; 20]; let mut n = 0; let mut v = value;
    loop { digits[n] = b'0' + (v % 10) as u8; n += 1; v /= 10; if v == 0 { break; } }
    let mut len = 0;
    for i in (0..n).rev() {
        out[len] = digits[i]; len += 1;
        if i > 0 && i % 3 == 0 { out[len] = b' '; len += 1; }
    }
    core::str::from_utf8(&out[..len]).unwrap_or("")
}

/// Size with a unit: 999 B, 12.3 K, 4.0 M, 1.2 G.
pub fn human_size(bytes: u64, out: &mut crate::util::FixedBuf<16>) {
    use core::fmt::Write;
    out.clear();
    let units = [(1u64 << 30, 'G'), (1 << 20, 'M'), (1 << 10, 'K')];
    for (unit, letter) in units {
        if bytes >= unit {
            let tenths = bytes * 10 / unit;
            let _ = if tenths >= 1000 { write!(out, "{}{}", tenths / 10, letter) } else { write!(out, "{}.{}{}", tenths / 10, tenths % 10, letter) };
            return;
        }
    }
    let _ = write!(out, "{}B", bytes);
}
