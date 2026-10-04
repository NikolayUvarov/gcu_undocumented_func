//! The file manager: two panels, the built-in viewer, quick view, information, find, the menu and the key bar
//! (docs/tools §4.1, phase 1: read-only). The disk is reached through `Disk` (vfs_server in the system, memory in tests).
use crate::abi::*;
use crate::keys::{Code, Key};
use crate::panel::{self, join, matches, parent, Entry, Mode, Panel, Sort};
use crate::tui::viewer::{Action, Source, Viewer};
use crate::tui::widgets::{dialog, fkey_bar, input_dialog, message, Edit, InputLine, ListState, MenuAction, MenuBar};
use crate::tui::{Grid, Line, Rect, Theme};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// The file system as the file manager sees it.
pub trait Disk {
    /// Entries of a directory (`""` is the root).
    fn list(&mut self, path: &str) -> Result<Vec<Entry>, String>;
    /// A file to read.
    fn open(&mut self, path: &str) -> Option<Box<dyn Source>>;
    /// Starts a program (in the background); returns its PID.
    fn run(&mut self, path: &str) -> Result<u64, String>;
}

/// A boxed source as a `Source`.
pub struct Boxed(pub Box<dyn Source>);
impl Source for Boxed {
    fn size(&self) -> u64 { self.0.size() }
    fn read(&mut self, offset: u64, out: &mut [u8]) -> usize { self.0.read(offset, out) }
}

/// The first bytes of a file, for quick view.
struct Bytes<'a>(&'a [u8]);
impl Source for Bytes<'_> {
    fn size(&self) -> u64 { self.0.len() as u64 }
    fn read(&mut self, offset: u64, out: &mut [u8]) -> usize {
        let start = (offset as usize).min(self.0.len());
        let n = out.len().min(self.0.len() - start);
        out[..n].copy_from_slice(&self.0[start..start + n]);
        n
    }
}

const PREVIEW: usize = 8 * 1024;
const FIND_MAX: usize = 500;
const VOLUMES: [&str; 1] = ["A: boot disk (FAT through vfs_server, read-only)"];

const MENU_TITLES: [&str; 5] = ["Left", "Files", "Commands", "Options", "Right"];
const PANEL_ITEMS: [&str; 11] = ["Brief", "Full", "Info", "Quick view", "Name", "Extension", "Time", "Size", "Reverse order", "Reread  Ctrl+R", "Volume  Alt+F1/F2"];
const FILES_ITEMS: [&str; 5] = ["View  F3", "Run or open  Enter", "Select group  +", "Unselect group  -", "Invert selection  *"];
const COMMAND_ITEMS: [&str; 2] = ["Find file  Alt+F7", "Swap panels  Ctrl+U"];
const OPTION_ITEMS: [&str; 1] = ["Hidden and system files  Ctrl+H"];
const MENU_ITEMS: [&[&str]; 5] = [&PANEL_ITEMS, &FILES_ITEMS, &COMMAND_ITEMS, &OPTION_ITEMS, &PANEL_ITEMS];

const HELP: [&str; 12] = [
    "Tab — other panel; Enter — open a directory, run a program, view a file",
    "Backspace — parent directory; ↑ ↓ PgUp PgDn Home End (← → in brief mode)",
    "F3 — view; F9 — menu; F10 or Esc — quit",
    "Ins — mark; + / - — mark or unmark by mask; * — invert",
    "Ctrl+F3 / F4 / F5 / F6 — sort by name / extension / time / size",
    "Ctrl+H — hidden files; Ctrl+R — reread; Ctrl+U — swap panels",
    "Ctrl+L — information, Ctrl+Q — quick view in the other panel",
    "Alt+F1 / Alt+F2 — volume of the left / right panel; Alt+F7 — find",
    "Programs started here run in the background: FG <pid> in the shell",
    "shows them. F4 (edit) and F5–F8 (copy, move, mkdir, delete) come",
    "with the editor and VFS v2.",
    "",
];

