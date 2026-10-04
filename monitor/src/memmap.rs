//! `memmap`: the physical memory map, the kernel arena by use, a task's address space and the quotas (docs/tools §4.5).
//! No memory contents and no physical addresses of task pages are shown.
use crate::abi::*;
use crate::keys::{Code, Key};
use crate::model::*;
use crate::text;
use crate::tui::widgets::ListState;
use crate::tui::{Grid, Rect, Style, Theme};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View { Physical, Arena, Process, Quotas }

const VIEWS: [(View, &str); 4] = [(View::Physical, "1 Physical"), (View::Arena, "2 Kernel arena"), (View::Process, "3 Process"), (View::Quotas, "4 Quotas")];
const GIB4: u64 = 4 << 30;

/// Colour of a physical range kind on the bar.
pub fn kind_color(kind: u32) -> u32 {
    match kind {
        7 => 0x50C878, 3 | 4 => 0x3A9A9A, 1 | 2 => 0x7070E0, 5 | 6 => 0xE0A040, 9 | 10 => 0xE0E060, 11 | 12 => 0x9090A0,
        PHYS_ARENA => 0xF080C0, PHYS_BOOT_IMAGE => 0xC080FF, PHYS_FRAMEBUFFER => 0x60C0FF,
        PHYS_AP_TRAMPOLINE => 0xFFFFFF, PHYS_PCI_BAR => 0xC0C0C0, _ => 0x905050,
    }
}

/// Firmware ranges of the same kind that touch are merged; platform layout ranges stay as they are. The result is
/// ordered by address, the layout after the firmware range it lies in.
pub fn merge(ranges: &[Range]) -> Vec<Range> {
    let mut firmware: Vec<Range> = ranges.iter().copied().filter(|r| r.kind < PHYS_PLATFORM).collect();
    firmware.sort_by_key(|r| r.start);
    let mut out: Vec<Range> = Vec::new();
    for r in firmware {
        match out.last_mut() { Some(last) if last.kind == r.kind && last.end() == r.start => last.bytes += r.bytes, _ => out.push(r) }
    }
    out.extend(ranges.iter().copied().filter(|r| r.kind >= PHYS_PLATFORM));
    out.sort_by(|a, b| a.start.cmp(&b.start).then((a.kind >= PHYS_PLATFORM).cmp(&(b.kind >= PHYS_PLATFORM))));
    out
}

/// The kind shown in each of `cells` cells covering [0, `top`): the layout range covering most of the cell, else the
/// firmware range covering most of it, else none (a hole in the map).
pub fn bar_kinds(ranges: &[Range], top: u64, cells: usize) -> Vec<Option<u32>> {
    (0..cells).map(|i| {
        let (a, b) = ((top as u128 * i as u128 / cells as u128) as u64, (top as u128 * (i as u128 + 1) / cells as u128) as u64);
        let best = |layout: bool| ranges.iter().filter(|r| (r.kind >= PHYS_PLATFORM) == layout)
            .map(|r| (r.end().min(b).saturating_sub(r.start.max(a)), r.kind)).filter(|&(overlap, _)| overlap > 0).max_by_key(|&(overlap, _)| overlap).map(|(_, kind)| kind);
        best(true).or_else(|| best(false))
    }).collect()
}

/// Bytes of firmware RAM: (usable: loader, boot services and free; free).
pub fn ram(ranges: &[Range]) -> (u64, u64) {
    let usable = ranges.iter().filter(|r| matches!(r.kind, 1..=4 | 7)).map(|r| r.bytes).sum();
    let free = ranges.iter().filter(|r| r.kind == 7).map(|r| r.bytes).sum();
    (usable, free)
}

pub struct Memmap {
    pub view: View,
    pub ranges: Vec<Range>,
    pub memory: Memory,
    pub tasks: Vec<Task>,
    pub regions: Vec<Region>,
    pub regions_pid: u64,
    pub merged: bool,
    pub zoom: bool, // the bar covers RAM only, not 0..4 GiB
    pub list: ListState,
    pub procs: ListState,
    height: usize,
}

