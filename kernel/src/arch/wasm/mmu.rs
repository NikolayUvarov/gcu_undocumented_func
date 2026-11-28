// The aarch64 descriptors of arch/aarch64/mmu.rs (215-PRT-0017), which the `a64` emulator walks for a task: the same
// encodings, so paging.rs builds the same spaces. The kernel itself reaches all memory directly (the module's memory is
// its physical memory), so it has no identity map: a space's root is the task's alone.
use crate::memory::Region;
use core::sync::atomic::{AtomicUsize, Ordering};

const PAGE: usize = 4096;
pub const VALID: u64 = 1;
pub const TABLE: u64 = 3; // a table descriptor (the leaf's permissions decide)
const PAGE_DESC: u64 = 3; // a level-3 page
const BLOCK: u64 = 1; // a level-1 or level-2 block
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
static KERNEL_ROOT: AtomicUsize = AtomicUsize::new(0);
/// Root entries every space shares with the kernel: its identity map of the first TiB.
pub const KERNEL_ENTRIES: usize = 2;
pub const IDENTITY_END: u64 = (KERNEL_ENTRIES as u64) << 39;
pub fn high_ram(_start: u64, _end: u64) -> Option<(u64, u64)> { None }
/// Where a task's window starts: L0 entry 255, clear of the identity map.
pub const USER_IMAGE: usize = 255 << 39;

/// A user page: never executable by the kernel; data never executable by the task.
pub fn leaf(physical: u64, writable: bool, executable: bool, device: bool) -> u64 {
    physical | PAGE_DESC | AF | NG | PXN | AP_EL0 | if writable { 0 } else { AP_RO } | if executable { 0 } else { UXN }
        | if device { ATTR_DEVICE } else { ATTR_NORMAL | INNER_SHAREABLE }
}
/// A user page of a framebuffer: as other memory here (the x86 kernel makes it write-combining, 000-KRN-0081).
pub fn write_combining(entry: u64) -> u64 { entry }
pub fn user_table(entry: u64) -> bool { entry & 3 == TABLE }
pub fn user_readable(entry: u64) -> bool { entry & 3 == PAGE_DESC && entry & AP_EL0 != 0 }
pub fn user_writable(entry: u64) -> bool { user_readable(entry) && entry & AP_RO == 0 }
/// (writable, executable, device) of a user page.
pub fn attributes(entry: u64) -> (bool, bool, bool) { (entry & AP_RO == 0, entry & UXN == 0, entry & ATTR_MASK == ATTR_DEVICE) }
/// A user page's or block's entry made writable or read-only.
pub fn with_write(entry: u64, writable: bool) -> u64 { if writable { entry & !AP_RO } else { entry | AP_RO } }
/// Whether a valid entry at level 1 or 2 maps memory itself: a 1 GiB or 2 MiB block (262-KRN-0161).
pub fn large(entry: u64) -> bool { entry & 3 == BLOCK }
/// A user page's descriptor as a block with the same attributes.
pub fn block(page: u64) -> u64 { page & !3 | BLOCK }
/// A block's descriptor in a page's form, for the checks of its permissions.
pub fn page(entry: u64) -> u64 { entry | PAGE_DESC }
/// Level-1 blocks (1 GiB) exist with the 4 KiB granule.
pub fn huge_pages() -> bool { true }
/// No identity map: the kernel runs outside the emulator, so a space's kernel entries stay invalid.
pub fn kernel_entry(_index: usize) -> u64 { 0 }
pub fn kernel_root() -> usize { KERNEL_ROOT.load(Ordering::Acquire) }

/// The space's translations changed: the emulator forgets cached ones.
pub fn flush(_root: usize) { super::run::flush_translations(); }

/// The space the emulator walks from now on.
pub unsafe fn activate(root: usize) { super::run::set_root(root as u64); }

/// Whether the kernel reaches [start, start + len) for its own writes: all of the module's memory.
pub unsafe fn reach_device(start: usize, len: usize) -> bool { start.checked_add(len).is_some_and(|end| end <= super::run::memory_bytes()) }

/// A root of no entries for the kernel's own turns (idle), when no task's space is active.
pub unsafe fn init(_map: &[crate::abi::StatPhys]) -> Result<(), &'static str> {
    let root = Region::new(PAGE, PAGE)?;
    core::ptr::write_bytes(root.ptr(), 0, PAGE);
    KERNEL_ROOT.store(root.ptr() as usize, Ordering::Release);
    core::mem::forget(root);
    activate(kernel_root());
    Ok(())
}

pub unsafe fn uncached(_physical: usize) {}