pub enum Dialog {
    Help,
    Message { title: String, lines: Vec<String> },
    Mask { select: bool, line: InputLine },
    Find { line: InputLine },
    Results { mask: String, found: Vec<String>, list: ListState },
    Volume { side: usize, list: ListState },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome { Quit, Redraw, Ignored }

pub struct Fm<'b> {
    pub panels: [Panel; 2],
    pub active: usize,
    pub menu: MenuBar<'static>,
    pub dialog: Option<Dialog>,
    pub notice: Option<String>,
    viewer: Option<Viewer<'b, Boxed>>,
    window: Option<&'b mut [u8]>,
    quick: Vec<u8>,
    preview: Option<(String, Vec<u8>, u64)>, // name, first bytes, size: what quick view shows
}

impl<'b> Fm<'b> {
    /// Both panels on the root; `window` is the viewer's buffer (64 KiB is plenty).
    pub fn new(window: &'b mut [u8], disk: &mut dyn Disk) -> Self {
        let mut fm = Self { panels: [Panel::new(Mode::Full), Panel::new(Mode::Brief)], active: 0, menu: MenuBar::new(&MENU_TITLES, &MENU_ITEMS), dialog: None, notice: None,
                            viewer: None, window: Some(window), quick: vec![0; PREVIEW], preview: None };
        fm.load(0, "", None, disk);
        fm.load(1, "", None, disk);
        fm
    }

    pub fn viewing(&self) -> bool { self.viewer.is_some() }

    /// Lists `path` into a panel; on an error the panel keeps its listing and shows the error.
    pub fn load(&mut self, side: usize, path: &str, focus: Option<&str>, disk: &mut dyn Disk) {
        match disk.list(path) {
            Ok(entries) => self.panels[side].set(path, entries, focus),
            Err(error) => {
                let text = format!("{}: {}", if path.is_empty() { "/" } else { path }, error);
                self.notice = Some(format!("Cannot list {}", text));
                self.panels[side].error = Some(text);
            }
        }
        self.update_preview(disk);
    }

    fn reread(&mut self, side: usize, disk: &mut dyn Disk) { let path = self.panels[side].path.clone(); self.load(side, &path, None, disk); }

    // Quick view shows the file under the cursor of the active panel in the other one.
    fn update_preview(&mut self, disk: &mut dyn Disk) {
        self.preview = None;
        if self.panels[1 - self.active].mode != Mode::Quick { return; }
        let panel = &self.panels[self.active];
        let Some(entry) = panel.current().filter(|e| !e.dir) else { return };
        let path = join(&panel.path, &entry.name);
        if let Some(mut source) = disk.open(&path) {
            let size = source.size();
            let mut bytes = vec![0u8; (size as usize).min(PREVIEW)];
            let n = source.read(0, &mut bytes);
            bytes.truncate(n);
            self.preview = Some((entry.name.clone(), bytes, size));
        }
    }

    fn current_path(&self) -> Option<(String, Entry)> {
        let panel = &self.panels[self.active];
        panel.current().map(|e| (join(&panel.path, &e.name), e.clone()))
    }

    fn view(&mut self, path: &str, disk: &mut dyn Disk) {
        let Some(source) = disk.open(path) else { self.notice = Some(format!("Cannot open {}", path)); return };
        let Some(window) = self.window.take() else { return };
        self.viewer = Some(Viewer::new(Boxed(source), window, path));
    }

    fn up(&mut self, disk: &mut dyn Disk) {
        let side = self.active;
        if self.panels[side].path.is_empty() { return; }
        let (up, name) = parent(&self.panels[side].path);
        self.load(side, &up, Some(&name), disk);
    }

    /// Enter: a directory is opened, a program started, any other file viewed.
    fn open(&mut self, disk: &mut dyn Disk) {
        let Some((path, entry)) = self.current_path() else { return };
        if entry.is_up() { self.up(disk); }
        else if entry.dir { self.load(self.active, &path, None, disk); }
        else if entry.is_program() {
            self.notice = Some(match disk.run(&path) {
                Ok(pid) => format!("Started {} as PID {} in the background; FG {} in the shell shows it", entry.name, pid, pid),
                Err(error) => format!("Cannot start {}: {}", entry.name, error),
            });
        } else { self.view(&path, disk); }
    }

