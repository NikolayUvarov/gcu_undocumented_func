use crate::abi::*;
use crate::input::{Events, Queue};
use crate::memory::{Frames, Region};
use crate::task_state::{self, State};
use crate::{context, cpu, elf, interrupts, paging, pci, platform, port, serial_write_byte};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

mod stat;

const STACK_SIZE: usize = 64 * 1024;
const ENDPOINTS: usize = 128; // the endpoint table's first size; it grows on demand (issue 171)
const FIRST_ENDPOINT: usize = 1; // endpoint 0 is never handed out
const ENDPOINT_ALL: u8 = CAP_READ | CAP_WRITE | CAP_GRANT | CAP_KEEP;
const MEMORY_ALL: u8 = CAP_READ | CAP_WRITE | CAP_GRANT;
const GRANT_BYTES: usize = core::mem::size_of::<Grant>();
const RECEIVE_MASK: usize = (1 << IPC_TIMEOUT_SHIFT) - 1; // arg2 of IPC: the receive slot below the timeout (issue 172)
// A console read within this long pushes back on its writer (LOG takes only what fits); an application counts as read from its start.
const CONSOLE_READER_NS: u64 = 1_000_000_000;
const GHOSTS_MAX: usize = 256; // removed nodes kept for revocation; a drop beyond it leaves the subtree unrevocable
// Interrupt lines: 1..15 on the PIC, then MSI-X vectors 0x40..0x4F as lines 16..31 (allocated by PLATFORM_DEVICE_MSIX).
// Every task's log also goes to the kernel's console while no driver holds it (aarch64: until the shell takes the PL011).
static MIRROR_LOGS: AtomicBool = AtomicBool::new(cfg!(target_arch = "aarch64"));
use context::{Event, MSI_FIRST}; const MSI_VECTORS: usize = 16; const LINES: usize = MSI_FIRST + MSI_VECTORS;
const DMA_LIMIT: usize = 8 * 1024 * 1024; // all DMA regions handed out through PLATFORM_DMA

// Identity of a capability in the derivation tree: a copy or mint is a child of its source; a move keeps the node.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
struct Node { id: u64, parent: u64 } // parent 0: root
// Capability waiting in a blocked send: copy (new child node) or move (node of the sender's slot `from`).
#[derive(Clone, Copy)]
struct Pending { cap: Capability, node: Node, moved_from: Option<usize> }
// Counters for STAT (MC-10.2: observation, never authority).
struct Accounting {
    last_switch: [u64; cpu::MAX], busy_ns: [u64; cpu::MAX], idle_ns: [u64; cpu::MAX], interrupts: [u64; cpu::MAX], switches: [u64; cpu::MAX],
    irqs: [u64; LINES], endpoint: Vec<EndpointCounters>,
}
#[derive(Clone, Copy, Default)]
struct EndpointCounters { messages: u64, busy: u64, timeouts: u64 }
impl Accounting {
    fn new() -> Self { Self { last_switch: [0; cpu::MAX], busy_ns: [0; cpu::MAX], idle_ns: [0; cpu::MAX], interrupts: [0; cpu::MAX], switches: [0; cpu::MAX], irqs: [0; LINES], endpoint: alloc::vec![EndpointCounters::default(); ENDPOINTS] } }
}
// Memory kept alive by references after its owner let go; charged to the owner's heap quota while that owner lives.
struct Orphan { region: Region, owner: Option<(usize, u64)> }

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Capability { Endpoint(usize, u8, u16), Memory(usize, usize, u8), Dma(usize, usize), Mmio(usize, usize), IoPorts(u16, u16), Interrupt(u8), Input, Display, Spawn, Reply(usize, u64, u64), Platform, Control, Restart, Observe, Escrow(u8), Firmware }

// Task name (for ps and spawn requests); application images are not indexed by a kernel table.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Name { bytes: [u8; NAME_MAX], len: u8 }
impl Name {
    pub fn new(text: &[u8]) -> Self { let len = text.len().min(NAME_MAX); let mut bytes = [0; NAME_MAX]; bytes[..len].copy_from_slice(&text[..len]); Self { bytes, len: len as u8 } }
}

// Where the ELF comes from: a boot image from the UEFI bootloader or a buffer passed by the spawner.
enum Source<'a> { Boot(usize), Image(&'a [u8]) }

impl Capability {
    // A privilege by its kind (the escrowable ones).
    fn privilege(kind: usize) -> Option<Self> {
        match kind { CAP_KIND_INPUT => Some(Self::Input), CAP_KIND_DISPLAY => Some(Self::Display), CAP_KIND_SPAWN => Some(Self::Spawn), CAP_KIND_CONTROL => Some(Self::Control), CAP_KIND_OBSERVE => Some(Self::Observe), CAP_KIND_FIRMWARE => Some(Self::Firmware), _ => None }
    }
    fn overlaps(self, physical: usize, size: usize) -> bool {
        match self { Self::Memory(p, s, _) | Self::Dma(p, s) => p < physical + size && physical < p + s, _ => false }
    }
}

// A task and its kernel structures live in the frame pool, charged to its payers (171-KRN-0032).
type TaskBox = alloc::boxed::Box<Task, Frames>;

struct Task {
    pid: u64, name: Name, service: bool, state: State, sp: usize, cpu: usize,
    space: paging::Space, heap: crate::user_heap::Heap, context: Region, _exit: Region,
    runs: u64, ticks: u64, calls: u64, run_ns: u64, sends: u64, receives: u64, started_ns: u64, _image: Region, _stack: Region, screen: Option<Region>, abi: Region,
    input: Events<INPUT_QUEUE>, pointer: bool, log: Queue<4096>, console: Queue<4096>, console_read_ns: u64, dirty: bool,
    // Capability table, grown on demand up to CAP_SLOTS_MAX (issue 171); the generation of each kernel-allocated slot.
    cspace: Vec<Option<Capability>, Frames>, generations: Vec<u32, Frames>,
    nodes: Vec<Node, Frames>,
    kernel: usize, // bytes of its kernel structures charged to its payers (171-KRN-0032)
    pending_cap: Option<Pending>, pending_call: bool, pending_badge: u16, send_seq: u64, // send waiting for a receiver
    reply_to: Option<(usize, u64, u64)>, // slot, PID and call number of the client awaiting a reply
    call_seq: u64, // number of this task's current call; a reply must name it
    deadline: u64, // uptime ms at which a blocked IPC fails with ERR_TIMEOUT (0: none)
    watch: Option<usize>, // endpoint of the lifecycle owner that gets this task's exit notice
    band: u8, budget_ns: u64, period_ns: u64, period_start: u64, consumed: u64, // scheduling context (C7)
    parent: Option<(usize, u64)>, quota_tasks: usize, quota_endpoints: usize, // accounting owner and delegated quotas
    // Private memory (heap blocks and memory objects): the task's quota, what it and its live descendants hold, and
    // the task that pays above it (its spawner, or the spawner's payer once the spawner ended; MC-3.13, issue 150).
    memory_quota: usize, memory_tree: usize, payer: Option<(usize, u64)>,
    exit_reason: usize, // why it ended (EXIT_*), for a watch that comes after the exit
    handed_by: Option<(usize, u64)>, // the task in front that started it with SPAWN_FOREGROUND: the focus returns there (issue 160)
}
// What a task's kernel structures take of the frame pool: the task itself, its context, exit and info pages, its page
// tables and its capability table (171-KRN-0032).
fn kernel_bytes(task: &Task) -> usize {
    core::mem::size_of::<Task>() + task.context.len() + task._exit.len() + task.abi.len() + task.space.table_count() * paging::PAGE
        + task.cspace.capacity() * core::mem::size_of::<Option<Capability>>() + task.generations.capacity() * core::mem::size_of::<u32>()
        + task.nodes.capacity() * core::mem::size_of::<Node>()
}
// A copy of `list` with room for exactly its elements, if the kernel heap has it.
fn exact_copy(list: &[usize]) -> Option<Vec<usize>> { let mut copy = Vec::new(); copy.try_reserve_exact(list.len()).ok()?; copy.extend_from_slice(list); Some(copy) }
// A set of CPUs, one bit each.
#[derive(Clone, Copy)]
struct Cpus([u64; cpu::MAX.div_ceil(64)]);
impl Cpus {
    const NONE: Self = Self([0; cpu::MAX.div_ceil(64)]);
    fn mark(&mut self, cpu: usize) { self.0[cpu / 64] |= 1 << (cpu % 64); }
    fn has(&self, cpu: usize) -> bool { self.0[cpu / 64] & 1 << (cpu % 64) != 0 }
    fn is_empty(&self) -> bool { self.0.iter().all(|&word| word == 0) }
}
// A driver bound to an interrupt line (IRQ_BIND): its endpoint, the binder, an interrupt it has not received yet, and
// one it has not acknowledged. A shared line stays masked until every binder acknowledged (issue 159).
#[derive(Clone, Copy, PartialEq)]
struct IrqBinding { ep: usize, slot: usize, pid: u64, pending: bool, unacked: bool }
// An INPUT_LISTEN registration: the key and modifiers, the listening task, and whether a taken press awaits release.
#[derive(Clone, Copy)]
struct Listener { key: u16, mods: u8, slot: usize, pid: u64, down: bool }
const LISTEN_MODS: u8 = MOD_SHIFT | MOD_CTRL | MOD_ALT;
// The task table (issue 171): chunks of 32 slots added as needed and dropped when empty at the end. A task never moves:
// the system call path holds raw pointers to tasks while the table may grow.
const CHUNK: usize = 32;
struct Table<T> { chunks: Vec<alloc::boxed::Box<[Option<T>]>> }
impl<T> Table<T> {
    fn new() -> Self { let mut table = Self { chunks: Vec::new() }; assert!(table.grow(), "task table"); table }
    fn grow(&mut self) -> bool {
        let mut chunk = Vec::new();
        if self.chunks.try_reserve(1).is_err() || chunk.try_reserve_exact(CHUNK).is_err() { return false; }
        for _ in 0..CHUNK { chunk.push(None); }
        self.chunks.push(chunk.into_boxed_slice()); true
    }
    // Drops empty chunks at the end, keeping the first.
    fn shrink(&mut self) { while self.chunks.len() > 1 && self.chunks.last().unwrap().iter().all(Option::is_none) { self.chunks.pop(); } }
    fn len(&self) -> usize { self.chunks.len() * CHUNK }
    fn iter(&self) -> impl Iterator<Item = &Option<T>> { self.chunks.iter().flat_map(|c| c.iter()) }
    fn iter_mut(&mut self) -> impl Iterator<Item = &mut Option<T>> { self.chunks.iter_mut().flat_map(|c| c.iter_mut()) }
    fn ptr(&mut self, index: usize) -> *mut Option<T> { &mut self.chunks[index / CHUNK][index % CHUNK] }
}
impl<T> core::ops::Index<usize> for Table<T> { type Output = Option<T>; fn index(&self, index: usize) -> &Option<T> { &self.chunks[index / CHUNK][index % CHUNK] } }
impl<T> core::ops::IndexMut<usize> for Table<T> { fn index_mut(&mut self, index: usize) -> &mut Option<T> { &mut self.chunks[index / CHUNK][index % CHUNK] } }

struct Scheduler {
    boot: BootInfo, tasks: Table<TaskBox>, current: [usize; cpu::MAX], idle_sp: [usize; cpu::MAX],
    faults: [Option<FaultInfo>; 16], fault_cursor: usize, next_pid: u64,
    foreground: usize, // focused task: its screen is shown and it receives input
    focus_owner: usize, // holder of process control that set the focus; focus returns to it
    listeners: [Option<Listener>; INPUT_LISTENERS], // keys taken out of the focused stream (INPUT_LISTEN)
    notices: [usize; 8], notice_count: usize, // NOTICE values for the focus owner
    exited_console: Option<(u64, Queue<4096>)>, // unread output of the last focused or screenless task that exited
    ended: [(u64, usize); EXIT_STATUSES], ended_next: usize, // (PID, reason) of the last tasks that ended (EXIT_STATUS)
    dirty: bool, endpoints: Vec<bool>, endpoint_owner: Vec<Option<(usize, u64)>>, irq_bind: [[Option<IrqBinding>; IRQ_SHARERS]; LINES], irq_pending: [bool; LINES], msi: [Option<(usize, u16)>; MSI_VECTORS], send_seq: u64, flush: [bool; cpu::MAX], woken: [bool; cpu::MAX], readied: Cpus, // woken: a wake IPI is on its way to that CPU; readied: CPUs a task became ready for since the last wake_idle
    on_cpu: [Vec<usize>; cpu::MAX], // each CPU's task slots in order, sized to them: added at spawn, removed at reap
    accounting: Accounting, cursor: [[usize; 2]; cpu::MAX], // last slot picked per CPU and band: round robin within each band
    orphans: Vec<Orphan>, // memory freed or detached by its owner that is still mapped or held via a capability
    exits: Vec<(usize, u64, usize)>, // undelivered exit notices: endpoint, PID, reason
    exits_lost: usize,
    ghosts: Vec<Node>, // removed capabilities that still have descendants (copies, mints or mappings) to revoke; allocated once
    devices: Vec<pci::Device>, // PCI enumeration: discovery is a kernel mechanism, the choice of drivers is init's
    dma: Vec<Region>, // DMA regions handed out to init; they outlive driver restarts
    reserve: usize, // frame pool bytes application-band tasks may not take (MEMORY_RESERVE, issue 169)
    composited: usize, // screen the compositor already holds a capability for
    next_node: u64, // capability identities are never reused
}

static mut SCHEDULER: Option<Scheduler> = None;
static LOCK: AtomicBool = AtomicBool::new(false);
struct Guard; impl Drop for Guard { fn drop(&mut self) { LOCK.store(false, Ordering::Release); } }
fn locked<T>(f: impl FnOnce() -> T) -> T { interrupts::without(|| { while LOCK.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() { while LOCK.load(Ordering::Relaxed) { core::hint::spin_loop(); } } let _guard = Guard; f() }) }

// The mailbox of the task each CPU runs (0: idle), and the system calls answered there without the lock since the
// CPU last selected: they are added to the task and the CPU at the next select (000-KRN-0011).
static MAILBOX: [AtomicUsize; cpu::MAX] = [const { AtomicUsize::new(0) }; cpu::MAX];
static FAST_CALLS: [AtomicU64; cpu::MAX] = [const { AtomicU64::new(0) }; cpu::MAX];
// System calls that read only clocks, answered before the scheduler lock: with many CPUs they queued for it behind
// work they do not touch (000-KRN-0011). The running task's mailbox stays while it runs: reap skips current tasks.
unsafe fn unlocked_syscall(cpu: usize, sp: usize) -> Option<usize> {
    let mailbox = MAILBOX[cpu].load(Ordering::Acquire) as *mut SyscallMailbox;
    if mailbox.is_null() { return None; }
    let value = clock_syscall(core::ptr::read_volatile(core::ptr::addr_of!((*mailbox).syscall_num)), mailbox)?;
    core::ptr::write_volatile(core::ptr::addr_of_mut!((*mailbox).result), value);
    FAST_CALLS[cpu].fetch_add(1, Ordering::Relaxed);
    Some(sp)
}

// RDTSC, UPTIME and CLOCK: the result, with CLOCK's resolution and TSC rate written into the mailbox; None for others.
unsafe fn clock_syscall(number: usize, mailbox: *mut SyscallMailbox) -> Option<usize> {
    Some(match number {
        SYSCALL_RDTSC => cpu::cycles() as usize,
        SYSCALL_UPTIME => interrupts::milliseconds() as usize,
        SYSCALL_CLOCK => {
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*mailbox).arg2), crate::clock::resolution_ns() as usize);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*mailbox).msg[2]), crate::clock::tsc_hz() as usize);
            crate::clock::now_ns() as usize
        }
        _ => return None,
    })
}

unsafe fn scheduler() -> &'static mut Scheduler { (*core::ptr::addr_of_mut!(SCHEDULER)).as_mut().unwrap() }

fn frame_bytes(info: &BootInfo) -> usize { (info.stride * info.height * 4).div_ceil(4096) * 4096 }

