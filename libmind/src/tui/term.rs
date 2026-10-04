//! The grid on the program's screen: a back buffer the program draws into and a copy of what is on the screen;
//! `present` draws only the cells that differ, plus the cursor.
use super::{Cell, Grid, Style};
use crate::gfx::Screen;
use crate::mem::Pages;

pub struct Terminal { screen: Screen, back: Pages, front: Pages, cols: usize, rows: usize, x0: usize, y0: usize, cursor: Option<(usize, usize)>, shown: Option<(usize, usize)>, valid: bool }

fn cells(pages: &mut Pages, count: usize) -> &mut [Cell] {
    unsafe { core::slice::from_raw_parts_mut(pages.as_mut_slice().as_mut_ptr() as *mut Cell, count) }
}

impl Terminal {
    /// A grid covering the screen in 8x16 cells (centred if the screen is not a multiple of the cell size).
    pub fn new(screen: Screen) -> Option<Self> {
        let (cols, rows) = (screen.width / 8, screen.height / 16);
        if cols == 0 || rows == 0 { return None; }
        let bytes = cols * rows * core::mem::size_of::<Cell>();
        let (mut back, mut front) = (Pages::new(bytes)?, Pages::new(bytes)?);
        for cell in cells(&mut back, cols * rows) { *cell = Cell::BLANK; }
        for cell in cells(&mut front, cols * rows) { *cell = Cell::BLANK; }
        Some(Self { screen, back, front, cols, rows, x0: (screen.width - cols * 8) / 2, y0: (screen.height - rows * 16) / 2, cursor: None, shown: None, valid: false })
    }
    pub fn cols(&self) -> usize { self.cols }
    pub fn rows(&self) -> usize { self.rows }
    /// The back buffer to draw the next frame into.
    pub fn grid(&mut self) -> Grid<'_> { let count = self.cols * self.rows; Grid::new(cells(&mut self.back, count), self.cols, self.rows) }
    /// Cursor (an underline in the cell's text colour) or none.
    pub fn set_cursor(&mut self, at: Option<(usize, usize)>) { self.cursor = at.filter(|&(x, y)| x < self.cols && y < self.rows); }
    /// Redraw everything on the next `present` (e.g. after another program drew on the screen).
    pub fn invalidate(&mut self) { self.valid = false; }

    fn draw_cell(&self, x: usize, y: usize, cell: Cell, cursor: bool) {
        let (px, py) = (self.x0 + x * 8, self.y0 + y * 16);
        self.screen.glyph16(px, py, cell.ch, cell.style.fg, Some(cell.style.bg));
        if cursor { self.screen.fill(px, py + 14, 8, 2, cell.style.fg); }
    }

    /// Puts the back buffer on the screen; returns the number of cells drawn.
    pub fn present(&mut self) -> usize {
        let count = self.cols * self.rows;
        let full = !core::mem::replace(&mut self.valid, true);
        if full && (self.x0 > 0 || self.y0 > 0) { self.screen.clear(0); }
        let back = unsafe { core::slice::from_raw_parts(self.back.as_slice().as_ptr() as *const Cell, count) };
        let front = cells(&mut self.front, count);
        let mut drawn = 0;
        let moved = self.cursor != self.shown;
        for index in 0..count {
            let (x, y) = (index % self.cols, index / self.cols);
            let on_cursor = self.cursor == Some((x, y)) || self.shown == Some((x, y));
            if full || back[index] != front[index] || (moved && on_cursor) {
                front[index] = back[index];
                let (px, py) = (self.x0 + x * 8, self.y0 + y * 16);
                self.screen.glyph16(px, py, back[index].ch, back[index].style.fg, Some(back[index].style.bg));
                drawn += 1;
            }
        }
        if let Some((x, y)) = self.cursor { let cell = back[y * self.cols + x]; self.draw_cell(x, y, cell, true); }
        self.shown = self.cursor;
        drawn
    }

    /// Fills the back buffer with one style (start of a frame).
    pub fn clear(&mut self, style: Style) { self.grid().clear(style); }
}
