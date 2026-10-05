//! A panel of the file manager: a directory listing, sorted and filtered, with the cursor and marked files.
use crate::keys::{Code, Key};
use crate::tui::widgets::ListState;
use crate::tui::{Grid, Line, Rect, Style, Theme};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering;

/// Attribute bits of a directory entry (`mind::fs::ENTRY_*`, idl/vfs.wit `entry.attributes`).
pub const VFS_ENTRY_DIR: u8 = 1;
pub const VFS_ENTRY_HIDDEN: u8 = 2;
pub const VFS_ENTRY_SYSTEM: u8 = 4;
pub const VFS_ENTRY_READ_ONLY: u8 = 8;
pub const VFS_ENTRY_ARCHIVE: u8 = 16;

/// A directory entry as `vfs_server` lists it (`flags`: `VFS_ENTRY_*`; `modified`: FAT date << 16 | FAT time).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry { pub name: String, pub size: u64, pub dir: bool, pub flags: u8, pub modified: u32 }

impl Entry {
    pub fn file(name: &str, size: u64) -> Self { Self { name: name.into(), size, ..Self::default() } }
    pub fn directory(name: &str) -> Self { Self { name: name.into(), dir: true, flags: VFS_ENTRY_DIR, ..Self::default() } }
    /// The entry that leads to the parent directory.
    pub fn up() -> Self { Self::directory("..") }
    pub fn is_up(&self) -> bool { self.name == ".." }
    pub fn hidden(&self) -> bool { self.flags & (VFS_ENTRY_HIDDEN | VFS_ENTRY_SYSTEM) != 0 }
    pub fn extension(&self) -> &str {
        if self.dir { return ""; }
        match self.name.rfind('.') { Some(i) if i > 0 => &self.name[i + 1..], _ => "" }
    }
    pub fn is_program(&self) -> bool { !self.dir && self.extension().eq_ignore_ascii_case("elf") }
}

/// A FAT date and time as (year, month, day, hour, minute, second).
pub fn fat_time(modified: u32) -> (u32, u32, u32, u32, u32, u32) {
    let (date, time) = (modified >> 16, modified & 0xFFFF);
    (1980 + (date >> 9), (date >> 5) & 0xF, date & 0x1F, time >> 11, (time >> 5) & 0x3F, (time & 0x1F) * 2)
}

/// `2026-10-04` and `12:34`, or blanks when the entry has no time.
pub fn date_time(modified: u32) -> (String, String) {
    if modified == 0 { return (String::new(), String::new()); }
    let (y, mo, d, h, mi, _) = fat_time(modified);
    (format!("{}-{:02}-{:02}", y, mo, d), format!("{:02}:{:02}", h, mi))
}

/// Size with a unit for narrow columns: 999, 12K, 3.4M.
pub fn short_size(bytes: u64) -> String {
    if bytes < 100_000 { return format!("{}", bytes); }
    for (unit, letter) in [(1u64 << 30, 'G'), (1 << 20, 'M'), (1 << 10, 'K')] {
        if bytes >= unit {
            let tenths = bytes * 10 / unit;
            return if tenths >= 100 { format!("{}{}", tenths / 10, letter) } else { format!("{}.{}{}", tenths / 10, tenths % 10, letter) };
        }
    }
    format!("{}", bytes)
}

/// Wildcard masks (`mind::pattern`, shared with `find`): `*`, `?`, case-insensitive, several separated by `,`, `;` or
/// spaces; `*.*` matches names without an extension too.
pub use crate::pattern::matches;

/// The volume of a path and the path on it: `ram:docs` -> (`ram:`, `docs`); a path without one is on the boot disk
/// (`docs` -> (``, `docs`)).
pub fn volume(path: &str) -> (&str, &str) {
    match path.find(':') { Some(i) if !path[..i].contains('/') => (&path[..=i], &path[i + 1..]), _ => ("", path) }
}

/// The path is the root of its volume.
pub fn is_root(path: &str) -> bool { volume(path).1.is_empty() }

/// Two paths are on the same volume.
pub fn same_volume(a: &str, b: &str) -> bool { volume(a).0.eq_ignore_ascii_case(volume(b).0) }

/// How a path is shown: `A:/docs`, `ram:/docs`.
pub fn display(path: &str) -> String { let (v, rest) = volume(path); format!("{}/{}", if v.is_empty() { "A:" } else { v }, rest) }

