// Task address spaces (four-level page tables; 4 KiB pages, and 2 MiB and 1 GiB pages for memory, 262-KRN-0161): the walk
// is the same on every architecture; the descriptor format and the switch are in arch/*/mmu.rs (issue 201).
use crate::memory::{Frames, Region};
use crate::mmu;
pub use crate::mmu::{activate, init, kernel_root, uncached};

pub const PAGE: usize = 4096;
/// How a user page is cached: memory (write-back), device registers (uncached), a framebuffer (write-combining).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Caching { Memory, Device, WriteCombining }
pub const USER_IMAGE: usize = mmu::USER_IMAGE; // the start of a task's window: the architecture's
pub const USER_STACK: usize = USER_IMAGE + 0x0100_1000;
pub const USER_SCREEN: usize = USER_IMAGE + 0x0200_0000;
pub const USER_INFO: usize = USER_IMAGE + 0x0400_0000;
pub const USER_MAILBOX: usize = USER_INFO + PAGE;
pub const USER_EXIT: usize = USER_IMAGE + 0x0500_0000;
pub const USER_HEAP: usize = USER_IMAGE + 0x0600_0000;
pub const USER_END: usize = USER_IMAGE + (1 << 39); // the task's whole top-level entry: its heap window holds 511 GiB (262-KRN-0160)
const TABLES: usize = 33_792; // 64 GiB mapped in 4 KiB pages and the levels above (each table a page of the frame pool, charged: 171-KRN-0032)
const ADDRESS: u64 = 0x000f_ffff_ffff_f000;

pub struct Space {
    tables: alloc::vec::Vec<Region, Frames>,
}

// How a range is filled: 4 KiB pages, the largest pages that fit (262-KRN-0161), or one frame at every page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fill { Small, Large, Repeated }

#[cfg(test)]
mod tests {
    use super::*;
    fn leaf(space: &Space, address: usize) -> u64 {
        let (entry, _) = space.find(address).unwrap();
        assert!(mmu::user_readable(entry));
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
    #[test]
    fn large_pages_translate_unmap_whole_and_reject_an_overlap() {
        // 262-KRN-0161: one 1 GiB page, one 2 MiB page and one 4 KiB page, in four tables (root, PDPT, PD, PT).
        const LARGE: usize = 2 << 20;
        const HUGE: usize = 1 << 30;
        let (base, frame, size) = (USER_IMAGE + 4 * HUGE, 0x40_0000_0000, HUGE + LARGE + PAGE);
        let mut space = Space::new().unwrap();
        space.map(base, frame, size, true, false).unwrap();
        assert_eq!(space.table_count(), 4);
        for offset in [12_345, HUGE - 1, HUGE + 5, HUGE + LARGE + 7] {
            assert_eq!(space.readable(base + offset), Some(frame + offset));
            assert_eq!(space.writable(base + offset), Some(frame + offset));
        }
        assert_eq!(mmu::attributes(leaf(&space, base)), (true, false, false));
        assert_eq!(mmu::attributes(leaf(&space, base + HUGE)), (true, false, false));
        assert!(space.readable(base + size).is_none() && space.readable(base - 1).is_none());
        assert!(space.validate_read(base + HUGE - 2, LARGE + 4) && !space.validate_read(base, size + 1));
        let mut regions = alloc::vec::Vec::new();
        space.regions(|start, size, w, x, d| regions.push((start, size, w, x, d)));
        assert_eq!(regions, [(base, size, true, false, false)]);
        // An overlap with either large page is refused and leaves nothing behind.
        for at in [base + 100 * PAGE, base + HUGE + LARGE - PAGE, base - PAGE] {
            assert!(space.map(at, 0x1000_0000, 2 * PAGE, true, false).is_err());
            assert_eq!(space.table_count(), 4);
        }
        // Unmapped whole: every table goes.
        space.unmap(base, size);
        assert_eq!(space.table_count(), 1);
        assert!(space.readable(base).is_none() && space.readable(base + HUGE).is_none());
    }
    #[test]
    fn large_pages_carry_bytes_and_other_ranges_keep_small_ones() {
        const LARGE: usize = 2 << 20;
        let mut memory = Region::new(2 * LARGE, LARGE).unwrap();
        let physical = memory.ptr() as usize;
        // Read and written through: two 2 MiB pages in one PD.
        let mut space = Space::new().unwrap();
        space.map(USER_HEAP, physical, 2 * LARGE, true, false).unwrap();
        assert_eq!(space.table_count(), 3);
        unsafe { (space.writable(USER_HEAP + LARGE + 3).unwrap() as *mut u8).write(0xAB); }
        assert_eq!(memory.bytes_mut()[LARGE + 3], 0xAB);
        assert_eq!(unsafe { (space.readable(USER_HEAP + LARGE + 3).unwrap() as *const u8).read() }, 0xAB);
        space.unmap(USER_HEAP, 2 * LARGE);
        // 4 KiB pages when asked, for code, and where the two addresses are not aligned alike: a PT for each 2 MiB.
        space.map_small(USER_HEAP, physical, 2 * LARGE, true).unwrap();
        assert_eq!(space.table_count(), 5);
        space.unmap(USER_HEAP, 2 * LARGE);
        space.map(USER_HEAP, physical, LARGE, false, true).unwrap();
        assert_eq!(space.table_count(), 4);
        space.unmap(USER_HEAP, LARGE);
        space.map(USER_HEAP, physical + PAGE, LARGE, true, false).unwrap();
        assert_eq!(space.table_count(), 4);
        space.unmap(USER_HEAP, LARGE);
        assert_eq!(space.table_count(), 1);
        // An empty table kept by unmap_leaves takes 4 KiB pages instead of a large one.
        space.map(USER_HEAP, physical, PAGE, true, false).unwrap();
        space.unmap_leaves(USER_HEAP, PAGE);
        space.map(USER_HEAP, physical, LARGE, true, false).unwrap();
        assert_eq!(space.table_count(), 4);
        assert_eq!(space.readable(USER_HEAP + LARGE - 1), Some(physical + LARGE - 1));
    }
}

impl Space {
    pub fn new() -> Result<Self, &'static str> {
        let mut space = Self {
            tables: alloc::vec::Vec::new_in(Frames),
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
        let table = Region::task(PAGE, PAGE)?;
        let pointer = table.ptr() as usize;
        self.tables.try_reserve(1).map_err(|_| "OUT OF MEMORY")?;
        self.tables.push(table);
        Ok(pointer)
    }
    /// Page tables owned by this space (each one page of the frame pool, charged to the task's payers).
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
        self.map_checked(virtual_start, physical, size, writable, executable, Caching::Memory, Fill::Large)
    }

