use crate::abi::{HEAP_MAX_BLOCKS, SHARED_MAX_BYTES};
use crate::memory::Region;
use crate::paging::{Space, PAGE, USER_END, USER_HEAP};

struct Block {
    address: usize,
    physical: usize,
    memory: Option<Region>, // None: foreign physical memory (shared mapping)
    size: usize,
    node: u64, // derivation node of the capability a foreign mapping was made from
    writable: bool,
    device: bool,
    sink: bool, // a revoked lease's place: one zeroed page of the task's own at each of its pages (000-KRN-0152)
}

impl Block {
    // The task's own bytes in this block: one page for a revoked lease's place.
    fn owned(&self) -> usize { if self.sink { PAGE } else { self.size } }
}

pub struct Heap {
    blocks: [Option<Block>; HEAP_MAX_BLOCKS],
    bytes: usize,
    pub retained: usize, // freed or detached blocks others still hold, charged until released
    shared: usize,
    limit: usize, // the task's memory quota: private and retained bytes together
}

// Large blocks are aligned for large pages where the pool allows (262-KRN-0160, 0161): from 2 MiB on to 2 MiB, from
// 1 GiB on to 1 GiB, in both their addresses; the space then maps them in 2 MiB and 1 GiB pages, unless the task asks for
// 4 KiB pages (`small`).
pub const LARGE: usize = 2 * 1024 * 1024;
pub const HUGE: usize = 1 << 30;
fn alignment(size: usize) -> usize { if size >= HUGE { HUGE } else if size >= LARGE { LARGE } else { PAGE } }

// The pieces a large block is zeroed in, its CPU taking its interrupts between them (262-KRN-0165).
pub fn pieces(len: usize) -> impl Iterator<Item = core::ops::Range<usize>> { (0..len).step_by(LARGE).map(move |at| at..(at + LARGE).min(len)) }

// Frames for a block of `size`, as aligned as the pool allows; zeroed or not (the caller then zeroes them).
fn frames(size: usize, zeroed: bool) -> Option<Region> {
    let mut align = alignment(size);
    loop {
        let memory = if zeroed { Region::task(size, align) } else { Region::task_unzeroed(size, align) };
        if let Ok(memory) = memory { return Some(memory); }
        if align == PAGE { return None; }
        align = if align == HUGE { LARGE } else { PAGE };
    }
}

