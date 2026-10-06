//! `top`: the task table with CPU use from run-time deltas, per-CPU load, kernel memory and IPC rates (docs/tools §4.4).
use crate::abi::*;
use crate::keys::{Code, Key};
use crate::model::*;
use crate::text;
use crate::tui::widgets::{buttons_key, dialog, message, ListState};
use crate::tui::{Grid, Style, Theme};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort { Cpu, Memory, Pid, Time }

/// Refresh intervals that `+` and `-` step through.
pub const INTERVALS: [u64; 4] = [500, 1000, 2000, 5000];

/// Per-second rates over the last interval.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rates { pub messages: u64, pub syscalls: u64, pub interrupts: u64, pub switches: u64 }

/// A task with what top computed for it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage { pub pid: u64, pub cpu: u32, pub syscalls: u64 } // CPU per mille of one CPU, syscalls per second

/// The details window of one task.
pub struct Details { pub task: Task, pub regions: Vec<Region>, pub caps: Vec<Capability>, pub parent: String }

pub struct Top {
    pub tasks: Vec<Task>,
    pub usage: Vec<Usage>,
    previous: Vec<(u64, u64, u64)>, // pid, run_ns, calls at the last refresh
    previous_ns: u64,
    pub busy: Vec<u16>, // per online CPU, per mille over the last interval
    pub memory: Memory,
    pub load: Load,
    pub rates: Rates,
    pub sort: Sort,
    pub hide_services: bool,
    pub tree: bool,
    pub list: ListState,
    selected: Option<u64>, // PID of the selected task, kept across refreshes and sorts
    pub details: Option<Details>,
    pub interval_ms: u64,
    pub notice: Option<String>,
    online: usize,
    height: usize, // table rows on screen at the last draw
    /// k or r asked for: (restart, PID, name, service) and the selected button.
    pub confirm: Option<(bool, u64, String, bool)>,
    choice: usize,
}

impl Default for Top { fn default() -> Self { Self::new() } }

impl Top {
    pub fn new() -> Self {
        Self { tasks: Vec::new(), usage: Vec::new(), previous: Vec::new(), previous_ns: 0, busy: Vec::new(), memory: Memory::default(), load: Load::default(),
               rates: Rates::default(), sort: Sort::Cpu, hide_services: false, tree: false, list: ListState::default(), selected: None, details: None,
               interval_ms: 1000, notice: None, online: 0, height: 10, confirm: None, choice: 1 }
    }

    /// New task records at `now_ns`: CPU use and syscall rate from the deltas since the previous ones; a task seen
    /// for the first time is measured since its start.
    pub fn update(&mut self, tasks: Vec<Task>, now_ns: u64) {
        self.usage = tasks.iter().map(|t| {
            let (run, calls, since) = match self.previous.iter().find(|p| p.0 == t.pid) {
                Some(&(_, run, calls)) if now_ns > self.previous_ns && t.run_ns >= run => (t.run_ns - run, t.calls.saturating_sub(calls), now_ns - self.previous_ns),
                _ => (t.run_ns, t.calls, now_ns.saturating_sub(t.started_ns)),
            };
            if since == 0 { return Usage { pid: t.pid, cpu: 0, syscalls: 0 }; }
            Usage { pid: t.pid, cpu: (run as u128 * 1000 / since as u128).min(1000) as u32, syscalls: (calls as u128 * 1_000_000_000 / since as u128) as u64 }
        }).collect();
        self.previous = tasks.iter().map(|t| (t.pid, t.run_ns, t.calls)).collect();
        self.previous_ns = now_ns;
        self.tasks = tasks;
    }

    /// Per-CPU load and rates from the samples of the last interval.
    pub fn sampled(&mut self, samples: &[Sample], period_ms: u32) {
        let n = samples.len().max(1) as u64;
        self.busy = (0..self.online.clamp(1, 8)).map(|cpu| (samples.iter().map(|s| s.busy[cpu] as u64).sum::<u64>() / n) as u16).collect();
        let window = n * period_ms.max(1) as u64;
        let rate = |f: fn(&Sample) -> u32| samples.iter().map(|s| f(s) as u64).sum::<u64>() * 1000 / window;
        self.rates = Rates { messages: rate(|s| s.messages), syscalls: rate(|s| s.syscalls), interrupts: rate(|s| s.interrupts), switches: rate(|s| s.switches) };
    }

