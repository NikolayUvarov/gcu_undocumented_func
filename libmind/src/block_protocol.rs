//! The server side of the block protocol (idl/block.wit) without IPC: what a driver does for one request, and the write
//! rule of Appendix B.6 (writing needs `BADGE_WRITE` in the badge of the client's capability and a writable medium).
//! A client may be bounded to a window of the device's sectors, named by its badge (307-DRV-0054): it sees the window as
//! the whole device, and nothing outside it. Builds on the host for tests (tests/block_host.rs).
use crate::abi::*;
use crate::sys::{Error, Result};

/// Badge of the block client that may write; init mints it for vfs_server only.
pub const BADGE_WRITE: u16 = 1;
/// Device kind of the RAM disk, after BLOCK_KIND_ATA, _AHCI and _USB (BLOCK_KIND_VIRTIO follows it).
pub const KIND_RAM: usize = 4;
/// Windows a driver instance holds (307-DRV-0054), numbered 1 to WINDOWS; 0 is the whole device.
pub const WINDOWS: usize = 7;
/// The badge bits that name a client's window: bits 1 to 3.
pub const BADGE_WINDOW: u16 = 0b1110;
/// The badge of the client that sets windows (`window`, block.wit 1.2): `init`'s own, which it mints for itself alone.
/// Its window bits are 0, so it reads the whole device, and it carries no write right.
pub const BADGE_WINDOWS: u16 = 0x8000;

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

/// Clients of one driver instance that hold a buffer at once, each badge its own (307-KRN-0177): clients that share a
/// disk, as vfs_server's FAT partitions and blockstore's window on one stick, never read into each other's buffer.
pub const BUFFERS: usize = 8;

/// The transfer buffers lent to a driver instance, one per client badge. A badge that attaches again replaces its own;
/// with every place another badge's, attach is refused (`limit`), and nothing attached before is touched.
pub struct Buffers<T> { slots: [Option<(u16, T)>; BUFFERS] }

impl<T> Buffers<T> {
    pub const fn new() -> Self { Buffers { slots: [const { None }; BUFFERS] } }

    /// Drops `badge`'s buffer, if any, then lends it the one `make` maps; `make` is not called when no place is left.
    pub fn attach(&mut self, badge: u16, make: impl FnOnce() -> Result<T>) -> Result<()> {
        if let Some(own) = self.slots.iter_mut().find(|s| s.as_ref().is_some_and(|(b, _)| *b == badge)) { *own = None; }
        let place = self.slots.iter_mut().find(|s| s.is_none()).ok_or(Error::Other(ERR_LIMIT))?;
        *place = Some((badge, make()?));
        Ok(())
    }

    /// `badge`'s buffer; None when it attached none (a restarted driver's clients, before they attach again).
    pub fn get(&mut self, badge: u16) -> Option<&mut T> {
        self.slots.iter_mut().flatten().find(|(b, _)| *b == badge).map(|(_, buffer)| buffer)
    }
}

/// The badge of a client of `window` (0: the whole device), that may write if `write`.
pub const fn badge_for(window: u8, write: bool) -> u16 { ((window as u16) << 1 & BADGE_WINDOW) | if write { BADGE_WRITE } else { 0 } }

/// The window a badge names: 0 for the whole device.
pub const fn window_of(badge: u16) -> usize { ((badge & BADGE_WINDOW) >> 1) as usize }

/// A driver instance's windows (307-DRV-0054): each a first sector and a count, or not set. A restart starts with none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Windows([Option<(u64, u64)>; WINDOWS]);

impl Windows {
    pub const fn new() -> Self { Windows([None; WINDOWS]) }

