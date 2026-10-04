// Observation commands of the shell (STAT through the process-control privilege): memory, address spaces, physical
// map, interrupts, devices, endpoints, capabilities and task details. Console counterparts of `memmap` and `top`.
use core::fmt::Write;
use mind::abi::*;
use mind::stat::{self, Records};

fn name(bytes: &[u8]) -> &str { core::str::from_utf8(bytes).unwrap_or("?").trim_end_matches('\0') }

pub fn free(out: &mut impl Write) {
    let mut buffer = [0u8; 512];
    let Some(m) = stat::read(STAT_MEMORY, 1, &mut buffer).ok().and_then(|r| r.iter::<MemoryStat>().next()) else { let _ = writeln!(out, "ERROR: STAT NOT AVAILABLE"); return };
    let _ = writeln!(out, "MEMORY: ARENA={} USED={} FREE={} LARGEST={}", m.arena_bytes, m.arena_used, m.arena_free, m.largest_free);
    let _ = writeln!(out, "  TASKS={}/{} IMAGES={} STACKS={} SCREENS={} HEAPS={} KERNEL={} PAGE_TABLES={}", m.tasks, m.tasks_limit, m.task_images, m.task_stacks, m.task_screens, m.task_heaps, m.task_kernel, m.page_tables);
    let _ = writeln!(out, "  OBJECTS={}/{} DMA={}/{} MAPPED={} OTHER={} ENDPOINTS={}/{}", m.objects, m.objects_limit, m.dma, m.dma_limit, m.shared_mapped, m.kernel_other, m.endpoints, m.endpoints_limit);
}

pub fn cpus(out: &mut impl Write) {
    let mut buffer = [0u8; 1024];
    let Ok(records) = stat::read(STAT_CPUS, 0, &mut buffer) else { return };
    for (index, c) in records.iter::<CpuStat>().enumerate() {
        let _ = writeln!(out, "CPU={} APIC={} ONLINE={} TICKS={} BUSY_MS={} IDLE_MS={} SWITCHES={} IRQS={} CURRENT={}", index, c.apic, c.online != 0, c.ticks, c.busy_ns / 1_000_000, c.idle_ns / 1_000_000, c.switches, c.interrupts, c.current);
    }
}

fn task(pid: u64, buffer: &mut [u8]) -> Option<TaskStat> {
    stat::read(STAT_TASKS, 0, buffer).ok()?.iter::<TaskStat>().find(|t| t.pid == pid)
}

pub fn task_details(out: &mut impl Write, pid: u64) {
    let mut buffer = [0u8; 4096];
    let Some(t) = task(pid, &mut buffer) else { let _ = writeln!(out, "ERROR: NO SUCH PID"); return };
    let now = mind::time::monotonic_ns();
    let _ = writeln!(out, "TASK PID={} NAME={} STATE={} WAIT={} CPU={} PARENT={}{}{}", t.pid, name(&t.name), stat::state_name(t.state), t.wait, t.cpu, t.parent,
                     if t.flags & TASK_FLAG_SERVICE != 0 { " SERVICE" } else { "" }, if t.flags & TASK_FLAG_FOCUS != 0 { " FOCUS" } else { "" });
    let _ = writeln!(out, "  RUN_MS={} AGE_MS={} RUNS={} TICKS={} SYSCALLS={} SENT={} RECEIVED={}", t.run_ns / 1_000_000, now.saturating_sub(t.started_ns) / 1_000_000, t.runs, t.ticks, t.calls, t.sent, t.received);
    let _ = writeln!(out, "  IMAGE={} STACK={} SCREEN={} HEAP={} BLOCKS={}/{} MAPPED={} KERNEL={} CAPS={}/{}", t.image_bytes, t.stack_bytes, t.screen_bytes, t.heap_bytes, t.heap_blocks, HEAP_MAX_BLOCKS, t.shared_bytes, t.kernel_bytes, t.caps, CAP_SLOTS - 1);
    let _ = writeln!(out, "  QUOTA TASKS={}/{} ENDPOINTS={}/{}", t.used_tasks, t.quota_tasks, t.used_endpoints, t.quota_endpoints);
}

pub fn pmap(out: &mut impl Write, pid: u64) {
    let mut buffer = [0u8; 4096];
    let records: Records = match stat::read(STAT_VMAP, pid, &mut buffer) { Ok(r) => r, Err(_) => { let _ = writeln!(out, "ERROR: NO SUCH PID"); return } };
    let _ = writeln!(out, "PMAP PID={} REGIONS={}", pid, records.total());
    let (mut mapped, mut heap) = (0u64, 0u64);
    for r in records.iter::<VmRegion>() {
        let rights = [(VM_READ, 'r'), (VM_WRITE, 'w'), (VM_EXEC, 'x')].map(|(bit, ch)| if r.flags & bit != 0 { ch } else { '-' });
        let _ = writeln!(out, "{:#018x} {:>9} {}{}{} {}", r.start, r.bytes, rights[0], rights[1], rights[2], stat::vm_name(r.kind));
        if r.kind != VM_GUARD { mapped += r.bytes; }
        if r.kind == VM_HEAP { heap += r.bytes; }
    }
    let _ = writeln!(out, "TOTAL MAPPED={} HEAP={}/{}", mapped, heap, HEAP_MAX_BYTES);
}

