#![no_std]
#![no_main]
// ramdisk: a block device in the service's own memory (8 MiB) for scratch files: vfs_server formats it as FAT on first
// mount and serves it as `ram:`. Its contents never outlive the boot. Writes need the write badge like any drive's.
use mind::abi::{BootInfo, BLOCK_SECTOR};
use mind::block::{self, Driver};
use mind::mem::Pages;

const BYTES: usize = 8 << 20;

struct Ram { pages: Pages }

impl Driver for Ram {
    fn sectors(&self) -> u64 { (BYTES / BLOCK_SECTOR) as u64 }
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> bool {
        let at = lba as usize * BLOCK_SECTOR;
        out[..count * BLOCK_SECTOR].copy_from_slice(&self.pages.as_slice()[at..at + count * BLOCK_SECTOR]);
        true
    }
    fn write(&mut self, lba: u64, count: usize, data: &[u8]) -> bool {
        let at = lba as usize * BLOCK_SECTOR;
        self.pages.as_mut_slice()[at..at + count * BLOCK_SECTOR].copy_from_slice(&data[..count * BLOCK_SECTOR]);
        true
    }
    fn read_only(&self) -> bool { false }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut ram = Pages::new(BYTES).map(|pages| Ram { pages });
    match &ram {
        Some(_) => mind::println!("[RAMDISK] {} KB", BYTES / 1024),
        None => mind::println!("[RAMDISK] NO MEMORY"),
    }
    block::serve(block::KIND_RAM, ram.as_mut().map(|r| r as &mut dyn Driver));
}
