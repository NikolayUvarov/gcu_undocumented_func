use crate::memory::Region;
use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const PAGE: usize = 4096;
pub const USER_IMAGE: usize = 0x80_0000_0000;
pub const USER_STACK: usize = USER_IMAGE + 0x0100_1000;
pub const USER_SCREEN: usize = USER_IMAGE + 0x0200_0000;
pub const USER_INFO: usize = USER_IMAGE + 0x0400_0000;
pub const USER_MAILBOX: usize = USER_INFO + PAGE;
pub const USER_EXIT: usize = USER_IMAGE + 0x0500_0000;
pub const USER_HEAP: usize = USER_IMAGE + 0x0600_0000;
pub const USER_END: usize = USER_IMAGE + 0x0800_0000;
const PRESENT: u64 = 1;
const WRITE: u64 = 2;
const USER: u64 = 4;
const NX: u64 = 1 << 63;
const ADDRESS: u64 = 0x000f_ffff_ffff_f000;
static KERNEL_ROOT: AtomicUsize = AtomicUsize::new(0);
static KERNEL_PDPT: AtomicUsize = AtomicUsize::new(0);

pub fn kernel_root() -> usize {
    KERNEL_ROOT.load(Ordering::Acquire)
}

pub unsafe fn init() -> Result<(), &'static str> {
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
    (root.ptr() as *mut u64).write(pdpt.ptr() as u64 | 3);
    KERNEL_PDPT.store(pdpt.ptr() as usize, Ordering::Release);
    KERNEL_ROOT.store(root.ptr() as usize, Ordering::Release);
    core::mem::forget(pdpt);
    core::mem::forget(root);
    enable_protection();
    activate(kernel_root());
    Ok(())
}

pub unsafe fn enable_protection() {
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
    // Flush inherited global translations as well. No user FSGSBASE, PCID, or
    // XSAVE/AVX state outside our FXSAVE context.
    cr4 &= !((1 << 7) | (1 << 16) | (1 << 17) | (1 << 18));
    asm!("mov cr4, {}", in(reg) cr4);
}

pub unsafe fn activate(root: usize) {
    asm!("mov cr3, {}", in(reg) root);
}

