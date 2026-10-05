// ACPI on `virt`: the MCFG for the PCI ECAM, the MADT for the CPUs (their MPIDRs, issue 203) and the FADT for PSCI's
// conduit (HVC or SMC); reset and power off go through PSCI. The tables are untrusted input: every length is checked
// against the identity-mapped 4 GiB.
use crate::serial_print;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static SMC: AtomicBool = AtomicBool::new(false); // PSCI through SMC instead of HVC
// The enabled CPUs' MPIDR affinities in MADT order (the boot CPU among them), and how many.
pub static CPUS: [AtomicU64; crate::cpu::MAX] = [const { AtomicU64::new(0) }; crate::cpu::MAX];
pub static CPU_COUNT: AtomicUsize = AtomicUsize::new(0);

const WINDOW: u64 = 0x1_0000_0000;

unsafe fn bytes(address: u64, len: u64) -> Option<&'static [u8]> {
    (address != 0 && address.checked_add(len)? <= WINDOW).then(|| core::slice::from_raw_parts(address as *const u8, len as usize))
}
fn u32_at(data: &[u8], at: usize) -> u32 { u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) }
fn u64_at(data: &[u8], at: usize) -> u64 { u64::from_le_bytes(data[at..at + 8].try_into().unwrap()) }
unsafe fn table(address: u64) -> Option<&'static [u8]> { let header = bytes(address, 36)?; bytes(address, u32_at(header, 4).max(36) as u64) }

/// Reads the MCFG (the segment-0 ECAM goes to PCI), the MADT and the FADT through the XSDT.
pub unsafe fn init(rsdp: u64) {
    let Some(root) = bytes(rsdp, 36).filter(|r| &r[..8] == b"RSD PTR " && r[15] >= 2) else { serial_print("MIND CORE KERNEL: ACPI: NO RSDP\n"); return };
    let Some(list) = table(u64_at(root, 24)) else { return };
    let mut ecam = false;
    for at in (36..list.len().saturating_sub(7)).step_by(8) {
        let Some(table) = table(u64_at(list, at)) else { continue };
        match &table[..4] {
            // Allocation entries of 16 bytes from offset 44: base, segment, first bus, last bus.
            b"MCFG" => for entry in (44..table.len().saturating_sub(15)).step_by(16) {
                if !ecam && u16::from_le_bytes([table[entry + 8], table[entry + 9]]) == 0 { crate::pcicfg::configure(u64_at(table, entry), table[entry + 10], table[entry + 11]); ecam = true; }
            },
            // Interrupt controller structures from offset 44; GICC (type 0x0B): flags at 12 (bit 0 enabled), MPIDR at 68.
            b"APIC" => {
                let mut entry = 44;
                while entry + 2 <= table.len() {
                    let (kind, length) = (table[entry], table[entry + 1] as usize);
                    if length < 2 || entry + length > table.len() { break; }
                    let count = CPU_COUNT.load(Ordering::Relaxed);
                    if kind == 0x0B && length >= 76 && u32_at(table, entry + 12) & 1 != 0 && count < CPUS.len() {
                        CPUS[count].store(u64_at(table, entry + 68) & 0xFF_00FF_FFFF, Ordering::Relaxed);
                        CPU_COUNT.store(count + 1, Ordering::Release);
                    }
                    entry += length;
                }
            }
            // ARM_BOOT_ARCH at 129: bit 0 PSCI compliant, bit 1 PSCI through HVC.
            b"FACP" if table.len() >= 131 => SMC.store(table[129] & 1 != 0 && table[129] & 2 == 0, Ordering::Relaxed),
            _ => {}
        }
    }
    if !ecam { serial_print("MIND CORE KERNEL: ACPI: NO MCFG\n"); }
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
