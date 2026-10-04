//! Block devices: client for vfs_server and a common service loop for storage drivers.
use crate::abi::*;
use crate::ipc::{self, Endpoint, Message};
use crate::mem::{Mapping, Pages};
use crate::sys::{check, Error, Result};

const RECEIVED_CAP: usize = 9;
pub const BUFFER: usize = BLOCK_MAX_SECTORS * BLOCK_SECTOR;

pub use crate::block_protocol::{handle, Driver};

/// Driver loop: INFO, ATTACH of the client buffer, READ, and WRITE/FLUSH for clients with the write badge. Without a
/// device, replies NOT_FOUND.
pub fn serve(kind: usize, mut driver: Option<&mut dyn Driver>) -> ! {
    let mut buffer: Option<Mapping> = None;
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        let (op, count, lba) = (request.data[0] & 0xFF, request.data[0] >> 8, request.data[1] as u64);
        let reply = match (op, driver.as_deref_mut()) {
            (_, None) => [ERR_NOT_FOUND, 0],
            (BLOCK_ATTACH, Some(_)) if request.cap_received => { drop(buffer.take()); buffer = Mapping::new(RECEIVED_CAP).ok(); [if buffer.is_some() { 0 } else { ERR_NO_MEMORY }, 0] }
            (_, Some(device)) => handle(op, count, lba, request.badge, kind, buffer.as_mut().map(|b| b.as_mut_slice()), device),
        };
        if request.cap_received { let _ = ipc::drop_cap(RECEIVED_CAP); }
        if request.is_call { let _ = ipc::reply(&Message::new(reply[0], reply[1])); }
    }
}

/// Block device behind a driver's IPC endpoint (client side).
pub struct Device { endpoint: Endpoint, buffer: Pages, sectors: u64, kind: usize, read_only: bool }

impl Device {
    /// Waits for the driver to be ready; Err(NotFound) if there is no drive.
    pub fn open(endpoint: Endpoint) -> Result<Self> {
        let info = endpoint.call(&Message::new(BLOCK_INFO, 0), 0)?;
        let sectors = check(info.data[0])? as u64;
        let buffer = Pages::new(BUFFER).ok_or(Error::NoMemory)?;
        let cap = buffer.share()?;
        let attached = endpoint.call(&Message::new(BLOCK_ATTACH, 0).with_cap(cap, 0), 0);
        let _ = ipc::drop_cap(cap); // the driver keeps its own copy of the capability
        check(attached?.data[0])?;
        Ok(Self { endpoint, buffer, sectors, kind: info.data[1] & 0xFF, read_only: info.data[1] & BLOCK_INFO_READ_ONLY != 0 })
    }
    pub fn sectors(&self) -> u64 { self.sectors }
    /// Device kind (BLOCK_KIND_*), to tell drives apart.
    pub fn kind(&self) -> usize { self.kind }
    /// Writes are refused: the medium is protected or this capability has no write badge.
    pub fn read_only(&self) -> bool { self.read_only }
    /// Reads up to BLOCK_MAX_SECTORS sectors; the slice is valid until the next read.
    pub fn read(&mut self, lba: u64, count: usize) -> Result<&[u8]> {
        let reply = self.endpoint.call(&Message::new(BLOCK_READ | count.min(BLOCK_MAX_SECTORS) << 8, lba as usize), 0)?;
        let got = check(reply.data[0])?;
        Ok(&self.buffer.as_slice()[..got * BLOCK_SECTOR])
    }
    /// Writes whole sectors (`data` up to BUFFER bytes, a multiple of BLOCK_SECTOR); returns the sectors written.
    pub fn write(&mut self, lba: u64, data: &[u8]) -> Result<usize> {
        if data.is_empty() || data.len() % BLOCK_SECTOR != 0 || data.len() > BUFFER { return Err(Error::Invalid); }
        self.buffer.as_mut_slice()[..data.len()].copy_from_slice(data);
        let reply = self.endpoint.call(&Message::new(BLOCK_WRITE | (data.len() / BLOCK_SECTOR) << 8, lba as usize), 0)?;
        check(reply.data[0])
    }
    /// Asks the drive to write its cache to the medium.
    pub fn flush(&mut self) -> Result<()> { check(self.endpoint.call(&Message::new(BLOCK_FLUSH, 0), 0)?.data[0]).map(drop) }
}
