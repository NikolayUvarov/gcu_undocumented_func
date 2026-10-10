//! `wm`'s Settings (000-APP-0048): one window for what can be configured, opened from the top bar or with Alt+S. Its
//! first page is the desktop background (000-APP-0047, 000-APP-0050), kept in `data/wm.conf`. The date and time page
//! sets the clock through the shell, which asks the user in its own window first (000-APP-0055). The other system pages
//! are to read and say where each setting is made. Every row and page answers Enter and a click: it changes, or a line
//! says why not (000-APP-0056). Host-tested in tests/wm_host.rs.
use crate::background::{Config, Picture, Place, COMPLEXITY, PATTERNS};
use crate::keys::{Code, Key};
use crate::tui::widgets::{dialog, Edit, InputLine};
use crate::tui::{Grid, Rect, Theme};
use alloc::{format, string::String, vec::Vec};

pub const PAGES: [&str; 6] = ["Background", "Keyboard", "Date and time", "Network", "Sound", "Screen"];
/// The background page's rows.
pub const ROWS: [&str; 12] = ["Picture", "Image file", "Pattern", "Speed", "Contrast", "Complexity", "Brightness", "Time", "Date", "CPU load", "Network", "Place"];
// The rows ← and → step through (the others: the file typed, and boxes ticked).
const STEPPED: [usize; 7] = [0, 2, 3, 4, 5, 6, 11];
const FILE: usize = 1;
const PLACES: [Place; 5] = [Place::TopLeft, Place::TopRight, Place::Center, Place::BottomLeft, Place::BottomRight];
// The speeds offered; the percentages go in fives.
const SPEEDS: [u8; 10] = [1, 2, 3, 5, 8, 10, 15, 20, 30, 50];
/// The date and time page, and its rows: the fields, then the button.
pub const DATE: usize = 2;
pub const CLOCK_ROWS: [&str; 7] = ["Year", "Month", "Day", "Hours", "Minutes", "Seconds", "Set the clock"];
const SET: usize = 6;
// Each field's range and digits.
const FIELDS: [(u32, u32, u32); 6] = [(2000, 2099, 4), (1, 12, 2), (1, 31, 2), (0, 23, 2), (0, 59, 2), (0, 59, 2)];
const WIDTH: usize = 84;
const HEIGHT: usize = 17;
const LIST: usize = 17; // the pages' column

/// The window's state: the page, the row on it, whether the keys move in the pages' list, and the image's file as typed.
/// On the date and time page: the clock as last read (`now`: year, month, day; seconds since midnight), the fields
/// being set, whether the user changed them, digits being typed, its row, and whether `wm` holds the shell's commands
/// (`shell`), through which the clock is set. `message`: a line about the last try on any page.
pub struct Settings {
    pub page: usize, pub row: usize, pub on_pages: bool, pub file: InputLine, screen: (usize, usize),
    pub now: Option<((u32, u32, u32), u32)>, pub fields: [u32; 6], pub edited: bool, typed: Option<(u32, u32)>, pub clock_row: usize,
    pub message: Option<String>, pub shell: bool,
}

/// What a key or a click did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome { Stay, Close, Changed(Config), SetClock { date: u32, seconds: u32 } }

/// Days since 2000-01-01 of a date from 2000 to 2099; None for a day the month does not have.
pub fn days(year: u32, month: u32, day: u32) -> Option<u32> {
    if !(2000..=2099).contains(&year) || !(1..=12).contains(&month) { return None; }
    let leap = year % 4 == 0; // 2000 is one too, and 2100 is past the range
    let lengths = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if day == 0 || day > lengths[month as usize - 1] { return None; }
    let years = year - 2000;
    let before = years * 365 + years.div_ceil(4);
    Some(before + lengths[..month as usize - 1].iter().sum::<u32>() + day - 1)
}

impl Settings {
    /// For a screen of `screen` cells; the image file starts as the configuration names it.
    pub fn new(config: &Config, screen: (usize, usize)) -> Self {
        let mut file = InputLine::new();
        if let Picture::Image(name) = &config.picture { for ch in name.chars() { file.insert(ch); } }
        Self { page: 0, row: 0, on_pages: true, file, screen, now: None, fields: [2026, 1, 1, 0, 0, 0], edited: false, typed: None, clock_row: 0, message: None, shell: false }
    }

    /// The clock as `wm` read it: the fields follow it until the user changes one.
    pub fn set_now(&mut self, date: Option<(u32, u32, u32)>, seconds: Option<usize>) {
        self.now = date.zip(seconds).map(|(d, s)| (d, s as u32));
        if let (Some(((y, mo, d), s)), false) = (self.now, self.edited) { self.fields = [y, mo, d, s / 3600, s / 60 % 60, s % 60]; }
    }

