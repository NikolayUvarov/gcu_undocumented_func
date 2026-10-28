// The chipset's watchdog (000-KRN-0076): the TCO timer of Intel's LPC bridge (ICH9 to the 7-series PCH, and QEMU's q35),
// in its ACPI I/O space, or the AMD FCH's timer (213-KRN-0125, fch.rs). Armed with the tick and fed every second from the
// boot CPU's tick: a machine whose boot CPU stops is reset at the timer's expiry. A halt the kernel chose (a panic, a
// fatal error, HALT) stops it first.
use super::fch;
use super::pcicfg;
use super::port::{inl, outl, read, write};
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

static TCO: AtomicU16 = AtomicU16::new(0); // the TCO registers' I/O base while the watchdog runs
static FCH: AtomicU32 = AtomicU32::new(0); // the AMD FCH timer's address while it runs
static RESTARTED: AtomicBool = AtomicBool::new(false);
static FRESH: AtomicBool = AtomicBool::new(false);
const RLD: u16 = 0x00;
const STS1: u16 = 0x04;
const STS2: u16 = 0x06;
const CNT1: u16 = 0x08;
const TMR: u16 = 0x12;
const TMR_HLT: usize = 1 << 11;
const TIMEOUT: usize = 1 << 3;
const SECOND_TO: usize = 1 << 1;
const BOOT_STS: usize = 1 << 2;
const SMI_EN: u16 = 0x30;
const TCO_EN: u32 = 1 << 13;
const GCS: usize = 0x3410; // in the chipset configuration (RCBA)
const NO_REBOOT: u32 = 1 << 5;
const QEMU_ICH9: u32 = 0x2918; // q35's LPC bridge: OVMF leaves the chipset configuration unmapped
const RCBA_DEFAULT: u32 = 0xFED1_C000;
/// Ticks of 0.6 s to the first expiry; the reset comes at the second: 30 s without the boot CPU's tick.
pub const TICKS: u16 = 25;

/// Whether this watchdog's reset ended the last run: the second expiry's status outlives the reset.
pub fn restarted() -> bool { RESTARTED.load(Ordering::Relaxed) }

/// Whether no run of this kernel armed the timer since the machine was powered on (QEMU keeps its count across resets).
pub fn fresh() -> bool { FRESH.load(Ordering::Relaxed) }

/// Finds and arms the watchdog; the boot line names it, or why there is none.
pub unsafe fn start() {
    let (id, class) = (pcicfg::read(0, 31, 0, 0), pcicfg::read(0, 31, 0, 8) >> 8);
    if id & 0xFFFF != 0x8086 || class != 0x06_0100 { return amd(); }
    let pmbase = (pcicfg::read(0, 31, 0, 0x40) & 0xFF80) as u16;
    if pmbase == 0 || pcicfg::read(0, 31, 0, 0x44) & 0x80 == 0 { return none("THE LPC BRIDGE'S ACPI I/O IS OFF"); }
    let tco = pmbase + 0x60;
    RESTARTED.store(read(tco + STS2, 2) & SECOND_TO != 0, Ordering::Relaxed);
    write(tco + STS2, 2, SECOND_TO | BOOT_STS);
    write(tco + STS1, 2, TIMEOUT);
    // NO_REBOOT set, an expiry only sets status bits.
    let mut rcba = pcicfg::read(0, 31, 0, 0xF0);
    if rcba & 1 == 0 && id >> 16 == QEMU_ICH9 { pcicfg::write(0, 31, 0, 0xF0, RCBA_DEFAULT | 1); rcba = pcicfg::read(0, 31, 0, 0xF0); }
    if rcba & 1 == 0 { return none("NO CHIPSET CONFIGURATION: THIS PCH KEEPS NO_REBOOT ELSEWHERE"); }
    let gcs = (rcba & 0xFFFF_C000) as usize + GCS;
    super::mmu::uncached(gcs);
    let value = core::ptr::read_volatile(gcs as *const u32);
    if value & NO_REBOOT != 0 { core::ptr::write_volatile(gcs as *mut u32, value & !NO_REBOOT); }
    if core::ptr::read_volatile(gcs as *const u32) & NO_REBOOT != 0 { return none("NO_REBOOT HELD BY THE FIRMWARE"); }
    // No SMI at the first expiry: no handler of the kernel's would take it.
    let smi = inl(pmbase + SMI_EN);
    if smi & TCO_EN != 0 { outl(pmbase + SMI_EN, smi & !TCO_EN); }
    FRESH.store(read(tco + TMR, 2) != TICKS as usize, Ordering::Relaxed);
    write(tco + TMR, 2, TICKS as usize);
    write(tco + RLD, 2, 1);
    write(tco + CNT1, 2, read(tco + CNT1, 2) & !TMR_HLT);
    if read(tco + CNT1, 2) & TMR_HLT != 0 { return none("ITS TIMER STAYS HALTED"); }
    TCO.store(tco, Ordering::Release);
    let _ = write!(crate::PanicSerial, "MIND CORE KERNEL: WATCHDOG: INTEL TCO, A RESET AFTER {} S WITHOUT THE BOOT CPU'S TICK{}\n",
                   TICKS as u32 * 12 / 10, if restarted() { "; IT RESTARTED THE LAST RUN" } else { "" });
}

