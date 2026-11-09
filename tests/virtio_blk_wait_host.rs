//! virtio_blk's wait for a request against a clock (000-DRV-0065): a request that ends within the spin costs no sleep,
//! a slow one sleeps between polls, and one that never ends fails at the deadline, not by a count of polls.
#[path = "../virtio_blk/src/wait.rs"]
mod wait;

use std::cell::Cell;

/// A clock that moves `step` ns at each reading, and the sleeps taken, each moving it 1 ms.
fn run(step: u64, ends_at: Option<u64>) -> (bool, u64, u32) {
    let (clock, sleeps) = (Cell::new(0u64), Cell::new(0u32));
    let now = || { clock.set(clock.get() + step); clock.get() };
    let sleep = || { sleeps.set(sleeps.get() + 1); clock.set(clock.get() + 1_000_000); };
    let done = || ends_at.is_some_and(|t| clock.get() >= t);
    let ok = wait::wait(now, sleep, done);
    (ok, clock.get(), sleeps.get())
}

#[test]
fn a_quick_request_costs_no_sleep() {
    // Ends after 300 µs, a TCG read's time: within the spin.
    let (ok, _, sleeps) = run(10_000, Some(300_000));
    assert!(ok);
    assert_eq!(sleeps, 0);
}

#[test]
fn a_slow_request_sleeps_only_after_the_spin() {
    let (ok, at, sleeps) = run(10_000, Some(wait::SPIN_NS + 5_000_000));
    assert!(ok);
    assert!(at >= wait::SPIN_NS + 5_000_000 && sleeps >= 1 && sleeps <= 6, "{at} {sleeps}");
}

#[test]
fn a_request_that_never_ends_fails_at_the_deadline() {
    let (ok, at, sleeps) = run(10_000, None);
    assert!(!ok);
    assert!(at >= wait::DEADLINE_NS && at < wait::DEADLINE_NS + 2_000_000, "{at}");
    // A sleep a millisecond: about the deadline less the spin, in sleeps.
    assert!(sleeps as u64 >= (wait::DEADLINE_NS - wait::SPIN_NS) / 1_000_000 / 2, "{sleeps}");
}

#[test]
fn a_request_done_at_once_reads_the_clock_once() {
    let readings = Cell::new(0u32);
    let ok = wait::wait(|| { readings.set(readings.get() + 1); 0 }, || {}, || true);
    assert!(ok);
    assert_eq!(readings.get(), 1);
}
