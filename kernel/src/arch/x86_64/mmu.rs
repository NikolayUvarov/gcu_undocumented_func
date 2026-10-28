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
const PWT: u64 = 0x08; // PWT alone: PAT entry 1, write-combining (000-KRN-0081)
const PAT: u64 = 0x0007_0406_0007_0106;
static KERNEL_ROOT: AtomicUsize = AtomicUsize::new(0);
static KERNEL_PDPT: AtomicUsize = AtomicUsize::new(0);

/// A user page.
pub fn leaf(physical: u64, writable: bool, executable: bool, device: bool) -> u64 {
    physical | VALID | USER | if writable { WRITE } else { 0 } | if executable { 0 } else { NX } | if device { UNCACHED } else { 0 }
}
/// A user page of a framebuffer: write-combining, so the CPU sends a run of pixel stores to the bus as one burst.
pub fn write_combining(entry: u64) -> u64 { entry & !UNCACHED | PWT }
/// Whether a table entry on the way to a user page lets the task through (x86 checks every level).
pub fn user_table(entry: u64) -> bool { entry & (VALID | USER) == VALID | USER }
pub fn user_readable(entry: u64) -> bool { entry & (VALID | USER) == VALID | USER }
pub fn user_writable(entry: u64) -> bool { entry & (VALID | USER | WRITE) == VALID | USER | WRITE }
/// (writable, executable, device) of a user page.
pub fn attributes(entry: u64) -> (bool, bool, bool) { (entry & WRITE != 0, entry & NX == 0, entry & UNCACHED == UNCACHED) }
const PS: u64 = 1 << 7; // a 2 MiB or 1 GiB page (262-KRN-0161)
static HUGE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(cfg!(test));
/// Whether a valid entry one or two levels above the 4 KiB pages maps memory itself: a 2 MiB or 1 GiB page.
pub fn large(entry: u64) -> bool { entry & PS != 0 }
/// A user page's entry as a 2 MiB or 1 GiB page with the same attributes (its PAT bit stays clear: write-back).
pub fn block(page: u64) -> u64 { page | PS }
/// A large page's entry in a 4 KiB page's form, for the checks of its permissions.
pub fn page(entry: u64) -> u64 { entry & !PS }
/// Whether the CPU has 1 GiB pages (CPUID 80000001h EDX bit 26).
pub fn huge_pages() -> bool { HUGE.load(Ordering::Relaxed) }
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

// UEFI RAM types; reserved, unusable, MMIO and unknown types never justify a cached identity mapping.
fn ram(kind: u32) -> bool { matches!(kind, 1..=7 | 9 | 10) }
fn end(entry: &crate::abi::StatPhys) -> u64 { entry.start.saturating_add(entry.pages.saturating_mul(PAGE as u64)) }

// The whole interval must be RAM, even across adjacent descriptors; a conflicting non-RAM descriptor wins.
fn ram_covers(map: &[crate::abi::StatPhys], start: u64, limit: u64) -> bool {
    if map.iter().any(|e| !ram(e.kind) && e.start < limit && start < end(e)) { return false; }
    let mut at = start;
    while at < limit {
        let next = map.iter().filter(|e| ram(e.kind) && e.start <= at).map(end).max().unwrap_or(0);
        if next <= at { return false; }
        at = next;
    }
    true
}

