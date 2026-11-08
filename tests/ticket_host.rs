//! Host tests of the scheduler's ticket lock (common/ticket.rs, 000-KRN-0219): mutual exclusion, and its fairness beside
//! the test-and-set lock it replaced. A thread that takes the lock again and again gets it at most once while another
//! waits; the test-and-set lock's count is printed beside it, for it can be larger (round 91's starved CPU).
#[path = "../common/ticket.rs"]
mod ticket;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use ticket::Ticket;

trait Lock: Send + Sync { fn take(&self); fn give(&self); }
impl Lock for Ticket { fn take(&self) { self.lock() } fn give(&self) { self.unlock() } }
// The kernel's lock before 000-KRN-0219: test and test-and-set.
struct TestAndSet(AtomicBool);
impl Lock for TestAndSet {
    fn take(&self) {
        while self.0.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            while self.0.load(Ordering::Relaxed) { std::hint::spin_loop(); }
        }
    }
    fn give(&self) { self.0.store(false, Ordering::Release) }
}

// A thread that takes the lock `rounds` times in a tight loop, and another that takes it `tries` times. The most times the
// first took the lock while the second waited for one turn, and the counter both increment under the lock.
fn race(lock: Arc<dyn Lock>, rounds: u64, tries: u64) -> (u64, u64) {
    let taken = Arc::new(AtomicU64::new(0)); // the busy thread's acquisitions, counted under the lock
    let shared = Arc::new(AtomicU64::new(0)); // a plain counter both increment under the lock
    let started = Arc::new(AtomicBool::new(false));
    let busy = {
        let (lock, taken, shared, started) = (lock.clone(), taken.clone(), shared.clone(), started.clone());
        thread::spawn(move || {
            started.store(true, Ordering::Release);
            for _ in 0..rounds {
                lock.take();
                taken.store(taken.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
                shared.store(shared.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
                for _ in 0..20 { std::hint::spin_loop(); } // a short hold, like a system call's
                lock.give();
            }
        })
    };
    while !started.load(Ordering::Acquire) { std::hint::spin_loop(); }
    let mut worst = 0;
    for _ in 0..tries {
        let before = taken.load(Ordering::Relaxed);
        lock.take();
        let during = taken.load(Ordering::Relaxed) - before;
        shared.store(shared.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
        lock.give();
        worst = worst.max(during);
        for _ in 0..50 { std::hint::spin_loop(); }
    }
    busy.join().unwrap();
    (worst, shared.load(Ordering::Relaxed) - rounds - tries)
}

#[test]
fn the_ticket_lock_excludes_and_counts() {
    let lock = Ticket::new();
    assert_eq!(lock.queued(), 0);
    lock.lock();
    assert_eq!(lock.queued(), 1);
    lock.unlock();
    assert_eq!(lock.queued(), 0);
    // Four threads, 20 000 increments each of a counter read and written apart: none lost.
    let lock = Arc::new(Ticket::new());
    let count = Arc::new(AtomicU64::new(0));
    let threads: Vec<_> = (0..4).map(|_| { let (lock, count) = (lock.clone(), count.clone()); thread::spawn(move || {
        for _ in 0..20_000 { lock.lock(); count.store(count.load(Ordering::Relaxed) + 1, Ordering::Relaxed); lock.unlock(); }
    }) }).collect();
    for t in threads { t.join().unwrap(); }
    assert_eq!(count.load(Ordering::Relaxed), 80_000);
}

#[test]
fn a_waiter_is_served_before_the_holder_takes_it_again() {
    // Only on a machine with two CPUs or more do the two threads race; with one, the test says so and checks the counts.
    let cpus = thread::available_parallelism().map_or(1, |n| n.get());
    let (fair, lost) = race(Arc::new(Ticket::new()), 2_000_000, 2_000);
    assert_eq!(lost, 0);
    assert!(fair <= 1, "the busy thread took the ticket lock {fair} times while the other waited once");
    let (unfair, lost) = race(Arc::new(TestAndSet(AtomicBool::new(false))), 2_000_000, 2_000);
    assert_eq!(lost, 0);
    println!("on {cpus} CPUs: while one turn was waited for, the busy thread took the ticket lock at most {fair} time(s), the test-and-set lock up to {unfair}");
}
