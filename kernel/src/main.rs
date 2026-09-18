#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc; 

use core::arch::asm;
use core::panic::PanicInfo;
use core::ffi::c_void;
use linked_list_allocator::LockedHeap;
use alloc::format;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();
static mut HEAP_MEMORY: [u8; 1024 * 1024] = [0; 1024 * 1024];

#[path = "../../common/abi.rs"]
mod abi;
use abi::{BootInfo, SyscallMailbox, SYSCALL_RTC_TIME, RTC_UNAVAILABLE};
mod rtc;

unsafe fn outb(port: u16, val: u8) { asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack)); }
unsafe fn inb(port: u16) -> u8 { let mut val: u8; asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack)); val }

const COM1: u16 = 0x3F8;
unsafe fn init_serial() { outb(COM1 + 1, 0x00); outb(COM1 + 3, 0x80); outb(COM1 + 0, 0x03); outb(COM1 + 1, 0x00); outb(COM1 + 3, 0x03); outb(COM1 + 2, 0xC7); outb(COM1 + 4, 0x0B); }
unsafe fn serial_has_data() -> bool { (inb(COM1 + 5) & 1) != 0 }
unsafe fn serial_read_byte() -> u8 { if serial_has_data() { inb(COM1) } else { 0 } }
unsafe fn serial_is_transmit_empty() -> bool { (inb(COM1 + 5) & 0x20) != 0 }
unsafe fn serial_write_byte(b: u8) { while !serial_is_transmit_empty() {} outb(COM1, b); }

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct IdtEntry { offset_low: u16, selector: u16, ist: u8, type_attr: u8, offset_mid: u16, offset_high: u32, zero: u32 }
#[repr(C, packed)]
struct IdtPtr { limit: u16, base: u64 }
#[repr(C)]
pub struct InterruptFrame { pub rip: u64, pub cs: u64, pub rflags: u64, pub rsp: u64, pub ss: u64 }

static mut IDT: [IdtEntry; 256] = [IdtEntry { offset_low: 0, selector: 0, ist: 0, type_attr: 0, offset_mid: 0, offset_high: 0, zero: 0 }; 256];
static mut MAILBOX: SyscallMailbox = SyscallMailbox { syscall_num: 0, arg1: 0, arg2: 0, result: 0 };
static mut SYSTEM_TICKS: usize = 0;
static mut LAST_KEY: u8 = 0;

extern "x86-interrupt" fn syscall_handler(_frame: &mut InterruptFrame) {
    unsafe {
        let mb = core::ptr::addr_of_mut!(MAILBOX);
        if (*mb).syscall_num == 1 {
            let mut lo: u32; let mut hi: u32; asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack));
            (*mb).result = (((hi as u64) << 32) | (lo as u64)) as usize;
        } else if (*mb).syscall_num == 2 {
            if serial_has_data() { (*mb).result = serial_read_byte() as usize; } 
            else {
                let mut status: u8; asm!("in al, 0x64", out("al") status, options(nomem, nostack));
                if (status & 1) == 1 { let mut scancode: u8; asm!("in al, 0x60", out("al") scancode, options(nomem, nostack)); (*mb).result = scancode as usize; } 
                else { (*mb).result = 0; }
            }
        } else if (*mb).syscall_num == 3 {
            let ptr = (*mb).arg1 as *const u8; let len = (*mb).arg2;
            for i in 0..len { serial_write_byte(core::ptr::read_volatile(ptr.add(i))); }
            (*mb).result = len;
        } else if (*mb).syscall_num == SYSCALL_RTC_TIME {
            (*mb).result = rtc::read_time().unwrap_or(RTC_UNAVAILABLE);
        }
    }
}

unsafe fn init_idt() {
    let mut cs: u16; asm!("mov ax, cs", out("ax") cs, options(nomem, nostack));
    let s_addr = syscall_handler as *const () as u64;
    IDT[128].offset_low = s_addr as u16; IDT[128].selector = cs; IDT[128].type_attr = 0x8E; 
    IDT[128].offset_mid = (s_addr >> 16) as u16; IDT[128].offset_high = (s_addr >> 32) as u32;
    let idt_ptr = IdtPtr { limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16, base: core::ptr::addr_of!(IDT) as u64 };
    asm!("lidt [{}]", in(reg) &idt_ptr);
}

#[path = "../../common/font.rs"]
mod font;
use font::FONT;

const SCANCODE_TO_ASCII: &[u8] = b"??1234567890-=?\tqwertyuiop[]\n?asdfghjkl;'`?\\zxcvbnm,./?*? ?";

