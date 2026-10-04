//! Widgets over a `Grid`: scrollable list state, input line with history, menu bar, F-key bar, dialogs, progress.
//! They hold state and draw; the program owns the event loop and passes keys in.
use super::{Grid, Line, Rect, Style, Theme};
use crate::keys::{Code, Key};

/// Selection and scroll position of a list of `len` items shown in `height` rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListState { pub selected: usize, pub top: usize }

impl ListState {
    /// Moves the selection for navigation keys; returns true if the key was one of them.
    pub fn key(&mut self, key: Key, len: usize, height: usize) -> bool {
        if len == 0 { *self = Self::default(); return matches!(key.code(), Code::Up | Code::Down | Code::PageUp | Code::PageDown | Code::Home | Code::End); }
        let page = height.max(1);
        let last = len - 1;
        self.selected = match key.code() {
            Code::Up => self.selected.saturating_sub(1),
            Code::Down => (self.selected + 1).min(last),
            Code::PageUp => self.selected.saturating_sub(page - 1),
            Code::PageDown => (self.selected + page - 1).min(last),
            Code::Home => 0,
            Code::End => last,
            _ => return false,
        };
        self.scroll(len, height);
        true
    }
    /// Keeps the selection inside `len` and visible in `height` rows.
    pub fn scroll(&mut self, len: usize, height: usize) {
        if len == 0 { *self = Self::default(); return; }
        self.selected = self.selected.min(len - 1);
        let height = height.max(1);
        if self.selected < self.top { self.top = self.selected; }
        if self.selected >= self.top + height { self.top = self.selected + 1 - height; }
        if self.top + height > len { self.top = len.saturating_sub(height); }
    }
    pub fn select(&mut self, index: usize, len: usize, height: usize) { self.selected = index; self.scroll(len, height); }
}

/// What a key did to an input line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edit { Unchanged, Changed, Moved, Submit, Cancel, Ignored }

pub const LINE_MAX: usize = 256;

/// One line of UTF-8 text with a cursor: insertion, deletion and movement by character and word.
#[derive(Clone)]
pub struct InputLine { bytes: [u8; LINE_MAX], len: usize, cursor: usize, scroll: usize }

impl Default for InputLine { fn default() -> Self { Self::new() } }

