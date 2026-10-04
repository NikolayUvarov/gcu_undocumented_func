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
use mind::idl::codec::Text;
use mind::idl::sysinfo::{self, Error, Request};
use mind::idl::wire::{self, Call};
use mind::ipc::Endpoint;
use mind::mem::Pages;
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
            for (index, cpu) in records.iter::<StatCpu>().enumerate().take(8) {
                let busy = cpu.busy_ns.saturating_sub(self.totals.busy[index]);
                let idle = cpu.idle_ns.saturating_sub(self.totals.idle[index]);
                sample.busy[index] = if busy + idle == 0 { 0 } else { (busy * 1000 / (busy + idle)) as u16 };
                self.totals.busy[index] = cpu.busy_ns; self.totals.idle[index] = cpu.idle_ns;
                interrupts += cpu.interrupts; switches += cpu.switches;
            }
        }
        let (mut syscalls, mut tasks, mut runnable) = (0u64, 0u8, 0u8);
        if let Ok(records) = stat::read(STAT_TASKS, 0, self.scratch.as_mut_slice()) {
            for task in records.iter::<StatTask>() {
                syscalls += task.calls; tasks += 1;
                if stat::runnable(task.wait) { runnable += 1; }
            }
        }
        let mut messages = 0u64;
        if let Ok(records) = stat::read(STAT_ENDPOINTS, 0, self.scratch.as_mut_slice()) { messages = records.iter::<StatEndpoint>().map(|e| e.messages).sum(); }
        if let Ok(memory) = stat::one::<StatMemory>(STAT_MEMORY) { sample.used_kib = (memory.used / 1024) as u32; }
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

fn name(bytes: &[u8; NAME_MAX]) -> Text<16> {
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(NAME_MAX);
    Text::new(core::str::from_utf8(&bytes[..len]).unwrap_or("?")).unwrap_or_default()
}