    /// `window`: window `index` (1 to WINDOWS) is the `count` sectors from `first` of a device of `sectors`, for
    /// `badge`, which must be BADGE_WINDOWS exactly; set again, it moves. Rights for another badge; invalid for an index
    /// out of range, a count of 0 or a range past the device.
    pub fn set(&mut self, badge: u16, sectors: u64, index: u8, first: u64, count: u64) -> Result<()> {
        if badge != BADGE_WINDOWS { return Err(Error::Rights); }
        if !(1..=WINDOWS as u8).contains(&index) || count == 0 || first.checked_add(count).is_none_or(|end| end > sectors) { return Err(Error::Invalid); }
        self.0[index as usize - 1] = Some((first, count));
        Ok(())
    }

    /// What `badge`'s client sees of a device of `sectors`: the first sector and the count of its window, the whole
    /// device for window 0. Not-found for a window not set (a restarted driver, before `init` sets it again): never the
    /// whole device.
    pub fn view(&self, badge: u16, sectors: u64) -> Result<(u64, u64)> {
        match window_of(badge) {
            0 => Ok((0, sectors)),
            k => self.0[k - 1].ok_or(Error::NotFound),
        }
    }
}

/// Whether a client whose capability carries `badge` may write: the medium allows it, the badge carries the right, and
/// the window it names, if any, is set.
pub fn writable(badge: u16, driver: &dyn Driver, windows: &Windows) -> bool {
    badge & BADGE_WRITE != 0 && !driver.read_only() && windows.view(badge, driver.sectors()).is_ok()
}

/// `sectors`: the device's, or the client's window's.
pub fn sectors(driver: &dyn Driver, windows: &Windows, badge: u16) -> Result<u64> { windows.view(badge, driver.sectors()).map(|(_, count)| count) }

// Sectors of a request of `count` at `lba` within a view of `size` sectors, a buffer of `bytes` and BLOCK_MAX_SECTORS.
fn span(size: u64, count: u16, lba: u64, bytes: usize) -> Result<usize> {
    if count == 0 || lba >= size { return Err(Error::Invalid); }
    let count = (count as usize).min(BLOCK_MAX_SECTORS).min((size - lba) as usize);
    if bytes < count * BLOCK_SECTOR { return Err(Error::Invalid); }
    Ok(count)
}

/// `read`: `count` sectors from `lba` of the client's view into `target` (the attached buffer), clipped at the view's
/// end and the buffer's.
pub fn read(driver: &mut dyn Driver, windows: &Windows, badge: u16, target: &mut [u8], count: u16, lba: u64) -> Result<u16> {
    let (first, size) = windows.view(badge, driver.sectors())?;
    let fits = (target.len() / BLOCK_SECTOR).min(u16::MAX as usize) as u16;
    let count = span(size, count.min(fits), lba, target.len())?;
    if driver.read(first + lba, count, &mut target[..count * BLOCK_SECTOR]) { Ok(count as u16) } else { Err(Error::Peer) }
}

/// `write`: `count` sectors to `lba` of the client's view from `data` (sealed memory of at least `count` sectors,
/// clipped at the view's end). Rights without the write badge or on a protected medium.
pub fn write(driver: &mut dyn Driver, windows: &Windows, badge: u16, data: &[u8], count: u16, lba: u64) -> Result<u16> {
    let (first, size) = windows.view(badge, driver.sectors())?;
    if !writable(badge, driver, windows) { return Err(Error::Rights); }
    if data.len() < count as usize * BLOCK_SECTOR { return Err(Error::Invalid); }
    let count = span(size, count, lba, data.len())?;
    if driver.write(first + lba, count, &data[..count * BLOCK_SECTOR]) { Ok(count as u16) } else { Err(Error::Peer) }
}

/// `flush`: empties the drive's write cache. Rights without the write badge; not-found for a window not set.
pub fn flush(driver: &mut dyn Driver, windows: &Windows, badge: u16) -> Result<()> {
    windows.view(badge, driver.sectors())?;
    if !writable(badge, driver, windows) { return Err(Error::Rights); }
    if driver.flush() { Ok(()) } else { Err(Error::Peer) }
}