    pub fn usage_of(&self, pid: u64) -> Usage { self.usage.iter().find(|u| u.pid == pid).copied().unwrap_or_default() }

    fn order(&self, a: &Task, b: &Task) -> Ordering {
        let by = match self.sort {
            Sort::Cpu => self.usage_of(b.pid).cpu.cmp(&self.usage_of(a.pid).cpu),
            Sort::Memory => b.memory().cmp(&a.memory()),
            Sort::Time => b.run_ns.cmp(&a.run_ns),
            Sort::Pid => Ordering::Equal,
        };
        by.then(a.pid.cmp(&b.pid))
    }

    /// The table: shown tasks in order, with their depth in the spawn tree (0 when not in tree mode).
    pub fn rows(&self) -> Vec<(&Task, usize)> {
        let shown: Vec<&Task> = self.tasks.iter().filter(|t| !(self.hide_services && t.service())).collect();
        if self.tree { return tree(&shown, &|a, b| self.order(a, b)); }
        let mut rows: Vec<(&Task, usize)> = shown.into_iter().map(|t| (t, 0)).collect();
        rows.sort_by(|a, b| self.order(a.0, b.0));
        rows
    }

    // Keeps the selection on the same task when the order changes.
    fn follow(&mut self) {
        let (index, len) = { let rows = self.rows(); (self.selected.and_then(|pid| rows.iter().position(|r| r.0.pid == pid)), rows.len()) };
        if let Some(index) = index { self.list.selected = index; }
        self.list.scroll(len, self.height);
        self.selected = self.rows().get(self.list.selected).map(|r| r.0.pid);
    }

    pub fn selected_pid(&self) -> Option<u64> { self.selected }

    fn open_details(&mut self, source: &mut dyn Source) {
        let Some(task) = self.selected.and_then(|pid| self.tasks.iter().find(|t| t.pid == pid)).cloned() else { return };
        let regions = source.vmap(task.pid).unwrap_or_default();
        let caps = source.caps(task.pid).unwrap_or_default();
        let parent = String::from(task_name(&self.tasks, task.parent));
        self.details = Some(Details { task, regions, caps, parent });
    }

    /// Lines of the details window.
    pub fn details_lines(details: &Details, now_ns: u64) -> Vec<String> {
        let t = &details.task;
        let mut lines = Vec::new();
        let kind = if t.service() { "service" } else { "application" };
        let screen = if t.flags & TASK_SCREEN != 0 { ", with a screen" } else { ", console" };
        let focus = if t.flags & TASK_FOCUS != 0 { ", in focus" } else { "" };
        lines.push(format!("PID {}  {}  {}{}{}", t.pid, t.name, kind, screen, focus));
        lines.push(format!("Parent {} {}   CPU {}   {}", t.parent, details.parent, t.cpu, text::waits_for(t.state, t.wait)));
        lines.push(format!("Run time {}   age {}   runs {}   syscalls {}", text::cpu_time(t.run_ns), text::uptime(now_ns.saturating_sub(t.started_ns) / 1_000_000), text::count(t.runs), text::count(t.calls)));
        lines.push(format!("IPC: sent {}, received {}", text::count(t.sent), text::count(t.received)));
        lines.push(format!("Memory: image {}, stack {}, screen {}, retained {}, kernel {}", text::size(t.image), text::size(t.stack), text::size(t.screen), text::size(t.retained), text::size(t.kernel)));
        lines.push(format!("Heap {} in {}/{} blocks, shared mappings {}", text::size(t.heap), t.heap_blocks, HEAP_MAX_BLOCKS, text::size(t.shared)));
        let mapped: u64 = details.regions.iter().filter(|r| r.kind != REGION_GUARD).map(|r| r.bytes).sum();
        lines.push(format!("Address space: {} regions, {} mapped", details.regions.len(), text::size(mapped)));
        let mut kinds: Vec<(&str, usize)> = Vec::new();
        for c in &details.caps {
            let name = text::cap_kind(c.kind);
            match kinds.iter_mut().find(|k| k.0 == name) { Some(k) => k.1 += 1, None => kinds.push((name, 1)) }
        }
        let list: Vec<String> = kinds.iter().map(|(name, n)| format!("{} {}", name, n)).collect();
        lines.push(format!("Capabilities {}/{}: {}", t.caps, CAP_SLOTS_MAX - 1, list.join(", ")));
        // Endpoint indexes are labels (the `ipc` view and the shell's `endpoints` use the same ones), not authority.
        let endpoints: Vec<String> = details.caps.iter().filter(|c| c.kind as usize == CAP_KIND_ENDPOINT && c.endpoint != 0).map(|c| format!("{}→{}", c.slot, c.endpoint)).collect();
        if !endpoints.is_empty() { lines.push(format!("Endpoints (slot→index): {}", endpoints.join(" "))); }
        lines.push(format!("Quotas: tasks {}/{}, endpoints {}/{}", t.used_tasks, t.quota_tasks, t.used_endpoints, t.quota_endpoints));
        lines
    }