impl Default for Memmap { fn default() -> Self { Self::new() } }

impl Memmap {
    pub fn new() -> Self {
        Self { view: View::Physical, ranges: Vec::new(), memory: Memory::default(), tasks: Vec::new(), regions: Vec::new(), regions_pid: 0, merged: true, zoom: false,
               list: ListState::default(), procs: ListState::default(), height: 10 }
    }

    pub fn shown_ranges(&self) -> Vec<Range> { if self.merged { merge(&self.ranges) } else { let mut r = self.ranges.clone(); r.sort_by_key(|r| r.start); r } }

    fn process_pid(&self) -> Option<u64> { self.tasks.get(self.procs.selected).map(|t| t.pid) }

    fn load_regions(&mut self, source: &mut dyn Source) {
        if let Some(pid) = self.process_pid() {
            self.regions = source.vmap(pid).unwrap_or_default();
            self.regions_pid = pid;
        }
    }

    fn tabs(&self, grid: &mut Grid, theme: &Theme) {
        let w = grid.cols;
        grid.fill(Rect::new(0, 0, w, 1), ' ', theme.status);
        grid.text(1, 0, "memmap", theme.status);
        let mut x = 9;
        for (view, title) in VIEWS {
            let style = if view == self.view { theme.menu_selected } else { theme.status };
            grid.text(x, 0, &format!(" {} ", title), style);
            x += title.chars().count() + 3;
        }
    }

    fn physical(&mut self, grid: &mut Grid, theme: &Theme) {
        let (w, h) = (grid.cols, grid.rows);
        let ram_top = self.ranges.iter().filter(|r| r.kind < PHYS_PLATFORM && matches!(r.kind, 1..=7)).map(|r| r.end()).max().unwrap_or(GIB4);
        let top = if self.zoom { ram_top.max(1) } else { GIB4 };
        grid.text(1, 2, &format!("Physical address space 0–{} ({}; z: {})", text::size(top), if self.zoom { "RAM" } else { "first 4 GiB" }, if self.zoom { "0–4G" } else { "RAM only" }), theme.header);
        let cells = w.saturating_sub(2);
        for (i, kind) in bar_kinds(&self.ranges, top, cells).into_iter().enumerate() {
            let style = match kind { Some(k) => Style::new(kind_color(k), theme.panel.bg), None => theme.dim };
            let ch = if kind.is_some() { '█' } else { '·' };
            grid.put(1 + i, 3, ch, style); grid.put(1 + i, 4, ch, style);
        }
        for quarter in 0..=4u64 {
            let label = text::size(top / 4 * quarter);
            let x = 1 + (cells as u64 * quarter / 4) as usize;
            let x = if quarter == 4 { x.saturating_sub(label.chars().count()) } else { x };
            grid.text(x, 5, &label, theme.dim);
        }
        // Legend: the kinds present.
        let mut kinds: Vec<u32> = Vec::new();
        for r in &self.ranges { if !kinds.contains(&r.kind) { kinds.push(r.kind); } }
        kinds.sort();
        let (mut x, mut y) = (1, 6);
        for kind in kinds {
            let name = text::phys_kind(kind);
            if x + name.chars().count() + 3 > w { x = 1; y += 1; }
            grid.put(x, y, '█', Style::new(kind_color(kind), theme.panel.bg));
            grid.text(x + 2, y, name, theme.panel);
            x += name.chars().count() + 4;
        }
        let (usable, free) = ram(&self.ranges);
        let above: u64 = self.ranges.iter().filter(|r| matches!(r.kind, 1..=7) && r.end() > GIB4).map(|r| r.end() - r.start.max(GIB4)).sum();
        grid.text(1, y + 1, &format!("RAM {} usable, {} free; {} ranges{}", text::size(usable), text::size(free), self.ranges.len(),
                                      if above > 0 { format!("; RAM above 4 GiB: {}", text::size(above)) } else { String::new() }), theme.panel);
        let head = y + 3;
        grid.fill(Rect::new(0, head, w, 1), ' ', theme.menu);
        grid.text(1, head, "  START          END                 SIZE  KIND", theme.menu);
        grid.text_right(w - 1, head, if self.merged { "merged (m: raw)" } else { "raw (m: merge)" }, theme.menu);
        let shown = self.shown_ranges();
        self.height = h.saturating_sub(head + 2);
        self.list.scroll(shown.len(), self.height);
        for (i, r) in shown.iter().enumerate().skip(self.list.top).take(self.height) {
            let row = head + 1 + i - self.list.top;
            let style = if i == self.list.selected { theme.selected } else if r.kind >= PHYS_PLATFORM { theme.accent } else { theme.panel };
            grid.fill(Rect::new(0, row, w, 1), ' ', style);
            grid.put(1, row, '█', if i == self.list.selected { style } else { Style::new(kind_color(r.kind), style.bg) });
            let detail = match r.kind { PHYS_BOOT_IMAGE => format!(" (image {})", r.detail), PHYS_PCI_BAR => format!(" (device {})", r.detail), _ => String::new() };
            grid.text(3, row, &format!("{:#014x} {:#014x} {:>9}  {}{}", r.start, r.end().saturating_sub(1), text::size(r.bytes), text::phys_kind(r.kind), detail), style);
        }
    }

