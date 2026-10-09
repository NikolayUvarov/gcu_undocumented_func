// x86-64 page tables: the descriptor format, the kernel's identity map and CR3 (issue 201: the walk is generic, in
// kernel/src/paging.rs).
use crate::memory::Region;
use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering};

const PAGE: usize = 4096;
pub const VALID: u64 = 1; // present
pub const TABLE: u64 = 7; // present, writable, user: the leaf decides
const WRITE: u64 = 2;
const USER: u64 = 4;
const NX: u64 = 1 << 63;
const UNCACHED: u64 = 0x18; // PCD | PWT: uncached device registers
static KERNEL_ROOT: AtomicUsize = AtomicUsize::new(0);
static KERNEL_PDPT: AtomicUsize = AtomicUsize::new(0);

/// A user page.
pub fn leaf(physical: u64, writable: bool, executable: bool, device: bool) -> u64 {
    physical | VALID | USER | if writable { WRITE } else { 0 } | if executable { 0 } else { NX } | if device { UNCACHED } else { 0 }
}
/// Whether a table entry on the way to a user page lets the task through (x86 checks every level).
pub fn user_table(entry: u64) -> bool { entry & (VALID | USER) == VALID | USER }
pub fn user_readable(entry: u64) -> bool { entry & (VALID | USER) == VALID | USER }
pub fn user_writable(entry: u64) -> bool { entry & (VALID | USER | WRITE) == VALID | USER | WRITE }
/// (writable, executable, device) of a user page.
pub fn attributes(entry: u64) -> (bool, bool, bool) { (entry & WRITE != 0, entry & NX == 0, entry & UNCACHED != 0) }
/// Root entry 0 of every space: the kernel's identity map.
/// Root entries every space shares with the kernel: entry 0, its identity map of the first 4 GiB and of free RAM above.
pub const KERNEL_ENTRIES: usize = 1;
pub fn kernel_entry(_index: usize) -> u64 { KERNEL_PDPT.load(Ordering::Acquire) as u64 | 3 }
/// The end of the kernel's identity map of everything (RAM and devices); devices above it are not used.
pub const IDENTITY_END: u64 = 1 << 32;
/// Above IDENTITY_END only free RAM is mapped, in whole 2 MiB pages, up to the end of root entry 0 (issue 171).
pub const RAM_END: u64 = 1 << 39;
const LARGE: u64 = 0x20_0000;
const CONVENTIONAL: u32 = 7;
/// The part of free RAM [start, end) above IDENTITY_END that the identity map covers: whole 2 MiB pages.
pub fn high_ram(start: u64, end: u64) -> Option<(u64, u64)> {
    let (start, end) = (start.max(IDENTITY_END).next_multiple_of(LARGE), end.min(RAM_END) & !(LARGE - 1));
    (end > start).then_some((start, end))
}
/// Where a task's window starts (root entry 1).
pub const USER_IMAGE: usize = 0x80_0000_0000;
/// The space's translations changed: reload them if it is the active one.
pub fn flush(root: usize) {
    #[cfg(not(test))]
    unsafe {
        let current: usize;
        asm!("mov {}, cr3", out(reg) current);
        if current == root { activate(current); }
    }
    #[cfg(test)]
    let _ = root;
}

pub fn kernel_root() -> usize {
    KERNEL_ROOT.load(Ordering::Acquire)
}

pub unsafe fn init(map: &[crate::abi::StatPhys]) -> Result<(), &'static str> {
    // The bootloader reserves all runtime RAM below 4 GiB. Retain supervisor
    // identity mappings for the kernel, boot stack and MMIO on every CR3.
    let root = Region::new(PAGE, PAGE)?;
    let pdpt = Region::new(PAGE, PAGE)?;
    for gigabyte in 0..4 {
        let pd = Region::new(PAGE, PAGE)?;
        for page in 0..512 {
            let physical = (gigabyte * 512 + page) * 0x200000;
            let uncached = if physical >= 0xc0000000 { 0x18 } else { 0 };
            (pd.ptr() as *mut u64)
                .add(page)
                .write(physical as u64 | 0x83 | uncached);
        }
        (pdpt.ptr() as *mut u64)
            .add(gigabyte)
            .write(pd.ptr() as u64 | 3);
        core::mem::forget(pd);
    }
    // Free RAM above 4 GiB, write-back, never device windows: one page directory for each gigabyte that has some.
    for entry in map.iter().filter(|e| e.kind == CONVENTIONAL) {
        let Some((start, end)) = high_ram(entry.start, entry.start + entry.pages * PAGE as u64) else { continue };
        for large in (start..end).step_by(LARGE as usize) {
            let slot = (pdpt.ptr() as *mut u64).add((large >> 30) as usize);
            if slot.read() & VALID == 0 { let pd = Region::new(PAGE, PAGE)?; slot.write(pd.ptr() as u64 | 3); core::mem::forget(pd); }
            ((slot.read() & !0xFFF) as *mut u64).add((large >> 21 & 511) as usize).write(large | 0x83);
        }
    }
    (root.ptr() as *mut u64).write(pdpt.ptr() as u64 | 3);
    KERNEL_PDPT.store(pdpt.ptr() as usize, Ordering::Release);
    KERNEL_ROOT.store(root.ptr() as usize, Ordering::Release);
    core::mem::forget(pdpt);
    core::mem::forget(root);
    enable_protection(true);
    activate(kernel_root());
    Ok(())
}