fn none(why: &str) { let _ = write!(crate::PanicSerial, "MIND CORE KERNEL: WATCHDOG: NONE ({})\n", why); }

// An AMD machine: the FCH's timer counts seconds, so it resets the machine as long after the last feed as TCO's does.
unsafe fn amd() {
    let seconds = TICKS as u32 * 12 / 10;
    match fch::arm(&mut Fch, seconds) {
        Ok(armed) => {
            RESTARTED.store(armed.restarted, Ordering::Relaxed);
            FRESH.store(!armed.restarted, Ordering::Relaxed);
            FCH.store(armed.base, Ordering::Release);
            let _ = write!(crate::PanicSerial, "MIND CORE KERNEL: WATCHDOG: AMD FCH, A RESET AFTER {} S WITHOUT THE BOOT CPU'S TICK{}\n",
                           seconds, if armed.restarted { "; IT RESTARTED THE LAST RUN" } else { "" });
        }
        Err("NO AMD FCH AT 00:14.0") => none("NO INTEL LPC BRIDGE AT 00:1F.0, NO AMD FCH AT 00:14.0"),
        Err(why) => none(why),
    }
}

// The FCH's registers: the SMBus function in configuration space, the PM bytes and the timer's words in MMIO.
struct Fch;

impl fch::Registers for Fch {
    fn smbus(&mut self) -> (u32, u8) { unsafe { (pcicfg::read(0, 0x14, 0, 0), pcicfg::read(0, 0x14, 0, 8) as u8) } }
    fn pm(&mut self, offset: u8) -> u8 { unsafe { core::ptr::read_volatile(mapped(fch::PM + offset as u32) as *const u8) } }
    fn set_pm(&mut self, offset: u8, value: u8) { unsafe { core::ptr::write_volatile(mapped(fch::PM + offset as u32) as *mut u8, value) } }
    fn timer(&mut self, address: u32) -> u32 { unsafe { core::ptr::read_volatile(mapped(address) as *const u32) } }
    fn set_timer(&mut self, address: u32, value: u32) { unsafe { core::ptr::write_volatile(mapped(address) as *mut u32, value) } }
}

// The identity mapping of an FCH register, uncached; the first use of a 2 MiB region sets it.
unsafe fn mapped(address: u32) -> usize {
    static DONE: AtomicU32 = AtomicU32::new(0);
    let region = address >> 21;
    if DONE.swap(region, Ordering::Relaxed) != region { super::mmu::uncached(address as usize); }
    address as usize
}

/// On the boot CPU's tick, every second.
pub fn feed() {
    let tco = TCO.load(Ordering::Relaxed);
    if tco != 0 { unsafe { write(tco + RLD, 2, 1); } }
    let base = FCH.load(Ordering::Relaxed);
    if base != 0 { fch::feed(&mut Fch, base); }
}

/// Before a halt the kernel chose: what it shows stays on the screen.
pub fn stop() {
    let tco = TCO.swap(0, Ordering::AcqRel);
    if tco != 0 { unsafe { write(tco + CNT1, 2, read(tco + CNT1, 2) | TMR_HLT); } }
    let base = FCH.swap(0, Ordering::AcqRel);
    if base != 0 { fch::stop(&mut Fch, base); }
}
