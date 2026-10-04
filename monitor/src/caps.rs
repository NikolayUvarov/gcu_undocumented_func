//! `caps`: the capabilities of a task, the derivation tree across tasks and what a revoke would remove (docs/tools
//! §4.7). Who holds what is the authority graph: sysmon tells it only to a client with the authority badge
//! (`REQUEST_AUTHORITY`); without it the tool says so and shows nothing.
use crate::abi::*;
use crate::ipc::rights;
use crate::keys::{Code, Key};
use crate::model::*;
use crate::text;
use crate::tui::widgets::{dialog, ListState};
use crate::tui::{Grid, Rect, Theme};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View { Task, Tree }

const VIEWS: [(View, &str); 2] = [(View::Task, "1 Task"), (View::Tree, "2 Tree")];

/// The derivation forest of `entries` in depth-first order: (index into `entries`, depth). A root is a capability
/// derived from none the list holds (the kernel's originals, or a parent already gone); siblings go by PID and slot.
pub fn forest(entries: &[AuthorityEntry]) -> Vec<(usize, usize)> {
    let known = |node: u64| node != 0 && entries.iter().any(|e| e.node == node);
    let roots: Vec<usize> = ordered(entries, |e| !known(e.parent) || e.parent == e.node);
    let mut out = Vec::with_capacity(entries.len());
    for root in roots { visit(entries, root, 0, &mut out); }
    out
}

/// The capabilities below `node` (the ones a revoke of it removes), depth-first with their depth below it (from 1).
pub fn subtree(entries: &[AuthorityEntry], node: u64) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for child in ordered(entries, |e| e.parent == node && e.node != node) { visit(entries, child, 1, &mut out); }
    out
}

// Indexes of the entries `pick` chooses, by PID and slot.
fn ordered(entries: &[AuthorityEntry], pick: impl Fn(&AuthorityEntry) -> bool) -> Vec<usize> {
    let mut list: Vec<usize> = (0..entries.len()).filter(|&i| pick(&entries[i])).collect();
    list.sort_by_key(|&i| (entries[i].pid, entries[i].slot));
    list
}

fn visit(entries: &[AuthorityEntry], at: usize, depth: usize, out: &mut Vec<(usize, usize)>) {
    if out.iter().any(|&(i, _)| i == at) || depth > 64 { return; } // never loop on a malformed table
    out.push((at, depth));
    let node = entries[at].node;
    for child in ordered(entries, |e| e.parent == node && e.node != node) { visit(entries, child, depth + 1, out); }
}

pub struct Caps {
    pub view: View,
    pub tasks: Vec<Task>,
    pub entries: Vec<AuthorityEntry>,
    /// The task whose slots the Task view shows.
    pub pid: u64,
    pub list: ListState,
    /// The revoke window: the capability and the ones below it.
    pub revoke: Option<(AuthorityEntry, Vec<(AuthorityEntry, usize)>)>,
    /// sysmon refused: the program has no authority client.
    pub denied: bool,
    pub notice: Option<String>,
    height: usize,
}

impl Caps {
    /// Starts on the Task view of `pid` (0: the first task).
    pub fn new(pid: u64) -> Self {
        Self { view: View::Task, tasks: Vec::new(), entries: Vec::new(), pid, list: ListState::default(), revoke: None, denied: false, notice: None, height: 10 }
    }

    /// The selected task's capabilities by slot.
    pub fn slots(&self) -> Vec<&AuthorityEntry> {
        let mut slots: Vec<&AuthorityEntry> = self.entries.iter().filter(|e| e.pid == self.pid).collect();
        slots.sort_by_key(|e| e.slot);
        slots
    }

    /// The rows of the current view, as indexes into `entries` with their depth.
    pub fn rows(&self) -> Vec<(usize, usize)> {
        match self.view {
            View::Task => {
                let mut rows: Vec<(usize, usize)> = (0..self.entries.len()).filter(|&i| self.entries[i].pid == self.pid).map(|i| (i, 0)).collect();
                rows.sort_by_key(|&(i, _)| self.entries[i].slot);
                rows
            }
            View::Tree => forest(&self.entries),
        }
    }

    pub fn selected(&self) -> Option<AuthorityEntry> { self.rows().get(self.list.selected).map(|&(i, _)| self.entries[i]) }