/// A path typed by the user, relative to `current` unless it names a volume (`A:/x`, `ram:x`) or starts with `/` (the
/// root of `current`'s volume).
pub fn resolve(current: &str, typed: &str) -> String {
    let typed = typed.trim();
    let (v, rest) = volume(typed);
    let clean = |rest: &str| -> String { rest.split('/').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("/") };
    if v.eq_ignore_ascii_case("a:") { return clean(rest); }
    if !v.is_empty() { return format!("{}{}", v.to_lowercase(), clean(rest)); }
    if typed.starts_with('/') { return format!("{}{}", volume(current).0, clean(typed)); }
    let mut path = String::from(current);
    for part in typed.split('/').filter(|p| !p.is_empty() && *p != ".") { path = if part == ".." { parent(&path).0 } else { join(&path, part) }; }
    path
}

/// `path/name` (`name` alone in the root of the boot disk, `ram:name` in the root of the RAM disk).
pub fn join(path: &str, name: &str) -> String { if is_root(path) { format!("{}{}", path, name) } else { format!("{}/{}", path, name) } }

/// The parent of `path` and the name of `path` in it.
pub fn parent(path: &str) -> (String, String) {
    let (v, rest) = volume(path);
    match rest.rfind('/') { Some(i) => (format!("{}{}", v, &rest[..i]), rest[i + 1..].into()), None => (v.into(), rest.into()) }
}

/// `path` is `dir` or below it.
pub fn inside(path: &str, dir: &str) -> bool {
    let (path, dir) = (path.to_lowercase(), dir.to_lowercase());
    path == dir || path.starts_with(&format!("{}/", dir)) || (is_root(&dir) && same_volume(&path, &dir))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort { Name, Extension, Size, Time }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Brief, Full, Info, Quick }

pub struct Panel {
    pub path: String,
    pub all: Vec<Entry>,
    /// What is shown: `..` first (except in the root), directories, then files.
    pub items: Vec<Entry>,
    pub list: ListState,
    pub sort: Sort,
    pub reverse: bool,
    pub hidden: bool,
    pub mode: Mode,
    pub marked: Vec<String>,
    pub error: Option<String>,
    page: usize, // items visible at the last draw
    rows: usize, // rows of a brief column at the last draw
}

impl Panel {
    pub fn new(mode: Mode) -> Self {
        Self { path: String::new(), all: Vec::new(), items: Vec::new(), list: ListState::default(), sort: Sort::Name, reverse: false, hidden: true, mode,
               marked: Vec::new(), error: None, page: 20, rows: 20 }
    }

    /// A new listing of `path`; the cursor goes to `focus`, or stays on the current name.
    pub fn set(&mut self, path: &str, entries: Vec<Entry>, focus: Option<&str>) {
        let keep = if path == self.path { self.current().map(|e| e.name.clone()) } else { None };
        if path != self.path { self.marked.clear(); self.list = ListState::default(); }
        self.path = path.into();
        self.all = entries;
        self.marked.retain(|m| self.all.iter().any(|e| &e.name == m));
        self.error = None;
        self.arrange(focus.map(String::from).or(keep).as_deref());
    }

    /// Sorts and filters again (after a change of order or of hidden files).
    pub fn arrange(&mut self, focus: Option<&str>) {
        let keep: Option<String> = focus.map(String::from).or_else(|| self.current().map(|e| e.name.clone()));
        let mut items: Vec<Entry> = self.all.iter().filter(|e| self.hidden || !e.hidden()).cloned().collect();
        items.sort_by(|a, b| self.compare(a, b));
        if !is_root(&self.path) { items.insert(0, Entry::up()); }
        self.items = items;
        if let Some(index) = keep.and_then(|name| self.items.iter().position(|e| e.name.eq_ignore_ascii_case(&name))) { self.list.selected = index; }
        self.scroll();
    }

    fn compare(&self, a: &Entry, b: &Entry) -> Ordering {
        let name = || a.name.to_lowercase().cmp(&b.name.to_lowercase());
        let by = match self.sort {
            Sort::Name => name(),
            Sort::Extension => a.extension().to_lowercase().cmp(&b.extension().to_lowercase()).then_with(name),
            Sort::Size => b.size.cmp(&a.size).then_with(name),
            Sort::Time => b.modified.cmp(&a.modified).then_with(name),
        };
        b.dir.cmp(&a.dir).then(if self.reverse { by.reverse() } else { by })
    }

    pub fn current(&self) -> Option<&Entry> { self.items.get(self.list.selected) }
    pub fn is_marked(&self, name: &str) -> bool { self.marked.iter().any(|m| m == name) }

    /// Marks or unmarks the current entry and moves down (Insert).
    pub fn toggle_mark(&mut self) {
        let Some(entry) = self.current().filter(|e| !e.is_up()).map(|e| e.name.clone()) else { return };
        match self.marked.iter().position(|m| *m == entry) { Some(i) => { self.marked.remove(i); } None => self.marked.push(entry) }
        self.list.selected = (self.list.selected + 1).min(self.items.len().saturating_sub(1));
        self.scroll();
    }

