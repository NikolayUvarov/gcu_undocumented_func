// The page's monotonic clock in nanoseconds (215-PRT-0017).
pub fn now_ns() -> u64 { unsafe { super::host::now_ns() } }
pub fn calibrate() {}
pub fn tsc_hz() -> u64 { 1_000_000_000 }
pub fn resolution_ns() -> u64 { 1 }
pub fn cycles() -> u64 { now_ns() }
pub fn spin_us(us: u64) { let until = now_ns() + us * 1000; while now_ns() < until {} }
