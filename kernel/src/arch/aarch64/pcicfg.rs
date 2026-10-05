// PCI configuration through ECAM (issue 202), at the base the ACPI MCFG gives. Only an ECAM in the identity-mapped
// 4 GiB is used: `virt` puts it at 0x3F00_0000 with `highmem=off`, otherwise above it and PCI stays off. Legacy
// interrupts INTA-D of slot s, pin p are SPIs 3 + (s + p - 1) % 4: device lines 3-6. MSI needs the GICv3 ITS, not used yet.
use core::sync::atomic::{AtomicUsize, Ordering};

static ECAM: AtomicUsize = AtomicUsize::new(0); // base | last bus (bits 0-7); 0: none
const WINDOW: u64 = 0x1_0000_0000;

/// Takes segment 0 of the MCFG, starting at bus 0, if its whole range lies below 4 GiB.
pub fn configure(base: u64, first_bus: u8, last_bus: u8) {
    let end = base + (last_bus as u64 + 1) * (1 << 20);
    if first_bus != 0 || base & 0xFF_FFFF != 0 || end > WINDOW { crate::serial_print("MIND CORE KERNEL: PCI: ECAM ABOVE 4 GIB, NOT USED\n"); return; }
    ECAM.store(base as usize | last_bus as usize, Ordering::Release);
}
pub fn last_bus() -> Option<u8> { let ecam = ECAM.load(Ordering::Acquire); (ecam != 0).then_some(ecam as u8) }

fn address(bus: u8, device: u8, function: u8, offset: u8) -> usize {
    (ECAM.load(Ordering::Relaxed) & !0xFF) | (bus as usize) << 20 | (device as usize) << 15 | (function as usize) << 12 | (offset as usize & 0xFC)
}
pub unsafe fn read(bus: u8, device: u8, function: u8, offset: u8) -> u32 { core::ptr::read_volatile(address(bus, device, function, offset) as *const u32) }
pub unsafe fn write(bus: u8, device: u8, function: u8, offset: u8, value: u32) { core::ptr::write_volatile(address(bus, device, function, offset) as *mut u32, value) }
pub unsafe fn line(bus: u8, device: u8, function: u8) -> u8 {
    let pin = (read(bus, device, function, 0x3C) >> 8) as u8;
    if pin == 0 || pin > 4 || bus != 0 { return 0; }
    3 + (device + pin - 1) % 4
}
pub fn msi_address(_cpu: u32) -> Option<u64> { None }
