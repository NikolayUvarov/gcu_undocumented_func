//! The text of `console` (issue u004): what its programs printed and what it says itself, in lines, with the command
//! line under them; wrapped to the grid, scrolled back with PgUp/PgDn or the mouse wheel; ↑ ↓ recall earlier commands.
//! No system calls: tests/console_host.rs.
use crate::keys::{Code, Key};
use crate::tui::widgets::{Edit, InputLine};
use crate::tui::{Grid, Style};
use alloc::string::String;
use alloc::vec::Vec;

pub const LINES_MAX: usize = 2000;
pub const HISTORY: usize = 32;
pub const PROMPT: &str = "> ";

/// Whose a line is: a program's output, a command typed, what `console` says, a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Output, Command, Note, Error }

impl Kind {
    pub fn style(self) -> Style {
        match self {
            Kind::Output => Style::new(0xD0D0D0, BACKGROUND), Kind::Command => Style::new(0xFFFFFF, BACKGROUND),
            Kind::Note => Style::new(0x70A0C0, BACKGROUND), Kind::Error => Style::new(0xF07070, BACKGROUND),
        }
    }
}
pub const BACKGROUND: u32 = 0x101418;

/// What a line typed asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command<'a> { Nothing, Help, Clear, Exit, List, Run { name: &'a str, args: &'a str } }

pub fn parse(line: &str) -> Command<'_> {
    let line = line.trim();
    let (name, args) = line.split_once(char::is_whitespace).map_or((line, ""), |(n, a)| (n, a.trim()));
    match name {
        "" => Command::Nothing,
        "help" | "?" => Command::Help,
        "clear" | "cls" => Command::Clear,
        "exit" | "quit" => Command::Exit,
        "list" | "ls" if args.is_empty() => Command::List,
        _ => Command::Run { name: name.strip_suffix(".elf").unwrap_or(name), args },
    }
}

pub struct Screen {
    pub lines: Vec<(String, Kind)>,
    /// A program's output since its last newline, shown as the last line.
    partial: String,
    /// The start of a character a message split.
    carry: Vec<u8>,
    /// Rows scrolled back from the end.
    pub scroll: usize,
    pub line: InputLine,
    history: Vec<String>,
    recall: Option<usize>,
}

impl Default for Screen { fn default() -> Self { Self::new() } }

impl Screen {
    pub fn new() -> Self { Self { lines: Vec::new(), partial: String::new(), carry: Vec::new(), scroll: 0, line: InputLine::new(), history: Vec::new(), recall: None } }

    fn push(&mut self, text: String, kind: Kind) {
        if self.lines.len() == LINES_MAX { self.lines.remove(0); }
        self.lines.push((text, kind));
    }

    /// Bytes a program printed (UTF-8, perhaps cut in the middle of a character).
    pub fn output(&mut self, bytes: &[u8]) {
        self.carry.extend_from_slice(bytes);
        let carry = core::mem::take(&mut self.carry);
        let mut rest = &carry[..];
        loop {
            match core::str::from_utf8(rest) {
                Ok(text) => { self.chars(text); break; }
                Err(error) => {
                    let (good, after) = rest.split_at(error.valid_up_to());
                    self.chars(core::str::from_utf8(good).unwrap_or(""));
                    match error.error_len() {
                        None => { self.carry = after.to_vec(); break; } // the rest of the character comes next
                        Some(bad) => { self.chars("?"); rest = &after[bad..]; }
                    }
                }
            }
        }
    }

    fn chars(&mut self, text: &str) {
        for ch in text.chars() {
            match ch {
                '\n' => { let line = core::mem::take(&mut self.partial); self.push(line, Kind::Output); }
                '\t' => { let next = (self.partial.chars().count() / 8 + 1) * 8; while self.partial.chars().count() < next { self.partial.push(' '); } }
                c if (c as u32) < 0x20 || c == '\u{7F}' => {} // \r and other controls
                c => self.partial.push(c),
            }
        }
    }

    /// A line of `console`'s own; a program's unfinished line ends first.
    pub fn say(&mut self, text: &str, kind: Kind) {
        if !self.partial.is_empty() { let line = core::mem::take(&mut self.partial); self.push(line, Kind::Output); }
        for line in text.split('\n') { self.push(String::from(line), kind); }
    }

