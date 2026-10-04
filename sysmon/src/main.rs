#![no_std]
#![no_main]
// sysmon: holds the observe privilege (from init) and serves idl/sysinfo.wit: the kernel's STAT records and a history of
// load samples every 100 ms (300 kept) and every second (600 kept), with load averages. Clients are rate-limited
// (MC-10.2): each may make 20 requests at once and 40 per second. Sampling runs at a fixed period into rings allocated
// once at start; replies are built on the program heap (the 64 KiB stack is too small for 600 samples).
extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;
use mind::abi::*;
use mind::idl::sysinfo::{self, Error, Request};
use mind::idl::wire;
use mind::ipc::{self, Endpoint};
use mind::mem::{Mapping, Pages};
use mind::stat;

mod limit;
use limit::Limiter;

const RECEIVED: usize = 9;
const FAST_MS: u64 = 100;
const FAST: usize = 300;
const SLOW: usize = 600;

#[derive(Clone, Copy, Default)]
struct Sample { busy: [u16; 8], interrupts: u32, syscalls: u32, messages: u32, switches: u32, used_kib: u32, tasks: u8, runnable: u8 }

struct Ring<const N: usize> { items: [Sample; N], next: usize, count: usize }
impl<const N: usize> Ring<N> {
    const fn new() -> Self { Self { items: [Sample { busy: [0; 8], interrupts: 0, syscalls: 0, messages: 0, switches: 0, used_kib: 0, tasks: 0, runnable: 0 }; N], next: 0, count: 0 } }
    fn push(&mut self, sample: Sample) { self.items[self.next] = sample; self.next = (self.next + 1) % N; self.count = (self.count + 1).min(N); }
    // The last `count` samples, oldest first.
    fn last(&self, count: usize) -> impl Iterator<Item = &Sample> { let count = count.min(self.count); (0..count).map(move |i| &self.items[(self.next + N - count + i) % N]) }
}

// Totals at the previous sample, to turn the kernel's counters into rates.
#[derive(Default)]
struct Totals { busy: [u64; 8], idle: [u64; 8], interrupts: u64, syscalls: u64, messages: u64, switches: u64 }

struct Monitor {
    scratch: Pages, fast: Box<Ring<FAST>>, slow: Box<Ring<SLOW>>, totals: Totals, pending: [Sample; 10], pending_count: usize,
    load: [u64; 3], // runnable tasks, fixed point x 2048
    limiter: Limiter,
}

// e^(-1/60), e^(-1/300), e^(-1/900) x 2048: one-second samples into 1, 5 and 15 minute averages.
const DECAY: [u64; 3] = [2014, 2041, 2046];

impl Monitor {
    fn buffer(&mut self) -> &mut [u8] { self.scratch.as_mut_slice() }

    fn sample(&mut self) {
        let mut sample = Sample::default();
        let (mut interrupts, mut switches) = (0u64, 0u64);
        if let Ok(records) = stat::read(STAT_CPUS, 0, self.scratch.as_mut_slice()) {
            for (index, cpu) in records.iter::<CpuStat>().enumerate().take(8) {
                let busy = cpu.busy_ns.saturating_sub(self.totals.busy[index]);
                let idle = cpu.idle_ns.saturating_sub(self.totals.idle[index]);
                sample.busy[index] = if busy + idle == 0 { 0 } else { (busy * 1000 / (busy + idle)) as u16 };
                self.totals.busy[index] = cpu.busy_ns; self.totals.idle[index] = cpu.idle_ns;
                interrupts += cpu.interrupts; switches += cpu.switches;
            }
        }
        let (mut syscalls, mut tasks, mut runnable) = (0u64, 0u8, 0u8);
        if let Ok(records) = stat::read(STAT_TASKS, 0, self.scratch.as_mut_slice()) {
            for task in records.iter::<TaskStat>() {
                syscalls += task.calls; tasks += 1;
                if matches!(task.state, TASK_READY | TASK_RUNNING) { runnable += 1; }
            }
        }
        let mut messages = 0u64;
        if let Ok(records) = stat::read(STAT_ENDPOINTS, 0, self.scratch.as_mut_slice()) { messages = records.iter::<EndpointStat>().map(|e| e.messages).sum(); }
        if let Ok(memory) = stat::one::<MemoryStat>(STAT_MEMORY) { sample.used_kib = (memory.arena_used / 1024) as u32; }
        let delta = |now: u64, before: &mut u64| { let d = now.saturating_sub(*before); *before = now; d.min(u32::MAX as u64) as u32 };
        sample.interrupts = delta(interrupts, &mut self.totals.interrupts);
        sample.switches = delta(switches, &mut self.totals.switches);
        sample.syscalls = delta(syscalls, &mut self.totals.syscalls);
        sample.messages = delta(messages, &mut self.totals.messages);
        sample.tasks = tasks; sample.runnable = runnable.saturating_sub(1); // not counting sysmon itself
        self.fast.push(sample);
        // Every tenth sample makes a one-second sample: average load, summed counts, the latest levels.
        self.pending[self.pending_count] = sample; self.pending_count += 1;
        if self.pending_count == self.pending.len() {
            let pending = &self.pending[..self.pending_count];
            let mut slow = *pending.last().unwrap();
            for cpu in 0..8 { slow.busy[cpu] = (pending.iter().map(|s| s.busy[cpu] as u32).sum::<u32>() / pending.len() as u32) as u16; }
            slow.interrupts = pending.iter().map(|s| s.interrupts).sum(); slow.syscalls = pending.iter().map(|s| s.syscalls).sum();
            slow.messages = pending.iter().map(|s| s.messages).sum(); slow.switches = pending.iter().map(|s| s.switches).sum();
            let runnable = pending.iter().map(|s| s.runnable as u64).sum::<u64>() * 2048 / pending.len() as u64;
            for (load, decay) in self.load.iter_mut().zip(DECAY) { *load = (*load * decay + runnable * (2048 - decay)) / 2048; }
            self.slow.push(slow);
            self.pending_count = 0;
        }
    }
}

