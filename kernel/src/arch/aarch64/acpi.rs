// ACPI on `virt`: the MCFG for the PCI ECAM; reset goes through PSCI (HVC conduit), not the FADT. The tables are
// untrusted input: every length is checked against the identity-mapped 4 GiB.
use crate::serial_print;

const WINDOW: u64 = 0x1_0000_0000;

unsafe fn bytes(address: u64, len: u64) -> Option<&'static [u8]> {
    (address != 0 && address.checked_add(len)? <= WINDOW).then(|| core::slice::from_raw_parts(address as *const u8, len as usize))
}
fn u32_at(data: &[u8], at: usize) -> u32 { u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) }
fn u64_at(data: &[u8], at: usize) -> u64 { u64::from_le_bytes(data[at..at + 8].try_into().unwrap()) }
unsafe fn table(address: u64) -> Option<&'static [u8]> { let header = bytes(address, 36)?; bytes(address, u32_at(header, 4).max(36) as u64) }

/// Finds the MCFG through the XSDT and hands its segment-0 ECAM to PCI.
pub unsafe fn init(rsdp: u64) {
    let Some(root) = bytes(rsdp, 36).filter(|r| &r[..8] == b"RSD PTR " && r[15] >= 2) else { serial_print("MIND CORE KERNEL: ACPI: NO RSDP\n"); return };
    let Some(list) = table(u64_at(root, 24)) else { return };
    for at in (36..list.len().saturating_sub(7)).step_by(8) {
        let Some(mcfg) = table(u64_at(list, at)) else { continue };
        if &mcfg[..4] != b"MCFG" { continue; }
        // Allocation entries of 16 bytes from offset 44: base, segment, first bus, last bus.
        for entry in (44..mcfg.len().saturating_sub(15)).step_by(16) {
            if u16::from_le_bytes([mcfg[entry + 8], mcfg[entry + 9]]) == 0 { crate::pcicfg::configure(u64_at(mcfg, entry), mcfg[entry + 10], mcfg[entry + 11]); return; }
        }
    }
    serial_print("MIND CORE KERNEL: ACPI: NO MCFG\n");
}

pub unsafe fn reboot() -> ! {
    serial_print("MIND CORE KERNEL: REBOOT VIA PSCI SYSTEM_RESET\n");
    core::arch::asm!("hvc #0", in("x0") 0x8400_0009u64, options(nostack)); // PSCI SYSTEM_RESET
    crate::cpu::halt_all()
}
