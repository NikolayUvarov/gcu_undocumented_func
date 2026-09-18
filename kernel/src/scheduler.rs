use crate::abi::{
    BootInfo, ProgramImage, SyscallMailbox, RTC_UNAVAILABLE, SYSCALL_EXIT, SYSCALL_RTC_TIME,
    SYSCALL_UPTIME, SYSCALL_WAIT,
};
use crate::input::{Keyboard, Queue};
use crate::memory::Region;
use crate::task_state::{self, State};
use crate::{context, elf, interrupts, outb, rtc, serial_write_byte};
use core::arch::asm;

pub const MAX_TASKS: usize = 8;
const SLOTS: usize = MAX_TASKS + 1;
const STACK_SIZE: usize = 64 * 1024;
pub const PROGRAM_NAMES: [&str; 3] = ["app", "app2", "clock"];

#[repr(C)]
struct TaskAbi {
    info: BootInfo,
    mailbox: SyscallMailbox,
}

struct Task {
    pid: u64,
    program: usize,
    state: State,
    sp: usize,
    entry: usize,
    runs: u64,
    ticks: u64,
    calls: u64,
    _image: Region,
    _stack: Region,
    screen: Region,
    // Keep application-visible memory out of the scheduler's borrowed struct:
    // the application retains &BootInfo while interrupt handlers mutate its TCB.
    abi: Region,
    input: Queue<128>,
    log: Queue<4096>,
    log_line_start: bool,
    dirty: bool,
}

struct Scheduler {
    boot: BootInfo,
    tasks: [Option<Task>; SLOTS],
    current: usize,
    shell_sp: usize,
    next_pid: u64,
    foreground: usize,
    keyboard: Keyboard,
    shell_input: Queue<128>,
    shell_screen: Region,
    shadow: Region,
    dirty: bool,
    shadow_valid: bool,
    notice: Option<(u64, bool)>,
}

static mut SCHEDULER: Option<Scheduler> = None;

// Single BSP only. All callers hold IF=0. No references survive switching stacks.
unsafe fn scheduler() -> &'static mut Scheduler {
    (*core::ptr::addr_of_mut!(SCHEDULER)).as_mut().unwrap()
}

pub fn init(info: &BootInfo) -> Result<*mut u32, &'static str> {
    let bytes = info
        .stride
        .checked_mul(info.height)
        .and_then(|n| n.checked_mul(4))
        .ok_or("FRAMEBUFFER SIZE OVERFLOW")?;
    let shell_screen = Region::new(bytes, 16)?;
    let fb = shell_screen.ptr().cast();
    let shadow = Region::new(bytes, 16)?;
    unsafe {
        *core::ptr::addr_of_mut!(SCHEDULER) = Some(Scheduler {
            boot: *info,
            tasks: core::array::from_fn(|_| None),
            current: 0,
            shell_sp: 0,
            next_pid: 1,
            foreground: 0,
            keyboard: Keyboard::new(),
            shell_input: Queue::new(),
            shell_screen,
            shadow,
            dirty: true,
            shadow_valid: false,
            notice: None,
        });
    }
    Ok(fb)
}

impl Scheduler {
    fn states(&self) -> [State; SLOTS] {
        core::array::from_fn(|i| {
            if i == 0 {
                State::Ready
            } else {
                self.tasks[i].as_ref().map_or(State::Empty, |t| t.state)
            }
        })
    }

    fn select(&mut self, sp: usize) -> usize {
        if self.current == 0 {
            self.shell_sp = sp;
        } else {
            self.tasks[self.current].as_mut().unwrap().sp = sp;
        }
        let next = task_state::next(&self.states(), self.current);
        self.current = next;
        if next == 0 {
            self.shell_sp
        } else {
            let task = self.tasks[next].as_mut().unwrap();
            task.runs += 1;
            task.sp
        }
    }

