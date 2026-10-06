// Task address spaces (four-level page tables, 4 KiB pages): the walk is the same on every architecture; the
// descriptor format and the switch are in arch/*/mmu.rs (issue 201).
use crate::memory::Region;
use crate::mmu;
pub use crate::mmu::{activate, init, kernel_root, uncached};

pub const PAGE: usize = 4096;
pub const USER_IMAGE: usize = mmu::USER_IMAGE; // the start of a task's window: the architecture's
pub const USER_STACK: usize = USER_IMAGE + 0x0100_1000;
pub const USER_SCREEN: usize = USER_IMAGE + 0x0200_0000;
pub const USER_INFO: usize = USER_IMAGE + 0x0400_0000;
pub const USER_MAILBOX: usize = USER_INFO + PAGE;
pub const USER_EXIT: usize = USER_IMAGE + 0x0500_0000;
pub const USER_HEAP: usize = USER_IMAGE + 0x0600_0000;
pub const USER_END: usize = USER_IMAGE + 0x4000_0000; // 928 MiB heap window: private quota + frame/IPC mappings (issue 150)
const TABLES: usize = 640; // the whole heap window mapped (each table one page of the frame pool)
const ADDRESS: u64 = 0x000f_ffff_ffff_f000;

pub struct Space {
    tables: alloc::vec::Vec<Region>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn leaf(space: &Space, address: usize) -> u64 {
        let mut table = space.root();
        let mut entry = 0;
        for shift in [39, 30, 21, 12] {
            entry = unsafe { (table as *const u64).add((address >> shift) & 511).read() };
            assert!(if shift == 12 { mmu::user_readable(entry) } else { mmu::user_table(entry) });
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
        assert_eq!(mmu::attributes(leaf(&space, USER_IMAGE)), (false, true, false));
        assert_eq!(mmu::attributes(leaf(&space, USER_STACK)), (true, false, false));
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
        let count = space.table_count();
        assert!(space
            .map(USER_HEAP, 0x200000, PAGE * 2, true, false)
            .is_err());
        assert_eq!(space.table_count(), count);
        assert!(space.readable(USER_HEAP).is_none());
        assert_eq!(space.readable(USER_HEAP + PAGE), Some(0x100000));
        space
            .map(USER_HEAP + PAGE * 3, 0x300000, PAGE, true, false)
            .unwrap();
        space.unmap(USER_HEAP + PAGE, PAGE);
        assert_eq!(space.table_count(), count);
        assert_eq!(space.readable(USER_HEAP + PAGE * 3), Some(0x300000));
        space.unmap(USER_HEAP + PAGE * 3, PAGE);
        assert_eq!(space.table_count(), 1); // shared kernel tables were never owned
        assert!(space.readable(USER_HEAP + PAGE * 3).is_none());
    }
}

impl Space {
    pub fn new() -> Result<Self, &'static str> {
        let mut space = Self {
            tables: alloc::vec::Vec::new(),
        };
        let root = space.table()?;
        unsafe {
            for index in 0..mmu::KERNEL_ENTRIES { (root as *mut u64).add(index).write(mmu::kernel_entry(index)); }
        }
        Ok(space)
    }
    fn table(&mut self) -> Result<usize, &'static str> {
        if self.tables.len() == TABLES {
            return Err("PAGE TABLE LIMIT");
        }
        // From the frame pool, as task memory (issue 171): at most one table per 2 MiB mapped, besides the first few.
        let table = Region::task(PAGE, PAGE)?;
        let pointer = table.ptr() as usize;
        self.tables.try_reserve(1).map_err(|_| "OUT OF MEMORY")?;
        self.tables.push(table);
        Ok(pointer)
    }
    /// Page tables owned by this space (each one page of the frame pool).
    pub fn table_count(&self) -> usize { self.tables.len() }
    pub fn root(&self) -> usize {
        self.tables[0].ptr() as usize
    }

    pub fn map(
        &mut self,
        virtual_start: usize,
        physical: usize,
        size: usize,
        writable: bool,
        executable: bool,
    ) -> Result<(), &'static str> {
        self.map_with(virtual_start, physical, size, writable, executable, false)
    }

    // MMIO registers are mapped uncached (RW+NX).
    pub fn map_device(&mut self, virtual_start: usize, physical: usize, size: usize) -> Result<(), &'static str> {
        self.map_with(virtual_start, physical, size, true, false, true)
    }

    fn map_with(
        &mut self,
        virtual_start: usize,
        physical: usize,
        size: usize,
        writable: bool,
        executable: bool,
        device: bool,
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
        let result = self.map_pages(virtual_start, physical, size, writable, executable, device);
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
        device: bool,
    ) -> Result<(), &'static str> {
        for offset in (0..size).step_by(PAGE) {
            let address = virtual_start + offset;
            let mut table = self.root();
            for shift in [39, 30, 21] {
                let entry = unsafe { (table as *mut u64).add((address >> shift) & 511) };
                unsafe {
                    if entry.read() & mmu::VALID == 0 {
                        entry.write(self.table()? as u64 | mmu::TABLE);
                    }
                    table = (entry.read() & ADDRESS) as usize;
                }
            }
            let entry = unsafe { (table as *mut u64).add((address >> 12) & 511) };
            unsafe {
                if entry.read() & mmu::VALID != 0 {
                    return Err("OVERLAPPING USER PAGES");
                }
                entry.write(mmu::leaf((physical + offset) as u64, writable, executable, device));
            }
        }
        Ok(())
    }

    // Called with local IRQs off and the scheduler lock held. A Space is either
    // inactive or belongs to the sole, pinned task currently in this syscall.
    // Detach empty tables, flush translations, THEN release their physical RAM.
    pub fn unmap(&mut self, start: usize, size: usize) { self.unmap_with(start, size, true) }
    // Clears only the page entries and keeps the tables, for a space another CPU may still be using.
    pub fn unmap_leaves(&mut self, start: usize, size: usize) { self.unmap_with(start, size, false) }

    fn unmap_with(&mut self, start: usize, size: usize, reclaim: bool) {
        assert!(start >= USER_IMAGE && start % PAGE == 0);
        assert!(start.checked_add(size).is_some_and(|end| end <= USER_END));
        let mut retired: alloc::vec::Vec<Region> = alloc::vec::Vec::new();
        for offset in (0..size).step_by(PAGE) {
            let address = start + offset;
            let mut table = self.root();
            let mut parents = [core::ptr::null_mut::<u64>(); 3];
            let mut children = [0usize; 3];
            let mut depth = 0;
            for shift in [39, 30, 21] {
                let entry = unsafe { (table as *mut u64).add((address >> shift) & 511) };
                let value = unsafe { entry.read() };
                if value & mmu::VALID == 0 {
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
                if !reclaim { break; }
                let child = children[level];
                let empty =
                    (0..512).all(|i| unsafe { (child as *const u64).add(i).read() & mmu::VALID == 0 });
                if !empty {
                    break;
                }
                unsafe {
                    parents[level].write(0);
                }
                let index = (1..self.tables.len())
                    .find(|&i| self.tables[i].ptr() as usize == child)
                    .unwrap();
                retired.push(self.tables.swap_remove(index));
            }
        }
        self.flush();
        // The list keeps the capacity growth would give its length, so arena use depends only on the tables held.
        let capacity = self.tables.len().next_power_of_two().max(4);
        if self.tables.capacity() > capacity { self.tables.shrink_to(capacity); }
        // retired drops here, after invalidating paging-structure caches too.
    }

    // Mapped user ranges with equal attributes, coalesced: (start, size, writable, executable, device). Absent tables are
    // skipped whole, so the walk is bounded by the number of tables (STAT VMAP).
    pub fn regions(&self, mut emit: impl FnMut(usize, usize, bool, bool, bool)) {
        let mut run: Option<(usize, usize, (bool, bool, bool))> = None;
        let mut address = USER_IMAGE;
        while address < USER_END {
            let mut table = self.root();
            let mut leaf = None; let mut span = PAGE;
            for shift in [39, 30, 21, 12] {
                let value = unsafe { (table as *const u64).add((address >> shift) & 511).read() };
                if value & mmu::VALID == 0 { span = (1usize << shift) - (address & ((1usize << shift) - 1)); break; }
                if shift == 12 { leaf = Some(mmu::attributes(value)); } else { table = (value & ADDRESS) as usize; }
            }
            match (run, leaf) {
                (Some((start, size, flags)), Some(f)) if start + size == address && flags == f => run = Some((start, size + PAGE, flags)),
                (_, Some(f)) => { if let Some((s, z, (w, x, d))) = run { emit(s, z, w, x, d); } run = Some((address, PAGE, f)); }
                (_, None) => { if let Some((s, z, (w, x, d))) = run.take() { emit(s, z, w, x, d); } }
            }
            address = address.saturating_add(span);
        }
        if let Some((s, z, (w, x, d))) = run { emit(s, z, w, x, d); }
    }

    fn flush(&self) { mmu::flush(self.root()); }

    // Translate through this task's locked page tables, never by trusting a
    // user-supplied kernel pointer. Null, supervisor, noncanonical and holes fail.
    pub fn readable(&self, address: usize) -> Option<usize> {
        if !(USER_IMAGE..USER_END).contains(&address) {
            return None;
        }
        let mut table = self.root();
        for shift in [39, 30, 21, 12] {
            let entry = unsafe { (table as *const u64).add((address >> shift) & 511).read() };
            if !(if shift == 12 { mmu::user_readable(entry) } else { mmu::user_table(entry) }) {
                return None;
            }
            table = (entry & ADDRESS) as usize;
        }
        Some(table + (address & 4095))
    }

    // Like readable, but only for writable pages (buffers filled by the kernel).
    pub fn writable(&self, address: usize) -> Option<usize> {
        if !(USER_IMAGE..USER_END).contains(&address) {
            return None;
        }
        let mut table = self.root();
        for shift in [39, 30, 21, 12] {
            let entry = unsafe { (table as *const u64).add((address >> shift) & 511).read() };
            if !(if shift == 12 { mmu::user_writable(entry) } else { mmu::user_table(entry) }) {
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