pub struct Space {
    tables: [Option<Region>; 32],
    count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn leaf(space: &Space, address: usize) -> u64 {
        let mut table = space.root();
        let mut entry = 0;
        for shift in [39, 30, 21, 12] {
            entry = unsafe { (table as *const u64).add((address >> shift) & 511).read() };
            assert_eq!(entry & 5, 5);
            table = (entry & ADDRESS) as usize;
        }
        entry
    }
    #[test]
    fn same_virtual_address_maps_private_frames_in_distinct_cr3s() {
        let mut a = Space::new().unwrap();
        let mut b = Space::new().unwrap();
        a.map(USER_IMAGE, 0x100000, PAGE, false, true).unwrap();
        b.map(USER_IMAGE, 0x200000, PAGE, false, true).unwrap();
        assert_ne!(a.root(), b.root());
        assert_eq!(a.readable(USER_IMAGE + 31), Some(0x10001f));
        assert_eq!(b.readable(USER_IMAGE + 31), Some(0x20001f));
        assert!(a.readable(0x100000).is_none());
        assert!(b.readable(0x200000).is_none());
    }
    #[test]
    fn permissions_are_rx_or_rw_nx_and_guard_pages_stay_unmapped() {
        let mut space = Space::new().unwrap();
        space.map(USER_IMAGE, 0x100000, PAGE, false, true).unwrap();
        space.map(USER_STACK, 0x200000, PAGE, true, false).unwrap();
        assert_eq!(leaf(&space, USER_IMAGE) & (WRITE | NX), 0);
        assert_eq!(leaf(&space, USER_STACK) & (WRITE | NX), WRITE | NX);
        assert!(space.readable(USER_STACK - 1).is_none());
        assert!(space.readable(USER_STACK + PAGE).is_none());
        assert!(space.map(USER_EXIT, 0x300000, PAGE, true, true).is_err());
        assert!(space.map(USER_IMAGE, 0x300000, PAGE, true, false).is_err());
    }
    #[test]
    fn validates_full_buffers_including_overflow_holes_and_supervisor_ranges() {
        let mut space = Space::new().unwrap();
        space
            .map(USER_STACK, 0x200000, PAGE * 2, true, false)
            .unwrap();
        assert!(space.validate_read(USER_STACK + PAGE - 2, 4));
        assert!(!space.validate_read(USER_STACK + PAGE * 2 - 2, 4));
        assert!(!space.validate_read(usize::MAX - 1, 4));
        assert!(!space.validate_read(0x100000, 4));
        assert!(!space.validate_read(0x8000_0000_0000, 1));
    }
    #[test]
    fn overlap_is_atomic_and_unmap_reclaims_only_empty_tables() {
        let mut space = Space::new().unwrap();
        space
            .map(USER_HEAP + PAGE, 0x100000, PAGE, true, false)
            .unwrap();
        let count = space.count;
        assert!(space
            .map(USER_HEAP, 0x200000, PAGE * 2, true, false)
            .is_err());
        assert_eq!(space.count, count);
        assert!(space.readable(USER_HEAP).is_none());
        assert_eq!(space.readable(USER_HEAP + PAGE), Some(0x100000));
        space
            .map(USER_HEAP + PAGE * 3, 0x300000, PAGE, true, false)
            .unwrap();
        space.unmap(USER_HEAP + PAGE, PAGE);
        assert_eq!(space.count, count);
        assert_eq!(space.readable(USER_HEAP + PAGE * 3), Some(0x300000));
        space.unmap(USER_HEAP + PAGE * 3, PAGE);
        assert_eq!(space.count, 1); // shared kernel tables were never owned
        assert!(space.readable(USER_HEAP + PAGE * 3).is_none());
    }
}

