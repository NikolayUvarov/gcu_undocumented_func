// ACPI: the FADT reset register and the MADT's processors, read once at boot from the firmware's tables (below 4 GiB,
// identity-mapped). The tables are untrusted input: every length is checked against the 4 GiB window.
use super::port::{inb, outb, outl};
use crate::serial_print;
use core::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

// Reset register: address space << 56 | value << 48 | address (0: none).
static RESET: AtomicU64 = AtomicU64::new(0);
// xAPIC IDs of the enabled processors the MADT lists, in its order (issue 171: as many as it lists, up to xAPIC's 255).
pub static CPUS: [AtomicU32; crate::cpu::MAX] = [const { AtomicU32::new(0) }; crate::cpu::MAX];
pub static CPU_COUNT: AtomicUsize = AtomicUsize::new(0);
// ACPI PM timer (211-PRT-0003): its I/O port, bit 31 set when it counts 32 bits rather than 24 (0: none).
static PM_TIMER: AtomicU32 = AtomicU32::new(0);
const WINDOW: u64 = 0x1_0000_0000;

unsafe fn bytes(address: u64, len: u64) -> Option<&'static [u8]> {
    (address != 0 && address.checked_add(len)? <= WINDOW).then(|| core::slice::from_raw_parts(address as *const u8, len as usize))
}
fn u32_at(data: &[u8], at: usize) -> u32 { u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) }
fn u64_at(data: &[u8], at: usize) -> u64 { u64::from_le_bytes(data[at..at + 8].try_into().unwrap()) }
// A whole table with signature check, or None.
unsafe fn table(address: u64) -> Option<&'static [u8]> { let header = bytes(address, 36)?; bytes(address, u32_at(header, 4).max(36) as u64) }

static RSDP: AtomicU64 = AtomicU64::new(0);

/// Finds the FADT through the RSDP the bootloader passed and keeps its reset register, if it has one.
pub unsafe fn init(rsdp: u64) {
    RSDP.store(rsdp, Ordering::Relaxed);
    let Some(root) = bytes(rsdp, 36).filter(|r| &r[..8] == b"RSD PTR ") else { serial_print("MIND CORE KERNEL: ACPI: NO RSDP\n"); return };
    let (list, entry) = if root[15] >= 2 && u64_at(root, 24) != 0 { (u64_at(root, 24), 8) } else { (u32_at(root, 16) as u64, 4) };
    let Some(list) = table(list) else { return };
    let mut fadt = false;
    for at in (36..list.len().saturating_sub(entry - 1)).step_by(entry) {
        let address = if entry == 8 { u64_at(list, at) } else { u32_at(list, at) as u64 };
        let Some(table) = table(address) else { continue };
        if &table[..4] == b"APIC" { processors(table); continue; }
        // Allocation entries of 16 bytes from offset 44: base, segment, first bus, last bus.
        if &table[..4] == b"MCFG" {
            if let Some(entry) = (44..table.len().saturating_sub(15)).step_by(16).find(|&e| u16::from_le_bytes([table[e + 8], table[e + 9]]) == 0) {
                crate::pcicfg::reserve_ecam(u64_at(table, entry), table[entry + 10], table[entry + 11]);
            }
            continue;
        }
        // Flags bit 10: RESET_REG_SUP; the generic address at 116, the value at 128 (FADT revision 2 and later).
        if &table[..4] != b"FACP" { continue; }
        fadt = true;
        // PM timer: X_PM_TMR_BLK (generic address at 208, I/O space) or PM_TMR_BLK at 76 (length 4 at 91); flags bit 8:
        // a 32-bit counter.
        let port = if table.len() >= 220 && table[208] == 1 && u64_at(table, 212) != 0 { u64_at(table, 212) }
                   else if table.len() >= 92 && table[91] == 4 { u32_at(table, 76) as u64 } else { 0 };
        if port != 0 && port < 0x1_0000 { PM_TIMER.store(port as u32 | ((table.len() >= 116 && u32_at(table, 112) & 1 << 8 != 0) as u32) << 31, Ordering::Release); }
        if table.len() < 129 || u32_at(table, 112) & 1 << 10 == 0 { serial_print(if table.len() < 129 { "MIND CORE KERNEL: ACPI: FADT WITHOUT A RESET REGISTER (REVISION 1)\n" } else { "MIND CORE KERNEL: ACPI: RESET REGISTER NOT SUPPORTED\n" }); continue; }
        let (space, register) = (table[116], u64_at(table, 120));
        if matches!(space, 0..=2) && register != 0 && register < 1 << 48 { RESET.store((space as u64) << 56 | (table[128] as u64) << 48 | register, Ordering::Release); serial_print("MIND CORE KERNEL: ACPI: RESET REGISTER FOUND\n"); }
    }
    if !fadt { serial_print("MIND CORE KERNEL: ACPI: NO FADT\n"); }
}

