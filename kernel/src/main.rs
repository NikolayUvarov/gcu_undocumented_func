#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

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
mod clock;
mod context;
mod cpu;
mod elf;
#[path = "../../bootloader/src/elf_reloc.rs"]
mod elf_reloc;
mod input;
mod interrupts;
mod memory;
mod paging;
mod pci;
mod scheduler;
mod task_state;
mod user_heap;

unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}
unsafe fn outl(port: u16, val: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") val, options(nomem, nostack));
}
unsafe fn inl(port: u16) -> u32 {
    let mut val: u32;
    asm!("in eax, dx", out("eax") val, in("dx") port, options(nomem, nostack));
    val
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
unsafe fn serial_is_transmit_empty() -> bool {
    (inb(COM1 + 5) & 0x20) != 0
}
unsafe fn serial_write_byte(b: u8) {
    while !serial_is_transmit_empty() {}
    outb(COM1, b);
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

// Boot line and kernel diagnostics go to COM1; the command shell is a ring 3 service started by init.
fn serial_print(text: &str) { for byte in text.bytes() { unsafe { if byte == b'\n' { serial_write_byte(b'\r'); } serial_write_byte(byte); } } }

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(info: &BootInfo) -> ! {
    unsafe {
        asm!("cli");
        init_serial();
        ALLOCATOR.lock().init(info.heap_ptr, info.heap_len);
        paging::init().expect("Kernel page tables");
        cpu::prepare(info).expect("CPU state");
        scheduler::init(info).expect("Scheduler init failed");
        scheduler::spawn_init().expect("init spawn");
        // Printed before any task can run, so the kernel's line never interleaves with the shell's output on COM1.
        serial_print("MIND CORE KERNEL: INIT STARTED\n");
        interrupts::init();
        clock::calibrate();
        cpu::start(info);
    }
    // The BSP idle loop reclaims exited tasks and otherwise sleeps until the next interrupt.
    loop {
        scheduler::reap();
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
