// The chipset's watchdog (000-KRN-0076): Intel's TCO timer (tco.rs), in the LPC bridge's ACPI I/O space (ICH9 to the
// 7-series PCH, and QEMU's q35) or behind the SMBus controller (the PCHs from Cannon Lake's on, 801-KRN-0258), or the
// AMD FCH's timer (213-KRN-0125, fch.rs). The PCHs' TCO behind the SMBus controller is armed as the kernel starts, as it
// needs only configuration space and I/O ports; the others as soon as the kernel has its page tables and the screen
// (801-KRN-0258). Fed every second from the boot CPU's tick: a machine whose boot CPU
// stops is reset at the timer's expiry. A halt the kernel chose (a fatal error, HALT; a panic outside a trial) stops it
// first.
use super::fch;
use super::pcicfg;
use super::port::{read, write};
use super::tco::{self, Registers as _};
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicUsize, Ordering};

static TCO: AtomicU16 = AtomicU16::new(0); // the TCO registers' I/O base while the watchdog runs
static FCH: AtomicU32 = AtomicU32::new(0); // the AMD FCH timer's address while it runs
static RESTARTED: AtomicBool = AtomicBool::new(false);
static FRESH: AtomicBool = AtomicBool::new(false);
/// Ticks of 0.6 s to the first expiry; the reset comes at the second: 30 s without the boot CPU's tick.
pub const TICKS: u16 = 25;

/// Whether this watchdog's reset ended the last run: the second expiry's status outlives the reset.
pub fn restarted() -> bool { RESTARTED.load(Ordering::Relaxed) }

/// Whether no run of this kernel armed the timer since the machine was powered on (QEMU keeps its count across resets).
pub fn fresh() -> bool { FRESH.load(Ordering::Relaxed) }

// What `early` found, for `start`: written once on the boot CPU before any other runs.
static mut EARLY: Option<Result<tco::Armed, tco::Refused>> = None;

/// As the kernel starts, before its page tables and the FADT: the TCO of a PCH listed by its SMBus controller, which
/// needs only configuration space and I/O ports (801-KRN-0258). `start` says what it found.
pub unsafe fn early() {
    if tco::listed(&mut Chipset).is_none() { return; }
    let found = tco::arm(&mut Early, TICKS);
    if let Ok(armed) = found { TCO.store(armed.base, Ordering::Release); }
    core::ptr::addr_of_mut!(EARLY).write(Some(found));
}

/// Finds and arms the watchdog, or takes what `early` armed and quiets its SMI now that the FADT is read; the boot line
/// names it, or why there is none.
pub unsafe fn start() {
    let seconds = TICKS as u32 * 12 / 10;
    let found = match core::ptr::addr_of_mut!(EARLY).read() {
        Some(Ok(armed)) => Ok(tco::Armed { smi_kept: tco::no_smi(&mut Chipset, Chipset.pm_base()), ..armed }),
        Some(refused) => refused,
        None => tco::arm(&mut Chipset, TICKS),
    };
    let armed = match found {
        Ok(armed) => armed,
        Err(tco::Refused::NotIntel) => return amd(),
        Err(tco::Refused::Unknown(id)) => return none(format_args!("AN INTEL SMBUS CONTROLLER 8086:{:04X} AT 00:1F.4 NOT KNOWN HERE", id)),
        Err(tco::Refused::BehindP2sb(id)) => return none(format_args!("INTEL PCH WITH SMBUS 8086:{:04X}: ITS NO_REBOOT IS BEHIND THE P2SB, NOT DONE HERE", id)),
        Err(tco::Refused::Why(why)) => return none(format_args!("{}", why)),
    };
    RESTARTED.store(armed.restarted, Ordering::Relaxed);
    FRESH.store(armed.fresh, Ordering::Relaxed);
    TCO.store(armed.base, Ordering::Release);
    let mut out = crate::PanicSerial;
    let _ = match armed.kind {
        tco::Kind::Lpc => write!(out, "MIND CORE KERNEL: WATCHDOG: INTEL TCO"),
        tco::Kind::Smbus(id, name) => write!(out, "MIND CORE KERNEL: WATCHDOG: INTEL TCO OF THE {} PCH (SMBUS 8086:{:04X}, TCO AT {:#X})", name, id, armed.base),
    };
    let _ = write!(out, ", A RESET AFTER {} S WITHOUT THE BOOT CPU'S TICK{}{}{}\n", seconds,
                   if armed.decode_on { "; ITS DECODE TURNED ON HERE" } else { "" },
                   if armed.smi_kept { "; THE FIRMWARE'S SMI AT THE FIRST EXPIRY KEPT" } else { "" },
                   if armed.restarted { "; IT RESTARTED THE LAST RUN" } else { "" });
}

