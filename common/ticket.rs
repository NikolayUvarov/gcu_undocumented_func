#![allow(dead_code)]
// A ticket lock (000-KRN-0219): each taker draws a ticket and is served in the order it drew, so no one waits behind a
// holder that takes the lock again and again, as one CPU did behind a test-and-set lock in round 91's gate. The caller
// keeps interrupts off around it (the kernel's `locked`); this file holds only the counting, for the kernel and the host
// tests alike.
use core::sync::atomic::{AtomicU32, Ordering};

pub struct Ticket { next: AtomicU32, serving: AtomicU32 }

impl Ticket {
    pub const fn new() -> Ticket { Ticket { next: AtomicU32::new(0), serving: AtomicU32::new(0) } }
    /// Waits until this taker's turn; the lock is then held until `unlock`.
    pub fn lock(&self) {
        let ticket = self.next.fetch_add(1, Ordering::Relaxed);
        while self.serving.load(Ordering::Acquire) != ticket { core::hint::spin_loop(); }
    }
    /// Lets the next ticket in; only the holder calls it.
    pub fn unlock(&self) {
        let now = self.serving.load(Ordering::Relaxed);
        self.serving.store(now.wrapping_add(1), Ordering::Release);
    }
    /// How many takers hold or wait for the lock.
    pub fn queued(&self) -> u32 { self.next.load(Ordering::Relaxed).wrapping_sub(self.serving.load(Ordering::Relaxed)) }
}
