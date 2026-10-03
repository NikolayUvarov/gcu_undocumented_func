use crate::abi::*;
use crate::input::{Keyboard, Queue};
use crate::memory::Region;
use crate::task_state::{self, State};
use crate::{context, cpu, elf, interrupts, outb, paging, pci, serial_write_byte};
use alloc::vec::Vec;
use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

pub const MAX_TASKS: usize = 20; // сервисы + приложения
pub const MAX_APPS: usize = 8; // сервисы не отнимают слоты у пользовательских программ
const SLOTS: usize = MAX_TASKS + 1;
const STACK_SIZE: usize = 64 * 1024;
const ENDPOINTS: usize = 64;
const AUDIO_DMA_BYTES: usize = 33 * 4096; // 32 буфера PCM + список дескрипторов AC97
const AHCI_DMA_BYTES: usize = 128 * 1024; // команды, FIS и буфер данных 64 КиБ
const XHCI_DMA_BYTES: usize = 256 * 1024; // кольца, контексты, scratchpad и буфер данных 64 КиБ

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Capability { Endpoint(usize, u8), Memory(usize, usize), Dma(usize, usize), Mmio(usize, usize), IoPorts(u16, u16), Interrupt(u8), Input, Display, Spawn }

// Имя задачи (для ps и запросов запуска); образы приложений больше не индексируются таблицей ядра.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Name { bytes: [u8; NAME_MAX], len: u8 }
impl Name {
    pub fn new(text: &[u8]) -> Self { let len = text.len().min(NAME_MAX); let mut bytes = [0; NAME_MAX]; bytes[..len].copy_from_slice(&text[..len]); Self { bytes, len: len as u8 } }
    pub fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("?") }
}

// Откуда берётся ELF: образ сервиса от загрузчика UEFI или буфер, переданный сервисом loader.
enum Source<'a> { Boot(usize), Image(&'a [u8]) }

