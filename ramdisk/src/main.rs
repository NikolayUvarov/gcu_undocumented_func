#![no_std]
#![no_main]
// ramdisk: a block device in the service's own memory (8 MiB) for scratch files: vfs_server formats it as FAT on first
// mount and serves it as `ram:`. Its contents never outlive the boot. Writes need the write badge like any drive's.
// Given a memory object in SLOT_MEM (ramdisk#2, 352-KRN-0268), it serves that instead, read-only: the boot volume the
// loader read into memory from an El Torito entry.
use mind::abi::{BootInfo, BLOCK_SECTOR, CAP_KIND_MEMORY, SLOT_MEM};
use mind::block::{self, Driver, Outcome};
use mind::mem::{Mapping, Pages};

const BYTES: usize = 8 << 20;

enum Store { Own(Pages), Lent(Mapping) }

struct Ram { store: Store, sectors: u64 }

impl Ram {
    fn bytes(&self) -> &[u8] { match &self.store { Store::Own(pages) => pages.as_slice(), Store::Lent(map) => map.as_slice() } }
}

impl Driver for Ram {
    fn sectors(&self) -> u64 { self.sectors }
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> Outcome {
        let at = lba as usize * BLOCK_SECTOR;
        out[..count * BLOCK_SECTOR].copy_from_slice(&self.bytes()[at..at + count * BLOCK_SECTOR]);
        Ok(())
    }
    fn write(&mut self, lba: u64, count: usize, data: &[u8]) -> Outcome {
        let Store::Own(pages) = &mut self.store else { return Err(block::DriverError::Range) };
        let at = lba as usize * BLOCK_SECTOR;
        pages.as_mut_slice()[at..at + count * BLOCK_SECTOR].copy_from_slice(&data[..count * BLOCK_SECTOR]);
        Ok(())
    }
    fn read_only(&self) -> bool { matches!(self.store, Store::Lent(_)) }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut ram = if mind::dev::cap_info(SLOT_MEM).0 == CAP_KIND_MEMORY {
        let ram = Mapping::new(SLOT_MEM).ok().map(|map| { let sectors = (map.len() / BLOCK_SECTOR) as u64; Ram { store: Store::Lent(map), sectors } });
        match &ram {
            Some(r) => mind::println!("[RAMDISK] THE BOOT VOLUME IN MEMORY: {} KB, READ-ONLY", r.sectors as usize * BLOCK_SECTOR / 1024),
            None => mind::println!("[RAMDISK] THE BOOT VOLUME IN MEMORY COULD NOT BE MAPPED"),
        }
        ram
    } else {
        let ram = Pages::new(BYTES).map(|pages| Ram { store: Store::Own(pages), sectors: (BYTES / BLOCK_SECTOR) as u64 });
        match &ram {
            Some(_) => mind::println!("[RAMDISK] {} KB", BYTES / 1024),
            None => mind::println!("[RAMDISK] NO MEMORY"),
        }
        ram
    };
    block::serve(block::KIND_RAM, ram.as_mut().map(|r| r as &mut dyn Driver));
}
