// The chipset's watchdog (000-KRN-0076): the TCO timer of Intel's LPC bridge (ICH9 to the 7-series PCH, and QEMU's q35),
// in its ACPI I/O space. Armed with the tick and fed every second from the boot CPU's tick: a machine whose boot CPU stops
// is reset at the timer's second expiry. A halt the kernel chose (a panic, a fatal error, HALT) stops it first.
use super::pcicfg;
use super::port::{inl, outl, read, write};
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU16, Ordering};

static TCO: AtomicU16 = AtomicU16::new(0); // the TCO registers' I/O base while the watchdog runs
static RESTARTED: AtomicBool = AtomicBool::new(false);
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

/// Finds and arms the watchdog; the boot line names it, or why there is none.
pub unsafe fn start() {
    let (id, class) = (pcicfg::read(0, 31, 0, 0), pcicfg::read(0, 31, 0, 8) >> 8);
    if id & 0xFFFF != 0x8086 || class != 0x06_0100 { return none("NO INTEL LPC BRIDGE AT 00:1F.0"); }
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
    write(tco + TMR, 2, TICKS as usize);
    write(tco + RLD, 2, 1);
    write(tco + CNT1, 2, read(tco + CNT1, 2) & !TMR_HLT);
    if read(tco + CNT1, 2) & TMR_HLT != 0 { return none("ITS TIMER STAYS HALTED"); }
    TCO.store(tco, Ordering::Release);
    let _ = write!(crate::PanicSerial, "MIND CORE KERNEL: WATCHDOG: INTEL TCO, A RESET AFTER {} S WITHOUT THE BOOT CPU'S TICK{}\n",
                   TICKS as u32 * 12 / 10, if restarted() { "; IT RESTARTED THE LAST RUN" } else { "" });
}

fn none(why: &str) { let _ = write!(crate::PanicSerial, "MIND CORE KERNEL: WATCHDOG: NONE ({})\n", why); }

/// On the boot CPU's tick, every second.
pub fn feed() {
    let tco = TCO.load(Ordering::Relaxed);
    if tco != 0 { unsafe { write(tco + RLD, 2, 1); } }
}

/// Before a halt the kernel chose: what it shows stays on the screen.
pub fn stop() {
    let tco = TCO.swap(0, Ordering::AcqRel);
    if tco != 0 { unsafe { write(tco + CNT1, 2, read(tco + CNT1, 2) | TMR_HLT); } }
}