impl InputLine {
    pub const fn new() -> Self { Self { bytes: [0; LINE_MAX], len: 0, cursor: 0, scroll: 0 } }
    pub fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("") }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    /// Cursor position in characters.
    pub fn cursor_chars(&self) -> usize { self.as_str()[..self.cursor].chars().count() }
    pub fn clear(&mut self) { self.len = 0; self.cursor = 0; self.scroll = 0; }
    pub fn set(&mut self, text: &str) {
        let mut len = text.len().min(LINE_MAX);
        while !text.is_char_boundary(len) { len -= 1; }
        self.bytes[..len].copy_from_slice(&text.as_bytes()[..len]); self.len = len; self.cursor = len;
    }
    pub fn insert(&mut self, ch: char) -> bool {
        let mut buffer = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buffer).as_bytes();
        if self.len + encoded.len() > LINE_MAX { return false; }
        self.bytes.copy_within(self.cursor..self.len, self.cursor + encoded.len());
        self.bytes[self.cursor..self.cursor + encoded.len()].copy_from_slice(encoded);
        self.len += encoded.len(); self.cursor += encoded.len();
        true
    }
    fn prev(&self, at: usize) -> usize { self.as_str()[..at].char_indices().next_back().map_or(0, |(i, _)| i) }
    fn next(&self, at: usize) -> usize { self.as_str()[at..].chars().next().map_or(at, |c| at + c.len_utf8()) }
    fn word_left(&self) -> usize {
        let text = self.as_str(); let mut at = self.cursor;
        while at > 0 && text[..at].ends_with(' ') { at = self.prev(at); }
        while at > 0 && !text[..at].ends_with(' ') { at = self.prev(at); }
        at
    }
    fn word_right(&self) -> usize {
        let text = self.as_str(); let mut at = self.cursor;
        while at < self.len && !text[at..].starts_with(' ') { at = self.next(at); }
        while at < self.len && text[at..].starts_with(' ') { at = self.next(at); }
        at
    }
    fn remove(&mut self, from: usize, to: usize) {
        self.bytes.copy_within(to..self.len, from); self.len -= to - from; self.cursor = from;
    }
    pub fn key(&mut self, key: Key) -> Edit {
        let before = self.cursor;
        match key.code() {
            Code::Enter => return Edit::Submit,
            Code::Esc => return Edit::Cancel,
            Code::Left if key.ctrl() => self.cursor = self.word_left(),
            Code::Right if key.ctrl() => self.cursor = self.word_right(),
            Code::Left => self.cursor = self.prev(self.cursor),
            Code::Right => self.cursor = self.next(self.cursor),
            Code::Home => self.cursor = 0,
            Code::End => self.cursor = self.len,
            Code::Backspace if key.ctrl() || key.alt() => { let to = self.cursor; let from = self.word_left(); if from == to { return Edit::Unchanged; } self.remove(from, to); return Edit::Changed; }
            Code::Backspace => { if self.cursor == 0 { return Edit::Unchanged; } let from = self.prev(self.cursor); self.remove(from, self.cursor); return Edit::Changed; }
            Code::Delete => { if self.cursor == self.len { return Edit::Unchanged; } let to = self.next(self.cursor); self.remove(self.cursor, to); return Edit::Changed; }
            _ if key.is_ctrl('y') => { if self.len == 0 { return Edit::Unchanged; } self.clear(); return Edit::Changed; }
            _ => return match key.text() { Some(ch) => if self.insert(ch) { Edit::Changed } else { Edit::Unchanged }, None => Edit::Ignored },
        }
        if self.cursor == before { Edit::Unchanged } else { Edit::Moved }
    }
    /// Draws the line in `width` cells, scrolled so the cursor is visible; returns the cursor column.
    pub fn draw(&mut self, grid: &mut Grid, x: usize, y: usize, width: usize, style: Style) -> usize {
        let width = width.max(1);
        let cursor = self.cursor_chars();
        if cursor < self.scroll { self.scroll = cursor; }
        if cursor >= self.scroll + width { self.scroll = cursor + 1 - width; }
        let start = self.as_str().char_indices().nth(self.scroll).map_or(self.len, |(i, _)| i);
        let text = core::str::from_utf8(&self.bytes[start..self.len]).unwrap_or("");
        grid.text_padded(x, y, text, width, style);
        x + cursor - self.scroll
    }
}

/// The last `N` submitted lines, newest last; ↑/↓ walk through them.
pub struct History<const N: usize> { lines: [[u8; LINE_MAX]; N], lens: [usize; N], count: usize, next: usize, browse: Option<usize>, draft: InputLine }

impl<const N: usize> Default for History<N> { fn default() -> Self { Self::new() } }

