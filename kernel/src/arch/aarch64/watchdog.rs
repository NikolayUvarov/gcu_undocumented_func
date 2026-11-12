// The Allwinner SoCs' watchdog (100004-PRT-0015): the A31's, which the A64, the H5 and the H616 name in their device
// trees, at the base board.rs's WATCHDOG holds; its registers as the SoCs' user manuals name them. Armed with the tick to
// reset the whole system 16 s after the last feed (its longest interval), fed every second from the boot CPU's tick, and
// stopped before a halt the kernel chose, a reboot (TF-A's SYSTEM_RESET may use the same timer) and a power-off. QEMU's
// `virt` has none. The register code is sunxi_wdt.rs.
use super::sunxi_wdt::{arm, disable, restart, Registers, SECONDS};
use core::fmt::Write;
use core::sync::atomic::{AtomicUsize, Ordering};

// Its base while it runs; 0: none, or stopped.
static BASE: AtomicUsize = AtomicUsize::new(0);

struct Mapped(usize);

impl Registers for Mapped {
    fn read(&mut self, offset: usize) -> u32 { unsafe { core::ptr::read_volatile((self.0 + offset) as *const u32) } }
    fn write(&mut self, offset: usize, value: u32) { unsafe { core::ptr::write_volatile((self.0 + offset) as *mut u32, value) } }
}

/// Arms the watchdog the device tree names; the boot line names it, or says there is none.
pub fn start() {
    let base = super::board::get(&super::board::WATCHDOG);
    if base == 0 { return crate::serial_print("MIND CORE KERNEL: WATCHDOG: NONE (THE FIRMWARE NAMES NO WATCHDOG THIS KERNEL DRIVES)\n"); }
    match arm(&mut Mapped(base)) {
        Ok(()) => {
            BASE.store(base, Ordering::Release);
            let _ = writeln!(crate::PanicSerial, "MIND CORE KERNEL: WATCHDOG: ALLWINNER A31'S AT {:#x}, A RESET AFTER {} S WITHOUT THE BOOT CPU'S TICK\r", base, SECONDS);
        }
        Err(why) => { let _ = writeln!(crate::PanicSerial, "MIND CORE KERNEL: WATCHDOG: NONE (ALLWINNER A31'S AT {:#x}: {})\r", base, why); }
    }
}

/// On the boot CPU's tick, every second.
pub fn feed() {
    let base = BASE.load(Ordering::Relaxed);
    if base != 0 { restart(&mut Mapped(base)); }
}

/// Before a halt the kernel chose, a reboot or a power-off.
pub fn stop() {
    let base = BASE.swap(0, Ordering::AcqRel);
    if base != 0 { disable(&mut Mapped(base)); }
}
