use crate::abi::{ BootInfo, ProgramImage, SyscallMailbox, RTC_UNAVAILABLE, SYSCALL_ALLOC, SYSCALL_EXIT, SYSCALL_FREE, SYSCALL_UPTIME, SYSCALL_WAIT, SYSCALL_IPC_SEND, SYSCALL_IPC_RECV, SYSCALL_ENDPOINT_CREATE, SYSCALL_SPAWN, SYSCALL_CAP_DROP, SYSCALL_MEM_SHARE, SYSCALL_MEM_MAP };
use crate::input::{Keyboard, Queue};
use crate::memory::Region;
use crate::task_state::{self, State};
use crate::{context, cpu, elf, interrupts, outb, paging, serial_write_byte};
use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

pub const MAX_TASKS: usize = 8;
const SLOTS: usize = MAX_TASKS + 1;
const STACK_SIZE: usize = 64 * 1024;
pub const PROGRAM_NAMES: [&str; crate::abi::PROGRAM_COUNT] = ["app", "app2", "clock", "dzen-clock", "ping", "pong", "rtc"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Capability { Endpoint(usize, u8), Memory(usize, usize), IOPort(u16) }

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EpState { Unused, Idle, Sending(usize), Receiving(usize) }

pub struct Endpoint { pub state: EpState }

pub fn heap_test() -> (usize, usize, bool) { locked(|| { let before = crate::ALLOCATOR.lock().used(); let test = alloc::format!("Dynamic allocation test at {} ms", interrupts::milliseconds()); core::hint::black_box(&test); drop(test); let heap = crate::ALLOCATOR.lock(); (heap.used(), heap.free(), heap.used() == before) }) }

struct Task { pid: u64, program: usize, state: State, sp: usize, cpu: usize, space: paging::Space, heap: crate::user_heap::Heap, context: Region, _exit: Region, runs: u64, ticks: u64, calls: u64, _image: Region, _stack: Region, screen: Region, abi: Region, input: Queue<128>, log: Queue<4096>, log_line_start: bool, dirty: bool, cspace: [Option<Capability>; 32] }
struct Scheduler { boot: BootInfo, tasks: [Option<Task>; SLOTS], current: [usize; cpu::MAX], idle_sp: [usize; cpu::MAX], faults: [Option<Fault>; 16], fault_cursor: usize, next_pid: u64, foreground: usize, keyboard: Keyboard, shell_input: Queue<128>, shell_screen: Region, shadow: Region, dirty: bool, shadow_valid: bool, notice: Option<(u64, bool)>, endpoints: [Endpoint; 64] }

static mut SCHEDULER: Option<Scheduler> = None;
static LOCK: AtomicBool = AtomicBool::new(false);
struct Guard; impl Drop for Guard { fn drop(&mut self) { LOCK.store(false, Ordering::Release); } }
fn locked<T>(f: impl FnOnce() -> T) -> T { interrupts::without(|| { while LOCK.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() { core::hint::spin_loop(); } let _guard = Guard; f() }) }
#[derive(Clone, Copy)] pub struct Fault { pub pid: u64, pub cpu: usize, pub vector: u64, pub error: u64, pub rip: u64, pub address: u64 }

unsafe fn scheduler() -> &'static mut Scheduler { (*core::ptr::addr_of_mut!(SCHEDULER)).as_mut().unwrap() }

pub fn init(info: &BootInfo) -> Result<*mut u32, &'static str> {
    let bytes = info.stride.checked_mul(info.height).and_then(|n| n.checked_mul(4)).ok_or("FRAMEBUFFER SIZE OVERFLOW")?;
    let shell_screen = Region::new(bytes, 16)?; let fb = shell_screen.ptr().cast(); let shadow = Region::new(bytes, 16)?;
    unsafe { *core::ptr::addr_of_mut!(SCHEDULER) = Some(Scheduler { boot: *info, tasks: core::array::from_fn(|_| None), current: [0; cpu::MAX], idle_sp: [0; cpu::MAX], faults: [None; 16], fault_cursor: 0, next_pid: 1, foreground: 0, keyboard: Keyboard::new(), shell_input: Queue::new(), shell_screen, shadow, dirty: true, shadow_valid: false, notice: None, endpoints: core::array::from_fn(|_| Endpoint { state: EpState::Unused }) }); } Ok(fb)
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
    fn poll_input(&mut self) { for _ in 0..32 { let Some(key) = self.keyboard.read() else { break; }; if key.background && self.foreground != 0 { let pid = self.tasks[self.foreground].as_ref().unwrap().pid; self.focus(0); self.notice = Some((pid, false)); } else if self.foreground == 0 { if !key.background { self.shell_input.push(key.shell); } } else if key.app != 0 { let task = self.tasks[self.foreground].as_mut().unwrap(); task.input.push(key.app); if matches!(task.state, State::Sleeping(_)) { task.state = State::Ready; } } } }
    fn exit_current(&mut self, cpu: usize) {
        let task = self.tasks[self.current[cpu]].as_mut().unwrap(); let pid = task.pid; task.state = State::Exited;
        for ep in self.endpoints.iter_mut() { if ep.state == EpState::Sending(self.current[cpu]) || ep.state == EpState::Receiving(self.current[cpu]) { ep.state = EpState::Idle; } }
        if self.foreground == self.current[cpu] { self.focus(0); self.notice = Some((pid, true)); }
    }
    fn find(&self, pid: u64) -> Option<usize> { (1..SLOTS).find(|&i| { self.tasks[i].as_ref().is_some_and(|t| t.pid == pid && t.state != State::Exited) }) }
    
    fn spawn_internal(&mut self, program: usize, background: bool, init_cap: Option<Capability>) -> Result<u64, &'static str> {
        let slot = (1..SLOTS).find(|&i| self.tasks[i].is_none()).ok_or("TASK LIMIT REACHED (8)")?;
        let pid = self.next_pid; let next_pid = pid.checked_add(1).ok_or("PID SPACE EXHAUSTED")?;
        let source = self.boot.programs.get(program).ok_or("UNKNOWN PROGRAM")?; let file = unsafe { core::slice::from_raw_parts(source.data, source.len) }; let elf = elf::Image::parse(file)?;
        let mut image = Region::new(elf.size.div_ceil(4096) * 4096, 4096)?; let entry = elf.load(image.bytes_mut(), paging::USER_IMAGE)?;
        let mut space = paging::Space::new()?;
        for (offset, size, flags) in elf.segments() { space.map(paging::USER_IMAGE + offset, image.ptr() as usize + offset, size, flags & 2 != 0, flags & 1 != 0)?; }
        let stack = Region::new(STACK_SIZE, 4096)?; let screen = Region::new(self.shell_screen.len().div_ceil(4096) * 4096, 4096)?; let abi = Region::new(8192, 4096)?;
        let mut info = self.boot; info.fb_ptr = paging::USER_SCREEN as *mut u32; info.heap_ptr = core::ptr::null_mut(); info.heap_len = 0; info.programs = [ProgramImage { data: core::ptr::null(), len: 0 }; crate::abi::PROGRAM_COUNT]; info.ap_trampoline = 0; info.cpu_count = 0; info.apic_ids = [0; 8];
        unsafe { (abi.ptr() as *mut BootInfo).write(info); }
        let exit = Region::new(4096, 4096)?; let code = unsafe { core::slice::from_raw_parts_mut(exit.ptr(), 21) }; code[0..2].copy_from_slice(&[0x48, 0xb8]); code[2..10].copy_from_slice(&(paging::USER_MAILBOX as u64).to_le_bytes()); code[10..21].copy_from_slice(&[0x48, 0xc7, 0x00, 7, 0, 0, 0, 0xcd, 0x80, 0x0f, 0x0b]); let user_sp = paging::USER_STACK + STACK_SIZE - 8; unsafe { ((stack.ptr() as usize + STACK_SIZE - 8) as *mut usize).write(paging::USER_EXIT); }
        space.map(paging::USER_STACK, stack.ptr() as usize, stack.len(), true, false)?; space.map(paging::USER_SCREEN, screen.ptr() as usize, screen.len(), true, false)?; space.map(paging::USER_INFO, abi.ptr() as usize, 4096, false, false)?; space.map(paging::USER_MAILBOX, abi.ptr() as usize + 4096, 4096, true, false)?; space.map(paging::USER_EXIT, exit.ptr() as usize, 4096, false, true)?;
        let context = Region::new(context::SIZE, 16)?; let sp = context.ptr() as usize; unsafe { context::initial(sp, entry, user_sp); }
        let cpu = (0..cpu::COUNT.load(Ordering::Acquire)).filter(|&i| cpu::ONLINE[i].load(Ordering::Acquire)).min_by_key(|&i| { self.tasks.iter().flatten().filter(|t| t.cpu == i && t.state != State::Exited).count() }).unwrap_or(0);
        
        let mut cspace = [None; 32];
        if program == 6 { // rtc driver
            self.endpoints[2].state = EpState::Idle;
            cspace[1] = Some(Capability::Endpoint(2, crate::abi::CAP_READ | crate::abi::CAP_WRITE | crate::abi::CAP_GRANT));
            cspace[2] = Some(Capability::IOPort(0x70));
            cspace[3] = Some(Capability::IOPort(0x71));
        } else {
            cspace[1] = init_cap; 
            cspace[2] = Some(Capability::Endpoint(2, crate::abi::CAP_WRITE | crate::abi::CAP_GRANT)); // Доступ к RTC
        }

        self.tasks[slot] = Some(Task { pid, program, state: State::Ready, sp, cpu, space, heap: crate::user_heap::Heap::new(), context, _exit: exit, runs: 0, ticks: 0, calls: 0, _image: image, _stack: stack, screen, abi, input: Queue::new(), log: Queue::new(), log_line_start: true, dirty: true, cspace });
        self.next_pid = next_pid; if !background { self.focus(slot); } Ok(pid)
    }
}

