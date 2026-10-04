//! Host tests of the block protocol's server side (libmind/src/block_protocol.rs, idl/block.wit 1.1): writing needs
//! the write badge on the client's capability and a writable medium (Appendix B.6); the data must hold every sector
//! written; reads, clipping at the end, errors.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/sys.rs"]
mod sys;
#[path = "../libmind/src/block_protocol.rs"]
mod block_protocol;
use abi::*;
use block_protocol::{flush, read, writable, write, Driver, BADGE_WRITE};
use sys::Error;

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

const WRITER: u16 = BADGE_WRITE;

#[test]
fn writing_needs_the_badge() {
    let mut disk = Memory::new(64);
    let data = vec![0xA5u8; 4 * BLOCK_SECTOR];
    assert_eq!(write(&mut disk, 0, &data, 4, 10), Err(Error::Rights), "no badge");
    assert_eq!(write(&mut disk, 2, &data, 4, 10), Err(Error::Rights), "another badge");
    assert_eq!(flush(&mut disk, 0), Err(Error::Rights));
    assert!(disk.data.iter().all(|&b| b == 0) && disk.flushes == 0);
    assert_eq!(write(&mut disk, WRITER, &data, 4, 10), Ok(4));
    assert_eq!(flush(&mut disk, WRITER), Ok(()));
    assert_eq!(disk.flushes, 1);
    assert!(disk.data[10 * BLOCK_SECTOR..14 * BLOCK_SECTOR].iter().all(|&b| b == 0xA5));
    assert!(disk.data[14 * BLOCK_SECTOR..].iter().all(|&b| b == 0));
    // Reading needs no badge.
    let mut out = vec![0u8; 2 * BLOCK_SECTOR];
    assert_eq!(read(&mut disk, &mut out, 2, 12), Ok(2));
    assert!(out.iter().all(|&b| b == 0xA5));
}

#[test]
fn writable_tells_whether_the_client_can_write() {
    let mut disk = Memory::new(64);
    assert!(writable(WRITER, &disk));
    assert!(!writable(0, &disk));
    disk.protected = true;
    assert!(!writable(WRITER, &disk));
    let data = vec![1u8; BLOCK_SECTOR];
    assert_eq!(write(&mut disk, WRITER, &data, 1, 0), Err(Error::Rights), "write-protected medium");
    // A driver without a write path is read-only for everyone.
    assert!(!writable(WRITER, &ReadOnly));
    assert_eq!(write(&mut ReadOnly, WRITER, &data, 1, 0), Err(Error::Rights));
}

#[test]
fn requests_are_clipped_and_checked() {
    let mut disk = Memory::new(16);
    let data = vec![7u8; 8 * BLOCK_SECTOR];
    // At the end of the disk only what is there is written.
    assert_eq!(write(&mut disk, WRITER, &data, 8, 12), Ok(4));
    // The sealed data must hold every sector asked for.
    assert_eq!(write(&mut disk, WRITER, &data[..BLOCK_SECTOR], 2, 0), Err(Error::Invalid));
    assert_eq!(write(&mut disk, WRITER, &data, 0, 0), Err(Error::Invalid), "no sectors");
    assert_eq!(write(&mut disk, WRITER, &data, 1, 16), Err(Error::Invalid), "past the end");
    // Not beyond the attached buffer either.
    let mut small = vec![0u8; 2 * BLOCK_SECTOR];
    assert_eq!(read(&mut disk, &mut small, 8, 0), Ok(2));
    assert_eq!(read(&mut disk, &mut small, 1, 16), Err(Error::Invalid), "past the end");
    assert_eq!(read(&mut disk, &mut small, 0, 0), Err(Error::Invalid), "no sectors");
    assert_eq!(read(&mut disk, &mut [], 1, 0), Err(Error::Invalid), "an empty buffer");
    disk.fail = true;
    assert_eq!(write(&mut disk, WRITER, &data, 1, 0), Err(Error::Peer), "a device error");
    assert_eq!(read(&mut disk, &mut small, 1, 0), Err(Error::Peer));
}