    // A key on the date and time page.
    fn clock_key(&mut self, key: Key) -> Outcome {
        let row = self.clock_row;
        match key.code() {
            Code::Up => { self.clock_row = row.saturating_sub(1); self.typed = None; }
            Code::Down => { self.clock_row = (row + 1).min(SET); self.typed = None; }
            Code::Left | Code::Right if row < SET => {
                let (lo, hi, _) = FIELDS[row];
                let v = self.fields[row];
                self.fields[row] = if key.code() == Code::Right { if v >= hi { lo } else { v + 1 } } else if v <= lo { hi } else { v - 1 };
                (self.edited, self.typed, self.message) = (true, None, None);
            }
            Code::Char if row < SET && key.char().is_some_and(|c| c.is_ascii_digit()) => {
                // Digits replace the field as they are typed; the field's last digit ends it.
                let digit = key.char().and_then(|c| c.to_digit(10)).unwrap_or(0);
                let (value, count) = self.typed.unwrap_or((0, 0));
                let (value, count) = (value * 10 + digit, count + 1);
                self.fields[row] = value;
                self.typed = (count < FIELDS[row].2).then_some((value, count));
                (self.edited, self.message) = (true, None);
            }
            Code::Enter => return self.set(),
            Code::Char if key.char() == Some(' ') => return self.set(),
            _ => {}
        }
        Outcome::Stay
    }

    // Enter: the clock set to the fields, once they make a time and the shell can be asked.
    fn set(&mut self) -> Outcome {
        self.typed = None;
        let [year, month, day, h, m, s] = self.fields;
        let Some(date) = days(year, month, day).filter(|_| h < 24 && m < 60 && s < 60) else {
            self.message = Some(format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} is not a time the clock takes", year, month, day, h, m, s));
            return Outcome::Stay;
        };
        if !self.shell {
            self.message = Some(String::from("wm holds no shell's commands, so it cannot set the clock: date set in the shell's window"));
            return Outcome::Stay;
        }
        self.message = Some(format!("Asked the shell to set {:04}-{:02}-{:02} {:02}:{:02}:{:02}: answer it in its window", year, month, day, h, m, s));
        self.edited = false;
        Outcome::SetClock { date, seconds: h * 3600 + m * 60 + s }
    }

    // The configuration with row `row` of the background page changed by `step`: +1 or -1 (numbers stop at their
    // ends), 0 toggled or cycled on (numbers back to the first after the last).
    fn change(&self, config: &Config, row: usize, step: i32) -> Option<Config> {
        let mut c = config.clone();
        let file = String::from(self.file.as_str().trim());
        let cycle = |at: usize, len: usize| (at + if step < 0 { len - 1 } else { 1 }) % len;
        let fives: Vec<u8> = (0..=100).step_by(5).collect();
        let complexities: Vec<u8> = (COMPLEXITY.0..=COMPLEXITY.1).collect();
        match row {
            0 => {
                let kinds = [Picture::None, Picture::Abstract, Picture::Image(file.clone())];
                let at = match c.picture { Picture::None => 0, Picture::Abstract => 1, Picture::Image(_) => 2 };
                c.picture = kinds[cycle(at, 3)].clone();
                if matches!(c.picture, Picture::Image(ref f) if f.is_empty()) { c.picture = Picture::Image(String::from("data/background.bmp")); }
            }
            FILE => { if file.is_empty() { return None; } c.picture = Picture::Image(file); }
            2 => c.pattern = PATTERNS[cycle(PATTERNS.iter().position(|p| p.1 == c.pattern).unwrap_or(0), PATTERNS.len())].1,
            3 => c.speed = next(&SPEEDS, c.speed, step),
            4 => c.contrast = next(&fives, c.contrast, step),
            5 => c.complexity = next(&complexities, c.complexity, step),
            6 => c.info = next(&fives, c.info, step),
            7 => c.time = !c.time,
            8 => c.date = !c.date,
            9 => c.cpu = !c.cpu,
            10 => c.net = !c.net,
            11 => c.place = PLACES[cycle(PLACES.iter().position(|p| *p == c.place).unwrap_or(4), PLACES.len())],
            _ => return None,
        }
        (c != *config).then_some(c)
    }