// Запрос шелла к loader: запуск или список программ; итог забирает шелл.
#[derive(Clone, Copy)]
struct Request { id: usize, background: bool, spawned: Option<Result<u64, &'static str>>, done: Option<usize> }

impl Capability {
    fn overlaps(self, physical: usize, size: usize) -> bool {
        match self { Self::Memory(p, s) | Self::Dma(p, s) => p < physical + size && physical < p + s, _ => false }
    }
}

pub fn heap_test() -> (usize, usize, bool) { locked(|| { let before = crate::ALLOCATOR.lock().used(); let test = alloc::format!("Dynamic allocation test at {} ms", interrupts::milliseconds()); core::hint::black_box(&test); drop(test); let heap = crate::ALLOCATOR.lock(); (heap.used(), heap.free(), heap.used() == before) }) }

struct Task {
    pid: u64, name: Name, service: bool, state: State, sp: usize, cpu: usize,
    space: paging::Space, heap: crate::user_heap::Heap, context: Region, _exit: Region,
    runs: u64, ticks: u64, calls: u64, _image: Region, _stack: Region, screen: Option<Region>, abi: Region,
    input: Queue<128>, log: Queue<4096>, log_line_start: bool, dirty: bool,
    cspace: [Option<Capability>; CAP_SLOTS],
    pending_cap: Option<Capability>, pending_call: bool, send_seq: u64, // отправка, ждущая получателя
    reply_to: Option<(usize, u64)>, // слот и PID клиента, ждущего ответа
}
struct Scheduler {
    boot: BootInfo, tasks: [Option<Task>; SLOTS], current: [usize; cpu::MAX], idle_sp: [usize; cpu::MAX],
    faults: [Option<Fault>; 16], fault_cursor: usize, next_pid: u64, foreground: usize, keyboard: Keyboard,
    shell_input: Queue<128>, shell_screen: Region, dirty: bool, notice: Option<(u64, bool)>,
    endpoints: [bool; ENDPOINTS], irq_bind: [Option<usize>; 16], irq_pending: [bool; 16], send_seq: u64,
    orphans: Vec<Region>, // освобождённая владельцем память, которую ещё отображают или держат мандатом
    ac97: Option<pci::Ac97>, ahci: Option<pci::Device>, xhci: Option<pci::Device>,
    dma: Vec<(&'static str, Region)>, // DMA-области драйверов переживают их перезапуск
    composited: usize, // экран, на который у композитора уже есть мандат
    request: Option<Request>, request_page: Region, next_request: usize, loader_notify: bool,
}

static mut SCHEDULER: Option<Scheduler> = None;
static LOCK: AtomicBool = AtomicBool::new(false);
struct Guard; impl Drop for Guard { fn drop(&mut self) { LOCK.store(false, Ordering::Release); } }
fn locked<T>(f: impl FnOnce() -> T) -> T { interrupts::without(|| { while LOCK.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() { core::hint::spin_loop(); } let _guard = Guard; f() }) }
#[derive(Clone, Copy)] pub struct Fault { pub pid: u64, pub cpu: usize, pub vector: u64, pub error: u64, pub rip: u64, pub address: u64 }

unsafe fn scheduler() -> &'static mut Scheduler { (*core::ptr::addr_of_mut!(SCHEDULER)).as_mut().unwrap() }

pub fn service_index(name: &[u8]) -> Option<usize> { BOOT_SERVICES.iter().position(|p| p.as_bytes().eq_ignore_ascii_case(name)) }
fn frame_bytes(info: &BootInfo) -> usize { (info.stride * info.height * 4).div_ceil(4096) * 4096 }

pub fn init(info: &BootInfo) -> Result<*mut u32, &'static str> {
    let bytes = info.stride.checked_mul(info.height).and_then(|n| n.checked_mul(4)).ok_or("FRAMEBUFFER SIZE OVERFLOW")?;
    let shell_screen = Region::new(bytes.div_ceil(4096) * 4096, 4096)?; let fb = shell_screen.ptr().cast();
    let ac97 = unsafe { pci::find_ac97() };
    let ahci = unsafe { pci::find(0x01_06_01, 0xFF_FF_FF) }.filter(|d| !d.bars[5].io && d.bars[5].size != 0);
    let xhci = unsafe { pci::find(0x0C_03_30, 0xFF_FF_FF) }.filter(|d| !d.bars[0].io && d.bars[0].size != 0);
    let mut endpoints = [false; ENDPOINTS]; endpoints[..EP_RESERVED].fill(true);
    unsafe { *core::ptr::addr_of_mut!(SCHEDULER) = Some(Scheduler { boot: *info, tasks: core::array::from_fn(|_| None), current: [0; cpu::MAX], idle_sp: [0; cpu::MAX], faults: [None; 16], fault_cursor: 0, next_pid: 1, foreground: 0, keyboard: Keyboard::new(), shell_input: Queue::new(), shell_screen, dirty: true, notice: None, endpoints, irq_bind: [None; 16], irq_pending: [false; 16], send_seq: 0, orphans: Vec::new(), ac97, ahci, xhci, dma: Vec::new(), composited: 0, request: None, request_page: Region::new(4096, 4096)?, next_request: 1, loader_notify: false }); }
    Ok(fb)
}

impl Scheduler {
    fn states(&self, cpu: usize) -> [State; SLOTS] { core::array::from_fn(|i| { if i == 0 { State::Ready } else { self.tasks[i].as_ref().filter(|t| t.cpu == cpu).map_or(State::Empty, |t| t.state) } }) }
    fn select(&mut self, sp: usize, cpu: usize) -> usize {
        let current = self.current[cpu];
        if current == 0 { self.idle_sp[cpu] = sp; } else { let task = self.tasks[current].as_mut().unwrap(); unsafe { context::save(sp, task.context.ptr() as usize); } }
        let next = task_state::next(&self.states(cpu), current); self.current[cpu] = next;
        if next == 0 { unsafe { paging::activate(paging::kernel_root()); } self.idle_sp[cpu] } else { let task = self.tasks[next].as_mut().unwrap(); task.runs += 1; unsafe { paging::activate(task.space.root()); } task.sp }
    }
    fn focus(&mut self, slot: usize) { if self.foreground != 0 { if let Some(task) = self.tasks[self.foreground].as_mut() { task.input.clear(); } } self.shell_input.clear(); if slot != 0 { self.tasks[slot].as_mut().unwrap().input.clear(); } self.foreground = slot; self.dirty = true; }
    fn route_key(&mut self, app: u8, shell: u8, background: bool) {
        if background && self.foreground != 0 { let pid = self.tasks[self.foreground].as_ref().unwrap().pid; self.focus(0); self.notice = Some((pid, false)); }
        else if self.foreground == 0 { if !background { self.shell_input.push(shell); } }
        else if app != 0 { let task = self.tasks[self.foreground].as_mut().unwrap(); task.input.push(app); if matches!(task.state, State::Sleeping(_)) { task.state = State::Ready; } }
    }
    fn poll_serial(&mut self) { for _ in 0..32 { let Some(key) = self.keyboard.read_serial() else { break; }; self.route_key(key.app, key.shell, key.background); } }

    fn mailbox(&self, slot: usize) -> *mut SyscallMailbox { unsafe { self.tasks[slot].as_ref().unwrap().abi.ptr().add(4096).cast() } }

