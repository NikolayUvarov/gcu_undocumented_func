//! `wm`'s Settings (000-APP-0048): one window for what can be configured, opened from the top bar or with Alt+S. Its
//! first page is the desktop background (000-APP-0047), kept in `data/wm.conf`; the system's pages say where each
//! setting is made until the shell's channel (211-APP-0044) lets them act from here. Host-tested in tests/wm_host.rs.
use crate::background::{Config, Picture, Place};
use crate::keys::{Code, Key};
use crate::tui::widgets::{dialog, Edit, InputLine};
use crate::tui::{Grid, Rect, Theme};
use alloc::{format, string::String, vec::Vec};

pub const PAGES: [&str; 6] = ["Background", "Keyboard", "Date and time", "Network", "Sound", "Screen"];
/// The background page's rows.
pub const ROWS: [&str; 7] = ["Picture", "Image file", "Time", "Date", "CPU load", "Network", "Place"];
const PLACES: [Place; 5] = [Place::TopLeft, Place::TopRight, Place::Center, Place::BottomLeft, Place::BottomRight];
const WIDTH: usize = 84;
const HEIGHT: usize = 16;
const LIST: usize = 17; // the pages' column

/// The window's state: the page, the row on it, whether the keys move in the pages' list, and the image's file as typed.
pub struct Settings { pub page: usize, pub row: usize, pub on_pages: bool, pub file: InputLine, screen: (usize, usize) }

/// What a key or a click did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome { Stay, Close, Changed(Config) }

impl Settings {
    /// For a screen of `screen` cells; the image file starts as the configuration names it.
    pub fn new(config: &Config, screen: (usize, usize)) -> Self {
        let mut file = InputLine::new();
        if let Picture::Image(name) = &config.picture { for ch in name.chars() { file.insert(ch); } }
        Self { page: 0, row: 0, on_pages: true, file, screen }
    }

    // The configuration with row `row` of the background page changed by `step` (+1, -1; 0: toggled or cycled on).
    fn change(&self, config: &Config, row: usize, step: i32) -> Option<Config> {
        let mut c = config.clone();
        let file = String::from(self.file.as_str().trim());
        match row {
            0 => {
                let kinds = [Picture::None, Picture::Abstract, Picture::Image(file.clone())];
                let at = match c.picture { Picture::None => 0, Picture::Abstract => 1, Picture::Image(_) => 2 };
                c.picture = kinds[(at as i32 + if step < 0 { 2 } else { 1 }) as usize % 3].clone();
                if matches!(c.picture, Picture::Image(ref f) if f.is_empty()) { c.picture = Picture::Image(String::from("data/background.bmp")); }
            }
            1 => { if file.is_empty() { return None; } c.picture = Picture::Image(file); }
            2 => c.time = !c.time,
            3 => c.date = !c.date,
            4 => c.cpu = !c.cpu,
            5 => c.net = !c.net,
            6 => {
                let at = PLACES.iter().position(|p| *p == c.place).unwrap_or(4);
                c.place = PLACES[(at as i32 + if step < 0 { 4 } else { 1 }) as usize % 5];
            }
            _ => return None,
        }
        (c != *config).then_some(c)
    }

    /// A key: arrows move (Tab between the pages and the page), Left/Right, Space or Enter change the row, typing
    /// edits the image's file on its row (Enter takes it), Esc or Alt+S closes.
    pub fn key(&mut self, key: Key, config: &Config) -> Outcome {
        if key.code() == Code::Esc || (key.alt() && key.letter() == Some('s')) { return Outcome::Close; }
        if key.code() == Code::Tab { self.on_pages = !self.on_pages || self.page != 0; return Outcome::Stay; }
        if self.on_pages {
            match key.code() {
                Code::Up => self.page = self.page.saturating_sub(1),
                Code::Down => self.page = (self.page + 1).min(PAGES.len() - 1),
                Code::Right | Code::Enter if self.page == 0 => self.on_pages = false,
                _ => {}
            }
            return Outcome::Stay;
        }
        match key.code() {
            Code::Up => { self.row = self.row.saturating_sub(1); Outcome::Stay }
            Code::Down => { self.row = (self.row + 1).min(ROWS.len() - 1); Outcome::Stay }
            _ if self.row == 1 => match self.file.key(key) {
                Edit::Submit => self.change(config, 1, 0).map_or(Outcome::Stay, Outcome::Changed),
                _ => Outcome::Stay,
            },
            Code::Left if self.row == 0 || self.row == 6 => self.change(config, self.row, -1).map_or(Outcome::Stay, Outcome::Changed),
            Code::Left => { self.on_pages = true; Outcome::Stay }
            Code::Right | Code::Enter => self.change(config, self.row, 1).map_or(Outcome::Stay, Outcome::Changed),
            Code::Char if key.char() == Some(' ') => self.change(config, self.row, 1).map_or(Outcome::Stay, Outcome::Changed),
            _ => Outcome::Stay,
        }
    }

    /// The window's frame on a screen of the given cells.
    pub fn area(&self) -> Rect { Rect::new(0, 0, self.screen.0, self.screen.1).centered(WIDTH, HEIGHT) }