    // Memory in 4 KiB pages only, as a program may ask (ALLOC_SMALL_PAGES, 262-KRN-0161).
    pub fn map_small(&mut self, virtual_start: usize, physical: usize, size: usize, writable: bool) -> Result<(), &'static str> {
        self.map_checked(virtual_start, physical, size, writable, false, Caching::Memory, Fill::Small)
    }

    // MMIO registers are mapped uncached (RW+NX).
    pub fn map_device(&mut self, virtual_start: usize, physical: usize, size: usize) -> Result<(), &'static str> {
        self.map_checked(virtual_start, physical, size, true, false, Caching::Device, Fill::Small)
    }

    // A framebuffer: write-combining where the architecture has it (000-KRN-0081).
    pub fn map_framebuffer(&mut self, virtual_start: usize, physical: usize, size: usize, writable: bool) -> Result<(), &'static str> {
        self.map_checked(virtual_start, physical, size, writable, false, Caching::WriteCombining, Fill::Small)
    }

    // One frame at every page of a range: a revoked lease's place in its borrower (000-KRN-0152).
    pub fn map_repeated(&mut self, virtual_start: usize, physical: usize, size: usize, writable: bool) -> Result<(), &'static str> {
        self.map_checked(virtual_start, physical, size, writable, false, Caching::Memory, Fill::Repeated)
    }

    #[allow(clippy::too_many_arguments)]
    fn map_checked(
        &mut self,
        virtual_start: usize,
        physical: usize,
        size: usize,
        writable: bool,
        executable: bool,
        caching: Caching,
        fill: Fill,
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
        let mut address = virtual_start;
        while address < virtual_start + size {
            match self.find(address) { Ok(_) => return Err("OVERLAPPING USER PAGES"), Err(absent) => address += absent }
        }
        let result = self.map_pages(virtual_start, physical, size, writable, executable, caching, fill);
        if result.is_err() {
            self.unmap(virtual_start, size);
        } else {
            self.flush();
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn map_pages(
        &mut self,
        virtual_start: usize,
        physical: usize,
        size: usize,
        writable: bool,
        executable: bool,
        caching: Caching,
        fill: Fill,
    ) -> Result<(), &'static str> {
        // Data in memory goes in the largest pages that lie whole in the range and on their own boundary in both addresses;
        // code, devices, framebuffers and a revoked lease's place stay in 4 KiB pages (262-KRN-0161).
        let large = fill == Fill::Large && caching == Caching::Memory && !executable;
        let mut offset = 0;
        while offset < size {
            let address = virtual_start + offset;
            let frame = if fill == Fill::Repeated { physical } else { physical + offset };
            let mut table = self.root();
            for shift in [39, 30, 21, 12] {
                let span = 1usize << shift;
                let entry = unsafe { (table as *mut u64).add((address >> shift) & 511) };
                let value = unsafe { entry.read() };
                let fits = shift == 12
                    || (large && shift <= 30 && (shift == 21 || mmu::huge_pages()) && (address | frame) & (span - 1) == 0 && size - offset >= span);
                // An empty table already there (kept by unmap_leaves) takes smaller pages instead.
                if fits && value & mmu::VALID == 0 {
                    let leaf = mmu::leaf(frame as u64, writable, executable, caching == Caching::Device);
                    let leaf = if caching == Caching::WriteCombining { mmu::write_combining(leaf) } else { leaf };
                    unsafe { entry.write(if shift == 12 { leaf } else { mmu::block(leaf) }); }
                    offset += span;
                    break;
                }
                if shift == 12 || (shift <= 30 && value & mmu::VALID != 0 && mmu::large(value)) {
                    return Err("OVERLAPPING USER PAGES");
                }
                if value & mmu::VALID == 0 {
                    unsafe { entry.write(self.table()? as u64 | mmu::TABLE); }
                }
                table = (unsafe { entry.read() } & ADDRESS) as usize;
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
        let mut address = start;
        while address < start + size {
            let mut table = self.root();
            let mut parents = [core::ptr::null_mut::<u64>(); 3];
            let mut children = [0usize; 3];
            let mut depth = 0;
            let mut step = PAGE;
            for shift in [39, 30, 21, 12] {
                let span = 1usize << shift;
                let entry = unsafe { (table as *mut u64).add((address >> shift) & 511) };
                let value = unsafe { entry.read() };
                step = span - (address & (span - 1));
                if value & mmu::VALID == 0 {
                    break;
                }
                // A page, or a large page whole: blocks are freed whole, so one never lies only partly in the range.
                if shift == 12 || (shift <= 30 && mmu::large(value)) {
                    unsafe { entry.write(0); }
                    break;
                }
                table = (value & ADDRESS) as usize;
                parents[depth] = entry;
                children[depth] = table;
                depth += 1;
            }
            address += step;
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
        // The list keeps the capacity growth would give its length, so its memory depends only on the tables held.
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
            let (leaf, span) = match self.find(address) {
                Ok((entry, size)) => (Some(mmu::attributes(entry)), size - (address & (size - 1))),
                Err(absent) => (None, absent),
            };
            match (run, leaf) {
                (Some((start, size, flags)), Some(f)) if start + size == address && flags == f => run = Some((start, size + span, flags)),
                (_, Some(f)) => { if let Some((s, z, (w, x, d))) = run { emit(s, z, w, x, d); } run = Some((address, span, f)); }
                (_, None) => { if let Some((s, z, (w, x, d))) = run.take() { emit(s, z, w, x, d); } }
            }
            address = address.saturating_add(span);
        }
        if let Some((s, z, (w, x, d))) = run { emit(s, z, w, x, d); }
    }

    fn flush(&self) { mmu::flush(self.root()); }
    // Drops this CPU's cached translations of the space, before an access that faulted is tried again.
    pub fn forget_cached(&self) { self.flush(); }

    // Translate through this task's locked page tables, never by trusting a
    // user-supplied kernel pointer. Null, supervisor, noncanonical and holes fail.
    pub fn readable(&self, address: usize) -> Option<usize> {
        let (entry, size) = self.user_page(address)?;
        mmu::user_readable(entry).then(|| ((entry & ADDRESS) as usize & !(size - 1)) | (address & (size - 1)))
    }

    // Like readable, but only for writable pages (buffers filled by the kernel).
    pub fn writable(&self, address: usize) -> Option<usize> {
        let (entry, size) = self.user_page(address)?;
        mmu::user_writable(entry).then(|| ((entry & ADDRESS) as usize & !(size - 1)) | (address & (size - 1)))
    }

    // The page of the task's window that holds `address`: its entry in a 4 KiB page's form and its size.
    fn user_page(&self, address: usize) -> Option<(u64, usize)> {
        if !(USER_IMAGE..USER_END).contains(&address) {
            return None;
        }
        self.find(address).ok()
    }

    // The entry that maps `address` (a large page's in a 4 KiB page's form) and its page's size; where none does, the
    // bytes from `address` to the end of the entry that stopped the walk. Each table on the way must let the task through.
    fn find(&self, address: usize) -> Result<(u64, usize), usize> {
        let mut table = self.root();
        for shift in [39, 30, 21, 12] {
            let span = 1usize << shift;
            let entry = unsafe { (table as *const u64).add((address >> shift) & 511).read() };
            if entry & mmu::VALID == 0 {
                return Err(span - (address & (span - 1)));
            }
            if shift == 12 {
                return Ok((entry, span));
            }
            if shift <= 30 && mmu::large(entry) {
                return Ok((mmu::page(entry), span));
            }
            if !mmu::user_table(entry) {
                return Err(span - (address & (span - 1)));
            }
            table = (entry & ADDRESS) as usize;
        }
        unreachable!()
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
            let Some((_, size)) = self.user_page(address).filter(|&(entry, _)| mmu::user_readable(entry)) else {
                return false;
            };
            // The next page, past this one's end (a large page is checked once).
            let next = (address | (size - 1)) + 1;
            if next > end {
                return true;
            }
            address = next;
        }
    }
}

