//! `wm`'s desk over two screens (177-APP-0094, on KRN's 177-KRN-0274): the first screen and the second placed on a side
//! of it, aligned at its top or left edge, as one desk of cells; which screen a cell is on; the screen beside another;
//! a window moved to another screen, keeping its place there as it had it on its own. No system calls:
//! tests/wm_host.rs.
use crate::tui::Rect;
use alloc::vec::Vec;

/// Where the second screen is, beside the first (`display.wit` 1.5's `side`, as planned).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side { Right, Left, Above, Below }

impl Side {
    pub fn name(self) -> &'static str { match self { Side::Right => "right", Side::Left => "left", Side::Above => "above", Side::Below => "below" } }
    pub fn from_name(word: &str) -> Option<Side> {
        match word { "right" => Some(Side::Right), "left" => Some(Side::Left), "above" => Some(Side::Above), "below" => Some(Side::Below), _ => None }
    }
}

/// The desk of the screens: its size in cells and each screen's rectangle in it, the first screen first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout { pub cols: usize, pub rows: usize, pub screens: Vec<Rect> }

/// The desk for the first screen of `first` cells and, if extended, the second of `second` cells on `side`. What
/// neither screen covers, when their sizes differ, is in the desk but on no screen.
pub fn layout(first: (usize, usize), second: Option<((usize, usize), Side)>) -> Layout {
    let (w1, h1) = first;
    let Some(((w2, h2), side)) = second else { return Layout { cols: w1, rows: h1, screens: alloc::vec![Rect::new(0, 0, w1, h1)] } };
    let (a, b) = match side {
        Side::Right => (Rect::new(0, 0, w1, h1), Rect::new(w1, 0, w2, h2)),
        Side::Left => (Rect::new(w2, 0, w1, h1), Rect::new(0, 0, w2, h2)),
        Side::Below => (Rect::new(0, 0, w1, h1), Rect::new(0, h1, w2, h2)),
        Side::Above => (Rect::new(0, h2, w1, h1), Rect::new(0, 0, w2, h2)),
    };
    let cols = (a.x + a.w).max(b.x + b.w);
    let rows = (a.y + a.h).max(b.y + b.h);
    Layout { cols, rows, screens: alloc::vec![a, b] }
}

fn holds(r: &Rect, x: usize, y: usize) -> bool { x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h }

impl Layout {
    /// The screen cell (x, y) is on; in the corner neither covers, the nearest one.
    pub fn screen_of(&self, x: usize, y: usize) -> usize {
        if let Some(i) = self.screens.iter().position(|r| holds(r, x, y)) { return i; }
        let distance = |r: &Rect| {
            let dx = if x < r.x { r.x - x } else { x.saturating_sub(r.x + r.w - 1) };
            let dy = if y < r.y { r.y - y } else { y.saturating_sub(r.y + r.h - 1) };
            dx + dy
        };
        (0..self.screens.len()).min_by_key(|&i| distance(&self.screens[i])).unwrap_or(0)
    }

    /// The screen on `side` of screen `from`, if one lies there.
    pub fn beside(&self, from: usize, side: Side) -> Option<usize> {
        let f = *self.screens.get(from)?;
        self.screens.iter().position(|r| match side {
            Side::Right => r.x >= f.x + f.w,
            Side::Left => r.x + r.w <= f.x,
            Side::Below => r.y >= f.y + f.h,
            Side::Above => r.y + r.h <= f.y,
        })
    }

    /// A window of `window` (in the desk) moved from its screen to screen `to`: at the same place within it, in
    /// proportion to the screens' sizes, its size kept as far as the screen holds it, and inside it.
    pub fn moved(&self, window: Rect, to: usize) -> Rect {
        let from = self.screens[self.screen_of(window.x, window.y)];
        let dest = self.screens[to];
        let (w, h) = (window.w.min(dest.w), window.h.min(dest.h));
        let scale = |offset: usize, from_len: usize, to_len: usize, len: usize| {
            let room = to_len.saturating_sub(len);
            if from_len <= 1 { 0 } else { (offset * to_len / from_len).min(room) }
        };
        let x = dest.x + scale(window.x.saturating_sub(from.x), from.w, dest.w, w);
        let y = dest.y + scale(window.y.saturating_sub(from.y), from.h, dest.h, h);
        Rect::new(x, y, w, h)
    }
}