    // Recursive search from `path` for names matching `mask`.
    fn find(&self, path: &str, mask: &str, depth: usize, disk: &mut dyn Disk, found: &mut Vec<String>) {
        let Ok(entries) = disk.list(path) else { return };
        for entry in entries {
            if found.len() >= FIND_MAX { return; }
            let full = join(path, &entry.name);
            if matches(mask, &entry.name) { found.push(full.clone()); }
            if entry.dir && depth < 16 { self.find(&full, mask, depth + 1, disk, found); }
        }
    }

    /// Goes to the directory of `path` with the cursor on it.
    fn go_to(&mut self, path: &str, disk: &mut dyn Disk) {
        let (dir, name) = parent(path);
        self.load(self.active, &dir, Some(&name), disk);
    }

    fn panel_command(&mut self, side: usize, item: usize, disk: &mut dyn Disk) {
        let panel = &mut self.panels[side];
        match item {
            0 => panel.mode = Mode::Brief, 1 => panel.mode = Mode::Full, 2 => panel.mode = Mode::Info, 3 => panel.mode = Mode::Quick,
            4..=7 => {
                let sort = [Sort::Name, Sort::Extension, Sort::Time, Sort::Size][item - 4];
                panel.sort = sort; panel.arrange(None);
            }
            8 => { panel.reverse = !panel.reverse; panel.arrange(None); }
            9 => self.reread(side, disk),
            _ => self.dialog = Some(Dialog::Volume { side, list: ListState::default() }),
        }
        self.update_preview(disk);
    }

    fn command(&mut self, menu: usize, item: usize, disk: &mut dyn Disk) {
        match (menu, item) {
            (0, item) => self.panel_command(0, item, disk),
            (4, item) => self.panel_command(1, item, disk),
            (1, 0) => if let Some((path, entry)) = self.current_path() { if !entry.dir { self.view(&path, disk); } },
            (1, 1) => self.open(disk),
            (1, 2) => self.dialog = Some(Dialog::Mask { select: true, line: mask_line() }),
            (1, 3) => self.dialog = Some(Dialog::Mask { select: false, line: mask_line() }),
            (1, _) => self.panels[self.active].invert(),
            (2, 0) => self.dialog = Some(Dialog::Find { line: mask_line() }),
            (2, _) => self.panels.swap(0, 1),
            (3, _) => for panel in self.panels.iter_mut() { panel.hidden = !panel.hidden; panel.arrange(None); },
            _ => {}
        }
    }

    fn dialog_key(&mut self, key: Key, disk: &mut dyn Disk) -> Outcome {
        let Some(mut dialog) = self.dialog.take() else { return Outcome::Ignored };
        let keep = match &mut dialog {
            Dialog::Help | Dialog::Message { .. } => !matches!(key.code(), Code::Esc | Code::Enter | Code::F(10) | Code::F(1)),
            Dialog::Mask { select, line } => match line.key(key) {
                Edit::Submit => { let n = self.panels[self.active].mark_mask(line.as_str(), *select); self.notice = Some(format!("{} files {}", n, if *select { "marked" } else { "unmarked" })); false }
                Edit::Cancel => false,
                _ => true,
            },
            Dialog::Find { line } => match line.key(key) {
                Edit::Submit => {
                    let mask = String::from(line.as_str());
                    let mut found = Vec::new();
                    let start = self.panels[self.active].path.clone();
                    self.find(&start, &mask, 0, disk, &mut found);
                    found.sort_by_key(|path| path.to_lowercase());
                    self.dialog = Some(Dialog::Results { mask, found, list: ListState::default() });
                    return Outcome::Redraw;
                }
                Edit::Cancel => false,
                _ => true,
            },
            Dialog::Results { found, list, .. } => {
                if list.key(key, found.len(), 12) { true } else {
                    match key.code() {
                        Code::Enter => { if let Some(path) = found.get(list.selected).cloned() { self.go_to(&path, disk); } false }
                        Code::F(3) => { if let Some(path) = found.get(list.selected).cloned() { self.view(&path, disk); } true }
                        Code::Esc | Code::F(10) => false,
                        _ => true,
                    }
                }
            }
            Dialog::Volume { side, list } => {
                if list.key(key, VOLUMES.len(), VOLUMES.len()) { true } else {
                    match key.code() {
                        Code::Enter => { let side = *side; self.load(side, "", None, disk); false }
                        Code::Esc | Code::F(10) => false,
                        _ => true,
                    }
                }
            }
        };
        if keep && self.dialog.is_none() { self.dialog = Some(dialog); }
        Outcome::Redraw
    }