struct Console { fb: *mut u32, width: usize, height: usize, stride: usize, cx: usize, cy: usize, bg_color: u32, fg_color: u32 }
impl Console {
    fn scroll(&mut self) {
        let limit = self.height - 10;
        for y in 10..limit { for x in 0..self.width { unsafe { core::ptr::write_volatile(self.fb.add((y - 10) * self.stride + x), core::ptr::read_volatile(self.fb.add(y * self.stride + x))); } } }
        for y in (limit - 10)..limit { for x in 0..self.width { unsafe { core::ptr::write_volatile(self.fb.add(y * self.stride + x), self.bg_color); } } }
        self.cy -= 10;
    }
    fn print_char(&mut self, ch: u8) {
        unsafe { if ch == b'\n' { serial_write_byte(b'\r'); } serial_write_byte(ch); }
        if ch == b'\n' { self.cx = 0; self.cy += 10; } 
        else if ch == 0x08 { 
            if self.cx >= 8 { self.cx -= 8; }
            for row in 0..8 { for col in 0..8 { unsafe { core::ptr::write_volatile(self.fb.add((self.cy + row) * self.stride + (self.cx + col)), self.bg_color); } } }
        } else {
            let idx = if ch >= 32 && ch <= 95 { (ch - 32) as usize } else if ch >= 97 && ch <= 122 { (ch - 97 + 33) as usize } else { 0 };
            let bitmap = FONT[idx];
            for row in 0..8 {
                let row_data = (bitmap >> ((7 - row) * 8)) & 0xFF;
                for col in 0..8 {
                    let color = if (row_data & (1 << (7 - col))) != 0 { self.fg_color } else { self.bg_color };
                    unsafe { core::ptr::write_volatile(self.fb.add((self.cy + row) * self.stride + (self.cx + col)), color); }
                }
            }
            self.cx += 8;
        }
        if self.cx >= self.width { self.cx = 0; self.cy += 10; }
        if self.cy >= self.height - 10 { self.scroll(); }
    }
    fn print(&mut self, s: &str) { for b in s.bytes() { self.print_char(b); } }
    fn clear(&mut self) {
        for y in 0..self.height { for x in 0..self.width { unsafe { core::ptr::write_volatile(self.fb.add(y * self.stride + x), self.bg_color); } } }
        self.cx = 0; self.cy = 0;
    }
}

fn streq(a: &[u8], b: &[u8]) -> bool { a.eq_ignore_ascii_case(b) }

fn list_programs(term: &mut Console) {
    term.print("PROGRAMS:\n");
    term.print("  app   - ROTATING SQUARE\n");
    term.print("  app2  - BOUNCING SQUARE AND COUNTERS\n");
    term.print("  clock - DIGITAL CLOCK\n");
    term.print("USE: RUN <NAME>. ESC RETURNS TO THE SHELL.\n");
}