    fn focus(&mut self, slot: usize) {
        // Already buffered input belongs to the old foreground, never to the
        // newly selected task (or the command shell).
        if self.foreground != 0 {
            if let Some(task) = self.tasks[self.foreground].as_mut() {
                task.input.clear();
            }
        }
        self.shell_input.clear();
        if slot != 0 {
            self.tasks[slot].as_mut().unwrap().input.clear();
        }
        self.foreground = slot;
        self.dirty = true;
    }

    fn poll_input(&mut self) {
        for _ in 0..32 {
            let Some(key) = self.keyboard.read() else {
                break;
            };
            if key.background && self.foreground != 0 {
                let pid = self.tasks[self.foreground].as_ref().unwrap().pid;
                self.focus(0);
                self.notice = Some((pid, false));
            } else if self.foreground == 0 {
                if !key.background {
                    self.shell_input.push(key.shell);
                }
            } else if key.app != 0 {
                let task = self.tasks[self.foreground].as_mut().unwrap();
                task.input.push(key.app);
                if matches!(task.state, State::Sleeping(_)) {
                    task.state = State::Ready;
                }
            }
        }
    }

    fn exit_current(&mut self) {
        let task = self.tasks[self.current].as_mut().unwrap();
        let pid = task.pid;
        task.state = State::Exited;
        if self.foreground == self.current {
            self.focus(0);
            self.notice = Some((pid, true));
        }
    }

    fn find(&self, pid: u64) -> Option<usize> {
        (1..SLOTS).find(|&i| {
            self.tasks[i]
                .as_ref()
                .is_some_and(|t| t.pid == pid && t.state != State::Exited)
        })
    }
}

pub fn spawn(program: usize, background: bool) -> Result<u64, &'static str> {
    interrupts::without(|| unsafe {
        let s = scheduler();
        let slot = (1..SLOTS)
            .find(|&i| s.tasks[i].is_none())
            .ok_or("TASK LIMIT REACHED (8)")?;
        let pid = s.next_pid;
        let next_pid = pid.checked_add(1).ok_or("PID SPACE EXHAUSTED")?;
        let source = s.boot.programs.get(program).ok_or("UNKNOWN PROGRAM")?;
        let file = core::slice::from_raw_parts(source.data, source.len);
        let elf = elf::Image::parse(file)?;
        let mut image = Region::new(elf.size, 4096)?;
        let base = image.ptr() as usize;
        let entry = elf.load(image.bytes_mut(), base)?;
        let stack = Region::new(STACK_SIZE, 16)?;
        let screen = Region::new(s.shell_screen.len(), 16)?;
        let mut info = s.boot;
        info.fb_ptr = screen.ptr().cast();
        // Programs see their own surface; no allocator/ELF catalogue is exposed
        // through the ABI. Ring 0 still provides no hardware memory protection.
        info.heap_ptr = core::ptr::null_mut();
        info.heap_len = 0;
        info.programs = [ProgramImage {
            data: core::ptr::null(),
            len: 0,
        }; 3];
        let abi = Region::new(
            core::mem::size_of::<TaskAbi>(),
            core::mem::align_of::<TaskAbi>(),
        )?;
        (abi.ptr() as *mut TaskAbi).write(TaskAbi {
            info,
            mailbox: SyscallMailbox::EMPTY,
        });
        let sp = context::initial(
            stack.ptr() as usize + STACK_SIZE,
            task_start as *const () as usize,
        );
        s.tasks[slot] = Some(Task {
            pid,
            program,
            state: State::Ready,
            sp,
            entry,
            runs: 0,
            ticks: 0,
            calls: 0,
            _image: image,
            _stack: stack,
            screen,
            abi,
            input: Queue::new(),
            log: Queue::new(),
            log_line_start: true,
            dirty: true,
        });
        s.next_pid = next_pid;
        if !background {
            s.focus(slot);
        }
        Ok(pid)
    })
}

