//! The monitors' program side: a sysmon client over the endpoint the shell lends in slot `SLOT_SYSINFO`
//! (`Endpoint::SYSINFO`) and the loop that refreshes, draws and passes keys to a `Tool`.
use crate::model::*;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::BootInfo;
use mind::gfx::Screen;
use mind::idl::{sysinfo, wire};
use mind::ipc::Endpoint;
use mind::tui::widgets::message;
use mind::tui::{Terminal, DARK};

/// sysmon through a buffer lent with every call (idl/sysinfo.wit); replies are copied out at once.
pub struct Client { shared: wire::Shared }

fn take<T>(reply: mind::Result<core::result::Result<T, sysinfo::Error>>) -> Result<T, Problem> {
    match reply {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(sysinfo::Error::Busy)) => Err(Problem::Busy),
        Ok(Err(sysinfo::Error::NotFound)) => Err(Problem::NotFound),
        Ok(Err(_)) => Err(Problem::Failed),
        // Nothing in the slot (or no right to call): the program was not given a sysmon client.
        Err(mind::Error::Invalid) | Err(mind::Error::Rights) => Err(Problem::NoAccess),
        Err(_) => Err(Problem::Failed),
    }
}

impl Client {
    pub fn new() -> Option<Self> { wire::Shared::new(32 * 1024).ok().map(|shared| Self { shared }) }
}

impl Source for Client {
    fn tasks(&mut self) -> Result<Vec<Task>, Problem> {
        let list = take(sysinfo::tasks(Endpoint::SYSINFO, self.shared.buffer()))?;
        Ok(list.iter().map(|t| Task { pid: t.pid, parent: t.parent, run_ns: t.run_ns, runs: t.runs, calls: t.calls, sent: t.sent, received: t.received, started_ns: t.started_ns,
            image: t.image, stack: t.stack, screen: t.screen, heap: t.heap, shared: t.shared, kernel: t.kernel, wait: t.wait, heap_blocks: t.heap_blocks, caps: t.caps,
            quota_tasks: t.quota_tasks, used_tasks: t.used_tasks, quota_endpoints: t.quota_endpoints, used_endpoints: t.used_endpoints, name: String::from(t.name),
            state: t.state, cpu: t.cpu, flags: t.flags }).collect())
    }
    fn cpus(&mut self) -> Result<Vec<Cpu>, Problem> {
        let list = take(sysinfo::cpus(Endpoint::SYSINFO, self.shared.buffer()))?;
        Ok(list.iter().map(|c| Cpu { busy_ns: c.busy_ns, idle_ns: c.idle_ns, ticks: c.ticks, switches: c.switches, interrupts: c.interrupts, current: c.current, apic: c.apic, online: c.online }).collect())
    }
    fn memory(&mut self) -> Result<Memory, Problem> {
        let m = take(sysinfo::memory(Endpoint::SYSINFO, self.shared.buffer()))?;
        Ok(Memory { arena: m.arena, used: m.used, free: m.free, largest_free: m.largest_free, images: m.images, stacks: m.stacks, screens: m.screens, heaps: m.heaps,
                    task_kernel: m.task_kernel, page_tables: m.page_tables, objects: m.objects, objects_limit: m.objects_limit, dma: m.dma, dma_limit: m.dma_limit,
                    mapped: m.mapped, other: m.other, tasks: m.tasks, tasks_limit: m.tasks_limit, endpoints: m.endpoints, endpoints_limit: m.endpoints_limit })
    }
    fn physmap(&mut self) -> Result<Vec<Range>, Problem> {
        let list = take(sysinfo::physmap(Endpoint::SYSINFO, self.shared.buffer()))?;
        Ok(list.iter().map(|r| Range { start: r.start, bytes: r.bytes, kind: r.kind, detail: r.detail }).collect())
    }
    fn vmap(&mut self, pid: u64) -> Result<Vec<Region>, Problem> {
        let list = take(sysinfo::vmap(Endpoint::SYSINFO, pid, self.shared.buffer()))?;
        Ok(list.iter().map(|r| Region { start: r.start, bytes: r.bytes, kind: r.kind, flags: r.flags }).collect())
    }
    fn caps(&mut self, pid: u64) -> Result<Vec<Capability>, Problem> {
        let list = take(sysinfo::caps(Endpoint::SYSINFO, pid, self.shared.buffer()))?;
        Ok(list.iter().map(|c| Capability { node: c.node, parent: c.parent, size: c.size, base: c.base, slot: c.slot, generation: c.generation, kind: c.kind, rights: c.rights, endpoint: c.endpoint }).collect())
    }
    fn irqs(&mut self) -> Result<Vec<Irq>, Problem> {
        let list = take(sysinfo::irqs(Endpoint::SYSINFO, self.shared.buffer()))?;
        Ok(list.iter().map(|i| Irq { count: i.count, line: i.line, holder: i.holder, endpoint: i.endpoint, masked: i.masked }).collect())
    }
    fn devices(&mut self) -> Result<Vec<Device>, Problem> {
        let list = take(sysinfo::devices(Endpoint::SYSINFO, self.shared.buffer()))?;
        Ok(list.iter().map(|d| Device { bars: [d.bar0, d.bar1, d.bar2, d.bar3, d.bar4, d.bar5], class: d.class, irq: d.irq, holder: d.holder, index: d.index, location: d.location, io_bars: d.io_bars }).collect())
    }
    fn history(&mut self, slow: bool, count: u16) -> Result<Vec<Sample>, Problem> {
        let list = take(sysinfo::history(Endpoint::SYSINFO, self.shared.buffer(), slow, count))?;
        let unpack = |low: u64, high: u64| core::array::from_fn(|i| (if i < 4 { low >> (16 * i) } else { high >> (16 * (i - 4)) } & 0xFFFF) as u16);
        Ok(list.iter().map(|s| Sample { busy: unpack(s.busy_low, s.busy_high), interrupts: s.interrupts, syscalls: s.syscalls, messages: s.messages, switches: s.switches,
                                        used_kib: s.used_kib, tasks: s.tasks, runnable: s.runnable }).collect())
    }
    fn load(&mut self) -> Result<Load, Problem> {
        let l = take(sysinfo::load(Endpoint::SYSINFO, self.shared.buffer()))?;
        Ok(Load { one: l.one, five: l.five, fifteen: l.fifteen, uptime_ms: l.uptime_ms, fast_ms: l.fast_ms, slow_ms: l.slow_ms, fast_count: l.fast_count, slow_count: l.slow_count })
    }
    fn now_ns(&self) -> u64 { mind::time::monotonic_ns() }
}

