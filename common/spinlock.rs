#![allow(dead_code)]
// The scheduler's lock (000-KRN-0219): a test-and-test-and-set lock that whichever waiter runs takes when it is let go,
// until one has waited PATIENCE turns: that one is then marked next, and no other takes the lock before it has, unless
// the lock stays free ABSENT turns, when the marked waiter is not running. A ticket lock served every taker in the order
// it drew, so under QEMU with more vCPUs than host CPUs each turn waited for the host to run that one thread, and 16 CPUs
// crawled (round 95's gate). The caller keeps interrupts off around it (the kernel's `locked`); this file holds only the
// counting, for the kernel and the host tests alike.
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Turns a waiter spins before it is marked next.
pub const PATIENCE: u32 = 1024;
/// Turns in a row another waiter sees the lock free while a mark stands before it takes the lock: the marked one is not
/// running (its vCPU descheduled by the host), and a running one takes a free lock within a turn or two.
pub const ABSENT: u32 = 256;

pub struct SpinLock {
    held: AtomicBool,
    next: AtomicUsize,  // the mark of the waiter that is next; 0: none, the lock is anyone's
    drawn: AtomicUsize, // marks drawn so far, so that no two waiters share one
}

impl SpinLock {
    pub const fn new() -> SpinLock { SpinLock { held: AtomicBool::new(false), next: AtomicUsize::new(0), drawn: AtomicUsize::new(0) } }

    /// Waits until the lock is held, held then until `unlock`; returns how many turns it waited.
    pub fn lock(&self) -> u32 { self.lock_with(PATIENCE, ABSENT) }

    /// `lock`, marking this waiter next once it has waited `patience` turns, and passing another's mark once the lock
    /// has stayed free `absent` turns in a row.
    pub fn lock_with(&self, patience: u32, absent: u32) -> u32 {
        let mut mine = 0; // this waiter's mark, drawn once it has waited `patience` turns
        let mut turns = 0u32;
        let mut free = 0u32; // turns in a row this waiter saw the lock free while another's mark stood
        loop {
            let next = self.next.load(Ordering::Relaxed);
            if self.held.load(Ordering::Relaxed) {
                free = 0;
            } else {
                let ours = next == 0 || next == mine;
                if !ours { free = free.saturating_add(1); }
                if (ours || free >= absent) && self.held.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                    if mine != 0 && next == mine { self.next.store(0, Ordering::Relaxed); }
                    return turns;
                }
            }
            turns = turns.saturating_add(1);
            if turns >= patience {
                if mine == 0 { mine = self.drawn.fetch_add(1, Ordering::Relaxed).wrapping_add(1).max(1); }
                if next == 0 { let _ = self.next.compare_exchange(0, mine, Ordering::Relaxed, Ordering::Relaxed); }
            }
            core::hint::spin_loop();
        }
    }

    /// Lets the lock go; only the holder calls it.
    pub fn unlock(&self) { self.held.store(false, Ordering::Release); }

    /// Whether a waiter is marked next (for the tests).
    pub fn marked(&self) -> bool { self.next.load(Ordering::Relaxed) != 0 }

    /// A mark no running waiter holds, as one whose vCPU the host stopped (the host tests only).
    #[cfg(test)]
    pub fn mark_absent(&self) { self.next.store(usize::MAX, Ordering::Relaxed); }
}
