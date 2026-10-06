// Monotonic clock (MC-5.6): the generic timer's virtual count at its fixed frequency.
use core::arch::asm;

fn frequency() -> u64 { let f: u64; unsafe { asm!("mrs {}, cntfrq_el0", out(reg) f); } f.max(1) }
fn count() -> u64 { let c: u64; unsafe { asm!("isb", "mrs {}, cntvct_el0", out(reg) c); } c }

pub fn calibrate() {}
/// The counter's frequency (the TSC frequency on x86).
pub fn tsc_hz() -> u64 { frequency() }
pub fn resolution_ns() -> u64 { (1_000_000_000 / frequency()).max(1) }
pub fn now_ns() -> u64 { (count() as u128 * 1_000_000_000 / frequency() as u128) as u64 }
pub fn cycles() -> u64 { count() }
