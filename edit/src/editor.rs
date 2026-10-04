//! The editor: cursor and selection over a `Buffer`, keys (docs/tools §4.2), dialogs and drawing. It does no I/O:
//! saving is asked for with `Outcome::Save`, and the program reports back with `saved`.
use crate::buffer::{decode, Buffer};
use crate::keys::{Code, Key};
use crate::tui::widgets::{fkey_bar, input_dialog, message, Edit, InputLine, KeyBars, MenuAction, MenuBar};
use crate::tui::{Grid, Rect, Style, Theme};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// What a key asks of the program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome { Redraw, Ignored, Quit, Save(String), SaveAndQuit(String) }

pub enum Dialog { Find(InputLine), Replace { find: InputLine, with: InputLine, second: bool }, Goto(InputLine), SaveAs(InputLine, bool), Unsaved(usize), Help }

const TITLES: [&str; 4] = ["File", "Edit", "Search", "Options"];
const FILE_ITEMS: [&str; 3] = ["Save  F2", "Save as  Shift+F2", "Quit  F10"];
const EDIT_ITEMS: [&str; 6] = ["Undo  Ctrl+U", "Redo  Ctrl+Y", "Cut  Ctrl+X", "Copy  Ctrl+C", "Paste  Ctrl+V", "Select all  Ctrl+A"];
const SEARCH_ITEMS: [&str; 4] = ["Find  F7", "Find next  Shift+F7", "Replace  Ctrl+F7", "Go to line  Alt+F8"];
const OPTION_ITEMS: [&str; 2] = ["Tab width 4 / 8", "Insert / overwrite  Ins"];
const ITEMS: [&[&str]; 4] = [&FILE_ITEMS, &EDIT_ITEMS, &SEARCH_ITEMS, &OPTION_ITEMS];
/// What a read-only editor says when it opens and when a key would change the text.
pub const READ_ONLY: &str = "READ-ONLY: this file cannot be changed here (Shift+F2 saves a copy where you may write)";
const KEYS: KeyBars<'static> = KeyBars { plain: ["Help", "Save", "", "", "", "", "Search", "", "Menu", "Quit"], shift: ["", "Save as", "", "", "", "", "Next", "", "", ""],
                                         ctrl: ["", "", "", "", "", "", "Replace", "", "", ""], alt: ["", "", "", "", "", "", "", "Go to", "", ""] };
const HELP: [&str; 9] = [
    "Arrows, Home/End, PgUp/PgDn; Ctrl+Home/End: text; Ctrl+←/→: words",
    "Shift + movement selects; Ctrl+A: all; Ctrl+C/X/V: copy, cut, paste",
    "Ctrl+U or Alt+Backspace: undo; Ctrl+Y: redo; Ins: overwrite",
    "F2: save; Shift+F2: save as; F10 or Esc: quit",
    "F7: find; Shift+F7: next; Ctrl+F7: replace all; Alt+F8: go to line",
    "F9: menu; F1: these keys",
    "Line endings (LF or CRLF) and invalid UTF-8 are kept as they are.",
    "Saving writes name.tmp and renames it over the file.",
    "",
];

pub struct Editor {
    pub buffer: Buffer,
    pub cursor: usize,
    pub anchor: Option<usize>,
    goal: Option<usize>, // display column kept by up/down
    pub top: usize,
    pub left: usize,
    pub path: String,
    pub read_only: bool,
    pub overwrite: bool,
    pub tab: usize,
    clipboard: Vec<u8>,
    pub menu: MenuBar<'static>,
    pub dialog: Option<Dialog>,
    pub notice: Option<String>,
    query: String,
    height: usize,
    width: usize,
    /// The modifiers held (MOD_*, `mind::input::modifiers`): the key bar shows what the keys do with them.
    pub modifiers: u8,
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }

impl Editor {
    /// A read-only editor says so at once (and READ-ONLY stays in the status line).
    pub fn new(text: Vec<u8>, path: &str, read_only: bool) -> Self {
        let notice = read_only.then(|| String::from(READ_ONLY));
        Self { buffer: Buffer::new(text), cursor: 0, anchor: None, goal: None, top: 0, left: 0, path: String::from(path), read_only, overwrite: false, tab: 4,
               clipboard: Vec::new(), menu: MenuBar::new(&TITLES, &ITEMS), dialog: None, notice, query: String::new(), height: 20, width: 80, modifiers: 0 }
    }

