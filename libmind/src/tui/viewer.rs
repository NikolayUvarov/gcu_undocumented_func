//! File viewer core (the `view` program, F3 in `fm`): UTF-8 text with or without wrapping and a hex dump, search,
//! go to a line/offset/percentage. The file is read on demand through a 64 KiB window, never loaded whole.
use super::widgets::{fkey_bar, input_dialog, message, Edit, InputLine, KeyBars};
use super::{Grid, Rect, Style, Theme};
use crate::keys::{Code, Key};
use core::fmt::Write;

/// Where the bytes come from (a VFS file in the system, memory in tests).
pub trait Source {
    fn size(&self) -> u64;
    /// Reads at `offset` into `out`; returns the number of bytes read.
    fn read(&mut self, offset: u64, out: &mut [u8]) -> usize;
}

/// A window of the file in a caller-provided buffer.
pub struct Cache<'b, S: Source> { source: S, buf: &'b mut [u8], start: u64, len: usize, size: u64 }

impl<'b, S: Source> Cache<'b, S> {
    pub fn new(source: S, buf: &'b mut [u8]) -> Self { let size = source.size(); Self { source, buf, start: 0, len: 0, size } }
    pub fn size(&self) -> u64 { self.size }
    /// The buffer back, for the next file.
    pub fn into_buffer(self) -> &'b mut [u8] { self.buf }
    pub fn byte(&mut self, at: u64) -> Option<u8> {
        if at >= self.size { return None; }
        if at < self.start || at >= self.start + self.len as u64 {
            // Centre the window a little behind `at`: scrolling goes both ways.
            self.start = at.saturating_sub(self.buf.len() as u64 / 4);
            self.len = self.source.read(self.start, self.buf);
            if at >= self.start + self.len as u64 { return None; }
        }
        Some(self.buf[(at - self.start) as usize])
    }
    /// The UTF-8 character at `at` and its length in bytes; invalid bytes are U+FFFD of length 1.
    pub fn char_at(&mut self, at: u64) -> Option<(char, usize)> {
        let lead = self.byte(at)?;
        let (need, init) = match lead { 0x00..=0x7F => return Some((lead as char, 1)), 0xC2..=0xDF => (1, (lead & 0x1F) as u32), 0xE0..=0xEF => (2, (lead & 0x0F) as u32), 0xF0..=0xF4 => (3, (lead & 0x07) as u32), _ => return Some(('\u{FFFD}', 1)) };
        let mut value = init;
        for i in 1..=need {
            match self.byte(at + i as u64) { Some(b) if b & 0xC0 == 0x80 => value = value << 6 | (b & 0x3F) as u32, _ => return Some(('\u{FFFD}', 1)) }
        }
        match char::from_u32(value) { Some(ch) if ch.len_utf8() == need + 1 => Some((ch, need + 1)), _ => Some(('\u{FFFD}', 1)) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Text, Hex }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action { None, Quit }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Prompt { Search, Goto }

const HEX_ROW: u64 = 16;
const NAME_MAX: usize = 128;

pub struct Viewer<'b, S: Source> {
    cache: Cache<'b, S>, pub mode: Mode, pub wrap: bool, top: u64, left: usize, width: usize, height: usize,
    name: [u8; NAME_MAX], name_len: usize,
    query: [u8; 128], query_len: usize, found: Option<(u64, usize)>,
    prompt: Option<(Prompt, InputLine)>, note: Option<&'static str>, help: bool,
    line_mark: (u64, u64), // (offset, number of the line starting there): line numbers are counted from the nearest known point
    /// The modifiers held (MOD_*, `mind::input::modifiers`): the key bar shows what the keys do with them.
    pub modifiers: u8,
}

impl<'b, S: Source> Viewer<'b, S> {
    pub fn new(source: S, buf: &'b mut [u8], name: &str) -> Self {
        let mut bytes = [0u8; NAME_MAX];
        let mut len = name.len().min(NAME_MAX);
        while !name.is_char_boundary(len) { len -= 1; }
        bytes[..len].copy_from_slice(&name.as_bytes()[..len]);
        Self { cache: Cache::new(source, buf), mode: Mode::Text, wrap: true, top: 0, left: 0, width: 80, height: 24, name: bytes, name_len: len,
               query: [0; 128], query_len: 0, found: None, prompt: None, note: None, help: false, line_mark: (0, 1), modifiers: 0 }
    }
    pub fn top(&self) -> u64 { self.top }
    pub fn size(&self) -> u64 { self.cache.size() }
    /// Closes the viewer and gives its window buffer back.
    pub fn into_buffer(self) -> &'b mut [u8] { self.cache.into_buffer() }
    fn name(&self) -> &str { core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("?") }

    // --- text layout -------------------------------------------------------------------------------------------
    /// End of the visual row starting at `start`: the start of the next row.
    fn row_end(&mut self, start: u64) -> u64 {
        let (mut at, mut col) = (start, 0usize);
        while let Some((ch, len)) = self.cache.char_at(at) {
            if ch == '\n' { return at + 1; }
            let w = if ch == '\t' { 8 - col % 8 } else { 1 };
            if self.wrap && col + w > self.width && col > 0 { return at; }
            col += w; at += len as u64;
        }
        at
    }
    fn next_row(&mut self, start: u64) -> Option<u64> { let end = self.row_end(start); (end > start && end < self.size()).then_some(end) }
    /// Start of the logical line containing `at`.
    fn line_start(&mut self, at: u64) -> u64 {
        let mut pos = at;
        while pos > 0 { if self.cache.byte(pos - 1) == Some(b'\n') { break; } pos -= 1; }
        pos
    }
    fn prev_row(&mut self, start: u64) -> Option<u64> {
        if start == 0 { return None; }
        let line = self.line_start(start - 1);
        if !self.wrap { return Some(line); }
        let mut row = line;
        loop { let next = self.row_end(row); if next >= start || next == row { return Some(row); } row = next; }
    }
    /// Row start containing `at` (text mode).
    fn row_of(&mut self, at: u64) -> u64 {
        let mut row = self.line_start(at);
        if !self.wrap { return row; }
        loop { let next = self.row_end(row); if next > at || next == row || next >= self.size() { return row; } row = next; }
    }
    fn last_page(&mut self) -> u64 {
        match self.mode {
            Mode::Hex => { let rows = self.size().div_ceil(HEX_ROW); rows.saturating_sub(self.height as u64) * HEX_ROW }
            Mode::Text => {
                let mut top = self.row_of(self.size().saturating_sub(1));
                for _ in 1..self.height { match self.prev_row(top) { Some(p) => top = p, None => break } }
                top
            }
        }
    }
    /// Line number (from 1) of the line starting at or containing `at`, counted from the nearest known point.
    fn line_number(&mut self, at: u64) -> u64 {
        let (mut pos, mut line) = self.line_mark;
        if at >= pos {
            while pos < at { if self.cache.byte(pos) == Some(b'\n') { line += 1; } pos += 1; }
        } else {
            while pos > at { pos -= 1; if self.cache.byte(pos) == Some(b'\n') { line -= 1; } }
        }
        self.line_mark = (at, line);
        line
    }

    // --- navigation ---------------------------------------------------------------------------------------------
    fn down(&mut self, rows: usize) {
        let last = self.last_page();
        for _ in 0..rows {
            if self.top >= last { break; }
            match self.mode { Mode::Hex => self.top += HEX_ROW, Mode::Text => match self.next_row(self.top) { Some(n) => self.top = n, None => break } }
        }
    }
    fn up(&mut self, rows: usize) {
        for _ in 0..rows {
            match self.mode { Mode::Hex => self.top = self.top.saturating_sub(HEX_ROW), Mode::Text => match self.prev_row(self.top) { Some(p) => self.top = p, None => break } }
        }
    }
    fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.top = match mode { Mode::Hex => self.top & !(HEX_ROW - 1), Mode::Text => self.row_of(self.top) };
    }
    /// Moves the view to `offset`: hex row, or the text row containing it.
    pub fn jump(&mut self, offset: u64) {
        let offset = offset.min(self.size().saturating_sub(1));
        self.top = match self.mode { Mode::Hex => offset & !(HEX_ROW - 1), Mode::Text => self.row_of(offset) };
        let last = self.last_page();
        if self.top > last { self.top = last; }
    }

    /// Next match of the query after `from`, ignoring case (Latin, Cyrillic and other letters); wraps to the start
    /// once. Returns the offset and the length in bytes of the matched text.
    fn search(&mut self, from: u64) -> Option<(u64, usize)> {
        let fold = |ch: char| ch.to_lowercase().next().unwrap_or(ch);
        let text = core::str::from_utf8(&self.query[..self.query_len]).unwrap_or("");
        let mut query = ['\0'; 128]; let mut count = 0;
        for ch in text.chars() { query[count] = fold(ch); count += 1; }
        if count == 0 { return None; }
        let size = self.size();
        let matches = |viewer: &mut Self, at: u64| -> Option<usize> {
            let mut pos = at;
            for &want in &query[..count] {
                let (ch, len) = viewer.cache.char_at(pos)?;
                if fold(ch) != want { return None; }
                pos += len as u64;
            }
            Some((pos - at) as usize)
        };
        let first = query[0];
        for at in (from..size).chain(0..from.min(size)) {
            // Cheap pre-check on the first byte of ASCII queries.
            if first.is_ascii() && self.cache.byte(at).map(|b| b.to_ascii_lowercase()) != Some(first as u8) { continue; }
            if let Some(len) = matches(self, at) { return Some((at, len)); }
        }
        None
    }
    fn search_next(&mut self) {
        let from = self.found.map_or(self.top, |(at, _)| at + 1);
        match self.search(from) {
            Some((at, len)) => { self.found = Some((at, len)); self.jump(at); self.note = None; }
            None => { self.found = None; self.note = Some("Не найдено / Not found"); }
        }
    }
    fn goto(&mut self, text: &str) {
        let text = text.trim();
        let size = self.size();
        let target = if let Some(percent) = text.strip_suffix('%') {
            percent.trim().parse::<u64>().ok().map(|p| size * p.min(100) / 100)
        } else if self.mode == Mode::Hex || text.starts_with("0x") {
            u64::from_str_radix(text.trim_start_matches("0x"), 16).ok()
        } else {
            // Line number: count lines from the start.
            text.parse::<u64>().ok().map(|line| {
                let (mut pos, mut current) = (0u64, 1u64);
                while current < line.max(1) && pos < size { if self.cache.byte(pos) == Some(b'\n') { current += 1; } pos += 1; }
                self.line_mark = (pos, current);
                pos
            })
        };
        match target { Some(offset) => { self.jump(offset); self.note = None; } None => self.note = Some("Неверное значение / Bad value") }
    }

    /// Handles a key; the caller redraws afterwards.
    pub fn key(&mut self, key: Key) -> Action {
        if self.help { self.help = false; return Action::None; }
        if let Some((kind, mut line)) = self.prompt.take() {
            match line.key(key) {
                Edit::Submit => {
                    let mut copy = [0u8; 256]; let text = line.as_str(); let n = text.len(); copy[..n].copy_from_slice(text.as_bytes());
                    let text = core::str::from_utf8(&copy[..n]).unwrap_or("");
                    match kind {
                        Prompt::Search => { let n = n.min(self.query.len()); self.query[..n].copy_from_slice(&text.as_bytes()[..n]); self.query_len = n; self.found = None; self.search_next(); }
                        Prompt::Goto => self.goto(text),
                    }
                }
                Edit::Cancel => {}
                _ => self.prompt = Some((kind, line)),
            }
            return Action::None;
        }
        self.note = None;
        match key.code() {
            Code::Esc | Code::F(10) | Code::F(3) => return Action::Quit,
            Code::Down => self.down(1),
            Code::Up => self.up(1),
            Code::PageDown => self.down(self.height.saturating_sub(1).max(1)),
            Code::Char if key.text() == Some(' ') => self.down(self.height.saturating_sub(1).max(1)),
            Code::PageUp => self.up(self.height.saturating_sub(1).max(1)),
            Code::Home => { self.top = 0; self.left = 0; }
            Code::End => self.top = self.last_page(),
            Code::Left if self.mode == Mode::Text && !self.wrap => self.left = self.left.saturating_sub(8),
            Code::Right if self.mode == Mode::Text && !self.wrap => self.left += 8,
            Code::F(1) => self.help = true,
            Code::F(2) if self.mode == Mode::Text => { self.wrap = !self.wrap; self.left = 0; self.top = self.row_of(self.top); }
            Code::F(4) => self.set_mode(if self.mode == Mode::Text { Mode::Hex } else { Mode::Text }),
            Code::F(5) => self.prompt = Some((Prompt::Goto, InputLine::new())),
            Code::F(7) if key.shift() => self.search_next(),
            Code::F(7) => {
                let mut line = InputLine::new();
                line.set(core::str::from_utf8(&self.query[..self.query_len]).unwrap_or(""));
                self.prompt = Some((Prompt::Search, line));
            }
            _ => {}
        }
        Action::None
    }

    // --- drawing ------------------------------------------------------------------------------------------------
    fn style_at(&self, at: u64, normal: Style, theme: &Theme) -> Style {
        match self.found { Some((start, len)) if at >= start && at < start + len as u64 => theme.selected, _ => normal }
    }

    fn draw_text(&mut self, grid: &mut Grid, area: Rect, theme: &Theme) {
        let mut row_start = Some(self.top);
        for y in 0..area.h {
            let Some(start) = row_start else { break };
            let end = self.row_end(start);
            let (mut at, mut col) = (start, 0usize);
            while at < end {
                let Some((ch, len)) = self.cache.char_at(at) else { break };
                if ch == '\n' { break; }
                let style = self.style_at(at, theme.panel, theme);
                let w = if ch == '\t' { 8 - col % 8 } else { 1 };
                for i in 0..w {
                    let x = col + i;
                    if x >= self.left && x - self.left < area.w {
                        let shown = if ch == '\t' || ch == '\r' { ' ' } else if ch.is_control() { '·' } else { ch };
                        grid.put(area.x + x - self.left, area.y + y, shown, style);
                    }
                }
                col += w; at += len as u64;
            }
            if !self.wrap && col > self.left + area.w { grid.put(area.right() - 1, area.y + y, '»', theme.accent); }
            row_start = (end > start && end < self.size()).then_some(end);
        }
    }

    fn draw_hex(&mut self, grid: &mut Grid, area: Rect, theme: &Theme) {
        for y in 0..area.h {
            let offset = self.top + y as u64 * HEX_ROW;
            if offset >= self.size() { break; }
            let mut text = crate::util::FixedBuf::<16>::new();
            let _ = write!(text, "{:08X}:", offset);
            grid.text(area.x, area.y + y, core::str::from_utf8(text.as_bytes()).unwrap_or(""), theme.dim);
            for i in 0..HEX_ROW {
                let at = offset + i;
                let Some(byte) = self.cache.byte(at) else { break };
                let style = self.style_at(at, theme.panel, theme);
                let x = area.x + 10 + i as usize * 3 + if i >= 8 { 1 } else { 0 };
                let digits = [b"0123456789ABCDEF"[(byte >> 4) as usize], b"0123456789ABCDEF"[(byte & 15) as usize]];
                grid.put(x, area.y + y, digits[0] as char, style); grid.put(x + 1, area.y + y, digits[1] as char, style);
                let ch = if (0x20..0x7F).contains(&byte) { byte as char } else { '·' };
                grid.put(area.x + 10 + 16 * 3 + 2 + i as usize, area.y + y, ch, style);
            }
        }
    }

    /// Draws the viewer in `area` (status line on top, key bar at the bottom); returns the cursor of an open prompt.
    pub fn draw(&mut self, grid: &mut Grid, area: Rect, theme: &Theme) -> Option<(usize, usize)> {
        let body = Rect::new(area.x, area.y + 1, area.w, area.h.saturating_sub(2));
        if self.mode == Mode::Text && (self.width != body.w || self.height != body.h) {
            self.width = body.w; self.height = body.h; self.top = self.row_of(self.top);
        }
        self.width = body.w; self.height = body.h;
        grid.fill(area, ' ', theme.panel);
        match self.mode { Mode::Text => self.draw_text(grid, body, theme), Mode::Hex => self.draw_hex(grid, body, theme) }
        // Status: name, line or offset, percentage, size, mode.
        let size = self.size();
        let percent = if size == 0 { 100 } else { ((self.top + 1) * 100 / size).min(100) };
        let mut status = crate::util::FixedBuf::<160>::new();
        if self.mode == Mode::Text {
            let line = self.line_number(self.top);
            let _ = write!(status, "Стр {} │ ", line);
        }
        let mut grouped = [0u8; 32];
        let _ = write!(status, "{:#X} │ {}% │ {} Б │ {}{} ", self.top, percent, super::grouped(size, &mut grouped),
                       if self.mode == Mode::Hex { "HEX" } else { "UTF-8" }, if self.mode == Mode::Text && self.wrap { " ↵" } else { "" });
        let status = core::str::from_utf8(status.as_bytes()).unwrap_or("");
        grid.fill(Rect::new(area.x, area.y, area.w, 1), ' ', theme.status);
        // The name gets what the status leaves.
        let status_width = status.chars().count().min(area.w);
        let name_width = area.w.saturating_sub(status_width + 2);
        grid.text_right(area.right(), area.y, status, theme.status);
        grid.text_padded(area.x, area.y, " ", 1, theme.status);
        grid.text_max(area.x + 1, area.y, self.name(), name_width, theme.status);
        {
            let wrap_label = if self.wrap { "Unwrap" } else { "Wrap" };
            let mode_label = if self.mode == Mode::Hex { "Text" } else { "Hex" };
            let bars = KeyBars { plain: ["Help", if self.mode == Mode::Text { wrap_label } else { "" }, "Quit", mode_label, "Goto", "", "Search", "", "", "Quit"],
                                 shift: ["", "", "", "", "", "", "Next", "", "", ""], ctrl: [""; 10], alt: [""; 10] };
            fkey_bar_at(grid, area, bars.labels(self.modifiers), theme);
        }
        if let Some(note) = self.note { grid.text_padded(area.x, area.bottom() - 2, note, area.w, theme.error); }
        if self.help {
            message(grid, "view", &["↑ ↓ PgUp PgDn Пробел Home End — прокрутка", "← → — сдвиг строки без переноса", "F2 — перенос, F4 — hex/текст",
                                    "F5 — строка, смещение (0x…, hex) или N%", "F7 — поиск, Shift+F7 — дальше", "Esc, F3, F10 — выход"], &["OK"], 0, theme);
        }
        match self.prompt.as_mut() {
            Some((Prompt::Search, line)) => Some(input_dialog(grid, "Поиск / Search", "Текст:", line, 50, theme)),
            Some((Prompt::Goto, line)) => Some(input_dialog(grid, "Переход / Go to", "Строка, 0x смещение или N%:", line, 50, theme)),
            None => None,
        }
    }
}

// The key bar on the last row of `area`.
fn fkey_bar_at(grid: &mut Grid, area: Rect, labels: &[&str], theme: &Theme) {
    if area.x == 0 && area.w == grid.cols { fkey_bar(grid, area.bottom() - 1, labels, theme); }
}