impl Heap {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::with_limit(crate::abi::HEAP_MAX_BYTES)
    }
    pub fn with_limit(limit: usize) -> Self {
        Self { blocks: core::array::from_fn(|_| None), bytes: 0, retained: 0, shared: 0, limit }
    }

    fn find_hole(&self, size: usize) -> Option<usize> {
        let align = alignment(size);
        let mut address = (USER_HEAP + PAGE).next_multiple_of(align);
        loop {
            let end = address.checked_add(size + PAGE)?;
            if end > USER_END { return None; }
            if let Some(block) = self.blocks.iter().flatten().find(|b| address < b.address + b.size + PAGE && end > b.address) {
                address = (block.address + block.size + PAGE).next_multiple_of(align);
            } else { break; }
        }
        Some(address)
    }

    pub fn allocate(&mut self, space: &mut Space, requested: usize, small: bool) -> Option<usize> {
        if requested == 0 { return None; }
        let size = requested.checked_add(PAGE - 1)? & !(PAGE - 1);
        if size > self.limit.saturating_sub(self.bytes + self.retained) { return None; }
        let slot = self.blocks.iter().position(Option::is_none)?;
        let address = self.find_hole(size)?;
        let memory = frames(size, true)?;
        Self::map_own(space, address, &memory, small)?;
        self.blocks[slot] = Some(Block { address, physical: memory.ptr() as usize, memory: Some(memory), size, node: 0, writable: true, device: false, sink: false });
        self.bytes += size;
        Some(address)
    }

    // A large block in two steps (262-KRN-0160): `reserve` checks the quota and a free slot and counts the block's bytes;
    // the caller takes its frames (`frames_unzeroed`) and zeroes them outside the scheduler's lock; then `place` maps
    // them, or `unreserve` gives the bytes back.
    pub fn reserve(&mut self, requested: usize) -> Option<usize> {
        let size = requested.checked_add(PAGE - 1)? & !(PAGE - 1);
        if size == 0 || size > self.limit.saturating_sub(self.bytes + self.retained) || self.blocks.iter().all(Option::is_some) { return None; }
        self.bytes += size;
        Some(size)
    }
    pub fn frames_unzeroed(size: usize) -> Option<Region> { frames(size, false) }
    pub fn unreserve(&mut self, size: usize) { self.bytes -= size; }
    pub fn place(&mut self, space: &mut Space, memory: Region, small: bool) -> Option<usize> {
        let size = memory.len();
        let slot = self.blocks.iter().position(Option::is_none)?;
        let address = self.find_hole(size)?;
        Self::map_own(space, address, &memory, small)?;
        self.blocks[slot] = Some(Block { address, physical: memory.ptr() as usize, memory: Some(memory), size, node: 0, writable: true, device: false, sink: false });
        Some(address)
    }

    fn map_own(space: &mut Space, address: usize, memory: &Region, small: bool) -> Option<()> {
        let (physical, size) = (memory.ptr() as usize, memory.len());
        if small { space.map_small(address, physical, size, true) } else { space.map(address, physical, size, true, false) }.ok()
    }

    // Mappings of foreign memory count against their own limit and don't eat into the private heap: the larger of
    // SHARED_MAX_BYTES and the task's memory quota, so a program can map an object as large as it may hold (262-KRN-0162).
    fn shared_limit(&self) -> usize { SHARED_MAX_BYTES.max(self.limit) }

    pub fn map_shared(&mut self, space: &mut Space, physical: usize, requested: usize, caching: crate::paging::Caching, writable: bool, node: u64) -> Option<usize> {
        let device = caching == crate::paging::Caching::Device;
        if requested == 0 || physical % PAGE != 0 { return None; }
        let size = requested.checked_add(PAGE - 1)? & !(PAGE - 1);
        if size > self.shared_limit().saturating_sub(self.shared) { return None; }
        let slot = self.blocks.iter().position(Option::is_none)?;
        let address = self.find_hole(size)?;
        match caching {
            crate::paging::Caching::Device => space.map_device(address, physical, size).ok()?,
            crate::paging::Caching::WriteCombining => space.map_framebuffer(address, physical, size, writable).ok()?,
            crate::paging::Caching::Memory => space.map(address, physical, size, writable, false).ok()?,
        }
        self.blocks[slot] = Some(Block { address, physical, memory: None, size, node, writable: writable || device, device, sink: false });
        self.shared += size;
        Some(address)
    }

    // Only the exact start of a heap block can be shared: this prevents handing out code, stack or foreign pages.
    // Own blocks only: sharing a foreign mapping would mint a new root with rights the mapper never had.
    pub fn shareable(&self, address: usize, requested: usize) -> Option<(usize, usize)> {
        let block = self.blocks.iter().flatten().find(|b| b.address == address && b.memory.is_some() && !b.sink)?;
        let size = if requested == 0 { block.size } else { requested.checked_add(PAGE - 1)? & !(PAGE - 1) };
        (size <= block.size).then_some((block.physical, size))
    }

    // Whether a mapped foreign block overlaps the given physical range.
    pub fn maps_foreign(&self, physical: usize, size: usize) -> bool {
        self.blocks.iter().flatten().any(|b| b.memory.is_none() && b.physical < physical + size && physical < b.physical + b.size)
    }

    // Takes foreign mappings made from revoked capabilities off the owner's memory; without `reclaim` page tables stay
    // (another CPU may still walk them until it switches address space). In a memory lease's place goes one zeroed page
    // of the task's own, at each of its pages, so a late access reads zeros and writes there instead of faulting
    // (000-KRN-0152); with no room for that page, for a device or for a task that ended (no `sink`), the range is left
    // unmapped. Returns whether anything changed and how many such pages were put in.
    pub fn revoke(&mut self, space: &mut Space, nodes: &[u64], reclaim: bool, sink: bool) -> (bool, usize) {
        let (mut any, mut sinks) = (false, 0);
        for index in 0..self.blocks.len() {
            let Some(block) = self.blocks[index].as_ref().filter(|b| b.memory.is_none() && nodes.contains(&b.node)) else { continue };
            let (address, size, writable, device) = (block.address, block.size, block.writable, block.device);
            space.unmap_leaves(address, size);
            self.shared -= size; self.blocks[index] = None; any = true;
            let room = PAGE <= self.limit.saturating_sub(self.bytes + self.retained);
            let page = if device || !room || !sink { None } else { Region::task(PAGE, PAGE).ok() };
            match page {
                Some(page) if space.map_repeated(address, page.ptr() as usize, size, writable).is_ok() => {
                    self.blocks[index] = Some(Block { address, physical: page.ptr() as usize, memory: Some(page), size, node: 0, writable, device: false, sink: true });
                    self.bytes += PAGE; sinks += 1;
                }
                _ => if reclaim { space.unmap(address, size); },
            }
        }
        (any, sinks)
    }

    // Statistics (STAT): private bytes and blocks, mapped foreign bytes, and what a heap-window address belongs to.
    pub fn bytes(&self) -> usize { self.bytes }
    pub fn shared_bytes(&self) -> usize { self.shared }
    pub fn block_count(&self) -> usize { self.blocks.iter().flatten().count() }
    pub fn kind_at(&self, address: usize) -> Option<(bool, bool)> {
        self.blocks.iter().flatten().find(|b| address >= b.address && address < b.address + b.size).map(|b| (b.memory.is_some(), b.device))
    }

    // Whether a foreign mapping was made from capability node `id`.
    pub fn made_from(&self, id: u64) -> bool { self.blocks.iter().flatten().any(|b| b.memory.is_none() && b.node == id) }

    // Whether any writable mapping (own block or foreign) overlaps the physical range.
    pub fn writes(&self, physical: usize, size: usize) -> bool {
        self.blocks.iter().flatten().any(|b| b.writable && b.physical < physical + size && physical < b.physical + b.owned())
    }

    // Takes an own block out of the address space and hands its memory to the caller (MEM_DETACH).
    pub fn detach(&mut self, space: &mut Space, address: usize) -> Option<Region> {
        let index = self.blocks.iter().position(|b| b.as_ref().is_some_and(|b| b.address == address && b.memory.is_some() && !b.sink))?;
        let block = self.blocks[index].take().unwrap();
        space.unmap(block.address, block.size);
        self.bytes -= block.size;
        block.memory
    }

    // Hands back owned regions when the task is destroyed so the kernel can decide whether they can be freed.
    pub fn take_regions(&mut self) -> impl Iterator<Item = Region> + '_ {
        self.blocks.iter_mut().filter_map(|b| b.as_mut().and_then(|b| b.memory.take()))
    }

    // Returns the freed region to the caller: if others still use it, the kernel holds on to it.
    pub fn free(&mut self, space: &mut Space, address: usize) -> Option<Option<Region>> {
        // A device mapping may be named by the address of registers inside its first page (MEM_MAP, 211-KRN-0021).
        let slot = self.blocks.iter().position(|b| b.as_ref().is_some_and(|b| b.address == address || (b.device && b.address == address & !(PAGE - 1))))?;
        let block = self.blocks[slot].take().unwrap();
        space.unmap(block.address, block.size);
        if block.memory.is_some() { self.bytes -= block.owned(); } else { self.shared -= block.size; }
        Some(block.memory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::HEAP_MAX_BYTES;

    #[test]
    fn the_pieces_cover_a_block_once_and_none_is_larger_than_2_mib() {
        for len in [0, 1, PAGE, LARGE - PAGE, LARGE, LARGE + PAGE, 64 * LARGE + 3 * PAGE, HUGE] {
            let mut end = 0;
            for piece in pieces(len) {
                assert_eq!(piece.start, end);
                assert!(!piece.is_empty() && piece.len() <= LARGE);
                end = piece.end;
            }
            assert_eq!(end, len);
        }
    }

    #[test]
    fn holes_are_reused_and_frees_are_checked() {
        let (mut space, mut heap) = (Space::new().unwrap(), Heap::new());
        let a = heap.allocate(&mut space, 4096, false).unwrap();
        let b = heap.allocate(&mut space, 8192, false).unwrap();
        assert!(b > a && space.writable(a).is_some() && space.writable(b + 4096).is_some());
        assert!(heap.free(&mut space, a).is_some());
        assert!(space.readable(a).is_none());
        assert!(heap.free(&mut space, a).is_none(), "double free");
        assert!(heap.free(&mut space, b + 4096).is_none(), "not the start of a block");
        assert_eq!(heap.allocate(&mut space, 4096, false), Some(a), "first fit reuses the hole");
    }

    #[test]
    fn block_and_byte_limits_include_retained_memory() {
        let (mut space, mut heap) = (Space::new().unwrap(), Heap::new());
        let blocks: Vec<usize> = (0..HEAP_MAX_BLOCKS).map(|_| heap.allocate(&mut space, 4096, false).unwrap()).collect();
        assert!(heap.allocate(&mut space, 4096, false).is_none(), "block limit");
        for block in blocks { heap.free(&mut space, block); }
        assert!(heap.allocate(&mut space, HEAP_MAX_BYTES + 1, false).is_none());
        heap.retained = HEAP_MAX_BYTES - 4096;
        assert!(heap.allocate(&mut space, 8192, false).is_none(), "retained blocks count against the quota");
        assert!(heap.allocate(&mut space, 4096, false).is_some());
    }

    #[test]
    fn only_own_blocks_are_shareable_and_revoke_unmaps_by_node() {
        let (mut space, mut heap) = (Space::new().unwrap(), Heap::new());
        let own = heap.allocate(&mut space, 4096, false).unwrap();
        let foreign = Region::new(8192, PAGE).unwrap();
        let mapped = heap.map_shared(&mut space, foreign.ptr() as usize, 8192, crate::paging::Caching::Memory, false, 7).unwrap();
        assert!(heap.shareable(own, 0).is_some());
        assert!(heap.shareable(mapped, 0).is_none(), "a mapping is not re-shareable");
        assert!(space.readable(mapped).is_some() && space.writable(mapped).is_none(), "read-only mapping");
        assert!(heap.made_from(7) && !heap.made_from(8));
        assert_eq!(heap.revoke(&mut space, &[8], true, false), (false, 0));
        assert_eq!(heap.revoke(&mut space, &[7], false, false), (true, 0), "an ended task gets no page in the lease's place");
        assert!(space.readable(mapped).is_none() && !heap.made_from(7));
        assert!(heap.map_shared(&mut space, foreign.ptr() as usize, SHARED_MAX_BYTES, crate::paging::Caching::Memory, true, 9).is_some(), "shared quota returned");
    }

    #[test]
    fn the_shared_limit_is_the_larger_of_its_own_and_the_memory_quota() {
        // 262-KRN-0162: a task with a quota above SHARED_MAX_BYTES may map foreign memory up to its quota, and no more.
        let quota = SHARED_MAX_BYTES + 2 * LARGE;
        let (mut space, mut heap) = (Space::new().unwrap(), Heap::with_limit(quota));
        let foreign = 0x40_0000_0000;
        let mapped = heap.map_shared(&mut space, foreign, SHARED_MAX_BYTES + LARGE, crate::paging::Caching::Memory, false, 7).unwrap();
        assert!(heap.map_shared(&mut space, foreign, 2 * LARGE, crate::paging::Caching::Memory, false, 8).is_none(), "past the quota");
        assert!(heap.map_shared(&mut space, foreign, LARGE, crate::paging::Caching::Memory, false, 8).is_some());
        assert!(heap.free(&mut space, mapped).is_some() && heap.shared_bytes() == LARGE);
        let (mut space, mut heap) = (Space::new().unwrap(), Heap::with_limit(LARGE));
        assert!(heap.map_shared(&mut space, foreign, SHARED_MAX_BYTES, crate::paging::Caching::Memory, false, 7).is_some(), "a small quota keeps SHARED_MAX_BYTES");
        assert!(heap.map_shared(&mut space, foreign, PAGE, crate::paging::Caching::Memory, false, 8).is_none());
    }

    #[test]
    fn a_revoked_lease_leaves_a_zeroed_page_of_the_borrowers_own() {
        let (mut space, mut heap) = (Space::new().unwrap(), Heap::with_limit(2 * PAGE));
        let mut foreign = Region::new(3 * PAGE, PAGE).unwrap();
        foreign.bytes_mut().fill(0x5A);
        let mapped = heap.map_shared(&mut space, foreign.ptr() as usize, 3 * PAGE, crate::paging::Caching::Memory, true, 7).unwrap();
        assert_eq!(heap.revoke(&mut space, &[7], true, true), (true, 1));
        // Every page of the range is one zeroed page of the task's own, writable as the lease was; the owner's is untouched.
        let pages: Vec<usize> = (0..3).map(|i| space.writable(mapped + i * PAGE).unwrap()).collect();
        assert!(pages.iter().all(|&p| p == pages[0] && p != foreign.ptr() as usize));
        unsafe { (pages[2] as *mut u8).write(7); assert_eq!((pages[0] as *const u8).read(), 7); }
        assert!(foreign.bytes_mut().iter().all(|&b| b == 0x5A), "the owner's memory is out of reach");
        assert!(!heap.made_from(7) && heap.shareable(mapped, 0).is_none() && heap.detach(&mut space, mapped).is_none());
        assert_eq!(heap.bytes(), PAGE, "one page charged, not the range");
        // Freeing the range releases the page.
        let page = heap.free(&mut space, mapped).unwrap().unwrap();
        assert_eq!(page.len(), PAGE);
        assert!(space.readable(mapped).is_none() && heap.bytes() == 0);
        // With no room left for the page, the range is unmapped as before.
        heap.allocate(&mut space, 2 * PAGE, false).unwrap();
        let again = heap.map_shared(&mut space, foreign.ptr() as usize, PAGE, crate::paging::Caching::Memory, false, 8).unwrap();
        assert_eq!(heap.revoke(&mut space, &[8], true, true), (true, 0));
        assert!(space.readable(again).is_none());
    }

    #[test]
    fn detach_leaves_the_address_space_and_the_quota() {
        let (mut space, mut heap) = (Space::new().unwrap(), Heap::new());
        let block = heap.allocate(&mut space, HEAP_MAX_BYTES, false).unwrap();
        let region = heap.detach(&mut space, block).unwrap();
        assert_eq!(region.len(), HEAP_MAX_BYTES);
        assert!(space.readable(block).is_none() && heap.free(&mut space, block).is_none());
        assert!(heap.allocate(&mut space, 4096, false).is_some());
    }

    #[test]
    fn large_blocks_are_aligned_and_a_refused_one_leaves_nothing() {
        // 262-KRN-0160: from 2 MiB on, a block lies on a 2 MiB boundary in both its addresses, after a small one too.
        let (mut space, mut heap) = (Space::new().unwrap(), Heap::new());
        let small = heap.allocate(&mut space, PAGE, false).unwrap();
        let large = heap.allocate(&mut space, 3 * LARGE, false).unwrap();
        assert!(large > small && large % LARGE == 0 && space.readable(large).unwrap() % LARGE == 0);
        // In two steps: reserved bytes count, a refused reservation counts nothing, a placed block is mapped and aligned.
        let before = heap.bytes();
        assert!(heap.reserve(HEAP_MAX_BYTES).is_none() && heap.bytes() == before);
        let size = heap.reserve(2 * LARGE).unwrap();
        assert_eq!(heap.bytes(), before + size);
        let placed = heap.place(&mut space, Heap::frames_unzeroed(size).unwrap(), false).unwrap();
        assert!(placed % LARGE == 0 && space.readable(placed + size - 1).is_some() && heap.bytes() == before + size);
        assert!(heap.free(&mut space, placed).is_some() && heap.bytes() == before);
        heap.reserve(LARGE).map(|size| heap.unreserve(size));
        assert_eq!(heap.bytes(), before);
    }
}
