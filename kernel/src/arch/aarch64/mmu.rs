// ARMv8 stage 1 translation, 4 KiB granule, 48-bit addresses in TTBR0 (issue 201): every space's L0 entry 0 is the
// kernel's identity map of the first 4 GiB (1 GiB blocks, EL1 only), the user window is L0 entry 1 like on x86. The
// walk is generic (kernel/src/paging.rs); the descriptors are here.
use crate::memory::Region;
use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering};

const PAGE: usize = 4096;
pub const VALID: u64 = 1;
pub const TABLE: u64 = 3; // a table descriptor (the leaf's permissions decide)
const PAGE_DESC: u64 = 3; // a level-3 page
const BLOCK: u64 = 1; // a level-1 block
const ATTR_DEVICE: u64 = 0 << 2; // MAIR attribute 0: Device-nGnRnE
const ATTR_NORMAL: u64 = 1 << 2; // MAIR attribute 1: Normal, write-back
const ATTR_MASK: u64 = 7 << 2;
const AP_EL0: u64 = 1 << 6; // EL0 may access
const AP_RO: u64 = 1 << 7; // read-only at both levels
const INNER_SHAREABLE: u64 = 3 << 8;
const AF: u64 = 1 << 10; // accessed: no access-flag faults
const NG: u64 = 1 << 11; // not global (per space)
const PXN: u64 = 1 << 53; // EL1 never executes it
const UXN: u64 = 1 << 54; // EL0 never executes it
const MAIR: u64 = 0x00 | 0xFF << 8 | 0x44 << 16; // device nGnRnE, normal write-back, normal non-cacheable
static KERNEL_ROOT: AtomicUsize = AtomicUsize::new(0);
static KERNEL_L1: AtomicUsize = AtomicUsize::new(0);

/// A user page: never executable by the kernel; data never executable by the task.
pub fn leaf(physical: u64, writable: bool, executable: bool, device: bool) -> u64 {
    physical | PAGE_DESC | AF | NG | PXN | AP_EL0 | if writable { 0 } else { AP_RO } | if executable { 0 } else { UXN }
        | if device { ATTR_DEVICE } else { ATTR_NORMAL | INNER_SHAREABLE }
}
pub fn user_table(entry: u64) -> bool { entry & 3 == TABLE }
pub fn user_readable(entry: u64) -> bool { entry & 3 == PAGE_DESC && entry & AP_EL0 != 0 }
pub fn user_writable(entry: u64) -> bool { user_readable(entry) && entry & AP_RO == 0 }
/// (writable, executable, device) of a user page.
pub fn attributes(entry: u64) -> (bool, bool, bool) { (entry & AP_RO == 0, entry & UXN == 0, entry & ATTR_MASK == ATTR_DEVICE) }
pub fn kernel_entry() -> u64 { KERNEL_L1.load(Ordering::Acquire) as u64 | TABLE }
pub fn kernel_root() -> usize { KERNEL_ROOT.load(Ordering::Acquire) }

/// The space's translations changed: forget cached ones (no ASIDs: all of them).
pub fn flush(_root: usize) {
    unsafe { asm!("dsb ishst", "tlbi vmalle1is", "dsb ish", "isb", options(nostack)); }
}

pub unsafe fn activate(root: usize) {
    asm!("msr ttbr0_el1, {}", "isb", "tlbi vmalle1", "dsb nsh", "isb", in(reg) root, options(nostack));
}

/// The identity map: the first GiB holds the devices of `virt`, the next three RAM (RAM starts at 1 GiB).
pub unsafe fn init() -> Result<(), &'static str> {
    let root = Region::new(PAGE, PAGE)?;
    let l1 = Region::new(PAGE, PAGE)?;
    for gigabyte in 0..4u64 {
        let attributes = if gigabyte == 0 { ATTR_DEVICE | PXN | UXN } else { ATTR_NORMAL | INNER_SHAREABLE | UXN };
        (l1.ptr() as *mut u64).add(gigabyte as usize).write(gigabyte << 30 | BLOCK | AF | attributes);
    }
    (root.ptr() as *mut u64).write(l1.ptr() as u64 | TABLE);
    KERNEL_L1.store(l1.ptr() as usize, Ordering::Release);
    KERNEL_ROOT.store(root.ptr() as usize, Ordering::Release);
    core::mem::forget(l1);
    core::mem::forget(root);
    enable();
    activate(kernel_root());
    Ok(())
}

// MAIR and TCR for 48-bit TTBR0 walks with the physical address size the CPU has; TTBR1 is unused.
unsafe fn enable() {
    let mmfr0: u64;
    asm!("mrs {}, id_aa64mmfr0_el1", out(reg) mmfr0);
    let ips = (mmfr0 & 0xF).min(5);
    let tcr = 16 // T0SZ: 48 bits
        | 1 << 8 | 1 << 10 | 3 << 12 // inner/outer write-back, inner shareable walks
        | 1 << 23 // EPD1: no TTBR1 walks
        | ips << 32;
    asm!("msr mair_el1, {}", "msr tcr_el1, {}", "isb", in(reg) MAIR, in(reg) tcr, options(nostack));
}

/// Device registers the kernel writes itself (MSI-X tables): the first GiB is already device memory.
pub unsafe fn uncached(_physical: usize) {}