    /// "name (PID n)".
    pub fn who(&self, pid: u64) -> String { format!("{} (PID {})", task_name(&self.tasks, pid), pid) }

    /// Where a capability came from: the holder and slot of its parent node, or "kernel" for a root.
    pub fn origin(&self, entry: &AuthorityEntry) -> String {
        match self.entries.iter().find(|e| e.node == entry.parent && entry.parent != 0) {
            Some(parent) => format!("{} slot {}", self.who(parent.pid), parent.slot),
            None => String::from("kernel"),
        }
    }

    /// One capability as a line: slot, kind, rights, badge and what it names.
    pub fn describe(&self, e: &AuthorityEntry) -> String {
        let what = match e.kind as usize {
            CAP_KIND_ENDPOINT => format!("EP {}", e.endpoint),
            CAP_KIND_MEMORY | CAP_KIND_DMA | CAP_KIND_MMIO => text::size(e.size),
            _ => String::new(),
        };
        let badge = if e.badge == 0 { String::from("—") } else { format!("{:#x}", e.badge) };
        format!("slot {:>2}  {:<8} {:<4}  badge {:<6} {}", e.slot, text::cap_kind(e.kind), rights(e.rights), badge, what)
    }

    fn step_task(&mut self, forward: bool) {
        let mut pids: Vec<u64> = self.tasks.iter().map(|t| t.pid).collect();
        pids.sort_unstable();
        if pids.is_empty() { return; }
        let at = pids.iter().position(|&p| p == self.pid);
        let next = match (at, forward) {
            (Some(i), true) => pids[(i + 1) % pids.len()],
            (Some(i), false) => pids[(i + pids.len() - 1) % pids.len()],
            (None, _) => pids[0],
        };
        self.pid = next;
        self.list = ListState::default();
    }

    fn open_revoke(&mut self) {
        let Some(entry) = self.selected() else { return };
        let below = subtree(&self.entries, entry.node).into_iter().map(|(i, depth)| (self.entries[i], depth)).collect();
        self.revoke = Some((entry, below));
    }

    fn tabs(&self, grid: &mut Grid, theme: &Theme) {
        grid.fill(Rect::new(0, 0, grid.cols, 1), ' ', theme.status);
        grid.text(1, 0, "caps", theme.status);
        let mut x = 7;
        for (view, title) in VIEWS {
            let style = if view == self.view { theme.menu_selected } else { theme.status };
            grid.text(x, 0, &format!(" {} ", title), style);
            x += title.chars().count() + 3;
        }
    }
}

impl Tool for Caps {
    fn refresh(&mut self, source: &mut dyn Source) -> Result<(), Problem> {
        let selected = self.selected().map(|e| (e.pid, e.slot));
        self.tasks = source.tasks()?;
        match source.authority() {
            Ok(entries) => { self.entries = entries; self.denied = false; }
            Err(Problem::Denied) => { self.entries.clear(); self.denied = true; }
            Err(problem) => return Err(problem),
        }
        if self.pid == 0 || !self.tasks.iter().any(|t| t.pid == self.pid) { self.pid = self.tasks.iter().map(|t| t.pid).min().unwrap_or(0); }
        // Keep the chosen capability when the table changes.
        if let Some(at) = selected.and_then(|(pid, slot)| self.rows().iter().position(|&(i, _)| self.entries[i].pid == pid && self.entries[i].slot == slot)) { self.list.selected = at; }
        Ok(())
    }