pub fn spawn(program: usize, background: bool) -> Result<u64, &'static str> { locked(|| unsafe { scheduler().spawn_internal(program, background, None) }) }

pub extern "C" fn interrupt(sp: usize) -> usize {
    unsafe {
        let registers = context::registers(sp); let vector = registers[15]; let cpu = cpu::id();
        if vector == 0x31 { asm!("cli"); loop { asm!("hlt"); } }
        if vector < 32 && registers[18] & 3 == 0 { for &b in b"KERNEL EXCEPTION VECTOR=" { serial_write_byte(b); } serial_number(vector); for &b in b" RIP=" { serial_write_byte(b); } serial_hex(registers[17]); for &b in b" ERROR=" { serial_write_byte(b); } serial_hex(registers[16]); for &b in b"\r\n" { serial_write_byte(b); } cpu::halt_all(); }
        if vector == 32 { interrupts::advance(); outb(0x20, 0x20); cpu::eoi(); cpu::tick_others(); } else if vector == 48 { cpu::eoi(); }
        locked(|| {
            let s = scheduler(); let slot = s.current[cpu];
            if vector == 32 || vector == 48 {
                cpu::TICKS[cpu].fetch_add(1, Ordering::Relaxed); let now = interrupts::milliseconds(); for task in s.tasks.iter_mut().flatten() { task.state.wake(now); }
                if cpu == 0 { s.poll_input(); } if slot == 0 && cpu == 0 { return sp; } if slot != 0 { let t = s.tasks[slot].as_mut().unwrap(); t.ticks += 1; t.dirty = true; }
                return s.select(sp, cpu);
            }
            if vector < 32 {
                let mut address = 0u64; if vector == 14 { asm!("mov {}, cr2", out(reg) address); }
                let pid = s.tasks[slot].as_ref().unwrap().pid; let at = s.fault_cursor % s.faults.len();
                s.faults[at] = Some(Fault { pid, cpu, vector, error: registers[16], rip: registers[17], address });
                s.fault_cursor += 1; s.exit_current(cpu); return s.select(sp, cpu);
            }
            if cpu == 0 { s.poll_input(); } if slot == 0 { return s.select(sp, cpu); }
            if s.tasks[slot].as_ref().unwrap().state == State::Exited { return s.select(sp, cpu); }
            
            let tasks_ptr = s.tasks.as_mut_ptr();
            let task = (*tasks_ptr.add(slot)).as_mut().unwrap();
            
            task.calls += 1; let ptr = task.abi.ptr().add(4096).cast::<SyscallMailbox>(); let request = core::ptr::read_volatile(ptr);
            let result = match request.syscall_num {
                1 => { let lo: u32; let hi: u32; asm!("rdtsc", out("eax") lo, out("edx") hi); (((hi as u64) << 32) | lo as u64) as usize }
                2 => task.input.pop().unwrap_or(0) as usize,
                3 => {
                    let length = request.arg2.min(4096);
                    if !task.space.validate_read(request.arg1, length) { usize::MAX } else {
                        for i in 0..length {
                            let physical = task.space.readable(request.arg1 + i).unwrap(); let byte = core::ptr::read_volatile(physical as *const u8);
                            task.log.push(byte);
                            if s.foreground == slot { if task.log_line_start && byte != b'\r' && byte != b'\n' { for b in b"[PID " { serial_write_byte(*b); } serial_number(task.pid); for b in b"] " { serial_write_byte(*b); } } serial_write_byte(byte); }
                            task.log_line_start = byte == b'\n';
                        } length
                    }
                }
                SYSCALL_UPTIME => interrupts::milliseconds() as usize,
                SYSCALL_ALLOC => task.heap.allocate(&mut task.space, request.arg1).unwrap_or(0), SYSCALL_FREE => { if task.heap.free(&mut task.space, request.arg1) { 0 } else { usize::MAX } }
                SYSCALL_WAIT => {
                    let now = interrupts::milliseconds(); let duration = request.arg1.min(60_000).div_ceil(10).max(1) as u64 * 10;
                    core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).result), now as usize);
                    task.state = if task.input.is_empty() { State::Sleeping(now.wrapping_add(duration)) } else { State::Ready };
                    task.dirty = true; return s.select(sp, cpu);
                }
                SYSCALL_EXIT => { s.exit_current(cpu); return s.select(sp, cpu); }
                
                SYSCALL_ENDPOINT_CREATE => {
                    if let Some(ep_id) = s.endpoints.iter().position(|e| e.state == EpState::Unused) {
                        if let Some(cap_slot) = task.cspace.iter().skip(1).position(|c| c.is_none()) {
                            let actual_slot = cap_slot + 1;
                            s.endpoints[ep_id].state = EpState::Idle;
                            task.cspace[actual_slot] = Some(Capability::Endpoint(ep_id, crate::abi::CAP_READ | crate::abi::CAP_WRITE | crate::abi::CAP_GRANT));
                            actual_slot
                        } else { usize::MAX - 1 }
                    } else { usize::MAX }
                }
                SYSCALL_CAP_DROP => {
                    let cap_idx = request.arg1;
                    if cap_idx > 0 && cap_idx < 32 { task.cspace[cap_idx] = None; 0 } else { usize::MAX }
                }
                SYSCALL_SPAWN => {
                    let name_len = request.arg2.min(32);
                    if !task.space.validate_read(request.arg1, name_len) { usize::MAX } else {
                        let mut name_buf = [0u8; 32];
                        for i in 0..name_len { let p = task.space.readable(request.arg1 + i).unwrap(); name_buf[i] = core::ptr::read_volatile(p as *const u8); }
                        if let Ok(name_str) = core::str::from_utf8(&name_buf[..name_len]) {
                            if let Some(prog_idx) = PROGRAM_NAMES.iter().position(|p| p.eq_ignore_ascii_case(name_str)) {
                                let grant_slot = request.msg[0]; let grant_rights = request.msg[1] as u8;
                                let cap = if grant_slot < 32 { task.cspace[grant_slot] } else { None };
                                let delegated_cap = if let Some(Capability::Endpoint(id, r)) = cap {
                                    if r & crate::abi::CAP_GRANT != 0 { Some(Capability::Endpoint(id, r & grant_rights)) } else { None }
                                } else { None };
                                
                                match s.spawn_internal(prog_idx, true, delegated_cap) {
                                    Ok(pid) => pid as usize, Err(_) => usize::MAX - 1
                                }
                            } else { usize::MAX - 2 }
                        } else { usize::MAX - 3 }
                    }
                }
                SYSCALL_MEM_SHARE => {
                    let vaddr = request.arg1;
                    let size = request.arg2;
                    if let Some(phys) = task.space.readable(vaddr) {
                        if let Some(cap_slot) = task.cspace.iter().skip(1).position(|c| c.is_none()) {
                            let actual_slot = cap_slot + 1;
                            task.cspace[actual_slot] = Some(Capability::Memory(phys, size));
                            actual_slot
                        } else { usize::MAX - 1 }
                    } else { usize::MAX - 2 }
                }
                SYSCALL_MEM_MAP => {
                    let cap_idx = request.arg1;
                    if cap_idx < 32 {
                        if let Some(Capability::Memory(phys, size)) = task.cspace[cap_idx] {
                            task.heap.map_shared(&mut task.space, phys, size).unwrap_or(usize::MAX)
                        } else { usize::MAX - 1 }
                    } else { usize::MAX }
                }
                17 => { // SYSCALL_PORT_IN
                    let cap_idx = request.arg1; let port = request.arg2 as u16;
                    if cap_idx < 32 {
                        if let Some(Capability::IOPort(p)) = task.cspace[cap_idx] {
                            if p == port { let mut val: u8; asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack)); val as usize } else { usize::MAX - 2 }
                        } else { usize::MAX - 1 }
                    } else { usize::MAX }
                }
                18 => { // SYSCALL_PORT_OUT
                    let cap_idx = request.arg1; let port = request.arg2 as u16; let val = request.msg[0] as u8;
                    if cap_idx < 32 {
                        if let Some(Capability::IOPort(p)) = task.cspace[cap_idx] {
                            if p == port { asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack)); 0 } else { usize::MAX - 2 }
                        } else { usize::MAX - 1 }
                    } else { usize::MAX }
                }
                SYSCALL_IPC_SEND => {
                    let cap_idx = request.arg1;
                    if cap_idx >= 32 { usize::MAX } else if let Some(Capability::Endpoint(ep_id, rights)) = task.cspace[cap_idx] {
                        if rights & crate::abi::CAP_WRITE == 0 { usize::MAX - 2 } else {
                            let transfer_cap_idx = request.msg[0];
                            let transfer_cap = if transfer_cap_idx > 0 && transfer_cap_idx < 32 && (rights & crate::abi::CAP_GRANT != 0) {
                                match task.cspace[transfer_cap_idx] {
                                    Some(Capability::Endpoint(t_id, t_r)) => Some(Capability::Endpoint(t_id, t_r & (request.msg[1] as u8))),
                                    Some(Capability::Memory(p, sz)) => Some(Capability::Memory(p, sz)),
                                    Some(Capability::IOPort(p)) => Some(Capability::IOPort(p)),
                                    None => None,
                                }
                            } else { None };

                            match s.endpoints[ep_id].state {
                                EpState::Receiving(target_slot) => { 
                                    let target_task = (*tasks_ptr.add(target_slot)).as_mut().unwrap();
                                    let target_ptr = target_task.abi.ptr().add(4096).cast::<SyscallMailbox>();
                                    (*target_ptr).msg = request.msg; (*target_ptr).result = 0;
                                    
                                    let target_recv_idx = (*target_ptr).arg2;
                                    if target_recv_idx > 0 && target_recv_idx < 32 { target_task.cspace[target_recv_idx] = transfer_cap; }
                                    target_task.state = State::Ready; s.endpoints[ep_id].state = EpState::Idle; 0 
                                }
                                EpState::Idle => { s.endpoints[ep_id].state = EpState::Sending(slot); task.state = State::BlockedIpc; task.dirty = true; return s.select(sp, cpu); }
                                _ => usize::MAX - 1,
                            }
                        }
                    } else { usize::MAX }
                }
                SYSCALL_IPC_RECV => {
                    let cap_idx = request.arg1;
                    if cap_idx >= 32 { usize::MAX } else if let Some(Capability::Endpoint(ep_id, rights)) = task.cspace[cap_idx] {
                        if rights & crate::abi::CAP_READ == 0 { usize::MAX - 2 } else {
                            match s.endpoints[ep_id].state {
                                EpState::Sending(sender_slot) => { 
                                    let sender_task = (*tasks_ptr.add(sender_slot)).as_mut().unwrap();
                                    let sender_ptr = sender_task.abi.ptr().add(4096).cast::<SyscallMailbox>();
                                    (*ptr).msg = (*sender_ptr).msg; (*sender_ptr).result = 0; 

                                    let recv_idx = request.arg2; let transfer_cap_idx = (*sender_ptr).msg[0];
                                    if recv_idx > 0 && recv_idx < 32 && transfer_cap_idx > 0 && transfer_cap_idx < 32 {
                                        if let Some(cap) = sender_task.cspace[transfer_cap_idx] {
                                            let new_cap = match cap { Capability::Endpoint(s_id, s_r) => Capability::Endpoint(s_id, s_r & (*sender_ptr).msg[1] as u8), Capability::Memory(p, sz) => Capability::Memory(p, sz), Capability::IOPort(p) => Capability::IOPort(p) };
                                            task.cspace[recv_idx] = Some(new_cap);
                                        }
                                    }
                                    sender_task.state = State::Ready; s.endpoints[ep_id].state = EpState::Idle; 0 
                                }
                                EpState::Idle => { s.endpoints[ep_id].state = EpState::Receiving(slot); task.state = State::BlockedIpc; task.dirty = true; return s.select(sp, cpu); }
                                _ => usize::MAX - 1, 
                            }
                        }
                    } else { usize::MAX }
                }
                _ => usize::MAX,
            };
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*ptr).result), result); sp
        })
    }
}