    /// Kernel arena categories: name, bytes, colour, limit (0: none).
    pub fn categories(m: &Memory) -> [(&'static str, u64, u32, u64); 9] {
        [("task images", m.images, 0xC080FF, 0), ("task stacks", m.stacks, 0x80D0FF, 0), ("screens", m.screens, 0x60C0FF, 0), ("program heaps", m.heaps, 0x50C878, 0),
         ("task kernel pages", m.task_pages, 0xE0A040, 0), ("page tables", m.page_tables, 0xE0E060, 0), ("memory objects", m.objects, 0xF080C0, m.objects_limit),
         ("DMA buffers", m.dma, 0xFFB000, m.dma_limit), ("other kernel", m.other(), 0x9090A0, 0)]
    }

    fn arena(&self, grid: &mut Grid, theme: &Theme) {
        let w = grid.cols;
        let m = &self.memory;
        let percent = if m.arena == 0 { 0 } else { m.used * 1000 / m.arena } as u32;
        grid.text(1, 2, &format!("Kernel arena {}: used {} ({}%), free {}, largest free block {}", text::size(m.arena), text::size(m.used), text::permille(percent), text::size(m.free),
                                 text::size(m.largest_free)), theme.header);
        // A stacked bar of the categories; the rest is free.
        let cells = w.saturating_sub(2) as u64;
        let mut x = 1;
        let mut total = 0u64;
        for (_, bytes, color, _) in Self::categories(m) {
            total += bytes;
            let end = 1 + (total.min(m.arena) as u128 * cells as u128 / m.arena.max(1) as u128) as usize;
            while x < end { grid.put(x, 3, '█', Style::new(color, theme.panel.bg)); grid.put(x, 4, '█', Style::new(color, theme.panel.bg)); x += 1; }
        }
        while x < 1 + cells as usize { grid.put(x, 3, '·', theme.dim); grid.put(x, 4, '·', theme.dim); x += 1; }
        grid.fill(Rect::new(0, 6, w, 1), ' ', theme.menu);
        grid.text(1, 6, "USE                          SIZE  OF ARENA  LIMIT", theme.menu);
        let mut y = 7;
        for (name, bytes, color, limit) in Self::categories(m) {
            grid.put(1, y, '█', Style::new(color, theme.panel.bg));
            grid.text(3, y, name, theme.panel);
            grid.text_right(36, y, &text::size(bytes), theme.panel);
            grid.text_right(46, y, &format!("{}%", text::permille((bytes * 1000 / m.arena.max(1)) as u32)), theme.panel);
            if limit != 0 {
                grid.text(48, y, &format!("{} of {}", text::size(bytes), text::size(limit)), theme.panel);
                grid.bar(66, y, 12.min(w.saturating_sub(68)), bytes, limit, Style::new(color, theme.panel.bg), theme.dim);
            }
            y += 1;
        }
        grid.text(3, y, "free", theme.panel);
        grid.text_right(36, y, &text::size(m.free), theme.panel);
        grid.text_right(46, y, &format!("{}%", text::permille((m.free * 1000 / m.arena.max(1)) as u32)), theme.panel);
        y += 2;
        // Program images and screens need contiguous blocks: free memory outside the largest one cannot hold a large one.
        grid.text(1, y, &format!("Free outside the largest block: {} (fragmentation); shared memory mapped by tasks: {}", text::size(m.free.saturating_sub(m.largest_free)),
                                 text::size(m.shared)), theme.panel);
        y += 1;
        grid.text(1, y, &format!("Tasks {}/{}, endpoints {}/{}", m.tasks, m.tasks_limit, m.endpoints, m.endpoints_limit), theme.panel);
    }

    fn process(&mut self, grid: &mut Grid, theme: &Theme) {
        let (w, h) = (grid.cols, grid.rows);
        let left = 24.min(w / 3);
        grid.frame_titled(Rect::new(0, 1, left, h - 2), crate::tui::Line::Single, "Tasks", theme.frame, theme.header);
        self.height = h.saturating_sub(4);
        self.procs.scroll(self.tasks.len(), self.height);
        for (i, t) in self.tasks.iter().enumerate().skip(self.procs.top).take(self.height) {
            let style = if i == self.procs.selected { theme.selected } else if t.service() { theme.dim } else { theme.panel };
            grid.text_padded(1, 2 + i - self.procs.top, &format!("{:>4} {}", t.pid, t.name), left - 2, style);
        }
        let x = left + 1;
        let Some(task) = self.tasks.get(self.procs.selected) else { return };
        grid.text(x, 2, &format!("Address space of {} (PID {})", task.name, task.pid), theme.header);
        grid.fill(Rect::new(x, 3, w - x, 1), ' ', theme.menu);
        grid.text(x + 1, 3, "START                   SIZE  RIGHTS KIND", theme.menu);
        let mut y = 4;
        for r in self.regions.iter().take(h.saturating_sub(8)) {
            let style = if matches!(r.kind, REGION_SHARED | REGION_DEVICE) { theme.accent } else { theme.panel };
            grid.text(x + 1, y, &format!("{:#018x} {:>9}  {}    {}", r.start, text::size(r.bytes), text::rights(r.flags), text::region_kind(r.kind)), style);
            y += 1;
        }
        if self.regions.len() > h.saturating_sub(8) { grid.text(x + 1, y, &format!("… {} more", self.regions.len() - h.saturating_sub(8)), theme.dim); y += 1; }
        let mapped: u64 = self.regions.iter().filter(|r| r.kind != REGION_GUARD).map(|r| r.bytes).sum();
        let heap: u64 = self.regions.iter().filter(|r| r.kind == REGION_HEAP).map(|r| r.bytes).sum();
        let shared: u64 = self.regions.iter().filter(|r| r.kind == REGION_SHARED).map(|r| r.bytes).sum();
        grid.text(x + 1, y + 1, &format!("mapped {}", text::size(mapped)), theme.panel);
        grid.text(x + 1, y + 2, &format!("heap {} of {} in {}/{} blocks; shared {} of {}", text::size(heap), text::size(HEAP_MAX_BYTES as u64), task.heap_blocks, HEAP_MAX_BLOCKS,
                                          text::size(shared), text::size(SHARED_MAX_BYTES as u64)), theme.panel);
    }

    fn quotas(&mut self, grid: &mut Grid, theme: &Theme) {
        let (w, h) = (grid.cols, grid.rows);
        grid.text(1, 2, "Task and endpoint quotas by spawner (MC-3.13): a child's reservation comes from its spawner", theme.header);
        grid.fill(Rect::new(0, 3, w, 1), ' ', theme.menu);
        grid.text(1, 3, "PID  NAME                    TASKS    ENDPOINTS  CAPS", theme.menu);
        let tasks: Vec<&Task> = self.tasks.iter().collect();
        let rows = tree(&tasks, &|a, b| a.pid.cmp(&b.pid));
        self.height = h.saturating_sub(5);
        self.list.scroll(rows.len(), self.height);
        for (i, &(t, depth)) in rows.iter().enumerate().skip(self.list.top).take(self.height) {
            let y = 4 + i - self.list.top;
            let style = if i == self.list.selected { theme.selected } else if t.service() { theme.dim } else { theme.panel };
            grid.fill(Rect::new(0, y, w, 1), ' ', style);
            let mut name = String::new();
            for _ in 0..depth { name.push_str("  "); }
            name.push_str(&t.name);
            grid.text(1, y, &format!("{:>3}  {:<22} {:>3}/{:<3}  {:>3}/{:<3}    {:>2}/{}", t.pid, name, t.used_tasks, t.quota_tasks, t.used_endpoints, t.quota_endpoints, t.caps, CAP_SLOTS - 1), style);
        }
    }
}

impl Tool for Memmap {
    fn refresh(&mut self, source: &mut dyn Source) -> Result<(), Problem> {
        self.ranges = source.physmap()?;
        self.memory = source.memory()?;
        let pid = self.process_pid();
        self.tasks = source.tasks()?;
        // Keep the chosen task when the table changes.
        if let Some(index) = pid.and_then(|pid| self.tasks.iter().position(|t| t.pid == pid)) { self.procs.selected = index; }
        self.procs.scroll(self.tasks.len(), self.height);
        if self.view == View::Process { self.load_regions(source); }
        Ok(())
    }

