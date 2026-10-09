// Platform resources init may hand to drivers outside PCI (PLATFORM_PORTS, PLATFORM_IRQ, PLATFORM_MMIO).
// LEGACY: ISA devices of the platform profile (docs/legacy.md): PS/2, CMOS, primary ATA, COM1. The PIC, PIT and PCI
// configuration ports stay with the kernel.
pub const PORTS: [(u16, u16); 6] = [(0x60, 1), (0x64, 1), (0x70, 2), (0x1F0, 8), (0x3F6, 1), (0x3F8, 8)];
/// Registers outside PCI: only the TPM's (351-KRN-0052), where the TPM2 table names them; the UART and the RTC are on
/// ports.
pub fn mmio(index: usize) -> Option<(usize, usize)> {
    if index != crate::abi::PLATFORM_TPM { return None; }
    let mut found = None;
    super::acpi::tables(|table| if found.is_none() { found = crate::tpm2::registers(table, true); });
    found.map(|base| (base as usize, 0x1000))
}
pub fn console() -> usize { usize::MAX } // the kernel's console is not MMIO here
/// ISA lines 1..15 except the cascade (2).
pub fn irq(line: usize) -> bool { (1..16).contains(&line) && line != 2 }
