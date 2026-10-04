//! `ipc`: endpoints with their servers, holders and queues, and who waits for whom (docs/tools §4.7). Endpoint indexes
//! are labels the kernel reports, not authority (MC-10.2).
use crate::abi::*;
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
pub enum View { Endpoints, Waits }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort { Index, Messages, Queue }

const VIEWS: [(View, &str); 2] = [(View::Endpoints, "1 Endpoints"), (View::Waits, "2 Waits")];

/// An edge of the wait-for graph: `from` waits for `to` — the server of the endpoint it sends to, or the task it waits
/// for a reply from. `to` is 0 for an endpoint without a server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge { pub from: u64, pub to: u64, pub endpoint: Option<u32> }

/// The wait-for graph of blocked senders and callers. Tasks waiting to receive wait for nobody in particular.
pub fn edges(tasks: &[Task], endpoints: &[EndpointInfo]) -> Vec<Edge> {
    tasks.iter().filter_map(|t| match t.state {
        WAIT_SEND => {
            let index = t.wait as u32;
            Some(Edge { from: t.pid, to: endpoints.iter().find(|e| e.index == index).map_or(0, |e| e.server), endpoint: Some(index) })
        }
        WAIT_REPLY => Some(Edge { from: t.pid, to: t.wait, endpoint: None }),
        _ => None,
    }).collect()
}

/// Cycles of the wait-for graph (deadlocks), each as the PIDs around it starting at the smallest, once. A task waits for
/// one thing at a time, so following the edges from any task is a path that ends or closes a loop.
pub fn cycles(edges: &[Edge]) -> Vec<Vec<u64>> {
    let mut found: Vec<Vec<u64>> = Vec::new();
    for start in edges {
        let mut path = vec![start.from];
        while let Some(next) = edges.iter().find(|e| e.from == *path.last().unwrap()).map(|e| e.to).filter(|&to| to != 0) {
            if let Some(at) = path.iter().position(|&p| p == next) {
                let mut cycle = path[at..].to_vec();
                let smallest = (0..cycle.len()).min_by_key(|&i| cycle[i]).unwrap_or(0);
                cycle.rotate_left(smallest);
                if !found.contains(&cycle) { found.push(cycle); }
                break;
            }
            if path.len() > edges.len() { break; }
            path.push(next);
        }
    }
    found
}

/// `rwgk` rights of an endpoint capability.
pub fn rights(rights: u32) -> String {
    [(CAP_READ, 'r'), (CAP_WRITE, 'w'), (CAP_GRANT, 'g'), (CAP_KEEP, 'k')].iter().map(|&(bit, ch)| if rights as u8 & bit != 0 { ch } else { '-' }).collect()
}

pub struct Ipc {
    pub view: View,
    pub sort: Sort,
    pub endpoints: Vec<EndpointInfo>,
    pub tasks: Vec<Task>,
    pub list: ListState,
    /// The holders window: the endpoint and its holders.
    pub holders: Option<(u32, Vec<Holder>)>,
    pub notice: Option<String>,
    height: usize,
}

impl Ipc {
    pub fn new() -> Self {
        Self { view: View::Endpoints, sort: Sort::Index, endpoints: Vec::new(), tasks: Vec::new(), list: ListState::default(), holders: None, notice: None, height: 10 }
    }

    /// The endpoints in the chosen order (busy ones first for the counters).
    pub fn rows(&self) -> Vec<&EndpointInfo> {
        let mut rows: Vec<&EndpointInfo> = self.endpoints.iter().collect();
        match self.sort {
            Sort::Index => rows.sort_by_key(|e| e.index),
            Sort::Messages => rows.sort_by(|a, b| b.messages.cmp(&a.messages).then(a.index.cmp(&b.index))),
            Sort::Queue => rows.sort_by(|a, b| (b.senders + b.receiving).cmp(&(a.senders + a.receiving)).then(a.index.cmp(&b.index))),
        }
        rows
    }

    /// "name (PID n)", or "—" for no task.
    pub fn who(&self, pid: u64) -> String {
        if pid == 0 { String::from("—") } else { format!("{} (PID {})", task_name(&self.tasks, pid), pid) }
    }

