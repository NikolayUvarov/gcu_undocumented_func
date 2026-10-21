//! `wm`'s Settings (000-APP-0048): one window for what can be configured, opened from the top bar or with Alt+S. Its
//! first page is the desktop background (000-APP-0047, 000-APP-0050), kept in `data/wm.conf`. The date and time page
//! sets the clock through the shell, at once and without a question (000-APP-0055, 000-APP-0060). The screen page sets
//! wm's scale and the pointer's size and colour (000-APP-0061), kept in `data/wm.conf` too. The network page opens what
//! shows or sets each part of the network: `wifi`, and the shell's commands in `console` (550-APP-0033). The other
//! system pages are to read and say where each setting is made. Every row and page answers Enter and a click: it changes, or a line
//! says why not (000-APP-0056). Host-tested in tests/wm_host.rs.
use crate::background::{Config, Face, Picture, Place, COMPLEXITY, FACES, PATTERNS, TEXT_SIZES, TIME_SIZES};
use crate::keys::{Code, Key};
use crate::tui::widgets::{dialog, Edit, InputLine};
use crate::tui::{Grid, Rect, Theme};
use alloc::{format, string::String, vec::Vec};

pub const PAGES: [&str; 6] = ["Background", "Keyboard", "Date and time", "Network", "Sound", "Screen"];
/// The background page's rows.
pub const ROWS: [&str; 18] = ["Picture", "Image file", "Pattern", "Speed", "Contrast", "Complexity", "Brightness", "Time", "Seconds", "Time size", "Date",
    "CPU load", "Free memory", "Battery", "Network", "Text size", "Place", "Face"];
// The rows ← and → step through (the others: the file typed, and boxes ticked).
const STEPPED: [usize; 10] = [0, 2, 3, 4, 5, 6, 9, 15, 16, 17];
const FILE: usize = 1;
const PLACES: [Place; 5] = [Place::TopLeft, Place::TopRight, Place::Center, Place::BottomLeft, Place::BottomRight];
// The speeds offered; the percentages go in fives.
const SPEEDS: [u8; 10] = [1, 2, 3, 5, 8, 10, 15, 20, 30, 50];
/// The date and time page, and its rows: the fields, then the button.
pub const DATE: usize = 2;
/// The screen page and its rows: wm's scale, the pointer's size and its colour (000-APP-0061).
pub const SCREEN: usize = 5;
pub const SCREEN_ROWS: [&str; 3] = ["Scale", "Pointer", "Colour"];
/// The network page (550-APP-0033) and its rows: a name, what Enter starts, and what it is for.
pub const NETWORK: usize = 3;
pub const NETWORK_ROWS: [(&str, &str, &str); 5] = [
    ("Wi-Fi", "wifi", "the networks around; join one"),
    ("Addresses", "console ip", "address, gateway, DNS server"),
    ("Cards", "console net", "MAC address, link, counters"),
    ("Flow grants", "console netgrants", "what each program may reach"),
    ("Policy", "console netpolicy", "its lines; a change asks first"),
];
pub const CLOCK_ROWS: [&str; 8] = ["Year", "Month", "Day", "Hours", "Minutes", "Seconds", "Time zone", "Set the clock"];
const ZONE: usize = 6;
const SET: usize = 7;
// The zone row's step and ends, in minutes east of UTC (000-APP-0099); others, as UTC+05:45, through `date zone`.
const ZONE_STEP: i32 = 30;
const ZONE_ENDS: (i32, i32) = (-12 * 60, 14 * 60);
// Each field's range and digits.
const FIELDS: [(u32, u32, u32); 6] = [(2000, 2099, 4), (1, 12, 2), (1, 31, 2), (0, 23, 2), (0, 59, 2), (0, 59, 2)];
const WIDTH: usize = 84;
const HEIGHT: usize = 23;
const LIST: usize = 17; // the pages' column