/// Runs `tool` on the program's screen until it quits. `name` prefixes the log lines (`[TOP] READY`).
pub fn run(info: &'static BootInfo, name: &str, tool: &mut dyn Tool) {
    let Some(mut term) = Screen::new(info).and_then(Terminal::new) else { return };
    let Some(mut client) = Client::new() else { mind::println!("[{}] OUT OF MEMORY", name); return };
    let mut problem = tool.refresh(&mut client).err();
    mind::println!("[{}] READY {}x{}{}", name, term.cols(), term.rows(), if problem == Some(Problem::NoAccess) { " NO ACCESS TO SYSMON" } else { "" });
    let mut next = mind::time::uptime_ms() as u64 + tool.interval_ms();
    loop {
        {
            let mut grid = term.grid();
            tool.draw(&mut grid, &DARK);
            match problem {
                Some(Problem::NoAccess) => message(&mut grid, "No access to sysmon", &["This program reads system information through a sysmon client", "that its launcher lends it. Start it from the shell."], &["Quit"], 0, &DARK),
                Some(Problem::Failed) => message(&mut grid, "sysmon", &["sysmon is not answering; the data shown is old.", "It is retried at every refresh."], &["OK"], 0, &DARK),
                _ => {}
            }
        }
        term.set_cursor(None);
        term.present();
        let now = mind::time::uptime_ms() as u64;
        if now >= next {
            // A refused request (Busy) keeps the previous data.
            problem = match tool.refresh(&mut client) { Err(Problem::Busy) => problem, result => result.err() };
            next = now + tool.interval_ms();
            continue;
        }
        let Some(key) = mind::input::wait_key((next - now) as usize) else { continue };
        if problem == Some(Problem::NoAccess) { if key.is_escape() || key.code() == mind::keys::Code::Enter { break; } continue; }
        match tool.key(key, &mut client) {
            Flow::Quit => break,
            Flow::Refresh => { problem = tool.refresh(&mut client).err(); next = mind::time::uptime_ms() as u64 + tool.interval_ms(); }
            Flow::Redraw | Flow::Ignored => {}
        }
        mind::println!("[{}] {}", name, tool.status());
    }
    mind::println!("[{}] DONE", name);
}