    /// Handles a key; the caller redraws afterwards.
    pub fn key(&mut self, key: Key, disk: &mut dyn Disk) -> Outcome {
        if let Some(viewer) = self.viewer.as_mut() {
            if viewer.key(key) == Action::Quit { let viewer = self.viewer.take().unwrap(); self.window = Some(viewer.into_buffer()); }
            return Outcome::Redraw;
        }
        if self.dialog.is_some() { return self.dialog_key(key, disk); }
        if self.menu.open {
            if let MenuAction::Chosen(menu, item) = self.menu.key(key) { self.command(menu, item, disk); }
            return Outcome::Redraw;
        }
        self.notice = None;
        let side = self.active;
        if self.panels[side].key(key) { self.update_preview(disk); return Outcome::Redraw; }
        match key.code() {
            Code::F(10) | Code::Esc => return Outcome::Quit,
            Code::Tab => { self.active = 1 - self.active; self.update_preview(disk); return Outcome::Redraw; }
            Code::Enter => { self.open(disk); return Outcome::Redraw; }
            Code::Backspace => { self.up(disk); return Outcome::Redraw; }
            Code::Insert => { self.panels[side].toggle_mark(); self.update_preview(disk); return Outcome::Redraw; }
            Code::F(1) if key.alt() => { self.dialog = Some(Dialog::Volume { side: 0, list: ListState::default() }); return Outcome::Redraw; }
            Code::F(2) if key.alt() => { self.dialog = Some(Dialog::Volume { side: 1, list: ListState::default() }); return Outcome::Redraw; }
            Code::F(7) if key.alt() => { self.dialog = Some(Dialog::Find { line: mask_line() }); return Outcome::Redraw; }
            Code::F(n @ 3..=6) if key.ctrl() => {
                let panel = &mut self.panels[side];
                panel.sort = [Sort::Name, Sort::Extension, Sort::Time, Sort::Size][n as usize - 3];
                panel.arrange(None);
                return Outcome::Redraw;
            }
            Code::F(1) => { self.dialog = Some(Dialog::Help); return Outcome::Redraw; }
            Code::F(3) => {
                if let Some((path, entry)) = self.current_path() { if entry.dir { self.open(disk); } else { self.view(&path, disk); } }
                return Outcome::Redraw;
            }
            Code::F(4) => { self.notice = Some(String::from("The editor comes with issue 047")); return Outcome::Redraw; }
            Code::F(5..=8) => { self.notice = Some(String::from("Read-only: copy, move, mkdir and delete come with VFS v2 (issues 046, 048)")); return Outcome::Redraw; }
            Code::F(9) => { self.menu.open = true; self.menu.menu = if side == 0 { 0 } else { 4 }; self.menu.item = 0; return Outcome::Redraw; }
            _ => {}
        }
        if key.is_ctrl('r') { self.reread(side, disk); return Outcome::Redraw; }
        if key.is_ctrl('u') { self.panels.swap(0, 1); self.update_preview(disk); return Outcome::Redraw; }
        if key.is_ctrl('h') { self.command(3, 0, disk); return Outcome::Redraw; }
        if key.is_ctrl('l') || key.is_ctrl('q') {
            let mode = if key.is_ctrl('l') { Mode::Info } else { Mode::Quick };
            let other = &mut self.panels[1 - side];
            other.mode = if other.mode == mode { Mode::Full } else { mode };
            self.update_preview(disk);
            return Outcome::Redraw;
        }
        match key.text() {
            Some('+') => { self.dialog = Some(Dialog::Mask { select: true, line: mask_line() }); Outcome::Redraw }
            Some('-') => { self.dialog = Some(Dialog::Mask { select: false, line: mask_line() }); Outcome::Redraw }
            Some('*') => { self.panels[side].invert(); Outcome::Redraw }
            _ => Outcome::Ignored,
        }
    }