fn serve(monitor: &mut Monitor, request: Request, call: Call) -> mind::Result<()> {
    macro_rules! records { ($class:expr, $arg:expr) => {{
        let scratch = monitor.scratch.as_mut_slice();
        match stat::read($class, $arg, scratch) { Ok(records) => Ok(records), Err(mind::Error::NotFound) => Err(Error::NotFound), Err(_) => Err(Error::Unavailable) }
    }}; }
    match request {
        Request::Tasks => {
            let items: Result<Vec<sysinfo::Task>, Error> = records!(STAT_TASKS, 0).map(|r| r.iter::<StatTask>().take(40).map(|t| sysinfo::Task {
                pid: t.pid, parent: t.parent, run_ns: t.run_ns, runs: t.runs, ticks: t.ticks, calls: t.calls, sends: t.sends, receives: t.receives, started_ns: t.started_ns,
                image: t.image_bytes, stack: t.stack_bytes, screen: t.screen_bytes, heap: t.heap_bytes, shared: t.shared_bytes, retained: t.retained_bytes,
                budget_ns: t.budget_ns, period_ns: t.period_ns, wait_on: t.wait_on, heap_blocks: t.heap_blocks, caps: t.caps,
                quota_tasks: t.quota_tasks, used_tasks: t.used_tasks, quota_endpoints: t.quota_endpoints, used_endpoints: t.used_endpoints,
                name: name(&t.name), wait: t.wait, cpu: t.cpu, band: t.band, throttled: t.throttled != 0, kernel: t.kernel_bytes,
                flags: if t.service != 0 { stat::TASK_SERVICE } else { 0 } | if t.screen != 0 { stat::TASK_SCREEN } else { 0 } | if t.focus != 0 { stat::TASK_FOCUS } else { 0 },
            }).collect());
            sysinfo::reply_tasks(call, items.as_deref().map_err(|e| *e))
        }
        Request::Cpus => {
            let items: Result<Vec<sysinfo::Cpu>, Error> = records!(STAT_CPUS, 0).map(|r| r.iter::<StatCpu>().take(8).map(|c| sysinfo::Cpu {
                busy_ns: c.busy_ns, idle_ns: c.idle_ns, ticks: c.ticks, switches: c.switches, interrupts: c.interrupts, current: c.current_pid, apic: c.apic_id, online: c.online != 0 }).collect());
            sysinfo::reply_cpus(call, items.as_deref().map_err(|e| *e))
        }
        Request::Memory => {
            // Argument 1: the kernel also finds the largest free block (only for a client's request, not for samples).
            let memory = records!(STAT_MEMORY, 1).and_then(|r| r.iter::<StatMemory>().next().ok_or(Error::Unavailable)).map(|m| sysinfo::Memory {
                arena: m.arena, used: m.used, free: m.free, images: m.images, stacks: m.stacks, task_pages: m.task_pages, screens: m.screens, heaps: m.heaps,
                objects: m.objects, objects_limit: m.objects_limit, dma: m.dma, dma_limit: m.dma_limit, tasks: m.tasks as u32, endpoints: m.endpoints as u32,
                largest_free: m.largest_free, page_tables: m.page_tables, shared: m.shared, tasks_limit: m.tasks_limit, endpoints_limit: m.endpoints_limit });
            sysinfo::reply_memory(call, memory.as_ref().map_err(|e| *e))
        }
        Request::Physmap => {
            let items: Result<Vec<sysinfo::Range>, Error> = records!(STAT_PHYSMAP, 0).map(|r| r.iter::<StatPhys>().take(256).map(|p| sysinfo::Range { start: p.start, pages: p.pages, kind: p.kind, index: p.index }).collect());
            sysinfo::reply_physmap(call, items.as_deref().map_err(|e| *e))
        }
        Request::Vmap { pid } => {
            let items: Result<Vec<sysinfo::Region>, Error> = records!(STAT_VMAP, pid).map(|r| r.iter::<StatRegion>().take(80).map(|v| sysinfo::Region { start: v.start, size: v.size, kind: v.kind, flags: v.flags }).collect());
            sysinfo::reply_vmap(call, items.as_deref().map_err(|e| *e))
        }
        Request::Caps { pid } => {
            let items: Result<Vec<sysinfo::Capability>, Error> = records!(STAT_CAPS, pid).map(|r| r.iter::<StatCap>().take(64).map(|c| sysinfo::Capability {
                node: c.node, parent: c.parent, size: c.size, slot: c.slot, generation: c.generation, kind: c.kind, rights: c.rights, badge: c.badge, endpoint: c.endpoint }).collect());
            sysinfo::reply_caps(call, items.as_deref().map_err(|e| *e))
        }
        Request::Endpoints => {
            let items: Result<Vec<sysinfo::EndpointInfo>, Error> = records!(STAT_ENDPOINTS, 0).map(|r| r.iter::<StatEndpoint>().take(128).map(|e| sysinfo::EndpointInfo {
                messages: e.messages, busy: e.busy, timeouts: e.timeouts, creator: e.creator, index: e.index, receivers: e.receivers, senders: e.waiting_senders, receiving: e.waiting_receivers,
                server: e.server, holders: e.holders, irq: e.irq }).collect());
            sysinfo::reply_endpoints(call, items.as_deref().map_err(|e| *e))
        }
        Request::Irqs => {
            let items: Result<Vec<sysinfo::Irq>, Error> = records!(STAT_IRQS, 0).map(|r| r.iter::<StatIrq>().take(16).map(|i| sysinfo::Irq { count: i.count, holder: i.holder, line: i.line, endpoint: i.endpoint, masked: i.masked != 0, holders: i.holders }).collect());
            sysinfo::reply_irqs(call, items.as_deref().map_err(|e| *e))
        }
        Request::Devices => {
            let items: Result<Vec<sysinfo::Device>, Error> = records!(STAT_DEVICES, 0).map(|r| r.iter::<StatDevice>().take(64).map(|d| { let b = d.bar_sizes; sysinfo::Device {
                bar0: b[0], bar1: b[1], bar2: b[2], bar3: b[3], bar4: b[4], bar5: b[5], holder: d.holder, class: d.class, irq: d.irq, location: d.location, io_bars: d.io_bars } }).collect());
            sysinfo::reply_devices(call, items.as_deref().map_err(|e| *e))
        }
        Request::History { slow, count, start } => {
            let (count, start) = (count as usize, start as usize);
            let items: Vec<sysinfo::Sample> = if slow { monitor.slow.last(count).skip(start).take(150).map(wire_sample).collect() } else { monitor.fast.last(count).skip(start).take(150).map(wire_sample).collect() };
            sysinfo::reply_history(call, Ok(&items))
        }
        Request::Load => {
            let [one, five, fifteen] = monitor.load.map(|l| (l * 100 / 2048) as u32);
            sysinfo::reply_load(call, Ok(&sysinfo::Load { one, five, fifteen, uptime_ms: mind::time::uptime_ms() as u64, fast_ms: FAST_MS as u32, slow_ms: 1000, fast_count: monitor.fast.count as u32, slow_count: monitor.slow.count as u32 }))
        }
    }
}

// A refused request (over the rate) gets the `busy` error of its function.
fn refuse(request: Request, call: Call) -> mind::Result<()> {
    let busy = Error::Busy;
    match request {
        Request::Tasks => sysinfo::reply_tasks(call, Err(busy)), Request::Cpus => sysinfo::reply_cpus(call, Err(busy)),
        Request::Memory => sysinfo::reply_memory(call, Err(busy)), Request::Physmap => sysinfo::reply_physmap(call, Err(busy)),
        Request::Vmap { .. } => sysinfo::reply_vmap(call, Err(busy)), Request::Caps { .. } => sysinfo::reply_caps(call, Err(busy)),
        Request::Endpoints => sysinfo::reply_endpoints(call, Err(busy)), Request::Irqs => sysinfo::reply_irqs(call, Err(busy)),
        Request::Devices => sysinfo::reply_devices(call, Err(busy)), Request::History { .. } => sysinfo::reply_history(call, Err(busy)),
        Request::Load => sysinfo::reply_load(call, Err(busy)),
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
        let _ = match sysinfo::decode(&request, RECEIVED) {
            Ok((request_data, call)) if !monitor.limiter.admit(request.sender, now) => refuse(request_data, call),
            Ok((request_data, call)) => serve(&mut monitor, request_data, call),
            Err(reason) => if request.is_call { wire::reject(reason) } else { Ok(()) },
        };
    }
}