unsafe fn serial_hex(number: u64) { for shift in (0..16).rev() { let digit = ((number >> (shift * 4)) & 15) as u8; serial_write_byte(if digit < 10 { b'0' + digit } else { b'A' + digit - 10 }); } }
pub fn faults() -> [Option<Fault>; 16] { locked(|| unsafe { scheduler().faults }) }
unsafe fn serial_number(mut number: u64) { let mut buffer = [0; 20]; let mut at = buffer.len(); loop { at -= 1; buffer[at] = b'0' + (number % 10) as u8; number /= 10; if number == 0 { break; } } for &byte in &buffer[at..] { serial_write_byte(byte); } }
pub fn foreground() -> u64 { locked(|| unsafe { let s = scheduler(); s.tasks[s.foreground].as_ref().map_or(0, |t| t.pid) }) }
pub fn focus(pid: u64) -> Result<(), &'static str> { locked(|| unsafe { let s = scheduler(); let slot = s.find(pid).ok_or("NO SUCH PID")?; s.focus(slot); Ok(()) }) }
pub fn kill(pid: u64) -> Result<(), &'static str> { locked(|| unsafe { let s = scheduler(); let slot = s.find(pid).ok_or("NO SUCH PID")?; s.tasks[slot].as_mut().unwrap().state = State::Exited; if slot == s.foreground { s.focus(0); } Ok(()) }) }
pub struct Summary { pub pid: u64, pub name: &'static str, pub state: &'static str, pub foreground: bool, pub runs: u64, pub ticks: u64, pub calls: u64, pub cpu: usize }
pub fn summaries() -> [Option<Summary>; MAX_TASKS] { locked(|| unsafe { let s = scheduler(); core::array::from_fn(|i| { s.tasks[i + 1].as_ref().map(|t| Summary { pid: t.pid, name: PROGRAM_NAMES[t.program], state: if s.current.contains(&(i + 1)) { "RUNNING" } else { t.state.label() }, foreground: s.foreground == i + 1, runs: t.runs, ticks: t.ticks, calls: t.calls, cpu: t.cpu }) }) }) }
pub fn logs(pid: u64, buffer: &mut [u8]) -> Result<usize, &'static str> { locked(|| unsafe { let s = scheduler(); let slot = s.find(pid).ok_or("NO SUCH PID")?; let log = &mut s.tasks[slot].as_mut().unwrap().log; let mut len = 0; while len < buffer.len() { let Some(byte) = log.pop() else { break; }; buffer[len] = byte; len += 1; } Ok(len) }) }
pub fn input() -> Option<u8> { locked(|| unsafe { let s = scheduler(); s.poll_input(); s.shell_input.pop() }) }
pub fn notice() -> Option<(u64, bool)> { locked(|| unsafe { scheduler().notice.take() }) }
pub fn dirty() { locked(|| unsafe { scheduler().dirty = true; }); }
pub fn service() {
    locked(|| unsafe {
        let s = scheduler(); debug_assert_eq!(s.current[0], 0);
        for (index, task) in s.tasks.iter_mut().enumerate().skip(1) { if !s.current.contains(&index) && task.as_ref().is_some_and(|t| t.state == State::Exited) { *task = None; } }
        let source = if s.foreground == 0 { if !s.dirty { return; } s.shell_screen.ptr() } else { if s.current.contains(&s.foreground) { return; } let task = s.tasks[s.foreground].as_mut().unwrap(); if !s.dirty && !task.dirty { return; } task.dirty = false; task.screen.ptr() } as *const u32;
        let shadow = s.shadow.ptr() as *mut u32;
        for y in 0..s.boot.height { for x in 0..s.boot.width { let offset = y * s.boot.stride + x; let pixel = core::ptr::read_volatile(source.add(offset)); if pixel != core::ptr::read(shadow.add(offset)) || !s.shadow_valid { core::ptr::write_volatile(s.boot.fb_ptr.add(offset), pixel); core::ptr::write(shadow.add(offset), pixel); } } }
        s.dirty = false; s.shadow_valid = true;
    });
}
pub fn idle() { interrupts::without(|| unsafe { let ready = locked(|| { scheduler().states(cpu::id())[1..].iter().any(|s| *s == State::Ready) }); if ready { asm!("int 0x80"); } else { asm!("sti", "hlt", "cli"); } }); }
