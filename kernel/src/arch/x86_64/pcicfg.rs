// PCI configuration mechanism #1 and the x86 interrupt targets.
// LEGACY: ports 0xCF8/0xCFC; ECAM (ACPI MCFG) replaces them (docs/legacy.md).
use super::port::{inl, outl};

pub fn last_bus() -> Option<u8> { Some(255) }

// The MCFG's ECAM range for segment 0 (base, end): configuration goes through the ports, but those addresses are
// taken, so no BAR is moved there (211-KRN-0021).
static ECAM: [core::sync::atomic::AtomicU64; 2] = [const { core::sync::atomic::AtomicU64::new(0) }; 2];
pub fn reserve_ecam(base: u64, first_bus: u8, last_bus: u8) {
    let start = base + ((first_bus as u64) << 20);
    ECAM[0].store(start, core::sync::atomic::Ordering::Relaxed);
    ECAM[1].store(base + ((last_bus as u64 + 1) << 20), core::sync::atomic::Ordering::Release);
}
pub fn ecam() -> Option<(u64, u64)> {
    let end = ECAM[1].load(core::sync::atomic::Ordering::Acquire);
    (end != 0).then(|| (ECAM[0].load(core::sync::atomic::Ordering::Relaxed), end))
}

pub unsafe fn read(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    outl(0xCF8, 0x8000_0000 | (bus as u32) << 16 | (device as u32) << 11 | (function as u32) << 8 | (offset as u32 & 0xFC));
    inl(0xCFC)
}
pub unsafe fn write(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    outl(0xCF8, 0x8000_0000 | (bus as u32) << 16 | (device as u32) << 11 | (function as u32) << 8 | (offset as u32 & 0xFC));
    outl(0xCFC, value);
}
/// The legacy line the firmware assigned (8259 lines 1-15), 0 for none.
pub unsafe fn line(bus: u8, device: u8, function: u8) -> u8 {
    let irq = read(bus, device, function, 0x3C) as u8;
    if irq < 16 { irq } else { 0 }
}
/// MSI message (address, data) for MSI line `index`: vector 0x40 + index at the boot processor's local APIC.
pub unsafe fn msi_message(_location: u32, index: usize) -> Option<(u64, u32)> { Some((0xFEE0_0000 | (crate::cpu::apic_id(0) as u64) << 12, 0x40 + index as u32)) }
