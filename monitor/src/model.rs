//! What the monitors show, as plain values copied out of sysmon's replies (idl/sysinfo.wit), and the interface they
//! read it through.
use crate::keys::Key;
use crate::tui::{Grid, Theme};
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Task {
    pub pid: u64, pub parent: u64, pub run_ns: u64, pub runs: u64, pub calls: u64, pub sent: u64, pub received: u64, pub started_ns: u64,
    pub image: u64, pub stack: u64, pub screen: u64, pub heap: u64, pub shared: u64, pub retained: u64, pub wait: u64,
    pub heap_blocks: u32, pub caps: u32, pub quota_tasks: u32, pub used_tasks: u32, pub quota_endpoints: u32, pub used_endpoints: u32,
    pub name: String, pub state: u8, pub cpu: u8, pub flags: u8,
    /// Kernel memory for the task: context, mailbox, info and exit pages, page tables.
    pub kernel: u64,
}

/// Flags of a task (idl/sysinfo.wit `task.flags`, as sysmon sets them from `mind::stat::TASK_*`).
pub const TASK_SERVICE: u8 = 1;
pub const TASK_SCREEN: u8 = 2;
pub const TASK_FOCUS: u8 = 4;

// `state` is what the task waits for (WAIT_*), `wait` the endpoint index, IRQ line or server PID; `retained` is freed
// memory still referenced elsewhere and charged to the task; `flags`: `mind::stat::TASK_*`.
impl Task {
    pub fn service(&self) -> bool { self.flags & TASK_SERVICE != 0 }
    /// Kernel memory the task holds: image, stack, screen, heap and retained memory (shared mappings not counted).
    pub fn memory(&self) -> u64 { self.image + self.stack + self.screen + self.heap + self.retained }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cpu { pub busy_ns: u64, pub idle_ns: u64, pub ticks: u64, pub switches: u64, pub interrupts: u64, pub current: u64, pub apic: u32, pub online: bool }

/// The kernel arena by use (StatMemory, bytes), the largest block it can still allocate, the live tasks and endpoints
/// and the kernel's limits for them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Memory {
    pub arena: u64, pub used: u64, pub free: u64, pub images: u64, pub stacks: u64, pub task_pages: u64, pub screens: u64, pub heaps: u64,
    pub objects: u64, pub objects_limit: u64, pub dma: u64, pub dma_limit: u64, pub tasks: u32, pub endpoints: u32,
    pub largest_free: u64, pub page_tables: u64, pub shared: u64, pub tasks_limit: u32, pub endpoints_limit: u32,
}
impl Memory {
    /// Used arena bytes not in a category of their own (kernel structures).
    pub fn other(&self) -> u64 { self.used.saturating_sub(self.images + self.stacks + self.task_pages + self.page_tables + self.screens + self.heaps + self.objects + self.dma) }
}

/// A physical range: UEFI memory type (0..15) or platform layout (`PHYS_*` from PHYS_PLATFORM on); `detail` is the boot
/// image or device index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Range { pub start: u64, pub bytes: u64, pub kind: u32, pub detail: u32 }
impl Range { pub fn end(&self) -> u64 { self.start.saturating_add(self.bytes) } }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Region { pub start: u64, pub bytes: u64, pub kind: u32, pub flags: u32 }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capability { pub node: u64, pub parent: u64, pub size: u64, pub slot: u32, pub generation: u32, pub kind: u32, pub rights: u32, pub badge: u32, pub endpoint: u32 }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Irq { pub count: u64, pub line: u32, pub holder: u64, pub endpoint: u32, pub masked: bool, pub holders: u32 }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// `location`: bus << 8 | device << 3 | function; `io_bars`: bit i set if BAR i is a port range.
pub struct Device { pub bars: [u64; 6], pub class: u32, pub irq: u32, pub holder: u64, pub index: u32, pub location: u32, pub io_bars: u32 }