// Kernel spawn errors as ABI codes for the spawner.
fn spawn_error(error: &'static str) -> usize {
    match error { "TASK LIMIT REACHED" | "NO FREE TASK SLOT" | "OVER MEMORY QUOTA" => ERR_LIMIT, e if e.contains("MEMORY") || e.contains("PAGE TABLE") => ERR_NO_MEMORY, _ => ERR_INVALID }
}

// Copies bytes into a task's writable memory; false if any page is not writable.
fn copy_out(space: &paging::Space, address: usize, bytes: &[u8]) -> bool {
    if bytes.is_empty() { return true; }
    let Some(end) = address.checked_add(bytes.len() - 1) else { return false };
    if !(address / 4096..=end / 4096).all(|page| space.writable(page * 4096).is_some()) { return false; }
    for (i, &byte) in bytes.iter().enumerate() { unsafe { core::ptr::write_volatile(space.writable(address + i).unwrap() as *mut u8, byte); } }
    true
}

pub fn init(info: &BootInfo) -> Result<(), &'static str> {
    let devices = unsafe { pci::enumerate() };
    let mut endpoints = alloc::vec![false; ENDPOINTS]; endpoints[..FIRST_ENDPOINT].fill(true);
    unsafe { *core::ptr::addr_of_mut!(SCHEDULER) = Some(Scheduler { boot: *info, tasks: Table::new(), current: [0; cpu::MAX], idle_sp: [0; cpu::MAX], faults: [None; 16], fault_cursor: 0, next_pid: 1, foreground: 0, focus_owner: 0, listeners: [None; INPUT_LISTENERS], notices: [0; 8], notice_count: 0, exited_console: None, ended: [(0, 0); EXIT_STATUSES], ended_next: 0, dirty: true, endpoints, endpoint_owner: alloc::vec![None; ENDPOINTS], irq_bind: [[None; IRQ_SHARERS]; LINES], irq_pending: [false; LINES], msi: [None; MSI_VECTORS], send_seq: 0, flush: [false; cpu::MAX], woken: [false; cpu::MAX], readied: Cpus::NONE, on_cpu: [const { Vec::new() }; cpu::MAX], accounting: Accounting::new(), cursor: [[0; 2]; cpu::MAX], orphans: Vec::new(), exits: Vec::new(), exits_lost: 0, ghosts: Vec::with_capacity(GHOSTS_MAX), devices, dma: Vec::new(), reserve: 0, composited: 0, next_node: 1 }); }
    #[cfg(feature = "bar-move-test")]
    unsafe { (*core::ptr::addr_of_mut!(SCHEDULER)).as_mut().unwrap().bar_move_test(); }
    Ok(())
}

impl Scheduler {
    // Ready tasks of one band on `cpu` that have budget left; the idle slot takes part only in the application band, so
    // a CPU still passes through idle (and CPU 0 through reaping) when only applications run.
    fn ready(&self, slot: usize, cpu: usize, band: u8) -> bool {
        if slot == 0 { return band == BAND_APPLICATION as u8; }
        self.tasks[slot].as_ref().is_some_and(|t| t.cpu == cpu && t.band == band && (t.budget_ns == 0 || t.consumed < t.budget_ns) && t.state == State::Ready)
    }
    fn select(&mut self, sp: usize, cpu: usize) -> usize {
        let current = self.current[cpu]; self.woken[cpu] = false;
        // Every select reloads CR3, which completes a pending TLB flush of this CPU.
        if core::mem::take(&mut self.flush[cpu]) && !self.flush.iter().any(|&f| f) {
            for task in self.tasks.iter_mut().flatten() { if task.state == State::BlockedFlush { task.state = State::Ready; self.readied.mark(task.cpu); } }
            self.wake_idle(cpu, false);
        }
        // Time since the last switch on this CPU goes to the task that ran or to idle.
        let now = crate::clock::now_ns(); let elapsed = now.saturating_sub(core::mem::replace(&mut self.accounting.last_switch[cpu], now));
        if current == 0 { self.accounting.idle_ns[cpu] += elapsed; } else { self.accounting.busy_ns[cpu] += elapsed; }
        let fast = FAST_CALLS[cpu].swap(0, Ordering::Relaxed); self.accounting.interrupts[cpu] += fast;
        if current == 0 { self.idle_sp[cpu] = sp; } else { let task = self.tasks[current].as_mut().unwrap(); task.run_ns += elapsed; task.consumed += elapsed; task.calls += fast; unsafe { context::save(sp, task.context.ptr() as usize); } }
        // Only this CPU's tasks: three passes over every task of every CPU held the lock long with many (171-KRN-0009).
        let mut mine = core::mem::take(&mut self.on_cpu[cpu]);
        mine.retain(|&slot| slot < self.tasks.len() && self.tasks[slot].as_ref().is_some_and(|t| t.cpu == cpu));
        // A budget is refilled at the start of each of its periods.
        for &slot in &mine {
            let task = self.tasks[slot].as_mut().unwrap();
            if task.budget_ns != 0 && now >= task.period_start + task.period_ns { task.period_start = now - (now - task.period_start) % task.period_ns; task.consumed = 0; }
        }
        let [system, application] = self.cursor[cpu];
        let next = match task_state::next_in(&mine, system, |i| self.ready(i, cpu, BAND_SYSTEM as u8)) {
            0 => { let next = task_state::next_in(&mine, application, |i| self.ready(i, cpu, BAND_APPLICATION as u8)); self.cursor[cpu][1] = next; next }
            next => { self.cursor[cpu][0] = next; next }
        };
        self.on_cpu[cpu] = mine;
        self.current[cpu] = next;
        let (pid, name) = if next == 0 { (0, [0u8; 16]) } else { let task = self.tasks[next].as_ref().unwrap(); let mut name = [0u8; 16]; name[..task.name.len as usize].copy_from_slice(&task.name.bytes[..task.name.len as usize]); (task.pid, name) };
        for (word, value) in cpu::RUNNING[cpu].iter().zip([pid, u64::from_le_bytes(name[..8].try_into().unwrap()), u64::from_le_bytes(name[8..].try_into().unwrap())]) { word.store(value, Ordering::Relaxed); }
        if next != current { self.accounting.switches[cpu] += 1; }
        MAILBOX[cpu].store(if next == 0 { 0 } else { self.mailbox(next) as usize }, Ordering::Release);
        if next == 0 { unsafe { paging::activate(paging::kernel_root()); } self.idle_sp[cpu] } else { let task = self.tasks[next].as_mut().unwrap(); task.runs += 1; unsafe { paging::activate(task.space.root()); } task.sp }
    }
    // Other CPUs that sit idle while one of their tasks became ready get a wake IPI, one until that CPU selects again.
    // After an event only the CPUs in `readied` are looked at; the boot CPU's tick looks at every task (`every`), which
    // also finds a task whose budget period ended and bounds a missed mark to one tick. A pass over the tasks after
    // every event held the lock most of the time with 16 CPUs (171-KRN-0009).
    fn wake_idle(&mut self, this: usize, every: bool) {
        let count = cpu::COUNT.load(Ordering::Acquire);
        let mut ready = core::mem::replace(&mut self.readied, Cpus::NONE);
        if every {
            // A task out of budget counts once its period ends: select refills it, and an idle CPU has no tick to.
            let now = crate::clock::now_ns();
            let runnable = |t: &&TaskBox| t.state == State::Ready && t.cpu < count && (t.budget_ns == 0 || t.consumed < t.budget_ns || now >= t.period_start + t.period_ns);
            for t in self.tasks.iter().flatten().filter(runnable) { ready.mark(t.cpu); }
        }
        if ready.is_empty() { return; }
        for other in 0..count {
            if ready.has(other) && other != this && self.current[other] == 0 && !self.woken[other] { self.woken[other] = true; unsafe { cpu::wake(other); } }
        }
    }
    // Quota use of the task in `slot` (MC-1.7, MC-5.1): tasks reserved by its live children and endpoints it created or
    // delegated.
    fn used_tasks(&self, slot: usize) -> usize {
        let pid = self.tasks[slot].as_ref().unwrap().pid;
        self.tasks.iter().flatten().filter(|t| t.state != State::Exited && t.parent == Some((slot, pid))).map(|t| 1 + t.quota_tasks).sum()
    }
    fn used_endpoints(&self, slot: usize) -> usize {
        let pid = self.tasks[slot].as_ref().unwrap().pid;
        let created = (FIRST_ENDPOINT..self.endpoints.len()).filter(|&e| self.endpoints[e] && self.endpoint_owner[e] == Some((slot, pid))).count();
        created + self.tasks.iter().flatten().filter(|t| t.state != State::Exited && t.parent == Some((slot, pid))).map(|t| t.quota_endpoints).sum::<usize>()
    }
    fn live(&self, slot: usize) -> bool { slot != 0 && self.tasks[slot].as_ref().is_some_and(|t| t.state != State::Exited) }
    // Whether `slot` was spawned by `ancestor` or by one of its live descendants (the parent links, while each lives).
    fn descends_from(&self, slot: usize, ancestor: usize) -> bool {
        let mut at = slot;
        for _ in 0..self.tasks.len() {
            let Some((parent, pid)) = self.tasks[at].as_ref().and_then(|t| t.parent) else { return false };
            if !self.tasks[parent].as_ref().is_some_and(|t| t.pid == pid && t.state != State::Exited) { return false; }
            if parent == ancestor { return true; }
            at = parent;
        }
        false
    }
    // The task that pays for `slot`'s memory above it, if it still lives.
    fn payer_of(&self, slot: usize) -> Option<usize> {
        let (payer, pid) = self.tasks[slot].as_ref()?.payer?;
        self.tasks[payer].as_ref().is_some_and(|t| t.pid == pid && t.state != State::Exited).then_some(payer)
    }
    // Whether an application-band allocation of `bytes` leaves the recovery reserve in the frame pool (MC-6.5, issue 169).
    fn leaves_reserve(&self, application: bool, bytes: usize) -> bool {
        !application || self.reserve == 0 || crate::frames::stats().1.saturating_sub(bytes) >= self.reserve
    }
    // Charges private memory to the task and every payer above it; none may go over its quota (MC-3.13).
    fn charge(&mut self, slot: usize, bytes: usize) -> bool {
        let mut at = Some(slot); let mut depth = 0;
        while let Some(s) = at.filter(|_| depth < self.tasks.len()) {
            let task = self.tasks[s].as_ref().unwrap();
            if task.memory_tree.saturating_add(bytes) > task.memory_quota { return false; }
            at = self.payer_of(s); depth += 1;
        }
        self.walk_payers(slot, |task| task.memory_tree += bytes);
        true
    }
    fn uncharge(&mut self, slot: usize, bytes: usize) { self.walk_payers(slot, |task| task.memory_tree = task.memory_tree.saturating_sub(bytes)); }
    // Kernel structures a task grew or gave back since it was last looked at (its capability table, its page tables) go
    // to its payers' accounts, as its image and stack do; past a quota if need be, which then refuses the payer's next
    // allocation. Growth is bounded by CAP_SLOTS_MAX and the page-table limit (171-KRN-0032).
    fn settle_kernel(&mut self, slot: usize) {
        let task = self.tasks[slot].as_mut().unwrap();
        let (charged, now) = (task.kernel, kernel_bytes(task));
        if now == charged { return; }
        task.kernel = now;
        let Some(up) = self.payer_of(slot) else { return };
        if now > charged { self.walk_payers(up, |t| t.memory_tree += now - charged); } else { self.uncharge(up, charged - now); }
    }
    fn walk_payers(&mut self, slot: usize, mut f: impl FnMut(&mut Task)) {
        let mut at = Some(slot); let mut depth = 0;
        while let Some(s) = at.filter(|_| depth < self.tasks.len()) { f(self.tasks[s].as_mut().unwrap()); at = self.payer_of(s); depth += 1; }
    }
    fn focus(&mut self, slot: usize) {
        if let Some(task) = self.tasks[self.foreground].as_mut() { task.input.clear(); }
        if let Some(task) = self.tasks[slot].as_mut() { task.input.clear(); }
        self.foreground = slot; self.dirty = true;
    }
    fn push_notice(&mut self, value: usize) { if self.notice_count < self.notices.len() { self.notices[self.notice_count] = value; self.notice_count += 1; } }
    // Input event: the focus owner gets the `owner` byte, any other focused task the `app` byte; an attention key
    // (Ctrl+Z) takes the focus back to the owner.
    fn route_key(&mut self, app: usize, owner: usize, attention: bool) {
        let target = self.foreground;
        if attention {
            if target != self.focus_owner && self.live(target) && self.live(self.focus_owner) {
                let pid = self.tasks[target].as_ref().unwrap().pid; self.focus(self.focus_owner); self.push_notice(pid as usize);
            }
            return;
        }
        let (key, pressed) = (event_key(owner), event_pressed(owner));
        // A listened key goes to its listener; its release follows the press even if the modifiers changed meanwhile.
        let taken = self.listeners.iter().position(|l| l.is_some_and(|l| l.key == key && if pressed { event_mods(owner) & LISTEN_MODS == l.mods } else { l.down }));
        if let Some(index) = taken.filter(|_| key != 0) {
            let mut listener = self.listeners[index].unwrap(); listener.down = pressed; self.listeners[index] = Some(listener);
            if self.tasks[listener.slot].as_ref().is_some_and(|t| t.pid == listener.pid) && self.live(listener.slot) {
                let task = self.tasks[listener.slot].as_mut().unwrap(); task.input.push(owner);
                if matches!(task.state, State::Sleeping(_)) { task.state = State::Ready; self.readied.mark(task.cpu); }
            }
            return;
        }
        if !self.live(target) { return; }
        let event = if target == self.focus_owner { owner } else { app };
        let task = self.tasks[target].as_mut().unwrap();
        if event_key(event) == KEY_POINTER && !task.pointer { return; } // only tasks that asked for the pointer (INPUT_POINTER)
        task.input.push(event);
        if matches!(task.state, State::Sleeping(_)) { task.state = State::Ready; self.readied.mark(task.cpu); }
    }

    fn mailbox(&self, slot: usize) -> *mut SyscallMailbox { unsafe { self.tasks[slot].as_ref().unwrap().abi.ptr().add(4096).cast() } }

