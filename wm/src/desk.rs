//! The desktop of `wm` (issue 088): windows on the 8×16 cell grid — a frame with the title, a close mark and a resize
//! corner — stacked bottom to top with the top one focused; moving and resizing by keys and by the mouse; snapping to
//! halves, quarters and the whole screen; what each cell of the screen shows. No system calls: tests/wm_host.rs.
use crate::keys::{Code, Key};
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
}

impl Win {
    pub fn new(id: u32, owner: u64, content: Content, size: (usize, usize), title: &str) -> Self {
        Self { id, owner, content, size, title: String::from(title), rect: Rect::default(), restore: None, before_max: None }
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
        self.forget_restore(id);
        let Some(w) = self.get(id) else { return };
        let r = w.rect;
        self.set(id, Rect::new(r.x.saturating_add_signed(dx), r.y.saturating_add_signed(dy).max(self.area().y), r.w, r.h));
    }
    pub fn resize_by(&mut self, id: u32, dw: isize, dh: isize) {
        self.forget_restore(id);
        let Some(w) = self.get(id) else { return };
        let r = w.rect;
        let area = self.area();
        let (width, height) = (r.w.saturating_add_signed(dw).min(area.right() - r.x), r.h.saturating_add_signed(dh).min(area.bottom().saturating_sub(r.y)));
        self.set(id, Rect::new(r.x, r.y, width, height));
    }
    /// Moves a window's frame to `rect` (kept inside the area), as a hand does: it is no longer snapped.
    pub fn place(&mut self, id: u32, rect: Rect) { self.forget_restore(id); self.set(id, rect); }

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
    pub fn half(&mut self, id: u32, side: usize) { let r = self.half_rect(side); self.keep_restore(id); self.set(id, r); }
    pub fn quarter(&mut self, id: u32, n: usize) { let r = self.quarter_rect(n); self.keep_restore(id); self.set(id, r); }

    /// Alt+Enter: the whole area, or back to where it was maximized from.
    pub fn maximize(&mut self, id: u32) {
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
        format!("FOCUS={} WINDOWS={}", self.focus().map_or(String::from("-"), |id| format!("{}", id)), windows.join(" "))
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

pub enum Mode { Normal, Move { id: u32, before: Rect, restore: Option<Rect> }, Run(InputLine), Help }

pub const HELP: [&str; 14] = [
    "Alt+Tab, Alt+Shift+Tab — the next window, the previous one",
    "Alt+← → ↑ ↓ — half the screen; Alt+1…4 — a quarter; Alt+Enter — maximize or restore",
    "Alt+M — move and resize: arrows move, Shift+arrows resize, Enter ends (at an edge it snaps), Esc goes back",
    "Alt+W or Alt+F4 — close the window (its program ends)",
    "Alt+R — run a program in a new window: fm, top, clock, dzen-clock, edit <file>, …",
    "Alt+Q — leave wm: the programs keep running, the next wm shows them where they were",
    "Alt+X — close every window and leave",
    "Mouse: a click brings a window to the front and goes to its program, as the wheel does;",
    "  drag the title to move a window (it snaps at the edges; a snapped one gets its size back),",
    "  the ◆ corner to resize it; [▲] maximizes, [⇕] gives the size back; [×] closes",
    "Every other key goes to the window in front only.",
    "Programs started here get only what wm holds and they ask for: the user's files,",
    "  system information, a window; nothing else.",
    "",
];

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
}

impl Wm {
    pub fn new(cols: usize, rows: usize) -> Self { Self { desk: Desk::new(cols, rows), mode: Mode::Normal, notice: None, drag: None, pointer: None, buttons: 0, grab: None } }

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
            (Code::F(4), _) | (_, Some('w')) => focus.map_or(Action::Redraw, Action::Close),
            // Not Alt+F1: fm chooses the left panel's volume with it.
            (_, Some('h')) => { self.mode = Mode::Help; Action::Redraw }
            (_, Some('m')) => { if let Some(w) = self.desk.focused() { self.mode = Mode::Move { id: w.id, before: w.rect, restore: w.restore }; } Action::Redraw }
            (_, Some('r')) => { self.mode = Mode::Run(InputLine::new()); Action::Redraw }
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

