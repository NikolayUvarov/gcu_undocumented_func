//! The monitors' program side: a sysmon client over the endpoint the shell lends in slot `SLOT_SYSINFO`
//! (`Endpoint::SYSINFO`) and the loop that refreshes, draws and passes keys to a `Tool`.
use crate::model::*;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::BootInfo;
use mind::idl::{init as lifecycle, sysinfo};
use mind::ipc::Endpoint;
use mind::tui::widgets::message;
use mind::tui::{Terminal, DARK};

/// sysmon through its client endpoint (idl/sysinfo.wit); every call lends its own buffer and copies the reply out.
pub struct Client;

fn take<T>(reply: mind::Result<core::result::Result<T, sysinfo::Error>>) -> Result<T, Problem> {
    match reply {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(sysinfo::Error::Busy)) => Err(Problem::Busy),
        Ok(Err(sysinfo::Error::NotFound)) => Err(Problem::NotFound),
        Ok(Err(sysinfo::Error::Denied)) => Err(Problem::Denied),
        Ok(Err(_)) => Err(Problem::Failed),
        // Nothing in the slot (or no right to call): the program was not given a sysmon client.
        Err(mind::Error::Invalid) | Err(mind::Error::Rights) => Err(Problem::NoAccess),
        Err(_) => Err(Problem::Failed),
    }
}

impl Client {
    pub fn new() -> Option<Self> { Some(Self) }
}

// A reply of init's lifecycle interface as text for the notice line.
fn lifecycle_error(error: lifecycle::Error) -> String {
    String::from(match error {
        lifecycle::Error::NotFound => "no such task", lifecycle::Error::Running => "it runs already", lifecycle::Error::Stopped => "it does not run",
        lifecycle::Error::Denied => "init and the shell cannot be stopped", lifecycle::Error::NoDevice => "its device is missing", lifecycle::Error::Failed => "init could not do it",
    })
}

fn lifecycle_call<T>(reply: mind::Result<core::result::Result<T, lifecycle::Error>>) -> Result<T, String> {
    match reply {
        Ok(result) => result.map_err(lifecycle_error),
        Err(mind::Error::Invalid) | Err(mind::Error::Rights) => Err(String::from(NO_LIFECYCLE)),
        Err(error) => Err(alloc::format!("init did not answer ({:?})", error)),
    }
}

const LIFECYCLE: Endpoint = Endpoint(mind::abi::SLOT_LIFECYCLE);