extern "C" fn task_start() -> ! {
    let (entry, info, mailbox) = interrupts::without(|| unsafe {
        let s = scheduler();
        let task = s.tasks[s.current].as_mut().unwrap();
        let abi = task.abi.ptr() as *mut TaskAbi;
        (
            task.entry,
            core::ptr::addr_of!((*abi).info),
            core::ptr::addr_of_mut!((*abi).mailbox),
        )
    });
    let program: extern "sysv64" fn(*const BootInfo, *mut SyscallMailbox) =
        unsafe { core::mem::transmute(entry) };
    program(info, mailbox);
    unsafe {
        (*mailbox).syscall_num = SYSCALL_EXIT;
        asm!("int 0x80");
        core::hint::unreachable_unchecked();
    }
}

// Interrupt gates disable IF. Kernel code is not preempted, so allocator locks
// and scheduler data never get re-entered. Applications are preempted each tick.
pub extern "C" fn timer_interrupt(sp: usize) -> usize {
    interrupts::advance();
    unsafe {
        outb(0x20, 0x20);
        let s = scheduler();
        let now = interrupts::milliseconds();
        for task in s.tasks.iter_mut().flatten() {
            task.state.wake(now);
        }
        s.poll_input();
        if s.current == 0 {
            return sp;
        }
        let task = s.tasks[s.current].as_mut().unwrap();
        task.ticks += 1;
        task.dirty = true;
        s.select(sp)
    }
}

pub extern "C" fn syscall_interrupt(sp: usize) -> usize {
    unsafe {
        let s = scheduler();
        s.poll_input();
        if s.current == 0 {
            return s.select(sp);
        }
        let slot = s.current;
        let task = s.tasks[slot].as_mut().unwrap();
        task.calls += 1;
        let mb = &mut *core::ptr::addr_of_mut!((*(task.abi.ptr() as *mut TaskAbi)).mailbox);
        match mb.syscall_num {
            1 => {
                let lo: u32;
                let hi: u32;
                asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack));
                mb.result = (((hi as u64) << 32) | lo as u64) as usize;
            }
            2 => {
                mb.result = task.input.pop().unwrap_or(0) as usize;
            }
            3 => {
                // Ring-0 ABI: pointers are trusted. Bound output so a single
                // syscall cannot monopolize an unbounded interval with IF=0.
                let length = mb.arg2.min(4096);
                for i in 0..length {
                    let byte = core::ptr::read_volatile((mb.arg1 as *const u8).add(i));
                    task.log.push(byte);
                    if s.foreground == slot {
                        if task.log_line_start && byte != b'\r' && byte != b'\n' {
                            for b in b"[PID " {
                                serial_write_byte(*b);
                            }
                            serial_number(task.pid);
                            for b in b"] " {
                                serial_write_byte(*b);
                            }
                        }
                        serial_write_byte(byte);
                    }
                    task.log_line_start = byte == b'\n';
                }
                mb.result = length;
            }
            SYSCALL_RTC_TIME => {
                mb.result = rtc::read_time().unwrap_or(RTC_UNAVAILABLE);
            }
            SYSCALL_UPTIME => {
                mb.result = interrupts::milliseconds() as usize;
            }
            SYSCALL_WAIT => {
                let now = interrupts::milliseconds();
                let duration = mb.arg1.min(60_000).div_ceil(10).max(1) as u64 * 10;
                mb.result = now as usize;
                task.state = if task.input.is_empty() {
                    State::Sleeping(now.wrapping_add(duration))
                } else {
                    State::Ready
                };
                task.dirty = true;
                return s.select(sp);
            }
            SYSCALL_EXIT => {
                s.exit_current();
                return s.select(sp);
            }
            _ => {
                mb.result = usize::MAX;
            }
        }
        sp
    }
}

unsafe fn serial_number(mut number: u64) {
    let mut buffer = [0; 20];
    let mut at = buffer.len();
    loop {
        at -= 1;
        buffer[at] = b'0' + (number % 10) as u8;
        number /= 10;
        if number == 0 {
            break;
        }
    }
    for &byte in &buffer[at..] {
        serial_write_byte(byte);
    }
}

pub fn foreground() -> u64 {
    interrupts::without(|| unsafe {
        let s = scheduler();
        s.tasks[s.foreground].as_ref().map_or(0, |t| t.pid)
    })
}

