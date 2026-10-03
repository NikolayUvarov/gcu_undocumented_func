use crate::abi::*;
use crate::sys::call;

/// Sleeps up to `ms` milliseconds (10 ms granularity); input wakes the active program. Returns the time slept.
pub fn sleep(ms: usize) -> usize { call(SYSCALL_WAIT, ms, 0) }
pub fn uptime_ms() -> usize { call(SYSCALL_UPTIME, 0, 0) }
pub fn rdtsc() -> u64 { call(SYSCALL_RDTSC, 0, 0) as u64 }