    /// A click at cell (x, y): a page's name opens it, a row of the background page is chosen and changed, a click
    /// outside the window closes it.
    pub fn click(&mut self, x: usize, y: usize, config: &Config) -> Outcome {
        let inner = self.area().inner();
        if x < inner.x || x >= inner.right() || y < inner.y || y >= inner.bottom() { return Outcome::Close; }
        let (cx, cy) = (x - inner.x, y - inner.y);
        if cx < LIST {
            if cy < PAGES.len() { self.page = cy; self.on_pages = true; }
            return Outcome::Stay;
        }
        if self.page == 0 && (1..=ROWS.len()).contains(&cy) {
            self.row = cy - 1;
            self.on_pages = false;
            if self.row != 1 { return self.change(config, self.row, 1).map_or(Outcome::Stay, Outcome::Changed); }
        }
        Outcome::Stay
    }

    /// The background page's lines as shown: each row's name and value.
    pub fn rows(&self, config: &Config) -> Vec<(&'static str, String)> {
        let check = |on: bool| String::from(if on { "[x]" } else { "[ ]" });
        let picture = match config.picture { Picture::None => "none (the ░ desktop)", Picture::Abstract => "abstract (slowly moving)", Picture::Image(_) => "image" };
        let place = match config.place { Place::TopLeft => "top left", Place::TopRight => "top right", Place::Center => "centre", Place::BottomLeft => "bottom left", Place::BottomRight => "bottom right" };
        Vec::from([(ROWS[0], format!("< {} >", picture)), (ROWS[1], String::from(self.file.as_str())), (ROWS[2], check(config.time)), (ROWS[3], check(config.date)),
                   (ROWS[4], check(config.cpu)), (ROWS[5], format!("{} (no counters yet)", check(config.net).as_str())), (ROWS[6], format!("< {} >", place))])
    }

    /// Draws the window; returns where the text cursor is (on the image file's row while it is chosen).
    pub fn draw(&self, grid: &mut Grid, theme: &Theme, config: &Config) -> Option<(usize, usize)> {
        let inner = dialog(grid, "Settings", WIDTH, HEIGHT, theme);
        for (i, name) in PAGES.iter().enumerate() {
            let style = if i == self.page { if self.on_pages { theme.selected } else { theme.accent } } else { theme.dialog };
            grid.text_padded(inner.x, inner.y + i, &format!(" {}", name), LIST - 1, style);
        }
        for y in inner.y..inner.bottom() { grid.put(inner.x + LIST - 1, y, '│', theme.dialog); }
        let (x, w) = (inner.x + LIST + 1, inner.w.saturating_sub(LIST + 2));
        let mut cursor = None;
        if self.page == 0 {
            grid.text(x, inner.y, "The desktop background, kept in data/wm.conf:", theme.dialog);
            for (i, (name, value)) in self.rows(config).iter().enumerate() {
                let chosen = !self.on_pages && self.row == i;
                let line = format!("{:<11} {}", name, value);
                grid.text_padded(x, inner.y + 1 + i, &line, w, if chosen { theme.selected } else { theme.dialog });
                if chosen && i == 1 { cursor = Some((x + 12 + self.file.cursor_chars(), inner.y + 1 + i)); }
            }
        } else {
            for (i, line) in self.page_text().iter().enumerate() { grid.text_max(x, inner.y + i, line, w, theme.dialog); }
        }
        let help = if self.on_pages { "↑↓: page · Enter or →: its settings · Esc: close" } else if self.row == 1 { "type the file · Enter: use it · ↑↓: rows · Esc: close" } else { "↑↓: rows · ← → Space: change · Tab: pages · Esc: close" };
        grid.text_max(inner.x + 1, inner.bottom() - 1, help, inner.w - 2, theme.dialog);
        cursor
    }

    // The system's pages: where the setting is made today.
    fn page_text(&self) -> Vec<String> {
        let later = "From here once the shell's channel is in (211-APP-0044).";
        let lines: Vec<String> = match self.page {
            1 => ["The keyboard's layout and its switch:", "  keymap us|ru [--switch both|ctrl-shift|alt-shift|caps|none]", "in the shell's window (Ctrl+Alt+F5) or on its screen.", "", later].iter().map(|s| String::from(*s)).collect(),
            2 => ["The date and the time (the clock keeps no time zone):", "  date set YYYY-MM-DD HH:MM[:SS]", "in the shell's window (Ctrl+Alt+F5) or on its screen.", "", later].iter().map(|s| String::from(*s)).collect(),
            3 => ["The network: addresses, flow grants and the policy:", "  ip, netgrants, netrevoke <program>, netpolicy", "in the shell's window (Ctrl+Alt+F5) or on its screen.", "", later].iter().map(|s| String::from(*s)).collect(),
            4 => ["The sound has no volume to set yet: audio.wit has none", "(asked of the drivers track for this page).", "beep and say play at the level they make."].iter().map(|s| String::from(*s)).collect(),
            _ => Vec::from([format!("The screen: {} × {} cells of 8 × 16 pixels,", self.screen.0, self.screen.1), String::from("in the mode the firmware set at boot."), String::from("It is not changed from here yet.")]),
        };
        lines
    }
}
