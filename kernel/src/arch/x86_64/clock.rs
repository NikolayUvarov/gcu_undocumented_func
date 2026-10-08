// Monotonic clock (MC-5.6): the TSC calibrated against the ACPI PM timer, or the PIT tick without one, when it runs at
// a constant rate, else the tick. The tick: the boot CPU's LAPIC timer measured on the PM timer, else the PIT.
use super::port::{inb, inl, outb};
use crate::{cpu, interrupts};
use core::arch::x86_64::{__cpuid, _rdtsc};
use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

static TSC_HZ: AtomicU64 = AtomicU64::new(0); // 0: the tick is the clock
static TSC_BASE: AtomicU64 = AtomicU64::new(0); // TSC value at uptime 0
static LAST: [AtomicU64; cpu::MAX] = [const { AtomicU64::new(0) }; cpu::MAX]; // never go backwards on a CPU
const PM_HZ: u64 = 3_579_545;

// Constant-rate TSC: CPUID invariant TSC, or running under a hypervisor (it provides a constant-rate TSC).
fn constant_tsc() -> bool {
    let invariant = __cpuid(0x8000_0000).eax >= 0x8000_0007 && __cpuid(0x8000_0007).edx & (1 << 8) != 0;
    invariant || __cpuid(1).ecx & (1 << 31) != 0
}

// Whether the PIT's channel 0 counts: some chipsets gate its clock. Two latched reads about 2 ms apart (port 0x80).
unsafe fn pit_counts() -> bool {
    let count = || { outb(0x43, 0); u16::from_le_bytes([inb(0x40), inb(0x40)]) };
    let first = count();
    for _ in 0..2000 { outb(0x80, 0); }
    count() != first
}

/// The tick (211-PRT-0003), with interrupts still off. With an ACPI PM timer that counts, the TSC and the boot CPU's
/// LAPIC timer are measured over 50 ms of it and the LAPIC timer gives the tick; else the PIT does, through the 8259.
/// Machines whose PIT does not interrupt (a gated clock, HPET legacy routing) still tick; the boot line names the source.
pub unsafe fn start_tick() {
    let pit = if pit_counts() { "PIT COUNTING" } else { "PIT NOT COUNTING" };
    let pm = super::acpi::pm_timer().filter(|&(port, mask)| { let first = inl(port) & mask; (0..100_000).any(|_| inl(port) & mask != first) });
    let Some((port, mask)) = pm else { let _ = write!(crate::PanicSerial, "MIND CORE KERNEL: TICK: PIT, NO ACPI PM TIMER; {}\n", pit); return };
    let read = || inl(port) & mask;
    cpu::lapic_timer(u32::MAX, false);
    let (p0, t0, c0) = (read(), _rdtsc(), cpu::lapic_timer_count());
    let mut p1 = p0;
    while (p1.wrapping_sub(p0) & mask) < (PM_HZ / 20) as u32 { p1 = read(); }
    let (t1, c1) = (_rdtsc(), cpu::lapic_timer_count());
    let elapsed = (p1.wrapping_sub(p0) & mask) as u64;
    let per_tick = c0.wrapping_sub(c1) as u64 * PM_HZ / elapsed * interrupts::milliseconds_per_tick() / 1000;
    if !(100..=u32::MAX as u64).contains(&per_tick) {
        cpu::lapic_timer(0, false);
        let _ = write!(crate::PanicSerial, "MIND CORE KERNEL: TICK: PIT, LAPIC TIMER NOT MEASURABLE; {}\n", pit);
        return;
    }
    let hz = (t1 - t0) * PM_HZ / elapsed;
    if constant_tsc() && hz >= 1_000_000 { TSC_BASE.store(t1, Ordering::Relaxed); TSC_HZ.store(hz, Ordering::Release); }
    interrupts::set_irq_masked(0, true);
    interrupts::tick_from_lapic();
    cpu::lapic_timer(per_tick as u32, true);
    let _ = write!(crate::PanicSerial, "MIND CORE KERNEL: TICK: LAPIC TIMER, {} PER TICK, MEASURED ON THE ACPI PM TIMER; TSC {} MHZ; {}\n",
                   per_tick, tsc_hz() / 1_000_000, pit);
}

// Without the PM timer: the TSC over five PIT ticks; needs interrupts enabled on the BSP. A PIT that never interrupts
// is reported rather than waited for.
pub fn calibrate() {
    if !interrupts::tick_from_pit() { return; }
    let (edge, deadline) = (interrupts::milliseconds(), unsafe { _rdtsc() } + 10_000_000_000);
    while interrupts::milliseconds() == edge {
        if unsafe { _rdtsc() } > deadline { crate::serial_print("MIND CORE KERNEL: NO TICK FROM THE PIT: TIME DOES NOT ADVANCE\n"); return; }
        core::hint::spin_loop();
    }
    if !constant_tsc() { return; }
    let edge = interrupts::milliseconds();
    while interrupts::milliseconds() == edge { core::hint::spin_loop(); }
    let (ms0, tsc0) = (interrupts::milliseconds(), unsafe { _rdtsc() });
    while interrupts::milliseconds() < ms0 + 50 { core::hint::spin_loop(); }
    let (ms1, tsc1) = (interrupts::milliseconds(), unsafe { _rdtsc() });
    let hz = (tsc1 - tsc0) * 1000 / (ms1 - ms0).max(1);
    if hz < 1_000_000 { return; }
    TSC_BASE.store(tsc0 - hz / 1000 * ms0, Ordering::Relaxed);
    TSC_HZ.store(hz, Ordering::Release);
}

pub fn tsc_hz() -> u64 { TSC_HZ.load(Ordering::Acquire) }
pub fn resolution_ns() -> u64 { match tsc_hz() { 0 => interrupts::milliseconds_per_tick() * 1_000_000, hz => (1_000_000_000 / hz).max(1) } }

// Nanoseconds since boot on the calling CPU.
pub fn now_ns() -> u64 {
    let ns = match tsc_hz() {
        0 => interrupts::milliseconds() * 1_000_000,
        hz => (unsafe { _rdtsc() }.saturating_sub(TSC_BASE.load(Ordering::Relaxed)) as u128 * 1_000_000_000 / hz as u128) as u64,
    };
    let last = &LAST[cpu::id()];
    let ns = ns.max(last.load(Ordering::Relaxed));
    last.store(ns, Ordering::Relaxed);
    ns
}
