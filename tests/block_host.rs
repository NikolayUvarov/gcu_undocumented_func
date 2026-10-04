//! Host tests of the block protocol's server side (libmind/src/block_protocol.rs): writing needs the write badge on
//! the client's capability and a writable medium (Appendix B.6); reads, clipping at the end, errors.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/block_protocol.rs"]
mod block_protocol;
use abi::*;
use block_protocol::{handle, Driver};

struct Memory { data: Vec<u8>, protected: bool, flushes: usize, fail: bool }
impl Memory { fn new(sectors: usize) -> Self { Self { data: vec![0; sectors * BLOCK_SECTOR], protected: false, flushes: 0, fail: false } } }
impl Driver for Memory {
    fn sectors(&self) -> u64 { (self.data.len() / BLOCK_SECTOR) as u64 }
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> bool {
        let at = lba as usize * BLOCK_SECTOR;
        out[..count * BLOCK_SECTOR].copy_from_slice(&self.data[at..at + count * BLOCK_SECTOR]);
        !self.fail
    }
    fn write(&mut self, lba: u64, count: usize, data: &[u8]) -> bool {
        if self.fail { return false; }
        let at = lba as usize * BLOCK_SECTOR;
        self.data[at..at + count * BLOCK_SECTOR].copy_from_slice(&data[..count * BLOCK_SECTOR]);
        true
    }
    fn flush(&mut self) -> bool { self.flushes += 1; true }
    fn read_only(&self) -> bool { self.protected }
}

// A driver that only reads (the trait's defaults).
struct ReadOnly;
impl Driver for ReadOnly {
    fn sectors(&self) -> u64 { 16 }
    fn read(&mut self, _: u64, _: usize, _: &mut [u8]) -> bool { true }
}

const WRITER: u16 = BLOCK_BADGE_WRITE;

#[test]
fn writing_needs_the_badge() {
    let mut disk = Memory::new(64);
    let mut buffer = vec![0xA5u8; 4 * BLOCK_SECTOR];
    assert_eq!(handle(BLOCK_WRITE, 4, 10, 0, BLOCK_KIND_ATA, Some(&mut buffer), &mut disk), [ERR_RIGHTS, 0], "no badge");
    assert_eq!(handle(BLOCK_WRITE, 4, 10, 2, BLOCK_KIND_ATA, Some(&mut buffer), &mut disk), [ERR_RIGHTS, 0], "another badge");
    assert_eq!(handle(BLOCK_FLUSH, 0, 0, 0, BLOCK_KIND_ATA, None, &mut disk), [ERR_RIGHTS, 0]);
    assert!(disk.data.iter().all(|&b| b == 0) && disk.flushes == 0);
    assert_eq!(handle(BLOCK_WRITE, 4, 10, WRITER, BLOCK_KIND_ATA, Some(&mut buffer), &mut disk), [4, 0]);
    assert_eq!(handle(BLOCK_FLUSH, 0, 0, WRITER, BLOCK_KIND_ATA, None, &mut disk), [0, 0]);
    assert_eq!(disk.flushes, 1);
    assert!(disk.data[10 * BLOCK_SECTOR..14 * BLOCK_SECTOR].iter().all(|&b| b == 0xA5));
    assert!(disk.data[14 * BLOCK_SECTOR..].iter().all(|&b| b == 0));
    // Reading needs no badge.
    let mut out = vec![0u8; 2 * BLOCK_SECTOR];
    assert_eq!(handle(BLOCK_READ, 2, 12, 0, BLOCK_KIND_ATA, Some(&mut out), &mut disk), [2, 0]);
    assert!(out.iter().all(|&b| b == 0xA5));
}

#[test]
fn info_tells_whether_the_client_can_write() {
    let mut disk = Memory::new(64);
    assert_eq!(handle(BLOCK_INFO, 0, 0, WRITER, BLOCK_KIND_USB, None, &mut disk), [64, BLOCK_KIND_USB]);
    assert_eq!(handle(BLOCK_INFO, 0, 0, 0, BLOCK_KIND_USB, None, &mut disk), [64, BLOCK_KIND_USB | BLOCK_INFO_READ_ONLY]);
    disk.protected = true;
    assert_eq!(handle(BLOCK_INFO, 0, 0, WRITER, BLOCK_KIND_USB, None, &mut disk), [64, BLOCK_KIND_USB | BLOCK_INFO_READ_ONLY]);
    let mut buffer = vec![1u8; BLOCK_SECTOR];
    assert_eq!(handle(BLOCK_WRITE, 1, 0, WRITER, BLOCK_KIND_USB, Some(&mut buffer), &mut disk), [ERR_RIGHTS, 0], "write-protected medium");
    // A driver without a write path is read-only for everyone.
    assert_eq!(handle(BLOCK_INFO, 0, 0, WRITER, BLOCK_KIND_ATA, None, &mut ReadOnly), [16, BLOCK_KIND_ATA | BLOCK_INFO_READ_ONLY]);
    assert_eq!(handle(BLOCK_WRITE, 1, 0, WRITER, BLOCK_KIND_ATA, Some(&mut buffer), &mut ReadOnly), [ERR_RIGHTS, 0]);
}

#[test]
fn requests_are_clipped_and_checked() {
    let mut disk = Memory::new(16);
    let mut buffer = vec![7u8; 8 * BLOCK_SECTOR];
    // At the end of the disk only what is there is written.
    assert_eq!(handle(BLOCK_WRITE, 8, 12, WRITER, BLOCK_KIND_AHCI, Some(&mut buffer), &mut disk), [4, 0]);
    // Not beyond the attached buffer either.
    let mut small = vec![0u8; 2 * BLOCK_SECTOR];
    assert_eq!(handle(BLOCK_READ, 8, 0, 0, BLOCK_KIND_AHCI, Some(&mut small), &mut disk), [2, 0]);
    assert_eq!(handle(BLOCK_READ, 1, 16, 0, BLOCK_KIND_AHCI, Some(&mut small), &mut disk), [ERR_INVALID, 0], "past the end");
    assert_eq!(handle(BLOCK_READ, 0, 0, 0, BLOCK_KIND_AHCI, Some(&mut small), &mut disk), [ERR_INVALID, 0], "no sectors");
    assert_eq!(handle(BLOCK_READ, 1, 0, 0, BLOCK_KIND_AHCI, None, &mut disk), [ERR_INVALID, 0], "no buffer attached");
    assert_eq!(handle(99, 1, 0, WRITER, BLOCK_KIND_AHCI, None, &mut disk), [ERR_INVALID, 0]);
    disk.fail = true;
    assert_eq!(handle(BLOCK_WRITE, 1, 0, WRITER, BLOCK_KIND_AHCI, Some(&mut buffer), &mut disk), [ERR_PEER, 0], "a device error");
}
