//! `wm`'s desk over its screens (177-APP-0094, on KRN's 177-KRN-0274; three at once on the Ryzen, the maintainer's word of
//! 2026-11-27): the first screen and each other placed on a side of an earlier one, aligned at its top or left edge, as
//! one desk of cells; which screen a cell is on; the screen beside another; a window moved to another screen, keeping
//! its place there as it had it on its own. No system calls: tests/wm_host.rs.
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
    /// The words before "this one": "to the right of", "above".
    pub fn words(self) -> &'static str { match self { Side::Right => "to the right of", Side::Left => "to the left of", Side::Above => "above", Side::Below => "below" } }
}

/// The desk of the screens: its size in cells and each screen's rectangle in it, the first screen first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout { pub cols: usize, pub rows: usize, pub screens: Vec<Rect> }

/// A screen beside one placed before it: its size in cells, the side of that screen it is on, and that screen (0 the
/// first).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Beside { pub size: (usize, usize), pub side: Side, pub of: usize }

/// The desk for the first screen of `first` cells and each of `others` in turn on its side of an earlier screen (the
/// first when `of` names none placed yet), aligned at that one's top or left edge, and past any screen already on that
/// side. What no screen covers, when their sizes differ, is in the desk but on no screen.
pub fn layout(first: (usize, usize), others: &[Beside]) -> Layout {
    let mut placed: Vec<(i64, i64, i64, i64)> = alloc::vec![(0, 0, first.0 as i64, first.1 as i64)];
    let overlap = |a: (i64, i64, i64, i64), b: (i64, i64, i64, i64)| a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3;
    for b in others {
        let of = placed[if b.of < placed.len() { b.of } else { 0 }];
        let (w, h) = (b.size.0 as i64, b.size.1 as i64);
        let mut r = match b.side {
            Side::Right => (of.0 + of.2, of.1, w, h),
            Side::Left => (of.0 - w, of.1, w, h),
            Side::Below => (of.0, of.1 + of.3, w, h),
            Side::Above => (of.0, of.1 - h, w, h),
        };
        // Each step goes past one screen, on that side: so at most one step a screen placed.
        while let Some(&o) = placed.iter().find(|&&o| overlap(r, o)) {
            match b.side { Side::Right => r.0 = o.0 + o.2, Side::Left => r.0 = o.0 - w, Side::Below => r.1 = o.1 + o.3, Side::Above => r.1 = o.1 - h }
        }
        placed.push(r);
    }
    let (x0, y0) = (placed.iter().map(|r| r.0).min().unwrap_or(0), placed.iter().map(|r| r.1).min().unwrap_or(0));
    let screens: Vec<Rect> = placed.iter().map(|r| Rect::new((r.0 - x0) as usize, (r.1 - y0) as usize, r.2 as usize, r.3 as usize)).collect();
    let cols = screens.iter().map(|r| r.x + r.w).max().unwrap_or(0);
    let rows = screens.iter().map(|r| r.y + r.h).max().unwrap_or(0);
    Layout { cols, rows, screens }
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

    /// The screen on `side` of screen `from`: of those that lie wholly on that side and share some of its rows (or
    /// columns), the nearest.
    pub fn beside(&self, from: usize, side: Side) -> Option<usize> {
        let f = *self.screens.get(from)?;
        let shares = |a: usize, a_len: usize, b: usize, b_len: usize| a < b + b_len && b < a + a_len;
        let gap = |r: &Rect| match side {
            Side::Right if r.x >= f.x + f.w && shares(r.y, r.h, f.y, f.h) => Some(r.x - (f.x + f.w)),
            Side::Left if r.x + r.w <= f.x && shares(r.y, r.h, f.y, f.h) => Some(f.x - (r.x + r.w)),
            Side::Below if r.y >= f.y + f.h && shares(r.x, r.w, f.x, f.w) => Some(r.y - (f.y + f.h)),
            Side::Above if r.y + r.h <= f.y && shares(r.x, r.w, f.x, f.w) => Some(f.y - (r.y + r.h)),
            _ => None,
        };
        (0..self.screens.len()).filter(|&i| i != from).filter_map(|i| gap(&self.screens[i]).map(|g| (g, i))).min().map(|(_, i)| i)
    }

    /// A window of `window` (in the desk) moved from its screen to screen `to`: at the same place within it, in
    /// proportion to the screens' sizes, its size kept as far as the screen holds it, and inside it.
    pub fn moved(&self, window: Rect, to: usize) -> Rect { carried(window, self.screens[self.screen_of(window.x, window.y)], self.screens[to]) }
}

/// A window of `window` in rectangle `from` carried into `dest`: at the same place in proportion, its size kept as far
/// as `dest` holds it, and inside it.
pub fn carried(window: Rect, from: Rect, dest: Rect) -> Rect {
    let (w, h) = (window.w.min(dest.w), window.h.min(dest.h));
    let scale = |offset: usize, from_len: usize, to_len: usize, len: usize| {
        let room = to_len.saturating_sub(len);
        if from_len <= 1 { 0 } else { (offset * to_len / from_len).min(room) }
    };
    let x = dest.x + scale(window.x.saturating_sub(from.x), from.w, dest.w, w);
    let y = dest.y + scale(window.y.saturating_sub(from.y), from.h, dest.h, h);
    Rect::new(x, y, w, h)
}

/// A window of `window` in rectangle `from` given the same share of `dest`: its edges scaled, for one that fills a
/// half, a quarter or the whole of a screen's area.
pub fn scaled(window: Rect, from: Rect, dest: Rect) -> Rect {
    let along = |at: usize, start: usize, from_len: usize, to_start: usize, to_len: usize| to_start + at.saturating_sub(start).min(from_len) * to_len / from_len.max(1);
    let (x0, x1) = (along(window.x, from.x, from.w, dest.x, dest.w), along(window.right(), from.x, from.w, dest.x, dest.w));
    let (y0, y1) = (along(window.y, from.y, from.h, dest.y, dest.h), along(window.bottom(), from.y, from.h, dest.y, dest.h));
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// The cells where windows go on screen `s` of `layout`: the first below the top bar and above the status line, which
/// are its own; another screen whole.
pub fn area(layout: &Layout, s: usize) -> Rect {
    match layout.screens.get(s) {
        Some(&r) if s > 0 => r,
        _ => { let r = layout.screens[0]; Rect::new(r.x, r.y + 1, r.w, r.h.saturating_sub(2)) }
    }
}
