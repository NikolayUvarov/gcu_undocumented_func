// ACPI: the MCFG for the PCI ECAM, the MADT for the CPUs (their MPIDRs, issue 203) and the GIC, the FADT for PSCI's
// conduit (HVC or SMC), the SPCR for the console and the GTDT for the timer (issue 205); reset and power off go through
// PSCI. The tables are untrusted input: every length is checked against the identity map.
use super::board;
use crate::serial_print;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static SMC: AtomicBool = AtomicBool::new(false); // PSCI through SMC instead of HVC
// The enabled CPUs' MPIDR affinities in MADT order (the boot CPU among them), and how many.
pub static CPUS: [AtomicU64; crate::cpu::MAX] = [const { AtomicU64::new(0) }; crate::cpu::MAX];
pub static CPU_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static INTERFACES: [AtomicU64; crate::cpu::MAX] = [const { AtomicU64::new(0) }; crate::cpu::MAX]; // GICv2 CPU interface numbers

const WINDOW: u64 = crate::mmu::IDENTITY_END;

unsafe fn bytes(address: u64, len: u64) -> Option<&'static [u8]> {
    (address != 0 && address.checked_add(len)? <= WINDOW).then(|| core::slice::from_raw_parts(address as *const u8, len as usize))
}
fn u32_at(data: &[u8], at: usize) -> u32 { u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) }
fn u64_at(data: &[u8], at: usize) -> u64 { u64::from_le_bytes(data[at..at + 8].try_into().unwrap()) }
unsafe fn table(address: u64) -> Option<&'static [u8]> { let header = bytes(address, 36)?; bytes(address, u32_at(header, 4).max(36) as u64) }

// A device address the kernel can reach (its identity map), else 0: not used.
fn mapped(address: u64) -> usize { if address != 0 && address < WINDOW { address as usize } else { 0 } }