fn wire_sample(s: &Sample) -> sysinfo::Sample {
    let pack = |range: &[u16]| range.iter().enumerate().fold(0u64, |word, (i, &v)| word | (v as u64) << (16 * i));
    sysinfo::Sample { busy_low: pack(&s.busy[..4]), busy_high: pack(&s.busy[4..]), interrupts: s.interrupts, syscalls: s.syscalls, messages: s.messages, switches: s.switches, used_kib: s.used_kib, tasks: s.tasks, runnable: s.runnable }
}

fn serve(monitor: &mut Monitor, request: Request, out: &mut [u8]) -> mind::Result<()> {
    macro_rules! records { ($class:expr, $arg:expr, $ty:ty) => {{
        let scratch = monitor.scratch.as_mut_slice();
        match stat::read($class, $arg, scratch) { Ok(records) => Ok(records), Err(mind::Error::NotFound) => Err(Error::NotFound), Err(_) => Err(Error::Unavailable) }
    }}; }
    match request {
        Request::Tasks { .. } => {
            let list: Vec<TaskStat> = match records!(STAT_TASKS, 0, TaskStat) { Ok(r) => r.iter::<TaskStat>().take(24).collect(), Err(e) => return sysinfo::reply_tasks(out, Err(e)) };
            let n = list.len();
            let names: Vec<&str> = list.iter().map(|t| { let len = t.name.iter().position(|&b| b == 0).unwrap_or(t.name.len()); core::str::from_utf8(&t.name[..len]).unwrap_or("?") }).collect();
            let items: Vec<sysinfo::Task> = (0..n).map(|i| { let t = &list[i]; sysinfo::Task { pid: t.pid, parent: t.parent, run_ns: t.run_ns, runs: t.runs, calls: t.calls, sent: t.sent, received: t.received, started_ns: t.started_ns,
                image: t.image_bytes, stack: t.stack_bytes, screen: t.screen_bytes, heap: t.heap_bytes, shared: t.shared_bytes, kernel: t.kernel_bytes, wait: t.wait, heap_blocks: t.heap_blocks, caps: t.caps,
                quota_tasks: t.quota_tasks, used_tasks: t.used_tasks, quota_endpoints: t.quota_endpoints, used_endpoints: t.used_endpoints, name: names[i], state: t.state, cpu: t.cpu, flags: t.flags } }).collect();
            sysinfo::reply_tasks(out, Ok(&items))
        }
        Request::Cpus { .. } => {
            let mut items = [sysinfo::Cpu { busy_ns: 0, idle_ns: 0, ticks: 0, switches: 0, interrupts: 0, current: 0, apic: 0, online: false }; 8]; let mut n = 0;
            match records!(STAT_CPUS, 0, CpuStat) { Ok(r) => for c in r.iter::<CpuStat>().take(8) { items[n] = sysinfo::Cpu { busy_ns: c.busy_ns, idle_ns: c.idle_ns, ticks: c.ticks, switches: c.switches, interrupts: c.interrupts, current: c.current, apic: c.apic, online: c.online != 0 }; n += 1; }, Err(e) => return sysinfo::reply_cpus(out, Err(e)) }
            sysinfo::reply_cpus(out, Ok(&items[..n]))
        }
        Request::Memory { .. } => {
            let m = match stat::read(STAT_MEMORY, 1, monitor.scratch.as_mut_slice()).ok().and_then(|r| r.iter::<MemoryStat>().next()) { Some(m) => m, None => return sysinfo::reply_memory(out, Err(Error::Unavailable)) };
            sysinfo::reply_memory(out, Ok(sysinfo::Memory { arena: m.arena_bytes, used: m.arena_used, free: m.arena_free, largest_free: m.largest_free, images: m.task_images, stacks: m.task_stacks, screens: m.task_screens, heaps: m.task_heaps,
                task_kernel: m.task_kernel, page_tables: m.page_tables, objects: m.objects, objects_limit: m.objects_limit, dma: m.dma, dma_limit: m.dma_limit, mapped: m.shared_mapped, other: m.kernel_other,
                tasks: m.tasks, tasks_limit: m.tasks_limit, endpoints: m.endpoints, endpoints_limit: m.endpoints_limit }))
        }
        Request::Physmap { .. } => {
            let items: Vec<sysinfo::Range> = match records!(STAT_PHYSMAP, 0, PhysRange) { Ok(r) => r.iter::<PhysRange>().take(220).map(|p| sysinfo::Range { start: p.start, bytes: p.bytes, kind: p.kind, detail: p.detail }).collect(), Err(e) => return sysinfo::reply_physmap(out, Err(e)) };
            sysinfo::reply_physmap(out, Ok(&items))
        }
        Request::Vmap { pid, .. } => {
            let mut items = [sysinfo::Region { start: 0, bytes: 0, kind: 0, flags: 0 }; 80]; let mut n = 0;
            match records!(STAT_VMAP, pid, VmRegion) { Ok(r) => for v in r.iter::<VmRegion>().take(80) { items[n] = sysinfo::Region { start: v.start, bytes: v.bytes, kind: v.kind, flags: v.flags }; n += 1; }, Err(e) => return sysinfo::reply_vmap(out, Err(e)) }
            sysinfo::reply_vmap(out, Ok(&items[..n]))
        }
        Request::Caps { pid, .. } => {
            let mut items = [sysinfo::Capability { node: 0, parent: 0, size: 0, base: 0, slot: 0, generation: 0, kind: 0, rights: 0, endpoint: 0 }; 32]; let mut n = 0;
            match records!(STAT_CAPS, pid, CapStat) { Ok(r) => for c in r.iter::<CapStat>().take(32) { items[n] = sysinfo::Capability { node: c.node, parent: c.parent, size: c.size, base: c.base, slot: c.slot, generation: c.generation, kind: c.kind, rights: c.rights, endpoint: c.endpoint }; n += 1; }, Err(e) => return sysinfo::reply_caps(out, Err(e)) }
            sysinfo::reply_caps(out, Ok(&items[..n]))
        }
        Request::Endpoints { .. } => {
            let items: Vec<sysinfo::Endpoint> = match records!(STAT_ENDPOINTS, 0, EndpointStat) { Ok(r) => r.iter::<EndpointStat>().take(64).map(|e| sysinfo::Endpoint { messages: e.messages, busy: e.busy, timeouts: e.timeouts, index: e.index, creator: e.creator, server: e.server, receivers: e.receivers, holders: e.holders, waiting: e.waiting, receiving: e.receiving, irq: e.irq }).collect(), Err(e) => return sysinfo::reply_endpoints(out, Err(e)) };
            sysinfo::reply_endpoints(out, Ok(&items))
        }
        Request::Irqs { .. } => {
            let mut items = [sysinfo::Irq { count: 0, line: 0, holder: 0, endpoint: 0, masked: false }; 16]; let mut n = 0;
            match records!(STAT_IRQS, 0, IrqStat) { Ok(r) => for i in r.iter::<IrqStat>().take(16) { items[n] = sysinfo::Irq { count: i.count, line: i.line, holder: i.holder, endpoint: i.endpoint, masked: i.masked != 0 }; n += 1; }, Err(e) => return sysinfo::reply_irqs(out, Err(e)) }
            sysinfo::reply_irqs(out, Ok(&items[..n]))
        }
        Request::Devices { .. } => {
            let items: Vec<sysinfo::Device> = match records!(STAT_DEVICES, 0, DeviceStat) { Ok(r) => r.iter::<DeviceStat>().take(64).map(|d| { let b = d.bar_bytes; sysinfo::Device { bar0: b[0], bar1: b[1], bar2: b[2], bar3: b[3], bar4: b[4], bar5: b[5], class: d.class, irq: d.irq, holder: d.holder, index: d.index, location: d.location, io_bars: d.io_bars } }).collect(), Err(e) => return sysinfo::reply_devices(out, Err(e)) };
            sysinfo::reply_devices(out, Ok(&items))
        }
        Request::History { slow, count, .. } => {
            let items: Vec<sysinfo::Sample> = if slow { monitor.slow.last(count as usize).map(wire_sample).collect() } else { monitor.fast.last(count as usize).map(wire_sample).collect() };
            sysinfo::reply_history(out, Ok(&items))
        }
        Request::Load { .. } => {
            let [one, five, fifteen] = monitor.load.map(|l| (l * 100 / 2048) as u32);
            sysinfo::reply_load(out, Ok(sysinfo::Load { one, five, fifteen, uptime_ms: mind::time::uptime_ms() as u64, fast_ms: FAST_MS as u32, slow_ms: 1000, fast_count: monitor.fast.count as u32, slow_count: monitor.slow.count as u32 }))
        }
    }
}

