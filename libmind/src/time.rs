use crate::abi::*;
use crate::sys::call;

/// Sleeps up to `ms` milliseconds (10 ms granularity); input wakes the active program (in a window: the keys the
/// window manager queues, issue 088). Returns the time slept.
pub fn sleep(ms: usize) -> usize { if crate::windowed::active() { crate::windowed::wait(ms) } else { call(SYSCALL_WAIT, ms, 0) } }
pub fn uptime_ms() -> usize { call(SYSCALL_UPTIME, 0, 0) }
pub fn rdtsc() -> u64 { call(SYSCALL_RDTSC, 0, 0) as u64 }

/// Monotonic nanoseconds since boot (calibrated TSC, or the 10 ms tick where the TSC rate is not constant).
pub fn monotonic_ns() -> u64 { call(SYSCALL_CLOCK, 0, 0) as u64 }

/// (nanoseconds since boot, resolution in ns, TSC frequency in Hz or 0 for the tick clock).
pub fn clock_info() -> (u64, u64, u64) { let raw = crate::sys::syscall(SYSCALL_CLOCK, 0, 0, [0; 4]); (raw.result as u64, raw.arg2 as u64, raw.msg[2] as u64) }