/// The RSDP and every table the RSDT or XSDT lists (itself first), with the DSDT and FACS the FADT points at, for the
/// hardware report (174-KRN-0038).
pub fn tables(mut each: impl FnMut(&'static [u8])) {
    unsafe {
        let Some(root) = bytes(RSDP.load(Ordering::Relaxed), 20).filter(|r| &r[..8] == b"RSD PTR ") else { return };
        let root = if root[15] >= 2 { bytes(RSDP.load(Ordering::Relaxed), 36).unwrap_or(root) } else { root };
        each(root);
        let (list, entry) = if root.len() >= 36 && u64_at(root, 24) != 0 { (u64_at(root, 24), 8) } else { (u32_at(root, 16) as u64, 4) };
        let Some(list) = table(list) else { return };
        each(list);
        for at in (36..list.len().saturating_sub(entry - 1)).step_by(entry) {
            let Some(found) = table(if entry == 8 { u64_at(list, at) } else { u32_at(list, at) as u64 }) else { continue };
            each(found);
            // X_DSDT at 140, else DSDT at 40; X_FIRMWARE_CTRL at 132, else FIRMWARE_CTRL at 36 (the FACS).
            if &found[..4] == b"FACP" {
                let wide = |at: usize| if found.len() >= at + 8 { u64_at(found, at) } else { 0 };
                for (x, legacy) in [(140, 40), (132, 36)] {
                    let address = if wide(x) != 0 { wide(x) } else if found.len() >= legacy + 4 { u32_at(found, legacy) as u64 } else { 0 };
                    if let Some(pointed) = table(address) { each(pointed); }
                }
            }
        }
    }
}

/// The ACPI PM timer's port and counter mask, if the FADT names one.
pub fn pm_timer() -> Option<(u16, u32)> {
    let value = PM_TIMER.load(Ordering::Acquire);
    (value & 0xFFFF != 0).then(|| (value as u16, if value & 1 << 31 != 0 { u32::MAX } else { 0xFF_FFFF }))
}

// Processor Local APIC entries (type 0, 8 bytes): APIC ID at 3, flags at 4 (bit 0 enabled). x2APIC entries (type 9)
// name IDs above 254, which xAPIC mode cannot address: they are not started.
fn processors(madt: &[u8]) {
    let (mut at, mut count) = (44, 0);
    while at + 2 <= madt.len() {
        let (kind, len) = (madt[at], madt[at + 1] as usize);
        if len < 2 || at + len > madt.len() { break; }
        if kind == 0 && len >= 8 && u32_at(madt, at + 4) & 1 != 0 && madt[at + 3] != 0xFF && count < CPUS.len() {
            CPUS[count].store(madt[at + 3] as u32, Ordering::Relaxed); count += 1;
        }
        // Processor Local x2APIC (211-PRT-0002): firmware in x2APIC mode may list processors only this way.
        if kind == 9 && len >= 16 && u32_at(madt, at + 8) & 1 != 0 && u32_at(madt, at + 4) != u32::MAX && count < CPUS.len() {
            CPUS[count].store(u32_at(madt, at + 4), Ordering::Relaxed); count += 1;
        }
        at += len;
    }
    CPU_COUNT.store(count, Ordering::Release);
}

fn pause() { for _ in 0..1_000_000 { core::hint::spin_loop(); } }

/// Resets the machine; the other CPUs are stopped first.
/// Power off needs the ACPI sleep state S5 from the DSDT's AML: not read yet.
pub unsafe fn power_off() -> Result<usize, usize> { Err(crate::abi::ERR_INVALID) }

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
