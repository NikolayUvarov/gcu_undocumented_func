//! The desktop of `wm` (issue 088): windows on the 8×16 cell grid — a frame with the title, a close mark and a resize
//! corner — stacked bottom to top with the top one focused; moving and resizing by keys and by the mouse; snapping to
//! halves, quarters and the whole screen; what each cell of the screen shows. No system calls: tests/wm_host.rs.
use crate::keys::{Code, Key};
use crate::menu::{self, Menu};
use crate::tui::widgets::{message, Edit, InputLine};
use crate::tui::{Cell, Grid, Line, Rect, Style, Theme};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// What a window shows: cells the program wrote, or pixels drawn over the cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content { Text, Pixels }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Win {
    pub id: u32,
    pub owner: u64,
    pub content: Content,
    /// The content's size: cells (text) or pixels.
    pub size: (usize, usize),
    pub title: String,
    /// The frame on the screen, border included.
    pub rect: Rect,
    /// The frame it had before it was maximized or snapped to a half or a quarter (issue u002): `[⇕]` and a drag of
    /// the title give it back.
    pub restore: Option<Rect>,
    /// The frame Alt+Enter maximized it from (a snapped one too): Alt+Enter again goes back there.
    pub before_max: Option<Rect>,
    /// A program wm lent a lease of it to is recording it (issue u014): "● REC" on its frame.
    pub recording: bool,
    /// Alt+F (211-APP-0014): in front, its content covers the whole screen, without the frame or the bars; its frame
    /// stays as it was, for when it ends.
    pub full: bool,
}

impl Win {
    pub fn new(id: u32, owner: u64, content: Content, size: (usize, usize), title: &str) -> Self {
        Self { id, owner, content, size, title: String::from(title), rect: Rect::default(), restore: None, before_max: None, recording: false, full: false }
    }
    /// The frame that shows all of the content.
    pub fn natural(&self) -> (usize, usize) {
        match self.content { Content::Text => (self.size.0 + 2, self.size.1 + 2), Content::Pixels => (self.size.0.div_ceil(8) + 2, self.size.1.div_ceil(16) + 2) }
    }
}

/// What is under a cell of the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit { Desktop, Title(u32), Close(u32), Zoom(u32), Corner(u32), Border(u32), Content(u32) }

/// The cell under a pixel window's content: what is still this cell after everything is drawn shows the pixels.
pub const PIXELS: Cell = Cell { ch: ' ', style: Style { fg: 0, bg: 0 } };

/// Smallest frame, and how near an edge a moved window sticks to it (cells).
pub const MIN: (usize, usize) = (16, 4);
pub const SNAP: usize = 2;

pub struct Desk {
    pub cols: usize,
    pub rows: usize,
    /// Bottom to top: the last one is in front and has the focus.
    pub windows: Vec<Win>,
    changed: Vec<u32>,
}

impl Desk {
    pub fn new(cols: usize, rows: usize) -> Self { Self { cols, rows, windows: Vec::new(), changed: Vec::new() } }

    /// Where windows go: below the top bar, above the status line.
    pub fn area(&self) -> Rect { Rect::new(0, 1, self.cols, self.rows.saturating_sub(2)) }
    pub fn index(&self, id: u32) -> Option<usize> { self.windows.iter().position(|w| w.id == id) }
    pub fn get(&self, id: u32) -> Option<&Win> { self.windows.iter().find(|w| w.id == id) }
    /// The window in front, which gets the keys.
    pub fn focused(&self) -> Option<&Win> { self.windows.last() }
    pub fn focus(&self) -> Option<u32> { self.focused().map(|w| w.id) }
    /// The window on the whole screen: the one in front, if it is full screen.
    pub fn full_screen(&self) -> Option<u32> { self.focused().filter(|w| w.full).map(|w| w.id) }
    /// Where a window's content is drawn: inside its frame, or the whole screen.
    pub fn content(&self, id: u32) -> Rect {
        if self.full_screen() == Some(id) { return Rect::new(0, 0, self.cols, self.rows); }
        self.get(id).map_or(Rect::default(), |w| w.rect.inner())
    }

    /// Alt+F: full screen for a window, or its frame back.
    pub fn toggle_full(&mut self, id: u32) {
        let Some(index) = self.index(id) else { return };
        self.windows[index].full = !self.windows[index].full;
        if !self.changed.contains(&id) { self.changed.push(id); }
    }
    // A window moved, resized or snapped leaves full screen first.
    fn leave_full(&mut self, id: u32) { if self.get(id).is_some_and(|w| w.full) { self.toggle_full(id); } }

    /// Whether every cell of window `index`'s frame is under the frames of windows in front of it.
    pub fn covered(&self, index: usize) -> bool {
        let Some(w) = self.windows.get(index) else { return false };
        let above = &self.windows[index + 1..];
        if above.iter().any(|a| a.full && a.id == self.focus().unwrap_or(u32::MAX)) { return true; }
        let r = w.rect;
        (r.y..r.bottom()).all(|y| (r.x..r.right()).all(|x| above.iter().any(|a| x >= a.rect.x && x < a.rect.right() && y >= a.rect.y && y < a.rect.bottom())))
    }

    /// The windows as the list shows them (211-APP-0014): in the order they opened, each with its state.
    pub fn listing(&self) -> Vec<(u32, String)> {
        let area = self.area();
        let mut ids: Vec<u32> = self.windows.iter().map(|w| w.id).collect();
        ids.sort_unstable();
        ids.iter().map(|&id| {
            let index = self.index(id).unwrap_or(0);
            let w = &self.windows[index];
            let mut state = Vec::new();
            state.push(if Some(id) == self.focus() { "in front" } else if self.covered(index) { "hidden" } else { "behind" });
            if w.full { state.push("full screen"); }
            if w.rect == area { state.push("maximized"); }
            (id, state.join(", "))
        }).collect()
    }

