// Monotonic clock (MC-5.6): the generic timer's virtual count at its fixed frequency. Every read of the count is here,
// with the A64's workaround where the device tree names its erratum (100000-PRT-0012, common/counter.rs).
use core::arch::asm;
use core::sync::atomic::Ordering;

fn frequency() -> u64 { let f: u64; unsafe { asm!("mrs {}, cntfrq_el0", out(reg) f); } f.max(1) }
fn read() -> u64 { let c: u64; unsafe { asm!("isb", "mrs {}, cntvct_el0", out(reg) c); } c }
/// The virtual count, read again while it holds the A64's wrong values when the board has that erratum.
pub fn count() -> u64 { if super::board::COUNTER_SUN50I.load(Ordering::Relaxed) { super::counter::sun50i_stable(read) } else { read() } }

pub fn calibrate() {}
/// The counter's frequency (the TSC frequency on x86).
pub fn tsc_hz() -> u64 { frequency() }
pub fn resolution_ns() -> u64 { (1_000_000_000 / frequency()).max(1) }
pub fn now_ns() -> u64 { (count() as u128 * 1_000_000_000 / frequency() as u128) as u64 }
pub fn cycles() -> u64 { count() }
/// Waits at least `us` microseconds; usable before the tick runs.
pub fn spin_us(us: u64) {
    let end = count() + (us as u128 * frequency() as u128).div_ceil(1_000_000) as u64 + 1;
    while count() < end { core::hint::spin_loop(); }
}