    fn header(&self, grid: &mut Grid, theme: &Theme) -> usize {
        let w = grid.cols;
        grid.fill(crate::tui::Rect::new(0, 0, w, 1), ' ', theme.status);
        grid.text(1, 0, &format!("top   up {}   load average {} {} {}", text::uptime(self.load.uptime_ms), text::hundredths(self.load.one), text::hundredths(self.load.five), text::hundredths(self.load.fifteen)), theme.status);
        grid.text_right(w - 1, 0, &format!("every {}.{} s", self.interval_ms / 1000, self.interval_ms % 1000 / 100), theme.status);
        let count = |states: &[u8]| self.tasks.iter().filter(|t| states.contains(&t.state)).count();
        let summary = format!("Tasks {}: {} running, {} ready, {} sleeping, {} blocked    IPC {}/s   syscalls {}/s   interrupts {}/s   switches {}/s",
            self.tasks.len(), count(&[WAIT_RUNNING]), count(&[WAIT_NONE]), count(&[WAIT_SLEEP]), count(&[WAIT_SEND, WAIT_RECEIVE, WAIT_REPLY, WAIT_IRQ, WAIT_FLUSH]),
            text::count(self.rates.messages), text::count(self.rates.syscalls), text::count(self.rates.interrupts), text::count(self.rates.switches));
        grid.text(1, 1, &summary, theme.panel);
        // A busy bar per CPU, two per row when the screen is wide enough.
        let columns = if w >= 80 { 2 } else { 1 };
        let width = (w - 1) / columns;
        let fill = Style::new(theme.accent.fg, theme.panel.bg);
        let empty = Style::new(theme.dim.fg, theme.panel.bg);
        for (cpu, &busy) in self.busy.iter().enumerate() {
            let (x, y) = (1 + cpu % columns * width, 2 + cpu / columns);
            grid.text(x, y, &format!("CPU{}", cpu), theme.header);
            let bar = width.saturating_sub(14);
            grid.put(x + 5, y, '[', theme.dim);
            grid.bar(x + 6, y, bar, busy as u64, 1000, fill, empty);
            grid.put(x + 6 + bar, y, ']', theme.dim);
            grid.text_right(x + width - 1, y, &format!("{}%", text::permille(busy as u32)), theme.panel);
        }
        let y = 2 + self.busy.len().div_ceil(columns);
        let m = &self.memory;
        grid.text(1, y, "Mem", theme.header);
        let bar = (w / 3).max(10);
        grid.put(6, y, '[', theme.dim);
        grid.bar(7, y, bar, m.used, m.arena.max(1), Style::new(theme.marked.fg, theme.panel.bg), empty);
        grid.put(7 + bar, y, ']', theme.dim);
        grid.text(9 + bar, y, &format!("{}/{} used, largest free {}, tasks {}, endpoints {}", text::size(m.used), text::size(m.arena), text::size(m.largest_free),
                                         m.tasks, m.endpoints), theme.panel);
        y + 2
    }

    /// Columns that fit in `width` cells: name, width, right-aligned. Lower-priority columns are dropped first.
    pub fn columns(width: usize) -> Vec<(&'static str, usize, bool)> {
        // (title, width, right-aligned, priority: higher stays longer)
        const ALL: [(&str, usize, bool, u8); 13] = [("PID", 5, true, 9), ("PPID", 5, true, 3), ("NAME", 16, false, 9), ("STATE", 5, false, 8), ("CPU", 3, true, 5),
            ("%CPU", 5, true, 9), ("TIME", 9, true, 7), ("SYSC/s", 7, true, 6), ("MEM", 7, true, 7), ("HEAP", 7, true, 4), ("SHARED", 7, true, 2), ("CAPS", 4, true, 3), ("EP", 5, true, 1)];
        let mut keep = 0u8;
        loop {
            let used: usize = ALL.iter().filter(|c| c.3 > keep).map(|c| c.1 + 1).sum();
            if used <= width || keep >= 8 { return ALL.iter().filter(|c| c.3 > keep).map(|c| (c.0, c.1, c.2)).collect(); }
            keep += 1;
        }
    }

