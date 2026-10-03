use crate::abi::*;
use crate::input::Queue;
use crate::memory::Region;
use crate::task_state::{self, State};
use crate::{context, cpu, elf, interrupts, outb, paging, pci, serial_write_byte};
use alloc::vec::Vec;
use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

pub const MAX_TASKS: usize = 20; // services + applications
const SLOTS: usize = MAX_TASKS + 1;
const STACK_SIZE: usize = 64 * 1024;
const ENDPOINTS: usize = 64;
const DMA_LIMIT: usize = 8 * 1024 * 1024; // all DMA regions handed out through PLATFORM_DMA
// Legacy I/O ranges of the platform profile that may be handed to drivers: PS/2, CMOS, primary ATA, COM1.
// The PIC, PIT and PCI configuration ports stay with the kernel.
const LEGACY_PORTS: [(u16, u16); 6] = [(0x60, 1), (0x64, 1), (0x70, 2), (0x1F0, 8), (0x3F6, 1), (0x3F8, 8)];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Capability { Endpoint(usize, u8), Memory(usize, usize), Dma(usize, usize), Mmio(usize, usize), IoPorts(u16, u16), Interrupt(u8), Input, Display, Spawn, Reply(usize, u64), Platform, Control }

// Task name (for ps and spawn requests); application images are not indexed by a kernel table.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Name { bytes: [u8; NAME_MAX], len: u8 }
impl Name {
    pub fn new(text: &[u8]) -> Self { let len = text.len().min(NAME_MAX); let mut bytes = [0; NAME_MAX]; bytes[..len].copy_from_slice(&text[..len]); Self { bytes, len: len as u8 } }
}

// Where the ELF comes from: a boot image from the UEFI bootloader or a buffer passed by the spawner.
enum Source<'a> { Boot(usize), Image(&'a [u8]) }

impl Capability {
    fn overlaps(self, physical: usize, size: usize) -> bool {
        match self { Self::Memory(p, s) | Self::Dma(p, s) => p < physical + size && physical < p + s, _ => false }
    }
}

struct Task {
    pid: u64, name: Name, service: bool, state: State, sp: usize, cpu: usize,
    space: paging::Space, heap: crate::user_heap::Heap, context: Region, _exit: Region,
    runs: u64, ticks: u64, calls: u64, _image: Region, _stack: Region, screen: Option<Region>, abi: Region,
    input: Queue<128>, log: Queue<4096>, console: Queue<4096>, dirty: bool,
    cspace: [Option<Capability>; CAP_SLOTS],
    pending_cap: Option<Capability>, pending_call: bool, send_seq: u64, // send waiting for a receiver
    reply_to: Option<(usize, u64)>, // slot and PID of the client awaiting a reply
}
struct Scheduler {
    boot: BootInfo, tasks: [Option<Task>; SLOTS], current: [usize; cpu::MAX], idle_sp: [usize; cpu::MAX],
    faults: [Option<FaultInfo>; 16], fault_cursor: usize, next_pid: u64,
    foreground: usize, // focused task: its screen is shown and it receives input
    focus_owner: usize, // holder of process control that set the focus; focus returns to it
    notices: [usize; 8], notice_count: usize, // NOTICE values for the focus owner
    exited_console: Option<(u64, Queue<4096>)>, // console output of the last focused task that exited
    dirty: bool, endpoints: [bool; ENDPOINTS], irq_bind: [Option<usize>; 16], irq_pending: [bool; 16], send_seq: u64,
    orphans: Vec<Region>, // memory freed by its owner that is still mapped or held via a capability
    devices: Vec<pci::Device>, // PCI enumeration: discovery is a kernel mechanism, the choice of drivers is init's
    dma: Vec<Region>, // DMA regions handed out to init; they outlive driver restarts
    composited: usize, // screen the compositor already holds a capability for
}

static mut SCHEDULER: Option<Scheduler> = None;
static LOCK: AtomicBool = AtomicBool::new(false);
struct Guard; impl Drop for Guard { fn drop(&mut self) { LOCK.store(false, Ordering::Release); } }
fn locked<T>(f: impl FnOnce() -> T) -> T { interrupts::without(|| { while LOCK.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() { core::hint::spin_loop(); } let _guard = Guard; f() }) }

unsafe fn scheduler() -> &'static mut Scheduler { (*core::ptr::addr_of_mut!(SCHEDULER)).as_mut().unwrap() }

fn frame_bytes(info: &BootInfo) -> usize { (info.stride * info.height * 4).div_ceil(4096) * 4096 }

// Kernel spawn errors as ABI codes for the spawner.
fn spawn_error(error: &'static str) -> usize {
    match error { "TASK LIMIT REACHED" | "NO FREE TASK SLOT" => ERR_LIMIT, e if e.contains("MEMORY") || e.contains("PAGE TABLE") => ERR_NO_MEMORY, _ => ERR_INVALID }
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
    let mut endpoints = [false; ENDPOINTS]; endpoints[..EP_RESERVED].fill(true);
    unsafe { *core::ptr::addr_of_mut!(SCHEDULER) = Some(Scheduler { boot: *info, tasks: core::array::from_fn(|_| None), current: [0; cpu::MAX], idle_sp: [0; cpu::MAX], faults: [None; 16], fault_cursor: 0, next_pid: 1, foreground: 0, focus_owner: 0, notices: [0; 8], notice_count: 0, exited_console: None, dirty: true, endpoints, irq_bind: [None; 16], irq_pending: [false; 16], send_seq: 0, orphans: Vec::new(), devices, dma: Vec::new(), composited: 0 }); }
    Ok(())
}