// A refused request (over the rate) gets the `busy` error of its function.
fn refuse(request: Request, out: &mut [u8]) -> mind::Result<()> {
    let busy = Error::Busy;
    match request {
        Request::Tasks { .. } => sysinfo::reply_tasks(out, Err(busy)), Request::Cpus { .. } => sysinfo::reply_cpus(out, Err(busy)),
        Request::Memory { .. } => sysinfo::reply_memory(out, Err(busy)), Request::Physmap { .. } => sysinfo::reply_physmap(out, Err(busy)),
        Request::Vmap { .. } => sysinfo::reply_vmap(out, Err(busy)), Request::Caps { .. } => sysinfo::reply_caps(out, Err(busy)),
        Request::Endpoints { .. } => sysinfo::reply_endpoints(out, Err(busy)), Request::Irqs { .. } => sysinfo::reply_irqs(out, Err(busy)),
        Request::Devices { .. } => sysinfo::reply_devices(out, Err(busy)), Request::History { .. } => sysinfo::reply_history(out, Err(busy)),
        Request::Load { .. } => sysinfo::reply_load(out, Err(busy)),
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let Some(scratch) = Pages::new(16 * 1024) else { mind::println!("[SYSMON] NO MEMORY"); return };
    let mut monitor = Monitor { scratch, fast: Box::new(Ring::new()), slow: Box::new(Ring::new()), totals: Totals::default(), pending: [Sample::default(); 10], pending_count: 0, load: [0; 3], limiter: Limiter::new() };
    if stat::read(STAT_CPUS, 0, monitor.buffer()).is_err() { mind::println!("[SYSMON] NO OBSERVE PRIVILEGE"); return; }
    monitor.sample();
    mind::println!("[SYSMON] READY: SAMPLES EVERY {} MS", FAST_MS);
    let mut next = mind::time::uptime_ms() as u64 + FAST_MS;
    loop {
        let now = mind::time::uptime_ms() as u64;
        if now >= next {
            monitor.sample();
            next += FAST_MS;
            if next <= now { next = now + FAST_MS; } // fell behind: skip, never burst
            continue;
        }
        let wait = (next - now).max(10) as u32;
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, wait) else { continue };
        let decoded = sysinfo::decode(&request, RECEIVED);
        let mut mapping = if request.cap_received { Mapping::new(RECEIVED).ok() } else { None };
        let mut empty = [0u8; 0];
        let out: &mut [u8] = match mapping.as_mut() { Some(m) => m.as_mut_slice(), None => &mut empty };
        let _ = match decoded {
            Ok(request_data) if !monitor.limiter.admit(request.sender, now) => refuse(request_data, out),
            Ok(request_data) => serve(&mut monitor, request_data, out),
            Err(reason) => wire::reject(reason),
        };
        drop(mapping);
        if request.cap_received { let _ = ipc::drop_cap(RECEIVED); }
    }
}
