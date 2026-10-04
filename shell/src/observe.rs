// Observation commands of the shell (STAT through the process-control privilege): memory, address spaces, physical
// map, interrupts, devices, endpoints, capabilities and task details. Console counterparts of `memmap` and `top`.
use core::fmt::Write;
use mind::abi::*;
use mind::stat::{self, Records};

fn name(bytes: &[u8]) -> &str { core::str::from_utf8(bytes).unwrap_or("?").trim_end_matches('\0') }

pub fn free(out: &mut impl Write) {
    let Ok(m) = stat::one::<StatMemory>(STAT_MEMORY) else { let _ = writeln!(out, "ERROR: STAT NOT AVAILABLE"); return };
    let _ = writeln!(out, "MEMORY: ARENA={} USED={} FREE={}", m.arena, m.used, m.free);
    let _ = writeln!(out, "  TASKS={} IMAGES={} STACKS={} SCREENS={} HEAPS={} TASK_PAGES={}", m.tasks, m.images, m.stacks, m.screens, m.heaps, m.task_pages);
    let _ = writeln!(out, "  OBJECTS={}/{} DMA={}/{} ENDPOINTS={}", m.objects, m.objects_limit, m.dma, m.dma_limit, m.endpoints);
}

pub fn cpus(out: &mut impl Write) {
    let mut buffer = [0u8; 1024];
    let Ok(records) = stat::read(STAT_CPUS, 0, &mut buffer) else { return };
    for (index, c) in records.iter::<StatCpu>().enumerate() {
        let _ = writeln!(out, "CPU={} APIC={} ONLINE={} TICKS={} BUSY_MS={} IDLE_MS={} SWITCHES={} IRQS={} CURRENT={}", index, c.apic_id, c.online != 0, c.ticks, c.busy_ns / 1_000_000, c.idle_ns / 1_000_000, c.switches, c.interrupts, c.current_pid);
    }
}

fn task(pid: u64, buffer: &mut [u8]) -> Option<StatTask> {
    stat::read(STAT_TASKS, 0, buffer).ok()?.iter::<StatTask>().find(|t| t.pid == pid)
}

pub fn task_details(out: &mut impl Write, pid: u64) {
    let mut buffer = [0u8; 8192];
    let Some(t) = task(pid, &mut buffer) else { let _ = writeln!(out, "ERROR: NO SUCH PID"); return };
    let (list, count) = crate::tasks();
    let focus = list[..count].iter().any(|i| i.pid == pid && i.focus != 0);
    let now = mind::time::monotonic_ns();
    let _ = writeln!(out, "TASK PID={} NAME={} STATE={} WAIT={} CPU={} PARENT={}{}{}", t.pid, name(&t.name), stat::state_name(t.wait), t.wait_on, t.cpu, t.parent,
                     if t.service != 0 { " SERVICE" } else { "" }, if focus { " FOCUS" } else { "" });
    let _ = writeln!(out, "  RUN_MS={} AGE_MS={} RUNS={} TICKS={} SYSCALLS={} SENT={} RECEIVED={}", t.run_ns / 1_000_000, now.saturating_sub(t.started_ns) / 1_000_000, t.runs, t.ticks, t.calls, t.sends, t.receives);
    let _ = writeln!(out, "  IMAGE={} STACK={} SCREEN={} HEAP={} BLOCKS={}/{} MAPPED={} RETAINED={} CAPS={}/{}", t.image_bytes, t.stack_bytes, t.screen_bytes, t.heap_bytes, t.heap_blocks, HEAP_MAX_BLOCKS, t.shared_bytes, t.retained_bytes, t.caps, CAP_SLOTS - 1);
    let _ = writeln!(out, "  QUOTA TASKS={}/{} ENDPOINTS={}/{}", t.used_tasks, t.quota_tasks, t.used_endpoints, t.quota_endpoints);
    let _ = writeln!(out, "  BAND={} BUDGET_US={} PERIOD_US={}{}", t.band, t.budget_ns / 1000, t.period_ns / 1000, if t.throttled != 0 { " THROTTLED" } else { "" });
}

pub fn pmap(out: &mut impl Write, pid: u64) {
    let mut buffer = [0u8; 4096];
    let records: Records = match stat::read(STAT_VMAP, pid, &mut buffer) { Ok(r) => r, Err(_) => { let _ = writeln!(out, "ERROR: NO SUCH PID"); return } };
    let _ = writeln!(out, "PMAP PID={} REGIONS={}", pid, records.total());
    let (mut mapped, mut heap) = (0u64, 0u64);
    for r in records.iter::<StatRegion>() {
        let rights = [(REGION_READ, 'r'), (REGION_WRITE, 'w'), (REGION_EXECUTE, 'x')].map(|(bit, ch)| if r.flags & bit != 0 { ch } else { '-' });
        let _ = writeln!(out, "{:#018x} {:>9} {}{}{} {}", r.start, r.size, rights[0], rights[1], rights[2], stat::vm_name(r.kind));
        mapped += r.size;
        if r.kind == REGION_HEAP { heap += r.size; }
    }
    let _ = writeln!(out, "TOTAL MAPPED={} HEAP={}/{}", mapped, heap, HEAP_MAX_BYTES);
}

