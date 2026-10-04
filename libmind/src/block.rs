//! Block devices: client for vfs_server and a common service loop for storage drivers.
use crate::abi::*;
use crate::idl::{block, wire};
use crate::ipc::{self, Endpoint};
use crate::mem::{Mapping, Pages};
use crate::sys::{Error, Result};

const RECEIVED_CAP: usize = 9;
pub const BUFFER: usize = BLOCK_MAX_SECTORS * BLOCK_SECTOR;

/// Storage driver with 512-byte sectors.
pub trait Driver {
    fn sectors(&self) -> u64;
    /// Reads `count` sectors (at most BLOCK_MAX_SECTORS) into `out`.
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> bool;
}

/// Driver loop for idl/block.wit: sectors, kind, attach of the client's buffer, read. Without a device every call
/// reports not-found.
pub fn serve(kind: usize, mut driver: Option<&mut dyn Driver>) -> ! {
    let mut buffer: Option<Mapping> = None;
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
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
                    (Some(device), Some(target)) if count > 0 && lba < device.sectors() => {
                        let count = (count as usize).min(BLOCK_MAX_SECTORS).min(target.len() / BLOCK_SECTOR).min((device.sectors() - lba) as usize);
                        if device.read(lba, count, &mut target.as_mut_slice()[..count * BLOCK_SECTOR]) { Ok(count as u16) } else { Err(Error::Peer) }
                    }
                    _ => Err(Error::Invalid),
                };
                block::reply_read(call, result)
            }
        };
    }
}

/// Block device behind a driver's IPC endpoint (client side, idl/block.wit).
/// A restarted driver instance has no buffer attached and answers a read with not-found (MC-6.4): the buffer is attached
/// to the new instance and the read is repeated, which is safe because reads are idempotent (MC-6.6).
pub struct Device { endpoint: Endpoint, buffer: Pages, sectors: u64, kind: usize }

impl Device {
    /// Waits for the driver to be ready; Err(NotFound) if there is no drive.
    pub fn open(endpoint: Endpoint) -> Result<Self> {
        let sectors = block::sectors(endpoint)?;
        let kind = block::kind(endpoint)? as usize;
        let buffer = Pages::new(BUFFER).ok_or(Error::NoMemory)?;
        let device = Self { endpoint, buffer, sectors, kind };
        device.attach()?;
        Ok(device)
    }
    // Lends the transfer buffer to the current driver instance.
    fn attach(&self) -> Result<()> {
        let cap = self.buffer.share()?;
        let attached = block::attach(self.endpoint, cap);
        let _ = ipc::drop_cap(cap); // the driver maps its own copy
        attached
    }
    pub fn sectors(&self) -> u64 { self.sectors }
    /// Device kind (BLOCK_KIND_*), to tell drives apart.
    pub fn kind(&self) -> usize { self.kind }
    /// Reads up to BLOCK_MAX_SECTORS sectors; the slice is valid until the next read.
    pub fn read(&mut self, lba: u64, count: usize) -> Result<&[u8]> {
        let count = count.min(BLOCK_MAX_SECTORS) as u16;
        let got = match block::read(self.endpoint, count, lba) {
            Err(Error::NotFound | Error::Peer) => { self.attach()?; block::read(self.endpoint, count, lba)? }
            other => other?,
        };
        Ok(&self.buffer.as_slice()[..got as usize * BLOCK_SECTOR])
    }
}
