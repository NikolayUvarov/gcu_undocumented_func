//! What the monitors show, as plain values copied out of sysmon's replies (idl/sysinfo.wit), and the interface they
//! read it through.
use crate::keys::Key;
use crate::tui::{Grid, Theme};
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Task {
    pub pid: u64, pub parent: u64, pub run_ns: u64, pub runs: u64, pub calls: u64, pub sent: u64, pub received: u64, pub started_ns: u64,
    pub image: u64, pub stack: u64, pub screen: u64, pub heap: u64, pub shared: u64, pub kernel: u64, pub wait: u64,
    pub heap_blocks: u32, pub caps: u32, pub quota_tasks: u32, pub used_tasks: u32, pub quota_endpoints: u32, pub used_endpoints: u32,
    pub name: String, pub state: u8, pub cpu: u8, pub flags: u8,
}

impl Task {
    pub fn service(&self) -> bool { self.flags & crate::abi::TASK_FLAG_SERVICE != 0 }
    /// Kernel memory the task holds: image, stack, screen, heap and kernel pages (shared mappings not counted).
    pub fn memory(&self) -> u64 { self.image + self.stack + self.screen + self.heap + self.kernel }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cpu { pub busy_ns: u64, pub idle_ns: u64, pub ticks: u64, pub switches: u64, pub interrupts: u64, pub current: u64, pub apic: u32, pub online: bool }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Memory {
    pub arena: u64, pub used: u64, pub free: u64, pub largest_free: u64, pub images: u64, pub stacks: u64, pub screens: u64, pub heaps: u64,
    pub task_kernel: u64, pub page_tables: u64, pub objects: u64, pub objects_limit: u64, pub dma: u64, pub dma_limit: u64, pub mapped: u64,
    pub other: u64, pub tasks: u32, pub tasks_limit: u32, pub endpoints: u32, pub endpoints_limit: u32,
}

/// A physical range: UEFI memory type (0..15) or platform layout (`PHYS_*`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Range { pub start: u64, pub bytes: u64, pub kind: u32, pub detail: u32 }
impl Range { pub fn end(&self) -> u64 { self.start.saturating_add(self.bytes) } }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Region { pub start: u64, pub bytes: u64, pub kind: u32, pub flags: u32 }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capability { pub node: u64, pub parent: u64, pub size: u64, pub base: u64, pub slot: u32, pub generation: u32, pub kind: u32, pub rights: u32, pub endpoint: u32 }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Irq { pub count: u64, pub line: u32, pub holder: u32, pub endpoint: u32, pub masked: bool }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Device { pub bars: [u64; 6], pub class: u32, pub irq: u32, pub holder: u32, pub index: u32, pub location: u32, pub io_bars: u32 }

/// One load sample: busy per mille of CPUs 0..7 and counts during the sample period.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sample { pub busy: [u16; 8], pub interrupts: u32, pub syscalls: u32, pub messages: u32, pub switches: u32, pub used_kib: u32, pub tasks: u8, pub runnable: u8 }

/// Load averages (runnable tasks x 100), uptime and sampling periods.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Load { pub one: u32, pub five: u32, pub fifteen: u32, pub uptime_ms: u64, pub fast_ms: u32, pub slow_ms: u32, pub fast_count: u32, pub slow_count: u32 }

/// Why data could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// No sysmon client in the program's slot: it was not started by a launcher that lends one.
    NoAccess,
    /// sysmon refused the request (over the client's rate); the previous data stays.
    Busy,
    NotFound,
    /// sysmon is not running or answered with an error.
    Failed,
}

/// Where the data comes from: sysmon through the program's client endpoint (`app::Client`), or a fake in tests.
pub trait Source {
    fn tasks(&mut self) -> Result<Vec<Task>, Problem>;
    fn cpus(&mut self) -> Result<Vec<Cpu>, Problem>;
    fn memory(&mut self) -> Result<Memory, Problem>;
    fn physmap(&mut self) -> Result<Vec<Range>, Problem>;
    fn vmap(&mut self, pid: u64) -> Result<Vec<Region>, Problem>;
    fn caps(&mut self, pid: u64) -> Result<Vec<Capability>, Problem>;
    fn irqs(&mut self) -> Result<Vec<Irq>, Problem>;
    fn devices(&mut self) -> Result<Vec<Device>, Problem>;
    /// The last `count` samples, oldest first: every 100 ms (`slow` false) or every second.
    fn history(&mut self, slow: bool, count: u16) -> Result<Vec<Sample>, Problem>;
    fn load(&mut self) -> Result<Load, Problem>;
    /// Monotonic nanoseconds (the kernel's clock: the same base as `Task::started_ns`).
    fn now_ns(&self) -> u64;
}

/// What a key did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow { Quit, Redraw, Refresh, Ignored }

/// A monitor: refreshed every `interval_ms`, drawn after every refresh and key.
pub trait Tool {
    fn refresh(&mut self, source: &mut dyn Source) -> Result<(), Problem>;
    fn draw(&mut self, grid: &mut Grid, theme: &Theme);
    fn key(&mut self, key: Key, source: &mut dyn Source) -> Flow;
    fn interval_ms(&self) -> u64;
    /// One line of state for the log after each key (tests follow it).
    fn status(&self) -> String;
}

/// Name of the task with `pid`, or "?".
pub fn task_name(tasks: &[Task], pid: u64) -> &str { tasks.iter().find(|t| t.pid == pid).map_or("?", |t| t.name.as_str()) }

/// Tasks in spawn-tree order: each followed by its children (ordered by `before`), with its depth. A task whose parent
/// is not in the list is a root.
pub fn tree<'a>(tasks: &[&'a Task], before: &dyn Fn(&Task, &Task) -> core::cmp::Ordering) -> Vec<(&'a Task, usize)> {
    let mut out = Vec::with_capacity(tasks.len());
    let mut roots: Vec<&Task> = tasks.iter().copied().filter(|t| t.parent == 0 || t.parent == t.pid || !tasks.iter().any(|p| p.pid == t.parent)).collect();
    roots.sort_by(|a, b| before(a, b));
    fn visit<'a>(task: &'a Task, depth: usize, tasks: &[&'a Task], before: &dyn Fn(&Task, &Task) -> core::cmp::Ordering, out: &mut Vec<(&'a Task, usize)>) {
        if out.iter().any(|(t, _)| t.pid == task.pid) { return; } // never loop on a malformed table
        out.push((task, depth));
        let mut children: Vec<&Task> = tasks.iter().copied().filter(|t| t.parent == task.pid && t.pid != task.pid).collect();
        children.sort_by(|a, b| before(a, b));
        for child in children { visit(child, depth + 1, tasks, before, out); }
    }
    for root in roots { visit(root, 0, tasks, before, &mut out); }
    out
}
