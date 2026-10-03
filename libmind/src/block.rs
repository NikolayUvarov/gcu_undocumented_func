//! Block devices: client for vfs_server and a common service loop for storage drivers.
use crate::abi::*;
use crate::ipc::{self, Endpoint, Message};
use crate::mem::{Mapping, Pages};
use crate::sys::{check, Error, Result};

const RECEIVED_CAP: usize = 9;
pub const BUFFER: usize = BLOCK_MAX_SECTORS * BLOCK_SECTOR;

/// Storage driver with 512-byte sectors.
pub trait Driver {
    fn sectors(&self) -> u64;
    /// Reads `count` sectors (at most BLOCK_MAX_SECTORS) into `out`.
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> bool;
}

/// Driver loop: INFO, ATTACH of the client buffer, and READ. Without a device, replies NOT_FOUND.
pub fn serve(kind: usize, mut driver: Option<&mut dyn Driver>) -> ! {
    let mut buffer: Option<Mapping> = None;
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        let (op, count, lba) = (request.data[0] & 0xFF, request.data[0] >> 8, request.data[1] as u64);
        let reply = match (op, driver.as_deref_mut()) {
            (_, None) => [ERR_NOT_FOUND, 0],
            (BLOCK_INFO, Some(device)) => [device.sectors() as usize, kind],
            (BLOCK_ATTACH, Some(_)) if request.cap_received => { drop(buffer.take()); buffer = Mapping::new(RECEIVED_CAP).ok(); [if buffer.is_some() { 0 } else { ERR_NO_MEMORY }, 0] }
            (BLOCK_READ, Some(device)) => match buffer.as_mut() {
                Some(target) if count > 0 && lba < device.sectors() => {
                    let count = count.min(BLOCK_MAX_SECTORS).min(target.len() / BLOCK_SECTOR).min((device.sectors() - lba) as usize);
                    if device.read(lba, count, &mut target.as_mut_slice()[..count * BLOCK_SECTOR]) { [count, 0] } else { [ERR_PEER, 0] }
                }
                _ => [ERR_INVALID, 0],
            },
            _ => [ERR_INVALID, 0],
        };
        if request.cap_received { let _ = ipc::drop_cap(RECEIVED_CAP); }
        if request.is_call { let _ = ipc::reply(&Message::new(reply[0], reply[1])); }
    }
}

/// Block device behind a driver's IPC endpoint (client side).
pub struct Device { endpoint: Endpoint, buffer: Pages, sectors: u64, kind: usize }

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
        Ok(Self { endpoint, buffer, sectors, kind: info.data[1] })
    }
    pub fn sectors(&self) -> u64 { self.sectors }
    /// Device kind (BLOCK_KIND_*), to tell drives apart.
    pub fn kind(&self) -> usize { self.kind }
    /// Reads up to BLOCK_MAX_SECTORS sectors; the slice is valid until the next read.
    pub fn read(&mut self, lba: u64, count: usize) -> Result<&[u8]> {
        let reply = self.endpoint.call(&Message::new(BLOCK_READ | count.min(BLOCK_MAX_SECTORS) << 8, lba as usize), 0)?;
        let got = check(reply.data[0])?;
        Ok(&self.buffer.as_slice()[..got * BLOCK_SECTOR])
    }
}
