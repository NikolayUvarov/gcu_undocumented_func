// A request's wait (000-DRV-0065): the used ring polled against the clock for SPIN_NS, within which QEMU ends nearly
// every request, then a sleep between polls until DEADLINE_NS. A sleep is a scheduler tick (10 ms on aarch64), so one
// taken after a few microseconds cost almost every request a whole tick.

/// How long the ring is polled without sleeping.
pub const SPIN_NS: u64 = 2_000_000;
/// How long a request may take before it counts as failed.
pub const DEADLINE_NS: u64 = 10_000_000_000;
/// Polls between two readings of the clock, each a system call.
const POLLS: u32 = 64;

/// Whether `done` became true before the deadline; `now` is the monotonic clock in ns, `sleep` the shortest sleep.
pub fn wait(mut now: impl FnMut() -> u64, mut sleep: impl FnMut(), mut done: impl FnMut() -> bool) -> bool {
    let start = now();
    loop {
        for _ in 0..POLLS {
            if done() { return true; }
            core::hint::spin_loop();
        }
        let elapsed = now().saturating_sub(start);
        if elapsed >= DEADLINE_NS { return done(); }
        if elapsed >= SPIN_NS { sleep(); }
    }
}
