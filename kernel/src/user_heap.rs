use crate::abi::{HEAP_MAX_BLOCKS, HEAP_MAX_BYTES, SHARED_MAX_BYTES};
use crate::memory::Region;
use crate::paging::{Space, PAGE, USER_END, USER_HEAP};

struct Block {
    address: usize,
    physical: usize,
    memory: Option<Region>, // None: foreign physical memory (shared mapping)
    size: usize,
    node: u64, // derivation node of the capability a foreign mapping was made from
    writable: bool,
}

pub struct Heap {
    blocks: [Option<Block>; HEAP_MAX_BLOCKS],
    bytes: usize,
    pub retained: usize, // freed or detached blocks others still hold, charged until released
    shared: usize,
}

impl Heap {
    pub fn new() -> Self {
        Self { blocks: core::array::from_fn(|_| None), bytes: 0, retained: 0, shared: 0 }
    }

    fn find_hole(&self, size: usize) -> Option<usize> {
        let mut address = USER_HEAP + PAGE;
        loop {
            let end = address.checked_add(size + PAGE)?;
            if end > USER_END { return None; }
            if let Some(block) = self.blocks.iter().flatten().find(|b| address < b.address + b.size + PAGE && end > b.address) {
                address = block.address + block.size + PAGE;
            } else { break; }
        }
        Some(address)
    }

    pub fn allocate(&mut self, space: &mut Space, requested: usize) -> Option<usize> {
        if requested == 0 { return None; }
        let size = requested.checked_add(PAGE - 1)? & !(PAGE - 1);
        if size > HEAP_MAX_BYTES.saturating_sub(self.bytes + self.retained) { return None; }
        let slot = self.blocks.iter().position(Option::is_none)?;
        let address = self.find_hole(size)?;
        let memory = Region::new(size, PAGE).ok()?;
        space.map(address, memory.ptr() as usize, size, true, false).ok()?;
        self.blocks[slot] = Some(Block { address, physical: memory.ptr() as usize, memory: Some(memory), size, node: 0, writable: true });
        self.bytes += size;
        Some(address)
    }

    // Mappings of foreign memory count against their own quota and don't eat into the private heap.
    pub fn map_shared(&mut self, space: &mut Space, physical: usize, requested: usize, device: bool, writable: bool, node: u64) -> Option<usize> {
        if requested == 0 || physical % PAGE != 0 { return None; }
        let size = requested.checked_add(PAGE - 1)? & !(PAGE - 1);
        if size > SHARED_MAX_BYTES - self.shared { return None; }
        let slot = self.blocks.iter().position(Option::is_none)?;
        let address = self.find_hole(size)?;
        if device { space.map_device(address, physical, size).ok()?; } else { space.map(address, physical, size, writable, false).ok()?; }
        self.blocks[slot] = Some(Block { address, physical, memory: None, size, node, writable: writable || device });
        self.shared += size;
        Some(address)
    }

    // Only the exact start of a heap block can be shared: this prevents handing out code, stack or foreign pages.
    // Own blocks only: sharing a foreign mapping would mint a new root with rights the mapper never had.
    pub fn shareable(&self, address: usize, requested: usize) -> Option<(usize, usize)> {
        let block = self.blocks.iter().flatten().find(|b| b.address == address && b.memory.is_some())?;
        let size = if requested == 0 { block.size } else { requested.checked_add(PAGE - 1)? & !(PAGE - 1) };
        (size <= block.size).then_some((block.physical, size))
    }

    // Whether a mapped foreign block overlaps the given physical range.
    pub fn maps_foreign(&self, physical: usize, size: usize) -> bool {
        self.blocks.iter().flatten().any(|b| b.memory.is_none() && b.physical < physical + size && physical < b.physical + b.size)
    }

    // Unmaps foreign mappings made from revoked capabilities; without `reclaim` page tables stay (another CPU may still
    // walk them until it switches address space). Returns whether anything was unmapped.
    pub fn revoke(&mut self, space: &mut Space, nodes: &[u64], reclaim: bool) -> bool {
        let mut any = false;
        for index in 0..self.blocks.len() {
            let Some(block) = self.blocks[index].as_ref().filter(|b| b.memory.is_none() && nodes.contains(&b.node)) else { continue };
            if reclaim { space.unmap(block.address, block.size); } else { space.unmap_leaves(block.address, block.size); }
            self.shared -= block.size; self.blocks[index] = None; any = true;
        }
        any
    }