impl Space {
    pub fn new() -> Result<Self, &'static str> {
        let mut space = Self {
            tables: core::array::from_fn(|_| None),
            count: 0,
        };
        let root = space.table()?;
        unsafe {
            (root as *mut u64).write(KERNEL_PDPT.load(Ordering::Acquire) as u64 | 3);
        }
        Ok(space)
    }
    fn table(&mut self) -> Result<usize, &'static str> {
        if self.count == self.tables.len() {
            return Err("PAGE TABLE LIMIT");
        }
        let table = Region::new(PAGE, PAGE)?;
        let pointer = table.ptr() as usize;
        self.tables[self.count] = Some(table);
        self.count += 1;
        Ok(pointer)
    }
    pub fn root(&self) -> usize {
        self.tables[0].as_ref().unwrap().ptr() as usize
    }

    pub fn map(
        &mut self,
        virtual_start: usize,
        physical: usize,
        size: usize,
        writable: bool,
        executable: bool,
    ) -> Result<(), &'static str> {
        if virtual_start < USER_IMAGE
            || virtual_start >= USER_END
            || virtual_start % PAGE != 0
            || physical % PAGE != 0
            || (writable && executable)
            || virtual_start
                .checked_add(size)
                .is_none_or(|end| end > USER_END)
            || physical.checked_add(size).is_none()
        {
            return Err("INVALID USER MAPPING");
        }
        // Preflight overlap before writing anything, so rollback never removes
        // a pre-existing mapping. The scheduler lock excludes concurrent edits.
        for offset in (0..size).step_by(PAGE) {
            if self.readable(virtual_start + offset).is_some() {
                return Err("OVERLAPPING USER PAGES");
            }
        }
        let result = self.map_pages(virtual_start, physical, size, writable, executable);
        if result.is_err() {
            self.unmap(virtual_start, size);
        } else {
            self.flush();
        }
        result
    }

    fn map_pages(
        &mut self,
        virtual_start: usize,
        physical: usize,
        size: usize,
        writable: bool,
        executable: bool,
    ) -> Result<(), &'static str> {
        for offset in (0..size).step_by(PAGE) {
            let address = virtual_start + offset;
            let mut table = self.root();
            for shift in [39, 30, 21] {
                let entry = unsafe { (table as *mut u64).add((address >> shift) & 511) };
                unsafe {
                    if entry.read() & PRESENT == 0 {
                        entry.write(self.table()? as u64 | 7);
                    }
                    table = (entry.read() & ADDRESS) as usize;
                }
            }
            let entry = unsafe { (table as *mut u64).add((address >> 12) & 511) };
            unsafe {
                if entry.read() & PRESENT != 0 {
                    return Err("OVERLAPPING USER PAGES");
                }
                entry.write(
                    (physical + offset) as u64
                        | PRESENT
                        | USER
                        | if writable { WRITE } else { 0 }
                        | if executable { 0 } else { NX },
                );
            }
        }
        Ok(())
    }

    // Called with local IRQs off and the scheduler lock held. A Space is either
    // inactive or belongs to the sole, pinned task currently in this syscall.
    // Detach empty tables, flush translations, THEN release their physical RAM.
    pub fn unmap(&mut self, start: usize, size: usize) {
        assert!(start >= USER_IMAGE && start % PAGE == 0);
        assert!(start.checked_add(size).is_some_and(|end| end <= USER_END));
        let mut retired: [Option<Region>; 32] = core::array::from_fn(|_| None);
        let mut retired_count = 0;
        for offset in (0..size).step_by(PAGE) {
            let address = start + offset;
            let mut table = self.root();
            let mut parents = [core::ptr::null_mut::<u64>(); 3];
            let mut children = [0usize; 3];
            let mut depth = 0;
            for shift in [39, 30, 21] {
                let entry = unsafe { (table as *mut u64).add((address >> shift) & 511) };
                let value = unsafe { entry.read() };
                if value & PRESENT == 0 {
                    break;
                }
                table = (value & ADDRESS) as usize;
                parents[depth] = entry;
                children[depth] = table;
                depth += 1;
            }
            if depth == 3 {
                unsafe {
                    (table as *mut u64).add((address >> 12) & 511).write(0);
                }
            }
            for level in (0..depth).rev() {
                let child = children[level];
                let empty =
                    (0..512).all(|i| unsafe { (child as *const u64).add(i).read() & PRESENT == 0 });
                if !empty {
                    break;
                }
                unsafe {
                    parents[level].write(0);
                }
                let index = (1..self.count)
                    .find(|&i| self.tables[i].as_ref().unwrap().ptr() as usize == child)
                    .unwrap();
                retired[retired_count] = self.tables[index].take();
                retired_count += 1;
                self.count -= 1;
                self.tables.swap(index, self.count);
            }
        }
        self.flush();
        // retired drops here, after invalidating paging-structure caches too.
    }

    fn flush(&self) {
        #[cfg(not(test))]
        unsafe {
            let current: usize;
            asm!("mov {}, cr3", out(reg) current);
            if current == self.root() {
                activate(current);
            }
        }
    }

    // Translate through this task's locked page tables, never by trusting a
    // user-supplied kernel pointer. Null, supervisor, noncanonical and holes fail.
    pub fn readable(&self, address: usize) -> Option<usize> {
        if !(USER_IMAGE..USER_END).contains(&address) {
            return None;
        }
        let mut table = self.root();
        for shift in [39, 30, 21, 12] {
            let entry = unsafe { (table as *const u64).add((address >> shift) & 511).read() };
            if entry & (PRESENT | USER) != (PRESENT | USER) {
                return None;
            }
            table = (entry & ADDRESS) as usize;
        }
        Some(table + (address & 4095))
    }

    pub fn validate_read(&self, start: usize, size: usize) -> bool {
        if size == 0 {
            return true;
        }
        let Some(end) = start.checked_add(size - 1) else {
            return false;
        };
        let mut address = start;
        loop {
            if self.readable(address).is_none() {
                return false;
            }
            if address / PAGE == end / PAGE {
                return true;
            }
            address = (address & !4095) + PAGE;
        }
    }
}