    fn draw(&mut self, grid: &mut Grid, theme: &Theme) {
        let (w, h) = (grid.cols, grid.rows);
        grid.clear(theme.panel);
        self.tabs(grid, theme);
        match self.view { View::Physical => self.physical(grid, theme), View::Arena => self.arena(grid, theme), View::Process => self.process(grid, theme), View::Quotas => self.quotas(grid, theme) }
        grid.fill(Rect::new(0, h - 1, w, 1), ' ', theme.status);
        grid.text(1, h - 1, "Tab/1-4 view  ↑↓ PgUp PgDn move  m merge/raw  z zoom to RAM  q quit", theme.status);
    }

    fn key(&mut self, key: Key, source: &mut dyn Source) -> Flow {
        let index = VIEWS.iter().position(|v| v.0 == self.view).unwrap_or(0);
        let switch = |this: &mut Self, view: View, source: &mut dyn Source| { this.view = view; this.list = ListState::default(); if view == View::Process { this.load_regions(source); } Flow::Redraw };
        match key.code() {
            Code::Esc | Code::F(10) => return Flow::Quit,
            Code::Tab => return switch(self, VIEWS[(index + if key.shift() { 3 } else { 1 }) % 4].0, source),
            _ => {}
        }
        let moved = match self.view {
            View::Physical => { let len = self.shown_ranges().len(); self.list.key(key, len, self.height) }
            View::Quotas => self.list.key(key, self.tasks.len(), self.height),
            View::Process => { let moved = self.procs.key(key, self.tasks.len(), self.height); if moved { self.load_regions(source); } moved }
            View::Arena => false,
        };
        if moved { return Flow::Redraw; }
        match key.latin() {
            Some('q') | Some('Q') => Flow::Quit,
            Some(digit @ '1'..='4') => switch(self, VIEWS[digit as usize - '1' as usize].0, source),
            Some('m') | Some('M') => { self.merged = !self.merged; self.list = ListState::default(); Flow::Redraw }
            Some('z') | Some('Z') => { self.zoom = !self.zoom; Flow::Redraw }
            _ => Flow::Ignored,
        }
    }

    fn interval_ms(&self) -> u64 { 2000 }

    fn status(&self) -> String {
        let view = match self.view { View::Physical => "PHYSICAL", View::Arena => "ARENA", View::Process => "PROCESS", View::Quotas => "QUOTAS" };
        format!("VIEW={} PID={} MERGED={} ZOOM={}", view, self.process_pid().unwrap_or(0), self.merged as u8, self.zoom as u8)
    }
}