    // Whether a foreign mapping was made from capability node `id`.
    pub fn made_from(&self, id: u64) -> bool { self.blocks.iter().flatten().any(|b| b.memory.is_none() && b.node == id) }

    // Whether any writable mapping (own block or foreign) overlaps the physical range.
    pub fn writes(&self, physical: usize, size: usize) -> bool {
        self.blocks.iter().flatten().any(|b| b.writable && b.physical < physical + size && physical < b.physical + b.size)
    }

    // Takes an own block out of the address space and hands its memory to the caller (MEM_DETACH).
    pub fn detach(&mut self, space: &mut Space, address: usize) -> Option<Region> {
        let index = self.blocks.iter().position(|b| b.as_ref().is_some_and(|b| b.address == address && b.memory.is_some()))?;
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
        let slot = self.blocks.iter().position(|b| b.as_ref().is_some_and(|b| b.address == address))?;
        let block = self.blocks[slot].take().unwrap();
        space.unmap(block.address, block.size);
        if block.memory.is_some() { self.bytes -= block.size; } else { self.shared -= block.size; }
        Some(block.memory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holes_are_reused_and_frees_are_checked() {
        let (mut space, mut heap) = (Space::new().unwrap(), Heap::new());
        let a = heap.allocate(&mut space, 4096).unwrap();
        let b = heap.allocate(&mut space, 8192).unwrap();
        assert!(b > a && space.writable(a).is_some() && space.writable(b + 4096).is_some());
        assert!(heap.free(&mut space, a).is_some());
        assert!(space.readable(a).is_none());
        assert!(heap.free(&mut space, a).is_none(), "double free");
        assert!(heap.free(&mut space, b + 4096).is_none(), "not the start of a block");
        assert_eq!(heap.allocate(&mut space, 4096), Some(a), "first fit reuses the hole");
    }

    #[test]
    fn block_and_byte_limits_include_retained_memory() {
        let (mut space, mut heap) = (Space::new().unwrap(), Heap::new());
        let blocks: Vec<usize> = (0..HEAP_MAX_BLOCKS).map(|_| heap.allocate(&mut space, 4096).unwrap()).collect();
        assert!(heap.allocate(&mut space, 4096).is_none(), "block limit");
        for block in blocks { heap.free(&mut space, block); }
        assert!(heap.allocate(&mut space, HEAP_MAX_BYTES + 1).is_none());
        heap.retained = HEAP_MAX_BYTES - 4096;
        assert!(heap.allocate(&mut space, 8192).is_none(), "retained blocks count against the quota");
        assert!(heap.allocate(&mut space, 4096).is_some());
    }

    #[test]
    fn only_own_blocks_are_shareable_and_revoke_unmaps_by_node() {
        let (mut space, mut heap) = (Space::new().unwrap(), Heap::new());
        let own = heap.allocate(&mut space, 4096).unwrap();
        let foreign = Region::new(8192, PAGE).unwrap();
        let mapped = heap.map_shared(&mut space, foreign.ptr() as usize, 8192, false, false, 7).unwrap();
        assert!(heap.shareable(own, 0).is_some());
        assert!(heap.shareable(mapped, 0).is_none(), "a mapping is not re-shareable");
        assert!(space.readable(mapped).is_some() && space.writable(mapped).is_none(), "read-only mapping");
        assert!(heap.made_from(7) && !heap.made_from(8));
        assert!(!heap.revoke(&mut space, &[8], true));
        assert!(heap.revoke(&mut space, &[7], false));
        assert!(space.readable(mapped).is_none() && !heap.made_from(7));
        assert!(heap.map_shared(&mut space, foreign.ptr() as usize, SHARED_MAX_BYTES, false, true, 9).is_some(), "shared quota returned");
    }

    #[test]
    fn detach_leaves_the_address_space_and_the_quota() {
        let (mut space, mut heap) = (Space::new().unwrap(), Heap::new());
        let block = heap.allocate(&mut space, HEAP_MAX_BYTES).unwrap();
        let region = heap.detach(&mut space, block).unwrap();
        assert_eq!(region.len(), HEAP_MAX_BYTES);
        assert!(space.readable(block).is_none() && heap.free(&mut space, block).is_none());
        assert!(heap.allocate(&mut space, 4096).is_some());
    }
}