    fn info(&self, grid: &mut Grid, area: Rect, theme: &Theme) {
        grid.frame_titled(area, Line::Double, "Information", theme.frame, theme.frame);
        let inner = area.inner();
        let panel = &self.panels[self.active];
        let (files, bytes, dirs) = panel.totals();
        let (marked, marked_bytes) = panel.marked_size();
        let mut lines: Vec<(String, bool)> = vec![
            (String::from("MIND CORE file manager"), true),
            (String::from(VOLUMES[0]), false),
            (String::new(), false),
            (format!("Directory /{}", panel.path), true),
            (format!("  {} files, {} bytes; {} directories", files, bytes, dirs), false),
            (format!("  marked: {} files, {} bytes", marked, marked_bytes), false),
            (String::new(), false),
        ];
        if let Some(entry) = panel.current().filter(|e| !e.is_up()) {
            let (y, mo, d, h, mi, s) = panel::fat_time(entry.modified);
            lines.push((entry.name.clone(), true));
            lines.push((if entry.dir { String::from("  directory") } else { format!("  {} bytes", entry.size) }, false));
            if entry.modified != 0 { lines.push((format!("  modified {}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, mi, s), false)); }
            let attributes: Vec<&str> = [(VFS_ENTRY_READ_ONLY, "read-only"), (VFS_ENTRY_HIDDEN, "hidden"), (VFS_ENTRY_SYSTEM, "system"), (VFS_ENTRY_ARCHIVE, "archive")]
                .iter().filter(|(bit, _)| entry.flags & bit != 0).map(|(_, name)| *name).collect();
            if !attributes.is_empty() { lines.push((format!("  {}", attributes.join(", ")), false)); }
            if entry.is_program() { lines.push((String::from("  a program: Enter starts it"), false)); }
        }
        for (i, (text, head)) in lines.iter().enumerate().take(inner.h) {
            grid.text_max(inner.x + 1, inner.y + i, text, inner.w.saturating_sub(2), if *head { theme.header } else { theme.panel });
        }
    }

    fn quick_view(&mut self, grid: &mut Grid, area: Rect, theme: &Theme) {
        let inner = area.inner();
        match self.preview.as_ref() {
            Some((name, bytes, size)) => {
                let title = if *size as usize > bytes.len() { format!("Quick view: first {} of {} bytes", bytes.len(), size) } else { String::from("Quick view") };
                grid.frame_titled(area, Line::Double, &title, theme.frame, theme.frame);
                let mut viewer = Viewer::new(Bytes(bytes), &mut self.quick, name);
                viewer.wrap = true;
                viewer.draw(grid, inner, theme);
            }
            None => {
                grid.frame_titled(area, Line::Double, "Quick view", theme.frame, theme.frame);
                let text = match self.panels[self.active].current() { Some(e) if e.dir => "A directory", _ => "Nothing to show" };
                grid.text_centered(inner, inner.y + inner.h / 2, text, theme.dim);
            }
        }
    }

    /// Draws everything; returns the cursor of an open input line.
    pub fn draw(&mut self, grid: &mut Grid, theme: &Theme) -> Option<(usize, usize)> {
        let (w, h) = (grid.cols, grid.rows);
        grid.clear(theme.panel);
        if let Some(viewer) = self.viewer.as_mut() { let area = grid.area(); return viewer.draw(grid, area, theme); }
        let height = h.saturating_sub(2);
        let left = w / 2;
        for side in 0..2 {
            let area = if side == 0 { Rect::new(0, 0, left, height) } else { Rect::new(left, 0, w - left, height) };
            match self.panels[side].mode {
                Mode::Info if side != self.active => self.info(grid, area, theme),
                Mode::Quick if side != self.active => self.quick_view(grid, area, theme),
                _ => { let active = side == self.active; self.panels[side].draw(grid, area, theme, active, "A"); }
            }
        }
        // The line above the key bar: a notice, or where the active panel is.
        let line = self.notice.clone().unwrap_or_else(|| format!("A:/{}>", self.panels[self.active].path));
        grid.text_padded(0, h - 2, &line, w, if self.notice.is_some() { theme.marked } else { theme.fkey_number });
        fkey_bar(grid, h - 1, &["Help", "", "View", "Edit", "Copy", "RenMov", "Mkdir", "Delete", "PullDn", "Quit"], theme);
        if self.menu.open { self.menu.draw(grid, 0, theme); }
        match self.dialog.as_mut() {
            None => None,
            Some(Dialog::Help) => { message(grid, "fm — keys", &HELP[..HELP.len() - 1], &["OK"], 0, theme); None }
            Some(Dialog::Message { title, lines }) => { let refs: Vec<&str> = lines.iter().map(|l| l.as_str()).collect(); message(grid, title, &refs, &["OK"], 0, theme); None }
            Some(Dialog::Mask { select, line }) => Some(input_dialog(grid, if *select { "Select" } else { "Unselect" }, "Files matching (* and ?, several with ,):", line, 50, theme)),
            Some(Dialog::Find { line }) => Some(input_dialog(grid, "Find file", "Names matching (* and ?), from this directory down:", line, 60, theme)),
            Some(Dialog::Results { mask, found, list }) => {
                let rows = 12usize;
                let inner = dialog(grid, &format!("Found {} for {}", found.len(), mask), (w * 3 / 4).max(30).min(w), rows + 4, theme);
                list.scroll(found.len(), rows);
                for (i, path) in found.iter().enumerate().skip(list.top).take(rows) {
                    let style = if i == list.selected { theme.selected } else { theme.dialog };
                    grid.text_padded(inner.x + 1, inner.y + 1 + i - list.top, &format!("/{}", path), inner.w.saturating_sub(2), style);
                }
                if found.is_empty() { grid.text(inner.x + 1, inner.y + 1, "Nothing found", theme.dialog); }
                grid.text(inner.x + 1, inner.bottom() - 1, "Enter: go to   F3: view   Esc: close", theme.dialog);
                None
            }
            Some(Dialog::Volume { side, list }) => {
                let inner = dialog(grid, if *side == 0 { "Left panel volume" } else { "Right panel volume" }, 56, VOLUMES.len() + 4, theme);
                for (i, volume) in VOLUMES.iter().enumerate() {
                    grid.text_padded(inner.x + 1, inner.y + 1 + i, volume, inner.w.saturating_sub(2), if i == list.selected { theme.selected } else { theme.dialog });
                }
                None
            }
        }
    }

    /// One line of state for the log after each key (tests follow it).
    pub fn status(&self) -> String {
        let mode = |p: &Panel| match p.mode { Mode::Brief => "BRIEF", Mode::Full => "FULL", Mode::Info => "INFO", Mode::Quick => "QUICK" };
        let dialog = match self.dialog { None => "NONE", Some(Dialog::Help) => "HELP", Some(Dialog::Message { .. }) => "MESSAGE", Some(Dialog::Mask { .. }) => "MASK",
                                         Some(Dialog::Find { .. }) => "FIND", Some(Dialog::Results { .. }) => "RESULTS", Some(Dialog::Volume { .. }) => "VOLUME" };
        let panel = &self.panels[self.active];
        format!("LEFT=/{} {} RIGHT=/{} {} ACTIVE={} CURRENT={} MARKED={} DIALOG={} MENU={} VIEW={}", self.panels[0].path, mode(&self.panels[0]), self.panels[1].path,
                mode(&self.panels[1]), if self.active == 0 { "L" } else { "R" }, panel.current().map_or("", |e| e.name.as_str()), panel.marked.len(), dialog,
                self.menu.open as u8, self.viewer.as_ref().map_or(0, |v| v.top() as usize + 1))
    }
}

fn mask_line() -> InputLine { let mut line = InputLine::new(); line.set("*.*"); line }
