//! The server side of the block protocol without I/O: what a driver does for one request, and the write rule of
//! Appendix B.6 (writing needs `BLOCK_BADGE_WRITE` in the badge of the client's capability). Builds on the host for
//! tests (tests/block_host.rs).
use crate::abi::*;

/// Storage driver with 512-byte sectors.
pub trait Driver {
    fn sectors(&self) -> u64;
    /// Reads `count` sectors (at most BLOCK_MAX_SECTORS) into `out`.
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> bool;
    /// Writes `count` sectors from `data`; a driver that cannot write keeps the default.
    fn write(&mut self, _lba: u64, _count: usize, _data: &[u8]) -> bool { false }
    /// Empties the drive's write cache.
    fn flush(&mut self) -> bool { true }
    /// The medium is write-protected, or the driver cannot write.
    fn read_only(&self) -> bool { true }
}

/// The reply to INFO, READ, WRITE or FLUSH: `buffer` is the attached client buffer, `badge` the badge of the
/// capability the client used, `kind` the BLOCK_KIND_* of the driver.
pub fn handle(op: usize, count: usize, lba: u64, badge: u16, kind: usize, buffer: Option<&mut [u8]>, driver: &mut dyn Driver) -> [usize; 2] {
    let writer = badge & BLOCK_BADGE_WRITE != 0 && !driver.read_only();
    match op {
        // A client learns whether it can write: the medium allows it and its capability carries the right.
        BLOCK_INFO => [driver.sectors() as usize, kind | if writer { 0 } else { BLOCK_INFO_READ_ONLY }],
        BLOCK_READ | BLOCK_WRITE => {
            if op == BLOCK_WRITE && !writer { return [ERR_RIGHTS, 0]; }
            match buffer {
                Some(target) if count > 0 && lba < driver.sectors() => {
                    let count = count.min(BLOCK_MAX_SECTORS).min(target.len() / BLOCK_SECTOR).min((driver.sectors() - lba) as usize);
                    let bytes = &mut target[..count * BLOCK_SECTOR];
                    let done = if op == BLOCK_READ { driver.read(lba, count, bytes) } else { driver.write(lba, count, bytes) };
                    if done { [count, 0] } else { [ERR_PEER, 0] }
                }
                _ => [ERR_INVALID, 0],
            }
        }
        BLOCK_FLUSH if !writer => [ERR_RIGHTS, 0],
        BLOCK_FLUSH => if driver.flush() { [0, 0] } else { [ERR_PEER, 0] },
        _ => [ERR_INVALID, 0],
    }
}
