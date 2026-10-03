use crate::abi::*;
use crate::sys::call;

/// Спит до `ms` миллисекунд (шаг 10 мс); активную программу будит ввод. Возвращает время засыпания.
pub fn sleep(ms: usize) -> usize { call(SYSCALL_WAIT, ms, 0) }
pub fn uptime_ms() -> usize { call(SYSCALL_UPTIME, 0, 0) }
pub fn rdtsc() -> u64 { call(SYSCALL_RDTSC, 0, 0) as u64 }