    /// Windows whose frame changed since the last call: their place is saved, text programs get the new size.
    pub fn take_changed(&mut self) -> Vec<u32> { core::mem::take(&mut self.changed) }

    fn set(&mut self, id: u32, rect: Rect) {
        let rect = self.fit(rect);
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            if w.rect != rect { w.rect = rect; if !self.changed.contains(&id) { self.changed.push(id); } }
        }
    }

    // Before a window is maximized or snapped: the frame it has now is the one to give back, unless one is kept already.
    fn keep_restore(&mut self, id: u32) {
        if let Some(index) = self.index(id) { let w = &mut self.windows[index]; if w.restore.is_none() { w.restore = Some(w.rect); } }
    }
    /// A window moved or resized by hand is no longer snapped.
    pub fn forget_restore(&mut self, id: u32) { self.set_restore(id, None); }
    pub fn set_restore(&mut self, id: u32, restore: Option<Rect>) {
        if let Some(index) = self.index(id) { let w = &mut self.windows[index]; w.restore = restore; w.before_max = None; }
    }

    /// `rect` made at least `MIN`, at most the area, and moved inside it.
    pub fn fit(&self, rect: Rect) -> Rect {
        let area = self.area();
        let (w, h) = (rect.w.clamp(MIN.0.min(area.w), area.w.max(1)), rect.h.clamp(MIN.1.min(area.h), area.h.max(1)));
        let x = rect.x.min(area.right().saturating_sub(w));
        let y = rect.y.clamp(area.y, area.bottom().saturating_sub(h).max(area.y));
        Rect::new(x, y, w, h)
    }

    /// The first four windows take the quarters in turn, text ones filling them; later ones cascade from the top left.
    pub fn place_new(&self, win: &Win) -> Rect {
        let area = self.area();
        let natural = win.natural();
        let n = self.windows.len();
        if n < 4 {
            let quarter = self.quarter_rect(n + 1);
            return match win.content { Content::Text => quarter, Content::Pixels => Rect::new(quarter.x, quarter.y, natural.0.min(quarter.w), natural.1.min(quarter.h)) };
        }
        let step = 2 * (n - 4) % area.h.max(1).min(area.w.max(1));
        let size = match win.content { Content::Text => (82.min(area.w), 27.min(area.h)), Content::Pixels => natural };
        self.fit(Rect::new(area.x + step, area.y + step, size.0, size.1))
    }

    /// A window goes on top with the focus; one without a place gets one.
    pub fn add(&mut self, mut win: Win) {
        if win.rect.w == 0 || win.rect.h == 0 { win.rect = self.place_new(&win); }
        win.rect = self.fit(win.rect);
        let id = win.id;
        self.windows.retain(|w| w.id != id);
        self.windows.push(win);
        if !self.changed.contains(&id) { self.changed.push(id); }
    }

    pub fn remove(&mut self, id: u32) -> Option<Win> { let index = self.index(id)?; self.changed.retain(|&c| c != id); Some(self.windows.remove(index)) }

    /// Puts a window in front with the focus.
    pub fn raise(&mut self, id: u32) { if let Some(index) = self.index(id) { let w = self.windows.remove(index); self.windows.push(w); } }

    /// Alt+Tab: the window at the bottom comes to the front; Alt+Shift+Tab: the front one goes to the bottom.
    pub fn cycle(&mut self, back: bool) {
        if self.windows.len() < 2 { return; }
        if back { let w = self.windows.pop().unwrap(); self.windows.insert(0, w); } else { let w = self.windows.remove(0); self.windows.push(w); }
    }

    pub fn move_by(&mut self, id: u32, dx: isize, dy: isize) {
        self.leave_full(id);
        self.forget_restore(id);
        let Some(w) = self.get(id) else { return };
        let r = w.rect;
        self.set(id, Rect::new(r.x.saturating_add_signed(dx), r.y.saturating_add_signed(dy).max(self.area().y), r.w, r.h));
    }
    pub fn resize_by(&mut self, id: u32, dw: isize, dh: isize) {
        self.leave_full(id);
        self.forget_restore(id);
        let Some(w) = self.get(id) else { return };
        let r = w.rect;
        let area = self.area();
        let (width, height) = (r.w.saturating_add_signed(dw).min(area.right() - r.x), r.h.saturating_add_signed(dh).min(area.bottom().saturating_sub(r.y)));
        self.set(id, Rect::new(r.x, r.y, width, height));
    }
    /// Moves a window's frame to `rect` (kept inside the area), as a hand does: it is no longer snapped.
    pub fn place(&mut self, id: u32, rect: Rect) { self.leave_full(id); self.forget_restore(id); self.set(id, rect); }

    /// Half the area: 0 left, 1 right, 2 top, 3 bottom.
    pub fn half_rect(&self, side: usize) -> Rect {
        let a = self.area();
        match side { 0 => Rect::new(a.x, a.y, a.w / 2, a.h), 1 => Rect::new(a.x + a.w / 2, a.y, a.w - a.w / 2, a.h), 2 => Rect::new(a.x, a.y, a.w, a.h / 2), _ => Rect::new(a.x, a.y + a.h / 2, a.w, a.h - a.h / 2) }
    }
    /// A quarter: 1 top left, 2 top right, 3 bottom left, 4 bottom right.
    pub fn quarter_rect(&self, n: usize) -> Rect {
        let a = self.area();
        let (w, h) = (a.w / 2, a.h / 2);
        let (right, bottom) = (n == 2 || n == 4, n >= 3);
        Rect::new(if right { a.x + w } else { a.x }, if bottom { a.y + h } else { a.y }, if right { a.w - w } else { w }, if bottom { a.h - h } else { h })
    }
    pub fn half(&mut self, id: u32, side: usize) { let r = self.half_rect(side); self.leave_full(id); self.keep_restore(id); self.set(id, r); }
    pub fn quarter(&mut self, id: u32, n: usize) { let r = self.quarter_rect(n); self.leave_full(id); self.keep_restore(id); self.set(id, r); }

    /// Alt+Enter: the whole area, or back to where it was maximized from.
    pub fn maximize(&mut self, id: u32) {
        self.leave_full(id);
        let area = self.area();
        let Some(index) = self.index(id) else { return };
        let w = &mut self.windows[index];
        if w.rect == area {
            let Some(back) = w.before_max.take().or(w.restore) else { return };
            if w.restore == Some(back) { w.restore = None; } // back where it floated
            self.set(id, back);
        } else {
            w.before_max = Some(w.rect);
            self.keep_restore(id);
            self.set(id, area);
        }
    }

    /// `[⇕]`: back to the frame before it was maximized or snapped (issue u002).
    pub fn restore(&mut self, id: u32) {
        let Some(index) = self.index(id) else { return };
        let w = &mut self.windows[index];
        w.before_max = None;
        if let Some(old) = w.restore.take() { self.set(id, old); }
    }

    /// `[▲]` maximizes a window, `[⇕]` gives a maximized or snapped one its frame back.
    pub fn zoom(&mut self, id: u32) {
        if self.get(id).is_some_and(|w| w.restore.is_some()) { self.restore(id); } else { self.maximize(id); }
    }

    /// After a move: a window within `SNAP` cells of an edge sticks to it — a corner takes that quarter, a side half
    /// the area, the top the whole area.
    pub fn snap(&mut self, id: u32) {
        let Some(w) = self.get(id) else { return };
        let (r, a) = (w.rect, self.area());
        let (left, right) = (r.x <= a.x + SNAP, r.right() + SNAP >= a.right());
        let (top, bottom) = (r.y <= a.y + SNAP, r.bottom() + SNAP >= a.bottom());
        let target = match (left, right, top, bottom) {
            (true, true, ..) | (_, _, true, true) => return, // it already spans the area that way
            (true, false, true, false) => self.quarter_rect(1),
            (false, true, true, false) => self.quarter_rect(2),
            (true, false, false, true) => self.quarter_rect(3),
            (false, true, false, true) => self.quarter_rect(4),
            (true, false, false, false) => self.half_rect(0),
            (false, true, false, false) => self.half_rect(1),
            (false, false, true, false) => { if let Some(index) = self.index(id) { self.windows[index].before_max = Some(r); } a }
            (false, false, false, true) => self.half_rect(3),
            (false, false, false, false) => return,
        };
        self.keep_restore(id);
        self.set(id, target);
    }

    /// What is at cell (x, y): the front-most window there decides.
    pub fn hit(&self, x: usize, y: usize) -> Hit {
        if let Some(id) = self.full_screen() { return Hit::Content(id); }
        for w in self.windows.iter().rev() {
            let r = w.rect;
            if x < r.x || x >= r.right() || y < r.y || y >= r.bottom() { continue; }
            return if y == r.y {
                if r.w >= 8 && x + 5 >= r.right() && x + 2 < r.right() { Hit::Close(w.id) }
                else if r.w >= 11 && x + 8 >= r.right() && x + 5 < r.right() { Hit::Zoom(w.id) }
                else { Hit::Title(w.id) }
            } else if x + 1 == r.right() && y + 1 == r.bottom() { Hit::Corner(w.id) }
            else if x == r.x || x + 1 == r.right() || y + 1 == r.bottom() { Hit::Border(w.id) }
            else { Hit::Content(w.id) };
        }
        Hit::Desktop
    }

    /// Draws the desktop and the windows into `grid` (the screen's size). Text content comes from `cell` (window,
    /// column, row → character and colours); pixel content is drawn over the cells afterwards. Returns which window's
    /// content each cell shows (index + 1, 0: none), for the pixels.
    pub fn draw(&self, grid: &mut Grid, theme: &Theme, cell: &mut dyn FnMut(u32, usize, usize) -> Option<(char, u32, u32)>) -> Vec<u16> {
        let (cols, rows) = (self.cols.min(grid.cols), self.rows.min(grid.rows));
        let mut owner = alloc::vec![0u16; cols * rows];
        if let Some(id) = self.full_screen() {
            let (index, w) = (self.index(id).unwrap_or(0), self.focused().unwrap());
            for y in 0..rows {
                for x in 0..cols {
                    match w.content {
                        Content::Text => { let (ch, fg, bg) = if x < w.size.0 && y < w.size.1 { cell(id, x, y).unwrap_or((' ', 0, 0)) } else { (' ', theme.panel.fg, 0) }; grid.put(x, y, ch, Style::new(fg, bg)); }
                        Content::Pixels => { grid.put(x, y, PIXELS.ch, PIXELS.style); owner[y * cols + x] = index as u16 + 1; }
                    }
                }
            }
            return owner;
        }
        grid.fill(self.area(), '░', Style::new(0x30485C, 0x182430));
        let top = self.windows.len().saturating_sub(1);
        for (index, w) in self.windows.iter().enumerate() {
            let r = w.rect;
            if r.w < 2 || r.h < 2 { continue; }
            let focused = index == top;
            let frame = if focused { Style::new(0xFFFFFF, theme.panel.bg) } else { theme.frame };
            let title_style = if focused { theme.selected } else { theme.frame };
            grid.frame_titled(r, if focused { Line::Double } else { Line::Single }, &w.title, frame, title_style);
            if r.w >= 8 { grid.text(r.right() - 5, r.y, "[×]", if focused { Style::new(0xFFFFFF, 0xA03030) } else { frame }); }
            if r.w >= 11 { grid.text(r.right() - 8, r.y, if w.restore.is_some() { "[⇕]" } else { "[▲]" }, frame); }
            grid.put(r.right() - 1, r.bottom() - 1, '◆', if focused { theme.accent } else { frame });
            if w.recording && r.w >= 24 { grid.text(r.x + 1, r.y, " ● REC ", Style::new(0xFFFFFF, 0xC02020)); }
            let inner = r.inner();
            for y in inner.y..inner.bottom().min(rows) {
                for x in inner.x..inner.right().min(cols) {
                    let (cx, cy) = (x - inner.x, y - inner.y);
                    match w.content {
                        Content::Text => {
                            let (ch, fg, bg) = if cx < w.size.0 && cy < w.size.1 { cell(w.id, cx, cy).unwrap_or((' ', 0, 0)) } else { (' ', theme.panel.fg, 0) };
                            grid.put(x, y, ch, Style::new(fg, bg));
                            owner[y * cols + x] = 0;
                        }
                        Content::Pixels => { grid.put(x, y, PIXELS.ch, PIXELS.style); owner[y * cols + x] = index as u16 + 1; }
                    }
                }
            }
            // The frame covers what is below it.
            for y in r.y..r.bottom().min(rows) {
                for x in r.x..r.right().min(cols) { if x < inner.x || x >= inner.right() || y < inner.y || y >= inner.bottom() { owner[y * cols + x] = 0; } }
            }
        }
        owner
    }

    /// One line of state for the log (tests follow it): the windows bottom to top as `id@x,y,wxh`.
    pub fn status(&self) -> String {
        let windows: Vec<String> = self.windows.iter().map(|w| format!("{}@{},{},{}x{}", w.id, w.rect.x, w.rect.y, w.rect.w, w.rect.h)).collect();
        let full = self.full_screen().map_or(String::new(), |id| format!(" FULL={}", id));
        format!("FOCUS={} WINDOWS={}{}", self.focus().map_or(String::from("-"), |id| format!("{}", id)), windows.join(" "), full)
    }
}