    fn draw(&mut self, grid: &mut Grid, theme: &Theme) {
        let (w, h) = (grid.cols, grid.rows);
        grid.clear(theme.panel);
        self.tabs(grid, theme);
        if self.denied {
            grid.text(1, 2, "No authority: sysmon tells who holds what only to a program that asks for REQUEST_AUTHORITY.", theme.accent);
        } else {
            let header = match self.view {
                View::Task => format!("{}: {} capabilities  (←→ other task)", self.who(self.pid), self.slots().len()),
                View::Tree => format!("Derivation tree: {} capabilities of {} tasks", self.entries.len(), self.tasks.len()),
            };
            grid.text_max(1, 2, &header, w.saturating_sub(2), theme.header);
            grid.fill(Rect::new(0, 4, w, 1), ' ', theme.menu);
            grid.text(1, 4, match self.view { View::Task => "SLOT     KIND     RIGHTS BADGE      NAMES        FROM", View::Tree => "HOLDER / CAPABILITY" }, theme.menu);
            self.height = h.saturating_sub(7);
            let rows = self.rows();
            let mut list = self.list;
            list.scroll(rows.len(), self.height);
            for (row, &(i, depth)) in rows.iter().enumerate().skip(list.top).take(self.height) {
                let y = 5 + row - list.top;
                let style = if row == list.selected { theme.selected } else { theme.panel };
                let e = &self.entries[i];
                let line = match self.view {
                    View::Task => format!("{:<34} {}", self.describe(e), self.origin(e)),
                    View::Tree => format!("{}{}  {}", "  ".repeat(depth), self.who(e.pid), self.describe(e)),
                };
                grid.fill(Rect::new(0, y, w, 1), ' ', style);
                grid.text_max(1, y, &line, w.saturating_sub(2), style);
            }
            self.list = list;
        }
        let hint = self.notice.clone().unwrap_or_else(|| String::from("Tab/1-2 view  ↑↓ move  ←→ task  Enter what a revoke removes  r refresh  q quit"));
        grid.fill(Rect::new(0, h - 1, w, 1), ' ', theme.status);
        grid.text(1, h - 1, &hint, theme.status);
        if let Some((entry, below)) = &self.revoke {
            let mut lines = vec![format!("{} {}", self.who(entry.pid), self.describe(entry)), String::new()];
            if below.is_empty() { lines.push(String::from("Nothing was derived from it: a revoke removes nothing.")); }
            else { lines.push(format!("A revoke removes {} capabilities:", below.len())); }
            for (e, depth) in below { lines.push(format!("{}{}  {}", "  ".repeat(*depth), self.who(e.pid), self.describe(e))); }
            let width = (lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) + 4).min(w);
            let inner = dialog(grid, &format!("Revoke slot {} of {}", entry.slot, task_name(&self.tasks, entry.pid)), width, (lines.len() + 4).min(h), theme);
            for (i, line) in lines.iter().enumerate().take(inner.h.saturating_sub(2)) { grid.text_max(inner.x + 1, inner.y + 1 + i, line, inner.w.saturating_sub(2), theme.dialog); }
        }
    }

    fn key(&mut self, key: Key, _source: &mut dyn Source) -> Flow {
        self.notice = None;
        if self.revoke.is_some() {
            if matches!(key.code(), Code::Esc | Code::Enter | Code::F(10)) || matches!(key.latin(), Some('q')) { self.revoke = None; return Flow::Redraw; }
            return Flow::Ignored;
        }
        if self.list.key(key, self.rows().len(), self.height) { return Flow::Redraw; }
        let index = VIEWS.iter().position(|v| v.0 == self.view).unwrap_or(0);
        let switch = |this: &mut Self, view: View| { this.view = view; this.list = ListState::default(); Flow::Redraw };
        match key.code() {
            Code::Esc | Code::F(10) => return Flow::Quit,
            Code::Tab => return switch(self, VIEWS[(index + 1) % VIEWS.len()].0),
            Code::Left | Code::Right if self.view == View::Task => { self.step_task(key.code() == Code::Right); return Flow::Redraw; }
            Code::Enter => { self.open_revoke(); return Flow::Redraw; }
            _ => {}
        }
        match key.latin() {
            Some('q') | Some('Q') => Flow::Quit,
            Some('1') => switch(self, View::Task),
            Some('2') => switch(self, View::Tree),
            Some('r') | Some('R') => Flow::Refresh,
            _ => Flow::Ignored,
        }
    }

    fn interval_ms(&self) -> u64 { 2000 }

    fn status(&self) -> String {
        let view = match self.view { View::Task => "TASK", View::Tree => "TREE" };
        let selected = self.selected();
        let roots = forest(&self.entries).iter().filter(|&&(_, depth)| depth == 0).count();
        format!("VIEW={} PID={} SLOTS={} ENTRIES={} ROOTS={} SELECTED={}:{} KIND={} REVOKE={} DENIED={}", view, self.pid, self.slots().len(), self.entries.len(), roots,
                selected.map_or(0, |e| e.pid), selected.map_or(0, |e| e.slot), selected.map_or("-", |e| text::cap_kind(e.kind)),
                self.revoke.as_ref().map_or(String::from("-"), |r| format!("{}", r.1.len())), self.denied as u8)
    }
}
