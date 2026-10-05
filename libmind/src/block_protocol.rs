//! The server side of the block protocol (idl/block.wit) without IPC: what a driver does for one request, and the write
//! rule of Appendix B.6 (writing needs `BADGE_WRITE` in the badge of the client's capability and a writable medium).
//! Builds on the host for tests (tests/block_host.rs).
use crate::abi::*;
use crate::sys::{Error, Result};

/// Badge of the block client that may write; init mints it for vfs_server only.
pub const BADGE_WRITE: u16 = 1;
/// Device kind of the RAM disk, after BLOCK_KIND_ATA, _AHCI and _USB (BLOCK_KIND_VIRTIO follows it).
pub const KIND_RAM: usize = 4;

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

/// Whether a client whose capability carries `badge` may write: the medium allows it and the badge carries the right.
pub fn writable(badge: u16, driver: &dyn Driver) -> bool { badge & BADGE_WRITE != 0 && !driver.read_only() }

// Sectors of a request of `count` at `lba` within the device, a buffer of `bytes` and BLOCK_MAX_SECTORS.
fn span(driver: &dyn Driver, count: u16, lba: u64, bytes: usize) -> Result<usize> {
    if count == 0 || lba >= driver.sectors() { return Err(Error::Invalid); }
    let count = (count as usize).min(BLOCK_MAX_SECTORS).min((driver.sectors() - lba) as usize);
    if bytes < count * BLOCK_SECTOR { return Err(Error::Invalid); }
    Ok(count)
}

/// `read`: `count` sectors from `lba` into `target` (the attached buffer), clipped at the end of the device and of the
/// buffer.
pub fn read(driver: &mut dyn Driver, target: &mut [u8], count: u16, lba: u64) -> Result<u16> {
    let fits = (target.len() / BLOCK_SECTOR).min(u16::MAX as usize) as u16;
    let count = span(driver, count.min(fits), lba, target.len())?;
    if driver.read(lba, count, &mut target[..count * BLOCK_SECTOR]) { Ok(count as u16) } else { Err(Error::Peer) }
}

/// `write`: `count` sectors to `lba` from `data` (sealed memory of at least `count` sectors, clipped at the end of the
/// device). Rights without the write badge or on a protected medium.
pub fn write(driver: &mut dyn Driver, badge: u16, data: &[u8], count: u16, lba: u64) -> Result<u16> {
    if !writable(badge, driver) { return Err(Error::Rights); }
    if data.len() < count as usize * BLOCK_SECTOR { return Err(Error::Invalid); }
    let count = span(driver, count, lba, data.len())?;
    if driver.write(lba, count, &data[..count * BLOCK_SECTOR]) { Ok(count as u16) } else { Err(Error::Peer) }
}

/// `flush`: empties the drive's write cache. Rights without the write badge.
pub fn flush(driver: &mut dyn Driver, badge: u16) -> Result<()> {
    if !writable(badge, driver) { return Err(Error::Rights); }
    if driver.flush() { Ok(()) } else { Err(Error::Peer) }
}