/// What the window manager does after a key or a mouse event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Not for `wm`: the focused window's program gets it.
    Forward,
    /// Handled; draw again.
    Redraw,
    /// Close a window: its program is asked to end.
    Close(u32),
    /// Start a program in a new window.
    Run(String),
    /// Leave: the programs keep running, their windows hidden, until the next `wm`.
    Detach,
    /// Ask every program to end, then leave.
    CloseAll,
    /// A mouse event for the program of window `id` (issue u001): at cell (x, y) of its content, with the buttons
    /// held and the wheel's steps.
    Pointer { id: u32, x: usize, y: usize, buttons: u8, wheel: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drag {
    /// The title held at (dx, dy) of the frame, pressed at `from` until the mouse first moves; a maximized or snapped
    /// window gets its frame back then.
    Move { id: u32, dx: usize, dy: usize, from: Option<(usize, usize)>, unsnap: bool },
    Resize { id: u32 },
}

/// `List`: the window list (211-APP-0014), the selected window and its place in the list (kept when it closes).
pub enum Mode { Normal, Move { id: u32, before: Rect, restore: Option<Rect> }, Run(InputLine), Help, Menu(Menu), List { id: Option<u32>, at: usize } }

pub const HELP: [&str; 18] = [
    "Alt+Tab, Alt+Shift+Tab — the next window, the previous one",
    "Alt+← → ↑ ↓ — half the screen; Alt+1…4 — a quarter; Alt+Enter — maximize or restore",
    "Alt+F — full screen for the window in front, without the frame or the bars; Alt+F again: its frame back",
    "Alt+L — the list of windows: arrows and Enter or a click bring one to the front, Alt+W closes it",
    "Alt+M — move and resize: arrows move, Shift+arrows resize, Enter ends (at an edge it snaps), Esc goes back",
    "Alt+W or Alt+F4 — close the window (its program ends)",
    "Alt+R — run a program in a new window: fm, top, clock, dzen-clock, edit <file>, …",
    "Alt+P or a right click on the desktop — the programs by category: a click or Enter starts one",
    "Alt+Q — leave wm: the programs keep running, the next wm shows them where they were",
    "Alt+X — close every window and leave",
    "The items of the top bar can be clicked instead of their keys (\"wm\": the programs)",
    "Mouse: a click brings a window to the front and goes to its program, as the wheel does;",
    "  drag the title to move a window (it snaps at the edges; a snapped one gets its size back),",
    "  the ◆ corner to resize it; [▲] maximizes, [⇕] gives the size back; [×] closes",
    "Every other key goes to the window in front only.",
    "Programs started here get only what wm holds and they ask for: the user's files,",
    "  system information, a window; nothing else.",
    "",
];

/// What a click on an item of the top bar does (issue u008): the same as its key — for a host that keeps Alt+Tab and
/// the like for itself. "wm" at the left opens the programs, as Alt+P does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar { Programs, Next, Run, Move, Close, Help, Leave, List, Full }

pub const BAR: [(&str, Bar); 9] = [("Alt+Tab next", Bar::Next), ("Alt+P programs", Bar::Programs), ("Alt+R run", Bar::Run), ("Alt+M move", Bar::Move),
                                    ("Alt+W close", Bar::Close), ("Alt+H help", Bar::Help), ("Alt+Q leave", Bar::Leave), ("Alt+L windows", Bar::List),
                                    ("Alt+F full", Bar::Full)];

/// The items of the top bar on a screen `cols` wide: the cells each covers (x, width) and what it does; "wm" first,
/// then `│ label ` for each item that fits.
pub fn bar_items(cols: usize) -> Vec<(usize, usize, Bar)> {
    let mut items = alloc::vec![(0, 3.min(cols), Bar::Programs)];
    let mut x = 4;
    for (label, bar) in BAR {
        let width = label.chars().count() + 2;
        if x + 1 + width > cols { break; }
        items.push((x + 1, width, bar));
        x += 1 + width;
    }
    items
}

/// The window manager's state over the desk: modes, the drag of the mouse, the notice on the status line.
pub struct Wm {
    pub desk: Desk,
    pub mode: Mode,
    pub notice: Option<String>,
    drag: Option<Drag>,
    /// The mouse in cells, once it moved.
    pub pointer: Option<(usize, usize)>,
    buttons: u8,
    /// The window whose content got a button's press: it gets the mouse until every button is up.
    grab: Option<u32>,
    /// The desktop menu's programs by category (issue u003), from the boot disk.
    pub programs: Vec<menu::Item>,
}

impl Wm {
    pub fn new(cols: usize, rows: usize) -> Self { Self { desk: Desk::new(cols, rows), mode: Mode::Normal, notice: None, drag: None, pointer: None, buttons: 0, grab: None, programs: Vec::new() } }

    /// A key press (`wm` keys are Alt combinations; others go to the focused window).
    pub fn key(&mut self, key: Key) -> Action {
        match &mut self.mode {
            Mode::Help => { if matches!(key.code(), Code::Esc | Code::Enter | Code::F(1) | Code::F(10)) || key.alt() { self.mode = Mode::Normal; } return Action::Redraw; }
            Mode::Run(line) => {
                return match line.key(key) {
                    Edit::Submit => { let command = String::from(line.as_str().trim()); self.mode = Mode::Normal; if command.is_empty() { Action::Redraw } else { Action::Run(command) } }
                    Edit::Cancel => { self.mode = Mode::Normal; Action::Redraw }
                    _ => Action::Redraw,
                };
            }
            Mode::Menu(open) => {
                return match open.key(&self.programs, key) {
                    menu::Outcome::Stay => Action::Redraw,
                    menu::Outcome::Close => { self.mode = Mode::Normal; Action::Redraw }
                    menu::Outcome::Run(command) => { self.mode = Mode::Normal; Action::Run(command) }
                };
            }
            Mode::Move { id, before, restore } => {
                let (id, before, restore) = (*id, *before, *restore);
                let step = if key.ctrl() { 8 } else { 1 };
                match (key.code(), key.shift()) {
                    (Code::Left, false) => self.desk.move_by(id, -step, 0),
                    (Code::Right, false) => self.desk.move_by(id, step, 0),
                    (Code::Up, false) => self.desk.move_by(id, 0, -step),
                    (Code::Down, false) => self.desk.move_by(id, 0, step),
                    (Code::Left, true) => self.desk.resize_by(id, -step, 0),
                    (Code::Right, true) => self.desk.resize_by(id, step, 0),
                    (Code::Up, true) => self.desk.resize_by(id, 0, -step),
                    (Code::Down, true) => self.desk.resize_by(id, 0, step),
                    (Code::Enter, _) => { self.desk.snap(id); self.mode = Mode::Normal; }
                    (Code::Esc, _) => { self.desk.place(id, before); self.desk.set_restore(id, restore); self.mode = Mode::Normal; }
                    _ => {}
                }
                return Action::Redraw;
            }
            Mode::List { .. } => return self.list_key(key),
            Mode::Normal => {}
        }
        let focus = self.desk.focus();
        if !key.alt() {
            if focus.is_some() { return Action::Forward; }
            // An empty desktop: Enter runs a program, F1 shows the keys.
            return match key.code() {
                Code::Enter => { self.mode = Mode::Run(InputLine::new()); Action::Redraw }
                Code::F(1) => { self.mode = Mode::Help; Action::Redraw }
                _ => Action::Redraw,
            };
        }
        let letter = key.letter();
        match (key.code(), letter) {
            (Code::Tab, _) => { self.desk.cycle(key.shift()); Action::Redraw }
            (Code::Left | Code::Right | Code::Up | Code::Down, _) => {
                let side = match key.code() { Code::Left => 0, Code::Right => 1, Code::Up => 2, _ => 3 };
                if let Some(id) = focus { self.desk.half(id, side); }
                Action::Redraw
            }
            (Code::Enter, _) => { if let Some(id) = focus { self.desk.maximize(id); } Action::Redraw }
            (_, Some('f')) => { if let Some(id) = focus { self.desk.toggle_full(id); } Action::Redraw }
            (_, Some('l')) => { self.open_list(); Action::Redraw }
            (Code::F(4), _) | (_, Some('w')) => focus.map_or(Action::Redraw, Action::Close),
            // Not Alt+F1: fm chooses the left panel's volume with it.
            (_, Some('h')) => { self.mode = Mode::Help; Action::Redraw }
            (_, Some('m')) => { if let Some(w) = self.desk.focused() { self.mode = Mode::Move { id: w.id, before: w.rect, restore: w.restore }; } Action::Redraw }
            (_, Some('r')) => { self.mode = Mode::Run(InputLine::new()); Action::Redraw }
            (_, Some('p')) => { self.mode = Mode::Menu(Menu::new(0, 1)); Action::Redraw }
            (_, Some('q')) => Action::Detach,
            (_, Some('x')) => Action::CloseAll,
            (_, Some(n @ '1'..='4')) => { if let Some(id) = focus { self.desk.quarter(id, n as usize - '0' as usize); } Action::Redraw }
            _ => Action::Forward,
        }
    }

    /// The mouse went to cell (x, y) with `buttons` held (bit 0: left) and the wheel turned `wheel` steps. A press on
    /// a window's content brings it to the front and goes to its program, which then gets the mouse until every
    /// button is up; the wheel goes to the window under the mouse. Moves with no button held are not passed on.
    pub fn pointer(&mut self, x: usize, y: usize, buttons: u8, wheel: i32) -> Action {
        let (x, y) = (x.min(self.desk.cols.saturating_sub(1)), y.min(self.desk.rows.saturating_sub(1)));
        let pressed = buttons & 1 != 0 && self.buttons & 1 == 0;
        let released = buttons & 1 == 0 && self.buttons & 1 != 0;
        let other_pressed = buttons & !1 & !self.buttons != 0;
        self.buttons = buttons;
        self.pointer = Some((x, y));
        if let Some(id) = self.grab {
            if buttons == 0 { self.grab = None; }
            return self.to_window(id, x, y, buttons, wheel).unwrap_or(Action::Redraw);
        }
        if pressed && y == 0 && self.desk.full_screen().is_none() {
            if let Some(&(at, _, item)) = bar_items(self.desk.cols).iter().find(|&&(at, width, _)| x >= at && x < at + width) { return self.bar(item, at); }
        }
        // The keys' help closes on a click too.
        if pressed && matches!(self.mode, Mode::Help) { self.mode = Mode::Normal; return Action::Redraw; }
        // A click on an entry of the window list brings that window to the front; elsewhere it closes the list.
        if let Mode::List { .. } = self.mode {
            if pressed || other_pressed {
                let (inner, list) = (self.list_area(), self.desk.listing());
                if let Some((id, _)) = (y >= inner.y && x >= inner.x && x < inner.right()).then(|| list.get(y - inner.y)).flatten() { self.desk.raise(*id); }
                self.mode = Mode::Normal;
            }
            return Action::Redraw;
        }
        if let Mode::Menu(open) = &mut self.mode {
            let (cols, rows) = (self.desk.cols, self.desk.rows);
            return match open.pointer(&self.programs, cols, rows, x, y, pressed || other_pressed) {
                menu::Outcome::Stay => Action::Redraw,
                menu::Outcome::Close => { self.mode = Mode::Normal; Action::Redraw }
                menu::Outcome::Run(command) => { self.mode = Mode::Normal; Action::Run(command) }
            };
        }
        if (pressed || other_pressed || wheel != 0) && !matches!(self.mode, Mode::Normal) { return Action::Redraw; }
        if pressed || (other_pressed && self.drag.is_none()) {
            return match self.desk.hit(x, y) {
                Hit::Close(id) if pressed => Action::Close(id),
                Hit::Zoom(id) if pressed => { self.desk.raise(id); self.desk.zoom(id); Action::Redraw }
                Hit::Title(id) if pressed => {
                    self.desk.raise(id);
                    let w = self.desk.get(id).unwrap();
                    self.drag = Some(Drag::Move { id, dx: x - w.rect.x, dy: y - w.rect.y, from: Some((x, y)), unsnap: w.restore.is_some() });
                    Action::Redraw
                }
                Hit::Corner(id) if pressed => { self.desk.raise(id); self.desk.forget_restore(id); self.drag = Some(Drag::Resize { id }); Action::Redraw }
                Hit::Content(id) => { self.desk.raise(id); self.grab = Some(id); self.to_window(id, x, y, buttons, wheel).unwrap_or(Action::Redraw) }
                Hit::Border(id) | Hit::Title(id) | Hit::Corner(id) | Hit::Close(id) | Hit::Zoom(id) => { self.desk.raise(id); Action::Redraw }
                // A right click on the desktop: the programs (issue u003).
                Hit::Desktop if buttons & 2 != 0 => { self.mode = Mode::Menu(Menu::new(x, y)); Action::Redraw }
                Hit::Desktop => Action::Redraw,
            };
        }
        match self.drag {
            Some(Drag::Move { id, mut dx, dy, from, unsnap }) => {
                // A click on the title, the mouse not moved: nothing changes.
                if from == Some((x, y)) { if released { self.drag = None; } return Action::Redraw; }
                // A maximized or snapped window dragged by its title leaves the edge with the frame it had before,
                // held at the same share of its width (issue u002).
                if let (true, Some(w)) = (unsnap, self.desk.get(id)) {
                    let (now, old) = (w.rect, w.restore.unwrap_or(w.rect));
                    dx = (dx * old.w / now.w.max(1)).min(old.w.saturating_sub(1));
                    self.desk.place(id, Rect::new(now.x, now.y, old.w, old.h));
                }
                self.drag = Some(Drag::Move { id, dx, dy, from: None, unsnap: false });
                if let Some(w) = self.desk.get(id) { let r = w.rect; self.desk.place(id, Rect::new(x.saturating_sub(dx), y.saturating_sub(dy).max(self.desk.area().y), r.w, r.h)); }
                if released { self.desk.snap(id); self.drag = None; }
                Action::Redraw
            }
            Some(Drag::Resize { id }) => {
                if let Some(w) = self.desk.get(id) { let r = w.rect; self.desk.place(id, Rect::new(r.x, r.y, (x + 1).saturating_sub(r.x), (y + 1).saturating_sub(r.y))); }
                if released { self.drag = None; }
                Action::Redraw
            }
            None => match self.desk.hit(x, y) {
                Hit::Content(id) if wheel != 0 => self.to_window(id, x, y, buttons, wheel).unwrap_or(Action::Redraw),
                _ => Action::Redraw,
            },
        }
    }

    /// A click on item `item` of the top bar, at column `at`: what its key does. It leaves a dialog or mode of wm first;
    /// the programs item or help clicked again closes them.
    pub fn bar(&mut self, item: Bar, at: usize) -> Action {
        let before = core::mem::replace(&mut self.mode, Mode::Normal);
        match item {
            Bar::Programs => { if !matches!(before, Mode::Menu(_)) { self.mode = Mode::Menu(Menu::new(at, 1)); } Action::Redraw }
            Bar::Next => { self.desk.cycle(false); Action::Redraw }
            Bar::Run => { self.mode = Mode::Run(InputLine::new()); Action::Redraw }
            Bar::Move => { if let Some(w) = self.desk.focused() { self.mode = Mode::Move { id: w.id, before: w.rect, restore: w.restore }; } Action::Redraw }
            Bar::Close => self.desk.focus().map_or(Action::Redraw, Action::Close),
            Bar::Help => { if !matches!(before, Mode::Help) { self.mode = Mode::Help; } Action::Redraw }
            Bar::Leave => Action::Detach,
            Bar::List => { if !matches!(before, Mode::List { .. }) { self.open_list(); } Action::Redraw }
            Bar::Full => { if let Some(id) = self.desk.focus() { self.desk.toggle_full(id); } Action::Redraw }
        }
    }

    // Alt+L: the list, with the window in front selected.
    fn open_list(&mut self) {
        let id = self.desk.focus();
        let at = id.and_then(|id| self.desk.listing().iter().position(|(w, _)| *w == id)).unwrap_or(0);
        self.mode = Mode::List { id, at };
    }

    /// The selected entry of the window list: the window chosen, or the one now at its place when it closed.
    pub fn list_selected(&self) -> Option<(usize, u32)> {
        let Mode::List { id, at } = self.mode else { return None };
        let list = self.desk.listing();
        let at = id.and_then(|id| list.iter().position(|(w, _)| *w == id)).unwrap_or(at.min(list.len().saturating_sub(1)));
        list.get(at).map(|(w, _)| (at, *w))
    }

    // Keys in the window list: arrows choose, Enter brings to the front, Alt+W closes, Esc or Alt+L leaves.
    fn list_key(&mut self, key: Key) -> Action {
        let count = self.desk.listing().len();
        let Some((at, id)) = self.list_selected() else {
            if matches!(key.code(), Code::Esc | Code::Enter) || key.letter() == Some('l') { self.mode = Mode::Normal; }
            return Action::Redraw;
        };
        let select = |wm: &mut Wm, at: usize| { let id = wm.desk.listing().get(at).map(|(w, _)| *w); wm.mode = Mode::List { id, at }; };
        match (key.code(), key.letter()) {
            (Code::Up, _) => select(self, at.saturating_sub(1)),
            (Code::Down, _) => select(self, (at + 1).min(count - 1)),
            (Code::Home, _) => select(self, 0),
            (Code::End, _) => select(self, count - 1),
            (Code::Enter, _) => { self.desk.raise(id); self.mode = Mode::Normal; }
            (Code::Esc, _) => self.mode = Mode::Normal,
            (Code::F(4), _) | (_, Some('w')) if key.alt() => { self.mode = Mode::List { id: None, at }; return Action::Close(id); }
            (_, Some('l')) if key.alt() => self.mode = Mode::Normal,
            _ => {}
        }
        Action::Redraw
    }

    // The window list's lines, and the dialog's width.
    fn list_lines(&self) -> (Vec<String>, usize) {
        let lines: Vec<String> = self.desk.listing().iter().map(|(id, state)| {
            let w = self.desk.get(*id).unwrap();
            let title: String = w.title.chars().take(32).collect();
            format!(" {:<32} PID {:<6} {} ", title, w.owner, state)
        }).collect();
        let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0).max(48).min(self.desk.cols.saturating_sub(2));
        (lines, width)
    }

    /// Where the window list's entries are: the first row, x and width (one entry a row).
    pub fn list_area(&self) -> Rect {
        let (lines, width) = self.list_lines();
        let area = Rect::new(0, 0, self.desk.cols, self.desk.rows).centered(width + 2, lines.len().max(1) + 4);
        Rect::new(area.x + 1, area.y + 1, width, lines.len())
    }

    // The event for window `id` at screen cell (x, y), as a cell of its content (the nearest one when outside it).
    fn to_window(&self, id: u32, x: usize, y: usize, buttons: u8, wheel: i32) -> Option<Action> {
        self.desk.get(id)?;
        let inner = self.desk.content(id);
        if inner.w == 0 || inner.h == 0 { return None; }
        let (x, y) = (x.clamp(inner.x, inner.right() - 1) - inner.x, y.clamp(inner.y, inner.bottom() - 1) - inner.y);
        Some(Action::Pointer { id, x, y, buttons, wheel })
    }

    fn mode_name(&self) -> &'static str { match self.mode { Mode::Normal => "NORMAL", Mode::Move { .. } => "MOVE", Mode::Run(_) => "RUN", Mode::Help => "HELP", Mode::Menu(_) => "MENU", Mode::List { .. } => "LIST" } }

    /// Draws everything; returns which window's pixels each cell shows (as `Desk::draw`, without the cells a dialog
    /// covers) and the text cursor (of the run line, or the focused text window's from `cursor`).
    pub fn draw(&mut self, grid: &mut Grid, theme: &Theme, cell: &mut dyn FnMut(u32, usize, usize) -> Option<(char, u32, u32)>, cursor: Option<(usize, usize)>) -> (Vec<u16>, Option<(usize, usize)>) {
        let mut owner = self.desk.draw(grid, theme, cell);
        let cols = grid.cols;
        if let Some(id) = self.desk.full_screen() {
            let inner = self.desk.content(id);
            let mut shown = match self.desk.focused() { Some(w) if w.content == Content::Text => cursor.filter(|&(x, y)| x < inner.w && y < inner.h), _ => None };
            self.dialogs(grid, theme, &mut shown);
            for (index, tag) in owner.iter_mut().enumerate() { if *tag != 0 && grid.get(index % cols, index / cols) != PIXELS { *tag = 0; } }
            return (owner, shown);
        }
        let bar = Style::new(0x101820, 0x80A0C0);
        grid.fill(Rect::new(0, 0, cols, 1), ' ', bar);
        // The items can be clicked (issue u008); the one under the mouse is lit.
        let lit = Style::new(0xFFFFFF, 0x305070);
        for (index, &(at, width, _)) in bar_items(cols).iter().enumerate() {
            let hovered = self.pointer.is_some_and(|(x, y)| y == 0 && x >= at && x < at + width);
            if index == 0 {
                grid.fill(Rect::new(at, 0, width, 1), ' ', if hovered { lit } else { bar });
                grid.text(1, 0, "wm", if hovered { lit } else { Style::new(0x000000, 0x80A0C0) });
            } else {
                grid.put(at - 1, 0, '│', bar);
                grid.text_max(at, 0, &format!(" {} ", BAR[index - 1].0), width, if hovered { lit } else { bar });
            }
        }
        if let Some(w) = self.desk.focused() { let title = format!(" {} ", w.title); grid.text_right(cols, 0, &title, Style::new(0xFFFFFF, 0x305070)); }
        let status_y = grid.rows.saturating_sub(1);
        let status = match (&self.mode, &self.notice) {
            (Mode::Move { .. }, _) => String::from("MOVE: arrows move (Ctrl: 8 cells), Shift+arrows resize; Enter: done (snaps at the edges); Esc: back"),
            (Mode::Menu(_), _) => String::from("PROGRAMS: a click or Enter starts one in a window; arrows move; Esc or a click elsewhere: close"),
            (Mode::List { .. }, _) => String::from("WINDOWS: arrows and Enter or a click bring one to the front; Alt+W closes it; Esc: back"),
            (_, Some(notice)) => notice.clone(),
            _ if self.desk.windows.is_empty() => String::from("No windows. Enter or Alt+R: run a program in a window; F1 or Alt+H: keys; Alt+Q: leave"),
            _ => format!("{} windows; keys go to \"{}\"", self.desk.windows.len(), self.desk.focused().map_or("", |w| w.title.as_str())),
        };
        grid.text_padded(0, status_y, &status, cols, Style::new(0xE0E0E0, 0x000000));
        let mut shown = match self.desk.focused() { Some(w) if w.content == Content::Text => cursor.map(|(x, y)| (w.rect.x + 1 + x, w.rect.y + 1 + y)).filter(|&(x, y)| x + 1 < w.rect.right() && y + 1 < w.rect.bottom()), _ => None };
        self.dialogs(grid, theme, &mut shown);
        // A dialog over a pixel window: its cells are no longer the window's.
        for (index, tag) in owner.iter_mut().enumerate() {
            if *tag != 0 && grid.get(index % cols, index / cols) != PIXELS { *tag = 0; }
        }
        (owner, shown)
    }

    // The keys' help, the run line, the programs or the window list over the windows.
    fn dialogs(&mut self, grid: &mut Grid, theme: &Theme, shown: &mut Option<(usize, usize)>) {
        if let Mode::List { .. } = self.mode {
            let (selected, (lines, width), area) = (self.list_selected(), self.list_lines(), self.list_area());
            let inner = crate::tui::widgets::dialog(grid, "Windows", width + 2, lines.len().max(1) + 4, theme);
            if lines.is_empty() { grid.text(inner.x + 1, inner.y, "No windows", theme.dialog); }
            for (row, line) in lines.iter().enumerate() {
                let style = if selected.is_some_and(|(at, _)| at == row) { theme.selected } else { theme.dialog };
                grid.text_padded(area.x, area.y + row, line, area.w, style);
            }
            grid.text(inner.x + 1, inner.bottom() - 1, "Enter: to the front · Alt+W: close · Esc: back", theme.dialog);
            *shown = None;
            return;
        }
        match &mut self.mode {
            Mode::Help => { message(grid, "wm — keys", &HELP[..HELP.len() - 1], &["OK"], 0, theme); *shown = None; }
            Mode::Run(line) => { *shown = Some(crate::tui::widgets::input_dialog(grid, "Run in a window", "Program and arguments:", line, 60, theme)); }
            Mode::Menu(open) => { open.draw(&self.programs, grid, theme); *shown = None; }
            _ => {}
        }
    }

    /// The state line after each event.
    pub fn status(&self) -> String {
        let pointer = self.pointer.map_or(String::new(), |(x, y)| format!(" POINTER={},{}", x, y));
        let menu = match &self.mode {
            Mode::Menu(open) => format!(" MENU={}", open.path(&self.programs)),
            Mode::List { .. } => format!(" LIST={}", self.list_selected().map_or(String::from("-"), |(_, id)| format!("{}", id))),
            _ => String::new(),
        };
        format!("MODE={} {}{}{}", self.mode_name(), self.desk.status(), pointer, menu)
    }
}
