// The kernel's own lines, the last 32 KiB of them (174-KRN-0038): COM1 and the screen lose them on a machine without a
// serial port once a task draws, so the hardware report carries them.
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const SIZE: usize = 32 * 1024;
static mut RING: [u8; SIZE] = [0; SIZE];
static WRITTEN: AtomicUsize = AtomicUsize::new(0); // bytes ever kept
static BUSY: AtomicBool = AtomicBool::new(false);

/// Keeps `text`; never waits long, as a panic may print from inside a print.
pub fn keep(text: &str) {
    if !(0..10_000).any(|_| BUSY.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() || { core::hint::spin_loop(); false }) { return; }
    let mut at = WRITTEN.load(Ordering::Relaxed);
    for byte in text.bytes() { unsafe { (*core::ptr::addr_of_mut!(RING))[at % SIZE] = byte; } at += 1; }
    WRITTEN.store(at, Ordering::Relaxed);
    BUSY.store(false, Ordering::Release);
}

/// The kept lines, oldest first, and whether older ones were dropped.
pub fn copy(out: &mut alloc::vec::Vec<u8>) -> bool {
    while BUSY.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() { core::hint::spin_loop(); }
    let written = WRITTEN.load(Ordering::Relaxed);
    let ring = unsafe { &*core::ptr::addr_of!(RING) };
    let start = written.saturating_sub(SIZE);
    for at in start..written { out.push(ring[at % SIZE]); }
    BUSY.store(false, Ordering::Release);
    start > 0
}