impl Source for Client {
    fn stop(&mut self, task: &Task) -> Result<(), String> {
        if task.service() { lifecycle_call(lifecycle::stop(LIFECYCLE, &task.name)) } else { lifecycle_call(lifecycle::stop_task(LIFECYCLE, task.pid)) }
    }
    fn restart(&mut self, name: &str) -> Result<u64, String> { lifecycle_call(lifecycle::restart(LIFECYCLE, name)) }
    fn tasks(&mut self) -> Result<Vec<Task>, Problem> {
        // Page by page: there is no limit on tasks (sysinfo.wit 4.0).
        let mut tasks = Vec::new();
        loop {
            let list = take(sysinfo::tasks(Endpoint::SYSINFO, tasks.len() as u32))?;
            tasks.extend(list.as_slice().iter().map(|t| Task { pid: t.pid, parent: t.parent, run_ns: t.run_ns, runs: t.runs, calls: t.calls, sent: t.sends, received: t.receives, started_ns: t.started_ns,
            image: t.image, stack: t.stack, screen: t.screen, heap: t.heap, shared: t.shared, retained: t.retained, wait: t.wait_on as u64, heap_blocks: t.heap_blocks, caps: t.caps,
            quota_tasks: t.quota_tasks as u32, used_tasks: t.used_tasks as u32, quota_endpoints: t.quota_endpoints as u32, used_endpoints: t.used_endpoints as u32,
            name: String::from(t.name.as_str()), state: t.wait, cpu: t.cpu, flags: t.flags, kernel: t.kernel }));
            if list.len() < 40 { return Ok(tasks); }
        }
    }
    fn cpus(&mut self) -> Result<Vec<Cpu>, Problem> {
        // Page by page: the kernel starts every CPU the firmware reports (sysinfo.wit 4.0).
        let mut cpus = Vec::new();
        loop {
            let list = take(sysinfo::cpus(Endpoint::SYSINFO, cpus.len() as u32))?;
            cpus.extend(list.as_slice().iter().map(|c| Cpu { busy_ns: c.busy_ns, idle_ns: c.idle_ns, ticks: c.ticks, switches: c.switches, interrupts: c.interrupts, current: c.current, apic: c.apic, online: c.online }));
            if list.len() < 64 { return Ok(cpus); }
        }
    }
    fn memory(&mut self) -> Result<Memory, Problem> {
        let m = take(sysinfo::memory(Endpoint::SYSINFO))?;
        Ok(Memory { arena: m.arena, used: m.used, free: m.free, images: m.images, stacks: m.stacks, task_pages: m.task_pages, screens: m.screens, heaps: m.heaps,
                    objects: m.objects, objects_limit: m.objects_limit, dma: m.dma, dma_limit: m.dma_limit, tasks: m.tasks, endpoints: m.endpoints,
                    largest_free: m.largest_free, page_tables: m.page_tables, shared: m.shared, tasks_limit: m.tasks_limit, endpoints_limit: m.endpoints_limit })
    }
    fn physmap(&mut self) -> Result<Vec<Range>, Problem> {
        let list = take(sysinfo::physmap(Endpoint::SYSINFO))?;
        Ok(list.as_slice().iter().map(|r| Range { start: r.start, bytes: r.pages * 4096, kind: r.kind, detail: r.index }).collect())
    }
    fn vmap(&mut self, pid: u64) -> Result<Vec<Region>, Problem> {
        let list = take(sysinfo::vmap(Endpoint::SYSINFO, pid))?;
        Ok(list.as_slice().iter().map(|r| Region { start: r.start, bytes: r.size, kind: r.kind, flags: r.flags }).collect())
    }
    fn caps(&mut self, pid: u64) -> Result<Vec<Capability>, Problem> {
        // Page by page: a capability table grows to 4095 slots (sysinfo.wit 4.0).
        let mut caps = Vec::new();
        loop {
            let list = take(sysinfo::caps(Endpoint::SYSINFO, pid, caps.len() as u32))?;
            caps.extend(list.as_slice().iter().map(|c| Capability { node: c.node, parent: c.parent, size: c.size, slot: c.slot, generation: c.generation, kind: c.kind, rights: c.rights, badge: c.badge, endpoint: c.endpoint }));
            if list.len() < 64 { return Ok(caps); }
        }
    }
    fn irqs(&mut self) -> Result<Vec<Irq>, Problem> {
        let list = take(sysinfo::irqs(Endpoint::SYSINFO))?;
        Ok(list.as_slice().iter().map(|i| Irq { count: i.count, line: i.line, holder: i.holder, endpoint: i.endpoint, masked: i.masked, holders: i.holders }).collect())
    }
    fn devices(&mut self) -> Result<Vec<Device>, Problem> {
        let list = take(sysinfo::devices(Endpoint::SYSINFO))?;
        Ok(list.as_slice().iter().enumerate().map(|(i, d)| Device { bars: [d.bar0, d.bar1, d.bar2, d.bar3, d.bar4, d.bar5], class: d.class, irq: d.irq, holder: d.holder, index: i as u32, location: d.location, io_bars: d.io_bars }).collect())
    }
    fn history(&mut self, slow: bool, count: u16) -> Result<Vec<Sample>, Problem> {
        // In replies of up to 150 samples.
        let mut samples = Vec::new();
        while samples.len() < count as usize {
            let list = take(sysinfo::history(Endpoint::SYSINFO, slow, count, samples.len() as u16))?;
            samples.extend(list.as_slice().iter().map(|s| {
                let mut busy = [0; CPUS_KEPT];
                busy[..s.busy.len()].copy_from_slice(s.busy.as_slice());
                Sample { busy, busy_total: s.busy_total, busy_max: s.busy_max, interrupts: s.interrupts, syscalls: s.syscalls, messages: s.messages,
                         switches: s.switches, used_kib: s.used_kib, tasks: s.tasks, runnable: s.runnable }
            }));
            if list.len() < 150 { break; }
        }
        Ok(samples)
    }
    fn load(&mut self) -> Result<Load, Problem> {
        let l = take(sysinfo::load(Endpoint::SYSINFO))?;
        Ok(Load { one: l.one, five: l.five, fifteen: l.fifteen, uptime_ms: l.uptime_ms, fast_ms: l.fast_ms, slow_ms: l.slow_ms, fast_count: l.fast_count, slow_count: l.slow_count })
    }
    fn endpoints(&mut self) -> Result<Vec<EndpointInfo>, Problem> {
        let mut endpoints = Vec::new();
        loop {
            let list = take(sysinfo::endpoints(Endpoint::SYSINFO, endpoints.len() as u32))?;
            endpoints.extend(list.as_slice().iter().map(|e| EndpointInfo { index: e.index, creator: e.creator, server: e.server, holders: e.holders, receivers: e.receivers, senders: e.senders,
                                                                           receiving: e.receiving, messages: e.messages, busy: e.busy, timeouts: e.timeouts, irq: e.irq }));
            if list.len() < 128 { return Ok(endpoints); }
        }
    }
    fn holders(&mut self, index: u32) -> Result<Vec<Holder>, Problem> {
        let list = take(sysinfo::holders(Endpoint::SYSINFO, index))?;
        Ok(list.as_slice().iter().map(|h| Holder { pid: h.pid, slot: h.slot, rights: h.rights, badge: h.badge }).collect())
    }
    fn authority(&mut self) -> Result<Vec<AuthorityEntry>, Problem> {
        // In replies of up to 128 entries (64 slots of at most 40 tasks: 20 replies at most).
        let mut entries = Vec::new();
        for _ in 0..20 {
            let list = take(sysinfo::authority(Endpoint::SYSINFO, entries.len() as u32))?;
            entries.extend(list.as_slice().iter().map(|e| AuthorityEntry { node: e.node, parent: e.parent, pid: e.pid, size: e.size, slot: e.slot, generation: e.generation,
                                                                         kind: e.kind, rights: e.rights, badge: e.badge, endpoint: e.endpoint }));
            if list.len() < 128 { break; }
        }
        Ok(entries)
    }
    fn now_ns(&self) -> u64 { mind::time::monotonic_ns() }
}

/// Runs `tool` on the program's screen until it quits. `name` prefixes the log lines (`[TOP] READY`).
pub fn run(info: &'static BootInfo, name: &str, tool: &mut dyn Tool) {
    let Some(mut term) = Terminal::open(info, &name.to_ascii_lowercase()) else { return };
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