    pub fn clear(&mut self) { self.lines.clear(); self.partial.clear(); self.scroll = 0; }

    /// A key; Enter gives the line typed (it goes into the history and is shown after the prompt). `page`: the rows
    /// PgUp and PgDn scroll.
    pub fn key(&mut self, key: Key, page: usize) -> Option<String> {
        match key.code() {
            Code::Up | Code::Down => {
                if self.history.is_empty() { return None; }
                let last = self.history.len() - 1;
                self.recall = match (self.recall, key.code()) {
                    (None, Code::Up) => Some(last), (None, _) => None,
                    (Some(i), Code::Up) => Some(i.saturating_sub(1)), (Some(i), _) if i < last => Some(i + 1), _ => None,
                };
                match self.recall { Some(i) => { let text = self.history[i].clone(); self.line.set(&text); } None => self.line.clear() }
                None
            }
            Code::PageUp => { self.scroll += page.max(1); None }
            Code::PageDown => { self.scroll = self.scroll.saturating_sub(page.max(1)); None }
            Code::Esc => { self.line.clear(); self.recall = None; None }
            _ if key.is_ctrl('l') => { self.clear(); None }
            _ => match self.line.key(key) {
                Edit::Submit => {
                    let text = String::from(self.line.as_str());
                    self.line.clear();
                    self.recall = None;
                    self.scroll = 0;
                    self.say(&alloc::format!("{}{}", PROMPT, text), Kind::Command);
                    if !text.trim().is_empty() && self.history.last() != Some(&text) {
                        if self.history.len() == HISTORY { self.history.remove(0); }
                        self.history.push(text.clone());
                    }
                    Some(text)
                }
                _ => { self.scroll = 0; None }
            },
        }
    }

    /// The wheel turned `steps` (negative: up): three rows a step.
    pub fn wheel(&mut self, steps: i32) {
        if steps < 0 { self.scroll += 3 * steps.unsigned_abs() as usize; } else { self.scroll = self.scroll.saturating_sub(3 * steps as usize); }
    }

    /// Every line cut to `width` columns, the program's unfinished line last.
    pub fn rows(&self, width: usize) -> Vec<(String, Kind)> {
        let width = width.max(1);
        let mut rows = Vec::new();
        for (text, kind) in self.lines.iter().map(|(t, k)| (t.as_str(), *k)).chain((!self.partial.is_empty()).then_some((self.partial.as_str(), Kind::Output))) {
            let chars: Vec<char> = text.chars().collect();
            if chars.is_empty() { rows.push((String::new(), kind)); continue; }
            for piece in chars.chunks(width) { rows.push((piece.iter().collect(), kind)); }
        }
        rows
    }

    /// Draws the lines above the command line; returns the cursor. `busy`: what runs, shown at the right of the
    /// command line.
    pub fn draw(&mut self, grid: &mut Grid, busy: &str) -> Option<(usize, usize)> {
        let (w, h) = (grid.cols, grid.rows);
        grid.clear(Kind::Output.style());
        if h == 0 { return None; }
        let rows = self.rows(w);
        let view = h - 1;
        self.scroll = self.scroll.min(rows.len().saturating_sub(view));
        let end = rows.len() - self.scroll;
        for (y, (text, kind)) in rows[end.saturating_sub(view)..end].iter().enumerate() { grid.text_max(0, y, text, w, kind.style()); }
        if self.scroll > 0 { grid.text_right(w, 0, &alloc::format!(" ↑ {} more below ", self.scroll), Kind::Note.style()); }
        let y = h - 1;
        let note = if busy.is_empty() { String::new() } else { alloc::format!(" {} ", busy) };
        let room = w.saturating_sub(note.chars().count());
        grid.text_max(0, y, PROMPT, room, Kind::Command.style());
        let column = self.line.draw(grid, PROMPT.len().min(room), y, room.saturating_sub(PROMPT.len()), Kind::Command.style());
        if !note.is_empty() { grid.text_right(w, y, &note, Kind::Note.style()); }
        Some((column, y))
    }
}