pub fn physmap(out: &mut impl Write) {
    let mut buffer = [0u8; 8192];
    let Ok(records) = stat::read(STAT_PHYSMAP, 0, &mut buffer) else { return };
    let mut free = 0u64;
    for r in records.iter::<PhysRange>() {
        let _ = writeln!(out, "{:#014x}-{:#014x} {:>10}K {}", r.start, r.start + r.bytes.max(1) - 1, r.bytes / 1024, stat::phys_name(r.kind));
        if r.kind == 7 { free += r.bytes; }
    }
    let _ = writeln!(out, "RANGES={} FREE_RAM={}K", records.total(), free / 1024);
}

pub fn irqs(out: &mut impl Write) {
    let mut buffer = [0u8; 1024];
    let Ok(records) = stat::read(STAT_IRQS, 0, &mut buffer) else { return };
    for i in records.iter::<IrqStat>() {
        let _ = writeln!(out, "IRQ={} COUNT={} HOLDER={} ENDPOINT={} MASKED={}", i.line, i.count, i.holder, i.endpoint, i.masked != 0);
    }
}

pub fn devices(out: &mut impl Write) {
    let mut buffer = [0u8; 4096];
    let Ok(records) = stat::read(STAT_DEVICES, 0, &mut buffer) else { return };
    for d in records.iter::<DeviceStat>() {
        let _ = write!(out, "{:02x}:{:02x}.{} {:06X} {} IRQ={} HOLDER={} BARS=", d.location >> 16, (d.location >> 8) & 0xFF, d.location & 7, d.class, stat::class_name(d.class), d.irq, d.holder);
        for (i, &bytes) in d.bar_bytes.iter().enumerate().filter(|(_, b)| **b != 0) { let _ = write!(out, "{}:{}{} ", i, bytes, if d.io_bars & (1 << i) != 0 { "io" } else { "" }); }
        let _ = writeln!(out);
    }
}

pub fn endpoints(out: &mut impl Write) {
    let mut buffer = [0u8; 4096];
    let Ok(records) = stat::read(STAT_ENDPOINTS, 0, &mut buffer) else { return };
    for e in records.iter::<EndpointStat>() {
        let _ = writeln!(out, "EP={} CREATOR={} SERVER={} RECEIVERS={} HOLDERS={} WAITING={} RECEIVING={} MESSAGES={} BUSY={} TIMEOUTS={}{}", e.index, e.creator, e.server, e.receivers, e.holders, e.waiting, e.receiving, e.messages, e.busy, e.timeouts,
                         if e.irq != 0 { " IRQ" } else { "" });
    }
}

pub fn caps(out: &mut impl Write, pid: u64) {
    let mut buffer = [0u8; 4096];
    let records: Records = match stat::read(STAT_CAPS, pid, &mut buffer) { Ok(r) => r, Err(_) => { let _ = writeln!(out, "ERROR: NO SUCH PID"); return } };
    for c in records.iter::<CapStat>() {
        let rights = [(CAP_READ, 'r'), (CAP_WRITE, 'w'), (CAP_GRANT, 'g'), (CAP_KEEP, 'k')].map(|(bit, ch)| if c.rights as u8 & bit != 0 { ch } else { '-' });
        let _ = write!(out, "SLOT={} GEN={} {} NODE={} PARENT={}", c.slot, c.generation, stat::cap_name(c.kind), c.node, c.parent);
        match c.kind as usize {
            CAP_KIND_ENDPOINT => { let _ = write!(out, " EP={} RIGHTS={}{}{}{}", c.endpoint, rights[0], rights[1], rights[2], rights[3]); }
            CAP_KIND_MEMORY => { let _ = write!(out, " BYTES={} RIGHTS={}{}{}", c.size, rights[0], rights[1], rights[2]); }
            CAP_KIND_DMA | CAP_KIND_MMIO => { let _ = write!(out, " BYTES={}", c.size); }
            CAP_KIND_PORTS => { let _ = write!(out, " PORTS={:#x}+{}", c.base, c.size); }
            CAP_KIND_IRQ => { let _ = write!(out, " LINE={}", c.base); }
            _ => {}
        }
        let _ = writeln!(out);
    }
}

