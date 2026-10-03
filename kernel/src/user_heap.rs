use crate::abi::{HEAP_MAX_BLOCKS, HEAP_MAX_BYTES, SHARED_MAX_BYTES};
use crate::memory::Region;
use crate::paging::{Space, PAGE, USER_END, USER_HEAP};

struct Block {
    address: usize,
    physical: usize,
    memory: Option<Region>, // None: foreign physical memory (shared mapping)
    size: usize,
}

pub struct Heap {
    blocks: [Option<Block>; HEAP_MAX_BLOCKS],
    bytes: usize,
    shared: usize,
}

impl Heap {
    pub fn new() -> Self {
        Self { blocks: core::array::from_fn(|_| None), bytes: 0, shared: 0 }
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
        if size > HEAP_MAX_BYTES - self.bytes { return None; }
        let slot = self.blocks.iter().position(Option::is_none)?;
        let address = self.find_hole(size)?;
        let memory = Region::new(size, PAGE).ok()?;
        space.map(address, memory.ptr() as usize, size, true, false).ok()?;
        self.blocks[slot] = Some(Block { address, physical: memory.ptr() as usize, memory: Some(memory), size });
        self.bytes += size;
        Some(address)
    }

    // Mappings of foreign memory count against their own quota and don't eat into the private heap.
    pub fn map_shared(&mut self, space: &mut Space, physical: usize, requested: usize, device: bool) -> Option<usize> {
        if requested == 0 || physical % PAGE != 0 { return None; }
        let size = requested.checked_add(PAGE - 1)? & !(PAGE - 1);
        if size > SHARED_MAX_BYTES - self.shared { return None; }
        let slot = self.blocks.iter().position(Option::is_none)?;
        let address = self.find_hole(size)?;
        if device { space.map_device(address, physical, size).ok()?; } else { space.map(address, physical, size, true, false).ok()?; }
        self.blocks[slot] = Some(Block { address, physical, memory: None, size });
        self.shared += size;
        Some(address)
    }

    // Only the exact start of a heap block can be shared: this prevents handing out code, stack or foreign pages.
    pub fn shareable(&self, address: usize, requested: usize) -> Option<(usize, usize)> {
        let block = self.blocks.iter().flatten().find(|b| b.address == address)?;
        let size = if requested == 0 { block.size } else { requested.checked_add(PAGE - 1)? & !(PAGE - 1) };
        (size <= block.size).then_some((block.physical, size))
    }

    // Whether a mapped foreign block overlaps the given physical range.
    pub fn maps_foreign(&self, physical: usize, size: usize) -> bool {
        self.blocks.iter().flatten().any(|b| b.memory.is_none() && b.physical < physical + size && physical < b.physical + b.size)
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