/// The window's state: the page, the row on it, whether the keys move in the pages' list, and the image's file as typed.
/// On the date and time page: the clock as last read (`now`: year, month, day; seconds since midnight), the fields
/// being set, whether the user changed them, digits being typed, its row, and whether `wm` holds the shell's commands
/// (`shell`), through which the clock is set, and the zone it shows (`zone`, minutes east of UTC). On the screen page: its row, the screen in pixels and wm's scale in use.
/// `message`: a line about the last try on any page.
pub struct Settings {
    pub page: usize, pub row: usize, pub on_pages: bool, pub file: InputLine, screen: (usize, usize),
    pub now: Option<((u32, u32, u32), u32)>, pub fields: [u32; 6], pub edited: bool, typed: Option<(u32, u32)>, pub clock_row: usize, pub zone: i32,
    pub message: Option<String>, pub shell: bool, pub screen_row: usize, pub pixels: (usize, usize), pub scale: u16, pub net_row: usize,
    /// The picture's file chosen by browsing (000-APP-0072), and how a directory is read: its entries (name, directory).
    pub browser: Option<Browser>, pub lister: Lister,
}

/// Reads a directory: its entries' names and whether each is a directory, or why not.
pub type Lister = fn(&str) -> Result<Vec<(String, bool)>, String>;

/// No files to read (the default; `wm` gives the file system's).
pub fn no_files(_: &str) -> Result<Vec<(String, bool)>, String> { Err(String::from("no files")) }

/// The pictures `wm` reads (000-APP-0050).
pub const PICTURES: [&str; 4] = ["bmp", "png", "jpg", "jpeg"];

/// Browsing for the picture (000-APP-0072): a directory (`""` the boot disk's root, `ram:`, `data/x`), its directories
/// and pictures, the one chosen, and why it could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Browser { pub dir: String, pub entries: Vec<(String, bool)>, pub selected: usize, pub problem: Option<String> }

impl Browser {
    /// `dir` read through `lister`: `..` first below a root, then its directories and its pictures, each sorted; the
    /// boot disk's root lists the RAM disk too.
    pub fn open(dir: &str, lister: Lister) -> Self {
        let mut entries = Vec::new();
        let root = dir.is_empty() || dir.ends_with(':');
        if !root { entries.push((String::from(".."), true)); }
        let mut problem = None;
        match lister(dir) {
            Ok(found) => {
                let mut dirs: Vec<String> = found.iter().filter(|e| e.1 && e.0 != "." && e.0 != "..").map(|e| e.0.clone()).collect();
                let picture = |n: &str| n.rsplit_once('.').is_some_and(|(_, ext)| PICTURES.iter().any(|p| p.eq_ignore_ascii_case(ext)));
                let mut files: Vec<String> = found.iter().filter(|e| !e.1 && picture(&e.0)).map(|e| e.0.clone()).collect();
                dirs.sort_by_key(|n| n.to_lowercase());
                files.sort_by_key(|n| n.to_lowercase());
                entries.extend(dirs.into_iter().map(|d| (d, true)));
                entries.extend(files.into_iter().map(|f| (f, false)));
            }
            Err(why) => problem = Some(why),
        }
        if dir.is_empty() { entries.push((String::from("ram:"), true)); }
        Self { dir: String::from(dir), entries, selected: 0, problem }
    }
    /// The path of entry `name` here.
    pub fn path(&self, name: &str) -> String {
        if name.ends_with(':') { return String::from(name); }
        if self.dir.is_empty() || self.dir.ends_with(':') { format!("{}{}", self.dir, name) } else { format!("{}/{}", self.dir, name) }
    }
    /// The directory above.
    pub fn up(&self) -> String {
        match self.dir.rfind('/') { Some(i) => String::from(&self.dir[..i]), None => match self.dir.find(':') { Some(i) if i + 1 < self.dir.len() => String::from(&self.dir[..=i]), _ => String::new() } }
    }
}

/// What a key or a click did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome { Stay, Close, Changed(Config), SetClock { date: u32, seconds: u32 }, SetZone { west: bool, minutes: u32 }, Run(String) }