    pub fn line(&self) -> usize { self.buffer.line_of(self.cursor) }

    /// Display column of byte offset `at` on its line (tabs expanded).
    pub fn column_of(&self, at: usize) -> usize {
        let start = self.buffer.line_start(self.buffer.line_of(at));
        let bytes = self.buffer.bytes(start, at);
        let mut column = 0; let mut i = 0;
        while i < bytes.len() { let (c, n) = decode(&bytes, i); column = if c == '\t' { (column / self.tab + 1) * self.tab } else { column + 1 }; i += n; }
        column
    }

    // The byte offset on `line` at display column `column` (or the line end).
    fn offset_at(&self, line: usize, column: usize) -> usize {
        let (start, end) = (self.buffer.line_start(line), self.buffer.line_end(line));
        let bytes = self.buffer.bytes(start, end);
        let mut at = 0; let mut col = 0;
        while at < bytes.len() {
            let (c, n) = decode(&bytes, at);
            let next = if c == '\t' { (col / self.tab + 1) * self.tab } else { col + 1 };
            if next > column { break; }
            col = next; at += n;
        }
        start + at
    }

    pub fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        (anchor != self.cursor).then(|| (anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    fn move_to(&mut self, at: usize, select: bool, keep_goal: bool) {
        if select { self.anchor.get_or_insert(self.cursor); } else { self.anchor = None; }
        if !keep_goal { self.goal = None; }
        self.cursor = at.min(self.buffer.len());
        self.buffer.close_group();
    }

    fn word_left(&self) -> usize {
        let mut at = self.cursor;
        while at > 0 { let p = self.buffer.previous(at); if self.buffer.char_at(p).is_some_and(|(c, _)| is_word(c)) { break; } at = p; }
        while at > 0 { let p = self.buffer.previous(at); if !self.buffer.char_at(p).is_some_and(|(c, _)| is_word(c)) { break; } at = p; }
        at
    }

    fn word_right(&self) -> usize {
        let mut at = self.cursor;
        let len = self.buffer.len();
        while at < len && self.buffer.char_at(at).is_some_and(|(c, _)| is_word(c)) { at = self.buffer.next(at); }
        while at < len && !self.buffer.char_at(at).is_some_and(|(c, _)| is_word(c)) { at = self.buffer.next(at); }
        at
    }

    fn vertical(&mut self, lines: isize, select: bool) {
        let goal = self.goal.unwrap_or_else(|| self.column_of(self.cursor));
        let line = (self.line() as isize + lines).clamp(0, self.buffer.line_count() as isize - 1) as usize;
        let at = self.offset_at(line, goal);
        self.move_to(at, select, true);
        self.goal = Some(goal);
    }

    // Changes need a writable file.
    fn writable(&mut self) -> bool {
        if self.read_only { self.notice = Some(String::from(READ_ONLY)); }
        !self.read_only
    }

    fn delete_selection(&mut self) -> bool {
        match self.selection() {
            Some((start, end)) => { self.buffer.close_group(); self.buffer.delete(start, end, false); self.cursor = start; self.anchor = None; true }
            None => { self.anchor = None; false }
        }
    }

    // Puts `text` at the cursor, in place of the selection if there is one (one step for undo), and moves past it;
    // nothing happens if the text would grow over the limit.
    fn put(&mut self, text: &[u8], typing: bool) {
        let before = self.buffer.len();
        match self.selection() {
            Some((start, end)) => { self.buffer.replace(start, end, text); if self.buffer.len() + (end - start) != before + text.len() { return self.too_large(); } self.cursor = start + text.len(); }
            None => { self.buffer.insert(self.cursor, text, typing); if self.buffer.len() == before { return self.too_large(); } self.cursor += text.len(); }
        }
        self.anchor = None;
        self.goal = None;
    }

    fn too_large(&mut self) { self.notice = Some(String::from("The text would be larger than 8 MiB")); }

    /// Types `text` at the cursor (replacing the selection; in overwrite mode the character under the cursor).
    pub fn type_text(&mut self, text: &str) {
        if !self.writable() { return; }
        if self.overwrite && self.selection().is_none() && self.cursor < self.buffer.line_end(self.line()) {
            let next = self.buffer.next(self.cursor);
            self.buffer.delete(self.cursor, next, true);
        }
        self.put(text.as_bytes(), true);
    }

    fn newline(&mut self) {
        if !self.writable() { return; }
        // The line ending of the file, and the indentation of the current line.
        let at = self.selection().map_or(self.cursor, |(start, _)| start);
        let start = self.buffer.line_start(self.buffer.line_of(at));
        let indent: Vec<u8> = self.buffer.bytes(start, at).into_iter().take_while(|&b| b == b' ' || b == b'\t').collect();
        let mut text = Vec::from(if self.buffer.crlf() { &b"\r\n"[..] } else { &b"\n"[..] });
        text.extend(indent);
        self.buffer.close_group();
        self.put(&text, false);
    }

    // One step right or left; a CR LF is one step.
    fn right(&self, at: usize) -> usize {
        if self.buffer.byte(at) == Some(b'\r') && self.buffer.byte(at + 1) == Some(b'\n') { at + 2 } else { self.buffer.next(at) }
    }
    fn left(&self, at: usize) -> usize {
        let previous = self.buffer.previous(at);
        if self.buffer.byte(previous) == Some(b'\n') && previous > 0 && self.buffer.byte(previous - 1) == Some(b'\r') { previous - 1 } else { previous }
    }

    fn backspace(&mut self) {
        if !self.writable() || self.delete_selection() { return; }
        if self.cursor == 0 { return; }
        let start = self.left(self.cursor);
        self.buffer.delete(start, self.cursor, true);
        self.cursor = start; self.goal = None;
    }

    fn delete(&mut self) {
        if !self.writable() || self.delete_selection() { return; }
        if self.cursor >= self.buffer.len() { return; }
        let end = self.right(self.cursor);
        self.buffer.delete(self.cursor, end, true);
        self.goal = None;
    }

    fn copy(&mut self) -> bool {
        match self.selection() { Some((s, e)) => { self.clipboard = self.buffer.bytes(s, e); true } None => false }
    }

    fn paste(&mut self) {
        if !self.writable() || self.clipboard.is_empty() { return; }
        self.buffer.close_group();
        let text = self.clipboard.clone();
        self.put(&text, false);
    }

    fn find_next(&mut self) {
        if self.query.is_empty() { self.dialog = Some(Dialog::Find(InputLine::new())); return; }
        let from = self.selection().map_or(self.cursor, |(_, end)| end);
        match self.buffer.find(&self.query, from) {
            Some((start, end)) => { self.anchor = Some(start); self.cursor = end; self.goal = None; }
            None => self.notice = Some(format!("Not found: {}", self.query)),
        }
    }

    /// The file was saved (or not) as `path`.
    pub fn saved(&mut self, path: &str, result: Result<usize, String>) {
        match result {
            Ok(bytes) => { self.buffer.mark_saved(); self.path = String::from(path); self.read_only = false; self.notice = Some(format!("Saved {} bytes", bytes)); }
            Err(error) => self.notice = Some(format!("Not saved: {}", error)),
        }
    }

    fn save(&mut self) -> Outcome {
        if self.path.is_empty() { self.dialog = Some(Dialog::SaveAs(InputLine::new(), false)); return Outcome::Redraw; }
        Outcome::Save(self.path.clone())
    }

    fn quit(&mut self) -> Outcome {
        if self.buffer.modified() { self.dialog = Some(Dialog::Unsaved(0)); Outcome::Redraw } else { Outcome::Quit }
    }

    fn command(&mut self, menu: usize, item: usize) -> Outcome {
        match (menu, item) {
            (0, 0) => return self.save(),
            (0, 1) => { let mut line = InputLine::new(); line.set(&self.path.clone()); self.dialog = Some(Dialog::SaveAs(line, false)); }
            (0, _) => return self.quit(),
            (1, 0) => if let Some(at) = self.buffer.undo() { self.cursor = at.min(self.buffer.len()); self.anchor = None; },
            (1, 1) => if let Some(at) = self.buffer.redo() { self.cursor = at.min(self.buffer.len()); self.anchor = None; },
            (1, 2) => { if self.copy() && self.writable() { self.delete_selection(); } }
            (1, 3) => { self.copy(); }
            (1, 4) => self.paste(),
            (1, _) => { self.anchor = Some(0); self.cursor = self.buffer.len(); }
            (2, 0) => { let mut line = InputLine::new(); line.set(&self.query.clone()); self.dialog = Some(Dialog::Find(line)); }
            (2, 1) => self.find_next(),
            (2, 2) => { let mut find = InputLine::new(); find.set(&self.query.clone()); self.dialog = Some(Dialog::Replace { find, with: InputLine::new(), second: false }); }
            (2, _) => self.dialog = Some(Dialog::Goto(InputLine::new())),
            (3, 0) => self.tab = if self.tab == 4 { 8 } else { 4 },
            _ => self.overwrite = !self.overwrite,
        }
        Outcome::Redraw
    }

    fn dialog_key(&mut self, key: Key) -> Outcome {
        let Some(mut dialog) = self.dialog.take() else { return Outcome::Ignored };
        let keep = match &mut dialog {
            Dialog::Help => !matches!(key.code(), Code::Esc | Code::Enter | Code::F(1) | Code::F(10)),
            Dialog::Find(line) => match line.key(key) {
                Edit::Submit => { self.query = String::from(line.as_str()); self.find_next(); false }
                Edit::Cancel => false,
                _ => true,
            },
            Dialog::Goto(line) => match line.key(key) {
                Edit::Submit => {
                    match line.as_str().trim().parse::<usize>() {
                        Ok(n) if n >= 1 => { let at = self.buffer.line_start(n - 1); self.move_to(at, false, false); }
                        _ => self.notice = Some(String::from("A line number, please")),
                    }
                    false
                }
                Edit::Cancel => false,
                _ => true,
            },
            Dialog::Replace { find, with, second } => {
                let line = if *second { &mut *with } else { &mut *find };
                match line.key(key) {
                    Edit::Submit if !*second => { *second = true; true }
                    Edit::Submit => {
                        let needle = String::from(find.as_str());
                        if needle.is_empty() || !self.writable() { return Outcome::Redraw; }
                        let ranges = self.buffer.find_all(&needle);
                        self.buffer.replace_all(&ranges, with.as_str().as_bytes());
                        self.query = needle;
                        self.cursor = self.cursor.min(self.buffer.len()); self.anchor = None;
                        self.notice = Some(format!("Replaced {}", ranges.len()));
                        false
                    }
                    Edit::Cancel => false,
                    _ => true,
                }
            }
            Dialog::SaveAs(line, then_quit) => match line.key(key) {
                Edit::Submit => {
                    let path = String::from(line.as_str().trim());
                    if path.is_empty() { return Outcome::Redraw; }
                    return if *then_quit { Outcome::SaveAndQuit(path) } else { Outcome::Save(path) };
                }
                Edit::Cancel => false,
                _ => true,
            },
            Dialog::Unsaved(selected) => {
                let answer = match key.code() {
                    Code::Left => { *selected = (*selected + 2) % 3; None }
                    Code::Right | Code::Tab => { *selected = (*selected + 1) % 3; None }
                    Code::Enter => Some(*selected),
                    Code::Esc => Some(2),
                    _ => None,
                };
                match answer {
                    Some(0) if self.path.is_empty() => { self.dialog = Some(Dialog::SaveAs(InputLine::new(), true)); return Outcome::Redraw; }
                    Some(0) => return Outcome::SaveAndQuit(self.path.clone()),
                    Some(1) => return Outcome::Quit,
                    Some(_) => false,
                    None => true,
                }
            }
        };
        if keep && self.dialog.is_none() { self.dialog = Some(dialog); }
        Outcome::Redraw
    }

    pub fn key(&mut self, key: Key) -> Outcome {
        if self.dialog.is_some() { return self.dialog_key(key); }
        if self.menu.open {
            if let MenuAction::Chosen(menu, item) = self.menu.key(key) { return self.command(menu, item); }
            return Outcome::Redraw;
        }
        self.notice = None;
        let (shift, ctrl, alt) = (key.shift(), key.ctrl(), key.alt());
        let page = self.height.max(2) as isize - 1;
        match key.code() {
            Code::Left if ctrl => { let at = self.word_left(); self.move_to(at, shift, false); }
            Code::Right if ctrl => { let at = self.word_right(); self.move_to(at, shift, false); }
            Code::Left => { let at = match self.selection() { Some((start, _)) if !shift => start, _ => self.left(self.cursor) }; self.move_to(at, shift, false); }
            Code::Right => { let at = match self.selection() { Some((_, end)) if !shift => end, _ => self.right(self.cursor) }; self.move_to(at, shift, false); }
            Code::Up => self.vertical(-1, shift),
            Code::Down => self.vertical(1, shift),
            Code::PageUp => { self.vertical(-page, shift); self.top = self.top.saturating_sub(page as usize); }
            Code::PageDown => { self.vertical(page, shift); self.top += page as usize; }
            Code::Home if ctrl => self.move_to(0, shift, false),
            Code::End if ctrl => { let len = self.buffer.len(); self.move_to(len, shift, false); }
            Code::Home => {
                // First to the indentation's end, then to the line start.
                let start = self.buffer.line_start(self.line());
                let bytes = self.buffer.bytes(start, self.buffer.line_end(self.line()));
                let indent = start + bytes.iter().take_while(|&&b| b == b' ' || b == b'\t').count();
                let at = if self.cursor == indent { start } else { indent };
                self.move_to(at, shift, false);
            }
            Code::End => { let at = self.buffer.line_end(self.line()); self.move_to(at, shift, false); }
            Code::Enter => self.newline(),
            Code::Backspace if alt => { if let Some(at) = self.buffer.undo() { self.cursor = at.min(self.buffer.len()); self.anchor = None; } }
            Code::Backspace => self.backspace(),
            Code::Delete => self.delete(),
            Code::Tab => self.type_text("\t"),
            Code::Insert => self.overwrite = !self.overwrite,
            Code::Esc if self.anchor.is_some() => self.anchor = None,
            Code::Esc | Code::F(10) => return self.quit(),
            Code::F(1) => self.dialog = Some(Dialog::Help),
            Code::F(2) if shift => return self.command(0, 1),
            Code::F(2) => return self.save(),
            Code::F(7) if ctrl => return self.command(2, 2),
            Code::F(7) if shift => self.find_next(),
            Code::F(7) => return self.command(2, 0),
            Code::F(8) if alt => return self.command(2, 3),
            Code::F(9) => { self.menu.open = true; self.menu.menu = 0; self.menu.item = 0; }
            Code::Char if ctrl => match key.char().map(|c| c.to_ascii_lowercase()) {
                Some('c') => { self.copy(); }
                Some('x') => { if self.copy() && self.writable() { self.delete_selection(); } }
                Some('v') => self.paste(),
                Some('a') => { self.anchor = Some(0); self.cursor = self.buffer.len(); }
                Some('u') => return self.command(1, 0),
                Some('y') => return self.command(1, 1),
                _ => return Outcome::Ignored,
            },
            Code::Char => match key.text() { Some(c) => { let mut b = [0u8; 4]; self.type_text(c.encode_utf8(&mut b)); } None => return Outcome::Ignored },
            _ => return Outcome::Ignored,
        }
        Outcome::Redraw
    }

    /// Draws the editor; returns the cursor position for the terminal.
    pub fn draw(&mut self, grid: &mut Grid, theme: &Theme) -> Option<(usize, usize)> {
        let (w, h) = (grid.cols, grid.rows);
        grid.clear(theme.panel);
        self.height = h.saturating_sub(2).max(1);
        self.width = w.max(1);
        // Keep the cursor visible.
        let line = self.line();
        if line < self.top { self.top = line; }
        if line >= self.top + self.height { self.top = line + 1 - self.height; }
        self.top = self.top.min(self.buffer.line_count().saturating_sub(1));
        let column = self.column_of(self.cursor);
        if column < self.left { self.left = column; }
        if column >= self.left + self.width { self.left = column + 1 - self.width; }
        let selection = self.selection();
        for row in 0..self.height {
            let line = self.top + row;
            if line >= self.buffer.line_count() { break; }
            let start = self.buffer.line_start(line);
            let bytes = self.buffer.bytes(start, self.buffer.line_end(line));
            let (mut at, mut col) = (0usize, 0usize);
            while at < bytes.len() && col < self.left + self.width {
                let (c, n) = decode(&bytes, at);
                let next = if c == '\t' { (col / self.tab + 1) * self.tab } else { col + 1 };
                let selected = selection.is_some_and(|(s, e)| start + at >= s && start + at < e);
                let style = if selected { theme.selected } else if c == '\u{FFFD}' { theme.error } else { theme.panel };
                for x in col..next {
                    if x >= self.left && x < self.left + self.width {
                        let ch = if c == '\t' || x > col { ' ' } else if c.is_control() { '\u{FFFD}' } else { c };
                        grid.put(x - self.left, 1 + row, ch, style);
                    }
                }
                col = next; at += n;
            }
            // A selected line ending shows as one selected cell.
            if selection.is_some_and(|(s, e)| start + bytes.len() >= s && start + bytes.len() < e) && col >= self.left && col < self.left + self.width {
                grid.put(col - self.left, 1 + row, ' ', theme.selected);
            }
        }
        // Status: name, position, state.
        grid.fill(Rect::new(0, 0, w, 1), ' ', theme.status);
        let state = format!("{}Ln {} Col {}{}{}{}  UTF-8 {}", if self.read_only { "READ-ONLY  " } else { "" }, line + 1, column + 1, if self.buffer.modified() { "  *" } else { "" },
                            if self.overwrite { "  OVR" } else { "  INS" }, if self.tab == 8 { "  TAB 8" } else { "" }, if self.buffer.crlf() { "CRLF" } else { "LF" });
        let name = if self.path.is_empty() { "(new)" } else { self.path.as_str() };
        let state_width = state.chars().count() + 1;
        grid.text_right(w, 0, &format!("{} ", state), theme.status);
        // READ-ONLY stands out in the status line for as long as the file is open.
        if self.read_only && state_width <= w { grid.text_padded(w - state_width, 0, "READ-ONLY", 9, theme.error); }
        grid.text_max(1, 0, name, w.saturating_sub(state_width + 2), theme.status);
        fkey_bar(grid, h - 1, KEYS.labels(self.modifiers), theme);
        // A notice covers the key bar until the next key, unless a modifier is held to see what the keys do.
        if let Some(notice) = self.notice.as_ref().filter(|_| self.modifiers == 0) { grid.text_padded(0, h - 1, notice, w, theme.error); }
        if self.menu.open { self.menu.draw(grid, 0, theme); }
        let cursor = Some((column - self.left, 1 + line - self.top));
        match self.dialog.as_mut() {
            None => if self.menu.open { None } else { cursor },
            Some(Dialog::Help) => { message(grid, "edit — keys", &HELP[..HELP.len() - 1], &["OK"], 0, theme); None }
            Some(Dialog::Find(line)) => Some(input_dialog(grid, "Find", "Text (case ignored):", line, 60, theme)),
            Some(Dialog::Goto(line)) => Some(input_dialog(grid, "Go to line", "Line number:", line, 40, theme)),
            Some(Dialog::Replace { find, with, second }) => {
                if *second { Some(input_dialog(grid, "Replace all", "Replace with:", with, 60, theme)) } else { Some(input_dialog(grid, "Replace all", "Find (case ignored):", find, 60, theme)) }
            }
            Some(Dialog::SaveAs(line, _)) => Some(input_dialog(grid, "Save as", "File (ram:..., data/...):", line, 60, theme)),
            Some(Dialog::Unsaved(selected)) => {
                let lines = [String::from("The text has changed."), format!("Save {}?", if self.path.is_empty() { "it" } else { self.path.as_str() })];
                message(grid, "edit", &[lines[0].as_str(), lines[1].as_str()], &["Save", "Don't save", "Cancel"], *selected, theme);
                None
            }
        }
    }

    /// One line of state for the log after each key (tests follow it).
    pub fn status(&self) -> String {
        let dialog = match self.dialog { None => "NONE", Some(Dialog::Find(_)) => "FIND", Some(Dialog::Replace { .. }) => "REPLACE", Some(Dialog::Goto(_)) => "GOTO",
                                         Some(Dialog::SaveAs(..)) => "SAVEAS", Some(Dialog::Unsaved(_)) => "UNSAVED", Some(Dialog::Help) => "HELP" };
        format!("LINE={} COL={} BYTES={} LINES={} MODIFIED={} DIALOG={} MENU={}", self.line() + 1, self.column_of(self.cursor) + 1, self.buffer.len(), self.buffer.line_count(),
                self.buffer.modified() as u8, dialog, self.menu.open as u8)
    }

    /// Style of `Style` for the tests.
    pub fn style_at(grid: &Grid, x: usize, y: usize) -> Style { grid.get(x, y).style }
}
