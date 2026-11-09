//! Host tests of the scheduler's lock (common/spinlock.rs, 000-KRN-0219): mutual exclusion; a waiter marked next is let in
//! before the holder takes the lock again; and beside it the two locks it was measured against, printed: the test-and-set
//! lock, under which a busy thread can take it many times while another waits (round 91's starved CPU), and a ticket
//! lock, whose turns wait for the host to run the one thread next when there are more threads than CPUs (round 95's).
#[path = "../common/spinlock.rs"]
mod spinlock;

use spinlock::SpinLock;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

trait Lock: Send + Sync { fn take(&self); fn give(&self); }
impl Lock for SpinLock { fn take(&self) { self.lock(); } fn give(&self) { self.unlock() } }
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
// The ticket lock round 95's gate refused: each taker served in the order it drew.
struct Ticket { next: AtomicU32, serving: AtomicU32 }
impl Lock for Ticket {
    fn take(&self) {
        let ticket = self.next.fetch_add(1, Ordering::Relaxed);
        while self.serving.load(Ordering::Acquire) != ticket { std::hint::spin_loop(); }
    }
    fn give(&self) { self.serving.store(self.serving.load(Ordering::Relaxed).wrapping_add(1), Ordering::Release) }
}

// A thread that takes the lock `rounds` times in a tight loop, and another that takes it `tries` times. The most times the
// first took the lock while the second waited for one turn, and the increments of a counter both make under it lost.
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
    (worst, rounds + tries - shared.load(Ordering::Relaxed))
}

// `threads` threads taking the lock with a short hold for `time`: the turns taken, and the increments under it lost.
fn crowd(lock: Arc<dyn Lock>, threads: usize, time: Duration) -> (u64, u64) {
    let count = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let all: Vec<_> = (0..threads).map(|_| { let (lock, count, stop) = (lock.clone(), count.clone(), stop.clone()); thread::spawn(move || {
        let mut turns = 0;
        while !stop.load(Ordering::Relaxed) {
            lock.take();
            count.store(count.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
            for _ in 0..20 { std::hint::spin_loop(); }
            lock.give();
            turns += 1;
        }
        turns
    }) }).collect();
    thread::sleep(time);
    stop.store(true, Ordering::Relaxed);
    let turns: u64 = all.into_iter().map(|t| t.join().unwrap()).sum();
    (turns, turns - count.load(Ordering::Relaxed))
}

#[test]
fn the_lock_excludes() {
    let lock = SpinLock::new();
    assert_eq!(lock.lock(), 0);
    lock.unlock();
    // Four threads, 20 000 increments each of a counter read and written apart, some of them marked next: none lost.
    let lock = Arc::new(SpinLock::new());
    let count = Arc::new(AtomicU64::new(0));
    let threads: Vec<_> = (0..4).map(|_| { let (lock, count) = (lock.clone(), count.clone()); thread::spawn(move || {
        for _ in 0..20_000 { lock.lock_after(8); count.store(count.load(Ordering::Relaxed) + 1, Ordering::Relaxed); lock.unlock(); }
    }) }).collect();
    for t in threads { t.join().unwrap(); }
    assert_eq!(count.load(Ordering::Relaxed), 80_000);
    assert!(!lock.marked());
}

#[test]
fn a_marked_waiter_is_let_in_before_the_holder_takes_it_again() {
    let lock = Arc::new(SpinLock::new());
    let order = Arc::new(Mutex::new(Vec::new()));
    lock.lock();
    let waiter = { let (lock, order) = (lock.clone(), order.clone()); thread::spawn(move || {
        let turns = lock.lock_after(16);
        order.lock().unwrap().push("waiter");
        lock.unlock();
        turns
    }) };
    while !lock.marked() { thread::yield_now(); }
    // The holder lets go and asks again at once, without ever marking itself: the marked waiter is let in first.
    lock.unlock();
    lock.lock_after(u32::MAX);
    order.lock().unwrap().push("holder");
    lock.unlock();
    assert!(waiter.join().unwrap() >= 16);
    assert_eq!(*order.lock().unwrap(), ["waiter", "holder"]);
    assert!(!lock.marked());
}

#[test]
fn a_waiter_gets_its_turn_beside_a_thread_that_takes_the_lock_again_and_again() {
    let cpus = thread::available_parallelism().map_or(1, |n| n.get());
    let (patient, lost) = race(Arc::new(SpinLock::new()), 2_000_000, 2_000);
    assert_eq!(lost, 0);
    let (unfair, lost) = race(Arc::new(TestAndSet(AtomicBool::new(false))), 2_000_000, 2_000);
    assert_eq!(lost, 0);
    let (ordered, lost) = race(Arc::new(Ticket { next: AtomicU32::new(0), serving: AtomicU32::new(0) }), 2_000_000, 2_000);
    assert_eq!(lost, 0);
    println!("on {cpus} CPUs, while one turn was waited for the busy thread took the lock at most: {patient} times (this lock), {unfair} (test-and-set), {ordered} (ticket)");
}

#[test]
fn more_threads_than_cpus_take_the_lock_in_turn() {
    // Four threads to each CPU (at most 64) for half a second: the ticket lock's turns wait for the host to run the one next.
    let cpus = thread::available_parallelism().map_or(1, |n| n.get());
    let threads = (4 * cpus).min(64);
    let time = Duration::from_millis(500);
    let (patient, lost) = crowd(Arc::new(SpinLock::new()), threads, time);
    assert_eq!(lost, 0);
    assert!(patient > 0);
    let (unfair, lost) = crowd(Arc::new(TestAndSet(AtomicBool::new(false))), threads, time);
    assert_eq!(lost, 0);
    let (ordered, lost) = crowd(Arc::new(Ticket { next: AtomicU32::new(0), serving: AtomicU32::new(0) }), threads, time);
    assert_eq!(lost, 0);
    println!("{threads} threads on {cpus} CPUs for {time:?}, turns taken: {patient} (this lock), {unfair} (test-and-set), {ordered} (ticket)");
}