    fn cell(&self, title: &str, task: &Task, depth: usize) -> String {
        let usage = self.usage_of(task.pid);
        match title {
            "PID" => format!("{}", task.pid), "PPID" => format!("{}", task.parent),
            "NAME" => { let mut name = String::new(); for _ in 1..depth { name.push_str("  "); } if depth > 0 { name.push_str("└ "); } name.push_str(&task.name); name }
            "STATE" => String::from(text::state(task.state)), "CPU" => format!("{}", task.cpu), "%CPU" => text::permille(usage.cpu),
            "TIME" => text::cpu_time(task.run_ns), "SYSC/s" => text::count(usage.syscalls), "MEM" => text::size(task.memory()), "HEAP" => text::size(task.heap),
            "SHARED" => text::size(task.shared), "CAPS" => format!("{}", task.caps), "EP" => format!("{}/{}", task.used_endpoints, task.quota_endpoints),
            _ => String::new(),
        }
    }
}

impl Tool for Top {
    fn refresh(&mut self, source: &mut dyn Source) -> Result<(), Problem> {
        if self.online == 0 { self.online = source.cpus()?.iter().filter(|c| c.online).count().max(1); }
        let tasks = source.tasks()?;
        self.update(tasks, source.now_ns());
        self.follow();
        self.memory = source.memory()?;
        self.load = source.load()?;
        let samples = source.history(false, (self.interval_ms / 100).clamp(1, 50) as u16)?;
        let period = if self.load.fast_ms == 0 { 100 } else { self.load.fast_ms };
        self.sampled(&samples, period);
        Ok(())
    }