impl Scheduler {
    fn states(&self, cpu: usize) -> [State; SLOTS] { core::array::from_fn(|i| { if i == 0 { State::Ready } else { self.tasks[i].as_ref().filter(|t| t.cpu == cpu).map_or(State::Empty, |t| t.state) } }) }
    fn select(&mut self, sp: usize, cpu: usize) -> usize {
        let current = self.current[cpu];
        if current == 0 { self.idle_sp[cpu] = sp; } else { let task = self.tasks[current].as_mut().unwrap(); unsafe { context::save(sp, task.context.ptr() as usize); } }
        let next = task_state::next(&self.states(cpu), current); self.current[cpu] = next;
        if next == 0 { unsafe { paging::activate(paging::kernel_root()); } self.idle_sp[cpu] } else { let task = self.tasks[next].as_mut().unwrap(); task.runs += 1; unsafe { paging::activate(task.space.root()); } task.sp }
    }
    // Other CPUs that sit idle while one of their tasks became ready get a wake IPI.
    fn wake_idle(&self, this: usize) {
        for other in 0..cpu::COUNT.load(Ordering::Acquire) {
            if other != this && self.current[other] == 0 && self.tasks.iter().flatten().any(|t| t.cpu == other && t.state == State::Ready) { unsafe { cpu::wake(other); } }
        }
    }
    fn live(&self, slot: usize) -> bool { slot != 0 && self.tasks[slot].as_ref().is_some_and(|t| t.state != State::Exited) }
    fn focus(&mut self, slot: usize) {
        if let Some(task) = self.tasks[self.foreground].as_mut() { task.input.clear(); }
        if let Some(task) = self.tasks[slot].as_mut() { task.input.clear(); }
        self.foreground = slot; self.dirty = true;
    }
    fn push_notice(&mut self, value: usize) { if self.notice_count < self.notices.len() { self.notices[self.notice_count] = value; self.notice_count += 1; } }
    // Input event: the focus owner gets the `owner` byte, any other focused task the `app` byte; an attention key
    // (Ctrl+Z) takes the focus back to the owner.
    fn route_key(&mut self, app: u8, owner: u8, attention: bool) {
        let target = self.foreground;
        if attention {
            if target != self.focus_owner && self.live(target) && self.live(self.focus_owner) {
                let pid = self.tasks[target].as_ref().unwrap().pid; self.focus(self.focus_owner); self.push_notice(pid as usize);
            }
            return;
        }
        if !self.live(target) { return; }
        let byte = if target == self.focus_owner { owner } else { app };
        let task = self.tasks[target].as_mut().unwrap(); task.input.push(byte);
        if matches!(task.state, State::Sleeping(_)) { task.state = State::Ready; }
    }

    fn mailbox(&self, slot: usize) -> *mut SyscallMailbox { unsafe { self.tasks[slot].as_ref().unwrap().abi.ptr().add(4096).cast() } }