// One supervisor identity PDE and, at a RAM/device boundary, its owned 4 KiB page table (213-KRN-0112).
pub(crate) fn identity_region(map: &[crate::abi::StatPhys], physical: u64) -> Result<(u64, Option<Region>), &'static str> {
    let limit = physical + LARGE;
    if ram_covers(map, physical, limit) { return Ok((physical | 0x83, None)); }
    if !map.iter().any(|e| ram(e.kind) && e.start < limit && physical < end(e)) { return Ok((physical | 0x83 | UNCACHED, None)); }
    let table = Region::new(PAGE, PAGE)?;
    for page in 0..512 {
        let at = physical + (page * PAGE) as u64;
        let cache = if ram_covers(map, at, at + PAGE as u64) { 0 } else { UNCACHED };
        unsafe { (table.ptr() as *mut u64).add(page).write(at | VALID | WRITE | cache); }
    }
    Ok((table.ptr() as u64 | VALID | WRITE, Some(table)))
}

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
    HUGE.store(core::arch::x86_64::__cpuid(0x80000001).edx & (1 << 26) != 0, Ordering::Relaxed);
    // Firmware RAM may extend above 3 GiB: the Ryzen's kernel arena lies there (213-KRN-0112).
    let root = Region::new(PAGE, PAGE)?;
    let pdpt = Region::new(PAGE, PAGE)?;
    for gigabyte in 0..4 {
        let pd = Region::new(PAGE, PAGE)?;
        for page in 0..512 {
            let physical = (gigabyte * 512 + page) * 0x200000;
            let (entry, table) = identity_region(map, physical as u64)?;
            (pd.ptr() as *mut u64)
                .add(page)
                .write(entry);
            if let Some(table) = table { core::mem::forget(table); }
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
    activate_boot_map(kernel_root());
    protect_from_programs();
    Ok(())
}

// BSP only, before AP startup: Intel SDM 11.12.4/11.11.8 and AMD APM 7.8.7 cache-type serialization.
unsafe fn activate_boot_map(root: usize) {
    let features = core::arch::x86_64::__cpuid(1).edx;
    let mut cr0: usize;
    asm!("mov {}, cr0", out(reg) cr0);
    let (mut mtrr_low, mut mtrr_high) = (0u32, 0u32);
    if features & (1 << 12) != 0 {
        asm!("rdmsr", in("ecx") 0x2FFu32, out("eax") mtrr_low, out("edx") mtrr_high);
    }
    // All tables and inputs exist before caches stop filling; this block has no calls, allocation or stack access.
    asm!(
        "cli",
        "mov cr0, {disabled}",
        "wbinvd",
        "mov rax, cr4",
        "btr rax, 7",
        "mov cr4, rax",
        "mov rax, cr3",
        "mov cr3, rax",
        "test {features:e}, 0x1000",
        "jz 2f",
        "mov ecx, 0x2FF",
        "mov eax, {mtrr_low:e}",
        "and eax, 0xFFFFF7FF",
        "mov edx, {mtrr_high:e}",
        "wrmsr",
        "2:",
        "test {features:e}, 0x10000",
        "jz 3f",
        "mov ecx, 0x277",
        "mov eax, {pat_low}",
        "mov edx, {pat_high}",
        "wrmsr",
        "3:",
        "mov cr3, {root}",
        "test {features:e}, 0x1000",
        "jz 4f",
        "mov ecx, 0x2FF",
        "mov eax, {mtrr_low:e}",
        "mov edx, {mtrr_high:e}",
        "wrmsr",
        "4:",
        "wbinvd",
        "mov cr3, {root}",
        "mov cr0, {enabled}",
        disabled = in(reg) (cr0 | (1 << 30)) & !(1 << 29),
        enabled = in(reg) cr0 & !((1 << 30) | (1 << 29)),
        root = in(reg) root,
        features = in(reg) features,
        mtrr_low = in(reg) mtrr_low,
        mtrr_high = in(reg) mtrr_high,
        pat_low = const PAT as u32,
        pat_high = const (PAT >> 32) as u32,
        out("rax") _, out("rcx") _, out("rdx") _,
        options(nostack),
    );
}

// The boot CPU's SMEP, SMAP and UMIP, on the kernel's page tables (000-KRN-0039).
unsafe fn protect_from_programs() {
    let mut cr4: usize;
    asm!("mov {}, cr4", out(reg) cr4);
    asm!("mov cr4, {}", in(reg) cr4 | PROTECTION.load(Ordering::Acquire) as usize);
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
    // APs retain their per-CPU PAT setup; the BSP changes PAT with its serialized firmware-map transition.
    if !bsp && core::arch::x86_64::__cpuid(1).edx & (1 << 16) != 0 { asm!("wrmsr", in("ecx") 0x277u32, in("eax") PAT as u32, in("edx") (PAT >> 32) as u32); }
    let mut cr0: usize;
    asm!("mov {}, cr0", out(reg) cr0);
    cr0 = (cr0 | (1 << 16) | 2) & !12; // WP, MP; clear EM/TS for FXSAVE.
    asm!("mov cr0, {}", in(reg) cr0);
    let mut cr4: usize;
    asm!("mov {}, cr4", out(reg) cr4);
    cr4 |= (1 << 9) | (1 << 10);
    // Flush inherited global translations as well. No user FSGSBASE or PCID.
    cr4 &= !((1 << 7) | (1 << 16) | (1 << 17) | (1 << 18) | (SMEP | SMAP | UMIP) as usize);
    // XSAVE with every state component programs can use when the CPU has XSAVE and AVX (issue 153, 174-KRN-0037);
    // the BSP decides, the APs follow.
    let features = core::arch::x86_64::__cpuid(1).ecx;
    let avx = features & (1 << 26) != 0 && features & (1 << 28) != 0;
    let xsave = if bsp { avx } else { crate::context::XSAVE.load(Ordering::Acquire) };
    if xsave { cr4 |= 1 << 18; }
    // The kernel never executes (SMEP) nor reads or writes (SMAP) a program's page through the program's address, and
    // programs cannot read descriptor table addresses (UMIP), where the CPU has them (000-KRN-0039). The APs start on the
    // kernel's page tables; the BSP sets them in `protect_from_programs`, once off the firmware's, whose pages may be
    // user pages.
    if bsp { PROTECTION.store(protection_offered(), Ordering::Release); } else { cr4 |= PROTECTION.load(Ordering::Acquire) as usize; }
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

/// CR4's SMEP, SMAP and UMIP bits.
pub const SMEP: u64 = 1 << 20;
pub const SMAP: u64 = 1 << 21;
pub const UMIP: u64 = 1 << 11;
// The protection bits set in CR4 (the boot CPU's choice; the others follow).
static PROTECTION: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
pub fn protection() -> u64 { PROTECTION.load(Ordering::Acquire) }
fn protection_offered() -> u64 {
    if core::arch::x86_64::__cpuid(0).eax < 7 { return 0; }
    let leaf = core::arch::x86_64::__cpuid_count(7, 0);
    (if leaf.ebx & 1 << 7 != 0 { SMEP } else { 0 }) | (if leaf.ebx & 1 << 20 != 0 { SMAP } else { 0 }) | (if leaf.ecx & 1 << 2 != 0 { UMIP } else { 0 })
}
/// The protections in force, for the boot line and the report.
pub fn protection_names() -> &'static str {
    match (protection() & SMEP != 0, protection() & SMAP != 0, protection() & UMIP != 0) {
        (true, true, true) => "SMEP SMAP UMIP", (true, true, false) => "SMEP SMAP", (true, false, true) => "SMEP UMIP", (true, false, false) => "SMEP",
        (false, true, true) => "SMAP UMIP", (false, true, false) => "SMAP", (false, false, true) => "UMIP", (false, false, false) => "NONE",
    }
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

// A report may read a device only through an existing UC identity leaf; this never alters mappings.
pub unsafe fn identity_device_u32(physical: u64) -> bool {
    let pdpt = KERNEL_PDPT.load(Ordering::Acquire) as *const u64;
    !pdpt.is_null() && identity_device_u32_in(pdpt, physical)
}

// The caller supplies the kernel's valid page-table tree, held stable throughout this bounded walk.
pub(crate) unsafe fn identity_device_u32_in(pdpt: *const u64, physical: u64) -> bool {
    const ADDRESS: u64 = 0x000F_FFFF_FFFF_F000;
    if physical & 3 != 0 || physical.checked_add(4).is_none_or(|end| end > IDENTITY_END) { return false; }
    let pdpte = pdpt.add((physical >> 30) as usize).read();
    if pdpte & VALID == 0 || pdpte & (1 << 7) != 0 || pdpte & ADDRESS == 0 { return false; }
    let pde = ((pdpte & ADDRESS) as *const u64).add((physical >> 21 & 511) as usize).read();
    if pde & VALID == 0 { return false; }
    if pde & (1 << 7) != 0 {
        return pde & UNCACHED == UNCACHED && pde & ADDRESS & !(LARGE - 1) == physical & !(LARGE - 1);
    }
    if pde & ADDRESS == 0 { return false; }
    let pte = ((pde & ADDRESS) as *const u64).add((physical >> 12 & 511) as usize).read();
    pte & (VALID | UNCACHED) == VALID | UNCACHED && pte & ADDRESS == physical & !(PAGE as u64 - 1)
}

/// Makes the kernel's identity mapping of the 2 MiB page holding `physical` (below 4 GiB) uncached, for device
/// registers the kernel itself writes (MSI-X tables). The caller checks that the page holds no RAM.
pub unsafe fn uncached(physical: usize) {
    let pdpt = KERNEL_PDPT.load(Ordering::Acquire) as *const u64;
    let pd = (pdpt.add(physical >> 30).read() & !0xFFF) as *mut u64;
    let entry = pd.add(physical >> 21 & 511);
    uncache_identity(entry, physical & !(LARGE as usize - 1));
}

// The caller has checked this whole 2 MiB region for RAM; split mappings need their leaves changed, not the PDE.
pub(crate) unsafe fn uncache_identity(entry: *mut u64, physical: usize) {
    if entry.read() & (1 << 7) != 0 {
        entry.write(entry.read() | UNCACHED);
        #[cfg(not(test))]
        asm!("invlpg [{}]", in(reg) physical, options(nostack));
    } else {
        let table = (entry.read() & !0xFFF) as *mut u64;
        for page in 0..512 {
            let leaf = table.add(page);
            leaf.write(leaf.read() | UNCACHED);
            #[cfg(not(test))]
            asm!("invlpg [{}]", in(reg) physical + page * PAGE, options(nostack));
        }
    }
    #[cfg(test)]
    let _ = physical;
}