impl<const N: usize> History<N> {
    pub const fn new() -> Self { Self { lines: [[0; LINE_MAX]; N], lens: [0; N], count: 0, next: 0, browse: None, draft: InputLine::new() } }
    pub fn len(&self) -> usize { self.count }
    pub fn is_empty(&self) -> bool { self.count == 0 }
    /// Line `age` steps back (0 = the newest).
    pub fn get(&self, age: usize) -> Option<&str> {
        if age >= self.count { return None; }
        let index = (self.next + N - 1 - age) % N;
        core::str::from_utf8(&self.lines[index][..self.lens[index]]).ok()
    }
    /// Remembers a submitted line (not empty, not a repeat of the newest).
    pub fn push(&mut self, line: &str) {
        self.browse = None;
        if line.trim().is_empty() || self.get(0) == Some(line) { return; }
        let len = line.len().min(LINE_MAX);
        self.lines[self.next][..len].copy_from_slice(&line.as_bytes()[..len]); self.lens[self.next] = len;
        self.next = (self.next + 1) % N; self.count = (self.count + 1).min(N);
    }
    /// ↑ (older) or ↓ (newer) replaces `line`; the line being typed is kept and comes back after the newest.
    pub fn key(&mut self, key: Key, line: &mut InputLine) -> bool {
        match key.code() {
            Code::Up => {
                let age = self.browse.map_or(0, |a| a + 1);
                if age >= self.count { return true; }
                if self.browse.is_none() { self.draft = line.clone(); }
                let text = self.get(age).unwrap_or("");
                let mut copy = [0u8; LINE_MAX]; let len = text.len(); copy[..len].copy_from_slice(text.as_bytes());
                line.set(core::str::from_utf8(&copy[..len]).unwrap_or(""));
                self.browse = Some(age);
                true
            }
            Code::Down => {
                match self.browse {
                    None => {}
                    Some(0) => { *line = self.draft.clone(); line.cursor = line.len; self.browse = None; }
                    Some(age) => {
                        let text = self.get(age - 1).unwrap_or("");
                        let mut copy = [0u8; LINE_MAX]; let len = text.len(); copy[..len].copy_from_slice(text.as_bytes());
                        line.set(core::str::from_utf8(&copy[..len]).unwrap_or(""));
                        self.browse = Some(age - 1);
                    }
                }
                true
            }
            _ => false,
        }
    }
    /// Stops browsing (after the line was edited).
    pub fn reset(&mut self) { self.browse = None; }
}

/// What the menu bar did with a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction { None, Closed, Chosen(usize, usize) }

/// A menu bar with drop-down menus (F9 opens it in the tools).
pub struct MenuBar<'a> { pub titles: &'a [&'a str], pub items: &'a [&'a [&'a str]], pub open: bool, pub menu: usize, pub item: usize }