    // Common exit path (exit, kill, exception): wakes clients waiting for a reply from the task.
    fn terminate(&mut self, slot: usize, notify: bool) {
        let task = self.tasks[slot].as_mut().unwrap(); let pid = task.pid; task.state = State::Exited; task.pending_cap = None;
        for other in 1..SLOTS { if self.tasks[other].as_ref().is_some_and(|t| t.state == State::BlockedReply(slot)) { self.fail_reply(other); } }
        if self.foreground == slot {
            // Output the focus owner has not read yet is kept until the next focused task exits.
            let console = core::mem::replace(&mut self.tasks[slot].as_mut().unwrap().console, Queue::new());
            self.exited_console = Some((pid, console));
            let owner = if self.live(self.focus_owner) { self.focus_owner } else { 0 };
            self.focus(owner);
            if notify { self.push_notice(pid as usize | NOTICE_EXITED); }
        }
        if self.focus_owner == slot { self.focus_owner = 0; if self.foreground == slot { self.focus(0); } }
    }
    fn fail_reply(&mut self, slot: usize) {
        let mailbox = self.mailbox(slot);
        unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!((*mailbox).result), ERR_PEER); }
        self.tasks[slot].as_mut().unwrap().state = State::Ready;
    }
    fn find(&self, pid: u64) -> Option<usize> { (1..SLOTS).find(|&i| { self.tasks[i].as_ref().is_some_and(|t| t.pid == pid && t.state != State::Exited) }) }
    fn free_slot(cspace: &[Option<Capability>; CAP_SLOTS]) -> Option<usize> { (SLOT_DYNAMIC..CAP_SLOTS).find(|&i| cspace[i].is_none()) }

    // Whether a physical range is still in use by someone else: via a capability, an in-flight send or a mapping.
    fn referenced(&self, physical: usize, size: usize) -> bool {
        self.tasks.iter().flatten().any(|t| t.cspace.iter().chain(core::iter::once(&t.pending_cap)).flatten().any(|c| c.overlaps(physical, size)) || t.heap.maps_foreign(physical, size))
    }
    fn retire(&mut self, region: Region) { if self.referenced(region.ptr() as usize, region.len()) { self.orphans.push(region); } }

    // Frees exited tasks only after their CPU has switched to a different CR3.
    fn reap(&mut self) {
        let mut released: Vec<Region> = Vec::new();
        for slot in 1..SLOTS {
            if self.current.contains(&slot) || !self.tasks[slot].as_ref().is_some_and(|t| t.state == State::Exited) { continue; }
            let mut task = self.tasks[slot].take().unwrap();
            released.extend(task.heap.take_regions()); released.extend(task.screen.take());
        }
        // The IRQ binding is removed once nobody owns the line capability anymore.
        for irq in 0..16 {
            if self.irq_bind[irq].is_some() && !self.tasks.iter().flatten().any(|t| t.state != State::Exited && t.cspace.contains(&Some(Capability::Interrupt(irq as u8)))) {
                self.irq_bind[irq] = None; self.irq_pending[irq] = false; unsafe { interrupts::set_irq_masked(irq as u8, true); }
            }
        }
        for region in released { self.retire(region); }
        let mut index = 0;
        while index < self.orphans.len() { let region = &self.orphans[index]; if self.referenced(region.ptr() as usize, region.len()) { index += 1; } else { self.orphans.swap_remove(index); } }
        if self.orphans.is_empty() && self.orphans.capacity() != 0 { self.orphans = Vec::new(); } // an empty list holds no heap memory
        let mut used = [false; ENDPOINTS]; used[..EP_RESERVED].fill(true);
        for task in self.tasks.iter().flatten() { for cap in task.cspace.iter().chain(core::iter::once(&task.pending_cap)).flatten() { if let Capability::Endpoint(id, _) = cap { used[*id] = true; } } }
        for ep in self.irq_bind.iter().flatten() { used[*ep] = true; }
        self.endpoints = used;
    }

    // Capability over a platform resource the kernel has validated (PLATFORM_CAP).
    fn platform_cap(&mut self, kind: usize, a: usize, b: usize) -> Result<Capability, usize> {
        let all = CAP_READ | CAP_WRITE | CAP_GRANT;
        match kind {
            PLATFORM_ENDPOINT if (1..EP_RESERVED).contains(&a) => Ok(Capability::Endpoint(a, all)),
            PLATFORM_PORTS => {
                let end = a.checked_add(b).ok_or(ERR_INVALID)?;
                if b == 0 || !LEGACY_PORTS.iter().any(|&(base, count)| a >= base as usize && end <= base as usize + count as usize) { return Err(ERR_RIGHTS); }
                Ok(Capability::IoPorts(a as u16, b as u16))
            }
            PLATFORM_IRQ if (1..16).contains(&a) && a != 2 => Ok(Capability::Interrupt(a as u8)),
            PLATFORM_DEVICE_BAR => {
                let device = *self.devices.get(a).ok_or(ERR_NOT_FOUND)?; let bar = *device.bars.get(b).ok_or(ERR_INVALID)?;
                if bar.size == 0 { return Err(ERR_NOT_FOUND); }
                unsafe { pci::enable(&device); }
                if bar.io { Ok(Capability::IoPorts(bar.base as u16, bar.size.min(0xFFFF) as u16)) } else { Ok(Capability::Mmio(bar.base as usize, (bar.size as usize).div_ceil(4096) * 4096)) }
            }
            PLATFORM_DEVICE_IRQ => match self.devices.get(a).ok_or(ERR_NOT_FOUND)?.irq { 0 | 2 => Err(ERR_NOT_FOUND), irq => Ok(Capability::Interrupt(irq)) },
            PLATFORM_FRAMEBUFFER => Ok(Capability::Memory(self.boot.fb_ptr as usize, frame_bytes(&self.boot))),
            PLATFORM_DMA => {
                // 64 KiB aligned so a driver's data buffer does not cross a DMA boundary.
                let bytes = a.checked_next_multiple_of(4096).filter(|&n| n > 0).ok_or(ERR_INVALID)?;
                if self.dma.iter().map(Region::len).sum::<usize>() + bytes > DMA_LIMIT { return Err(ERR_NO_MEMORY); }
                let region = Region::new(bytes, 64 * 1024).map_err(|_| ERR_NO_MEMORY)?;
                let cap = Capability::Dma(region.ptr() as usize, region.len()); self.dma.push(region); Ok(cap)
            }
            PLATFORM_PRIVILEGE => match a { CAP_KIND_INPUT => Ok(Capability::Input), CAP_KIND_DISPLAY => Ok(Capability::Display), CAP_KIND_SPAWN => Ok(Capability::Spawn), CAP_KIND_CONTROL => Ok(Capability::Control), _ => Err(ERR_INVALID) },
            _ => Err(ERR_INVALID),
        }
    }

    // New task from an ELF with the given capabilities; flags are SPAWN_SERVICE / SPAWN_SCREEN.
    fn spawn_internal(&mut self, source: Source, name: Name, args: &[u8], flags: usize, caps: [Option<Capability>; CAP_SLOTS]) -> Result<u64, &'static str> {
        let (service, has_screen) = (flags & SPAWN_SERVICE != 0, flags & SPAWN_SCREEN != 0);
        let live = |t: &&Task| t.state != State::Exited;
        if !service && self.tasks.iter().flatten().filter(live).filter(|t| !t.service).count() >= MAX_APPS { return Err("TASK LIMIT REACHED"); }
        let slot = (1..SLOTS).find(|&i| self.tasks[i].is_none()).ok_or("NO FREE TASK SLOT")?;
        let pid = self.next_pid; let next_pid = pid.checked_add(1).ok_or("PID SPACE EXHAUSTED")?;
        let file = match source { Source::Boot(index) => { let image = self.boot.programs.get(index).ok_or("UNKNOWN PROGRAM")?; if image.len == 0 { return Err("UNKNOWN PROGRAM"); } unsafe { core::slice::from_raw_parts(image.data, image.len) } } Source::Image(bytes) => bytes };
        let elf = elf::Image::parse(file)?;
        let mut image = Region::new(elf.size.div_ceil(4096) * 4096, 4096)?; let entry = elf.load(image.bytes_mut(), paging::USER_IMAGE)?;
        let mut space = paging::Space::new()?;
        for (offset, size, flags) in elf.segments() { space.map(paging::USER_IMAGE + offset, image.ptr() as usize + offset, size, flags & 2 != 0, flags & 1 != 0)?; }
        let stack = Region::new(STACK_SIZE, 4096)?; let abi = Region::new(8192, 4096)?;
        let screen = if has_screen { Some(Region::new(frame_bytes(&self.boot), 4096)?) } else { None };
        let mut info = self.boot; info.fb_ptr = if has_screen { paging::USER_SCREEN as *mut u32 } else { core::ptr::null_mut() }; info.heap_ptr = core::ptr::null_mut(); info.heap_len = 0; info.programs = [ProgramImage { data: core::ptr::null(), len: 0 }; BOOT_IMAGES]; info.ap_trampoline = 0; info.cpu_count = 0; info.apic_ids = [0; 8];
        unsafe { (abi.ptr() as *mut BootInfo).write(info); }
        let args = &args[..args.len().min(ARGS_MAX)];
        unsafe { let page = core::slice::from_raw_parts_mut(abi.ptr().add(ARGS_OFFSET), 2 + ARGS_MAX); page[..2].copy_from_slice(&(args.len() as u16).to_le_bytes()); page[2..2 + args.len()].copy_from_slice(args); }
        let exit = Region::new(4096, 4096)?; let code = unsafe { core::slice::from_raw_parts_mut(exit.ptr(), 21) }; code[0..2].copy_from_slice(&[0x48, 0xb8]); code[2..10].copy_from_slice(&(paging::USER_MAILBOX as u64).to_le_bytes()); code[10..21].copy_from_slice(&[0x48, 0xc7, 0x00, 7, 0, 0, 0, 0xcd, 0x80, 0x0f, 0x0b]); let user_sp = paging::USER_STACK + STACK_SIZE - 8; unsafe { ((stack.ptr() as usize + STACK_SIZE - 8) as *mut usize).write(paging::USER_EXIT); }
        space.map(paging::USER_STACK, stack.ptr() as usize, stack.len(), true, false)?;
        if let Some(screen) = &screen { space.map(paging::USER_SCREEN, screen.ptr() as usize, screen.len(), true, false)?; }
        space.map(paging::USER_INFO, abi.ptr() as usize, 4096, false, false)?; space.map(paging::USER_MAILBOX, abi.ptr() as usize + 4096, 4096, true, false)?; space.map(paging::USER_EXIT, exit.ptr() as usize, 4096, false, true)?;
        let context = Region::new(context::SIZE, 16)?; let sp = context.ptr() as usize; unsafe { context::initial(sp, entry, user_sp); }
        // Applications are balanced by per-CPU application count: sleeping services don't skew the balance.
        let cpu = (0..cpu::COUNT.load(Ordering::Acquire)).filter(|&i| cpu::ONLINE[i].load(Ordering::Acquire)).min_by_key(|&i| { self.tasks.iter().flatten().filter(|t| t.cpu == i && t.state != State::Exited && t.service == service).count() }).unwrap_or(0);
        self.tasks[slot] = Some(Task { pid, name, service, state: State::Ready, sp, cpu, space, heap: crate::user_heap::Heap::new(), context, _exit: exit, runs: 0, ticks: 0, calls: 0, _image: image, _stack: stack, screen, abi, input: Queue::new(), log: Queue::new(), console: Queue::new(), dirty: true, cspace: caps, pending_cap: None, pending_call: false, send_seq: 0, reply_to: None });
        self.next_pid = next_pid; Ok(pid)
    }

    fn cap(&self, slot: usize, index: usize) -> Option<Capability> { if index < CAP_SLOTS { self.tasks[slot].as_ref().unwrap().cspace[index] } else { None } }
    // Copy of a capability for transfer; IPC endpoint rights are narrowed by the sender's mask.
    fn transfer(&self, slot: usize, index: usize, mask: usize) -> Option<Capability> {
        if index == 0 { return None; }
        match self.cap(slot, index)? { Capability::Endpoint(id, rights) => Some(Capability::Endpoint(id, rights & mask as u8)), Capability::Reply(..) => None, other => Some(other) } // reply caps are one-shot and not transferable
    }
    fn blocked(&self, state: State) -> Option<usize> { (1..SLOTS).find(|&i| self.tasks[i].as_ref().is_some_and(|t| t.state == state)) }
    // Someone alive can still receive on the endpoint (otherwise a send would wait forever).
    fn receivable(&self, ep: usize) -> bool {
        self.tasks.iter().flatten().any(|t| t.state != State::Exited && t.cspace.iter().flatten().any(|c| matches!(c, Capability::Endpoint(id, rights) if *id == ep && rights & CAP_READ != 0))) || self.irq_bind.contains(&Some(ep))
    }

    // Delivers the message of a blocked or current sender to the receiver.
    unsafe fn deliver(&mut self, from: usize, to: usize) {
        let (from_mb, to_mb) = (self.mailbox(from), self.mailbox(to));
        let sender = self.tasks[from].as_mut().unwrap(); let (pid, call, cap) = (sender.pid, sender.pending_call, sender.pending_cap.take());
        (*to_mb).msg[2] = (*from_mb).msg[2]; (*to_mb).msg[3] = (*from_mb).msg[3]; (*to_mb).arg1 = pid as usize; (*to_mb).msg[1] = if call { MSG_FLAG_CALL } else { 0 };
        let receive = (*to_mb).arg2; let delivered = cap.is_some() && (1..CAP_SLOTS).contains(&receive);
        let receiver = self.tasks[to].as_mut().unwrap();
        if delivered { receiver.cspace[receive] = cap; }
        (*to_mb).msg[0] = delivered as usize; (*to_mb).result = 0; receiver.state = State::Ready;
        let previous = if call { receiver.reply_to.replace((from, pid)) } else { None };
        if let Some((old, old_pid)) = previous { if self.tasks[old].as_ref().is_some_and(|t| t.pid == old_pid && t.state == State::BlockedReply(to)) { self.fail_reply(old); } }
        let sender = self.tasks[from].as_mut().unwrap();
        if call { sender.state = State::BlockedReply(to); } else { (*from_mb).result = 0; sender.state = State::Ready; }
    }
    // IRQ notification from the kernel: sender PID 0, no capability.
    unsafe fn notify_irq(&mut self, to: usize, irq: usize) {
        let mb = self.mailbox(to);
        (*mb).msg = [0, MSG_FLAG_IRQ, irq, 0]; (*mb).arg1 = 0; (*mb).result = 0;
        self.tasks[to].as_mut().unwrap().state = State::Ready;
    }

    // Line interrupt: already masked; wakes the driver or records the event.
    unsafe fn raise_irq(&mut self, irq: usize) {
        if let Some(ep) = self.irq_bind[irq] {
            if let Some(receiver) = self.blocked(State::BlockedRecv(ep)) { self.notify_irq(receiver, irq); } else { self.irq_pending[irq] = true; }
            return;
        }
        let mut woken = false;
        for task in self.tasks.iter_mut().flatten() { if task.state == State::BlockedIrq(irq as u8) { task.state = State::Ready; woken = true; } }
        if !woken { self.irq_pending[irq] = true; }
    }

    // Ok(Some(sp)): caller is blocked, switch; Ok(None): return 0 immediately.
    unsafe fn ipc_send(&mut self, slot: usize, sp: usize, cpu: usize, request: &SyscallMailbox, call: bool) -> Result<Option<usize>, usize> {
        let Some(Capability::Endpoint(ep, rights)) = self.cap(slot, request.arg1) else { return Err(ERR_INVALID); };
        if rights & CAP_WRITE == 0 { return Err(ERR_RIGHTS); }
        let receiver = self.blocked(State::BlockedRecv(ep));
        if receiver.is_none() && !self.receivable(ep) { return Err(ERR_PEER); }
        let cap = if rights & CAP_GRANT != 0 { self.transfer(slot, request.msg[0], request.msg[1]) } else { None };
        self.send_seq += 1; let seq = self.send_seq;
        let task = self.tasks[slot].as_mut().unwrap(); task.pending_cap = cap; task.pending_call = call; task.send_seq = seq;
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
        let Some(Capability::Endpoint(ep, rights)) = self.cap(slot, request.arg1) else { return Err(ERR_INVALID); };
        if rights & CAP_READ == 0 { return Err(ERR_RIGHTS); }
        if let Some(irq) = (0..16).find(|&i| self.irq_bind[i] == Some(ep) && self.irq_pending[i]) { self.irq_pending[irq] = false; self.notify_irq(slot, irq); return Ok(None); }
        let sender = (1..SLOTS).filter(|&i| self.tasks[i].as_ref().is_some_and(|t| t.state == State::BlockedSend(ep))).min_by_key(|&i| self.tasks[i].as_ref().unwrap().send_seq);
        if let Some(sender) = sender { self.deliver(sender, slot); return Ok(None); }
        let task = self.tasks[slot].as_mut().unwrap(); task.state = State::BlockedRecv(ep); task.dirty = true;
        Ok(Some(self.select(sp, cpu)))
    }
    // Reply to the last client or to the client from a saved reply capability (arg1 is its slot).
    unsafe fn ipc_reply(&mut self, slot: usize, request: &SyscallMailbox) -> Result<usize, usize> {
        let target = if request.arg1 == 0 { self.tasks[slot].as_mut().unwrap().reply_to.take() } else {
            match self.cap(slot, request.arg1) { Some(Capability::Reply(caller, pid)) => { self.tasks[slot].as_mut().unwrap().cspace[request.arg1] = None; Some((caller, pid)) } _ => None }
        };
        let Some((caller, pid)) = target else { return Err(ERR_INVALID); };
        if !self.tasks[caller].as_ref().is_some_and(|t| t.pid == pid && t.state == State::BlockedReply(slot)) { return Err(ERR_PEER); }
        let cap = self.transfer(slot, request.msg[0], request.msg[1]); let mb = self.mailbox(caller);
        (*mb).msg[2] = request.msg[2]; (*mb).msg[3] = request.msg[3]; (*mb).arg1 = self.tasks[slot].as_ref().unwrap().pid as usize;
        let receive = (*mb).arg2; let delivered = cap.is_some() && (1..CAP_SLOTS).contains(&receive);
        let task = self.tasks[caller].as_mut().unwrap();
        if delivered { task.cspace[receive] = cap; }
        (*mb).msg[0] = delivered as usize; (*mb).msg[1] = 0; (*mb).result = 0; task.state = State::Ready;
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
        let platform = self.holds(slot, Capability::Platform);
        let task = self.tasks[slot].as_ref().unwrap();
        let length = request.arg2; let (count, flags) = (request.msg[3] & 0xFF, request.msg[3] >> 8);
        if length == 0 || length > NAME_MAX + 1 + ARGS_MAX || count > SPAWN_GRANTS_MAX || !task.space.validate_read(request.arg1, length) { return Err(ERR_INVALID); }
        if flags & SPAWN_SERVICE != 0 && !platform { return Err(ERR_RIGHTS); }
        let grant_bytes = count * core::mem::size_of::<Grant>();
        if count > 0 && !task.space.validate_read(request.msg[2], grant_bytes) { return Err(ERR_INVALID); }
        // `name\0arguments`
        let mut text = [0u8; NAME_MAX + 1 + ARGS_MAX];
        for (i, byte) in text[..length].iter_mut().enumerate() { *byte = core::ptr::read_volatile(task.space.readable(request.arg1 + i).unwrap() as *const u8); }
        let name_len = text[..length].iter().position(|&b| b == 0).unwrap_or(length);
        if name_len == 0 || name_len > NAME_MAX { return Err(ERR_INVALID); }
        let args = if name_len < length { &text[name_len + 1..length] } else { &[][..] };
        let mut raw = [0u8; SPAWN_GRANTS_MAX * 4];
        for (i, byte) in raw[..grant_bytes].iter_mut().enumerate() { *byte = core::ptr::read_volatile(task.space.readable(request.msg[2] + i).unwrap() as *const u8); }
        let mut caps = [None; CAP_SLOTS];
        for grant in raw[..grant_bytes].chunks(4) {
            let (child, own, rights) = (grant[0] as usize, grant[1] as usize, grant[2] as usize);
            if !(1..CAP_SLOTS).contains(&child) { return Err(ERR_INVALID); }
            caps[child] = Some(self.transfer(slot, own, rights).ok_or(ERR_INVALID)?);
        }
        let source = if request.msg[0] & SPAWN_BOOT != 0 {
            if !platform { return Err(ERR_RIGHTS); }
            Source::Boot(request.msg[0] & !SPAWN_BOOT)
        } else {
            match self.cap(slot, request.msg[0]) {
                Some(Capability::Memory(physical, size)) if request.msg[1] <= size => Source::Image(core::slice::from_raw_parts(physical as *const u8, request.msg[1])),
                _ => return Err(ERR_INVALID),
            }
        };
        self.spawn_internal(source, Name::new(&text[..name_len]), args, flags, caps).map(|pid| pid as usize).map_err(spawn_error)
    }

    // Process control (TASK_LIST ... HALT): only for the holder of the control capability.
    unsafe fn control(&mut self, slot: usize, ptr: *mut SyscallMailbox, request: &SyscallMailbox) -> Result<usize, usize> {
        if !self.holds(slot, Capability::Control) { return Err(ERR_RIGHTS); }
        let task_slot = |s: &Self, pid: usize| if pid == 0 { Some(slot) } else { s.find(pid as u64) };
        match request.syscall_num {
            SYSCALL_TASK_LIST => {
                let mut count = 0;
                for (index, task) in self.tasks.iter().enumerate().skip(1) {
                    let Some(task) = task else { continue };
                    if count >= request.arg2 { break; }
                    let mut info = TaskInfo { pid: task.pid, name: [0; NAME_MAX], state: [b' '; 8], cpu: task.cpu as u32, focus: (self.foreground == index) as u8, service: task.service as u8, screen: task.screen.is_some() as u8, reserved: 0, runs: task.runs, ticks: task.ticks, calls: task.calls };
                    info.name[..task.name.len as usize].copy_from_slice(&task.name.bytes[..task.name.len as usize]);
                    let label = if self.current.contains(&index) { "RUNNING" } else { task.state.label() }; info.state[..label.len()].copy_from_slice(label.as_bytes());
                    let bytes = core::slice::from_raw_parts((&info as *const TaskInfo).cast::<u8>(), core::mem::size_of::<TaskInfo>());
                    if !copy_out(&self.tasks[slot].as_ref().unwrap().space, request.arg1 + count * bytes.len(), bytes) { return Err(ERR_INVALID); }
                    count += 1;
                }
                Ok(count)
            }
            SYSCALL_TASK_KILL => { let target = self.find(request.arg1 as u64).ok_or(ERR_NOT_FOUND)?; self.terminate(target, false); Ok(0) }
            SYSCALL_FOCUS => {
                let target = task_slot(self, request.arg1).ok_or(ERR_NOT_FOUND)?;
                if self.tasks[target].as_ref().unwrap().screen.is_none() { return Err(ERR_INVALID); }
                if request.arg2 == 0 { self.tasks[target].as_mut().unwrap().console.clear(); }
                self.focus_owner = slot; self.focus(target); Ok(self.tasks[target].as_ref().unwrap().pid as usize)
            }
            SYSCALL_TASK_LOGS | SYSCALL_CONSOLE_READ => {
                let (address, capacity) = (request.msg[0], request.msg[1].min(4096));
                let console = request.syscall_num == SYSCALL_CONSOLE_READ;
                let queue = match self.find(request.arg1 as u64) {
                    Some(target) => { let t = self.tasks[target].as_mut().unwrap(); if console { &mut t.console } else { &mut t.log } }
                    None => match self.exited_console.as_mut() { Some((pid, queue)) if console && *pid == request.arg1 as u64 => queue, _ => return Err(ERR_NOT_FOUND) },
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
            _ => Err(ERR_INVALID),
        }
    }

    unsafe fn syscall(&mut self, slot: usize, sp: usize, cpu: usize) -> usize {
        let ptr = self.mailbox(slot); let request = core::ptr::read_volatile(ptr);
        let tasks = self.tasks.as_mut_ptr(); let task = (*tasks.add(slot)).as_mut().unwrap(); task.calls += 1;
        let result: Result<usize, usize> = match request.syscall_num {
            SYSCALL_RDTSC => { let lo: u32; let hi: u32; asm!("rdtsc", out("eax") lo, out("edx") hi); Ok((((hi as u64) << 32) | lo as u64) as usize) }
            SYSCALL_READ_KEY => Ok(task.input.pop().unwrap_or(0) as usize),
            SYSCALL_LOG => {
                // Kept twice: LOGS drains `log`, the focus owner mirrors `console` of the focused task.
                let length = request.arg2.min(4096);
                if !task.space.validate_read(request.arg1, length) { Err(ERR_INVALID) } else {
                    for i in 0..length {
                        let physical = task.space.readable(request.arg1 + i).unwrap(); let byte = core::ptr::read_volatile(physical as *const u8);
                        task.log.push(byte); task.console.push(byte);
                    }
                    Ok(length)
                }
            }
            SYSCALL_UPTIME => Ok(interrupts::milliseconds() as usize),
            SYSCALL_ALLOC => Ok(task.heap.allocate(&mut task.space, request.arg1).unwrap_or(0)),
            SYSCALL_FREE => match task.heap.free(&mut task.space, request.arg1) { None => Err(ERR_INVALID), Some(region) => { if let Some(region) = region { self.retire(region); } Ok(0) } },
            SYSCALL_WAIT => {
                let now = interrupts::milliseconds(); let duration = request.arg1.min(60_000).div_ceil(10).max(1) as u64 * 10;
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).result), now as usize);
                task.state = if task.input.is_empty() { State::Sleeping(now.wrapping_add(duration)) } else { State::Ready };
                task.dirty = true; return self.select(sp, cpu);
            }
            SYSCALL_EXIT => { self.terminate(slot, true); return self.select(sp, cpu); }
            SYSCALL_ENDPOINT_CREATE => match ((EP_RESERVED..ENDPOINTS).find(|&e| !self.endpoints[e]), Self::free_slot(&task.cspace)) {
                (Some(ep), Some(index)) => { self.endpoints[ep] = true; task.cspace[index] = Some(Capability::Endpoint(ep, CAP_READ | CAP_WRITE | CAP_GRANT)); Ok(index) }
                _ => Err(ERR_NO_SLOT),
            },
            SYSCALL_CAP_DROP => if (1..CAP_SLOTS).contains(&request.arg1) { task.cspace[request.arg1] = None; Ok(0) } else { Err(ERR_INVALID) },
            SYSCALL_SPAWN => self.spawn(slot, &request),
            SYSCALL_PLATFORM_CAP => {
                if !self.holds(slot, Capability::Platform) { Err(ERR_RIGHTS) } else {
                    match (Self::free_slot(&task.cspace), self.platform_cap(request.arg1, request.arg2, request.msg[0])) {
                        (Some(index), Ok(cap)) => { (*tasks.add(slot)).as_mut().unwrap().cspace[index] = Some(cap); Ok(index) }
                        (None, Ok(_)) => Err(ERR_NO_SLOT),
                        (_, Err(error)) => Err(error),
                    }
                }
            }
            SYSCALL_DEVICE_FIND => {
                if !self.holds(slot, Capability::Platform) { Err(ERR_RIGHTS) } else {
                    let (class, mask) = (request.arg1 as u32, request.arg2 as u32);
                    self.devices.iter().enumerate().filter(|(_, d)| d.class & mask == class & mask).nth(request.msg[0]).map(|(index, _)| index).ok_or(ERR_NOT_FOUND)
                }
            }
            SYSCALL_MEM_SHARE => match (task.heap.shareable(request.arg1, request.arg2), Self::free_slot(&task.cspace)) {
                (Some((physical, size)), Some(index)) => { task.cspace[index] = Some(Capability::Memory(physical, size)); Ok(index) }
                (None, _) => Err(ERR_INVALID),
                _ => Err(ERR_NO_SLOT),
            },
            SYSCALL_MEM_MAP => match self.cap(slot, request.arg1) {
                Some(cap @ (Capability::Memory(physical, size) | Capability::Dma(physical, size) | Capability::Mmio(physical, size))) => {
                    core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).arg2), size); // mapping size for the client
                    task.heap.map_shared(&mut task.space, physical, size, matches!(cap, Capability::Mmio(..))).ok_or(ERR_NO_MEMORY)
                }
                _ => Err(ERR_RIGHTS),
            },
            SYSCALL_MEM_PHYS => match self.cap(slot, request.arg1) { Some(Capability::Dma(physical, _)) => Ok(physical), _ => Err(ERR_RIGHTS) },
            SYSCALL_PORT_IN => {
                let (port, width) = (request.arg2, request.msg[1].max(1));
                if !matches!(width, 1 | 2 | 4) || !self.ports(slot, request.arg1, port, width) { Err(ERR_RIGHTS) } else { Ok(port_in(port as u16, width)) }
            }
            SYSCALL_PORT_OUT => {
                let (port, width) = (request.arg2, request.msg[1].max(1));
                if !matches!(width, 1 | 2 | 4) || !self.ports(slot, request.arg1, port, width) { Err(ERR_RIGHTS) } else { port_out(port as u16, width, request.msg[0]); Ok(0) }
            }
            SYSCALL_PORT_IN_BLOCK => {
                // Reads 16-bit words (ATA sector) straight into the process buffer, without a syscall per word.
                let (buffer, words) = (request.msg[2], request.msg[3]);
                let pages_ok = words > 0 && words <= 2048 && buffer % 2 == 0 && buffer.checked_add(words * 2).is_some() && (buffer / 4096..=(buffer + words * 2 - 1) / 4096).all(|page| task.space.writable(page * 4096).is_some());
                if !pages_ok || !self.ports(slot, request.arg1, request.arg2, 2) { Err(ERR_RIGHTS) } else {
                    for i in 0..words { let target = task.space.writable(buffer + i * 2).unwrap(); core::ptr::write_volatile(target as *mut u16, port_in(request.arg2 as u16, 2) as u16); }
                    Ok(words)
                }
            }
            SYSCALL_IRQ_WAIT => match self.cap(slot, request.arg1) {
                Some(Capability::Interrupt(irq)) if self.irq_bind[irq as usize].is_none() => {
                    interrupts::set_irq_masked(irq, false);
                    if core::mem::take(&mut self.irq_pending[irq as usize]) { Ok(0) } else {
                        core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).result), 0);
                        task.state = State::BlockedIrq(irq); return self.select(sp, cpu);
                    }
                }
                _ => Err(ERR_RIGHTS),
            },
            SYSCALL_IRQ_BIND => match (self.cap(slot, request.arg1), self.cap(slot, request.arg2)) {
                (Some(Capability::Interrupt(irq)), Some(Capability::Endpoint(ep, rights))) if rights & CAP_READ != 0 => { self.irq_bind[irq as usize] = Some(ep); interrupts::set_irq_masked(irq, false); Ok(0) }
                _ => Err(ERR_RIGHTS),
            },
            SYSCALL_IRQ_ACK => match self.cap(slot, request.arg1) { Some(Capability::Interrupt(irq)) => { interrupts::set_irq_masked(irq, false); Ok(0) } _ => Err(ERR_RIGHTS) },
            SYSCALL_INPUT_EVENT => {
                // Only a holder of the input capability (keyboard driver, shell for the UART) may inject input.
                if !self.holds(slot, Capability::Input) { Err(ERR_RIGHTS) } else { self.route_key(request.arg1 as u8, request.arg2 as u8, request.msg[0] != 0); Ok(0) }
            }
            SYSCALL_COMPOSITOR_PULL => {
                if !self.holds(slot, Capability::Display) || !(1..CAP_SLOTS).contains(&request.arg1) { Err(ERR_RIGHTS) } else {
                    let focused = self.foreground;
                    match (*tasks.add(focused)).as_mut().filter(|_| focused != 0).filter(|t| t.screen.is_some()) {
                        None => Ok(0),
                        Some(t) => {
                            let (source, dirty) = (t.screen.as_ref().unwrap().ptr() as usize, core::mem::take(&mut t.dirty) | core::mem::take(&mut self.dirty));
                            // New capability only on screen change: the compositor keeps the mapping across frames.
                            if source != self.composited { self.composited = source; task.cspace[request.arg1] = Some(Capability::Memory(source, frame_bytes(&self.boot))); Ok(2) } else { Ok(dirty as usize) }
                        }
                    }
                }
            }
            SYSCALL_CAP_INFO => {
                // Lets a driver learn what it was granted (e.g. the BAR port base) without seeing physical memory addresses.
                let (kind, base, size) = match self.cap(slot, request.arg1) {
                    None => (CAP_KIND_NONE, 0, 0),
                    Some(Capability::Endpoint(_, rights)) => (CAP_KIND_ENDPOINT, 0, rights as usize),
                    Some(Capability::Memory(_, size)) => (CAP_KIND_MEMORY, 0, size),
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
                };
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).arg2), base); core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).msg[2]), size);
                Ok(kind)
            }
            SYSCALL_TASK_ALIVE => Ok(self.find(request.arg1 as u64).is_some() as usize),
            SYSCALL_IPC_SEND | SYSCALL_IPC_CALL | SYSCALL_IPC_RECV => {
                let outcome = if request.syscall_num == SYSCALL_IPC_RECV { self.ipc_recv(slot, sp, cpu, &request) } else { self.ipc_send(slot, sp, cpu, &request, request.syscall_num == SYSCALL_IPC_CALL) };
                match outcome { Ok(Some(next)) => return next, Ok(None) => Ok(0), Err(error) => Err(error) }
            }
            SYSCALL_IPC_REPLY => self.ipc_reply(slot, &request),
            SYSCALL_IPC_SAVE_REPLY => match (task.reply_to, Self::free_slot(&task.cspace)) {
                // Deferred reply: the server accepts further requests and replies to this client later.
                (Some((caller, pid)), Some(index)) => { task.reply_to = None; task.cspace[index] = Some(Capability::Reply(caller, pid)); Ok(index) }
                (None, _) => Err(ERR_INVALID),
                _ => Err(ERR_NO_SLOT),
            },
            SYSCALL_TASK_LIST | SYSCALL_TASK_KILL | SYSCALL_FOCUS | SYSCALL_TASK_LOGS | SYSCALL_CONSOLE_READ | SYSCALL_NOTICE | SYSCALL_FAULTS | SYSCALL_CPU_INFO | SYSCALL_KERNEL_HEAP | SYSCALL_HALT => {
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

unsafe fn port_in(port: u16, width: usize) -> usize {
    match width { 1 => { let v: u8; asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack)); v as usize } 2 => { let v: u16; asm!("in ax, dx", out("ax") v, in("dx") port, options(nomem, nostack)); v as usize } _ => { let v: u32; asm!("in eax, dx", out("eax") v, in("dx") port, options(nomem, nostack)); v as usize } }
}
unsafe fn port_out(port: u16, width: usize, value: usize) {
    match width { 1 => asm!("out dx, al", in("dx") port, in("al") value as u8, options(nomem, nostack)), 2 => asm!("out dx, ax", in("dx") port, in("ax") value as u16, options(nomem, nostack)), _ => asm!("out dx, eax", in("dx") port, in("eax") value as u32, options(nomem, nostack)) }
}

