#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

use alloc::format;
use core::alloc::{GlobalAlloc, Layout};
use core::arch::asm;
use core::ffi::c_void;
use core::panic::PanicInfo;
use linked_list_allocator::LockedHeap;

static ALLOCATOR: LockedHeap = LockedHeap::empty();
// Never let a timer preempt a shell allocation while it holds the allocator
// lock: another CPU can hold the scheduler lock while waiting for that same
// allocator. All direct ALLOCATOR.lock() users must also have local IRQs off.
struct IrqAllocator;
#[global_allocator]
static GLOBAL_ALLOCATOR: IrqAllocator = IrqAllocator;
unsafe impl GlobalAlloc for IrqAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        interrupts::without(|| ALLOCATOR.alloc(layout))
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        interrupts::without(|| ALLOCATOR.dealloc(ptr, layout));
    }
}
#[path = "../../common/abi.rs"]
mod abi;
use abi::BootInfo;
mod context;
mod cpu;
mod elf;
#[path = "../../bootloader/src/elf_reloc.rs"]
mod elf_reloc;
mod input;
mod interrupts;
mod memory;
mod paging;
mod scheduler;
mod task_state;
mod user_heap;

unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}
unsafe fn inb(port: u16) -> u8 {
    let mut val: u8;
    asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack));
    val
}

const COM1: u16 = 0x3F8;
unsafe fn init_serial() {
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x80);
    outb(COM1 + 0, 0x03);
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x03);
    outb(COM1 + 2, 0xC7);
    outb(COM1 + 4, 0x0B);
}
unsafe fn serial_has_data() -> bool {
    (inb(COM1 + 5) & 1) != 0
}
unsafe fn serial_read_byte() -> u8 {
    if serial_has_data() {
        inb(COM1)
    } else {
        0
    }
}
unsafe fn serial_is_transmit_empty() -> bool {
    (inb(COM1 + 5) & 0x20) != 0
}
unsafe fn serial_write_byte(b: u8) {
    while !serial_is_transmit_empty() {}
    outb(COM1, b);
}

#[path = "../../common/font.rs"]
mod font;
use font::FONT;

struct Console {
    fb: *mut u32,
    width: usize,
    height: usize,
    stride: usize,
    cx: usize,
    cy: usize,
    bg_color: u32,
    fg_color: u32,
}
impl Console {
    fn scroll(&mut self) {
        let limit = self.height - 10;
        for y in 10..limit {
            for x in 0..self.width {
                unsafe {
                    core::ptr::write_volatile(
                        self.fb.add((y - 10) * self.stride + x),
                        core::ptr::read_volatile(self.fb.add(y * self.stride + x)),
                    );
                }
            }
        }
        for y in (limit - 10)..limit {
            for x in 0..self.width {
                unsafe {
                    core::ptr::write_volatile(self.fb.add(y * self.stride + x), self.bg_color);
                }
            }
        }
        self.cy -= 10;
    }
    fn print_char(&mut self, ch: u8) {
        if ch == b'\r' {
            return;
        }
        scheduler::dirty();
        unsafe {
            if ch == b'\n' {
                serial_write_byte(b'\r');
            }
            serial_write_byte(ch);
        }
        if ch == b'\n' {
            self.cx = 0;
            self.cy += 10;
        } else if ch == 0x08 {
            if self.cx >= 8 {
                self.cx -= 8;
            }
            for row in 0..8 {
                for col in 0..8 {
                    unsafe {
                        core::ptr::write_volatile(
                            self.fb.add((self.cy + row) * self.stride + (self.cx + col)),
                            self.bg_color,
                        );
                    }
                }
            }
        } else {
            let idx = if ch >= 32 && ch <= 95 {
                (ch - 32) as usize
            } else if ch >= 97 && ch <= 122 {
                (ch - 97 + 33) as usize
            } else {
                0
            };
            let bitmap = FONT[idx];
            for row in 0..8 {
                let row_data = (bitmap >> ((7 - row) * 8)) & 0xFF;
                for col in 0..8 {
                    let color = if (row_data & (1 << (7 - col))) != 0 {
                        self.fg_color
                    } else {
                        self.bg_color
                    };
                    unsafe {
                        core::ptr::write_volatile(
                            self.fb.add((self.cy + row) * self.stride + (self.cx + col)),
                            color,
                        );
                    }
                }
            }
            self.cx += 8;
        }
        if self.cx >= self.width {
            self.cx = 0;
            self.cy += 10;
        }
        if self.cy >= self.height - 10 {
            self.scroll();
        }
    }
    fn print(&mut self, s: &str) {
        for b in s.bytes() {
            self.print_char(b);
        }
    }
    fn clear(&mut self) {
        scheduler::dirty();
        for y in 0..self.height {
            for x in 0..self.width {
                unsafe {
                    core::ptr::write_volatile(self.fb.add(y * self.stride + x), self.bg_color);
                }
            }
        }
        self.cx = 0;
        self.cy = 0;
    }
}

