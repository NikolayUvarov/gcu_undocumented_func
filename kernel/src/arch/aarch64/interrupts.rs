// GICv3 (distributor and the CPU's redistributor at their `virt` addresses, the CPU interface through system
// registers) and the EL1 virtual timer as the 100 Hz tick (issue 201). Device lines are SPIs; line n is SPI 32 + n.
use super::context::Event;
use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

const GICD: usize = 0x0800_0000;
const GICR: usize = 0x080A_0000; // CPU 0's redistributor; its SGI/PPI page follows at +64 KiB
const TIMER_PPI: u32 = 27; // EL1 virtual timer
const TICK_MS: u64 = 10;
static TICKS: AtomicU64 = AtomicU64::new(0);
static TIMER_STEP: AtomicU64 = AtomicU64::new(0);
// SGIs (software interrupts between CPUs, issue 203): stop, tick, wake.
pub const SGI_STOP: u32 = 1; pub const SGI_TICK: u32 = 2; pub const SGI_WAKE: u32 = 3;

unsafe fn write32(address: usize, value: u32) { core::ptr::write_volatile(address as *mut u32, value) }
unsafe fn read32(address: usize) -> u32 { core::ptr::read_volatile(address as *const u32) }

pub fn advance() { TICKS.fetch_add(1, Ordering::Relaxed); }
pub fn milliseconds() -> u64 { TICKS.load(Ordering::Relaxed).wrapping_mul(TICK_MS) }

pub fn without<T>(f: impl FnOnce() -> T) -> T {
    unsafe {
        let daif: u64;
        asm!("mrs {}, daif", "msr daifset, #2", out(reg) daif, options(nostack));
        let result = f();
        if daif & (1 << 7) == 0 { asm!("msr daifclr, #2", options(nostack)); }
        result
    }
}

/// The distributor, this CPU's redistributor and CPU interface, and the tick.
pub unsafe fn init() {
    write32(GICD, 1 << 4 | 1 << 1); // ARE_NS, Group 1 non-secure
    // Device lines (SPIs): group 1, to CPU 0, level-triggered as reset; enabled by their drivers.
    let lines = ((read32(GICD + 4) & 0x1F) as usize + 1) * 32;
    for word in 1..lines / 32 { write32(GICD + 0x80 + 4 * word, u32::MAX); }
    for spi in 32..lines { core::ptr::write_volatile((GICD + 0x6000 + 8 * spi) as *mut u64, 0); }
    load();
    let frequency: u64; asm!("mrs {}, cntfrq_el0", out(reg) frequency);
    TIMER_STEP.store(frequency * TICK_MS / 1000, Ordering::Release);
    rearm();
    asm!("msr cntv_ctl_el0, {}", in(reg) 1u64); // enabled, not masked
}

// This CPU's redistributor and CPU interface.
pub unsafe fn load() {
    let waker = GICR + 0x14;
    write32(waker, read32(waker) & !(1 << 1)); // ProcessorSleep off
    while read32(waker) & (1 << 2) != 0 { core::hint::spin_loop(); } // ChildrenAsleep
    let sgi = GICR + 0x1_0000;
    write32(sgi + 0x80, u32::MAX); // IGROUPR0: SGIs and PPIs in group 1
    write32(sgi + 0x100, 1 << TIMER_PPI | 1 << SGI_STOP | 1 << SGI_TICK | 1 << SGI_WAKE); // ISENABLER0
    asm!("msr icc_sre_el1, {}", "isb", in(reg) 7u64);
    asm!("msr icc_pmr_el1, {}", in(reg) 0xFFu64);
    asm!("msr icc_igrpen1_el1, {}", "isb", in(reg) 1u64);
}

unsafe fn rearm() { asm!("msr cntv_tval_el0, {}", in(reg) TIMER_STEP.load(Ordering::Relaxed)); }

pub fn irq_masked(line: u8) -> bool {
    let spi = 32 + line as usize;
    unsafe { read32(GICD + 0x100 + spi / 32 * 4) & 1 << (spi % 32) == 0 }
}
pub unsafe fn set_irq_masked(line: u8, masked: bool) {
    let spi = 32 + line as usize;
    write32(GICD + if masked { 0x180 } else { 0x100 } + spi / 32 * 4, 1 << (spi % 32));
}

// Takes the pending interrupt from the CPU interface and ends it; a device line stays disabled until its driver
// acknowledges it.
pub unsafe fn acknowledge() -> Event {
    let intid: u64;
    asm!("mrs {}, icc_iar1_el1", out(reg) intid);
    let intid = intid as u32 & 0xFF_FFFF;
    if intid >= 1020 { return Event::Wake; } // spurious
    let event = match intid {
        TIMER_PPI => { rearm(); advance(); super::cpu::tick_others(); Event::Tick }
        SGI_STOP => Event::Stop,
        SGI_TICK => Event::Tick,
        SGI_WAKE => Event::Wake,
        32.. => { set_irq_masked((intid - 32) as u8, true); Event::Irq((intid - 32) as usize) }
        _ => Event::Wake,
    };
    asm!("msr icc_eoir1_el1, {}", "isb", in(reg) intid as u64);
    event
}