// Bootstrap authority (MC-3.12): the kernel starts only boot image 0 (`init`) with its endpoint, the platform and
// spawn privileges; everything else is distributed by init.
pub fn spawn_init() -> Result<u64, &'static str> {
    locked(|| unsafe {
        let mut caps = [None; CAP_SLOTS];
        caps[SLOT_SERVICE] = Some(Capability::Endpoint(EP_INIT, CAP_READ | CAP_WRITE | CAP_GRANT));
        caps[SLOT_DEV0] = Some(Capability::Platform); caps[SLOT_DEV1] = Some(Capability::Spawn);
        scheduler().spawn_internal(Source::Boot(0), Name::new(BOOT_SERVICES[0].as_bytes()), &[], SPAWN_SERVICE, caps)
    })
}

pub extern "C" fn interrupt(sp: usize) -> usize {
    unsafe {
        let registers = context::registers(sp); let vector = registers[15]; let cpu = cpu::id();
        if vector == 0x31 { asm!("cli"); loop { asm!("hlt"); } }
        if vector < 32 && registers[18] & 3 == 0 { for &b in b"KERNEL EXCEPTION VECTOR=" { serial_write_byte(b); } serial_number(vector); for &b in b" RIP=" { serial_write_byte(b); } serial_hex(registers[17]); for &b in b" ERROR=" { serial_write_byte(b); } serial_hex(registers[16]); for &b in b"\r\n" { serial_write_byte(b); } cpu::halt_all(); }
        let irq = (33..48).contains(&vector).then_some(vector as usize - 32);
        if vector == 32 { interrupts::advance(); outb(0x20, 0x20); cpu::eoi(); cpu::tick_others(); }
        else if let Some(irq) = irq { interrupts::set_irq_masked(irq as u8, true); if irq >= 8 { outb(0xA0, 0x20); } outb(0x20, 0x20); cpu::eoi(); } // the driver will unmask the line
        else if vector == 48 || vector == 50 { cpu::eoi(); }

        locked(|| {
            let s = scheduler(); let slot = s.current[cpu];
            let next = (|| {
            if let Some(irq) = irq { s.raise_irq(irq); return s.select(sp, cpu); }
            if vector == 32 || vector == 48 {
                cpu::TICKS[cpu].fetch_add(1, Ordering::Relaxed); let now = interrupts::milliseconds(); for task in s.tasks.iter_mut().flatten() { task.state.wake(now); }
                if slot == 0 && cpu == 0 { return sp; } if slot != 0 { let t = s.tasks[slot].as_mut().unwrap(); t.ticks += 1; t.dirty = true; }
                return s.select(sp, cpu);
            }
            if vector == 50 { return if slot == 0 { s.select(sp, cpu) } else { sp }; }
            if vector < 32 {
                let mut address = 0u64; if vector == 14 { asm!("mov {}, cr2", out(reg) address); }
                let pid = s.tasks[slot].as_ref().unwrap().pid; let at = s.fault_cursor % s.faults.len();
                s.faults[at] = Some(FaultInfo { pid, cpu: cpu as u64, vector, error: registers[16], rip: registers[17], address });
                s.fault_cursor += 1; s.terminate(slot, true); return s.select(sp, cpu);
            }
            if slot == 0 { return s.select(sp, cpu); }
            if s.tasks[slot].as_ref().unwrap().state == State::Exited { return s.select(sp, cpu); }
            s.syscall(slot, sp, cpu)
            })();
            s.wake_idle(cpu);
            next
        })
    }
}

unsafe fn serial_hex(number: u64) { for shift in (0..16).rev() { let digit = ((number >> (shift * 4)) & 15) as u8; serial_write_byte(if digit < 10 { b'0' + digit } else { b'A' + digit - 10 }); } }
unsafe fn serial_number(mut number: u64) { let mut buffer = [0; 20]; let mut at = buffer.len(); loop { at -= 1; buffer[at] = b'0' + (number % 10) as u8; number /= 10; if number == 0 { break; } } for &byte in &buffer[at..] { serial_write_byte(byte); } }
// Called from the BSP idle loop: reclaims memory of exited tasks.
pub fn reap() { locked(|| unsafe { scheduler().reap() }) }

pub fn idle() { interrupts::without(|| unsafe { let ready = locked(|| { scheduler().states(cpu::id())[1..].iter().any(|s| *s == State::Ready) }); if ready { asm!("int 0x80"); } else { asm!("sti", "hlt", "cli"); } }); }