fn none(why: core::fmt::Arguments) { let _ = write!(crate::PanicSerial, "MIND CORE KERNEL: WATCHDOG: NONE ({})\n", why); }

// Intel's chipset: bus 0's configuration space, port I/O, the chipset configuration's GCS uncached, and the PM base the
// FADT names where its PM1a control block follows its event block, as Intel lays them out.
struct Chipset;

impl tco::Registers for Chipset {
    fn config(&mut self, device: u8, function: u8, offset: u8) -> u32 { unsafe { pcicfg::read(0, device, function, offset) } }
    fn set_config(&mut self, device: u8, function: u8, offset: u8, value: u32) { unsafe { pcicfg::write(0, device, function, offset, value) } }
    fn io(&mut self, port: u16, width: u8) -> u32 { unsafe { read(port, width as usize) as u32 } }
    fn set_io(&mut self, port: u16, width: u8, value: u32) { unsafe { write(port, width as usize, value as usize) } }
    fn gcs(&mut self, address: usize) -> u32 { unsafe { core::ptr::read_volatile(uncached(address) as *const u32) } }
    fn set_gcs(&mut self, address: usize, value: u32) { unsafe { core::ptr::write_volatile(uncached(address) as *mut u32, value) } }
    fn pm_base(&mut self) -> Option<u16> {
        let fixed = super::acpi::fixed()?;
        let (event, control) = (fixed.pm1a_evt?, fixed.pm1a_cnt?);
        (control.port == event.port + 4).then_some(event.port)
    }
}

// The chipset before the kernel's page tables: no MMIO, and no FADT read yet. The PCHs' path `early` takes uses neither.
struct Early;

impl tco::Registers for Early {
    fn config(&mut self, device: u8, function: u8, offset: u8) -> u32 { Chipset.config(device, function, offset) }
    fn set_config(&mut self, device: u8, function: u8, offset: u8, value: u32) { Chipset.set_config(device, function, offset, value) }
    fn io(&mut self, port: u16, width: u8) -> u32 { Chipset.io(port, width) }
    fn set_io(&mut self, port: u16, width: u8, value: u32) { Chipset.set_io(port, width, value) }
    fn gcs(&mut self, _: usize) -> u32 { u32::MAX }
    fn set_gcs(&mut self, _: usize, _: u32) {}
    fn pm_base(&mut self) -> Option<u16> { None }
}

// The chipset configuration mapped uncached on its first use.
unsafe fn uncached(address: usize) -> usize {
    static DONE: AtomicUsize = AtomicUsize::new(0);
    if DONE.swap(address, Ordering::Relaxed) != address { super::mmu::uncached(address); }
    address
}

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
        Err("NO AMD FCH AT 00:14.0") => none(format_args!("NO INTEL LPC BRIDGE AT 00:1F.0, NO AMD FCH AT 00:14.0")),
        Err(why) => none(format_args!("{}", why)),
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
    let base = TCO.load(Ordering::Relaxed);
    if base != 0 { tco::feed(&mut Chipset, base); }
    let base = FCH.load(Ordering::Relaxed);
    if base != 0 { fch::feed(&mut Fch, base); }
}

/// Before a halt the kernel chose: what it shows stays on the screen.
pub fn stop() {
    let base = TCO.swap(0, Ordering::AcqRel);
    if base != 0 { tco::stop(&mut Chipset, base); }
    let base = FCH.swap(0, Ordering::AcqRel);
    if base != 0 { fch::stop(&mut Fch, base); }
}