/// Reads the MCFG (the segment-0 ECAM goes to PCI), the MADT, FADT, SPCR and GTDT through the XSDT.
pub unsafe fn init(rsdp: u64) {
    let Some(root) = bytes(rsdp, 36).filter(|r| &r[..8] == b"RSD PTR " && r[15] >= 2) else { serial_print("MIND CORE KERNEL: ACPI: NO RSDP\n"); return };
    let Some(list) = table(u64_at(root, 24)) else { return };
    // QEMU (`virt`): its PL011 and PL031, the SPCR naming the same UART.
    if list.len() >= 16 && &list[10..16] == b"BOCHS " { board::set(&board::UART, board::VIRT_UART); board::set(&board::RTC, board::VIRT_RTC); }
    let mut ecam = false;
    let mut definitions = [0u64; 8]; // the DSDT (from the FADT) and the SSDTs, scanned for pin controllers
    let mut count = 0;
    for at in (36..list.len().saturating_sub(7)).step_by(8) {
        let Some(table) = table(u64_at(list, at)) else { continue };
        if &table[..4] == b"SSDT" && count < definitions.len() { definitions[count] = u64_at(list, at); count += 1; }
        // The DSDT: X_DSDT at 140 (64-bit), else DSDT at 40.
        if &table[..4] == b"FACP" && count < definitions.len() {
            let x = if table.len() >= 148 { u64_at(table, 140) } else { 0 };
            definitions[count] = if x != 0 { x } else if table.len() >= 44 { u32_at(table, 40) as u64 } else { 0 }; count += 1;
        }
        match &table[..4] {
            // Allocation entries of 16 bytes from offset 44: base, segment, first bus, last bus.
            b"MCFG" => for entry in (44..table.len().saturating_sub(15)).step_by(16) {
                if !ecam && u16::from_le_bytes([table[entry + 8], table[entry + 9]]) == 0 { crate::pcicfg::configure(u64_at(table, entry), table[entry + 10], table[entry + 11]); ecam = true; }
            },
            // Interrupt controller structures from offset 44: GICC (0x0B: interface number at 4, flags at 12, bit 0
            // enabled; the GICv2 CPU interface at 32, GICR base at 60, MPIDR at 68), GICD (0x0C: base at 8, version at
            // 20), GICv2m MSI frame (0x0D: base at 8), GICR (0x0E: base of the first range at 4), ITS (0x0F: base at 8).
            b"APIC" => {
                let (mut entry, mut its, mut gicr, mut gicc_gicr) = (44, 0, 0, 0);
                while entry + 2 <= table.len() {
                    let (kind, length) = (table[entry], table[entry + 1] as usize);
                    if length < 2 || entry + length > table.len() { break; }
                    let count = CPU_COUNT.load(Ordering::Relaxed);
                    match kind {
                        0x0B if length >= 76 && u32_at(table, entry + 12) & 1 != 0 => {
                            if count < CPUS.len() {
                                CPUS[count].store(u64_at(table, entry + 68) & 0xFF_00FF_FFFF, Ordering::Relaxed);
                                INTERFACES[count].store(u32_at(table, entry + 4) as u64, Ordering::Relaxed);
                                CPU_COUNT.store(count + 1, Ordering::Release);
                            }
                            if gicc_gicr == 0 { gicc_gicr = u64_at(table, entry + 60); }
                            if count == 0 && u64_at(table, entry + 32) != 0 { board::set(&board::GICC, mapped(u64_at(table, entry + 32))); }
                        }
                        0x0C if length >= 24 => {
                            board::set(&board::GICD, mapped(u64_at(table, entry + 8)));
                            if matches!(table[entry + 20], 1 | 2) { board::set(&board::GIC_VERSION, 2); }
                        }
                        0x0D if length >= 24 => board::set(&board::V2M, mapped(u64_at(table, entry + 8))),
                        0x0E if length >= 16 && gicr == 0 => gicr = u64_at(table, entry + 4),
                        0x0F if length >= 20 && its == 0 => its = u64_at(table, entry + 8),
                        _ => {}
                    }
                    entry += length;
                }
                // A machine without an ITS has no MSIs here; redistributors are named by GICR ranges or in each GICC.
                board::set(&board::GITS, mapped(its));
                let first = if gicr != 0 { gicr } else { gicc_gicr };
                if first != 0 { board::set(&board::GICR, mapped(first)); }
            }
            // The console: interface type at 36 (3 a PL011, 0x0E the SBSA generic UART), the register address (a
            // generic address structure) at 40, its address at 44; the interrupt (GSIV) at 54 when bit 3 of 52 is set.
            b"SPCR" if table.len() >= 58 => {
                // Another kind of UART (a 16550, a mini UART) is not driven: no console rather than a wrong address.
                if matches!(table[36], 0x03 | 0x0E) && table[40] == 0 {
                    board::set(&board::UART, mapped(u64_at(table, 44)));
                    if table[52] & 8 != 0 { let gsiv = u32_at(table, 54) as usize; if gsiv >= 32 { board::set(&board::UART_LINE, gsiv - 32); } }
                }
            }
            // The generic timer: the virtual EL1 timer's GSIV at 64 (a PPI).
            b"GTDT" if table.len() >= 68 => { let ppi = u32_at(table, 64) as usize; if (16..32).contains(&ppi) { board::set(&board::TIMER_PPI, ppi); } }
            // ARM_BOOT_ARCH at 129: bit 0 PSCI compliant, bit 1 PSCI through HVC.
            b"FACP" if table.len() >= 131 => SMC.store(table[129] & 1 != 0 && table[129] & 2 == 0, Ordering::Relaxed),
            _ => {}
        }
    }
    if !ecam { serial_print("MIND CORE KERNEL: ACPI: NO MCFG\n"); }
    for &address in &definitions[..count] { if let Some(block) = table(address).filter(|t| t.len() > 36) { pins(&block[36..]); } }
    use core::fmt::Write;
    let _ = writeln!(crate::PanicSerial, "MIND CORE KERNEL: BOARD GICV{} GICD={:#x} GICR={:#x} ITS={:#x} UART={:#x} LINE {} TIMER PPI {} CPUS {}\r",
                     board::get(&board::GIC_VERSION), board::get(&board::GICD), board::get(&board::GICR), board::get(&board::GITS), board::get(&board::UART),
                     board::get(&board::UART_LINE), board::get(&board::TIMER_PPI), CPU_COUNT.load(Ordering::Acquire));
    report_pins();
    if CPU_COUNT.load(Ordering::Acquire) == 0 { serial_print("MIND CORE KERNEL: ACPI: NO GICC IN THE MADT, ONE CPU\n"); }
}