/// An endpoint (StatEndpoint): the index is a label, not an authority; the server is the task with the newest receive
/// right; `senders` wait to be received, in the order they sent, `receiving` wait for a message; `irq` is the bound line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EndpointInfo { pub index: u32, pub creator: u64, pub server: u64, pub holders: u32, pub receivers: u32, pub senders: u32, pub receiving: u32, pub messages: u64, pub busy: u64, pub timeouts: u64, pub irq: u32 }

/// A task holding a capability for an endpoint: slot, rights (CAP_*) and badge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Holder { pub pid: u64, pub slot: u32, pub rights: u32, pub badge: u32 }

/// A capability of some task with its derivation links (sysinfo 3.0 `authority`): `node` is its derivation node,
/// `parent` the node it was derived from (0: a root). Revoking it removes the capabilities below its node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthorityEntry { pub node: u64, pub parent: u64, pub pid: u64, pub size: u64, pub slot: u32, pub generation: u32, pub kind: u32, pub rights: u32, pub badge: u32, pub endpoint: u32 }

/// CPUs whose busy share a sample keeps one by one (sysinfo.wit 4.0); the mean and the maximum cover every CPU.
pub const CPUS_KEPT: usize = 16;

/// One load sample: busy per mille of CPUs 0..15, of every CPU on average and of the busiest, and counts during the period.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sample { pub busy: [u16; CPUS_KEPT], pub busy_total: u16, pub busy_max: u16, pub interrupts: u32, pub syscalls: u32, pub messages: u32, pub switches: u32, pub used_kib: u32, pub tasks: u32, pub runnable: u32 }

/// Load averages (runnable tasks x 100), uptime and sampling periods.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Load { pub one: u32, pub five: u32, pub fifteen: u32, pub uptime_ms: u64, pub fast_ms: u32, pub slow_ms: u32, pub fast_count: u32, pub slow_count: u32 }

/// Every item of a list sysmon gives page by page from a position on (sysinfo.wit 4.0): `fetch(start)` returns one page
/// of at most `page` items, and a shorter one is the last.
pub fn paged<T>(page: usize, mut fetch: impl FnMut(u32) -> Result<Vec<T>, Problem>) -> Result<Vec<T>, Problem> {
    let mut items = Vec::new();
    loop {
        let more = fetch(items.len() as u32)?;
        let last = more.len() < page;
        items.extend(more);
        if last { return Ok(items); }
    }
}

/// Why data could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// No sysmon client in the program's slot: it was not started by a launcher that lends one.
    NoAccess,
    /// sysmon refused the request (over the client's rate); the previous data stays.
    Busy,
    NotFound,
    /// sysmon tells who holds what only to a client with the authority badge (`REQUEST_AUTHORITY`).
    Denied,
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
    /// Live endpoints (sysinfo `endpoints`).
    fn endpoints(&mut self) -> Result<Vec<EndpointInfo>, Problem> { Ok(Vec::new()) }
    /// The tasks holding a capability for endpoint `index` (sysinfo 2.1 `holders`).
    fn holders(&mut self, _index: u32) -> Result<Vec<Holder>, Problem> { Ok(Vec::new()) }
    /// The capabilities of all tasks with their derivation links (sysinfo 3.0 `authority`, the authority badge).
    fn authority(&mut self) -> Result<Vec<AuthorityEntry>, Problem> { Ok(Vec::new()) }
    /// Monotonic nanoseconds (the kernel's clock: the same base as `Task::started_ns`).
    fn now_ns(&self) -> u64;
    /// Stops a task through init's lifecycle requests (idl/init.wit 1.1): a service by name, an application by PID.
    fn stop(&mut self, _task: &Task) -> Result<(), String> { Err(String::from(NO_LIFECYCLE)) }
    /// Restarts a service through init; returns its new PID.
    fn restart(&mut self, _name: &str) -> Result<u64, String> { Err(String::from(NO_LIFECYCLE)) }
}

/// What stop and restart say without a lifecycle client.
pub const NO_LIFECYCLE: &str = "no lifecycle control: start the tool from the shell";

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