impl<'a> MenuBar<'a> {
    pub const fn new(titles: &'a [&'a str], items: &'a [&'a [&'a str]]) -> Self { Self { titles, items, open: false, menu: 0, item: 0 } }
    pub fn key(&mut self, key: Key) -> MenuAction {
        if !self.open { return MenuAction::None; }
        let count = self.titles.len();
        let items = self.items.get(self.menu).map_or(0, |i| i.len());
        match key.code() {
            Code::Esc | Code::F(9) | Code::F(10) => { self.open = false; return MenuAction::Closed; }
            Code::Left => { self.menu = (self.menu + count - 1) % count; self.item = 0; }
            Code::Right => { self.menu = (self.menu + 1) % count; self.item = 0; }
            Code::Up if items > 0 => self.item = (self.item + items - 1) % items,
            Code::Down if items > 0 => self.item = (self.item + 1) % items,
            Code::Enter if items > 0 => { self.open = false; return MenuAction::Chosen(self.menu, self.item); }
            _ => {}
        }
        MenuAction::None
    }
    /// Draws the bar on row `y` (always) and the open menu below it.
    pub fn draw(&self, grid: &mut Grid, y: usize, theme: &Theme) {
        grid.fill(Rect::new(0, y, grid.cols, 1), ' ', theme.menu);
        let mut x = 2;
        for (index, title) in self.titles.iter().enumerate() {
            let style = if self.open && index == self.menu { theme.menu_selected } else { theme.menu };
            let len = title.chars().count();
            grid.put(x, y, ' ', style); grid.text(x + 1, y, title, style); grid.put(x + 1 + len, y, ' ', style);
            if self.open && index == self.menu {
                let items = self.items.get(index).copied().unwrap_or(&[]);
                let width = items.iter().map(|i| i.chars().count()).max().unwrap_or(0) + 4;
                let area = Rect::new(x, y + 1, width, items.len() + 2);
                grid.frame(area, Line::Single, theme.menu);
                for (row, item) in items.iter().enumerate() {
                    let style = if row == self.item { theme.menu_selected } else { theme.menu };
                    grid.text_padded(area.x + 1, area.y + 1 + row, "", width - 2, style);
                    grid.text(area.x + 2, area.y + 1 + row, item, style);
                }
            }
            x += len + 3;
        }
    }
}

/// The Norton Commander key bar on row `y`: `1Help 2Save ...`; empty labels leave their slot blank.
pub fn fkey_bar(grid: &mut Grid, y: usize, labels: &[&str], theme: &Theme) {
    let slots = labels.len().max(1);
    let width = grid.cols / slots;
    for (index, label) in labels.iter().enumerate() {
        let x = index * width;
        let number = index + 1;
        let digits = if number >= 10 { 2 } else { 1 };
        if digits == 2 { grid.put(x, y, '1', theme.fkey_number); grid.put(x + 1, y, char::from_digit((number % 10) as u32, 10).unwrap_or('0'), theme.fkey_number); }
        else { grid.put(x, y, char::from_digit(number as u32, 10).unwrap_or('0'), theme.fkey_number); }
        let w = if index + 1 == slots { grid.cols - x - digits } else { width - digits };
        grid.text_padded(x + digits, y, label, w, theme.fkey_label);
    }
}

/// A dialog frame centred on the grid; returns the area inside the frame.
pub fn dialog(grid: &mut Grid, title: &str, w: usize, h: usize, theme: &Theme) -> Rect {
    let area = grid.area().centered(w, h);
    grid.frame_titled(area, Line::Double, title, theme.dialog_frame, theme.dialog_frame);
    grid.restyle(area.inner(), theme.dialog);
    area.inner()
}

/// A message box: lines of text and a row of buttons, the `selected` one highlighted.
pub fn message(grid: &mut Grid, title: &str, lines: &[&str], buttons: &[&str], selected: usize, theme: &Theme) {
    let text_w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let buttons_w: usize = buttons.iter().map(|b| b.chars().count() + 4).sum::<usize>() + buttons.len().saturating_sub(1);
    let w = text_w.max(buttons_w).max(title.chars().count() + 4) + 6;
    let inner = dialog(grid, title, w, lines.len() + 4, theme);
    for (row, line) in lines.iter().enumerate() { grid.text_centered(inner, inner.y + row, line, theme.dialog); }
    let mut x = inner.x + (inner.w - buttons_w) / 2;
    for (index, button) in buttons.iter().enumerate() {
        let style = if index == selected { theme.selected } else { theme.dialog };
        grid.text(x, inner.bottom() - 1, "[ ", style);
        let len = grid.text(x + 2, inner.bottom() - 1, button, style);
        grid.text(x + 2 + len, inner.bottom() - 1, " ]", style);
        x += len + 5;
    }
}

/// Left/Right/Tab move between `count` buttons; Enter picks, Esc cancels.
pub fn buttons_key(key: Key, selected: &mut usize, count: usize) -> Option<Option<usize>> {
    match key.code() {
        Code::Left => { *selected = (*selected + count - 1) % count; None }
        Code::Right | Code::Tab => { *selected = (*selected + 1) % count; None }
        Code::Enter => Some(Some(*selected)),
        Code::Esc => Some(None),
        _ => None,
    }
}

/// A dialog with a prompt and an input line; returns the cursor position for the terminal.
pub fn input_dialog(grid: &mut Grid, title: &str, prompt: &str, line: &mut InputLine, width: usize, theme: &Theme) -> (usize, usize) {
    let inner = dialog(grid, title, width, 5, theme);
    grid.text(inner.x + 1, inner.y, prompt, theme.dialog);
    let column = line.draw(grid, inner.x + 1, inner.y + 1, inner.w - 2, theme.input);
    (column, inner.y + 1)
}

/// Progress bar with a percentage at its right.
pub fn progress(grid: &mut Grid, x: usize, y: usize, width: usize, done: u64, total: u64, fill: Style, empty: Style) {
    let percent = if total == 0 { 100 } else { (done.min(total) * 100 / total) as usize };
    let bar = width.saturating_sub(5);
    grid.bar(x, y, bar, done, total.max(1), fill, empty);
    let mut text = crate::util::FixedBuf::<8>::new();
    let _ = core::fmt::Write::write_fmt(&mut text, format_args!("{:>4}%", percent));
    grid.text(x + bar, y, core::str::from_utf8(text.as_bytes()).unwrap_or(""), empty);
}
