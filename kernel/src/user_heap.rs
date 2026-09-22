use crate::abi::{HEAP_MAX_BLOCKS, HEAP_MAX_BYTES};
use crate::memory::Region;
use crate::paging::{Space, PAGE, USER_END, USER_HEAP};

struct Block {
    address: usize,
    #[allow(dead_code)] memory: Option<Region>, // None означает, что физическая память чужая (Shared)
    size: usize,
}

pub struct Heap {
    blocks: [Option<Block>; HEAP_MAX_BLOCKS],
    bytes: usize,
}

impl Heap {
    pub fn new() -> Self {
        Self { blocks: core::array::from_fn(|_| None), bytes: 0 }
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
        self.blocks[slot] = Some(Block { address, memory: Some(memory), size });
        self.bytes += size;
        Some(address)
    }

    pub fn map_shared(&mut self, space: &mut Space, physical: usize, requested: usize) -> Option<usize> {
        if requested == 0 { return None; }
        let size = requested.checked_add(PAGE - 1)? & !(PAGE - 1);
        if size > HEAP_MAX_BYTES - self.bytes { return None; }
        let slot = self.blocks.iter().position(Option::is_none)?;
        let address = self.find_hole(size)?;
        
        // Мапим физические страницы, но НЕ владеем ими (memory: None)
        space.map(address, physical, size, true, false).ok()?;
        self.blocks[slot] = Some(Block { address, memory: None, size });
        self.bytes += size;
        Some(address)
    }

    pub fn free(&mut self, space: &mut Space, address: usize) -> bool {
        let Some(slot) = self.blocks.iter().position(|b| b.as_ref().is_some_and(|b| b.address == address)) else { return false; };
        let block = self.blocks[slot].take().unwrap();
        space.unmap(block.address, block.size);
        self.bytes -= block.size;
        true
    }
}
