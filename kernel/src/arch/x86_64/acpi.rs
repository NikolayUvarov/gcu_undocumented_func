// ACPI: only what a reset needs, the FADT reset register, read once at boot from the firmware's tables (below 4 GiB,
// identity-mapped). The tables are untrusted input: every length is checked against the 4 GiB window.
use super::port::{inb, outb, outl};
use crate::serial_print;
use core::sync::atomic::{AtomicU64, Ordering};

// Reset register: address space << 56 | value << 48 | address (0: none).
static RESET: AtomicU64 = AtomicU64::new(0);
const WINDOW: u64 = 0x1_0000_0000;

unsafe fn bytes(address: u64, len: u64) -> Option<&'static [u8]> {
    (address != 0 && address.checked_add(len)? <= WINDOW).then(|| core::slice::from_raw_parts(address as *const u8, len as usize))
}
fn u32_at(data: &[u8], at: usize) -> u32 { u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) }
fn u64_at(data: &[u8], at: usize) -> u64 { u64::from_le_bytes(data[at..at + 8].try_into().unwrap()) }
// A whole table with signature check, or None.
unsafe fn table(address: u64) -> Option<&'static [u8]> { let header = bytes(address, 36)?; bytes(address, u32_at(header, 4).max(36) as u64) }

/// Finds the FADT through the RSDP the bootloader passed and keeps its reset register, if it has one.
pub unsafe fn init(rsdp: u64) {
    let Some(root) = bytes(rsdp, 36).filter(|r| &r[..8] == b"RSD PTR ") else { serial_print("MIND CORE KERNEL: ACPI: NO RSDP\n"); return };
    let (list, entry) = if root[15] >= 2 && u64_at(root, 24) != 0 { (u64_at(root, 24), 8) } else { (u32_at(root, 16) as u64, 4) };
    let Some(list) = table(list) else { return };
    for at in (36..list.len().saturating_sub(entry - 1)).step_by(entry) {
        let address = if entry == 8 { u64_at(list, at) } else { u32_at(list, at) as u64 };
        let Some(fadt) = table(address) else { continue };
        // Flags bit 10: RESET_REG_SUP; the generic address at 116, the value at 128 (FADT revision 2 and later).
        if &fadt[..4] != b"FACP" { continue; }
        if fadt.len() < 129 || u32_at(fadt, 112) & 1 << 10 == 0 { serial_print(if fadt.len() < 129 { "MIND CORE KERNEL: ACPI: FADT WITHOUT A RESET REGISTER (REVISION 1)\n" } else { "MIND CORE KERNEL: ACPI: RESET REGISTER NOT SUPPORTED\n" }); return; }
        let (space, register) = (fadt[116], u64_at(fadt, 120));
        if matches!(space, 0..=2) && register != 0 && register < 1 << 48 { RESET.store((space as u64) << 56 | (fadt[128] as u64) << 48 | register, Ordering::Release); serial_print("MIND CORE KERNEL: ACPI: RESET REGISTER FOUND\n"); }
        return;
    }
    serial_print("MIND CORE KERNEL: ACPI: NO FADT\n");
}

fn pause() { for _ in 0..1_000_000 { core::hint::spin_loop(); } }

/// Resets the machine; the other CPUs are stopped first.
pub unsafe fn reboot() -> ! {
    core::arch::asm!("cli");
    crate::cpu::stop_others();
    let reset = RESET.load(Ordering::Acquire);
    if reset != 0 {
        serial_print("MIND CORE KERNEL: REBOOT VIA ACPI RESET REGISTER\n");
        let (space, value, address) = ((reset >> 56) as u8, (reset >> 48) as u8, reset & 0xFFFF_FFFF_FFFF);
        match space {
            0 if address < WINDOW => core::ptr::write_volatile(address as *mut u8, value),
            1 => outb(address as u16, value),
            // PCI configuration space: device << 32 | function << 16 | offset (bus 0).
            2 => { outl(0xCF8, 0x8000_0000 | ((address >> 32) as u32 & 0x1F) << 11 | ((address >> 16) as u32 & 7) << 8 | (address as u32 & 0xFC)); outb(0xCFC + (address as u16 & 3), value); }
            _ => {}
        }
        pause();
    }
    serial_print("MIND CORE KERNEL: REBOOT VIA PORT 0xCF9\n");
    outb(0xCF9, 0x02); pause(); outb(0xCF9, 0x06); pause();
    serial_print("MIND CORE KERNEL: REBOOT VIA THE 8042 CONTROLLER\n");
    for _ in 0..100_000 { if inb(0x64) & 2 == 0 { break; } }
    outb(0x64, 0xFE); pause();
    // A triple fault: no IDT, then an exception.
    serial_print("MIND CORE KERNEL: REBOOT BY TRIPLE FAULT\n");
    let empty = [0u16; 5];
    core::arch::asm!("lidt [{}]", "int3", in(reg) empty.as_ptr(), options(noreturn));
}