    /// Lines of the wait-for view.
    pub fn wait_lines(&self) -> Vec<String> {
        let edges = edges(&self.tasks, &self.endpoints);
        let mut lines = Vec::new();
        let cycles = cycles(&edges);
        for cycle in &cycles {
            let names: Vec<String> = cycle.iter().chain(cycle.first()).map(|&pid| self.who(pid)).collect();
            lines.push(format!("DEADLOCK: {}", names.join(" → ")));
        }
        for e in &edges {
            let target = if e.to == 0 { String::from("nobody serves it") } else { self.who(e.to) };
            lines.push(match e.endpoint {
                Some(index) => format!("{} sends to endpoint {} → {}", self.who(e.from), index, target),
                None => format!("{} waits for a reply from {}", self.who(e.from), target),
            });
        }
        if edges.is_empty() { lines.push(String::from("No task waits for another.")); }
        let receiving = self.tasks.iter().filter(|t| t.state == WAIT_RECEIVE).count();
        lines.push(String::new());
        lines.push(format!("{} tasks wait for a message on their own endpoints; {} edges, {} deadlocks.", receiving, edges.len(), cycles.len()));
        lines
    }

    fn open_holders(&mut self, source: &mut dyn Source) {
        let Some(index) = self.rows().get(self.list.selected).map(|e| e.index) else { return };
        match source.holders(index) {
            Ok(list) => self.holders = Some((index, list)),
            Err(problem) => self.notice = Some(format!("holders of endpoint {}: {:?}", index, problem)),
        }
    }

    fn tabs(&self, grid: &mut Grid, theme: &Theme) {
        let w = grid.cols;
        grid.fill(Rect::new(0, 0, w, 1), ' ', theme.status);
        grid.text(1, 0, "ipc", theme.status);
        let mut x = 6;
        for (view, title) in VIEWS {
            let style = if view == self.view { theme.menu_selected } else { theme.status };
            grid.text(x, 0, &format!(" {} ", title), style);
            x += title.chars().count() + 3;
        }
    }

    fn endpoints_view(&mut self, grid: &mut Grid, theme: &Theme) {
        let (w, h) = (grid.cols, grid.rows);
        let total: u64 = self.endpoints.iter().map(|e| e.messages).sum();
        grid.text(1, 2, &format!("{} endpoints, {} messages, a queue holds at most {} senders", self.endpoints.len(), text::count(total), ENDPOINT_QUEUE), theme.header);
        grid.fill(Rect::new(0, 4, w, 1), ' ', theme.menu);
        grid.text(1, 4, "  EP  SERVER                  HOLDERS  QUEUE  RECV   MESSAGES    BUSY  TIMEOUTS  IRQ  CREATOR", theme.menu);
        self.height = h.saturating_sub(7);
        let rows = self.rows();
        let mut list = self.list;
        list.scroll(rows.len(), self.height);
        for (i, e) in rows.iter().enumerate().skip(list.top).take(self.height) {
            let y = 5 + i - list.top;
            let style = if i == list.selected { theme.selected } else if e.senders > 0 { theme.accent } else { theme.panel };
            grid.fill(Rect::new(0, y, w, 1), ' ', style);
            let irq = if e.irq == 0 { String::from("—") } else { format!("{}", e.irq) };
            grid.text_max(1, y, &format!("{:>4}  {:<22} {:>7}  {:>2}/{}  {:>4}  {:>9}  {:>6}  {:>8}  {:>3}  {}", e.index, self.who(e.server), e.holders, e.senders, ENDPOINT_QUEUE,
                                         e.receiving, text::count(e.messages), text::count(e.busy), text::count(e.timeouts), irq, self.who(e.creator)), w.saturating_sub(2), style);
        }
        drop(rows);
        self.list = list;
    }

    fn waits_view(&mut self, grid: &mut Grid, theme: &Theme) {
        let (w, h) = (grid.cols, grid.rows);
        grid.text(1, 2, "Who waits for whom: blocked senders and callers (a cycle is a deadlock)", theme.header);
        let lines = self.wait_lines();
        self.height = h.saturating_sub(5);
        let mut list = self.list;
        list.scroll(lines.len(), self.height);
        for (i, line) in lines.iter().enumerate().skip(list.top).take(self.height) {
            let style = if line.starts_with("DEADLOCK") { theme.accent } else { theme.panel };
            grid.text_max(1, 4 + i - list.top, line, w.saturating_sub(2), style);
        }
        self.list = list;
    }
}

impl Default for Ipc { fn default() -> Self { Self::new() } }