    /// Marks (or unmarks) the files matching `mask`; returns how many changed.
    pub fn mark_mask(&mut self, mask: &str, on: bool) -> usize {
        let names: Vec<String> = self.items.iter().filter(|e| !e.dir && matches(mask, &e.name)).map(|e| e.name.clone()).collect();
        let mut changed = 0;
        for name in names {
            let at = self.marked.iter().position(|m| *m == name);
            match (at, on) { (None, true) => { self.marked.push(name); changed += 1; } (Some(i), false) => { self.marked.remove(i); changed += 1; } _ => {} }
        }
        changed
    }

    /// Inverts the marks of the files.
    pub fn invert(&mut self) {
        let files: Vec<String> = self.items.iter().filter(|e| !e.dir).map(|e| e.name.clone()).collect();
        let (on, off): (Vec<String>, Vec<String>) = files.into_iter().partition(|n| !self.is_marked(n));
        self.marked.retain(|m| !off.contains(m));
        self.marked.extend(on);
    }

    /// (marked files, their bytes).
    pub fn marked_size(&self) -> (usize, u64) {
        let marked: Vec<&Entry> = self.items.iter().filter(|e| self.is_marked(&e.name)).collect();
        (marked.len(), marked.iter().map(|e| e.size).sum())
    }

    /// (files, their bytes, directories) in the listing.
    pub fn totals(&self) -> (usize, u64, usize) {
        let files = self.all.iter().filter(|e| !e.dir);
        (files.clone().count(), files.map(|e| e.size).sum(), self.all.iter().filter(|e| e.dir).count())
    }

    fn scroll(&mut self) {
        let len = self.items.len();
        if self.mode == Mode::Brief && self.rows > 0 {
            if len == 0 { self.list = ListState::default(); return; }
            self.list.selected = self.list.selected.min(len - 1);
            let (rows, page) = (self.rows, self.page.max(self.rows));
            if self.list.selected < self.list.top { self.list.top = self.list.selected / rows * rows; }
            if self.list.selected >= self.list.top + page { self.list.top = (self.list.selected / rows + 1) * rows - page; }
        } else {
            self.list.scroll(len, self.page);
        }
    }

    /// Moves the cursor for navigation keys; returns true if the key was one.
    pub fn key(&mut self, key: Key) -> bool {
        let len = self.items.len();
        if len == 0 { return false; }
        let last = len - 1;
        let s = self.list.selected;
        self.list.selected = match (key.code(), self.mode) {
            (Code::Up, _) => s.saturating_sub(1),
            (Code::Down, _) => (s + 1).min(last),
            (Code::Left, Mode::Brief) => s.saturating_sub(self.rows),
            (Code::Right, Mode::Brief) => (s + self.rows).min(last),
            (Code::PageUp, _) => s.saturating_sub(self.page.max(2) - 1),
            (Code::PageDown, _) => (s + self.page.max(2) - 1).min(last),
            (Code::Home, _) => 0,
            (Code::End, _) => last,
            _ => return false,
        };
        self.scroll();
        true
    }

    /// Puts the cursor on entry `index` (a click, issue u001).
    pub fn select(&mut self, index: usize) {
        if self.items.is_empty() { return; }
        self.list.selected = index.min(self.items.len() - 1);
        self.scroll();
    }

    /// Moves the cursor `lines` down (up if negative): the mouse wheel.
    pub fn move_by(&mut self, lines: isize) {
        if self.items.is_empty() { return; }
        self.list.selected = (self.list.selected as isize + lines).clamp(0, self.items.len() as isize - 1) as usize;
        self.scroll();
    }

    /// The entry `draw` showed at cell (x, y) when it drew the panel in `area` (a click, issue u001).
    pub fn entry_at(&self, area: Rect, x: usize, y: usize) -> Option<usize> {
        if area.w < 8 || area.h < 6 { return None; }
        let inner = area.inner();
        let list_h = inner.h.saturating_sub(3);
        if x < inner.x || x >= inner.right() || y <= inner.y || y > inner.y + list_h { return None; }
        let row = y - inner.y - 1;
        let index = match self.mode {
            Mode::Full => self.list.top + row,
            _ => {
                let count = if inner.w >= 39 { 3 } else if inner.w >= 20 { 2 } else { 1 };
                let column = ((x - inner.x) / ((inner.w + 1) / count)).min(count - 1);
                self.list.top + column * list_h.max(1) + row
            }
        };
        (index < self.items.len()).then_some(index)
    }

    fn style(&self, entry: &Entry, cursor: bool, theme: &Theme) -> Style {
        let base = if self.is_marked(&entry.name) { theme.marked } else if entry.dir { theme.directory } else if entry.is_program() { theme.accent } else { theme.panel };
        if cursor { Style::new(if self.is_marked(&entry.name) { theme.marked.fg } else { theme.selected.fg }, theme.selected.bg) } else { base }
    }

