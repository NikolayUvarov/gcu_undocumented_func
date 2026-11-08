// Platform resources init may hand to drivers outside PCI (PLATFORM_PORTS, PLATFORM_IRQ, PLATFORM_MMIO).
// LEGACY: ISA devices of the platform profile (docs/legacy.md): PS/2, CMOS, primary ATA, COM1. The PIC, PIT and PCI
// configuration ports stay with the kernel.
pub const PORTS: [(u16, u16); 6] = [(0x60, 1), (0x64, 1), (0x70, 2), (0x1F0, 8), (0x3F6, 1), (0x3F8, 8)];
/// Whether ports `start..end` may go to a driver: the platform profile's above, one of the two of the embedded
/// controller that the ECDT names (211-PRT-0008: the battery service's), or a PM1 event block of the FADT's.
pub fn ports(start: usize, end: usize) -> bool {
    let within = |base: u16, count: u16| start >= base as usize && end <= base as usize + count as usize;
    PORTS.iter().any(|&(base, count)| within(base, count)) || ec_ports().iter().flatten().any(|&port| within(port, 1)) || pm1_events(start, end)
}
// A PM1 event block the FADT names (000-KRN-0216: init's power button), lent once the kernel has put the chipset in
// ACPI mode with every GPE and fixed event off; SMI_CMD, PM1 control and the GPE blocks stay with the kernel.
fn pm1_events(start: usize, end: usize) -> bool {
    let Some(f) = super::acpi::fixed() else { return false };
    [f.pm1a_evt, f.pm1b_evt].into_iter().flatten().any(|b| b.holds(start, end)) && unsafe { super::acpi::events_ready() }
}
// The ECDT's command/status and data ports (ACPI 6.5, 5.2.16), each in system I/O space.
fn ec_ports() -> [Option<u16>; 2] {
    let mut found = [None; 2];
    super::acpi::tables(|t| if t.starts_with(b"ECDT") && found[0].is_none() {
        let port = |at: usize| -> Option<u16> {
            if t.get(at) != Some(&1) { return None; }
            u16::try_from(u64::from_le_bytes(t.get(at + 4..at + 12)?.try_into().ok()?)).ok().filter(|&p| p != 0)
        };
        found = [port(36), port(48)];
    });
    found
}
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
/// A PC has ACPI: no device tree is read here (aarch64's reads one, 100000-PRT-0009).
pub unsafe fn board_from_tree(_address: u64) -> bool { false }
/// RAM reserved beyond the firmware's map: none on a PC.
pub fn reserved(_each: impl FnMut(u64, u64)) {}
