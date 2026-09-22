use super::outb;
use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

const TICK_MS: u64 = 10;
static TICKS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    zero: u32,
}
#[repr(C, packed)]
struct IdtPtr {
    limit: u16,
    base: u64,
}
#[repr(C)]
pub struct InterruptFrame {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

static mut IDT: [IdtEntry; 256] = [IdtEntry {
    offset_low: 0,
    selector: 0,
    ist: 0,
    type_attr: 0,
    offset_mid: 0,
    offset_high: 0,
    zero: 0,
}; 256];

unsafe fn set_handler(index: usize, address: u64, cs: u16) {
    IDT[index] = IdtEntry {
        offset_low: address as u16,
        selector: cs,
        ist: 0,
        type_attr: 0x8E,
        offset_mid: (address >> 16) as u16,
        offset_high: (address >> 32) as u32,
        zero: 0,
    };
}

extern "x86-interrupt" fn unexpected(_frame: &mut InterruptFrame) {
    crate::cpu::halt_all();
}
pub fn advance() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}
extern "x86-interrupt" fn spurious_master(_frame: &mut InterruptFrame) {}
extern "x86-interrupt" fn spurious_slave(_frame: &mut InterruptFrame) {
    unsafe {
        outb(0x20, 0x20);
    }
}

pub unsafe fn init() {
    asm!("cli");
    let cs = 8;
    for index in 0..256 {
        set_handler(index, unexpected as *const () as u64, cs);
    }
    for index in 0..32 {
        set_handler(
            index,
            (core::ptr::addr_of!(super::context::exception_table) as u64)
                .wrapping_add(super::context::exception_table[index]),
            cs,
        );
    }
    IDT[8].ist = 1;
    IDT[2].ist = 2;
    set_handler(
        0x20,
        super::context::task_timer_entry as *const () as u64,
        cs,
    );
    set_handler(0x30, super::context::task_ipi_entry as *const () as u64, cs);
    set_handler(
        0x31,
        super::context::task_stop_entry as *const () as u64,
        cs,
    );
    set_handler(0x27, spurious_master as *const () as u64, cs);
    set_handler(0x2f, spurious_slave as *const () as u64, cs);
    set_handler(0xff, spurious_master as *const () as u64, cs);
    set_handler(
        0x80,
        super::context::task_syscall_entry as *const () as u64,
        cs,
    );
    IDT[0x80].type_attr = 0xee; // only syscall is callable from ring 3
    load();

    // Remap the 8259 PICs away from CPU exceptions; unmask only PIT IRQ0.
    for (port, value) in [
        (0x20, 0x11),
        (0xA0, 0x11),
        (0x21, 0x20),
        (0xA1, 0x28),
        (0x21, 4),
        (0xA1, 2),
        (0x21, 1),
        (0xA1, 1),
        (0x21, 0xFE),
        (0xA1, 0xFF),
    ] {
        outb(port, value);
        outb(0x80, 0); // I/O delay for PIC initialization.
    }
    // PIT channel 0, square-wave mode, approximately 100 Hz.
    let divisor: u16 = 11932;
    outb(0x43, 0x36);
    outb(0x40, divisor as u8);
    outb(0x40, (divisor >> 8) as u8);
    asm!("sti");
}

pub unsafe fn load() {
    let idtr = IdtPtr {
        limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: core::ptr::addr_of!(IDT) as u64,
    };
    asm!("lidt [{}]", in(reg) &idtr);
}

pub fn milliseconds() -> u64 {
    TICKS.load(Ordering::Relaxed).wrapping_mul(TICK_MS)
}

// Used around shell access to scheduler state. Timer preemption is disabled
// inside interrupt gates already; restore the caller's original IF afterwards.
pub fn without<T>(f: impl FnOnce() -> T) -> T {
    unsafe {
        let flags: u64;
        asm!("pushfq", "pop {}", out(reg) flags);
        asm!("cli");
        let result = f();
        if flags & (1 << 9) != 0 {
            asm!("sti");
        }
        result
    }
}