fn streq(a: &[u8], b: &[u8]) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn list_programs(term: &mut Console) {
    term.print("PROGRAMS:\n");
    term.print("  app   - ROTATING SQUARE\n");
    term.print("  app2  - BOUNCING SQUARE AND COUNTERS\n");
    term.print("  clock - DIGITAL CLOCK\n");
    term.print("  dzen-clock - FIVE COLOR TIME INDICATORS\n");
    term.print("  ping  - IPC CLIENT (SHARED MEMORY)\n");
    term.print("  pong  - IPC SERVER (SUPERVISOR)\n");
    term.print("USE: RUN <NAME> [&]. CTRL+Z: BACKGROUND. ESC: EXIT.\n");
}

#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut c_void, c: i32, n: usize) -> *mut c_void {
    let s_u8 = s as *mut u8;
    for i in 0..n {
        core::ptr::write_volatile(s_u8.add(i), c as u8);
    }
    s
}
#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let d_u8 = dest as *mut u8;
    let s_u8 = src as *const u8;
    for i in 0..n {
        core::ptr::write_volatile(d_u8.add(i), core::ptr::read_volatile(s_u8.add(i)));
    }
    dest
}
#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 {
    let s1_u8 = s1 as *const u8;
    let s2_u8 = s2 as *const u8;
    for i in 0..n {
        let a = core::ptr::read_volatile(s1_u8.add(i));
        let b = core::ptr::read_volatile(s2_u8.add(i));
        if a != b {
            return (a as i32) - (b as i32);
        }
    }
    0
}

fn pid_arg(args: &[u8]) -> Option<u64> {
    if args.is_empty() {
        return None;
    }
    let mut pid = 0u64;
    for &byte in args {
        if !byte.is_ascii_digit() {
            return None;
        }
        pid = pid.checked_mul(10)?.checked_add((byte - b'0') as u64)?;
    }
    (pid != 0).then_some(pid)
}

fn report(term: &mut Console, error: &str) {
    term.print("ERROR: ");
    term.print(error);
    term.print("\n");
}