    // Common exit path (exit, kill, exception): wakes clients waiting for a reply from the task.
    fn terminate(&mut self, slot: usize, notify: bool, reason: usize) {
        let task = self.tasks[slot].as_mut().unwrap(); let pid = task.pid; task.state = State::Exited; task.pending_cap = None; task.exit_reason = reason;
        self.ended[self.ended_next] = (pid, reason); self.ended_next = (self.ended_next + 1) % EXIT_STATUSES;
        // Final recovery boundary (MC-6.8): without init no policy or bootstrap authority is left, so the system stops.
        if task.parent.is_none() { use core::fmt::Write; let _ = write!(crate::Fatal::begin(), "INIT EXITED: SYSTEM HALTED (REASON={:016X})\n", reason); cpu::halt_all(); }
        if let Some(ep) = task.watch { self.post_exit(ep, pid, reason); }
        // Messages queued for an instance that no longer exists are not handed to the next one (MC-6.4).
        for other in 1..self.tasks.len() {
            let Some(State::BlockedSend(ep)) = self.tasks[other].as_ref().map(|t| t.state) else { continue };
            if !self.receivable(ep) { let sender = self.tasks[other].as_mut().unwrap(); sender.pending_cap = None; sender.state = State::Ready; self.readied.mark(sender.cpu); unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!((*self.mailbox(other)).result), ERR_PEER); } }
        }
        for other in 1..self.tasks.len() { if self.tasks[other].as_ref().is_some_and(|t| t.state == State::BlockedReply(slot)) { self.fail_reply(other); } }
        if self.foreground == slot {
            // Output the focus owner has not read yet is kept until the next focused task exits.
            let console = core::mem::replace(&mut self.tasks[slot].as_mut().unwrap().console, Queue::new());
            self.exited_console = Some((pid, console));
            // A task started in front (SPAWN_FOREGROUND) gives the focus back to the one that started it, which the focus
            // owner still sees in front: no notice (issue 160).
            let handed = self.tasks[slot].as_ref().unwrap().handed_by.filter(|&(s, p)| s != self.focus_owner && self.tasks[s].as_ref().is_some_and(|t| t.pid == p) && self.live(s));
            if let Some((back, _)) = handed { self.focus(back); }
            else {
                let owner = if self.live(self.focus_owner) { self.focus_owner } else { 0 };
                self.focus(owner);
                if notify { self.push_notice(pid as usize | NOTICE_EXITED); }
            }
        }
        else if self.tasks[slot].as_ref().is_some_and(|t| t.screen.is_none() && !t.service && !t.console.is_empty()) {
            // A console program (no screen) that exits: its launcher reads the last output after the exit.
            let console = core::mem::replace(&mut self.tasks[slot].as_mut().unwrap().console, Queue::new());
            self.exited_console = Some((pid, console));
        }
        if self.focus_owner == slot { self.focus_owner = 0; if self.foreground == slot { self.focus(0); } }
        for listener in self.listeners.iter_mut() { if listener.is_some_and(|l| l.slot == slot) { *listener = None; } }
        // The task's own memory, image, stack, screen and kernel structures included, leaves its payers' accounts (what it keeps referenced is bounded by DETACHED_MAX_BYTES);
        // its children's memory is paid from now on by its payer, which already counts it.
        let task = self.tasks[slot].as_ref().unwrap(); let payer = task.payer;
        let own = task.heap.bytes() + task.heap.retained + task._image.len() + task._stack.len() + task.screen.as_ref().map_or(0, Region::len) + task.kernel;
        if let Some(up) = self.payer_of(slot) { self.uncharge(up, own); }
        for child in self.tasks.iter_mut().flatten() { if child.payer == Some((slot, pid)) { child.payer = payer; } }
        let task = self.tasks[slot].as_mut().unwrap(); task.payer = None; task.memory_tree = 0;
    }
    // Uptime at which an IPC with this handle word times out (0: never).
    fn deadline(word: usize) -> u64 { match word >> IPC_TIMEOUT_SHIFT { 0 => 0, ms => (interrupts::milliseconds() + ms as u64).max(1) } }
    // Fails blocked IPC whose deadline passed (MC-2.13): no message is half-delivered and no reply can reach a new call.
    fn expire(&mut self, now: u64) -> bool {
        let mut woken = false;
        for slot in 1..self.tasks.len() {
            let Some(task) = self.tasks[slot].as_mut() else { continue };
            if task.deadline == 0 || now < task.deadline || !matches!(task.state, State::BlockedSend(_) | State::BlockedRecv(_) | State::BlockedReply(_)) { continue; }
            // A late reply finds the caller no longer waiting for this call number and fails with ERR_PEER.
            if let State::BlockedSend(ep) | State::BlockedRecv(ep) = task.state { self.accounting.endpoint[ep].timeouts += 1; }
            let task = self.tasks[slot].as_mut().unwrap();
            task.state = State::Ready; task.pending_cap = None; task.deadline = 0; woken = true; self.readied.mark(task.cpu);
            unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!((*self.mailbox(slot)).result), ERR_TIMEOUT); }
        }
        woken
    }
    // The client is still blocked in exactly this call to `server`.
    fn awaits_reply(&self, client: usize, pid: u64, seq: u64, server: usize) -> bool {
        self.tasks[client].as_ref().is_some_and(|t| t.pid == pid && t.call_seq == seq && t.state == State::BlockedReply(server))
    }
    fn fail_reply(&mut self, slot: usize) {
        let mailbox = self.mailbox(slot);
        unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!((*mailbox).result), ERR_PEER); }
        let task = self.tasks[slot].as_mut().unwrap(); task.state = State::Ready; self.readied.mark(task.cpu);
    }
    fn find(&self, pid: u64) -> Option<usize> { (1..self.tasks.len()).find(|&i| { self.tasks[i].as_ref().is_some_and(|t| t.pid == pid && t.state != State::Exited) }) }
    // A free endpoint index; the tables double when full (issue 171), unless the kernel heap cannot grow them.
    fn new_endpoint(&mut self) -> Option<usize> {
        if let Some(ep) = (FIRST_ENDPOINT..self.endpoints.len()).find(|&e| !self.endpoints[e]) { return Some(ep); }
        let (len, more) = (self.endpoints.len(), self.endpoints.len());
        if self.endpoints.try_reserve_exact(more).is_err() || self.endpoint_owner.try_reserve_exact(more).is_err() || self.accounting.endpoint.try_reserve_exact(more).is_err() { return None; }
        self.endpoints.resize(len + more, false); self.endpoint_owner.resize(len + more, None); self.accounting.endpoint.resize(len + more, EndpointCounters::default());
        Some(len)
    }
    // A free kernel-allocated slot, or the next one the table can grow by (issue 171).
    fn free_slot(cspace: &[Option<Capability>]) -> Option<usize> { (SLOT_DYNAMIC..cspace.len()).find(|&i| cspace[i].is_none()).or((cspace.len() < CAP_SLOTS_MAX).then_some(cspace.len())) }
    // Handle -> slot index (MC-3.2): fixed slots take generation 0, kernel-allocated ones their current generation.
    fn index(&self, slot: usize, handle: usize) -> Option<usize> {
        let (index, generation) = (handle & HANDLE_SLOT_MASK, handle >> HANDLE_GENERATION_SHIFT);
        if index == 0 || index >= self.tasks[slot].as_ref().unwrap().cspace.len() { return None; }
        let expected = if index < SLOT_DYNAMIC { 0 } else { self.tasks[slot].as_ref().unwrap().generations[index] as usize };
        (generation == expected).then_some(index)
    }
    fn handle(task: &Task, index: usize) -> usize { if index < SLOT_DYNAMIC { index } else { index | (task.generations[index] as usize) << HANDLE_GENERATION_SHIFT } }
    // Stores a new capability in a free kernel-allocated slot and returns its handle.
    fn fresh(&mut self) -> u64 { self.next_node += 1; self.next_node }
    // Stores a new capability with its node in a free kernel-allocated slot and returns its handle.
    fn insert(task: &mut Task, cap: Capability, node: Node) -> Option<usize> {
        let index = Self::room(task)?;
        task.cspace[index] = Some(cap); task.nodes[index] = node; Some(Self::handle(task, index))
    }
    // A free kernel-allocated slot. The table doubles when full; a frame pool that cannot grow it refuses the capability.
    // The growth is charged at the task's next system call (settle_kernel).
    fn room(task: &mut Task) -> Option<usize> {
        let index = Self::free_slot(&task.cspace)?;
        if index == task.cspace.len() {
            let more = task.cspace.len().min(CAP_SLOTS_MAX - task.cspace.len());
            if task.cspace.try_reserve_exact(more).is_err() || task.generations.try_reserve_exact(more).is_err() || task.nodes.try_reserve_exact(more).is_err() { return None; }
            task.cspace.resize(task.cspace.len() + more, None); task.generations.resize(task.generations.len() + more, 1); task.nodes.resize(task.nodes.len() + more, Node::default());
        }
        Some(index)
    }
    fn root(&mut self) -> Node { Node { id: self.fresh(), parent: 0 } }
    // Frees a slot; a kernel-allocated slot moves to the next generation (20 bits) so old handles stay invalid.
    fn clear(task: &mut Task, index: usize) {
        task.cspace[index] = None;
        if index >= SLOT_DYNAMIC { task.generations[index] = (task.generations[index] % HANDLE_GENERATION_MAX as u32) + 1; }
    }

    // Whether anything still derives from node `id`: a capability, a send in flight, a mapping or another ghost.
    fn derived(&self, id: u64) -> bool {
        self.ghosts.iter().any(|g| g.parent == id) || self.tasks.iter().flatten().any(|t| {
            (1..t.cspace.len()).any(|i| t.cspace[i].is_some() && t.nodes[i].parent == id) || t.pending_cap.is_some_and(|p| p.node.parent == id) || t.heap.made_from(id)
        })
    }
    // Removes a capability from a table (drop, overwrite, exit); its descendants stay revocable through a ghost node.
    fn remove(&mut self, slot: usize, index: usize) {
        let task = self.tasks[slot].as_mut().unwrap();
        if task.cspace[index].is_none() { return; }
        let node = task.nodes[index]; Self::clear(task, index);
        if node.id != 0 && self.ghosts.len() < GHOSTS_MAX && self.derived(node.id) { self.ghosts.push(node); }
    }
    // Whether a physical range is still in use by someone else: via a capability, an in-flight send or a mapping.
    fn referenced(&self, physical: usize, size: usize) -> bool {
        self.tasks.iter().flatten().any(|t| t.cspace.iter().flatten().chain(t.pending_cap.as_ref().map(|p| &p.cap)).any(|c| c.overlaps(physical, size)) || t.heap.maps_foreign(physical, size))
    }
    // No writable capability, writable mapping or DMA region overlaps the range (sealed: SHARE_RO holds).
    fn sealed(&self, physical: usize, size: usize) -> bool {
        let writes = |c: &Capability| match *c { Capability::Memory(p, s, r) => r & CAP_WRITE != 0 && p < physical + size && physical < p + s, Capability::Dma(..) => c.overlaps(physical, size), _ => false };
        !self.dma.iter().any(|r| (r.ptr() as usize) < physical + size && physical < r.ptr() as usize + r.len())
            && !self.tasks.iter().flatten().any(|t| t.screen.as_ref().is_some_and(|s| (s.ptr() as usize) < physical + size && physical < s.ptr() as usize + s.len()) || t.cspace.iter().flatten().chain(t.pending_cap.as_ref().map(|p| &p.cap)).any(writes) || t.heap.writes(physical, size))
    }
    // Keeps memory alive while referenced; a living owner keeps paying for it (heap quota) until it is released.
    // Returns whether it was kept.
    fn retire(&mut self, region: Region, owner: Option<(usize, u64)>) -> bool {
        if !self.referenced(region.ptr() as usize, region.len()) { return false; }
        if let Some((slot, _)) = owner { self.tasks[slot].as_mut().unwrap().heap.retained += region.len(); }
        self.orphans.push(Orphan { region, owner });
        true
    }

    // Frees exited tasks only after their CPU has switched to a different CR3.
    fn reap(&mut self) {
        let mut released: Vec<Region> = Vec::new();
        for slot in 1..self.tasks.len() {
            if self.current.contains(&slot) || !self.tasks[slot].as_ref().is_some_and(|t| t.state == State::Exited) { continue; }
            for index in 1..self.tasks[slot].as_ref().unwrap().cspace.len() { self.remove(slot, index); }
            let mut task = self.tasks[slot].take().unwrap();
            // Its CPU's list loses the slot and keeps room for exactly the rest: the kernel heap returns as tasks end.
            let list = &mut self.on_cpu[task.cpu];
            if let Ok(at) = list.binary_search(&slot) {
                list.remove(at);
                if list.is_empty() { *list = Vec::new(); } else if let Some(exact) = exact_copy(list) { *list = exact; }
            }
            released.extend(task.heap.take_regions()); released.extend(task.screen.take());
        }
        self.tasks.shrink(); // empty chunks at the end of the table go back to the kernel heap
        // The IRQ binding is removed once nobody owns the line capability anymore.
        for irq in 0..LINES {
            // A binder that ended no longer holds the line back.
            let mut changed = false;
            for i in 0..IRQ_SHARERS {
                if let Some(b) = self.irq_bind[irq][i] { if !self.tasks[b.slot].as_ref().is_some_and(|t| t.pid == b.pid && t.state != State::Exited) { self.irq_bind[irq][i] = None; changed = true; } }
            }
            let bound = self.irq_bind[irq].iter().any(Option::is_some);
            if (bound || changed) && !self.tasks.iter().flatten().any(|t| t.state != State::Exited && t.cspace.contains(&Some(Capability::Interrupt(irq as u8)))) {
                self.irq_bind[irq] = [None; IRQ_SHARERS]; self.irq_pending[irq] = false; unsafe { interrupts::set_irq_masked(irq as u8, true); }
            } else if changed && bound && self.irq_bind[irq].iter().flatten().all(|b| !b.unacked) { unsafe { interrupts::set_irq_masked(irq as u8, false); } }
        }
        for region in released { let _ = self.retire(region, None); }
        // Nothing is freed while a CPU may still hold a stale translation of revoked memory.
        let mut index = 0;
        while index < self.orphans.len() && !self.flush.iter().any(|&f| f) {
            let (start, length) = (self.orphans[index].region.ptr() as usize, self.orphans[index].region.len());
            if self.referenced(start, length) { index += 1; continue; }
            if let Some((slot, pid)) = self.orphans[index].owner { if let Some(owner) = self.tasks[slot].as_mut().filter(|t| t.pid == pid) { owner.heap.retained -= length; self.uncharge(slot, length); } }
            self.orphans.swap_remove(index);
        }
        // Ghost nodes nobody derives from any more are forgotten.
        let mut index = 0;
        while index < self.ghosts.len() { if self.derived(self.ghosts[index].id) { index += 1; } else { self.ghosts.swap_remove(index); } }
        if self.orphans.is_empty() && self.orphans.capacity() != 0 { self.orphans = Vec::new(); } // an empty list holds no heap memory
        let mut used = alloc::vec![false; self.endpoints.len()]; used[..FIRST_ENDPOINT].fill(true);
        for task in self.tasks.iter().flatten() { for cap in task.cspace.iter().flatten().chain(task.pending_cap.as_ref().map(|p| &p.cap)) { if let Capability::Endpoint(id, _, _) = cap { used[*id] = true; } } }
        for b in self.irq_bind.iter().flatten().flatten() { used[b.ep] = true; }
        for (ep, owner) in self.endpoint_owner.iter_mut().enumerate() { if !used[ep] { *owner = None; } }
        self.exits.retain(|e| used[e.0]); // notices for an endpoint nobody holds any more
        // Unused endpoints at the end of a grown table go, back to its first size at least (issue 171).
        let keep = used.iter().rposition(|&u| u).map_or(0, |last| last + 1).max(ENDPOINTS);
        if keep < used.len() {
            used.truncate(keep); used.shrink_to_fit();
            self.endpoint_owner.truncate(keep); self.endpoint_owner.shrink_to_fit(); self.accounting.endpoint.truncate(keep); self.accounting.endpoint.shrink_to_fit();
        }
        self.endpoints = used;
    }

    // FIRMWARE_VARIABLE (351-KRN-0027): the request is copied in, the firmware sees only kernel memory, the answer is
    // copied back.
    unsafe fn firmware_variable(&mut self, slot: usize, ptr: *mut SyscallMailbox, request: &SyscallMailbox) -> Result<usize, usize> {
        let space = &self.tasks[slot].as_ref().unwrap().space;
        let (address, length) = (request.msg[0], request.msg[1]);
        if !(FIRMWARE_HEADER..=FIRMWARE_BUFFER).contains(&length) || !space.validate_read(address, length) { return Err(ERR_INVALID); }
        let mut buffer = alloc::vec![0u8; length];
        for (i, byte) in buffer.iter_mut().enumerate() { *byte = core::ptr::read_volatile(space.readable(address + i).unwrap() as *const u8); }
        let guid: [u8; 16] = buffer[..16].try_into().unwrap();
        let attributes = u32::from_le_bytes(buffer[16..20].try_into().unwrap());
        let (units, data_length) = (u16::from_le_bytes(buffer[20..22].try_into().unwrap()) as usize, u32::from_le_bytes(buffer[22..26].try_into().unwrap()) as usize);
        let name_end = FIRMWARE_HEADER + units * 2;
        if units == 0 || name_end > length { return Err(ERR_INVALID); }
        let mut name: alloc::vec::Vec<u16> = buffer[FIRMWARE_HEADER..name_end].chunks_exact(2).map(|u| u16::from_le_bytes([u[0], u[1]])).collect();
        name.push(0);
        match request.arg2 {
            FIRMWARE_GET => match crate::firmware::get(&name, &guid, &mut buffer[name_end..]) {
                Ok((attributes, got)) => {
                    buffer[16..20].copy_from_slice(&attributes.to_le_bytes()); buffer[22..26].copy_from_slice(&(got as u32).to_le_bytes());
                    if copy_out(space, address, &buffer[..name_end + got]) { Ok(got) } else { Err(ERR_INVALID) }
                }
                Err((crate::firmware::BUFFER_TOO_SMALL, needed)) => { core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).arg2), needed); Err(ERR_INVALID) }
                Err((crate::firmware::NOT_FOUND, _)) => Err(ERR_NOT_FOUND),
                Err(_) => Err(ERR_INVALID),
            },
            FIRMWARE_SET if name_end + data_length <= length => match crate::firmware::set(&name, &guid, attributes, &buffer[name_end..name_end + data_length]) {
                Ok(()) => Ok(0),
                Err(crate::firmware::NOT_FOUND) => Err(ERR_NOT_FOUND),
                Err(_) => Err(ERR_RIGHTS),
            },
            _ => Err(ERR_INVALID),
        }
    }

    // `bytes` in frames of their own, as a read-only memory object (kept for the system's life: asked for once a boot).
    fn copy_out(bytes: &[u8]) -> Result<Capability, usize> {
        let size = bytes.len().max(1).next_multiple_of(4096);
        let region = Region::task(size, 4096).map_err(|_| ERR_NO_MEMORY)?;
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), region.ptr(), bytes.len()); }
        let base = region.ptr() as usize;
        core::mem::forget(region);
        Ok(Capability::Memory(base, size, CAP_READ))
    }

    // Another kind of device with memory registers in the pages of `bar` of device `a`.
    fn page_sharer(&self, a: usize, bar: pci::Bar) -> Option<usize> {
        let (start, end, class) = (bar.base & !0xFFF, (bar.base + bar.size).next_multiple_of(4096), self.devices[a].class);
        self.devices.iter().enumerate().position(|(i, d)| i != a && d.class != class && d.bars.iter().any(|x| !x.io && x.size != 0 && x.base < end && x.base + x.size > start))
    }

    // Test-only (211-KRN-0021): the RTL8139's 256-byte register BAR is packed into the page of the SD host controller's,
    // as Apple's firmware packs EHCI next to AHCI; granted, it must move to a page of its own and answer there.
    #[cfg(feature = "bar-move-test")]
    fn bar_move_test(&mut self) {
        let say = |text: core::fmt::Arguments| { let _ = core::fmt::Write::write_fmt(&mut crate::PanicSerial, text); };
        let (Some(nic), Some(sd)) = (self.devices.iter().position(|d| d.id == 0x8139_10EC), self.devices.iter().position(|d| d.class == 0x08_05_01)) else {
            return say(format_args!("MIND CORE KERNEL: PCI TEST: NO RTL8139 AND SD HOST CONTROLLER\n"));
        };
        let packed = self.devices[sd].bars[0].base + 0x800;
        if !unsafe { pci::move_bar(&mut self.devices[nic], 1, packed) } { return say(format_args!("MIND CORE KERNEL: PCI TEST: THE BAR WAS NOT PACKED\n")); }
        say(format_args!("MIND CORE KERNEL: PCI TEST: BAR 1 OF {:06X} PACKED AT {:X}, IN THE PAGE OF {:06X}\n", self.devices[nic].location(), packed, self.devices[sd].location()));
        match self.platform_cap(PLATFORM_DEVICE_BAR, nic, 1) {
            Ok(Capability::Mmio(base, _)) => {
                let mac: [u8; 6] = core::array::from_fn(|i| unsafe { core::ptr::read_volatile((base + i) as *const u8) });
                say(format_args!("MIND CORE KERNEL: PCI TEST: GRANTED AT {:X}, MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} READ THERE\n", base, mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]));
            }
            Ok(_) => say(format_args!("MIND CORE KERNEL: PCI TEST: NOT A REGISTER CAPABILITY\n")),
            Err(code) => say(format_args!("MIND CORE KERNEL: PCI TEST: REFUSED ({:X})\n", code)),
        }
    }

    // A free 4 KiB page for a BAR moved off a shared one (211-KRN-0021): within the span the firmware gave bus 0's
    // devices (decoded to PCI), below the fixed ranges at 0xFEC0_0000, clear of every BAR, bridge window, the ECAM and
    // every range of the firmware's memory map.
    fn free_page(&self) -> Option<u64> {
        let below = |x: &pci::Bar| !x.io && x.size != 0 && x.base != 0 && x.base + x.size <= 1 << 32;
        let bus0 = || self.devices.iter().filter(|d| d.location() >> 8 == 0 && d.class >> 8 != 0x0604).flat_map(|d| d.bars).filter(below);
        let (low, high) = (bus0().map(|x| x.base).min()? & !0xFFF, bus0().map(|x| x.base + x.size).max()?.min(0xFEC0_0000));
        let mut page = low;
        while page + 4096 <= high {
            let end = page + 4096;
            let taken = self.devices.iter().flat_map(|d| d.bars.into_iter().filter(|x| !x.io && x.size != 0).map(|x| (x.base, x.base + x.size)).chain(d.windows))
                .chain(crate::pcicfg::ecam())
                .chain((0..self.boot.memory_map_len).map(|i| unsafe { *self.boot.memory_map.add(i) }).map(|r| (r.start as u64, r.start as u64 + r.pages as u64 * 4096)))
                .filter(|&(from, to)| to > from && from < end && to > page).map(|(_, to)| to).max();
            match taken { None => return Some(page), Some(to) => page = to.next_multiple_of(4096) }
        }
        None
    }

    // The PCI function with a BAR covering physical (or port) address `base`.
    fn device_at(&self, base: u64) -> Option<&pci::Device> { self.devices.iter().find(|d| d.bars.iter().any(|bar| bar.size != 0 && base >= bar.base && base < bar.base + bar.size)) }

    // Capability over a platform resource the kernel has validated (PLATFORM_CAP).
    fn platform_cap(&mut self, kind: usize, a: usize, b: usize) -> Result<Capability, usize> {
        match kind {
            PLATFORM_PORTS => {
                let end = a.checked_add(b).ok_or(ERR_INVALID)?;
                if b == 0 || !platform::PORTS.iter().any(|&(base, count)| a >= base as usize && end <= base as usize + count as usize) { return Err(ERR_RIGHTS); }
                Ok(Capability::IoPorts(a as u16, b as u16))
            }
            PLATFORM_IRQ if platform::irq(a) => Ok(Capability::Interrupt(a as u8)),
            PLATFORM_MMIO => {
                let (base, bytes) = platform::mmio(a).ok_or(ERR_NOT_FOUND)?;
                if base == platform::console() { MIRROR_LOGS.store(false, Ordering::Relaxed); } // its driver shows the logs now
                Ok(Capability::Mmio(base, bytes))
            }
            PLATFORM_DEVICE_BAR => {
                let device = *self.devices.get(a).ok_or(ERR_NOT_FOUND)?; let mut bar = *device.bars.get(b).ok_or(ERR_INVALID)?;
                if bar.size == 0 { return Err(ERR_NOT_FOUND); }
                if bar.io { unsafe { pci::enable(&device); } return Ok(Capability::IoPorts(bar.base as u16, bar.size.min(0xFFFF) as u16)); }
                // Registers are mapped by the page, and firmware may pack small BARs into one (Apple's EHCI next to AHCI,
                // 211-KRN-0021): such a BAR moves to a free page first. If none is free, other kinds of device there go
                // to no other driver once this one is granted, nor this one after theirs.
                if let Some(other) = self.page_sharer(a, bar).filter(|_| !device.granted && bar.size < 4096 && bar.base + bar.size <= 1 << 32) {
                    let other = self.devices[other].location();
                    if let Some(page) = self.free_page() {
                        let from = bar.base;
                        if unsafe { pci::move_bar(&mut self.devices[a], b, page) } {
                            bar.base = page;
                            let _ = core::fmt::Write::write_fmt(&mut crate::PanicSerial, format_args!("MIND CORE KERNEL: PCI: BAR {} OF {:06X} MOVED FROM {:X} TO {:X}: ITS PAGE HELD REGISTERS OF {:06X}\n", b, device.location(), from, page, other));
                        }
                    }
                }
                let (start, end) = (bar.base & !0xFFF, (bar.base + bar.size).next_multiple_of(4096));
                let mut sharers = self.devices.iter().enumerate().filter(|&(i, d)| i != a && d.class != device.class
                    && d.bars.iter().any(|b| !b.io && b.size != 0 && b.base < end && b.base + b.size > start));
                if let Some((_, other)) = sharers.clone().find(|(_, d)| d.granted) {
                    let _ = core::fmt::Write::write_fmt(&mut crate::PanicSerial, format_args!("MIND CORE KERNEL: PCI: BAR {} OF {:06X} AT {:X} NOT GRANTED: ITS PAGE HOLDS REGISTERS OF {:06X}, ALREADY GRANTED\n", b, device.location(), bar.base, other.location()));
                    return Err(ERR_RIGHTS);
                }
                if let Some((_, other)) = sharers.next() {
                    let _ = core::fmt::Write::write_fmt(&mut crate::PanicSerial, format_args!("MIND CORE KERNEL: PCI: BAR {} OF {:06X} AT {:X} SHARES ITS PAGE WITH {:06X} (CLASS {:06X}), WHICH NO OTHER DRIVER GETS\n", b, device.location(), bar.base, other.location(), other.class));
                }
                self.devices[a].granted = true;
                unsafe { pci::enable(&device); }
                Ok(Capability::Mmio(bar.base as usize, (end - bar.base) as usize))
            }
            PLATFORM_DEVICE_IRQ => match self.devices.get(a).ok_or(ERR_NOT_FOUND)?.irq { 0 | 2 => Err(ERR_NOT_FOUND), irq => Ok(Capability::Interrupt(irq)) },
            // An MSI-X vector for table entry `b` of device `a`, aimed at the BSP; the entry is programmed here, so a
            // driver never chooses where its device's messages go. The vector stays with that entry for good.
            PLATFORM_DEVICE_MSIX => {
                let device = *self.devices.get(a).ok_or(ERR_NOT_FOUND)?;
                let entry = u16::try_from(b).map_err(|_| ERR_INVALID)?;
                let index = match self.msi.iter().position(|m| *m == Some((a, entry))) { Some(index) => index, None => self.msi.iter().position(Option::is_none).ok_or(ERR_NO_SLOT)? };
                // The table's 2 MiB page becomes uncached in the kernel's identity map, unless the firmware lists RAM there.
                let at = unsafe { pci::msix_entry(&device, entry) }.ok_or(ERR_NOT_FOUND)? as usize & !0x1F_FFFF;
                let ram = (0..self.boot.memory_map_len).map(|i| unsafe { *self.boot.memory_map.add(i) })
                    .any(|r| !matches!(r.kind, 11 | 12) && (r.start as usize) < at + 0x20_0000 && at < r.start as usize + r.pages as usize * 4096);
                if ram { return Err(ERR_RIGHTS); }
                unsafe { paging::uncached(at); }
                unsafe { pci::msix(&device, entry, index) }.ok_or(ERR_NOT_FOUND)?;
                self.msi[index] = Some((a, entry));
                Ok(Capability::Interrupt((MSI_FIRST + index) as u8))
            }
            PLATFORM_FRAMEBUFFER => Ok(Capability::Memory(self.boot.fb_ptr as usize, frame_bytes(&self.boot), MEMORY_ALL)),
            // The hardware report and the ACPI tables, as read-only copies in frames of their own (174-KRN-0038).
            PLATFORM_REPORT => Self::copy_out(&crate::report::build(&self.boot, &self.devices)),
            PLATFORM_ACPI_TABLE => Self::copy_out(crate::report::acpi_table(a).ok_or(ERR_NOT_FOUND)?),
            PLATFORM_DMA => {
                // 64 KiB aligned so a driver's data buffer does not cross a DMA boundary.
                let bytes = a.checked_next_multiple_of(4096).filter(|&n| n > 0).ok_or(ERR_INVALID)?;
                if self.dma.iter().map(Region::len).sum::<usize>() + bytes > DMA_LIMIT { return Err(ERR_NO_MEMORY); }
                let region = Region::new(bytes, 64 * 1024).map_err(|_| ERR_NO_MEMORY)?;
                let cap = Capability::Dma(region.ptr() as usize, region.len()); self.dma.push(region); Ok(cap)
            }
            // In escrow the holder only passes the privilege on to a service it spawns (issue 170).
            PLATFORM_PRIVILEGE if b == PRIVILEGE_ESCROW => Capability::privilege(a).map(|_| Capability::Escrow(a as u8)).ok_or(ERR_INVALID),
            PLATFORM_PRIVILEGE if b != 0 => Err(ERR_INVALID),
            PLATFORM_PRIVILEGE => if a == CAP_KIND_RESTART { Ok(Capability::Restart) } else { Capability::privilege(a).ok_or(ERR_INVALID) },
            _ => Err(ERR_INVALID),
        }
    }

    // New task from an ELF with the given capabilities; flags are SPAWN_SERVICE / SPAWN_SCREEN.
    fn spawn_internal(&mut self, source: Source, name: Name, args: &[u8], flags: usize, caps: [Option<Capability>; SLOT_DYNAMIC], nodes: [Node; SLOT_DYNAMIC], parent: Option<(usize, u64)>, quotas: (usize, usize, usize)) -> Result<u64, &'static str> {
        let (service, has_screen) = (flags & SPAWN_SERVICE != 0, flags & SPAWN_SCREEN != 0);
        // A free slot, or a new chunk of the table (issue 171): the kernel heap is the only limit beyond the quotas.
        let slot = match (1..self.tasks.len()).find(|&i| self.tasks[i].is_none()) { Some(slot) => slot, None => { let len = self.tasks.len(); if !self.tasks.grow() { return Err("OUT OF MEMORY: TASK TABLE"); } len } };
        let pid = self.next_pid; let next_pid = pid.checked_add(1).ok_or("PID SPACE EXHAUSTED")?;
        let file = match source { Source::Boot(index) => { let image = self.boot.programs.get(index).ok_or("UNKNOWN PROGRAM")?; if image.len == 0 { return Err("UNKNOWN PROGRAM"); } unsafe { core::slice::from_raw_parts(image.data, image.len) } } Source::Image(bytes) => bytes };
        let elf = elf::Image::parse(file)?;
        let screen_bytes = if has_screen { frame_bytes(&self.boot) } else { 0 };
        if !self.leaves_reserve(!service, elf.size.div_ceil(4096) * 4096 + STACK_SIZE + screen_bytes + core::mem::size_of::<Task>() + 3 * 4096 + context::size()) { return Err("OUT OF MEMORY: RECOVERY RESERVE"); }
        let mut image = Region::task(elf.size.div_ceil(4096) * 4096, 4096)?; let entry = elf.load(image.bytes_mut(), paging::USER_IMAGE)?;
        let mut space = paging::Space::new()?;
        for (offset, size, flags) in elf.segments() { space.map(paging::USER_IMAGE + offset, image.ptr() as usize + offset, size, flags & 2 != 0, flags & 1 != 0)?; }
        let stack = Region::task(STACK_SIZE, 4096)?; let abi = Region::task(8192, 4096)?;
        let screen = if has_screen { Some(Region::task(screen_bytes, 4096)?) } else { None };
        let mut info = self.boot; info.cpu_features = cpu::features(); info.fb_ptr = if has_screen { paging::USER_SCREEN as *mut u32 } else { core::ptr::null_mut() }; info.heap_ptr = core::ptr::null_mut(); info.heap_len = 0; info.programs = self.boot.programs.map(|image| ProgramImage { data: core::ptr::null(), len: image.len }); info.ap_trampoline = 0; info.cpu_count = 0; info.apic_ids = [0; 8]; info.abi_version = if cfg!(feature = "abi-test") { ABI_VERSION + 1 } else { ABI_VERSION }; info.efi_runtime = 0; info.device_tree = 0; // which images exist, not where; no CPU or firmware addresses
        unsafe { (abi.ptr() as *mut BootInfo).write(info); }
        let args = &args[..args.len().min(ARGS_MAX)];
        unsafe { let page = core::slice::from_raw_parts_mut(abi.ptr().add(ARGS_OFFSET), 2 + ARGS_MAX); page[..2].copy_from_slice(&(args.len() as u16).to_le_bytes()); page[2..2 + args.len()].copy_from_slice(args); }
        let exit = Region::task(4096, 4096)?; let stub = context::exit_stub(paging::USER_MAILBOX as u64); unsafe { core::ptr::copy_nonoverlapping(stub.as_ptr(), exit.ptr(), stub.len()); } cpu::code_written(); let user_sp = unsafe { context::prepare_stack(stack.ptr() as usize, STACK_SIZE) };
        space.map(paging::USER_STACK, stack.ptr() as usize, stack.len(), true, false)?;
        if let Some(screen) = &screen { space.map(paging::USER_SCREEN, screen.ptr() as usize, screen.len(), true, false)?; }
        space.map(paging::USER_INFO, abi.ptr() as usize, 4096, false, false)?; space.map(paging::USER_MAILBOX, abi.ptr() as usize + 4096, 4096, true, false)?; space.map(paging::USER_EXIT, exit.ptr() as usize, 4096, false, true)?;
        let context = Region::task(context::size(), 64)?; let sp = context.ptr() as usize; unsafe { context::initial(sp, entry, user_sp); }
        // Applications are balanced by per-CPU application count: sleeping services don't skew the balance.
        let cpu = (0..cpu::COUNT.load(Ordering::Acquire)).filter(|&i| cpu::ONLINE[i].load(Ordering::Acquire)).min_by_key(|&i| { self.tasks.iter().flatten().filter(|t| t.cpu == i && t.state != State::Exited && t.service == service).count() }).unwrap_or(0);
        // The fixed slots from the grants, then free kernel-allocated ones; the table grows later as needed (issue 171).
        if self.on_cpu[cpu].try_reserve_exact(1).is_err() { return Err("OUT OF MEMORY: TASK LIST"); }
        let (mut cspace, mut generations, mut table) = (Vec::new_in(Frames), Vec::new_in(Frames), Vec::new_in(Frames));
        if cspace.try_reserve_exact(CAP_SLOTS).is_err() || generations.try_reserve_exact(CAP_SLOTS).is_err() || table.try_reserve_exact(CAP_SLOTS).is_err() { return Err("OUT OF MEMORY: CAPABILITY TABLE"); }
        cspace.extend_from_slice(&caps); cspace.resize(CAP_SLOTS, None); generations.resize(CAP_SLOTS, 1); table.extend_from_slice(&nodes); table.resize(CAP_SLOTS, Node::default());
        let nodes = table;
        let mut task = alloc::boxed::Box::try_new_in(Task { pid, name, service, state: State::Ready, sp, cpu, space, heap: crate::user_heap::Heap::with_limit(quotas.2), context, _exit: exit, runs: 0, ticks: 0, calls: 0, run_ns: 0, sends: 0, receives: 0, started_ns: crate::clock::now_ns(), _image: image, _stack: stack, screen, abi, input: Events::new(), pointer: false, log: Queue::new(), console: Queue::new(), console_read_ns: if service { 0 } else { crate::clock::now_ns() }, dirty: true, cspace, generations, nodes, pending_cap: None, pending_call: false, pending_badge: 0, send_seq: 0, reply_to: None, call_seq: 0, deadline: 0, watch: None, band: if service { BAND_SYSTEM as u8 } else { BAND_APPLICATION as u8 }, budget_ns: 0, period_ns: 0, period_start: 0, consumed: 0, parent, quota_tasks: quotas.0, quota_endpoints: quotas.1, memory_quota: quotas.2, memory_tree: 0, payer: parent, exit_reason: 0, handed_by: None, kernel: 0 }, Frames).map_err(|_| "OUT OF MEMORY: TASK")?;
        // Image, stack, screen and the task's kernel structures are charged to the spawner and every payer above it,
        // after the last fallible step (issue 168, 171-KRN-0032).
        task.kernel = kernel_bytes(&task);
        let fixed = task._image.len() + task._stack.len() + task.screen.as_ref().map_or(0, Region::len) + task.kernel;
        if let Some((spawner, _)) = parent { if !self.charge(spawner, fixed) { return Err("OVER MEMORY QUOTA"); } }
        self.tasks[slot] = Some(task);
        if let Err(at) = self.on_cpu[cpu].binary_search(&slot) { self.on_cpu[cpu].insert(at, slot); }
        self.readied.mark(cpu); self.next_pid = next_pid; Ok(pid)
    }

    fn cap(&self, slot: usize, handle: usize) -> Option<Capability> { self.index(slot, handle).and_then(|index| self.tasks[slot].as_ref().unwrap().cspace[index]) }
    // Copy of a capability for transfer; IPC endpoint rights are narrowed by the sender's mask.
    // Capability to hand over: a copy (child node, rights narrowed by the mask) or, with CAP_TRANSFER_MOVE, the same
    // node; reply caps are one-shot and not transferable.
    fn transfer(&mut self, slot: usize, handle: usize, mask: usize) -> Option<Pending> {
        if handle == 0 { return None; }
        let index = self.index(slot, handle)?;
        let cap = match self.cap(slot, handle)? { Capability::Endpoint(id, rights, badge) => Capability::Endpoint(id, rights & mask as u8, badge), Capability::Reply(..) => return None, other => other };
        if Self::move_only(cap) && mask & CAP_TRANSFER_MOVE == 0 { return None; } // a writable object without grant has one owner
        let source = self.tasks[slot].as_ref().unwrap().nodes[index];
        // A memory capability moves only unmapped: the sender's mapping would stay a second owner (issue 167).
        if mask & CAP_TRANSFER_MOVE != 0 && matches!(cap, Capability::Memory(..)) && self.tasks[slot].as_ref().unwrap().heap.made_from(source.id) { return None; }
        if mask & CAP_TRANSFER_MOVE != 0 { Some(Pending { cap, node: source, moved_from: Some(index) }) } else { let id = self.fresh(); Some(Pending { cap, node: Node { id, parent: source.id }, moved_from: None }) }
    }
    // Places a transferred capability in a fixed slot of `to`; a move empties the sender's slot if it still holds it.
    fn place(&mut self, from: usize, to: usize, receive: usize, pending: Pending) {
        if let Some(index) = pending.moved_from {
            let sender = self.tasks[from].as_mut().unwrap();
            if sender.nodes[index].id != pending.node.id || sender.cspace[index].is_none() { return; } // revoked meanwhile
            Self::clear(sender, index);
        }
        self.remove(to, receive); // a capability overwritten in a fixed slot is removed like a drop
        let receiver = self.tasks[to].as_mut().unwrap();
        receiver.cspace[receive] = Some(pending.cap); receiver.nodes[receive] = pending.node;
    }
    // Removes every descendant of `root` from all tables and blocked sends and unmaps the mappings made from them; returns
    // how many capabilities were removed and whether another CPU still runs an affected address space (flush pending).
    fn revoke(&mut self, root: u64, cpu: usize) -> (usize, bool) {
        let slots: usize = self.tasks.iter().flatten().map(|t| t.cspace.len() + 1).sum();
        let mut ids = alloc::vec![0u64; slots + GHOSTS_MAX + 1]; ids[0] = root; let (mut known, mut removed) = (1, 0);
        loop {
            let mut changed = false;
            for task in self.tasks.iter_mut().flatten() {
                for index in 1..task.cspace.len() {
                    if task.cspace[index].is_some() && ids[..known].contains(&task.nodes[index].parent) && !ids[..known].contains(&task.nodes[index].id) {
                        ids[known] = task.nodes[index].id; known += 1; Self::clear(task, index); removed += 1; changed = true;
                    }
                }
                // A moved capability in flight was already counted in its sender's slot.
                if let Some(p) = task.pending_cap { if ids[..known].contains(&p.node.parent) || (p.moved_from.is_some() && ids[1..known].contains(&p.node.id)) { task.pending_cap = None; if p.moved_from.is_none() { removed += 1; } changed = true; } }
            }
            let mut index = 0;
            while index < self.ghosts.len() {
                let ghost = self.ghosts[index];
                if ids[..known].contains(&ghost.parent) && known < ids.len() { ids[known] = ghost.id; known += 1; self.ghosts.swap_remove(index); changed = true; } else { index += 1; }
            }
            if !changed { break; }
        }
        let (current, mut wait) = (self.current, false);
        for (slot, task) in self.tasks.iter_mut().enumerate() {
            // Exited tasks too: one may still run on its CPU until that CPU switches away.
            let Some(task) = task.as_mut() else { continue };
            // A space live on another CPU keeps its page tables until that CPU has switched address space.
            let remote = task.cpu != cpu && current[task.cpu] == slot;
            if task.heap.revoke(&mut task.space, &ids[1..known], !remote) && remote { self.flush[task.cpu] = true; wait = true; unsafe { cpu::wake(task.cpu); } }
        }
        (removed, wait)
    }
    // A named capability that may only be moved but is being copied.
    fn copy_refused(&self, slot: usize, handle: usize, mask: usize) -> bool { handle != 0 && mask & CAP_TRANSFER_MOVE == 0 && self.cap(slot, handle).is_some_and(Self::move_only) }
    fn move_only(cap: Capability) -> bool { matches!(cap, Capability::Memory(_, _, r) if r & CAP_WRITE != 0 && r & CAP_GRANT == 0) }
    // Child with narrower authority (CAP_MINT): endpoint rights, port or page-aligned memory sub-range.
    fn mint(cap: Capability, mask: usize, offset: usize, length: usize, badge: usize) -> Option<Capability> {
        let range = |base: usize, size: usize, align: usize| -> Option<(usize, usize)> {
            let length = if length == 0 { size.checked_sub(offset)? } else { length };
            (offset % align == 0 && length % align == 0 && length > 0 && offset.checked_add(length)? <= size).then_some((base + offset, length))
        };
        match cap {
            // A badge is set once (MC-3.4): a child of a badged capability keeps it.
            Capability::Endpoint(id, rights, old) => (badge <= BADGE_MAX && (badge == 0 || old == 0 || old as usize == badge)).then(|| Capability::Endpoint(id, (rights | if rights & CAP_KEEP != 0 { CAP_READ } else { 0 }) & mask as u8, if old != 0 { old } else { badge as u16 })),
            Capability::IoPorts(base, count) => range(base as usize, count as usize, 1).map(|(b, c)| Capability::IoPorts(b as u16, c as u16)),
            Capability::Memory(base, size, rights) => {
                // Without grant only a read-only child: the writable owner stays unique.
                let allowed = if rights & CAP_GRANT == 0 { rights & !CAP_WRITE } else { rights };
                range(base, size, 4096).map(|(b, s)| Capability::Memory(b, s, allowed & mask as u8))
            }
            Capability::Dma(base, size) => range(base, size, 4096).map(|(b, s)| Capability::Dma(b, s)),
            Capability::Mmio(base, size) => range(base, size, 4096).map(|(b, s)| Capability::Mmio(b, s)),
            Capability::Reply(..) => None,
            other => Some(other),
        }
    }
    fn blocked(&self, state: State) -> Option<usize> { (1..self.tasks.len()).find(|&i| self.tasks[i].as_ref().is_some_and(|t| t.state == state)) }
    // Someone alive can still receive on the endpoint (otherwise a send would wait forever). Its creator, the usual
    // receiver, first: a send to a busy service passed over every task's capabilities each time (171-KRN-0009).
    fn receivable(&self, ep: usize) -> bool {
        let reads = |t: &Task| t.state != State::Exited && t.cspace.iter().flatten().any(|c| matches!(c, Capability::Endpoint(id, rights, _) if *id == ep && rights & CAP_READ != 0));
        let creator = self.endpoint_owner.get(ep).copied().flatten().filter(|&(slot, _)| slot < self.tasks.len());
        if creator.is_some_and(|(slot, pid)| self.tasks[slot].as_ref().is_some_and(|t| t.pid == pid && reads(t))) { return true; }
        self.tasks.iter().flatten().any(|t| reads(t)) || self.irq_bind.iter().flatten().flatten().any(|b| b.ep == ep)
    }

    // Delivers the message of a blocked or current sender to the receiver.
    unsafe fn deliver(&mut self, from: usize, to: usize) {
        let (from_mb, to_mb) = (self.mailbox(from), self.mailbox(to));
        let sender = self.tasks[from].as_mut().unwrap(); let (pid, call, cap, badge) = (sender.pid, sender.pending_call, sender.pending_cap.take(), sender.pending_badge);
        (*to_mb).msg[2] = (*from_mb).msg[2]; (*to_mb).msg[3] = (*from_mb).msg[3]; (*to_mb).arg1 = pid as usize; (*to_mb).msg[1] = if call { MSG_FLAG_CALL } else { 0 };
        let receive = (*to_mb).arg2 & RECEIVE_MASK; let delivered = cap.is_some() && (1..SLOT_DYNAMIC).contains(&receive); // fixed slots only
        (*to_mb).arg2 = badge as usize; // the badge of the capability the sender used
        if let (true, Some(pending)) = (delivered, cap) { self.place(from, to, receive, pending); }
        let receiver = self.tasks[to].as_mut().unwrap();
        (*to_mb).msg[0] = delivered as usize; (*to_mb).result = 0; receiver.state = State::Ready; receiver.receives += 1; self.readied.mark(receiver.cpu);
        let seq = if call { let sender = self.tasks[from].as_mut().unwrap(); sender.call_seq += 1; sender.call_seq } else { 0 };
        let receiver = self.tasks[to].as_mut().unwrap();
        let previous = if call { receiver.reply_to.replace((from, pid, seq)) } else { None };
        if let Some((old, old_pid, old_seq)) = previous { if self.awaits_reply(old, old_pid, old_seq, to) { self.fail_reply(old); } }
        let sender = self.tasks[from].as_mut().unwrap();
        if call { sender.state = State::BlockedReply(to); } else { (*from_mb).result = 0; sender.state = State::Ready; self.readied.mark(sender.cpu); }
    }
    // Exit notice for a lifecycle owner: delivered to a waiting receiver at once, otherwise kept (bounded).
    fn post_exit(&mut self, ep: usize, pid: u64, reason: usize) {
        if let Some(receiver) = self.blocked(State::BlockedRecv(ep)) { unsafe { self.notify_exit(receiver, pid, reason); } return; }
        if self.exits.len() < EXIT_NOTICES_MAX { self.exits.push((ep, pid, reason)); } else { self.exits_lost += 1; }
    }
    unsafe fn notify_exit(&mut self, to: usize, pid: u64, reason: usize) {
        let mb = self.mailbox(to); let lost = core::mem::take(&mut self.exits_lost);
        (*mb).msg = [0, MSG_FLAG_EXIT, pid as usize, reason | lost << 32]; (*mb).arg1 = 0; (*mb).arg2 = 0; (*mb).result = 0;
        let task = self.tasks[to].as_mut().unwrap(); task.state = State::Ready; self.readied.mark(task.cpu);
    }
    // IRQ notification from the kernel: sender PID 0, no capability.
    unsafe fn notify_irq(&mut self, to: usize, irq: usize) {
        let mb = self.mailbox(to);
        (*mb).msg = [0, MSG_FLAG_IRQ, irq, 0]; (*mb).arg1 = 0; (*mb).arg2 = 0; (*mb).result = 0;
        let task = self.tasks[to].as_mut().unwrap(); task.state = State::Ready; self.readied.mark(task.cpu);
    }

    // Line interrupt: already masked; wakes the driver or records the event.
    unsafe fn raise_irq(&mut self, irq: usize) {
        self.accounting.irqs[irq] += 1;
        // Every driver on a shared line hears it; the line stays masked until each has acknowledged.
        if self.irq_bind[irq].iter().any(Option::is_some) {
            for i in 0..IRQ_SHARERS {
                let Some(mut b) = self.irq_bind[irq][i] else { continue };
                b.unacked = true;
                if let Some(receiver) = self.blocked(State::BlockedRecv(b.ep)) { self.notify_irq(receiver, irq); } else { b.pending = true; }
                self.irq_bind[irq][i] = Some(b);
            }
            return;
        }
        let mut woken = false;
        for task in self.tasks.iter_mut().flatten() { if task.state == State::BlockedIrq(irq as u8) { task.state = State::Ready; woken = true; self.readied.mark(task.cpu); } }
        if !woken { self.irq_pending[irq] = true; }
    }

    // Ok(Some(sp)): caller is blocked, switch; Ok(None): return 0 immediately.
    unsafe fn ipc_send(&mut self, slot: usize, sp: usize, cpu: usize, request: &SyscallMailbox, call: bool) -> Result<Option<usize>, usize> {
        let Some(Capability::Endpoint(ep, rights, badge)) = self.cap(slot, request.arg1) else { return Err(ERR_INVALID); };
        if rights & CAP_WRITE == 0 { return Err(ERR_RIGHTS); }
        let receiver = self.blocked(State::BlockedRecv(ep));
        if receiver.is_none() && !self.receivable(ep) { return Err(ERR_PEER); }
        if self.copy_refused(slot, request.msg[0], request.msg[1]) { return Err(ERR_RIGHTS); }
        self.accounting.endpoint[ep].messages += 1;
        let cap = if rights & CAP_GRANT != 0 { self.transfer(slot, request.msg[0], request.msg[1]) } else { None };
        self.send_seq += 1; let seq = self.send_seq;
        let task = self.tasks[slot].as_mut().unwrap(); task.pending_cap = cap; task.pending_call = call; task.pending_badge = badge; task.send_seq = seq; task.sends += 1; task.deadline = Self::deadline(request.arg2);
        if let Some(receiver) = receiver {
            self.deliver(slot, receiver);
            if !call { return Ok(None); }
        } else {
            self.tasks[slot].as_mut().unwrap().state = State::BlockedSend(ep);
        }
        self.tasks[slot].as_mut().unwrap().dirty = true;
        Ok(Some(self.select(sp, cpu)))
    }
    unsafe fn ipc_recv(&mut self, slot: usize, sp: usize, cpu: usize, request: &SyscallMailbox) -> Result<Option<usize>, usize> {
        let Some(Capability::Endpoint(ep, rights, _)) = self.cap(slot, request.arg1) else { return Err(ERR_INVALID); };
        if rights & CAP_READ == 0 { return Err(ERR_RIGHTS); }
        let bound = (0..LINES).find_map(|line| (0..IRQ_SHARERS).find(|&i| self.irq_bind[line][i].is_some_and(|b| b.ep == ep && b.pending)).map(|i| (line, i)));
        if let Some((irq, i)) = bound { if let Some(b) = self.irq_bind[irq][i].as_mut() { b.pending = false; } self.notify_irq(slot, irq); return Ok(None); }
        if let Some(index) = self.exits.iter().position(|e| e.0 == ep) { let (_, pid, reason) = self.exits.remove(index); self.notify_exit(slot, pid, reason); return Ok(None); }
        let sender = (1..self.tasks.len()).filter(|&i| self.tasks[i].as_ref().is_some_and(|t| t.state == State::BlockedSend(ep))).min_by_key(|&i| self.tasks[i].as_ref().unwrap().send_seq);
        if let Some(sender) = sender { self.deliver(sender, slot); return Ok(None); }
        let task = self.tasks[slot].as_mut().unwrap(); task.state = State::BlockedRecv(ep); task.dirty = true; task.deadline = Self::deadline(request.arg2);
        Ok(Some(self.select(sp, cpu)))
    }
    // Reply to the last client or to the client from a saved reply capability (arg1 is its slot).
    unsafe fn ipc_reply(&mut self, slot: usize, request: &SyscallMailbox) -> Result<usize, usize> {
        let target = if request.arg1 == 0 { self.tasks[slot].as_mut().unwrap().reply_to.take() } else {
            match (self.cap(slot, request.arg1), self.index(slot, request.arg1)) { (Some(Capability::Reply(caller, pid, seq)), Some(index)) => { Self::clear(self.tasks[slot].as_mut().unwrap(), index); Some((caller, pid, seq)) } _ => None }
        };
        let Some((caller, pid, seq)) = target else { return Err(ERR_INVALID); };
        if !self.awaits_reply(caller, pid, seq, slot) { return Err(ERR_PEER); }
        if self.copy_refused(slot, request.msg[0], request.msg[1]) { return Err(ERR_RIGHTS); }
        let cap = self.transfer(slot, request.msg[0], request.msg[1]); let mb = self.mailbox(caller);
        (*mb).msg[2] = request.msg[2]; (*mb).msg[3] = request.msg[3]; (*mb).arg1 = self.tasks[slot].as_ref().unwrap().pid as usize;
        let receive = (*mb).arg2 & RECEIVE_MASK; let delivered = cap.is_some() && (1..SLOT_DYNAMIC).contains(&receive); // fixed slots only
        (*mb).arg2 = 0; // replies carry no badge
        if let (true, Some(pending)) = (delivered, cap) { self.place(slot, caller, receive, pending); }
        let task = self.tasks[caller].as_mut().unwrap();
        (*mb).msg[0] = delivered as usize; (*mb).msg[1] = 0; (*mb).result = 0; task.state = State::Ready; self.readied.mark(task.cpu);
        Ok(0)
    }
    fn ports(&self, slot: usize, index: usize, port: usize, width: usize) -> bool {
        matches!(self.cap(slot, index), Some(Capability::IoPorts(base, count)) if port >= base as usize && port + width <= base as usize + count as usize)
    }
    fn holds(&self, slot: usize, cap: Capability) -> bool { self.tasks[slot].as_ref().unwrap().cspace.contains(&Some(cap)) }

    // SPAWN: the image comes from a memory capability or (platform only) a boot image; the child's capabilities are
    // exactly the grant list.
    unsafe fn spawn(&mut self, slot: usize, request: &SyscallMailbox) -> Result<usize, usize> {
        if !self.holds(slot, Capability::Spawn) { return Err(ERR_RIGHTS); }
        // Boot images and services: the platform privilege, or the narrower restart privilege init keeps after boot.
        let platform = self.holds(slot, Capability::Platform) || self.holds(slot, Capability::Restart);
        let task = self.tasks[slot].as_ref().unwrap();
        let length = request.arg2; let (count, flags) = (request.msg[3] & 0xFF, (request.msg[3] >> 8) & 0xFF);
        let spawner = self.tasks[slot].as_ref().unwrap();
        // The child's memory quota is at most the spawner's: the subtree's memory is charged to both (MC-3.13).
        let memory = match request.msg[3] >> 48 { 0 => HEAP_MAX_BYTES.min(spawner.memory_quota), SPAWN_MEMORY_ALL => spawner.memory_quota, mib => mib << 20 };
        let quotas = ((request.msg[3] >> 16) & 0xFFFF, (request.msg[3] >> 32) & 0xFFFF, memory);
        if self.used_tasks(slot) + 1 + quotas.0 > spawner.quota_tasks || self.used_endpoints(slot) + quotas.1 > spawner.quota_endpoints || memory > spawner.memory_quota { return Err(ERR_LIMIT); }
        let parent = Some((slot, spawner.pid));
        if length == 0 || length > NAME_MAX + 1 + ARGS_MAX || count > SPAWN_GRANTS_MAX || !task.space.validate_read(request.arg1, length) { return Err(ERR_INVALID); }
        if flags & SPAWN_SERVICE != 0 && !platform { return Err(ERR_RIGHTS); }
        let service = flags & SPAWN_SERVICE != 0;
        // In front only for a task with a screen, and only from the task that has the focus now (issue 160).
        let front = if flags & SPAWN_FOREGROUND != 0 {
            if flags & SPAWN_SCREEN == 0 { return Err(ERR_INVALID); }
            let owner = self.find((request.msg[1] >> 32) as u64).ok_or(ERR_FOCUS)?;
            if owner != self.foreground || !self.live(owner) { return Err(ERR_FOCUS); }
            Some((owner, self.tasks[owner].as_ref().unwrap().pid))
        } else { None };
        let grant_bytes = count * GRANT_BYTES; // 16 bytes: own handle u64, child u8, rights u8, flags u16 (issue 172)
        if count > 0 && !task.space.validate_read(request.msg[2], grant_bytes) { return Err(ERR_INVALID); }
        // `name\0arguments`
        let mut text = [0u8; NAME_MAX + 1 + ARGS_MAX];
        for (i, byte) in text[..length].iter_mut().enumerate() { *byte = core::ptr::read_volatile(task.space.readable(request.arg1 + i).unwrap() as *const u8); }
        let name_len = text[..length].iter().position(|&b| b == 0).unwrap_or(length);
        if name_len == 0 || name_len > NAME_MAX { return Err(ERR_INVALID); }
        let args = if name_len < length { &text[name_len + 1..length] } else { &[][..] };
        let mut raw = [0u8; SPAWN_GRANTS_MAX * GRANT_BYTES];
        for (i, byte) in raw[..grant_bytes].iter_mut().enumerate() { *byte = core::ptr::read_volatile(task.space.readable(request.msg[2] + i).unwrap() as *const u8); }
        let (mut caps, mut nodes, mut moves) = ([None; SLOT_DYNAMIC], [Node::default(); SLOT_DYNAMIC], [None; SPAWN_GRANTS_MAX]);
        for (n, grant) in raw[..grant_bytes].chunks(GRANT_BYTES).enumerate() {
            let (own, child, rights, flags) = (u64::from_le_bytes(grant[..8].try_into().unwrap()) as usize, grant[8] as usize, grant[9] as usize, u16::from_le_bytes([grant[10], grant[11]]));
            if !(1..SLOT_DYNAMIC).contains(&child) { return Err(ERR_INVALID); }
            // A moved handle may appear once: two slots must never share one node.
            if raw[..grant_bytes].chunks(GRANT_BYTES).enumerate().any(|(m, other)| m != n && other[..8] == grant[..8] && (flags | u16::from_le_bytes([other[10], other[11]])) & GRANT_MOVE != 0) { return Err(ERR_INVALID); }
            let mut pending = self.transfer(slot, own, rights | if flags & GRANT_MOVE != 0 { CAP_TRANSFER_MOVE } else { 0 }).ok_or(ERR_INVALID)?;
            // An escrowed privilege becomes the privilege only in a service the spawner may start (issue 170).
            if let Capability::Escrow(kind) = pending.cap { if !service { return Err(ERR_INVALID); } pending.cap = Capability::privilege(kind as usize).ok_or(ERR_INVALID)?; }
            caps[child] = Some(pending.cap); nodes[child] = pending.node; moves[n] = pending.moved_from;
        }
        let source = if request.msg[0] & SPAWN_BOOT != 0 {
            if !platform { return Err(ERR_RIGHTS); }
            Source::Boot(request.msg[0] & !SPAWN_BOOT)
        } else {
            match self.cap(slot, request.msg[0]) {
                Some(Capability::Memory(physical, size, rights)) if rights & CAP_READ != 0 && request.msg[1] & 0xFFFF_FFFF <= size => Source::Image(core::slice::from_raw_parts(physical as *const u8, request.msg[1] & 0xFFFF_FFFF)),
                _ => return Err(ERR_INVALID),
            }
        };
        let pid = self.spawn_internal(source, Name::new(&text[..name_len]), args, flags, caps, nodes, parent, quotas).map_err(spawn_error)?;
        // Moved capabilities leave the spawner only once the child exists.
        for index in moves.into_iter().flatten() { Self::clear(self.tasks[slot].as_mut().unwrap(), index); }
        if let Some(owner) = front {
            let child = self.find(pid).unwrap();
            self.tasks[child].as_mut().unwrap().handed_by = Some(owner); self.focus(child);
            // The focus owner learns who is in front now (it shows that task's output too).
            if owner.0 != self.focus_owner && self.live(self.focus_owner) { self.push_notice(pid as usize | NOTICE_FRONT); }
        }
        Ok(pid as usize)
    }

    // Process control (TASK_LIST ... HALT): only for the holder of the control capability.
    unsafe fn control(&mut self, slot: usize, ptr: *mut SyscallMailbox, request: &SyscallMailbox) -> Result<usize, usize> {
        // Statistics need only the observe privilege; kill, focus, key listening, logs, console and halt need process control (MC-10.2).
        let observation = matches!(request.syscall_num, SYSCALL_TASK_LIST | SYSCALL_CPU_INFO | SYSCALL_KERNEL_HEAP | SYSCALL_FAULTS | SYSCALL_STAT);
        // A lifecycle owner ends its own descendants without process control (issue 170: init supervises without it).
        let descendant = request.syscall_num == SYSCALL_TASK_KILL && self.find(request.arg1 as u64).is_some_and(|target| self.descends_from(target, slot));
        // The restart privilege (init's) may reset the machine too, when a service with the right asks it (351-KRN-0022).
        let reset = request.syscall_num == SYSCALL_REBOOT && self.holds(slot, Capability::Restart);
        if !self.holds(slot, Capability::Control) && !(observation && self.holds(slot, Capability::Observe)) && !descendant && !reset { return Err(ERR_RIGHTS); }
        let task_slot = |s: &Self, pid: usize| if pid == 0 { Some(slot) } else { s.find(pid as u64) };
        match request.syscall_num {
            SYSCALL_STAT => self.stat(slot, request),
            // How a task that ended lately ended (issue 166): a launcher's results, msh's `err` for a failed program.
            SYSCALL_EXIT_STATUS => self.ended.iter().find(|e| e.0 != 0 && e.0 == request.arg1 as u64).map(|e| e.1).ok_or(ERR_NOT_FOUND),
            SYSCALL_TASK_LIST => {
                let mut count = 0;
                for (index, task) in self.tasks.iter().enumerate().skip(1) {
                    let Some(task) = task else { continue };
                    if count >= request.arg2 { break; }
                    let mut info = TaskInfo { pid: task.pid, name: [0; NAME_MAX], state: [b' '; 8], cpu: task.cpu as u32, focus: (self.foreground == index) as u8, service: task.service as u8, screen: task.screen.is_some() as u8, reserved: 0, runs: task.runs, ticks: task.ticks, calls: task.calls, quota_tasks: task.quota_tasks as u16, used_tasks: 0, quota_endpoints: task.quota_endpoints as u16, used_endpoints: 0 };
                    if task.state != State::Exited { info.used_tasks = self.used_tasks(index) as u16; info.used_endpoints = self.used_endpoints(index) as u16; }
                    info.name[..task.name.len as usize].copy_from_slice(&task.name.bytes[..task.name.len as usize]);
                    let label = if self.current.contains(&index) { "RUNNING" } else { task.state.label() }; info.state[..label.len()].copy_from_slice(label.as_bytes());
                    let bytes = core::slice::from_raw_parts((&info as *const TaskInfo).cast::<u8>(), core::mem::size_of::<TaskInfo>());
                    if !copy_out(&self.tasks[slot].as_ref().unwrap().space, request.arg1 + count * bytes.len(), bytes) { return Err(ERR_INVALID); }
                    count += 1;
                }
                Ok(count)
            }
            SYSCALL_TASK_KILL => {
                let target = self.find(request.arg1 as u64).ok_or(ERR_NOT_FOUND)?; self.terminate(target, false, EXIT_KILLED);
                let other = self.tasks[target].as_ref().unwrap().cpu;
                if other != cpu::id() && self.current[other] == target { cpu::wake(other); } // stops it before the next tick
                Ok(0)
            }
            SYSCALL_FOCUS => {
                let target = task_slot(self, request.arg1).ok_or(ERR_NOT_FOUND)?;
                if self.tasks[target].as_ref().unwrap().screen.is_none() { return Err(ERR_INVALID); }
                if request.arg2 == 0 { self.tasks[target].as_mut().unwrap().console.clear(); }
                self.tasks[target].as_mut().unwrap().handed_by = None; // it returns to the caller now (issue 160)
                self.focus_owner = slot; self.focus(target); Ok(self.tasks[target].as_ref().unwrap().pid as usize)
            }
            SYSCALL_INPUT_LISTEN => {
                let (key, mods) = (request.arg1 as u16, (request.arg1 >> 16) as u8);
                if key == 0 || key == KEY_POINTER || mods & !LISTEN_MODS != 0 || request.arg1 >> 24 != 0 { return Err(ERR_INVALID); }
                let pid = self.tasks[slot].as_ref().unwrap().pid;
                let same = self.listeners.iter().position(|l| l.is_some_and(|l| l.key == key && l.mods == mods));
                if request.arg2 == 0 {
                    // Only the listener itself stops listening.
                    return match same { Some(i) if self.listeners[i].unwrap().slot == slot => { self.listeners[i] = None; Ok(0) } _ => Err(ERR_NOT_FOUND) };
                }
                let index = same.or_else(|| self.listeners.iter().position(Option::is_none)).ok_or(ERR_NO_SLOT)?;
                self.listeners[index] = Some(Listener { key, mods, slot, pid, down: false }); Ok(0)
            }
            SYSCALL_TASK_LOGS | SYSCALL_CONSOLE_READ => {
                let (address, capacity) = (request.msg[0], request.msg[1].min(4096));
                let console = request.syscall_num == SYSCALL_CONSOLE_READ;
                let queue = match self.find(request.arg1 as u64) {
                    Some(target) => { let t = self.tasks[target].as_mut().unwrap(); if console { t.console_read_ns = crate::clock::now_ns(); &mut t.console } else { &mut t.log } }
                    // The last focused or screenless program that exited: its unread output (for TASK_LOGS too, so a
                    // background console program's output can be read after it ended).
                    None => match self.exited_console.as_mut() { Some((pid, queue)) if *pid == request.arg1 as u64 => queue, _ => return Err(ERR_NOT_FOUND) },
                };
                let mut buffer = [0u8; 4096]; let mut len = 0;
                while len < capacity { let Some(byte) = queue.pop() else { break }; buffer[len] = byte; len += 1; }
                if copy_out(&self.tasks[slot].as_ref().unwrap().space, address, &buffer[..len]) { Ok(len) } else { Err(ERR_INVALID) }
            }
            SYSCALL_NOTICE => {
                if self.notice_count == 0 { return Ok(0); }
                let value = self.notices[0]; self.notices.copy_within(1.., 0); self.notice_count -= 1; Ok(value)
            }
            SYSCALL_FAULTS => {
                let mut count = 0;
                for fault in self.faults.iter().flatten() {
                    if count >= request.arg2 { break; }
                    let bytes = core::slice::from_raw_parts((fault as *const FaultInfo).cast::<u8>(), core::mem::size_of::<FaultInfo>());
                    if !copy_out(&self.tasks[slot].as_ref().unwrap().space, request.arg1 + count * bytes.len(), bytes) { return Err(ERR_INVALID); }
                    count += 1;
                }
                Ok(count)
            }
            SYSCALL_CPU_INFO => {
                if request.arg1 >= cpu::COUNT.load(Ordering::Acquire) { return Err(ERR_NOT_FOUND); }
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).arg2), cpu::ONLINE[request.arg1].load(Ordering::Acquire) as usize);
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).msg[2]), cpu::TICKS[request.arg1].load(Ordering::Relaxed) as usize);
                Ok(cpu::apic_id(request.arg1) as usize)
            }
            SYSCALL_KERNEL_HEAP => {
                let before = crate::ALLOCATOR.lock().used();
                let test = alloc::format!("Dynamic allocation test at {} ms", interrupts::milliseconds()); core::hint::black_box(&test); drop(test);
                let heap = crate::ALLOCATOR.lock(); let (used, free) = (heap.used(), heap.free());
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).arg2), free);
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).msg[2]), (used == before) as usize);
                Ok(used)
            }
            SYSCALL_HALT => cpu::halt_all(),
            SYSCALL_REBOOT if request.arg1 == REBOOT_POWER_OFF => crate::acpi::power_off(),
            SYSCALL_REBOOT => crate::acpi::reboot(),
            _ => Err(ERR_INVALID),
        }
    }

    unsafe fn syscall(&mut self, slot: usize, sp: usize, cpu: usize) -> usize {
        let ptr = self.mailbox(slot); let request = core::ptr::read_volatile(ptr);
        self.settle_kernel(slot);
        let tasks: *mut Table<TaskBox> = &mut self.tasks; let task = (*(*tasks).ptr(slot)).as_mut().unwrap(); task.calls += 1;
        #[cfg(feature = "panic-test")] if request.syscall_num == SYSCALL_LOG { panic!("panic test"); }
        let result: Result<usize, usize> = match request.syscall_num {
            SYSCALL_RDTSC | SYSCALL_UPTIME | SYSCALL_CLOCK => Ok(clock_syscall(request.syscall_num, ptr).unwrap()),
            // The legacy byte of the next event that has one (events without a byte are skipped).
            SYSCALL_READ_KEY => Ok(loop { match task.input.pop() { None => break 0, Some(event) if event_byte(event) != 0 => break event_byte(event) as usize, Some(_) => {} } }),
            SYSCALL_READ_INPUT => Ok(task.input.pop().unwrap_or(0)),
            SYSCALL_INPUT_POINTER => { task.pointer = request.arg1 != 0; Ok(0) }
            SYSCALL_LOG => {
                // Kept twice: LOGS drains `log`, the focus owner mirrors `console` of the focused task.
                // A console its reader drained within the last second takes only what fits; the caller sends the rest again.
                let reader = task.console_read_ns != 0 && crate::clock::now_ns().saturating_sub(task.console_read_ns) < CONSOLE_READER_NS;
                let length = if reader { request.arg2.min(4096).min(task.console.room()) } else { request.arg2.min(4096) };
                if !task.space.validate_read(request.arg1, length) { Err(ERR_INVALID) } else {
                    // Until a task takes the screen, logs also go there in chunks (211-KRN-0017): a machine without COM1
                    // shows how far the services got.
                    let (mut shown, mut kept, showing) = ([0u8; 256], 0, crate::screen::showing());
                    for i in 0..length {
                        let physical = task.space.readable(request.arg1 + i).unwrap(); let byte = core::ptr::read_volatile(physical as *const u8);
                        task.log.push(byte); task.console.push(byte);
                        if MIRROR_LOGS.load(Ordering::Relaxed) { if byte == b'\n' { serial_write_byte(b'\r'); } serial_write_byte(byte); }
                        if showing { shown[kept] = byte; kept += 1; if kept == shown.len() { crate::screen::print_bytes(&shown); kept = 0; } }
                    }
                    if kept > 0 { crate::screen::print_bytes(&shown[..kept]); }
                    Ok(length)
                }
            }
            SYSCALL_ALLOC => match request.arg1.checked_add(HEAP_PAGE_SIZE - 1).map(|n| n & !(HEAP_PAGE_SIZE - 1)).filter(|&n| n > 0) {
                Some(size) if self.leaves_reserve(task.band == BAND_APPLICATION as u8, size) && self.charge(slot, size) => {
                    let task = (*(*tasks).ptr(slot)).as_mut().unwrap();
                    match task.heap.allocate(&mut task.space, size) { Some(address) => Ok(address), None => { self.uncharge(slot, size); Ok(0) } }
                }
                _ => Ok(0),
            },
            SYSCALL_FREE => match task.heap.free(&mut task.space, request.arg1) {
                None => Err(ERR_INVALID),
                Some(region) => {
                    // Memory others still map stays charged to the task until they let go.
                    if let Some(region) = region { let (owner, size) = (Some((slot, task.pid)), region.len()); if !self.retire(region, owner) { self.uncharge(slot, size); } }
                    Ok(0)
                }
            },
            SYSCALL_WAIT => {
                let now = interrupts::milliseconds(); let duration = request.arg1.min(60_000).div_ceil(10).max(1) as u64 * 10;
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).result), now as usize);
                task.state = if task.input.is_empty() { State::Sleeping(now.wrapping_add(duration)) } else { State::Ready };
                task.dirty = true; return self.select(sp, cpu);
            }
            SYSCALL_EXIT => { self.terminate(slot, true, EXIT_NORMAL | (request.arg1 & 0xFF_FFFF) << 8); return self.select(sp, cpu); }
            SYSCALL_ENDPOINT_CREATE => match (self.new_endpoint(), Self::room(task)) {
                _ if self.used_endpoints(slot) >= task.quota_endpoints => Err(ERR_LIMIT),
                (Some(ep), Some(_)) => { self.endpoints[ep] = true; self.endpoint_owner[ep] = Some((slot, task.pid)); let node = self.root(); Ok(Self::insert(task, Capability::Endpoint(ep, ENDPOINT_ALL, 0), node).unwrap()) }
                _ => Err(ERR_NO_SLOT),
            },
            SYSCALL_CAP_MINT => match (self.cap(slot, request.arg1), self.index(slot, request.arg1), Self::room(task)) {
                (Some(cap), Some(index), Some(_)) => match Self::mint(cap, request.arg2, request.msg[0], request.msg[1], request.msg[2]) {
                    Some(child) => { let node = Node { id: self.fresh(), parent: task.nodes[index].id }; Ok(Self::insert(task, child, node).unwrap()) }
                    None => Err(ERR_INVALID),
                },
                (Some(_), Some(_), None) => Err(ERR_NO_SLOT),
                _ => Err(ERR_INVALID),
            },
            SYSCALL_CAP_REVOKE => match self.index(slot, request.arg1).filter(|&i| task.cspace[i].is_some()) {
                Some(index) => {
                    let id = task.nodes[index].id; let (removed, wait) = self.revoke(id, cpu);
                    // Completion point (MC-3.6): return only once no CPU can still use a removed mapping.
                    if wait { core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).result), removed); self.tasks[slot].as_mut().unwrap().state = State::BlockedFlush; return self.select(sp, cpu); }
                    Ok(removed)
                }
                None => Err(ERR_INVALID),
            },
            SYSCALL_CAP_DROP => match self.index(slot, request.arg1) { Some(index) => { self.remove(slot, index); Ok(0) } None => Err(ERR_INVALID) },
            SYSCALL_SPAWN => self.spawn(slot, &request),
            SYSCALL_PLATFORM_CAP => {
                if !self.holds(slot, Capability::Platform) { Err(ERR_RIGHTS) } else {
                    match (Self::room(task), self.platform_cap(request.arg1, request.arg2, request.msg[0])) {
                        (Some(_), Ok(cap)) => { let node = self.root(); Ok(Self::insert((*(*tasks).ptr(slot)).as_mut().unwrap(), cap, node).unwrap()) }
                        (None, Ok(_)) => Err(ERR_NO_SLOT),
                        (_, Err(error)) => Err(error),
                    }
                }
            }
            SYSCALL_BOOT_CONFIRM => if self.holds(slot, Capability::Platform) { Ok(crate::trial::confirm() as usize) } else { Err(ERR_RIGHTS) },
            SYSCALL_FIRMWARE_VARIABLE => if !matches!(self.cap(slot, request.arg1), Some(Capability::Firmware)) { Err(ERR_RIGHTS) } else { self.firmware_variable(slot, ptr, &request) },
            SYSCALL_MEMORY_RESERVE => {
                let bytes = request.arg1.checked_next_multiple_of(4096);
                if !self.holds(slot, Capability::Platform) { Err(ERR_RIGHTS) }
                else { match bytes { Some(bytes) if crate::frames::ready() && bytes <= crate::frames::stats().0 => { self.reserve = bytes; Ok(0) } _ => Err(ERR_INVALID) } }
            }
            SYSCALL_DEVICE_STATE => match self.devices.get(request.arg1).copied() {
                // Whoever holds a capability over one of the device's registers may stop it.
                Some(device) if self.holds(slot, Capability::Platform) || task.cspace.iter().flatten().any(|c| device.bars.iter().any(|b| b.size != 0 && match *c {
                    Capability::Mmio(base, size) => !b.io && base as u64 >= b.base && (base + size) as u64 <= b.base + b.size.div_ceil(4096) * 4096,
                    Capability::IoPorts(base, count) => b.io && base as u64 >= b.base && base as u64 + count as u64 <= b.base + b.size,
                    _ => false,
                })) => match request.arg2 { DEVICE_STOP => { pci::quiesce(&device); Ok(0) } DEVICE_START => { pci::enable(&device); Ok(0) } _ => Err(ERR_INVALID) },
                Some(_) => Err(ERR_RIGHTS),
                None => Err(ERR_NOT_FOUND),
            },
            SYSCALL_DEVICE_FIND => {
                if !self.holds(slot, Capability::Platform) { Err(ERR_RIGHTS) } else {
                    let (class, mask) = (request.arg1 as u32, request.arg2 as u32);
                    self.devices.iter().enumerate().filter(|(_, d)| d.class & mask == class & mask && (request.msg[1] == 0 || d.id == request.msg[1] as u32)).nth(request.msg[0]).map(|(index, _)| index).ok_or(ERR_NOT_FOUND)
                }
            }
            // Configuration space, read only, of the PCI function one of whose BARs the capability covers; with the
            // platform privilege, of any device by index (msg[0]), without enabling it.
            SYSCALL_DEVICE_CONFIG => {
                let device = match self.cap(slot, request.arg1) {
                    Some(Capability::Mmio(base, _)) => self.device_at(base as u64),
                    Some(Capability::IoPorts(base, _)) => self.device_at(base as u64),
                    Some(Capability::Platform) => self.devices.get(request.msg[0]),
                    _ => None,
                };
                match device { Some(device) if request.arg2 < 256 => Ok(unsafe { pci::config(device, request.arg2 as u8) } as usize), Some(_) => Err(ERR_INVALID), None => Err(ERR_RIGHTS) }
            }
            SYSCALL_SCHED_SET => match self.find(request.arg1 as u64) {
                None => Err(ERR_NOT_FOUND),
                Some(target) => {
                    let (budget_us, period_us, band) = (request.arg2 as u64, request.msg[0] as u64, request.msg[1]);
                    let owner = self.tasks[target].as_ref().unwrap().parent == Some((slot, task.pid));
                    let control = self.holds(slot, Capability::Control);
                    let banding = control || self.holds(slot, Capability::Platform) || self.holds(slot, Capability::Restart);
                    if !(owner || control) || (band != BAND_KEEP && !banding) { Err(ERR_RIGHTS) }
                    else if (budget_us != 0 && (period_us < 10_000 || budget_us > period_us)) || !matches!(band, BAND_SYSTEM | BAND_APPLICATION | BAND_KEEP) { Err(ERR_INVALID) }
                    else {
                        let t = self.tasks[target].as_mut().unwrap();
                        t.budget_ns = budget_us * 1000; t.period_ns = period_us * 1000; t.period_start = crate::clock::now_ns(); t.consumed = 0;
                        if band != BAND_KEEP { t.band = band as u8; }
                        Ok(0)
                    }
                }
            },
            // A child that already ended (and is not reclaimed yet) gets its exit notice at once: a service that dies
            // before its owner watches it is not lost (issue 201).
            SYSCALL_TASK_WATCH if (1..self.tasks.len()).any(|i| self.tasks[i].as_ref().is_some_and(|t| t.pid == request.arg1 as u64 && t.state == State::Exited && t.parent == Some((slot, task.pid)))) => {
                let target = (1..self.tasks.len()).find(|&i| self.tasks[i].as_ref().is_some_and(|t| t.pid == request.arg1 as u64 && t.state == State::Exited)).unwrap();
                match self.cap(slot, request.arg2) {
                    Some(Capability::Endpoint(ep, rights, _)) if rights & CAP_READ != 0 => { let reason = self.tasks[target].as_ref().unwrap().exit_reason; self.post_exit(ep, request.arg1 as u64, reason); Ok(0) }
                    _ => Err(ERR_RIGHTS),
                }
            }
            SYSCALL_TASK_WATCH => match (self.find(request.arg1 as u64), self.cap(slot, request.arg2)) {
                // Only the lifecycle owner (the spawner) chooses where the exit notice goes.
                (Some(target), Some(Capability::Endpoint(ep, rights, _))) if rights & CAP_READ != 0 && self.tasks[target].as_ref().unwrap().parent == Some((slot, task.pid)) => { self.tasks[target].as_mut().unwrap().watch = Some(ep); Ok(0) }
                (None, _) => Err(ERR_NOT_FOUND),
                _ => Err(ERR_RIGHTS),
            },
            SYSCALL_MEM_DETACH => match task.heap.shareable(request.arg1, 0) {
                None => Err(ERR_INVALID),
                Some(_) if Self::room(task).is_none() => Err(ERR_NO_SLOT),
                Some((physical, size)) if self.referenced(physical, size) => Err(ERR_BUSY), // already shared: not a single owner
                Some((_, size)) if self.orphans.iter().map(|o| o.region.len()).sum::<usize>() + size > DETACHED_MAX_BYTES => Err(ERR_NO_MEMORY),
                Some((physical, size)) => {
                    // The detaching task keeps paying for the object while it lives (heap quota).
                    let region = task.heap.detach(&mut task.space, request.arg1).unwrap(); task.heap.retained += size;
                    self.orphans.push(Orphan { region, owner: Some((slot, task.pid)) });
                    let node = self.root(); Ok(Self::insert(task, Capability::Memory(physical, size, CAP_READ | CAP_WRITE), node).unwrap())
                }
            },
            SYSCALL_MEM_SHARE => match (task.heap.shareable(request.arg1, request.arg2), Self::room(task)) {
                (Some((physical, size)), Some(_)) => { let node = self.root(); Ok(Self::insert(task, Capability::Memory(physical, size, MEMORY_ALL), node).unwrap()) }
                (None, _) => Err(ERR_INVALID),
                _ => Err(ERR_NO_SLOT),
            },
            SYSCALL_MEM_MAP => {
                // The mapping remembers the capability's node: revoking that capability unmaps it.
                let (cap, node) = (self.cap(slot, request.arg1), self.index(slot, request.arg1).map_or(0, |i| task.nodes[i].id));
                let (physical, size, writable, device) = match cap {
                    Some(Capability::Memory(physical, size, rights)) if rights & CAP_READ != 0 => (physical, size, rights & CAP_WRITE != 0, false),
                    Some(Capability::Dma(physical, size)) => (physical, size, true, false),
                    Some(Capability::Mmio(physical, size)) => (physical, size, true, true),
                    _ => (0, 0, false, false),
                };
                // Registers that start inside a page: the page is mapped, the address of the first register returned.
                let within = if device { physical % 4096 } else { 0 };
                if size == 0 { Err(ERR_RIGHTS) } else {
                    core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).arg2), size); // mapping size for the client
                    task.heap.map_shared(&mut task.space, physical - within, size + within, device, writable, node).map(|address| address + within).ok_or(ERR_NO_MEMORY)
                }
            }
            SYSCALL_MEM_PHYS => match self.cap(slot, request.arg1) { Some(Capability::Dma(physical, _)) => Ok(physical), _ => Err(ERR_RIGHTS) },
            SYSCALL_PORT_IN => {
                let (port, width) = (request.arg2, request.msg[1].max(1));
                if !matches!(width, 1 | 2 | 4) || !self.ports(slot, request.arg1, port, width) { Err(ERR_RIGHTS) } else { Ok(port::read(port as u16, width)) }
            }
            SYSCALL_PORT_OUT => {
                let (port, width) = (request.arg2, request.msg[1].max(1));
                if !matches!(width, 1 | 2 | 4) || !self.ports(slot, request.arg1, port, width) { Err(ERR_RIGHTS) } else { port::write(port as u16, width, request.msg[0]); Ok(0) }
            }
            SYSCALL_PORT_IN_BLOCK => {
                // Reads 16-bit words (ATA sector) straight into the process buffer, without a syscall per word.
                let (buffer, words) = (request.msg[2], request.msg[3]);
                let pages_ok = words > 0 && words <= 2048 && buffer % 2 == 0 && buffer.checked_add(words * 2).is_some() && (buffer / 4096..=(buffer + words * 2 - 1) / 4096).all(|page| task.space.writable(page * 4096).is_some());
                if !pages_ok || !self.ports(slot, request.arg1, request.arg2, 2) { Err(ERR_RIGHTS) } else {
                    for i in 0..words { let target = task.space.writable(buffer + i * 2).unwrap(); core::ptr::write_volatile(target as *mut u16, port::read(request.arg2 as u16, 2) as u16); }
                    Ok(words)
                }
            }
            SYSCALL_PORT_OUT_BLOCK => {
                // Writes 16-bit words (ATA sector) from the process buffer, without a syscall per word.
                let (buffer, words) = (request.msg[2], request.msg[3]);
                let pages_ok = words > 0 && words <= 2048 && buffer % 2 == 0 && buffer.checked_add(words * 2).is_some() && (buffer / 4096..=(buffer + words * 2 - 1) / 4096).all(|page| task.space.readable(page * 4096).is_some());
                if !pages_ok || !self.ports(slot, request.arg1, request.arg2, 2) { Err(ERR_RIGHTS) } else {
                    for i in 0..words { let source = task.space.readable(buffer + i * 2).unwrap(); port::write(request.arg2 as u16, 2, core::ptr::read_volatile(source as *const u16) as usize); }
                    Ok(words)
                }
            }
            SYSCALL_IRQ_WAIT => match self.cap(slot, request.arg1) {
                Some(Capability::Interrupt(irq)) if self.irq_bind[irq as usize].iter().all(Option::is_none) => {
                    interrupts::set_irq_masked(irq, false);
                    if core::mem::take(&mut self.irq_pending[irq as usize]) { Ok(0) } else {
                        core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).result), 0);
                        task.state = State::BlockedIrq(irq); return self.select(sp, cpu);
                    }
                }
                _ => Err(ERR_RIGHTS),
            },
            SYSCALL_IRQ_BIND => match (self.cap(slot, request.arg1), self.cap(slot, request.arg2)) {
                (Some(Capability::Interrupt(irq)), Some(Capability::Endpoint(ep, rights, _))) if rights & CAP_READ != 0 => {
                    // A binder binds again in place (a restart reuses its slot); up to IRQ_SHARERS drivers share a line.
                    let line = &mut self.irq_bind[irq as usize]; let pid = task.pid;
                    match line.iter().position(|b| b.is_some_and(|b| b.slot == slot && b.pid == pid)).or_else(|| line.iter().position(Option::is_none)) {
                        Some(i) => {
                            line[i] = Some(IrqBinding { ep, slot, pid, pending: false, unacked: false });
                            if line.iter().flatten().all(|b| !b.unacked) { interrupts::set_irq_masked(irq, false); }
                            Ok(0)
                        }
                        None => Err(ERR_NO_SLOT),
                    }
                }
                _ => Err(ERR_RIGHTS),
            },
            SYSCALL_IRQ_ACK => match self.cap(slot, request.arg1) {
                Some(Capability::Interrupt(irq)) => {
                    let pid = task.pid;
                    for b in self.irq_bind[irq as usize].iter_mut().flatten() { if b.slot == slot && b.pid == pid { b.unacked = false; } }
                    if self.irq_bind[irq as usize].iter().flatten().all(|b| !b.unacked) { interrupts::set_irq_masked(irq, false); }
                    Ok(0)
                }
                _ => Err(ERR_RIGHTS),
            },
            SYSCALL_INPUT_EVENT => {
                // Only a holder of the input capability (keyboard driver, shell for the UART) may inject input.
                if !self.holds(slot, Capability::Input) { Err(ERR_RIGHTS) } else {
                    let word = |full: usize, byte: usize| if full != 0 { full } else if byte as u8 != 0 { input_event(byte as u8, 0, 0, true, 0) } else { 0 };
                    self.route_key(word(request.msg[1], request.arg1), word(request.msg[2], request.arg2), request.msg[0] != 0); Ok(0)
                }
            }
            SYSCALL_COMPOSITOR_PULL => {
                if !self.holds(slot, Capability::Display) || !(1..SLOT_DYNAMIC).contains(&request.arg1) { Err(ERR_RIGHTS) } else {
                    let focused = self.foreground;
                    match (*(*tasks).ptr(focused)).as_mut().filter(|_| focused != 0).filter(|t| t.screen.is_some()) {
                        None => Ok(0),
                        Some(t) => {
                            let (source, dirty) = (t.screen.as_ref().unwrap().ptr() as usize, core::mem::take(&mut t.dirty) | core::mem::take(&mut self.dirty));
                            // New capability only on screen change: the compositor keeps the mapping across frames.
                            // The compositor's first frame covers the whole screen: the kernel's boot lines and logs stop
                            // there, not when it is given the framebuffer (211-KRN-0017).
                            if source != self.composited { crate::screen::take(); self.composited = source; self.remove(slot, request.arg1); let task = (*(*tasks).ptr(slot)).as_mut().unwrap(); task.cspace[request.arg1] = Some(Capability::Memory(source, frame_bytes(&self.boot), CAP_READ)); task.nodes[request.arg1] = self.root(); Ok(2) } else { Ok(dirty as usize) }
                        }
                    }
                }
            }
            SYSCALL_CAP_INFO => {
                // Lets a driver learn what it was granted (e.g. the BAR port base) without seeing physical memory addresses.
                let (kind, base, size) = match self.cap(slot, request.arg1) {
                    None => (CAP_KIND_NONE, 0, 0),
                    Some(Capability::Endpoint(_, rights, badge)) => (CAP_KIND_ENDPOINT, badge as usize, rights as usize),
                    Some(Capability::Memory(_, size, rights)) => (CAP_KIND_MEMORY, rights as usize, size),
                    Some(Capability::Dma(_, size)) => (CAP_KIND_DMA, 0, size),
                    Some(Capability::Mmio(_, size)) => (CAP_KIND_MMIO, 0, size),
                    Some(Capability::IoPorts(base, count)) => (CAP_KIND_PORTS, base as usize, count as usize),
                    Some(Capability::Interrupt(irq)) => (CAP_KIND_IRQ, irq as usize, 0),
                    Some(Capability::Input) => (CAP_KIND_INPUT, 0, 0),
                    Some(Capability::Display) => (CAP_KIND_DISPLAY, 0, 0),
                    Some(Capability::Spawn) => (CAP_KIND_SPAWN, 0, 0),
                    Some(Capability::Reply(..)) => (CAP_KIND_REPLY, 0, 0),
                    Some(Capability::Platform) => (CAP_KIND_PLATFORM, 0, 0),
                    Some(Capability::Control) => (CAP_KIND_CONTROL, 0, 0),
                    Some(Capability::Restart) => (CAP_KIND_RESTART, 0, 0),
                    Some(Capability::Observe) => (CAP_KIND_OBSERVE, 0, 0),
                    Some(Capability::Firmware) => (CAP_KIND_FIRMWARE, 0, 0),
                    Some(Capability::Escrow(kind)) => (CAP_KIND_ESCROW, kind as usize, 0),
                };
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).arg2), base); core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).msg[2]), size);
                let sealed = match self.cap(slot, request.arg1) { Some(Capability::Memory(physical, size, _)) => self.sealed(physical, size), _ => false };
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).msg[3]), sealed as usize);
                Ok(kind)
            }
            SYSCALL_TASK_ALIVE => Ok(self.find(request.arg1 as u64).is_some() as usize),
            SYSCALL_IPC_SEND | SYSCALL_IPC_CALL | SYSCALL_IPC_RECV => {
                let outcome = if request.syscall_num == SYSCALL_IPC_RECV { self.ipc_recv(slot, sp, cpu, &request) } else { self.ipc_send(slot, sp, cpu, &request, request.syscall_num == SYSCALL_IPC_CALL) };
                match outcome { Ok(Some(next)) => return next, Ok(None) => Ok(0), Err(error) => Err(error) }
            }
            SYSCALL_IPC_REPLY => self.ipc_reply(slot, &request),
            SYSCALL_IPC_SAVE_REPLY => match (task.reply_to, Self::room(task)) {
                // Deferred reply: the server accepts further requests and replies to this client later.
                (Some((caller, pid, seq)), Some(_)) => { task.reply_to = None; let node = self.root(); Ok(Self::insert(task, Capability::Reply(caller, pid, seq), node).unwrap()) }
                (None, _) => Err(ERR_INVALID),
                _ => Err(ERR_NO_SLOT),
            },
            SYSCALL_TASK_LIST | SYSCALL_TASK_KILL | SYSCALL_FOCUS | SYSCALL_INPUT_LISTEN | SYSCALL_TASK_LOGS | SYSCALL_CONSOLE_READ | SYSCALL_NOTICE | SYSCALL_FAULTS | SYSCALL_CPU_INFO | SYSCALL_KERNEL_HEAP | SYSCALL_HALT | SYSCALL_REBOOT | SYSCALL_STAT | SYSCALL_EXIT_STATUS => {
                let result = self.control(slot, ptr, &request);
                // KILL of the caller itself or of the task it waits on is handled like an exit.
                if self.tasks[slot].as_ref().unwrap().state == State::Exited { return self.select(sp, cpu); }
                result
            }
            _ => Err(ERR_INVALID),
        };
        let (Ok(value) | Err(value)) = result;
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).result), value); sp
    }
}