fn launch_app(term: &mut Console, info: &BootInfo, entry: u64) {
    let app_entry: extern "sysv64" fn(&BootInfo, *mut SyscallMailbox) = unsafe { core::mem::transmute(entry as usize) };
    app_entry(info, core::ptr::addr_of_mut!(MAILBOX));
    term.clear();
    term.print("USERSPACE EXITED. KERNEL REPL RESUMED.\n");
}
#[no_mangle] pub unsafe extern "C" fn memset(s: *mut c_void, c: i32, n: usize) -> *mut c_void { let s_u8 = s as *mut u8; for i in 0..n { core::ptr::write_volatile(s_u8.add(i), c as u8); } s }
#[no_mangle] pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void { let d_u8 = dest as *mut u8; let s_u8 = src as *const u8; for i in 0..n { core::ptr::write_volatile(d_u8.add(i), core::ptr::read_volatile(s_u8.add(i))); } dest }
#[no_mangle] pub unsafe extern "C" fn memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 { let s1_u8 = s1 as *const u8; let s2_u8 = s2 as *const u8; for i in 0..n { let a = core::ptr::read_volatile(s1_u8.add(i)); let b = core::ptr::read_volatile(s2_u8.add(i)); if a != b { return (a as i32) - (b as i32); } } 0 }

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo) -> ! {
    unsafe { 
        init_serial(); 
        init_idt(); 
        ALLOCATOR.lock().init(HEAP_MEMORY.as_mut_ptr(), HEAP_MEMORY.len());
    }

    let mut term = Console { fb: info.fb_ptr as *mut u32, width: info.width, height: info.height, stride: info.stride, cx: 0, cy: 0, bg_color: 0x001E1E2E, fg_color: 0x00A6E3A1 };
    term.clear();
    term.print("MIND CORE. FAT32 ELF LOADER ACTIVE.\n");
    let msg = format!("MEMORY MANAGER INITIALIZED: 1 MB HEAP ALLOCATED.\n");
    term.print(&msg);
    term.print("LIST: PROGRAMS. RUN <NAME>: LAUNCH. HELP: COMMANDS.\n");
    term.print("MIND> ");

    let mut input_buf = [0u8; 128];
    let mut input_len = 0;
    let mut last_scancode = 0;

    loop {
        unsafe {
            let ptr = core::ptr::addr_of_mut!(SYSTEM_TICKS);
            core::ptr::write_volatile(ptr, core::ptr::read_volatile(ptr) + 1);
        }

        let mut ascii_input: u8 = 0;
        let serial_byte = unsafe { serial_read_byte() };
        if serial_byte != 0 {
            if serial_byte == 0x0D { ascii_input = b'\n'; } else if serial_byte == 0x7F { ascii_input = 0x08; } else { ascii_input = serial_byte; }
        } else {
            let mut status: u8; let mut scancode: u8 = 0;
            unsafe {
                asm!("in al, 0x64", out("al") status);
                if (status & 1) == 1 { asm!("in al, 0x60", out("al") scancode); }
            }
            if scancode != 0 && scancode != last_scancode && scancode < 0x80 {
                last_scancode = scancode;
                if scancode == 0x0E { ascii_input = 0x08; } else if scancode == 0x39 { ascii_input = b' '; } else if (scancode as usize) < SCANCODE_TO_ASCII.len() { ascii_input = SCANCODE_TO_ASCII[scancode as usize]; }
            } else if scancode >= 0x80 { last_scancode = scancode; }
        }

        if ascii_input != 0 && ascii_input != b'?' {
            if ascii_input == 0x08 { if input_len > 0 { input_len -= 1; term.print_char(0x08); } } 
            else if ascii_input == b'\n' { 
                term.print("\n");
                let line = input_buf[0..input_len].trim_ascii();
                let split = line.iter().position(|b| b.is_ascii_whitespace()).unwrap_or(line.len());
                let cmd = &line[..split];
                let args = line[split..].trim_ascii();
                if !cmd.is_empty() {
                    if !streq(cmd, b"run") && !args.is_empty() { term.print("THIS COMMAND TAKES NO ARGUMENTS.\n"); }
                    else if streq(cmd, b"help") { term.print("- help\n- list: show programs\n- run <name>: launch a program\n- boot: alias for run app\n- clear\n- heap\n- stop\nESC RETURNS FROM ANY APP.\n"); }
                    else if streq(cmd, b"list") { list_programs(&mut term); }
                    else if streq(cmd, b"clear") { term.clear(); } 
                    else if streq(cmd, b"stop") {
                        term.print("SYSTEM HALTED. CPU GOING TO SLEEP...\n");
                        unsafe { asm!("cli"); loop { asm!("hlt"); } }
                    }
                    else if streq(cmd, b"heap") {
                        let dyn_str = format!("Dynamic allocation works! System ticks: {}\n", unsafe { SYSTEM_TICKS });
                        term.print(&dyn_str);
                    }
                    else if streq(cmd, b"boot") {
                        term.print("STARTING APP...\n");
                        launch_app(&mut term, info, info.app_entry);
                    }
                    else if streq(cmd, b"run") {
                        let entry = if streq(args, b"app") { Some(info.app_entry) }
                            else if streq(args, b"app2") { Some(info.app2_entry) }
                            else if streq(args, b"clock") { Some(info.clock_entry) }
                            else { None };
                        if let Some(entry) = entry {
                            term.print("STARTING PROGRAM...\n");
                            launch_app(&mut term, info, entry);
                        } else if args.is_empty() {
                            term.print("USAGE: RUN <NAME>\n");
                            list_programs(&mut term);
                        } else {
                            term.print("UNKNOWN PROGRAM. TYPE LIST TO SEE PROGRAMS.\n");
                        }
                    } else { term.print("UNKNOWN COMMAND\n"); }
                }
                input_len = 0; term.print("MIND> ");
            } else { 
                if input_len < input_buf.len() { input_buf[input_len] = ascii_input; input_len += 1; term.print_char(ascii_input); }
            }
        }
    }
}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