fn command(term: &mut Console, line: &[u8]) {
    let line = line.trim_ascii();
    let split = line
        .iter()
        .position(|b| b.is_ascii_whitespace())
        .unwrap_or(line.len());
    let cmd = &line[..split];
    let args = line[split..].trim_ascii();
    if cmd.is_empty() {
        return;
    }
    if streq(cmd, b"run") || streq(cmd, b"boot") {
        let (name, background) = if streq(cmd, b"boot") {
            if !args.is_empty() {
                report(term, "BOOT TAKES NO ARGUMENTS");
                return;
            }
            (&b"app"[..], false)
        } else if let Some(name) = args.strip_suffix(b"&") {
            (name.trim_ascii(), true)
        } else {
            (args, false)
        };
        if name.is_empty() {
            term.print("USAGE: RUN <NAME> [&]\n");
            list_programs(term);
            return;
        }
        let Some(program) = scheduler::PROGRAM_NAMES
            .iter()
            .position(|p| streq(name, p.as_bytes()))
        else {
            report(term, "UNKNOWN PROGRAM. TYPE LIST TO SEE PROGRAMS.");
            return;
        };
        match scheduler::spawn(program, background) {
            Ok(pid) => term.print(&format!(
                "STARTED PID={} NAME={} {}\n",
                pid,
                scheduler::PROGRAM_NAMES[program],
                if background {
                    "BACKGROUND"
                } else {
                    "FOREGROUND"
                }
            )),
            Err(error) => report(term, error),
        }
    } else if streq(cmd, b"fg") || streq(cmd, b"kill") || streq(cmd, b"logs") {
        let Some(pid) = pid_arg(args) else {
            report(term, "EXPECTED ONE POSITIVE PID");
            return;
        };
        if streq(cmd, b"fg") {
            match scheduler::focus(pid) {
                Ok(()) => term.print(&format!(
                    "FOREGROUND PID={} (CTRL+Z: SHELL, ESC: EXIT)\n",
                    pid
                )),
                Err(error) => report(term, error),
            }
        } else if streq(cmd, b"kill") {
            match scheduler::kill(pid) {
                Ok(()) => term.print(&format!("KILLED PID={}\n", pid)),
                Err(error) => report(term, error),
            }
        } else {
            let mut buffer = [0; 4096];
            match scheduler::logs(pid, &mut buffer) {
                Ok(len) => {
                    term.print(&format!("LOGS PID={} (LAST 4096 BYTES, DRAINED):\n", pid));
                    for &byte in &buffer[..len] {
                        term.print_char(byte);
                    }
                    term.print("\nEND LOGS\n");
                }
                Err(error) => report(term, error),
            }
        }
    } else if !args.is_empty() {
        report(term, "THIS COMMAND TAKES NO ARGUMENTS");
    } else if streq(cmd, b"help") {
        term.print("- list: programs\n- run <name> [&]: new instance\n- boot: run app\n- cpus: online processors\n- faults: recent process faults\n- ps: tasks\n- fg <id>: foreground\n- kill <id>: terminate\n- logs <id>: buffered output\n- heap\n- clear\n- stop\nCTRL+Z: SHELL, KEEP RUNNING. ESC: EXIT FOREGROUND APP.\n");
    } else if streq(cmd, b"list") {
        list_programs(term);
    } else if streq(cmd, b"cpus") {
        use core::sync::atomic::Ordering;
        for i in 0..cpu::COUNT.load(Ordering::Acquire) {
            term.print(&format!(
                "CPU={} APIC={} ONLINE={} TICKS={}\n",
                i,
                cpu::apic_id(i),
                cpu::ONLINE[i].load(Ordering::Acquire),
                cpu::TICKS[i].load(Ordering::Relaxed)
            ));
        }
    } else if streq(cmd, b"faults") {
        for fault in scheduler::faults().into_iter().flatten() {
            term.print(&format!(
                "FAULT PID={} CPU={} VECTOR={} ERROR={:#x} RIP={:#x} ADDR={:#x}\n",
                fault.pid, fault.cpu, fault.vector, fault.error, fault.rip, fault.address
            ));
        }
    } else if streq(cmd, b"ps") {
        term.print("PID NAME STATE FOCUS CPU RUNS CPU_TICKS SYSCALLS\n");
        let mut count = 0;
        for task in scheduler::summaries().into_iter().flatten() {
            term.print(&format!(
                "{} {} {} {} {} {} {} {}\n",
                task.pid,
                task.name,
                task.state,
                if task.foreground { "FG" } else { "BG" },
                task.cpu,
                task.runs,
                task.ticks,
                task.calls
            ));
            count += 1;
        }
        term.print(&format!(
            "{} TASK(S); SHELL PID=0; LIMIT={}\n",
            count,
            scheduler::MAX_TASKS
        ));
    } else if streq(cmd, b"clear") {
        term.clear();
    } else if streq(cmd, b"stop") {
        term.print("SYSTEM HALTED. CPU GOING TO SLEEP...\n");
        scheduler::service();
        cpu::halt_all();
    } else if streq(cmd, b"heap") {
        let (used, free, freed) = scheduler::heap_test();
        term.print(&format!(
            "Dynamic allocation works! Uptime: {} ms\n",
            interrupts::milliseconds()
        ));
        term.print(&format!(
            "HEAP: USED={} FREE={} TEST FREED={}\n",
            used, free, freed
        ));
    } else {
        report(term, "UNKNOWN COMMAND");
    }
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo) -> ! {
    let fb;
    unsafe {
        asm!("cli");
        init_serial();
        ALLOCATOR.lock().init(info.heap_ptr, info.heap_len);
        paging::init().expect("Kernel page tables");
        cpu::prepare(info).expect("CPU state");
        fb = scheduler::init(info).expect("Scheduler init failed"); scheduler::spawn(6, true).expect("RTC spawn");
        interrupts::init();
        cpu::start(info);
    }
    let mut term = Console {
        fb,
        width: info.width,
        height: info.height,
        stride: info.stride,
        cx: 0,
        cy: 0,
        bg_color: 0x001E1E2E,
        fg_color: 0x00A6E3A1,
    };
    term.clear();
    term.print("MIND CORE v1.4 [Build: 2026-09-20]. SMP / RING 3 ELF SHELL.\n");
    term.print(&format!(
        "MEMORY MANAGER: {} MB HEAP.\n",
        info.heap_len / 1024 / 1024
    ));
    term.print("LIST: PROGRAMS. RUN <NAME> [&]. PS. FG <ID>. HELP.\n");
    term.print("MIND> ");
    let mut input_buf = [0u8; 128];
    let mut input_len = 0;
    loop {
        if let Some((pid, exited)) = scheduler::notice() {
            term.print(&format!(
                "\nPID={} {}. SHELL RESUMED.\n",
                pid,
                if exited { "EXITED" } else { "BACKGROUND" }
            ));
            term.print("MIND> ");
        }
        if scheduler::foreground() == 0 {
            if let Some(byte) = scheduler::input() {
                match byte {
                    8 if input_len != 0 => {
                        input_len -= 1;
                        term.print_char(8);
                    }
                    b'\n' => {
                        term.print("\n");
                        command(&mut term, &input_buf[..input_len]);
                        input_len = 0;
                        if scheduler::foreground() == 0 {
                            term.print("MIND> ");
                        }
                    }
                    32..=126 if input_len < input_buf.len() => {
                        input_buf[input_len] = byte;
                        input_len += 1;
                        term.print_char(byte);
                    }
                    _ => {}
                }
                // Drain a pasted line before sleeping; ready programs still get
                // a time slice at every iteration through idle().
            }
        }
        scheduler::service();
        scheduler::idle();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        asm!("cli");
        for &b in b"KERNEL PANIC\r\n" {
            serial_write_byte(b);
        }
        cpu::halt_all();
    }
}
