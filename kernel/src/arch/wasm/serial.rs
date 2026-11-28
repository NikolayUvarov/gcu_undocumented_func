// The page's console (215-PRT-0017): bytes out through `host::console_write`, bytes in from `run::console_in`, which
// raises the console's line. It is the kernel's console channel (PLATFORM_CONSOLE), as the PL011 is on aarch64.
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const RING: usize = 1024;
static mut INPUT: [u8; RING] = [0; RING];
static HEAD: AtomicUsize = AtomicUsize::new(0); // next byte to take
static TAIL: AtomicUsize = AtomicUsize::new(0); // next place to put
static RX: AtomicBool = AtomicBool::new(false);
/// The console's line, as the scheduler binds it.
pub const LINE: usize = 1;

pub unsafe fn init_serial() {}
pub unsafe fn serial_write_byte(b: u8) { super::host::console_write(b as u32); }

/// A byte typed on the page; dropped when the ring is full. The line rises while receiving is on.
pub fn put(byte: u8) {
    let (head, tail) = (HEAD.load(Ordering::Relaxed), TAIL.load(Ordering::Relaxed));
    if tail.wrapping_sub(head) >= RING { return; }
    unsafe { (*core::ptr::addr_of_mut!(INPUT))[tail % RING] = byte; }
    TAIL.store(tail.wrapping_add(1), Ordering::Relaxed);
    if RX.load(Ordering::Relaxed) { super::interrupts::raise(LINE); }
}

pub unsafe fn console_take(out: &mut [u8]) -> usize {
    let mut n = 0;
    while n < out.len() && HEAD.load(Ordering::Relaxed) != TAIL.load(Ordering::Relaxed) {
        let head = HEAD.load(Ordering::Relaxed);
        out[n] = (*core::ptr::addr_of!(INPUT))[head % RING];
        HEAD.store(head.wrapping_add(1), Ordering::Relaxed);
        n += 1;
    }
    n
}
/// The page takes every byte at once.
pub unsafe fn console_room() -> bool { true }
/// Receiving raises the line (and at once, for bytes typed before); sending needs no interrupt here.
pub unsafe fn console_interrupts(rx: bool, _tx: bool) {
    RX.store(rx, Ordering::Relaxed);
    if rx && HEAD.load(Ordering::Relaxed) != TAIL.load(Ordering::Relaxed) { super::interrupts::raise(LINE); }
}
