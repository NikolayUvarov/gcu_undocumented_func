//! The desktop menu of `wm` (issue u003): a right click on the desktop (or Alt+P) opens the programs on the boot disk
//! by category; a category's programs open beside it when the mouse is on it or Right is pressed; a click or Enter on
//! a program starts it in a window. No system calls: tests/wm_host.rs.
use crate::keys::{Code, Key};
use crate::tui::{Grid, Line, Rect, Style, Theme};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// An entry: a program to start (`command`) or a submenu (`children`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item { pub label: String, pub command: Option<String>, pub children: Vec<Item> }

impl Item {
    pub fn program(label: &str, command: &str) -> Self { Self { label: String::from(label), command: Some(String::from(command)), children: Vec::new() } }
    pub fn submenu(label: &str, children: Vec<Item>) -> Self { Self { label: String::from(label), command: None, children } }
}

/// How a program runs, from what it asks its launcher for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Window, Console, Manager }

/// The categories, in this order; programs not named here go under "Other".
pub const CATEGORIES: [(&str, &[&str]); 5] = [
    ("Files", &["fm", "edit", "view", "find", "grep", "df", "fsck", "format"]),
    ("System", &["console", "top", "memmap", "load", "hw", "ipc", "caps", "dmesg", "svc", "keys", "keymap", "screenshot", "uptime", "pinmap", "reboot"]),
    ("Clocks", &["clock", "dzen-clock"]),
    ("Sound and voice", &["beep", "say", "listen", "hear"]),
    ("Network", &["netcheck", "netbench"]),
];
/// Programs with a text face (issue 089) get a second entry for it.
const TEXT_FACES: [&str; 2] = ["clock", "dzen-clock"];

/// The menu of `programs`: the categories that have any, then "Other"; a console program runs inside `console` when
/// there is one (else it is left out, as are window managers).
pub fn catalogue(programs: &[(String, Kind)]) -> Vec<Item> {
    let has_console = programs.iter().any(|(name, kind)| name == "console" && *kind == Kind::Window);
    let entry = |name: &str, kind: Kind| -> Option<Item> {
        match kind {
            Kind::Window => Some(Item::program(name, name)),
            Kind::Console if has_console => Some(Item::program(name, &format!("console {}", name))),
            _ => None,
        }
    };
    let mut menu = Vec::new();
    for (category, names) in CATEGORIES {
        let mut items = Vec::new();
        for name in names {
            let Some(&(_, kind)) = programs.iter().find(|(n, _)| n == name) else { continue };
            if let Some(item) = entry(name, kind) { items.push(item); }
            if kind == Kind::Window && TEXT_FACES.contains(name) { items.push(Item::program(&format!("{} --text", name), &format!("{} --text", name))); }
        }
        if !items.is_empty() { menu.push(Item::submenu(category, items)); }
    }
    let mut other: Vec<&(String, Kind)> = programs.iter().filter(|(name, _)| !CATEGORIES.iter().any(|(_, names)| names.contains(&name.as_str()))).collect();
    other.sort_by(|a, b| a.0.cmp(&b.0));
    let others: Vec<Item> = other.iter().filter_map(|(name, kind)| entry(name, *kind)).collect();
    if !others.is_empty() { menu.push(Item::submenu("Other", others)); }
    if menu.is_empty() { menu.push(Item { label: String::from("No programs found"), command: None, children: Vec::new() }); }
    menu
}

/// No item of a level is highlighted.
pub const NONE: usize = usize::MAX;

/// What the menu did with an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome { Stay, Close, Run(String) }

/// An open menu: where it was opened and the highlighted item of each open level; level k + 1 lists the children of
/// level k's highlighted item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu { pub x: usize, pub y: usize, pub levels: Vec<usize> }

impl Menu {
    pub fn new(x: usize, y: usize) -> Self { Self { x, y, levels: vec![NONE] } }