pub unsafe fn enable_protection(bsp: bool) {
    let nx = core::arch::x86_64::__cpuid(0x80000001).edx & (1 << 20) != 0;
    assert!(nx, "NX support required");
    let mut lo: u32;
    let hi: u32;
    asm!("rdmsr", in("ecx") 0xc0000080u32, out("eax") lo, out("edx") hi);
    // This kernel exposes only int 0x80. Never inherit a firmware SYSCALL target.
    lo = (lo | (1 << 11)) & !1;
    asm!("wrmsr", in("ecx") 0xc0000080u32, in("eax") lo, in("edx") hi);
    asm!("wrmsr", in("ecx") 0x174u32, in("eax") 0u32, in("edx") 0u32); // SYSENTER_CS=0 => #GP
    let mut cr0: usize;
    asm!("mov {}, cr0", out(reg) cr0);
    cr0 = (cr0 | (1 << 16) | 2) & !12; // WP, MP; clear EM/TS for FXSAVE.
    asm!("mov cr0, {}", in(reg) cr0);
    let mut cr4: usize;
    asm!("mov {}, cr4", out(reg) cr4);
    cr4 |= (1 << 9) | (1 << 10);
    // Flush inherited global translations as well. No user FSGSBASE or PCID.
    cr4 &= !((1 << 7) | (1 << 16) | (1 << 17) | (1 << 18));
    // XSAVE with every state component programs can use when the CPU has XSAVE and AVX (issue 153, 174-KRN-0037);
    // the BSP decides, the APs follow.
    let features = core::arch::x86_64::__cpuid(1).ecx;
    let avx = features & (1 << 26) != 0 && features & (1 << 28) != 0;
    let xsave = if bsp { avx } else { crate::context::XSAVE.load(Ordering::Acquire) };
    if xsave { cr4 |= 1 << 18; }
    asm!("mov cr4, {}", in(reg) cr4);
    let xcr0 = if !xsave { 0 } else if bsp { components() } else { crate::context::XCR0.load(Ordering::Acquire) };
    if xsave { asm!("xsetbv", in("ecx") 0u32, in("eax") xcr0 as u32, in("edx") (xcr0 >> 32) as u32); }
    let size = if xsave { core::arch::x86_64::__cpuid_count(0xD, 0).ebx as usize } else { 512 };
    if bsp {
        crate::context::set_area(size);
        crate::context::XCR0.store(xcr0, Ordering::Release);
        crate::context::XSAVE.store(xsave, Ordering::Release);
    }
    assert!(size <= crate::context::area(), "XSAVE area too large");
}

// x87, SSE and AVX, and where CPUID 0xD lists all of a group: AVX-512's opmask, ZMM_Hi256 and Hi16_ZMM, AMX's
// XTILECFG and XTILEDATA.
fn components() -> u64 {
    let leaf = core::arch::x86_64::__cpuid_count(0xD, 0);
    let supported = (leaf.edx as u64) << 32 | leaf.eax as u64;
    let mut xcr0 = 0b111;
    for group in [AVX512, AMX] { if supported & group == group { xcr0 |= group; } }
    xcr0
}
/// XCR0's AVX-512 and AMX components.
pub const AVX512: u64 = 0b111 << 5;
pub const AMX: u64 = 0b11 << 17;

pub unsafe fn activate(root: usize) {
    asm!("mov cr3, {}", in(reg) root);
}

/// Makes [start, start + len) reachable through the kernel's identity map for its own writes (the framebuffer of
/// 211-KRN-0013): below 4 GiB it is; above, the 2 MiB pages not mapped yet are added, uncached. False beyond RAM_END.
pub unsafe fn reach_device(start: usize, len: usize) -> bool {
    let (start, end) = (start as u64, (start + len) as u64);
    if end <= IDENTITY_END { return true; }
    if end > RAM_END { return false; }
    let pdpt = KERNEL_PDPT.load(Ordering::Acquire) as *mut u64;
    for large in ((start.max(IDENTITY_END) & !(LARGE - 1))..end).step_by(LARGE as usize) {
        let slot = pdpt.add((large >> 30) as usize);
        if slot.read() & VALID == 0 { let Ok(pd) = Region::new(PAGE, PAGE) else { return false }; slot.write(pd.ptr() as u64 | 3); core::mem::forget(pd); }
        let entry = ((slot.read() & !0xFFF) as *mut u64).add((large >> 21 & 511) as usize);
        if entry.read() & VALID == 0 { entry.write(large | 0x83 | UNCACHED); }
    }
    true
}

/// Makes the kernel's identity mapping of the 2 MiB page holding `physical` (below 4 GiB) uncached, for device
/// registers the kernel itself writes (MSI-X tables). The caller checks that the page holds no RAM.
pub unsafe fn uncached(physical: usize) {
    let pdpt = KERNEL_PDPT.load(Ordering::Acquire) as *const u64;
    let pd = (pdpt.add(physical >> 30).read() & !0xFFF) as *mut u64;
    let entry = pd.add(physical >> 21 & 511);
    entry.write(entry.read() | 0x18);
    core::arch::asm!("invlpg [{}]", in(reg) physical, options(nostack));
}