    fn draw(&mut self, grid: &mut Grid, theme: &Theme) {
        let (w, h) = (grid.cols, grid.rows);
        grid.clear(theme.panel);
        let y = self.header(grid, theme).min(h.saturating_sub(3));
        let columns = Self::columns(w.saturating_sub(1));
        let mut x = 1;
        grid.fill(crate::tui::Rect::new(0, y, w, 1), ' ', theme.menu);
        let sorted = match self.sort { Sort::Cpu => "%CPU", Sort::Memory => "MEM", Sort::Pid => "PID", Sort::Time => "TIME" };
        for &(title, width, right) in &columns {
            let style = if title == sorted { theme.menu_selected } else { theme.menu };
            if right { grid.text_right(x + width, y, title, style); } else { grid.text(x, y, title, style); }
            x += width + 1;
        }
        self.height = h.saturating_sub(y + 2);
        self.list.scroll(self.rows().len(), self.height);
        let rows = self.rows();
        for (i, &(task, depth)) in rows.iter().enumerate().skip(self.list.top).take(self.height) {
            let row = y + 1 + i - self.list.top;
            let style = if i == self.list.selected { theme.selected } else if task.service() { theme.dim } else if task.flags & TASK_FOCUS != 0 { theme.accent } else { theme.panel };
            grid.fill(crate::tui::Rect::new(0, row, w, 1), ' ', style);
            let mut x = 1;
            for &(title, width, right) in &columns {
                let value = self.cell(title, task, depth);
                if right { grid.text_right(x + width, row, &value, style); } else { grid.text_max(x, row, &value, width, style); }
                x += width + 1;
            }
        }
        let hint = self.notice.clone().unwrap_or_else(|| String::from("P/M/N/T sort  S services  t tree  Enter details  k stop  r restart  +/- interval  q quit"));
        grid.fill(crate::tui::Rect::new(0, h - 1, w, 1), ' ', theme.status);
        grid.text(1, h - 1, &hint, theme.status);
        if let Some((restart, pid, name, _)) = &self.confirm {
            let line = format!("{} {} (PID {})?", if *restart { "Restart" } else { "Stop" }, name, pid);
            message(grid, if *restart { "Restart" } else { "Stop" }, &[line.as_str()], &[if *restart { "Restart" } else { "Stop" }, "Cancel"], self.choice, theme);
        }
        if let Some(details) = &self.details {
            let lines = Self::details_lines(details, self.previous_ns);
            let width = (lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) + 4).min(w);
            let inner = dialog(grid, &format!("Task {}", details.task.pid), width, lines.len() + 4, theme);
            for (i, line) in lines.iter().enumerate() { grid.text_max(inner.x + 1, inner.y + 1 + i, line, inner.w.saturating_sub(2), theme.dialog); }
        }
    }

    fn key(&mut self, key: Key, source: &mut dyn Source) -> Flow {
        self.notice = None;
        if let Some((restart, pid, name, _)) = self.confirm.clone() {
            match buttons_key(key, &mut self.choice, 2) {
                Some(Some(0)) => {
                    self.confirm = None;
                    let result = if restart { source.restart(&name).map(|new| format!("{} restarted as PID {}", name, new)) } else {
                        match self.tasks.iter().find(|t| t.pid == pid).cloned() { Some(task) => source.stop(&task).map(|_| format!("{} (PID {}) stopped", name, pid)), None => Err(String::from("it is gone")) }
                    };
                    self.notice = Some(result.unwrap_or_else(|error| format!("{} {}: {}", if restart { "Restart" } else { "Stop" }, name, error)));
                    return Flow::Refresh;
                }
                Some(_) => { self.confirm = None; return Flow::Redraw; }
                None => return Flow::Redraw,
            }
        }
        if self.details.is_some() {
            if matches!(key.code(), Code::Esc | Code::Enter) || matches!(key.latin(), Some('q')) || key.code() == Code::F(10) { self.details = None; return Flow::Redraw; }
            return Flow::Ignored;
        }
        let len = self.rows().len();
        if self.list.key(key, len, self.height) { self.selected = self.rows().get(self.list.selected).map(|r| r.0.pid); return Flow::Redraw; }
        match key.code() {
            Code::Esc | Code::F(10) => return Flow::Quit,
            Code::Enter => { self.open_details(source); return Flow::Redraw; }
            _ => {}
        }
        match key.latin() {
            Some('q') | Some('Q') => Flow::Quit,
            Some('p') | Some('P') => { self.sort = Sort::Cpu; self.follow(); Flow::Redraw }
            Some('m') | Some('M') => { self.sort = Sort::Memory; self.follow(); Flow::Redraw }
            Some('n') | Some('N') => { self.sort = Sort::Pid; self.follow(); Flow::Redraw }
            Some('T') => { self.sort = Sort::Time; self.follow(); Flow::Redraw }
            Some('t') => { self.tree = !self.tree; self.follow(); Flow::Redraw }
            Some('s') | Some('S') => { self.hide_services = !self.hide_services; self.follow(); Flow::Redraw }
            Some('+') | Some('=') => { self.interval_ms = INTERVALS.iter().copied().find(|&i| i > self.interval_ms).unwrap_or(self.interval_ms); Flow::Redraw }
            Some('-') => { self.interval_ms = INTERVALS.iter().rev().copied().find(|&i| i < self.interval_ms).unwrap_or(self.interval_ms); Flow::Redraw }
            Some(c @ ('k' | 'K' | 'r' | 'R')) => {
                let restart = c.eq_ignore_ascii_case(&'r');
                let Some(task) = self.selected.and_then(|pid| self.tasks.iter().find(|t| t.pid == pid)) else { return Flow::Ignored };
                if restart && !task.service() { self.notice = Some(String::from("Only boot services restart; k stops an application")); return Flow::Redraw; }
                self.confirm = Some((restart, task.pid, task.name.clone(), task.service()));
                self.choice = 1; // Cancel unless chosen
                Flow::Redraw
            }
            _ => Flow::Ignored,
        }
    }

    fn interval_ms(&self) -> u64 { self.interval_ms }

    fn status(&self) -> String {
        let sort = match self.sort { Sort::Cpu => "CPU", Sort::Memory => "MEM", Sort::Pid => "PID", Sort::Time => "TIME" };
        let confirm = match &self.confirm { None => "NONE", Some((true, ..)) => "RESTART", Some((false, ..)) => "STOP" };
        format!("SORT={} TREE={} HIDE={} SELECTED={} DETAILS={} ROWS={} CONFIRM={}", sort, self.tree as u8, self.hide_services as u8, self.selected.unwrap_or(0),
                self.details.as_ref().map_or(0, |d| d.task.pid), self.rows().len(), confirm)
    }
}