/// A PSCI call through the conduit the FADT names; its result.
pub unsafe fn psci(function: u64, a: u64, b: u64, c: u64) -> i64 {
    let result: i64;
    if SMC.load(Ordering::Relaxed) {
        core::arch::asm!("smc #0", inout("x0") function as i64 => result, in("x1") a, in("x2") b, in("x3") c, options(nostack));
    } else {
        core::arch::asm!("hvc #0", inout("x0") function as i64 => result, in("x1") a, in("x2") b, in("x3") c, options(nostack));
    }
    result
}
pub const PSCI_CPU_ON: u64 = 0xC400_0003;
const PSCI_SYSTEM_OFF: u64 = 0x8400_0008;
const PSCI_SYSTEM_RESET: u64 = 0x8400_0009;

pub unsafe fn reboot() -> ! {
    serial_print("MIND CORE KERNEL: REBOOT VIA PSCI SYSTEM_RESET\n");
    psci(PSCI_SYSTEM_RESET, 0, 0, 0);
    crate::cpu::halt_all()
}

/// Turns the machine off (PSCI SYSTEM_OFF).
pub unsafe fn power_off() -> ! {
    serial_print("MIND CORE KERNEL: POWER OFF VIA PSCI SYSTEM_OFF\n");
    psci(PSCI_SYSTEM_OFF, 0, 0, 0);
    crate::cpu::halt_all()
}

// Pin controllers of a definition block: those with a window in the identity map. The BCM2711's GPIO, whose _CRS the
// Raspberry Pi 4 firmware computes at run time, is taken at its fixed address only beside the BCM2711's GIC-400.
unsafe fn pins(aml: &[u8]) {
    use super::aml::Pins;
    super::aml::pin_controllers(aml, |kind, window| {
        let window = match (kind, window) {
            (_, Some((base, size))) if base != 0 && size != 0 && base.checked_add(size).is_some_and(|end| end <= WINDOW) => Some((base, size)),
            (Pins::Bcm2711, None) if board::get(&board::GICD) == BCM2711_GICD => Some((BCM2711_GPIO, 0x1000)),
            _ => None,
        };
        let Some((base, size)) = window else { serial_print("MIND CORE KERNEL: ACPI: A PIN CONTROLLER WITHOUT A READABLE WINDOW, NOT USED\n"); return };
        let first = if kind == Pins::Bcm2711 { board::PINS_BCM2711 } else { 0 };
        let slots = &board::PINS[first..first + crate::abi::PLATFORM_PINS_MAX];
        if slots.iter().any(|s| board::get(&s[0]) == base as usize) { return; } // named in two blocks
        if let Some(slot) = slots.iter().find(|s| board::get(&s[0]) == 0) { board::set(&slot[0], base as usize); board::set(&slot[1], size as usize); }
    });
}
const BCM2711_GICD: usize = 0xFF84_1000; const BCM2711_GPIO: u64 = 0xFE20_0000;

fn report_pins() {
    use core::fmt::Write;
    let mut any = false;
    for (slot, [base, size]) in board::PINS.iter().enumerate() {
        if board::get(base) == 0 { continue; }
        any = true;
        let kind = if slot >= board::PINS_BCM2711 { "BCM2711" } else { "PL061" };
        let _ = writeln!(crate::PanicSerial, "MIND CORE KERNEL: PINS {} AT {:#x} ({} BYTES)\r", kind, board::get(base), board::get(size));
    }
    if !any { serial_print("MIND CORE KERNEL: PINS: NO PIN CONTROLLER IN THE ACPI TABLES\n"); }
}