    /// A key: arrows move (Tab between the pages and the page), Left/Right step the row's value, Space or Enter
    /// change it on (and tick a box), typing edits the image's file on its row (Enter takes it), Esc or Alt+S closes.
    pub fn key(&mut self, key: Key, config: &Config) -> Outcome {
        if key.code() == Code::Esc || (key.alt() && key.letter() == Some('s')) { return Outcome::Close; }
        let enterable = self.page == 0 || self.page == DATE;
        if key.code() == Code::Tab { self.on_pages = !self.on_pages || !enterable; return Outcome::Stay; }
        if self.on_pages {
            match key.code() {
                Code::Up => self.open(self.page.saturating_sub(1)),
                Code::Down => self.open((self.page + 1).min(PAGES.len() - 1)),
                Code::Right | Code::Enter if enterable => self.on_pages = false,
                Code::Right | Code::Enter => self.to_read(),
                _ => {}
            }
            return Outcome::Stay;
        }
        if self.page == DATE { return self.clock_key(key); }
        match key.code() {
            Code::Up => { (self.row, self.message) = (self.row.saturating_sub(1), None); Outcome::Stay }
            Code::Down => { (self.row, self.message) = ((self.row + 1).min(ROWS.len() - 1), None); Outcome::Stay }
            _ if self.row == FILE => match self.file.key(key) {
                Edit::Submit => self.changed(config, FILE, 0),
                _ => Outcome::Stay,
            },
            Code::Left if STEPPED.contains(&self.row) => self.changed(config, self.row, -1),
            Code::Left => { self.on_pages = true; Outcome::Stay }
            Code::Right => self.changed(config, self.row, 1),
            Code::Enter => self.changed(config, self.row, 0),
            Code::Char if key.char() == Some(' ') => self.changed(config, self.row, 0),
            _ => Outcome::Stay,
        }
    }

    // Page `page` shown, its line about the last try gone.
    fn open(&mut self, page: usize) {
        if page != self.page { self.message = None; }
        self.page = page;
    }

    // Enter or a click on a page that is only to read: it says so.
    fn to_read(&mut self) {
        self.message = Some(format!("{}: a page to read; nothing is set here yet", PAGES[self.page]));
    }

    // Row `row` changed by `step`, or a line saying why it stays.
    fn changed(&mut self, config: &Config, row: usize, step: i32) -> Outcome {
        match self.change(config, row, step) {
            Some(c) => { self.message = None; Outcome::Changed(c) }
            None => { self.message = Some(self.why(config, row, step)); Outcome::Stay }
        }
    }

    // Why row `row` did not change.
    fn why(&self, config: &Config, row: usize, step: i32) -> String {
        let file = self.file.as_str().trim();
        let value = match row {
            3 => format!("{}", config.speed),
            4 => format!("{}%", config.contrast),
            5 => format!("{}", config.complexity),
            6 => format!("{}%", config.info),
            _ => String::new(),
        };
        match row {
            FILE if file.is_empty() => String::from("Type the image's file here (BMP, PNG or JPEG), then Enter"),
            FILE => format!("{} is the picture already", file),
            3..=6 => format!("{} is at its {} already: {}", ROWS[row], if step < 0 { "lowest" } else { "highest" }, value),
            _ => format!("{} stays as it is", ROWS[row]),
        }
    }

    /// The window's frame on a screen of the given cells.
    pub fn area(&self) -> Rect { Rect::new(0, 0, self.screen.0, self.screen.1).centered(WIDTH, HEIGHT) }

    /// A click at cell (x, y): a page's name opens it, a row of the background page is chosen and changed, a click on
    /// a page to read says so, a click outside the window closes it.
    pub fn click(&mut self, x: usize, y: usize, config: &Config) -> Outcome {
        let inner = self.area().inner();
        if x < inner.x || x >= inner.right() || y < inner.y || y >= inner.bottom() { return Outcome::Close; }
        let (cx, cy) = (x - inner.x, y - inner.y);
        if cx < LIST {
            if cy < PAGES.len() { self.open(cy); self.on_pages = true; }
            return Outcome::Stay;
        }
        if self.page == 0 && (1..=ROWS.len()).contains(&cy) {
            (self.row, self.on_pages, self.message) = (cy - 1, false, None);
            if self.row != FILE { return self.changed(config, self.row, 0); }
        }
        // A page to read says so.
        if self.page != 0 && self.page != DATE { self.to_read(); }
        // The date page: a field is chosen (← → change it), the button sets the clock.
        if self.page == DATE && (1..=CLOCK_ROWS.len()).contains(&cy) {
            (self.clock_row, self.on_pages, self.typed) = (cy - 1, false, None);
            if self.clock_row == SET { return self.set(); }
        }
        Outcome::Stay
    }