pub fn focus(pid: u64) -> Result<(), &'static str> {
    interrupts::without(|| unsafe {
        let s = scheduler();
        let slot = s.find(pid).ok_or("NO SUCH PID")?;
        s.focus(slot);
        Ok(())
    })
}

pub fn kill(pid: u64) -> Result<(), &'static str> {
    interrupts::without(|| unsafe {
        let s = scheduler();
        let slot = s.find(pid).ok_or("NO SUCH PID")?;
        s.tasks[slot].as_mut().unwrap().state = State::Exited;
        if slot == s.foreground {
            s.focus(0);
        }
        Ok(())
    })
}

pub struct Summary {
    pub pid: u64,
    pub name: &'static str,
    pub state: &'static str,
    pub foreground: bool,
    pub runs: u64,
    pub ticks: u64,
    pub calls: u64,
}
pub fn summaries() -> [Option<Summary>; MAX_TASKS] {
    interrupts::without(|| unsafe {
        let s = scheduler();
        core::array::from_fn(|i| {
            s.tasks[i + 1].as_ref().map(|t| Summary {
                pid: t.pid,
                name: PROGRAM_NAMES[t.program],
                state: t.state.label(),
                foreground: s.foreground == i + 1,
                runs: t.runs,
                ticks: t.ticks,
                calls: t.calls,
            })
        })
    })
}

pub fn logs(pid: u64, buffer: &mut [u8]) -> Result<usize, &'static str> {
    interrupts::without(|| unsafe {
        let s = scheduler();
        let slot = s.find(pid).ok_or("NO SUCH PID")?;
        let log = &mut s.tasks[slot].as_mut().unwrap().log;
        let mut len = 0;
        while len < buffer.len() {
            let Some(byte) = log.pop() else {
                break;
            };
            buffer[len] = byte;
            len += 1;
        }
        Ok(len)
    })
}

pub fn input() -> Option<u8> {
    interrupts::without(|| unsafe {
        let s = scheduler();
        s.poll_input();
        s.shell_input.pop()
    })
}
pub fn notice() -> Option<(u64, bool)> {
    interrupts::without(|| unsafe { scheduler().notice.take() })
}
pub fn dirty() {
    interrupts::without(|| unsafe {
        scheduler().dirty = true;
    });
}

pub fn service() {
    interrupts::without(|| unsafe {
        let s = scheduler();
        debug_assert_eq!(s.current, 0);
        // We are now on the shell stack, after IRET restored its context.
        for task in &mut s.tasks[1..] {
            if task.as_ref().is_some_and(|t| t.state == State::Exited) {
                *task = None;
            }
        }
        let source = if s.foreground == 0 {
            if !s.dirty {
                return;
            }
            s.shell_screen.ptr()
        } else {
            let task = s.tasks[s.foreground].as_mut().unwrap();
            if !s.dirty && !task.dirty {
                return;
            }
            task.dirty = false;
            task.screen.ptr()
        } as *const u32;
        let shadow = s.shadow.ptr() as *mut u32;
        for y in 0..s.boot.height {
            for x in 0..s.boot.width {
                let offset = y * s.boot.stride + x;
                let pixel = core::ptr::read(source.add(offset));
                if pixel != core::ptr::read(shadow.add(offset)) || !s.shadow_valid {
                    core::ptr::write_volatile(s.boot.fb_ptr.add(offset), pixel);
                    core::ptr::write(shadow.add(offset), pixel);
                }
            }
        }
        s.dirty = false;
        s.shadow_valid = true;
    });
}

pub fn idle() {
    // Check and sleep atomically. A timer arriving between this check and HLT
    // must not leave a newly runnable task asleep until some later interrupt.
    interrupts::without(|| unsafe {
        let ready = scheduler().states()[1..].iter().any(|s| *s == State::Ready);
        if ready {
            // PID 0 uses int 0x80 only to yield; it has no application mailbox.
            asm!("int 0x80");
        } else {
            asm!("sti", "hlt", "cli");
        }
    });
}