/// A zone's name: `UTC`, `UTC+03:00`, `UTC-05:30`.
pub fn zone_name(minutes: i32) -> String {
    if minutes == 0 { return String::from("UTC"); }
    let m = minutes.unsigned_abs();
    format!("UTC{}{:02}:{:02}", if minutes < 0 { '-' } else { '+' }, m / 60, m % 60)
}

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
        Self { page: 0, row: 0, on_pages: true, file, screen, now: None, fields: [2026, 1, 1, 0, 0, 0], edited: false, typed: None, clock_row: 0, zone: 0, message: None, shell: false,
               screen_row: 0, pixels: (screen.0 * 8, screen.1 * 16), scale: 100, net_row: 0, browser: None, lister: no_files }
    }

    /// The grid became `screen` cells at `scale` (000-APP-0061): the window is centred on it anew.
    pub fn resize(&mut self, screen: (usize, usize), scale: u16) { (self.screen, self.scale) = (screen, scale); }

    /// The scale's choices on this screen: auto, then each scale offered (`crate::scale::offered`).
    pub fn scales(&self) -> Vec<Option<u16>> { core::iter::once(None).chain(crate::scale::offered(self.pixels).map(Some)).collect() }

    // The configuration with the scale stepped by `step` (+1, -1; 0 on, back to auto after the last).
    fn rescaled(&self, config: &Config, step: i32) -> Option<Config> {
        let choices = self.scales();
        let at = choices.iter().position(|&c| c == config.scale).unwrap_or(0);
        let next = if step < 0 { (at + choices.len() - 1) % choices.len() } else { (at + 1) % choices.len() };
        (choices.len() > 1).then(|| Config { scale: choices[next], ..config.clone() })
    }

    // The screen page's value as shown: auto with what it picks here, or the percentage chosen.
    fn scale_text(&self, chosen: Option<u16>) -> String {
        match chosen { None => format!("auto ({}%)", crate::scale::auto(self.pixels)), Some(s) => format!("{}%", s) }
    }

    // The screen page's row `row` stepped by `step` (+1, -1; each goes round): the configuration changed, or a line
    // saying why it stays.
    fn screen_step(&mut self, config: &Config, row: usize, step: i32) -> Outcome {
        let round = |at: usize, len: usize| if step < 0 { (at + len - 1) % len } else { (at + 1) % len };
        let changed = match row {
            0 => self.rescaled(config, step),
            1 => {
                let sizes: Vec<Option<u16>> = core::iter::once(None).chain(crate::pointer::SIZES.into_iter().map(Some)).collect();
                let at = sizes.iter().position(|&s| s == config.pointer).unwrap_or(0);
                Some(Config { pointer: sizes[round(at, sizes.len())], ..config.clone() })
            }
            _ => Some(Config { pointer_colour: round(config.pointer_colour, crate::pointer::COLOURS.len()), ..config.clone() }),
        };
        match changed {
            Some(c) => { self.message = None; Outcome::Changed(c) }
            None => { self.message = Some(String::from("This screen offers 100% alone: larger scales leave too few cells for Settings")); Outcome::Stay }
        }
    }

    /// The screen page's lines as shown: each row's name and value.
    pub fn screen_rows(&self, config: &Config) -> Vec<(&'static str, String)> {
        let pointer = match config.pointer { None => format!("auto ({}%)", self.scale), Some(s) => format!("{}%", s) };
        let colour = crate::pointer::COLOURS.get(config.pointer_colour).map_or("white", |c| c.0);
        Vec::from([(SCREEN_ROWS[0], format!("< {} >", self.scale_text(config.scale))), (SCREEN_ROWS[1], format!("< {} >", pointer)),
                   (SCREEN_ROWS[2], format!("< {} >", colour))])
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
            // The zone at once, as the screen page's rows (000-APP-0099).
            Code::Left | Code::Right if row == ZONE => return self.zone_step(if key.code() == Code::Right { ZONE_STEP } else { -ZONE_STEP }),
            Code::Left | Code::Right if row < ZONE => {
                let (lo, hi, _) = FIELDS[row];
                let v = self.fields[row];
                self.fields[row] = if key.code() == Code::Right { if v >= hi { lo } else { v + 1 } } else if v <= lo { hi } else { v - 1 };
                (self.edited, self.typed, self.message) = (true, None, None);
            }
            Code::Char if row < ZONE && key.char().is_some_and(|c| c.is_ascii_digit()) => {
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

    // ← →: the zone `by` minutes on, around its ends, set through the shell at once.
    fn zone_step(&mut self, by: i32) -> Outcome {
        self.typed = None;
        if !self.shell {
            self.message = Some(String::from("wm holds no shell's commands, so it cannot set the zone: date zone in the shell's window"));
            return Outcome::Stay;
        }
        let (west, east) = ZONE_ENDS;
        let next = self.zone + by;
        self.zone = if next > east { west } else if next < west { east } else { next };
        self.message = Some(format!("Setting the zone to {}", zone_name(self.zone)));
        Outcome::SetZone { west: self.zone < 0, minutes: self.zone.unsigned_abs() }
    }

    // Enter: the clock set to the fields, once they make a time and wm holds the shell's commands.
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
        self.message = Some(format!("Setting the clock to {:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, h, m, s));
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
            8 => c.seconds = !c.seconds,
            9 => c.time_size = next(&TIME_SIZES, c.time_size, step),
            10 => c.date = !c.date,
            11 => c.cpu = !c.cpu,
            12 => c.memory = !c.memory,
            13 => c.battery = !c.battery,
            14 => c.net = !c.net,
            15 => c.text_size = next(&TEXT_SIZES, c.text_size, step),
            16 => c.place = PLACES[cycle(PLACES.iter().position(|p| *p == c.place).unwrap_or(4), PLACES.len())],
            17 => c.face = FACES[cycle(FACES.iter().position(|f| f.1 == c.face).unwrap_or(0), FACES.len())].1,
            _ => return None,
        }
        (c != *config).then_some(c)
    }

    /// A key: arrows move (Tab between the pages and the page), Left/Right step the row's value, Space or Enter
    /// change it on (and tick a box), typing edits the image's file on its row (Enter takes it), Esc or Alt+S closes.
    pub fn key(&mut self, key: Key, config: &Config) -> Outcome {
        if self.browser.is_some() { return self.browse_key(key, config); }
        if key.code() == Code::Esc || (key.alt() && key.letter() == Some('s')) { return Outcome::Close; }
        let enterable = [0, DATE, NETWORK, SCREEN].contains(&self.page);
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
        if self.page == NETWORK {
            match key.code() {
                Code::Up => self.net_row = self.net_row.saturating_sub(1),
                Code::Down => self.net_row = (self.net_row + 1).min(NETWORK_ROWS.len() - 1),
                Code::Left => self.on_pages = true,
                Code::Right | Code::Enter => return self.network(),
                Code::Char if key.char() == Some(' ') => return self.network(),
                _ => {}
            }
            return Outcome::Stay;
        }
        if self.page == SCREEN {
            let step = match key.code() {
                Code::Up => { (self.screen_row, self.message) = (self.screen_row.saturating_sub(1), None); return Outcome::Stay }
                Code::Down => { (self.screen_row, self.message) = ((self.screen_row + 1).min(SCREEN_ROWS.len() - 1), None); return Outcome::Stay }
                Code::Left => -1,
                Code::Right | Code::Enter => 1,
                Code::Char if key.char() == Some(' ') => 1,
                _ => return Outcome::Stay,
            };
            return self.screen_step(config, self.screen_row, step);
        }
        match key.code() {
            Code::Up => { (self.row, self.message) = (self.row.saturating_sub(1), None); Outcome::Stay }
            Code::Down => { (self.row, self.message) = ((self.row + 1).min(ROWS.len() - 1), None); Outcome::Stay }
            Code::F(3) if self.row == FILE => { self.browse(); Outcome::Stay }
            _ if self.row == FILE => match self.file.key(key) {
                Edit::Submit if self.file.as_str().trim().is_empty() => { self.browse(); Outcome::Stay }
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

    // Browsing for the picture (000-APP-0072): from the directory of the file named, else data/.
    fn browse(&mut self) {
        let named = self.file.as_str().trim();
        let dir = match named.rfind('/') { Some(i) => String::from(&named[..i]), None => match named.find(':') { Some(i) => String::from(&named[..=i]), None => String::from(if named.is_empty() { "data" } else { "" }) } };
        let mut browser = Browser::open(&dir, self.lister);
        if browser.problem.is_some() && !dir.is_empty() { browser = Browser::open("", self.lister); }
        if let Some(at) = browser.entries.iter().position(|e| !e.1 && self.file.as_str().trim().ends_with(e.0.as_str())) { browser.selected = at; }
        (self.browser, self.message) = (Some(browser), None);
    }

    // A key while browsing: ↑↓ PgUp PgDn Home End choose, Enter opens a directory or takes a picture, Backspace goes up,
    // Esc goes back to the rows.
    fn browse_key(&mut self, key: Key, config: &Config) -> Outcome {
        let Some(b) = self.browser.as_mut() else { return Outcome::Stay };
        let last = b.entries.len().saturating_sub(1);
        match key.code() {
            Code::Esc => { self.browser = None; return Outcome::Stay }
            Code::Up => b.selected = b.selected.saturating_sub(1),
            Code::Down => b.selected = (b.selected + 1).min(last),
            Code::PageUp => b.selected = b.selected.saturating_sub(10),
            Code::PageDown => b.selected = (b.selected + 10).min(last),
            Code::Home => b.selected = 0,
            Code::End => b.selected = last,
            Code::Backspace => { let up = b.up(); self.browser = Some(Browser::open(&up, self.lister)); }
            Code::Enter => return self.browse_enter(config),
            _ => {}
        }
        Outcome::Stay
    }

    // Enter on the entry chosen: a directory is opened (`..` the one above), a picture becomes the background.
    fn browse_enter(&mut self, config: &Config) -> Outcome {
        let Some(b) = self.browser.as_ref() else { return Outcome::Stay };
        let Some((name, dir)) = b.entries.get(b.selected).cloned() else { return Outcome::Stay };
        if dir {
            let (from, to) = (b.dir.clone(), if name == ".." { b.up() } else { b.path(&name) });
            let mut next = Browser::open(&to, self.lister);
            // Back up: the cursor on the directory left.
            if name == ".." { if let Some(at) = next.entries.iter().position(|e| e.1 && next.path(&e.0) == from) { next.selected = at; } }
            self.browser = Some(next);
            return Outcome::Stay;
        }
        let path = b.path(&name);
        self.browser = None;
        self.file = InputLine::new();
        for ch in path.chars() { self.file.insert(ch); }
        self.row = FILE;
        let config = Config { picture: Picture::Image(String::from("")), ..config.clone() };
        self.changed(&config, FILE, 0)
    }

    // The network page's row: what it starts, Settings closed so its window is in front.
    fn network(&mut self) -> Outcome {
        let (name, command, _) = NETWORK_ROWS[self.net_row];
        self.message = Some(format!("{}: {}", name, command));
        Outcome::Run(String::from(command))
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
            9 => format!("{}%", config.time_size),
            15 => format!("{}%", config.text_size),
            _ => String::new(),
        };
        match row {
            FILE if file.is_empty() => String::from("Type the image's file here (BMP, PNG or JPEG), then Enter; F3 or Enter: browse"),
            FILE => format!("{} is the picture already", file),
            3..=6 | 9 | 15 => format!("{} is at its {} already: {}", ROWS[row], if step < 0 { "lowest" } else { "highest" }, value),
            _ => format!("{} stays as it is", ROWS[row]),
        }
    }

    /// The window's frame on a screen of the given cells.
    pub fn area(&self) -> Rect { Rect::new(0, 0, self.screen.0, self.screen.1).centered(WIDTH, HEIGHT) }

    /// A click at cell (x, y): a page's name opens it, a row of the background page is chosen and changed, a click on
    /// a page to read says so, a click outside the window closes it.
    pub fn click(&mut self, x: usize, y: usize, config: &Config) -> Outcome {
        let inner = self.area().inner();
        if let Some(b) = self.browser.as_mut() {
            // A click on an entry chooses it and does what Enter does; elsewhere in the window nothing, outside it Esc.
            if x < inner.x || x >= inner.right() || y < inner.y || y >= inner.bottom() { self.browser = None; return Outcome::Stay; }
            let room = inner.h.saturating_sub(4);
            let first = b.selected.saturating_sub(room.saturating_sub(1));
            if x >= inner.x + LIST + 1 && y > inner.y && first + y - inner.y - 1 < b.entries.len() && y - inner.y - 1 < room {
                b.selected = first + y - inner.y - 1;
                return self.browse_enter(config);
            }
            return Outcome::Stay;
        }
        if x < inner.x || x >= inner.right() || y < inner.y || y >= inner.bottom() { return Outcome::Close; }
        let (cx, cy) = (x - inner.x, y - inner.y);
        if cx < LIST {
            if cy < PAGES.len() { self.open(cy); self.on_pages = true; }
            return Outcome::Stay;
        }
        if self.page == 0 && (1..=ROWS.len()).contains(&cy) {
            (self.row, self.on_pages, self.message) = (cy - 1, false, None);
            if self.row != FILE { return self.changed(config, self.row, 0); }
            if self.file.as_str().trim().is_empty() { self.browse(); return Outcome::Stay; }
        }
        // A page to read says so.
        if ![0, DATE, NETWORK, SCREEN].contains(&self.page) { self.to_read(); }
        // The network page: a click on a row starts what it opens.
        if self.page == NETWORK && (1..=NETWORK_ROWS.len()).contains(&cy) {
            (self.net_row, self.on_pages) = (cy - 1, false);
            return self.network();
        }
        // The screen page: a click on a row chooses it and steps it on.
        if self.page == SCREEN && (1..=SCREEN_ROWS.len()).contains(&cy) {
            (self.screen_row, self.on_pages) = (cy - 1, false);
            return self.screen_step(config, self.screen_row, 1);
        }
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
        Vec::from([(ROWS[0], format!("< {} >", picture)), (ROWS[1], if self.file.as_str().is_empty() { String::from("(type it, or Enter or F3: browse)") } else { String::from(self.file.as_str()) }), (ROWS[2], format!("< {} >", pattern)),
                   (ROWS[3], format!("< {} >  1 slow (for work) to 50 fast (for a show)", config.speed)), (ROWS[4], format!("< {}% >", config.contrast)),
                   (ROWS[5], format!("< {} >  1 to 5: more waves, rings, curtains, blobs", config.complexity)),
                   (ROWS[6], format!("< {}% >  of what is shown over the picture", config.info)),
                   (ROWS[7], check(config.time)), (ROWS[8], format!("{} of the time", check(config.seconds))),
                   (ROWS[9], format!("< {}% >  the time's digits", config.time_size)), (ROWS[10], check(config.date)), (ROWS[11], check(config.cpu)),
                   (ROWS[12], check(config.memory)), (ROWS[13], format!("{}  its charge, and the time left or to full", check(config.battery))),
                   (ROWS[14], format!("{} (no counters yet)", check(config.net).as_str())),
                   (ROWS[15], format!("< {}% >  the lines under the time", config.text_size)), (ROWS[16], format!("< {} >", place)),
                   (ROWS[17], String::from(if config.face == Face::Analog { "< analog >  a dial, and gauges of CPU, memory, battery" } else { "< digital >  digits, and a graph of the CPU" }))])
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
        if let (0, Some(b)) = (self.page, &self.browser) {
            // The picture's file (000-APP-0072): the directory, its directories and pictures, the one chosen.
            let shown = if b.dir.is_empty() { String::from("/") } else { b.dir.clone() };
            grid.text_max(x, inner.y, &format!("Choose the picture (BMP, PNG, JPEG): {}", shown), w, theme.dialog);
            let room = inner.h.saturating_sub(4);
            let first = b.selected.saturating_sub(room.saturating_sub(1));
            for (i, (name, dir)) in b.entries.iter().enumerate().skip(first).take(room) {
                let line = if *dir && !name.ends_with(':') { format!(" {}/", name) } else { format!(" {}", name) };
                grid.text_padded(x, inner.y + 1 + i - first, &line, w, if i == b.selected { theme.selected } else if *dir { theme.accent } else { theme.dialog });
            }
            let note = match (&b.problem, b.entries.iter().any(|e| !e.1)) {
                (Some(why), _) => format!("{} cannot be read: {}", shown, why),
                (None, false) => String::from("No picture here: a directory, or Backspace to go up"),
                (None, true) => format!("{} pictures here", b.entries.iter().filter(|e| !e.1).count()),
            };
            grid.text_max(x, inner.bottom().saturating_sub(2), &note, w, theme.accent);
        } else if self.page == 0 {
            grid.text(x, inner.y, "The desktop background, kept in data/wm.conf:", theme.dialog);
            for (i, (name, value)) in self.rows(config).iter().enumerate() {
                let chosen = !self.on_pages && self.row == i;
                let line = format!("{:<11} {}", name, value);
                grid.text_padded(x, inner.y + 1 + i, &line, w, if chosen { theme.selected } else { theme.dialog });
                if chosen && i == FILE { cursor = Some((x + 12 + self.file.cursor_chars(), inner.y + 1 + i)); }
            }
            if let Some(message) = &self.message { grid.text_max(x, inner.y + 1 + ROWS.len(), message, w, theme.accent); }
        } else if self.page == SCREEN {
            let (cw, ch) = crate::scale::cell(self.scale);
            let (cols, rows) = crate::scale::cells(self.scale, self.pixels);
            let offered: Vec<String> = self.scales().iter().map(|&c| self.scale_text(c)).collect();
            let (pw, ph) = crate::pointer::size(crate::pointer::percent(config.pointer, self.scale));
            grid.text_max(x, inner.y, "wm's scale and pointer, kept in data/wm.conf:", w, theme.dialog);
            for (i, (name, value)) in self.screen_rows(config).iter().enumerate() {
                let chosen = !self.on_pages && self.screen_row == i;
                grid.text_padded(x, inner.y + 1 + i, &format!("{:<11} {}", name, value), w, if chosen { theme.selected } else { theme.dialog });
            }
            let lines = [format!("The screen: {} × {} pixels.", self.pixels.0, self.pixels.1),
                         format!("At {}%: cells of {} × {} pixels, {} × {} of them.", self.scale, cw, ch, cols, rows),
                         format!("auto: the largest scale leaving {} × {} pixels at 100%.", crate::scale::AUTO_ROOM.0, crate::scale::AUTO_ROOM.1),
                         format!("Here: {}.", offered.join(", ")),
                         format!("Each leaves {} × {} cells or more. Windows and the", crate::scale::FEWEST.0, crate::scale::FEWEST.1),
                         String::from("background's lines grow with the scale."),
                         format!("The pointer: {} × {} pixels; auto draws it at the scale.", pw, ph)];
            let below = inner.y + 2 + SCREEN_ROWS.len();
            for (i, line) in lines.iter().enumerate() { grid.text_max(x, below + i, line, w, theme.dialog); }
            if let Some(message) = &self.message { grid.text_max(x, below + lines.len(), message, w, theme.accent); }
        } else if self.page == NETWORK {
            grid.text_max(x, inner.y, "The network: each row opens what shows or sets it.", w, theme.dialog);
            for (i, (name, command, what)) in NETWORK_ROWS.iter().enumerate() {
                let chosen = !self.on_pages && self.net_row == i;
                grid.text_padded(x, inner.y + 1 + i, &format!("{:<11} {:<17} {}", name, command, what), w, if chosen { theme.selected } else { theme.dialog });
            }
            let lines = ["Wi-Fi goes through the 802.11 station of the network track",
                         "(task 550). Until it runs, wifi says so; wifi --demo shows",
                         "the program with a stand-in station. console passes the",
                         "shell's commands to the shell; a change asks in its window."];
            let below = inner.y + 2 + NETWORK_ROWS.len();
            for (i, line) in lines.iter().enumerate() { grid.text_max(x, below + i, line, w, theme.dialog); }
            if let Some(message) = &self.message { grid.text_max(x, below + lines.len() + 1, message, w, theme.accent); }
        } else if self.page == DATE {
            let now = match self.now { Some(((y, mo, d), s)) => format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, s / 3600, s / 60 % 60, s % 60), None => String::from("cannot be read") };
            grid.text_max(x, inner.y, &format!("The clock, {}: {}", zone_name(self.zone), now), w, theme.dialog);
            for (i, name) in CLOCK_ROWS.iter().enumerate() {
                let chosen = !self.on_pages && self.clock_row == i;
                let line = if i == SET { format!("[ {} ]  at once, through the shell", name) } else if i == ZONE {
                    format!("{:<11} < {} >  at once; the RTC keeps UTC", name, zone_name(self.zone))
                } else {
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
        let help = if self.browser.is_some() { "↑↓: choose · Enter: open it or take the picture · Backspace: up · Esc: back" }
            else if self.on_pages && ![0, DATE, NETWORK, SCREEN].contains(&self.page) { "↑↓: page · this page is to read · Esc: close" }
            else if !self.on_pages && self.page == NETWORK { "↑↓: rows · Enter: open it in a window · Tab: pages · Esc: close" }
            else if !self.on_pages && self.page == SCREEN { "↑↓: rows · ← → Enter Space: change, used at once · Tab: pages · Esc: close" }
            else if self.on_pages { "↑↓: page · Enter or →: its settings · Esc: close" } else if self.page == DATE { "↑↓: rows · ← → or digits: change · Enter: set the clock · Tab: pages · Esc: close" }
            else if self.row == FILE { "type the file · Enter: use it (nothing typed: browse) · F3: browse · ↑↓: rows · Esc: close" } else { "↑↓: rows · ← → Space: change · Tab: pages · Esc: close" };
        grid.text_max(inner.x + 1, inner.bottom() - 1, help, inner.w - 2, theme.dialog);
        cursor
    }

    /// The pages to read: a first line that says so, then where the setting is made today.
    pub fn page_text(&self) -> Vec<String> {
        let shell = ["in the shell's window, which Shell in the menu opens", "(a right click on the desktop, Alt+P), as Ctrl+Alt+F5 does."];
        let lines: Vec<&str> = match self.page {
            1 => Vec::from(["To read: the keyboard is set in the shell.", "The layout and its switch:", "  keymap us|ru [--switch both|ctrl-shift|alt-shift|caps|none]", shell[0], shell[1]]),
            _ => Vec::from(["To read: the sound has no volume to set yet.", "audio.wit has none (asked of the drivers track for this page);", "beep and say play at the level they make."]),
        };
        lines.iter().map(|s| String::from(*s)).collect()
    }
}

// The next of `steps` from `now`: up or down (stopping at the ends), or on (0: back to the first after the last).
fn next<T: Copy + PartialOrd>(steps: &[T], now: T, step: i32) -> T {
    let (first, last) = (steps[0], steps[steps.len() - 1]);
    match step {
        s if s < 0 => steps.iter().rev().find(|&&v| v < now).copied().unwrap_or(first),
        s if s > 0 => steps.iter().find(|&&v| v > now).copied().unwrap_or(last),
        _ => steps.iter().find(|&&v| v > now).copied().unwrap_or(first),
    }
}