// Bootstrap authority (MC-3.12): the kernel starts only boot image 0 (`init`) with its endpoint, the platform and
// spawn privileges; everything else is distributed by init.
pub fn spawn_init() -> Result<u64, &'static str> {
    locked(|| unsafe {
        let mut caps = [None; SLOT_DYNAMIC];
        let s = scheduler(); let ep = FIRST_ENDPOINT; s.endpoints[ep] = true; // init's own endpoint, charged to its quota below
        caps[SLOT_SERVICE] = Some(Capability::Endpoint(ep, ENDPOINT_ALL, 0));
        caps[SLOT_DEV0] = Some(Capability::Platform); caps[SLOT_DEV1] = Some(Capability::Spawn);
        let nodes = core::array::from_fn(|i| if caps[i].is_some() { s.root() } else { Node::default() });
        // init holds the root quota: as many tasks and endpoints as a quota can say (16 bits in SPAWN; issue 171),
        // and all task memory: the frame pool (issue 150).
        let memory = if crate::frames::ready() { crate::frames::stats().0 } else { s.boot.heap_len };
        let pid = s.spawn_internal(Source::Boot(0), Name::new(BOOT_SERVICES[0].as_bytes()), &[], SPAWN_SERVICE, caps, nodes, None, (QUOTA_MAX, QUOTA_MAX, memory))?;
        s.endpoint_owner[ep] = (1..s.tasks.len()).find(|&i| s.tasks[i].as_ref().is_some_and(|t| t.pid == pid)).map(|i| (i, pid));
        Ok(pid)
    })
}

