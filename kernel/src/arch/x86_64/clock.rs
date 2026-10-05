// Monotonic clock (MC-5.6): the TSC calibrated against the PIT tick when it runs at a constant rate, else the tick.
use crate::{cpu, interrupts};
use core::arch::x86_64::{__cpuid, _rdtsc};
use core::sync::atomic::{AtomicU64, Ordering};

static TSC_HZ: AtomicU64 = AtomicU64::new(0); // 0: the tick is the clock
static TSC_BASE: AtomicU64 = AtomicU64::new(0); // TSC value at uptime 0
static LAST: [AtomicU64; cpu::MAX] = [const { AtomicU64::new(0) }; cpu::MAX]; // never go backwards on a CPU

// Constant-rate TSC: CPUID invariant TSC, or running under a hypervisor (it provides a constant-rate TSC).
fn constant_tsc() -> bool {
    let invariant = __cpuid(0x8000_0000).eax >= 0x8000_0007 && __cpuid(0x8000_0007).edx & (1 << 8) != 0;
    invariant || __cpuid(1).ecx & (1 << 31) != 0
}

// Measures the TSC over five PIT ticks; needs interrupts enabled on the BSP.
pub fn calibrate() {
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