    // Общая точка завершения (exit, kill, исключение): будит клиентов, ждущих ответа от задачи.
    fn terminate(&mut self, slot: usize, notify: bool) {
        let task = self.tasks[slot].as_mut().unwrap(); let pid = task.pid; task.state = State::Exited; task.pending_cap = None;
        for other in 1..SLOTS { if self.tasks[other].as_ref().is_some_and(|t| t.state == State::BlockedReply(slot)) { self.fail_reply(other); } }
        if self.foreground == slot { self.focus(0); if notify { self.notice = Some((pid, true)); } }
    }
    fn fail_reply(&mut self, slot: usize) {
        let mailbox = self.mailbox(slot);
        unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!((*mailbox).result), ERR_PEER); }
        self.tasks[slot].as_mut().unwrap().state = State::Ready;
    }
    fn find(&self, pid: u64) -> Option<usize> { (1..SLOTS).find(|&i| { self.tasks[i].as_ref().is_some_and(|t| t.pid == pid && t.state != State::Exited) }) }
    fn free_slot(cspace: &[Option<Capability>; CAP_SLOTS]) -> Option<usize> { (SLOT_DYNAMIC..CAP_SLOTS).find(|&i| cspace[i].is_none()) }

    // Используется ли физический диапазон кем-то ещё: мандатом, отправкой в пути или отображением.
    fn referenced(&self, physical: usize, size: usize) -> bool {
        self.tasks.iter().flatten().any(|t| t.cspace.iter().chain(core::iter::once(&t.pending_cap)).flatten().any(|c| c.overlaps(physical, size)) || t.heap.maps_foreign(physical, size))
    }
    fn retire(&mut self, region: Region) { if self.referenced(region.ptr() as usize, region.len()) { self.orphans.push(region); } }

    // Освобождает завершённые задачи только после того, как их CPU переключился на другой CR3.
    fn reap(&mut self) {
        let mut released: Vec<Region> = Vec::new();
        for slot in 1..SLOTS {
            if self.current.contains(&slot) || !self.tasks[slot].as_ref().is_some_and(|t| t.state == State::Exited) { continue; }
            let mut task = self.tasks[slot].take().unwrap();
            released.extend(task.heap.take_regions()); released.extend(task.screen.take());
        }
        // Привязку IRQ снимаем, когда мандат линии больше никому не принадлежит.
        for irq in 0..16 {
            if self.irq_bind[irq].is_some() && !self.tasks.iter().flatten().any(|t| t.state != State::Exited && t.cspace.contains(&Some(Capability::Interrupt(irq as u8)))) {
                self.irq_bind[irq] = None; self.irq_pending[irq] = false; unsafe { interrupts::set_irq_masked(irq as u8, true); }
            }
        }
        for region in released { self.retire(region); }
        let mut index = 0;
        while index < self.orphans.len() { let region = &self.orphans[index]; if self.referenced(region.ptr() as usize, region.len()) { index += 1; } else { self.orphans.swap_remove(index); } }
        if self.orphans.is_empty() && self.orphans.capacity() != 0 { self.orphans = Vec::new(); } // пустой список не держит память кучи
        let mut used = [false; ENDPOINTS]; used[..EP_RESERVED].fill(true);
        for task in self.tasks.iter().flatten() { for cap in task.cspace.iter().chain(core::iter::once(&task.pending_cap)).flatten() { if let Capability::Endpoint(id, _) = cap { used[*id] = true; } } }
        for ep in self.irq_bind.iter().flatten() { used[*ep] = true; }
        self.endpoints = used;
    }

    // DMA-область драйвера (выровнена на 64 КиБ, чтобы буфер данных не пересекал границу для DMA).
    fn dma(&mut self, name: &'static str, bytes: usize) -> Option<Capability> {
        if !self.dma.iter().any(|(owner, _)| *owner == name) { self.dma.push((name, Region::new(bytes, 64 * 1024).ok()?)); }
        self.dma.iter().find(|(owner, _)| *owner == name).map(|(_, r)| Capability::Dma(r.ptr() as usize, r.len()))
    }
    fn mmio(bar: pci::Bar) -> Option<Capability> { (!bar.io && bar.size != 0).then(|| Capability::Mmio(bar.base as usize, (bar.size as usize).div_ceil(4096) * 4096)) }

    // Сервис нужен, если для него есть оборудование (ahci, usb_storage) или он безусловный.
    fn wanted(&self, service: usize) -> bool {
        match BOOT_SERVICES[service] { "ahci" => self.ahci.is_some(), "usb_storage" => self.xhci.is_some(), _ => true }
    }

    fn initial_caps(&mut self, name: &str, service: bool, init_cap: Option<Capability>) -> [Option<Capability>; CAP_SLOTS] {
        let mut caps = [None; CAP_SLOTS]; let all = CAP_READ | CAP_WRITE | CAP_GRANT;
        match if service { name } else { "" } {
            "rtc" => { caps[SLOT_SERVICE] = Some(Capability::Endpoint(EP_RTC, all)); caps[SLOT_DEV0] = Some(Capability::IoPorts(0x70, 2)); }
            "ps2_kbd" => { caps[SLOT_DEV0] = Some(Capability::IoPorts(0x60, 1)); caps[SLOT_DEV1] = Some(Capability::IoPorts(0x64, 1)); caps[SLOT_IRQ] = Some(Capability::Interrupt(1)); caps[SLOT_PRIV] = Some(Capability::Input); }
            "compositor" => { caps[SLOT_MEM] = Some(Capability::Memory(self.boot.fb_ptr as usize, frame_bytes(&self.boot))); caps[SLOT_PRIV] = Some(Capability::Display); }
            "ata" => { caps[SLOT_SERVICE] = Some(Capability::Endpoint(EP_BLOCK_ATA, all)); caps[SLOT_DEV0] = Some(Capability::IoPorts(0x1F0, 8)); caps[SLOT_DEV1] = Some(Capability::IoPorts(0x3F6, 1)); }
            "ahci" => {
                caps[SLOT_SERVICE] = Some(Capability::Endpoint(EP_BLOCK_AHCI, all));
                if let Some(device) = self.ahci { caps[SLOT_DEV0] = Self::mmio(device.bars[5]); caps[SLOT_MEM] = self.dma("ahci", AHCI_DMA_BYTES); }
            }
            "usb_storage" => {
                caps[SLOT_SERVICE] = Some(Capability::Endpoint(EP_BLOCK_USB, all));
                if let Some(device) = self.xhci { caps[SLOT_DEV0] = Self::mmio(device.bars[0]); caps[SLOT_MEM] = self.dma("usb_storage", XHCI_DMA_BYTES); }
            }
            "vfs_server" => {
                // VFS видит только блочные устройства, драйверы которых действительно запускаются.
                caps[SLOT_SERVICE] = Some(Capability::Endpoint(EP_VFS, all));
                let devices = [(EP_BLOCK_ATA, true), (EP_BLOCK_AHCI, self.ahci.is_some()), (EP_BLOCK_USB, self.xhci.is_some())];
                for (slot, (ep, _)) in (SLOT_BLOCK_FIRST..).zip(devices.into_iter().filter(|(_, present)| *present)) { caps[slot] = Some(Capability::Endpoint(ep, CAP_WRITE | CAP_GRANT)); }
            }
            "loader" => {
                // Только loader может запускать образы из памяти; запрос шелла лежит в странице ядра.
                caps[SLOT_SERVICE] = Some(Capability::Endpoint(EP_LOADER, all)); caps[SLOT_VFS] = Some(Capability::Endpoint(EP_VFS, CAP_WRITE | CAP_GRANT));
                caps[SLOT_MEM] = Some(Capability::Memory(self.request_page.ptr() as usize, 4096)); caps[SLOT_PRIV] = Some(Capability::Spawn);
            }
            "audio_gw" => {
                caps[SLOT_SERVICE] = Some(Capability::Endpoint(EP_AUDIO, all));
                if let Some(ac) = self.ac97 {
                    caps[SLOT_DEV0] = Some(Capability::IoPorts(ac.mixer, 256)); caps[SLOT_DEV1] = Some(Capability::IoPorts(ac.bus_master, 64));
                    caps[SLOT_IRQ] = Some(Capability::Interrupt(ac.irq)); caps[SLOT_MEM] = self.dma("audio_gw", AUDIO_DMA_BYTES);
                }
            }
            _ => {
                caps[SLOT_INIT] = init_cap;
                caps[SLOT_RTC] = Some(Capability::Endpoint(EP_RTC, CAP_WRITE | CAP_GRANT));
                caps[SLOT_VFS] = Some(Capability::Endpoint(EP_VFS, CAP_WRITE | CAP_GRANT));
                caps[SLOT_AUDIO] = Some(Capability::Endpoint(EP_AUDIO, CAP_WRITE | CAP_GRANT));
                caps[SLOT_LOADER] = Some(Capability::Endpoint(EP_LOADER, CAP_WRITE | CAP_GRANT));
            }
        }
        caps
    }

    fn spawn_internal(&mut self, source: Source, name: Name, background: bool, init_cap: Option<Capability>) -> Result<u64, &'static str> {
        let service = matches!(source, Source::Boot(_));
        let live = |t: &&Task| t.state != State::Exited;
        if service && self.tasks.iter().flatten().filter(live).any(|t| t.service && t.name == name) { return Err("SERVICE ALREADY RUNNING"); }
        if !service && self.tasks.iter().flatten().filter(live).filter(|t| !t.service).count() >= MAX_APPS { return Err("TASK LIMIT REACHED (8)"); }
        let slot = (1..SLOTS).find(|&i| self.tasks[i].is_none()).ok_or("NO FREE TASK SLOT")?;
        let pid = self.next_pid; let next_pid = pid.checked_add(1).ok_or("PID SPACE EXHAUSTED")?;
        let file = match source { Source::Boot(index) => { let image = self.boot.programs.get(index).ok_or("UNKNOWN PROGRAM")?; unsafe { core::slice::from_raw_parts(image.data, image.len) } } Source::Image(bytes) => bytes };
        let elf = elf::Image::parse(file)?;
        let mut image = Region::new(elf.size.div_ceil(4096) * 4096, 4096)?; let entry = elf.load(image.bytes_mut(), paging::USER_IMAGE)?;
        let mut space = paging::Space::new()?;
        for (offset, size, flags) in elf.segments() { space.map(paging::USER_IMAGE + offset, image.ptr() as usize + offset, size, flags & 2 != 0, flags & 1 != 0)?; }
        let stack = Region::new(STACK_SIZE, 4096)?; let abi = Region::new(8192, 4096)?;
        let screen = if service { None } else { Some(Region::new(self.shell_screen.len(), 4096)?) }; // сервисам экран не нужен
        let mut info = self.boot; info.fb_ptr = if service { core::ptr::null_mut() } else { paging::USER_SCREEN as *mut u32 }; info.heap_ptr = core::ptr::null_mut(); info.heap_len = 0; info.programs = [ProgramImage { data: core::ptr::null(), len: 0 }; BOOT_IMAGES]; info.ap_trampoline = 0; info.cpu_count = 0; info.apic_ids = [0; 8];
        unsafe { (abi.ptr() as *mut BootInfo).write(info); }
        let exit = Region::new(4096, 4096)?; let code = unsafe { core::slice::from_raw_parts_mut(exit.ptr(), 21) }; code[0..2].copy_from_slice(&[0x48, 0xb8]); code[2..10].copy_from_slice(&(paging::USER_MAILBOX as u64).to_le_bytes()); code[10..21].copy_from_slice(&[0x48, 0xc7, 0x00, 7, 0, 0, 0, 0xcd, 0x80, 0x0f, 0x0b]); let user_sp = paging::USER_STACK + STACK_SIZE - 8; unsafe { ((stack.ptr() as usize + STACK_SIZE - 8) as *mut usize).write(paging::USER_EXIT); }
        space.map(paging::USER_STACK, stack.ptr() as usize, stack.len(), true, false)?;
        if let Some(screen) = &screen { space.map(paging::USER_SCREEN, screen.ptr() as usize, screen.len(), true, false)?; }
        space.map(paging::USER_INFO, abi.ptr() as usize, 4096, false, false)?; space.map(paging::USER_MAILBOX, abi.ptr() as usize + 4096, 4096, true, false)?; space.map(paging::USER_EXIT, exit.ptr() as usize, 4096, false, true)?;
        let context = Region::new(context::SIZE, 16)?; let sp = context.ptr() as usize; unsafe { context::initial(sp, entry, user_sp); }
        // Приложения распределяются по числу приложений на CPU: спящие сервисы не сдвигают баланс.
        let cpu = (0..cpu::COUNT.load(Ordering::Acquire)).filter(|&i| cpu::ONLINE[i].load(Ordering::Acquire)).min_by_key(|&i| { self.tasks.iter().flatten().filter(|t| t.cpu == i && t.state != State::Exited && t.service == service).count() }).unwrap_or(0);
        let cspace = self.initial_caps(name.as_str(), service, init_cap);
        self.tasks[slot] = Some(Task { pid, name, service, state: State::Ready, sp, cpu, space, heap: crate::user_heap::Heap::new(), context, _exit: exit, runs: 0, ticks: 0, calls: 0, _image: image, _stack: stack, screen, abi, input: Queue::new(), log: Queue::new(), log_line_start: true, dirty: true, cspace, pending_cap: None, pending_call: false, send_seq: 0, reply_to: None });
        self.next_pid = next_pid; if !background && !service { self.focus(slot); } Ok(pid)
    }

    fn cap(&self, slot: usize, index: usize) -> Option<Capability> { if index < CAP_SLOTS { self.tasks[slot].as_ref().unwrap().cspace[index] } else { None } }
    // Копия мандата для передачи; права точки IPC сужаются маской отправителя.
    fn transfer(&self, slot: usize, index: usize, mask: usize) -> Option<Capability> {
        if index == 0 { return None; }
        match self.cap(slot, index)? { Capability::Endpoint(id, rights) => Some(Capability::Endpoint(id, rights & mask as u8)), other => Some(other) }
    }
    fn blocked(&self, state: State) -> Option<usize> { (1..SLOTS).find(|&i| self.tasks[i].as_ref().is_some_and(|t| t.state == state)) }

    // Передаёт сообщение блокированного или текущего отправителя получателю.
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
    // Сообщение от ядра (IRQ или запрос шелла): отправитель с PID 0, без мандата.
    unsafe fn notify(&mut self, to: usize, flag: usize, value: usize) {
        let mb = self.mailbox(to);
        (*mb).msg = [0, flag, value, 0]; (*mb).arg1 = 0; (*mb).result = 0;
        self.tasks[to].as_mut().unwrap().state = State::Ready;
    }
    unsafe fn notify_irq(&mut self, to: usize, irq: usize) { self.notify(to, MSG_FLAG_IRQ, irq) }

    // Запрос шелла: страница [вид, фон, длина, имя]; loader получает уведомление или найдёт его при RECV.
    fn post_request(&mut self, kind: u8, name: &[u8], background: bool) -> Result<usize, &'static str> {
        if !self.tasks.iter().flatten().any(|t| t.service && t.state != State::Exited && t.name.as_str() == "loader") { return Err("LOADER NOT RUNNING"); }
        if self.request.is_some_and(|r| r.done.is_none()) { return Err("LOADER BUSY"); }
        let page = unsafe { core::slice::from_raw_parts_mut(self.request_page.ptr(), 4096) };
        let len = name.len().min(255); page[..3].copy_from_slice(&[kind, background as u8, len as u8]); page[3..3 + len].copy_from_slice(&name[..len]);
        let id = self.next_request; self.next_request += 1;
        self.request = Some(Request { id, background, spawned: None, done: None });
        match self.blocked(State::BlockedRecv(EP_LOADER)) { Some(loader) => unsafe { self.notify(loader, MSG_FLAG_KERNEL, id) }, None => self.loader_notify = true }
        Ok(id)
    }
    // Прерывание линии: маска уже выставлена; будит драйвер или запоминает событие.
    unsafe fn raise_irq(&mut self, irq: usize) {
        if let Some(ep) = self.irq_bind[irq] {
            if let Some(receiver) = self.blocked(State::BlockedRecv(ep)) { self.notify_irq(receiver, irq); } else { self.irq_pending[irq] = true; }
            return;
        }
        let mut woken = false;
        for task in self.tasks.iter_mut().flatten() { if task.state == State::BlockedIrq(irq as u8) { task.state = State::Ready; woken = true; } }
        if !woken { self.irq_pending[irq] = true; }
    }

    // Ok(Some(sp)) — вызывающий заблокирован, переключаемся; Ok(None) — сразу вернуть 0.
    unsafe fn ipc_send(&mut self, slot: usize, sp: usize, cpu: usize, request: &SyscallMailbox, call: bool) -> Result<Option<usize>, usize> {
        let Some(Capability::Endpoint(ep, rights)) = self.cap(slot, request.arg1) else { return Err(ERR_INVALID); };
        if rights & CAP_WRITE == 0 { return Err(ERR_RIGHTS); }
        let cap = if rights & CAP_GRANT != 0 { self.transfer(slot, request.msg[0], request.msg[1]) } else { None };
        self.send_seq += 1; let seq = self.send_seq;
        let task = self.tasks[slot].as_mut().unwrap(); task.pending_cap = cap; task.pending_call = call; task.send_seq = seq;
        if let Some(receiver) = self.blocked(State::BlockedRecv(ep)) {
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
        if ep == EP_LOADER && core::mem::take(&mut self.loader_notify) { if let Some(request) = self.request { self.notify(slot, MSG_FLAG_KERNEL, request.id); return Ok(None); } }
        let sender = (1..SLOTS).filter(|&i| self.tasks[i].as_ref().is_some_and(|t| t.state == State::BlockedSend(ep))).min_by_key(|&i| self.tasks[i].as_ref().unwrap().send_seq);
        if let Some(sender) = sender { self.deliver(sender, slot); return Ok(None); }
        let task = self.tasks[slot].as_mut().unwrap(); task.state = State::BlockedRecv(ep); task.dirty = true;
        Ok(Some(self.select(sp, cpu)))
    }
    unsafe fn ipc_reply(&mut self, slot: usize, request: &SyscallMailbox) -> Result<usize, usize> {
        let Some((caller, pid)) = self.tasks[slot].as_mut().unwrap().reply_to.take() else { return Err(ERR_INVALID); };
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
    fn holds_spawn(&self, slot: usize) -> bool { self.holds(slot, Capability::Spawn) }

    unsafe fn syscall(&mut self, slot: usize, sp: usize, cpu: usize) -> usize {
        let ptr = self.mailbox(slot); let request = core::ptr::read_volatile(ptr);
        let tasks = self.tasks.as_mut_ptr(); let task = (*tasks.add(slot)).as_mut().unwrap(); task.calls += 1;
        let result: Result<usize, usize> = match request.syscall_num {
            SYSCALL_RDTSC => { let lo: u32; let hi: u32; asm!("rdtsc", out("eax") lo, out("edx") hi); Ok((((hi as u64) << 32) | lo as u64) as usize) }
            SYSCALL_READ_KEY => Ok(task.input.pop().unwrap_or(0) as usize),
            SYSCALL_LOG => {
                let length = request.arg2.min(4096);
                if !task.space.validate_read(request.arg1, length) { Err(ERR_INVALID) } else {
                    for i in 0..length {
                        let physical = task.space.readable(request.arg1 + i).unwrap(); let byte = core::ptr::read_volatile(physical as *const u8);
                        task.log.push(byte);
                        if self.foreground == slot { if task.log_line_start && byte != b'\r' && byte != b'\n' { for b in b"[PID " { serial_write_byte(*b); } serial_number(task.pid); for b in b"] " { serial_write_byte(*b); } } serial_write_byte(byte); }
                        task.log_line_start = byte == b'\n';
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
            SYSCALL_SPAWN_IMAGE => {
                // ELF из памяти loader: ядро разбирает и копирует его, буфер loader освобождает сам.
                let length = request.arg2.min(NAME_MAX); let mut name = [0u8; NAME_MAX];
                let image = match self.cap(slot, request.msg[0]) { Some(Capability::Memory(physical, size)) if request.msg[1] <= size => Some((physical, request.msg[1])), _ => None };
                if !self.holds(slot, Capability::Spawn) { Err(ERR_RIGHTS) }
                else if length == 0 || !task.space.validate_read(request.arg1, length) || image.is_none() { Err(ERR_INVALID) }
                else {
                    for (i, byte) in name[..length].iter_mut().enumerate() { *byte = core::ptr::read_volatile(task.space.readable(request.arg1 + i).unwrap() as *const u8); }
                    let name = Name::new(&name[..length]);
                    if service_index(name.as_str().as_bytes()).is_some() { Err(ERR_INVALID) } else {
                        let (physical, size) = image.unwrap();
                        let mask = request.msg[3] & 0xFF; let id = request.msg[3] >> 16;
                        let delegated = match self.cap(slot, request.msg[2]) { Some(Capability::Endpoint(ep, r)) if request.msg[2] != 0 && r & CAP_GRANT != 0 => Some(Capability::Endpoint(ep, r & mask as u8)), _ => None };
                        let shell = self.request.filter(|r| id != 0 && r.id == id && r.done.is_none());
                        let bytes = core::slice::from_raw_parts(physical as *const u8, size);
                        let outcome = self.spawn_internal(Source::Image(bytes), name, shell.is_none_or(|r| r.background), delegated);
                        if let Some(request) = self.request.as_mut().filter(|_| shell.is_some()) { request.spawned = Some(outcome); }
                        outcome.map(|pid| pid as usize).map_err(|_| ERR_NO_SLOT)
                    }
                }
            }
            SYSCALL_LOADER_DONE => {
                // Итог запроса шелла: длина текста LIST или код ошибки запуска (после освобождения буфера образа).
                let allowed = self.holds_spawn(slot);
                match self.request.as_mut() { Some(r) if allowed && r.id == request.arg1 && r.done.is_none() => { r.done = Some(request.arg2); Ok(0) } _ => Err(ERR_INVALID) }
            }
            SYSCALL_MEM_SHARE => match (task.heap.shareable(request.arg1, request.arg2), Self::free_slot(&task.cspace)) {
                (Some((physical, size)), Some(index)) => { task.cspace[index] = Some(Capability::Memory(physical, size)); Ok(index) }
                (None, _) => Err(ERR_INVALID),
                _ => Err(ERR_NO_SLOT),
            },
            SYSCALL_MEM_MAP => match self.cap(slot, request.arg1) {
                Some(cap @ (Capability::Memory(physical, size) | Capability::Dma(physical, size) | Capability::Mmio(physical, size))) => {
                    core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).arg2), size); // размер отображения для клиента
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
                // Чтение 16-битных слов (сектор ATA) прямо в буфер процесса, без системного вызова на слово.
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
                // Только драйвер с мандатом ввода может подмешивать нажатия в шелл и программы.
                if !self.holds(slot, Capability::Input) { Err(ERR_RIGHTS) } else { self.route_key(request.arg1 as u8, request.arg2 as u8, request.msg[0] != 0); Ok(0) }
            }
            SYSCALL_COMPOSITOR_PULL => {
                if !self.holds(slot, Capability::Display) || !(1..CAP_SLOTS).contains(&request.arg1) { Err(ERR_RIGHTS) } else {
                    let screen = (self.foreground != 0).then(|| (*tasks.add(self.foreground)).as_mut()).flatten();
                    let (source, dirty) = match screen { Some(t) if t.screen.is_some() => (t.screen.as_ref().unwrap().ptr() as usize, core::mem::take(&mut t.dirty) | self.dirty), _ => (self.shell_screen.ptr() as usize, self.dirty) };
                    self.dirty = false;
                    // Новый мандат только при смене экрана: композитор держит отображение между кадрами.
                    if source != self.composited { self.composited = source; task.cspace[request.arg1] = Some(Capability::Memory(source, frame_bytes(&self.boot))); Ok(2) } else { Ok(dirty as usize) }
                }
            }
            SYSCALL_CAP_INFO => {
                // Драйвер узнаёт, что ему выдано (например, базу портов BAR), не видя физических адресов памяти.
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

pub fn spawn_service(index: usize) -> Result<u64, &'static str> { locked(|| unsafe { scheduler().spawn_internal(Source::Boot(index), Name::new(BOOT_SERVICES[index].as_bytes()), true, None) }) }
pub fn service_wanted(index: usize) -> bool { locked(|| unsafe { scheduler().wanted(index) }) }
pub fn post_request(kind: u8, name: &[u8], background: bool) -> Result<usize, &'static str> { locked(|| unsafe { scheduler().post_request(kind, name, background) }) }
// Итог запроса, когда loader его завершил: (запуск, код LOADER_DONE); текст LIST копируется в `text`.
pub fn take_request(id: usize, text: &mut [u8]) -> Option<(Option<Result<u64, &'static str>>, usize)> {
    locked(|| unsafe {
        let s = scheduler();
        let request = s.request.filter(|r| r.id == id)?; let code = request.done?;
        let page = core::slice::from_raw_parts(s.request_page.ptr().add(LOADER_REPLY), 4096 - LOADER_REPLY);
        let len = code.min(page.len()).min(text.len()); text[..len].copy_from_slice(&page[..len]);
        s.request = None; Some((request.spawned, code))
    })
}
pub fn cancel_request(id: usize) { locked(|| unsafe { let s = scheduler(); if s.request.is_some_and(|r| r.id == id) { s.request = None; s.loader_notify = false; } }) }

pub extern "C" fn interrupt(sp: usize) -> usize {
    unsafe {
        let registers = context::registers(sp); let vector = registers[15]; let cpu = cpu::id();
        if vector == 0x31 { asm!("cli"); loop { asm!("hlt"); } }
        if vector < 32 && registers[18] & 3 == 0 { for &b in b"KERNEL EXCEPTION VECTOR=" { serial_write_byte(b); } serial_number(vector); for &b in b" RIP=" { serial_write_byte(b); } serial_hex(registers[17]); for &b in b" ERROR=" { serial_write_byte(b); } serial_hex(registers[16]); for &b in b"\r\n" { serial_write_byte(b); } cpu::halt_all(); }
        let irq = (33..48).contains(&vector).then_some(vector as usize - 32);
        if vector == 32 { interrupts::advance(); outb(0x20, 0x20); cpu::eoi(); cpu::tick_others(); }
        else if let Some(irq) = irq { interrupts::set_irq_masked(irq as u8, true); if irq >= 8 { outb(0xA0, 0x20); } outb(0x20, 0x20); cpu::eoi(); } // линию откроет драйвер
        else if vector == 48 { cpu::eoi(); }

        locked(|| {
            let s = scheduler(); let slot = s.current[cpu];
            if let Some(irq) = irq { s.raise_irq(irq); return s.select(sp, cpu); }
            if vector == 32 || vector == 48 {
                cpu::TICKS[cpu].fetch_add(1, Ordering::Relaxed); let now = interrupts::milliseconds(); for task in s.tasks.iter_mut().flatten() { task.state.wake(now); }
                if cpu == 0 { s.poll_serial(); } if slot == 0 && cpu == 0 { return sp; } if slot != 0 { let t = s.tasks[slot].as_mut().unwrap(); t.ticks += 1; t.dirty = true; }
                return s.select(sp, cpu);
            }
            if vector < 32 {
                let mut address = 0u64; if vector == 14 { asm!("mov {}, cr2", out(reg) address); }
                let pid = s.tasks[slot].as_ref().unwrap().pid; let at = s.fault_cursor % s.faults.len();
                s.faults[at] = Some(Fault { pid, cpu, vector, error: registers[16], rip: registers[17], address });
                s.fault_cursor += 1; s.terminate(slot, true); return s.select(sp, cpu);
            }
            if cpu == 0 { s.poll_serial(); } if slot == 0 { return s.select(sp, cpu); }
            if s.tasks[slot].as_ref().unwrap().state == State::Exited { return s.select(sp, cpu); }
            s.syscall(slot, sp, cpu)
        })
    }
}

unsafe fn serial_hex(number: u64) { for shift in (0..16).rev() { let digit = ((number >> (shift * 4)) & 15) as u8; serial_write_byte(if digit < 10 { b'0' + digit } else { b'A' + digit - 10 }); } }
pub fn faults() -> [Option<Fault>; 16] { locked(|| unsafe { scheduler().faults }) }
unsafe fn serial_number(mut number: u64) { let mut buffer = [0; 20]; let mut at = buffer.len(); loop { at -= 1; buffer[at] = b'0' + (number % 10) as u8; number /= 10; if number == 0 { break; } } for &byte in &buffer[at..] { serial_write_byte(byte); } }
pub fn task_name(pid: u64) -> Option<Name> { locked(|| unsafe { let s = scheduler(); s.find(pid).map(|slot| s.tasks[slot].as_ref().unwrap().name) }) }
pub fn foreground() -> u64 { locked(|| unsafe { let s = scheduler(); s.tasks[s.foreground].as_ref().map_or(0, |t| t.pid) }) }
pub fn focus(pid: u64) -> Result<(), &'static str> { locked(|| unsafe { let s = scheduler(); let slot = s.find(pid).ok_or("NO SUCH PID")?; if s.tasks[slot].as_ref().unwrap().service { return Err("SERVICE HAS NO SCREEN"); } s.focus(slot); Ok(()) }) }
pub fn kill(pid: u64) -> Result<(), &'static str> { locked(|| unsafe { let s = scheduler(); let slot = s.find(pid).ok_or("NO SUCH PID")?; s.terminate(slot, false); Ok(()) }) }
pub struct Summary { pub pid: u64, pub name: Name, pub state: &'static str, pub foreground: bool, pub runs: u64, pub ticks: u64, pub calls: u64, pub cpu: usize }
pub fn summaries() -> [Option<Summary>; MAX_TASKS] { locked(|| unsafe { let s = scheduler(); core::array::from_fn(|i| { s.tasks[i + 1].as_ref().map(|t| Summary { pid: t.pid, name: t.name, state: if s.current.contains(&(i + 1)) { "RUNNING" } else { t.state.label() }, foreground: s.foreground == i + 1, runs: t.runs, ticks: t.ticks, calls: t.calls, cpu: t.cpu }) }) }) }
pub fn logs(pid: u64, buffer: &mut [u8]) -> Result<usize, &'static str> { locked(|| unsafe { let s = scheduler(); let slot = s.find(pid).ok_or("NO SUCH PID")?; let log = &mut s.tasks[slot].as_mut().unwrap().log; let mut len = 0; while len < buffer.len() { let Some(byte) = log.pop() else { break; }; buffer[len] = byte; len += 1; } Ok(len) }) }
pub fn input() -> Option<u8> { locked(|| unsafe { let s = scheduler(); s.poll_serial(); s.shell_input.pop() }) }
pub fn notice() -> Option<(u64, bool)> { locked(|| unsafe { scheduler().notice.take() }) }
pub fn dirty() { locked(|| unsafe { scheduler().dirty = true; }); }
// Вызывается шеллом на BSP: возвращает память завершённых задач (раньше это делал service()).
pub fn reap() { locked(|| unsafe { scheduler().reap() }) }

pub fn idle() { interrupts::without(|| unsafe { let ready = locked(|| { scheduler().states(cpu::id())[1..].iter().any(|s| *s == State::Ready) }); if ready { asm!("int 0x80"); } else { asm!("sti", "hlt", "cli"); } }); }