    // The event for window `id` at screen cell (x, y), as a cell of its content (the nearest one when outside it).
    fn to_window(&self, id: u32, x: usize, y: usize, buttons: u8, wheel: i32) -> Option<Action> {
        let inner = self.desk.get(id)?.rect.inner();
        if inner.w == 0 || inner.h == 0 { return None; }
        let (x, y) = (x.clamp(inner.x, inner.right() - 1) - inner.x, y.clamp(inner.y, inner.bottom() - 1) - inner.y);
        Some(Action::Pointer { id, x, y, buttons, wheel })
    }

    fn mode_name(&self) -> &'static str { match self.mode { Mode::Normal => "NORMAL", Mode::Move { .. } => "MOVE", Mode::Run(_) => "RUN", Mode::Help => "HELP" } }

    /// Draws everything; returns which window's pixels each cell shows (as `Desk::draw`, without the cells a dialog
    /// covers) and the text cursor (of the run line, or the focused text window's from `cursor`).
    pub fn draw(&mut self, grid: &mut Grid, theme: &Theme, cell: &mut dyn FnMut(u32, usize, usize) -> Option<(char, u32, u32)>, cursor: Option<(usize, usize)>) -> (Vec<u16>, Option<(usize, usize)>) {
        let mut owner = self.desk.draw(grid, theme, cell);
        let cols = grid.cols;
        let bar = Style::new(0x101820, 0x80A0C0);
        grid.fill(Rect::new(0, 0, cols, 1), ' ', bar);
        grid.text(1, 0, "wm", Style::new(0x000000, 0x80A0C0));
        grid.text_max(4, 0, "│ Alt+Tab next │ Alt+R run │ Alt+M move │ Alt+W close │ Alt+H help │ Alt+Q leave", cols.saturating_sub(4), bar);
        if let Some(w) = self.desk.focused() { let title = format!(" {} ", w.title); grid.text_right(cols, 0, &title, Style::new(0xFFFFFF, 0x305070)); }
        let status_y = grid.rows.saturating_sub(1);
        let status = match (&self.mode, &self.notice) {
            (Mode::Move { .. }, _) => String::from("MOVE: arrows move (Ctrl: 8 cells), Shift+arrows resize; Enter: done (snaps at the edges); Esc: back"),
            (_, Some(notice)) => notice.clone(),
            _ if self.desk.windows.is_empty() => String::from("No windows. Enter or Alt+R: run a program in a window; F1 or Alt+H: keys; Alt+Q: leave"),
            _ => format!("{} windows; keys go to \"{}\"", self.desk.windows.len(), self.desk.focused().map_or("", |w| w.title.as_str())),
        };
        grid.text_padded(0, status_y, &status, cols, Style::new(0xE0E0E0, 0x000000));
        let mut shown = match self.desk.focused() { Some(w) if w.content == Content::Text => cursor.map(|(x, y)| (w.rect.x + 1 + x, w.rect.y + 1 + y)).filter(|&(x, y)| x + 1 < w.rect.right() && y + 1 < w.rect.bottom()), _ => None };
        match &mut self.mode {
            Mode::Help => { message(grid, "wm — keys", &HELP[..HELP.len() - 1], &["OK"], 0, theme); shown = None; }
            Mode::Run(line) => { shown = Some(crate::tui::widgets::input_dialog(grid, "Run in a window", "Program and arguments:", line, 60, theme)); }
            _ => {}
        }
        // A dialog over a pixel window: its cells are no longer the window's.
        for (index, tag) in owner.iter_mut().enumerate() {
            if *tag != 0 && grid.get(index % cols, index / cols) != PIXELS { *tag = 0; }
        }
        (owner, shown)
    }

    /// The state line after each event.
    pub fn status(&self) -> String {
        let pointer = self.pointer.map_or(String::new(), |(x, y)| format!(" POINTER={},{}", x, y));
        format!("MODE={} {}{}", self.mode_name(), self.desk.status(), pointer)
    }
}