pub extern "C" fn interrupt(sp: usize) -> usize {
    unsafe {
        let cpu = cpu::id();
        let event = context::event(sp);
        match event {
            Event::Stop => cpu::halt_here(),
            // A checked read of a register the CPU may not have (the hardware report) resumes as "absent".
            Event::KernelFault { pc, .. } if crate::arch::report::resume_after_fault(pc).is_some() => { context::resume_at(sp, crate::arch::report::resume_after_fault(pc).unwrap()); return sp; }
            Event::KernelFault { code, pc, error } => { use core::fmt::Write; let _ = write!(crate::Fatal::begin(), "KERNEL EXCEPTION VECTOR={} RIP={:016X} ERROR={:016X}\n", code, pc, error); cpu::halt_all(); }
            Event::Syscall => if let Some(next) = unlocked_syscall(cpu, sp) { return next; },
            _ => {}
        }
        locked(|| {
            let s = scheduler(); let slot = s.current[cpu]; s.accounting.interrupts[cpu] += 1;
            let next = (|| match event {
                Event::Irq(irq) => { s.raise_irq(irq); s.select(sp, cpu) }
                Event::Tick => {
                    cpu::TICKS[cpu].fetch_add(1, Ordering::Relaxed);
                    // Sleeps and IPC deadlines end on the boot CPU's tick, the source of every other; idle CPUs whose
                    // tasks that makes ready get a wake IPI (issue 171).
                    if cpu == 0 { let now = interrupts::milliseconds(); for task in s.tasks.iter_mut().flatten() { task.state.wake(now); } s.expire(now); s.wake_idle(cpu, true); crate::trial::check(now); }
                    if slot == 0 && cpu == 0 { return sp; } if slot != 0 { let t = s.tasks[slot].as_mut().unwrap(); t.ticks += 1; t.dirty = true; }
                    s.select(sp, cpu)
                }
                Event::Wake => if slot == 0 || s.flush[cpu] || s.tasks[slot].as_ref().is_some_and(|t| t.state == State::Exited) { s.select(sp, cpu) } else { sp },
                Event::Fault { code, error, pc, address } => {
                    let pid = s.tasks[slot].as_ref().unwrap().pid; let at = s.fault_cursor % s.faults.len();
                    s.faults[at] = Some(FaultInfo { pid, cpu: cpu as u64, vector: code, error, rip: pc, address });
                    s.fault_cursor += 1; s.terminate(slot, true, EXIT_FAULT | (code as usize) << 8); s.select(sp, cpu)
                }
                _ => {
                    if slot == 0 { return s.select(sp, cpu); }
                    if s.tasks[slot].as_ref().unwrap().state == State::Exited { return s.select(sp, cpu); }
                    s.syscall(slot, sp, cpu)
                }
            })();
            s.wake_idle(cpu, false);
            next
        })
    }
}

// Called from the BSP idle loop: reclaims memory of exited tasks.
pub fn reap() { locked(|| unsafe { scheduler().reap() }) }

pub fn idle() { interrupts::without(|| unsafe { let ready = locked(|| { let s = scheduler(); let c = cpu::id(); s.on_cpu[c].iter().any(|&i| i < s.tasks.len() && (s.ready(i, c, BAND_SYSTEM as u8) || s.ready(i, c, BAND_APPLICATION as u8))) }); if ready { cpu::reschedule(); } else { cpu::wait_for_interrupt(); } }); }