    // The items of level `level`.
    fn items<'a>(&self, root: &'a [Item], level: usize) -> &'a [Item] {
        let mut items = root;
        for &index in &self.levels[..level] { items = items.get(index).map_or(&[][..], |item| &item.children[..]); }
        items
    }

    /// The frame of each open level on a screen of `cols` × `rows` (between its top bar and status line).
    pub fn rects(&self, root: &[Item], cols: usize, rows: usize) -> Vec<Rect> {
        let (top, bottom) = (1, rows.saturating_sub(1));
        let mut rects: Vec<Rect> = Vec::new();
        for level in 0..self.levels.len() {
            let items = self.items(root, level);
            let w = (items.iter().map(|i| i.label.chars().count()).max().unwrap_or(0) + 5).min(cols);
            let h = (items.len() + 2).min(bottom - top);
            let (x, y) = match rects.last() {
                None => (self.x, self.y),
                Some(parent) => {
                    let right = parent.right().saturating_sub(1);
                    (if right + w <= cols { right } else { parent.x.saturating_sub(w.saturating_sub(1)) }, parent.y + self.levels[level - 1])
                }
            };
            rects.push(Rect::new(x.min(cols - w), y.clamp(top, bottom.saturating_sub(h).max(top)), w, h));
        }
        rects
    }

    /// The item at cell (x, y): its level and index, the deepest level first.
    pub fn hit(&self, root: &[Item], cols: usize, rows: usize, x: usize, y: usize) -> Option<(usize, usize)> {
        let rects = self.rects(root, cols, rows);
        for (level, r) in rects.iter().enumerate().rev() {
            if !r.contains(x, y) { continue; }
            let inner = r.inner();
            let index = y.checked_sub(inner.y).filter(|&i| i < inner.h && i < self.items(root, level).len());
            return index.map(|i| (level, i));
        }
        None
    }

    /// Highlights item `index` of level `level`; its children open beside it.
    pub fn highlight(&mut self, root: &[Item], level: usize, index: usize) {
        self.levels.truncate(level + 1);
        self.levels[level] = index;
        if self.items(root, level).get(index).is_some_and(|item| !item.children.is_empty()) { self.levels.push(NONE); }
    }

    /// The mouse at cell (x, y), `pressed` if a button went down: the item under it is highlighted; a press on a
    /// program starts it, one outside the menu closes it.
    pub fn pointer(&mut self, root: &[Item], cols: usize, rows: usize, x: usize, y: usize, pressed: bool) -> Outcome {
        match self.hit(root, cols, rows, x, y) {
            Some((level, index)) => {
                self.highlight(root, level, index);
                match &self.items(root, level)[index].command { Some(command) if pressed => Outcome::Run(command.clone()), _ => Outcome::Stay }
            }
            None if pressed && !self.rects(root, cols, rows).iter().any(|r| r.contains(x, y)) => Outcome::Close,
            None => Outcome::Stay,
        }
    }

    /// ↑ ↓ move in the deepest level, → or Enter opens a category, Enter starts a program, ← or Esc goes back.
    pub fn key(&mut self, root: &[Item], key: Key) -> Outcome {
        let level = self.levels.len() - 1;
        let count = self.items(root, level).len();
        let current = self.levels[level];
        match key.code() {
            Code::Up | Code::Down if count > 0 => {
                let next = match (current, key.code()) { (NONE, Code::Up) => count - 1, (NONE, _) => 0, (i, Code::Up) => (i + count - 1) % count, (i, _) => (i + 1) % count };
                self.levels[level] = next;
            }
            Code::Right | Code::Enter if current != NONE => {
                let item = &self.items(root, level)[current];
                if let (Some(command), Code::Enter) = (&item.command, key.code()) { return Outcome::Run(command.clone()); }
                if !item.children.is_empty() { self.levels.push(0); }
            }
            Code::Left | Code::Esc => {
                if level == 0 { return Outcome::Close; }
                self.levels.pop();
            }
            _ => {}
        }
        Outcome::Stay
    }

    pub fn draw(&self, root: &[Item], grid: &mut Grid, theme: &Theme) {
        let rects = self.rects(root, grid.cols, grid.rows);
        for (level, r) in rects.iter().enumerate() {
            grid.fill(*r, ' ', theme.dialog);
            grid.frame(*r, Line::Single, theme.dialog);
            let inner = r.inner();
            for (index, item) in self.items(root, level).iter().enumerate().take(inner.h) {
                let style = if self.levels[level] == index { theme.selected } else if item.command.is_none() && item.children.is_empty() { Style::new(theme.dim.fg, theme.dialog.bg) } else { theme.dialog };
                grid.text_padded(inner.x, inner.y + index, &format!(" {}", item.label), inner.w, style);
                if !item.children.is_empty() { grid.put(inner.right().saturating_sub(1), inner.y + index, '►', style); }
            }
        }
    }

    /// The highlighted items' labels, `Files>fm`, for the log.
    pub fn path(&self, root: &[Item]) -> String {
        let mut labels = Vec::new();
        for level in 0..self.levels.len() {
            match self.items(root, level).get(self.levels[level]) { Some(item) => labels.push(item.label.as_str()), None => break }
        }
        labels.join(">")
    }
}
