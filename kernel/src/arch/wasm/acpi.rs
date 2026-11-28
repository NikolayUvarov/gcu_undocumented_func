// No ACPI in a page (215-PRT-0017): a power-off or a restart is the page's to do.
use crate::serial_print;

pub unsafe fn early(_rsdp: u64) {}
pub unsafe fn init(_rsdp: u64) {}
/// A restart before anything else is set up: asked of the page.
pub unsafe fn reset() { serial_print("MIND CORE KERNEL: RESTART ASKED OF THE PAGE\n"); super::host::halt(1); }
pub unsafe fn reboot() -> ! { serial_print("MIND CORE KERNEL: RESTART ASKED OF THE PAGE\n"); super::host::stop(1) }
pub unsafe fn power_off() -> ! { serial_print("MIND CORE KERNEL: POWER OFF: THE PAGE ENDS THE KERNEL\n"); super::host::stop(0) }
pub fn tables(_each: impl FnMut(&'static [u8])) {}