pub fn physmap(out: &mut impl Write) {
    let mut buffer = [0u8; 8192];
    let Ok(records) = stat::read(STAT_PHYSMAP, 0, &mut buffer) else { return };
    let mut free = 0u64;
    for r in records.iter::<StatPhys>() {
        let bytes = r.pages * 4096;
        let _ = writeln!(out, "{:#014x}-{:#014x} {:>10}K {}", r.start, r.start + bytes.max(1) - 1, bytes / 1024, stat::phys_name(r.kind));
        if r.kind == 7 { free += bytes; }
    }
    let _ = writeln!(out, "RANGES={} FREE_RAM={}K", records.total(), free / 1024);
}

pub fn irqs(out: &mut impl Write) {
    let mut buffer = [0u8; 1024];
    let Ok(records) = stat::read(STAT_IRQS, 0, &mut buffer) else { return };
    for i in records.iter::<StatIrq>() {
        let _ = writeln!(out, "IRQ={} COUNT={} HOLDER={} ENDPOINT={} MASKED={}", i.line, i.count, i.holder, i.endpoint, i.masked != 0);
    }
}

pub fn devices(out: &mut impl Write) {
    let mut buffer = [0u8; 4096];
    let Ok(records) = stat::read(STAT_DEVICES, 0, &mut buffer) else { return };
    for (index, d) in records.iter::<StatDevice>().enumerate() {
        let _ = write!(out, "{:02} {:06X} {} IRQ={} HOLDER={} BARS=", index, d.class, stat::class_name(d.class), d.irq, d.holder);
        for (i, &bytes) in d.bar_sizes.iter().enumerate().filter(|(_, b)| **b != 0) { let _ = write!(out, "{}:{} ", i, bytes); }
        let _ = writeln!(out);
    }
}

pub fn endpoints(out: &mut impl Write) {
    let mut buffer = [0u8; 8192];
    let Ok(records) = stat::read(STAT_ENDPOINTS, 0, &mut buffer) else { return };
    for e in records.iter::<StatEndpoint>() {
        let _ = writeln!(out, "EP={} CREATOR={} RECEIVERS={} WAITING={} RECEIVING={} MESSAGES={} BUSY={} TIMEOUTS={}", e.index, e.creator, e.receivers, e.waiting_senders, e.waiting_receivers, e.messages, e.busy, e.timeouts);
    }
}

pub fn caps(out: &mut impl Write, pid: u64) {
    let mut buffer = [0u8; 4096];
    let records: Records = match stat::read(STAT_CAPS, pid, &mut buffer) { Ok(r) => r, Err(_) => { let _ = writeln!(out, "ERROR: NO SUCH PID"); return } };
    for c in records.iter::<StatCap>() {
        let rights = [(CAP_READ, 'r'), (CAP_WRITE, 'w'), (CAP_GRANT, 'g'), (CAP_KEEP, 'k')].map(|(bit, ch)| if c.rights as u8 & bit != 0 { ch } else { '-' });
        let _ = write!(out, "SLOT={} GEN={} {} NODE={} PARENT={}", c.slot, c.generation, stat::cap_name(c.kind), c.node, c.parent);
        match c.kind as usize {
            CAP_KIND_ENDPOINT => {
                let _ = write!(out, " RIGHTS={}{}{}{}", rights[0], rights[1], rights[2], rights[3]);
                if c.badge != 0 { let _ = write!(out, " BADGE={:#x}", c.badge); }
            }
            CAP_KIND_MEMORY => { let _ = write!(out, " BYTES={} RIGHTS={}{}{}", c.size, rights[0], rights[1], rights[2]); }
            CAP_KIND_DMA | CAP_KIND_MMIO => { let _ = write!(out, " BYTES={}", c.size); }
            // Port ranges and IRQ lines carry their base in `rights`.
            CAP_KIND_PORTS => { let _ = write!(out, " PORTS={:#x}+{}", c.rights, c.size); }
            CAP_KIND_IRQ => { let _ = write!(out, " LINE={}", c.rights); }
            _ => {}
        }
        let _ = writeln!(out);
    }
}
