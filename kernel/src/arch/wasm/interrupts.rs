// The tick and the page's interrupt lines (215-PRT-0017). The tick is due every TICK_MS of the page's clock; a line is
// raised by the page (`run::raise`). `acknowledge` takes the tick first, then the lowest raised line that is not masked.
use super::context::{Event, MSI_FIRST};
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub const TICK_MS: u64 = 10;
const QUANTUM_MS: u64 = 10;
static TICKS: AtomicU64 = AtomicU64::new(0);
static NEXT_TICK_NS: AtomicU64 = AtomicU64::new(0);
/// Lines raised and not yet taken, and lines masked (bit n: line n; lines from MSI_FIRST are edge, never masked).
static RAISED: AtomicU32 = AtomicU32::new(0);
static MASKED: AtomicU32 = AtomicU32::new(0xFFFF);

/// Counts `ticks` ticks; true when they end a time slice.
pub fn advance(ticks: u64) -> bool {
    let slice = QUANTUM_MS / TICK_MS;
    let before = TICKS.fetch_add(ticks, Ordering::Relaxed);
    before / slice != (before + ticks) / slice
}
pub fn milliseconds() -> u64 { TICKS.load(Ordering::Relaxed).wrapping_mul(TICK_MS) }
/// Kernel code is never interrupted here: nothing to let in.
pub unsafe fn window() {}
pub fn without<T>(f: impl FnOnce() -> T) -> T { f() }
pub unsafe fn init() { NEXT_TICK_NS.store(super::clock::now_ns() + TICK_MS * 1_000_000, Ordering::Relaxed); }

pub fn irq_masked(line: u8) -> bool { (line as usize) < MSI_FIRST && MASKED.load(Ordering::Relaxed) & 1 << line != 0 }
pub unsafe fn set_irq_masked(line: u8, masked: bool) {
    if line as usize >= MSI_FIRST { return; }
    if masked { MASKED.fetch_or(1 << line, Ordering::Relaxed); } else { MASKED.fetch_and(!(1 << line), Ordering::Relaxed); }
}
pub fn raise(line: usize) { if line < 32 { RAISED.fetch_or(1 << line, Ordering::Relaxed); } }

fn deliverable() -> u32 { RAISED.load(Ordering::Relaxed) & !(MASKED.load(Ordering::Relaxed) & ((1 << MSI_FIRST) - 1)) }
fn tick_due(now: u64) -> bool { NEXT_TICK_NS.load(Ordering::Relaxed) != 0 && now >= NEXT_TICK_NS.load(Ordering::Relaxed) }
/// Whether an interrupt waits at `now`.
pub fn pending(now: u64) -> bool { tick_due(now) || deliverable() != 0 }
/// Nanoseconds from `now` to the next tick.
pub fn until_tick(now: u64) -> u64 { NEXT_TICK_NS.load(Ordering::Relaxed).saturating_sub(now) }

pub unsafe fn acknowledge() -> Event {
    let now = super::clock::now_ns();
    if tick_due(now) {
        // Ticks the page's thread missed (a slow turn, a hidden tab) are counted at once, as aarch64 rearms by the count.
        let next = NEXT_TICK_NS.load(Ordering::Relaxed);
        let step = TICK_MS * 1_000_000;
        let missed = (now - next) / step + 1;
        NEXT_TICK_NS.store(next + missed * step, Ordering::Relaxed);
        return if advance(missed) { Event::Tick } else { Event::Clock };
    }
    let lines = deliverable();
    if lines == 0 { return Event::Wake; }
    let line = lines.trailing_zeros() as usize;
    RAISED.fetch_and(!(1 << line), Ordering::Relaxed);
    // A level line stays masked until its driver acknowledges it, as a GIC SPI on aarch64.
    if line < MSI_FIRST { set_irq_masked(line as u8, true); }
    Event::Irq(line)
}