    /// Draws the listing (brief or full) in `area`, with the path in the frame and a status line at the bottom.
    pub fn draw(&mut self, grid: &mut Grid, area: Rect, theme: &Theme, active: bool) {
        if area.w < 8 || area.h < 6 { return; }
        let title = display(&self.path);
        let title_style = if active { theme.selected } else { theme.frame };
        grid.frame_titled(area, Line::Double, &title, theme.frame, title_style);
        let inner = area.inner();
        let list_h = inner.h.saturating_sub(3); // header, separator, status
        // Separator above the status line.
        let sep = inner.bottom() - 2;
        grid.put(area.x, sep, '╟', theme.frame);
        grid.hline(inner.x, sep, inner.w, Line::Single, theme.frame);
        grid.put(area.right() - 1, sep, '╢', theme.frame);
        match self.mode {
            Mode::Full => {
                let (size_w, date_w, time_w) = (9, 10, 5);
                let name_w = inner.w.saturating_sub(size_w + date_w + time_w + 3);
                let columns = [(inner.x, name_w, "Name"), (inner.x + name_w + 1, size_w, "Size"), (inner.x + name_w + size_w + 2, date_w, "Date"), (inner.x + name_w + size_w + date_w + 3, time_w, "Time")];
                for (i, &(x, w, title)) in columns.iter().enumerate() {
                    grid.text_centered(Rect::new(x, inner.y, w, 1), inner.y, title, theme.header);
                    if i > 0 { grid.vline(x - 1, inner.y, list_h + 1, Line::Single, theme.frame); grid.put(x - 1, sep, '┴', theme.frame); }
                }
                self.page = list_h; self.rows = list_h;
                self.scroll();
                for (row, index) in (self.list.top..self.items.len()).take(list_h).enumerate() {
                    let entry = &self.items[index];
                    let style = self.style(entry, active && index == self.list.selected, theme);
                    let y = inner.y + 1 + row;
                    let size = if entry.is_up() { String::from("►UP--DIR◄") } else if entry.dir { String::from("►SUB-DIR◄") } else { format!("{}", entry.size) };
                    let (date, time) = date_time(entry.modified);
                    grid.text_padded(columns[0].0, y, &entry.name, name_w, style);
                    grid.text_padded(columns[1].0, y, "", size_w, style); grid.text_right(columns[1].0 + size_w, y, &size, style);
                    grid.text_padded(columns[2].0, y, &date, date_w, style);
                    grid.text_padded(columns[3].0, y, &time, time_w, style);
                    for &(x, _, _) in &columns[1..] { grid.put(x - 1, y, '│', Style::new(theme.frame.fg, style.bg)); }
                }
            }
            _ => {
                // Brief: names in columns, filled top to bottom.
                let count = if inner.w >= 39 { 3 } else if inner.w >= 20 { 2 } else { 1 };
                let width = (inner.w + 1) / count;
                for c in 0..count {
                    let x = inner.x + c * width;
                    let w = if c + 1 == count { inner.right() - x } else { width - 1 };
                    grid.text_centered(Rect::new(x, inner.y, w, 1), inner.y, "Name", theme.header);
                    if c > 0 { grid.vline(x - 1, inner.y, list_h + 1, Line::Single, theme.frame); grid.put(x - 1, sep, '┴', theme.frame); }
                }
                self.rows = list_h.max(1); self.page = self.rows * count;
                self.scroll();
                for (i, index) in (self.list.top..self.items.len()).take(self.page).enumerate() {
                    let (c, row) = (i / self.rows, i % self.rows);
                    let x = inner.x + c * width;
                    let w = if c + 1 == count { inner.right() - x } else { width - 1 };
                    let entry = &self.items[index];
                    let style = self.style(entry, active && index == self.list.selected, theme);
                    let name = if entry.dir && !entry.is_up() { entry.name.to_uppercase() } else { entry.name.clone() };
                    grid.text_padded(x, inner.y + 1 + row, &name, w, style);
                }
            }
        }
        if let Some(error) = &self.error {
            grid.text_padded(inner.x, inner.y + 1, error, inner.w, theme.error);
        }
        // Status: the current entry, or what is marked.
        let status_y = inner.bottom() - 1;
        let (marked, bytes) = self.marked_size();
        let status = if marked > 0 {
            format!("{} bytes in {} marked files", bytes, marked)
        } else if let Some(e) = self.current() {
            let (date, time) = date_time(e.modified);
            let size = if e.is_up() { String::from("►UP--DIR◄") } else if e.dir { String::from("►SUB-DIR◄") } else { format!("{}", e.size) };
            format!("{}  {}  {} {}", e.name, size, date, time)
        } else { String::new() };
        grid.text_padded(inner.x, status_y, &status, inner.w, if marked > 0 { theme.marked } else { theme.panel });
    }
}