impl Tool for Ipc {
    fn refresh(&mut self, source: &mut dyn Source) -> Result<(), Problem> {
        let selected = self.rows().get(self.list.selected).map(|e| e.index);
        self.endpoints = source.endpoints()?;
        self.tasks = source.tasks()?;
        // Keep the chosen endpoint when the table changes.
        if let Some(at) = selected.and_then(|index| self.rows().iter().position(|e| e.index == index)) { self.list.selected = at; }
        Ok(())
    }

    fn draw(&mut self, grid: &mut Grid, theme: &Theme) {
        let (w, h) = (grid.cols, grid.rows);
        grid.clear(theme.panel);
        self.tabs(grid, theme);
        match self.view { View::Endpoints => self.endpoints_view(grid, theme), View::Waits => self.waits_view(grid, theme) }
        let hint = self.notice.clone().unwrap_or_else(|| String::from("Tab/1-2 view  ↑↓ move  Enter holders  i/m/w sort by index/messages/waiting  q quit"));
        grid.fill(Rect::new(0, h - 1, w, 1), ' ', theme.status);
        grid.text(1, h - 1, &hint, theme.status);
        if let Some((index, holders)) = &self.holders {
            let server = self.endpoints.iter().find(|e| e.index == *index).map_or(0, |e| e.server);
            let mut lines = vec![format!("Server: {}", self.who(server)), String::new(), String::from("HOLDER                     SLOT  RIGHTS  BADGE")];
            for holder in holders {
                let badge = if holder.badge == 0 { String::from("—") } else { format!("{:#x}", holder.badge) };
                lines.push(format!("{:<26} {:>4}  {:<6}  {}", self.who(holder.pid), holder.slot, rights(holder.rights), badge));
            }
            let width = (lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) + 4).min(w);
            let inner = dialog(grid, &format!("Endpoint {}: {} holders", index, holders.len()), width, (lines.len() + 4).min(h), theme);
            for (i, line) in lines.iter().enumerate().take(inner.h.saturating_sub(2)) { grid.text_max(inner.x + 1, inner.y + 1 + i, line, inner.w.saturating_sub(2), theme.dialog); }
        }
    }

    fn key(&mut self, key: Key, source: &mut dyn Source) -> Flow {
        self.notice = None;
        if self.holders.is_some() {
            if matches!(key.code(), Code::Esc | Code::Enter | Code::F(10)) || matches!(key.latin(), Some('q')) { self.holders = None; return Flow::Redraw; }
            return Flow::Ignored;
        }
        let len = match self.view { View::Endpoints => self.endpoints.len(), View::Waits => self.wait_lines().len() };
        if self.list.key(key, len, self.height) { return Flow::Redraw; }
        let index = VIEWS.iter().position(|v| v.0 == self.view).unwrap_or(0);
        let switch = |this: &mut Self, view: View| { this.view = view; this.list = ListState::default(); Flow::Redraw };
        match key.code() {
            Code::Esc | Code::F(10) => return Flow::Quit,
            Code::Tab => return switch(self, VIEWS[(index + 1) % VIEWS.len()].0),
            Code::Enter if self.view == View::Endpoints => { self.open_holders(source); return Flow::Redraw; }
            _ => {}
        }
        match key.latin() {
            Some('q') | Some('Q') => Flow::Quit,
            Some('1') => switch(self, View::Endpoints),
            Some('2') => switch(self, View::Waits),
            Some('i') | Some('I') => { self.sort = Sort::Index; Flow::Redraw }
            Some('m') | Some('M') => { self.sort = Sort::Messages; Flow::Redraw }
            Some('w') | Some('W') => { self.sort = Sort::Queue; Flow::Redraw }
            Some('r') | Some('R') => Flow::Refresh,
            _ => Flow::Ignored,
        }
    }

    fn interval_ms(&self) -> u64 { 2000 }

    fn status(&self) -> String {
        let view = match self.view { View::Endpoints => "ENDPOINTS", View::Waits => "WAITS" };
        let sort = match self.sort { Sort::Index => "INDEX", Sort::Messages => "MESSAGES", Sort::Queue => "QUEUE" };
        let edges = edges(&self.tasks, &self.endpoints);
        format!("VIEW={} SORT={} ENDPOINTS={} SELECTED={} HOLDERS={} EDGES={} DEADLOCKS={}", view, sort, self.endpoints.len(), self.rows().get(self.list.selected).map_or(0, |e| e.index),
                self.holders.as_ref().map_or(0, |h| h.1.len()), edges.len(), cycles(&edges).len())
    }
}
