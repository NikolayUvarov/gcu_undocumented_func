//! The grid on the program's screen: a back buffer the program draws into and a copy of what is on the screen;
//! `present` draws only the cells that differ, plus the cursor. In a window (issue 088) the cells go to the window's
//! surface with the rectangle that changed, and the window manager draws them; the grid follows the window's size.
use super::{Cell, Grid, Style};
use crate::abi::BootInfo;
use crate::gfx::Screen;
use crate::mem::Pages;
use crate::window::{Kind, Surface, MAX_COLUMNS, MAX_ROWS};

enum Output { Screen { screen: Screen, x0: usize, y0: usize }, Window(Surface) }

pub struct Terminal { output: Output, back: Pages, front: Pages, cols: usize, rows: usize, capacity: usize, cursor: Option<(usize, usize)>, shown: Option<(usize, usize)>, valid: bool }

fn cells(pages: &mut Pages, count: usize) -> &mut [Cell] {
    unsafe { core::slice::from_raw_parts_mut(pages.as_mut_slice().as_mut_ptr() as *mut Cell, count) }
}

// A cell no program draws: the front buffer holds it where the next `present` must draw.
const STALE: Cell = Cell { ch: '\u{FFFF}', style: Style { fg: 0, bg: 0 } };

impl Terminal {
    /// A grid covering the screen in 8x16 cells (centred if the screen is not a multiple of the cell size).
    pub fn new(screen: Screen) -> Option<Self> {
        let (cols, rows) = (screen.width / 8, screen.height / 16);
        Self::buffers(Output::Screen { screen, x0: (screen.width - cols * 8) / 2, y0: (screen.height - rows * 16) / 2 }, cols, rows, cols * rows)
    }

    /// The program's grid: in a text window titled `title` when its launcher started it in one (`mind::windowed`),
    /// else on its screen. The window has room for the screen's cells and starts at 80 × 25.
    pub fn open(info: &BootInfo, title: &str) -> Option<Self> {
        if crate::windowed::requested() {
            let capacity = ((info.width / 8).clamp(1, MAX_COLUMNS), (info.height / 16).clamp(1, MAX_ROWS));
            let size = (capacity.0.min(80), capacity.1.min(25));
            if let Some(surface) = crate::windowed::open(Kind::Text, capacity, size, title) {
                let mut terminal = Self::buffers(Output::Window(surface), size.0, size.1, capacity.0 * capacity.1)?;
                terminal.apply_resize();
                return Some(terminal);
            }
        }
        Screen::new(info).and_then(Self::new)
    }

    fn buffers(output: Output, cols: usize, rows: usize, capacity: usize) -> Option<Self> {
        if cols == 0 || rows == 0 { return None; }
        let bytes = capacity * core::mem::size_of::<Cell>();
        let (mut back, mut front) = (Pages::new(bytes)?, Pages::new(bytes)?);
        for cell in cells(&mut back, capacity) { *cell = Cell::BLANK; }
        for cell in cells(&mut front, capacity) { *cell = Cell::BLANK; }
        Some(Self { output, back, front, cols, rows, capacity, cursor: None, shown: None, valid: false })
    }

    // In a window: takes the size the manager asked for (within the window's memory) and draws everything again.
    fn apply_resize(&mut self) {
        let Output::Window(surface) = &self.output else { return };
        let Some((cols, rows)) = crate::windowed::resize() else { return };
        let (cols, rows) = (cols.clamp(1, MAX_COLUMNS), rows.clamp(1, MAX_ROWS));
        if cols * rows > self.capacity || !surface.set_size(cols, rows) { return; }
        self.cols = cols; self.rows = rows; self.valid = false;
        let capacity = self.capacity;
        for cell in cells(&mut self.back, capacity) { *cell = Cell::BLANK; }
    }

    pub fn cols(&self) -> usize { self.cols }
    pub fn rows(&self) -> usize { self.rows }
    /// The back buffer to draw the next frame into (in a window, at the size the manager last asked for).
    pub fn grid(&mut self) -> Grid<'_> { self.apply_resize(); let count = self.cols * self.rows; Grid::new(cells(&mut self.back, count), self.cols, self.rows) }
    /// Cursor (an underline in the cell's text colour) or none.
    pub fn set_cursor(&mut self, at: Option<(usize, usize)>) { self.cursor = at.filter(|&(x, y)| x < self.cols && y < self.rows); }
    /// Redraw everything on the next `present` (e.g. after another program drew on the screen).
    pub fn invalidate(&mut self) { self.valid = false; }
    /// Draw cell (x, y) on the next `present` (something else drew over it).
    pub fn touch(&mut self, x: usize, y: usize) { if x < self.cols && y < self.rows { let count = self.cols * self.rows; cells(&mut self.front, count)[y * self.cols + x] = STALE; } }
    /// The window's title (nothing on a screen).
    pub fn set_title(&mut self, title: &str) { if let Output::Window(surface) = &self.output { surface.set_title(title); } }
    /// The screen and where the grid starts on it, for a program that draws pixels between the cells (`wm`).
    pub fn screen(&self) -> Option<(Screen, usize, usize)> { match self.output { Output::Screen { screen, x0, y0 } => Some((screen, x0, y0)), Output::Window(_) => None } }

    /// Puts the back buffer on the screen or into the window; returns the number of cells drawn.
    pub fn present(&mut self) -> usize {
        let count = self.cols * self.rows;
        let full = !core::mem::replace(&mut self.valid, true);
        if let (true, Output::Screen { screen, x0, y0 }) = (full, &self.output) { if *x0 > 0 || *y0 > 0 { screen.clear(0); } }
        let back = unsafe { core::slice::from_raw_parts(self.back.as_slice().as_ptr() as *const Cell, count) };
        let front = cells(&mut self.front, count);
        let mut drawn = 0;
        let moved = self.cursor != self.shown;
        let (mut left, mut top, mut right, mut bottom) = (usize::MAX, usize::MAX, 0, 0);
        for index in 0..count {
            let (x, y) = (index % self.cols, index / self.cols);
            let on_cursor = self.cursor == Some((x, y)) || self.shown == Some((x, y));
            if full || back[index] != front[index] || (moved && on_cursor) {
                front[index] = back[index];
                let cell = back[index];
                match &self.output {
                    Output::Screen { screen, x0, y0 } => screen.glyph16(x0 + x * 8, y0 + y * 16, cell.ch, cell.style.fg, Some(cell.style.bg)),
                    Output::Window(surface) => surface.set_cell(x, y, cell.ch, cell.style.fg, cell.style.bg),
                }
                (left, top, right, bottom) = (left.min(x), top.min(y), right.max(x + 1), bottom.max(y + 1));
                drawn += 1;
            }
        }
        match &self.output {
            Output::Screen { screen, x0, y0 } => if let Some((x, y)) = self.cursor {
                let cell = back[y * self.cols + x];
                let (px, py) = (x0 + x * 8, y0 + y * 16);
                screen.glyph16(px, py, cell.ch, cell.style.fg, Some(cell.style.bg));
                screen.fill(px, py + 14, 8, 2, cell.style.fg);
            },
            Output::Window(surface) => {
                surface.set_cursor(self.cursor);
                if drawn > 0 { surface.changed(if full { None } else { Some((left, top, right - left, bottom - top)) }); }
                else if moved { surface.changed(self.cursor.or(self.shown).map(|(x, y)| (x, y, 1, 1))); }
            }
        }
        self.shown = self.cursor;
        drawn
    }

    /// Fills the back buffer with one style (start of a frame).
    pub fn clear(&mut self, style: Style) { self.grid().clear(style); }
}