    /// The background page's lines as shown: each row's name and value.
    pub fn rows(&self, config: &Config) -> Vec<(&'static str, String)> {
        let check = |on: bool| String::from(if on { "[x]" } else { "[ ]" });
        let picture = match config.picture { Picture::None => "none (the ░ desktop)", Picture::Abstract => "abstract (a moving pattern)", Picture::Image(_) => "image (BMP, PNG or JPEG)" };
        let pattern = PATTERNS.iter().find(|p| p.1 == config.pattern).map_or("waves", |p| p.0);
        let place = match config.place { Place::TopLeft => "top left", Place::TopRight => "top right", Place::Center => "centre", Place::BottomLeft => "bottom left", Place::BottomRight => "bottom right" };
        Vec::from([(ROWS[0], format!("< {} >", picture)), (ROWS[1], String::from(self.file.as_str())), (ROWS[2], format!("< {} >", pattern)),
                   (ROWS[3], format!("< {} >  1 slow (for work) to 50 fast (for a show)", config.speed)), (ROWS[4], format!("< {}% >", config.contrast)),
                   (ROWS[5], format!("< {} >  1 to 5: more waves, rings, curtains, blobs", config.complexity)),
                   (ROWS[6], format!("< {}% >  of the time, the date and the CPU load", config.info)),
                   (ROWS[7], check(config.time)), (ROWS[8], check(config.date)), (ROWS[9], check(config.cpu)),
                   (ROWS[10], format!("{} (no counters yet)", check(config.net).as_str())), (ROWS[11], format!("< {} >", place))])
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
                if chosen && i == FILE { cursor = Some((x + 12 + self.file.cursor_chars(), inner.y + 1 + i)); }
            }
            if let Some(message) = &self.message { grid.text_max(x, inner.y + 1 + ROWS.len(), message, w, theme.accent); }
        } else if self.page == DATE {
            let now = match self.now { Some(((y, mo, d), s)) => format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, s / 3600, s / 60 % 60, s % 60), None => String::from("cannot be read") };
            grid.text_max(x, inner.y, &format!("The clock (it keeps no time zone): {}", now), w, theme.dialog);
            for (i, name) in CLOCK_ROWS.iter().enumerate() {
                let chosen = !self.on_pages && self.clock_row == i;
                let line = if i == SET { format!("[ {} ]  the shell asks you to agree in its window", name) } else {
                    format!("{:<11} < {:0width$} >", name, self.fields[i], width = FIELDS[i].2 as usize)
                };
                grid.text_padded(x, inner.y + 1 + i, &line, w, if chosen { theme.selected } else { theme.dialog });
            }
            if let Some(message) = &self.message { grid.text_max(x, inner.y + 2 + CLOCK_ROWS.len(), message, w, theme.accent); }
        } else {
            let lines = self.page_text();
            for (i, line) in lines.iter().enumerate() { grid.text_max(x, inner.y + i, line, w, if i == 0 { theme.accent } else { theme.dialog }); }
            if let Some(message) = &self.message { grid.text_max(x, inner.y + 1 + lines.len(), message, w, theme.accent); }
        }
        let help = if self.on_pages && self.page != 0 && self.page != DATE { "↑↓: page · this page is to read · Esc: close" }
            else if self.on_pages { "↑↓: page · Enter or →: its settings · Esc: close" } else if self.page == DATE { "↑↓: rows · ← → or digits: change · Enter: set the clock · Tab: pages · Esc: close" }
            else if self.row == FILE { "type the file · Enter: use it · ↑↓: rows · Esc: close" } else { "↑↓: rows · ← → Space: change · Tab: pages · Esc: close" };
        grid.text_max(inner.x + 1, inner.bottom() - 1, help, inner.w - 2, theme.dialog);
        cursor
    }

    /// The pages to read: a first line that says so, then where the setting is made today.
    pub fn page_text(&self) -> Vec<String> {
        let shell = ["in the shell's window, which Shell in the menu opens", "(a right click on the desktop, Alt+P), as Ctrl+Alt+F5 does."];
        let lines: Vec<&str> = match self.page {
            1 => Vec::from(["To read: the keyboard is set in the shell.", "The layout and its switch:", "  keymap us|ru [--switch both|ctrl-shift|alt-shift|caps|none]", shell[0], shell[1]]),
            3 => Vec::from(["To read: the network is set in the shell.", "Addresses, flow grants and the policy:", "  ip, netgrants, netrevoke <program>, netpolicy", shell[0], shell[1]]),
            4 => Vec::from(["To read: the sound has no volume to set yet.", "audio.wit has none (asked of the drivers track for this page);", "beep and say play at the level they make."]),
            _ => return Vec::from([String::from("To read: the screen is not changed from here yet."), format!("{} × {} cells of 8 × 16 pixels,", self.screen.0, self.screen.1), String::from("in the mode the firmware set at boot.")]),
        };
        lines.iter().map(|s| String::from(*s)).collect()
    }
}

// The next of `steps` from `now`: up or down (stopping at the ends), or on (0: back to the first after the last).
fn next(steps: &[u8], now: u8, step: i32) -> u8 {
    let (first, last) = (steps[0], steps[steps.len() - 1]);
    match step {
        s if s < 0 => steps.iter().rev().find(|&&v| v < now).copied().unwrap_or(first),
        s if s > 0 => steps.iter().find(|&&v| v > now).copied().unwrap_or(last),
        _ => steps.iter().find(|&&v| v > now).copied().unwrap_or(first),
    }
}
