//! Block devices: client for vfs_server and a common service loop for storage drivers (idl/block.wit).
use crate::abi::*;
use crate::idl::{block, wire};
use crate::ipc::{self, Endpoint};
use crate::mem::{self, Mapping, Pages};
use crate::sys::{Error, Result};

const RECEIVED_CAP: usize = 9;
pub const BUFFER: usize = BLOCK_MAX_SECTORS * BLOCK_SECTOR;

pub use crate::block_protocol::{Driver, BADGE_WRITE, KIND_RAM};
use crate::block_protocol as protocol;

/// Driver loop for idl/block.wit: sectors, kind, attach of the client's buffer, read, and for a client with the write
/// badge write (from sealed memory) and flush. Without a device every call reports not-found.
pub fn serve(kind: usize, mut driver: Option<&mut dyn Driver>) -> ! {
    let mut buffer: Option<Mapping> = None;
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        let badge = request.badge;
        let _ = match block::decode(&request, RECEIVED_CAP) {
            Err(reason) => if request.is_call { wire::reject(reason) } else { Ok(()) },
            Ok((block::Request::Kind, call)) => block::reply_kind(call, kind as u8),
            Ok((block::Request::Sectors, call)) => block::reply_sectors(call, driver.as_deref().map(|d| d.sectors()).ok_or(Error::NotFound)),
            Ok((block::Request::Attach { buffer: cap }, call)) => {
                // The mapping outlives the received capability, which the reply drops.
                let attached = if driver.is_none() { Err(Error::NotFound) } else { drop(buffer.take()); Mapping::new(cap).map(|m| { buffer = Some(m); }) };
                block::reply_attach(call, attached)
            }
            Ok((block::Request::Read { count, lba }, call)) => {
                let result = match (driver.as_deref_mut(), buffer.as_mut()) {
                    (None, _) | (_, None) => Err(Error::NotFound),
                    (Some(device), Some(target)) => protocol::read(device, target.as_mut_slice(), count, lba),
                };
                block::reply_read(call, result)
            }
            Ok((block::Request::Writable, call)) => block::reply_writable(call, driver.as_deref().is_some_and(|d| protocol::writable(badge, d))),
            Ok((block::Request::Write { data, count, lba }, call)) => {
                // The data must be sealed: nobody can change it between the check and the write (SHARE_RO).
                let result = match driver.as_deref_mut() {
                    None => Err(Error::NotFound),
                    Some(device) if !protocol::writable(badge, device) => Err(Error::Rights),
                    Some(_) if !mem::sealed(data) => Err(Error::Invalid),
                    Some(device) => Mapping::new(data).and_then(|source| protocol::write(device, badge, source.as_slice(), count, lba)),
                };
                block::reply_write(call, result)
            }
            Ok((block::Request::Flush, call)) => block::reply_flush(call, match driver.as_deref_mut() { None => Err(Error::NotFound), Some(device) => protocol::flush(device, badge) }),
        };
    }
}

/// Block device behind a driver's IPC endpoint (client side, idl/block.wit).
/// A restarted driver instance has no buffer attached and answers a read with not-found (MC-6.4): the buffer is attached
/// to the new instance and the read is repeated, which is safe because reads are idempotent (MC-6.6); so is a write of
/// the same data.
/// The buffer is a LEASE the client ends with `CAP_REVOKE` on re-attach and on drop; the driver writes into it while
/// it is attached (a SHARE_RW adapter listed in docs/profile), and the client copies the data out after each reply.
/// Written data travels as a sealed read-only copy (SHARE_RO), so the driver writes exactly what it was given.
pub struct Device { endpoint: Endpoint, buffer: Pages, lease: usize, sectors: u64, kind: usize, writable: bool }

impl Device {
    /// Waits for the driver to be ready; Err(NotFound) if there is no drive.
    pub fn open(endpoint: Endpoint) -> Result<Self> {
        let sectors = block::sectors(endpoint)?;
        let kind = block::kind(endpoint)? as usize;
        let writable = block::writable(endpoint).unwrap_or(false);
        let buffer = Pages::new(BUFFER).ok_or(Error::NoMemory)?;
        let lease = buffer.share()?;
        let device = Self { endpoint, buffer, lease, sectors, kind, writable };
        device.attach()?;
        Ok(device)
    }
    // Lends the transfer buffer to the current driver instance; an earlier instance's access ends first.
    fn attach(&self) -> Result<()> {
        let _ = ipc::revoke(self.lease);
        block::attach(self.endpoint, self.lease)
    }
    pub fn sectors(&self) -> u64 { self.sectors }
    /// Device kind (BLOCK_KIND_*, KIND_RAM), to tell drives apart.
    pub fn kind(&self) -> usize { self.kind }
    /// Writes are refused: the medium is protected or this capability has no write badge.
    pub fn read_only(&self) -> bool { !self.writable }
    /// Reads up to BLOCK_MAX_SECTORS sectors; the slice is valid until the next read.
    pub fn read(&mut self, lba: u64, count: usize) -> Result<&[u8]> {
        let count = count.min(BLOCK_MAX_SECTORS) as u16;
        let got = match block::read(self.endpoint, count, lba) {
            Err(Error::NotFound | Error::Peer) => { self.attach()?; block::read(self.endpoint, count, lba)? }
            other => other?,
        };
        Ok(&self.buffer.as_slice()[..got as usize * BLOCK_SECTOR])
    }
    /// Writes whole sectors (`data` up to BUFFER bytes, a multiple of BLOCK_SECTOR); returns the sectors written.
    pub fn write(&mut self, lba: u64, data: &[u8]) -> Result<usize> {
        if data.is_empty() || data.len() % BLOCK_SECTOR != 0 || data.len() > BUFFER { return Err(Error::Invalid); }
        let count = (data.len() / BLOCK_SECTOR) as u16;
        let written = match self.write_sealed(data, count, lba) {
            Err(Error::NotFound | Error::Peer) => { self.attach()?; self.write_sealed(data, count, lba)? }
            other => other?,
        };
        Ok(written as usize)
    }
    fn write_sealed(&self, data: &[u8], count: u16, lba: u64) -> Result<u16> {
        let sealed = mem::sealed_copy(data)?;
        let result = block::write(self.endpoint, sealed, count, lba);
        if result.is_err() { let _ = ipc::drop_cap(sealed); } // still ours if the message was not delivered
        result
    }
    /// Asks the drive to write its cache to the medium.
    pub fn flush(&mut self) -> Result<()> { block::flush(self.endpoint) }
}

impl Drop for Device { fn drop(&mut self) { let _ = ipc::revoke(self.lease); let _ = ipc::drop_cap(self.lease); } }
