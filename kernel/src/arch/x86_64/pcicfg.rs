// PCI configuration mechanism #1 and the x86 interrupt targets.
// LEGACY: ports 0xCF8/0xCFC; ECAM (ACPI MCFG) replaces them (docs/legacy.md).
use super::port::{inl, outl};

pub fn last_bus() -> Option<u8> { Some(255) }

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
