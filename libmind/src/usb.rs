//! USB class drivers' side of `usb_host` (idl/usb.wit, issue 164): the badges that name a client's device class, the
//! description of a claimed interface, and a client that keeps its buffer attached to the current host instance.
use crate::idl::usb;
use crate::ipc::{self, Endpoint};
use crate::mem::Pages;
use crate::sys::{Error, Result};

/// A client's badge names the one device class it may claim.
pub const BADGE_HID: u16 = 1;
pub const BADGE_STORAGE: u16 = 2;
/// The interface class a badge allows (HID 3, mass storage 8).
pub fn class_of(badge: u16) -> Option<u8> { match badge { BADGE_HID => Some(3), BADGE_STORAGE => Some(8), _ => None } }

/// The client's buffer: control data and descriptions at its start, bulk data anywhere in it.
pub const BUFFER: usize = 68 * 1024;
/// The largest control transfer and bulk transfer.
pub const CONTROL_MAX: usize = 4096;
pub const BULK_MAX: usize = 64 * 1024;

pub const MAX_ENDPOINTS: usize = 4;

/// One endpoint of an interface: address (bit 7: IN), attributes (bits 0-1: 2 bulk, 3 interrupt), packet size, interval.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EndpointInfo { pub address: u8, pub attributes: u8, pub packet: u16, pub interval: u8 }

impl EndpointInfo {
    pub fn is_in(&self) -> bool { self.address & 0x80 != 0 }
    pub fn is_bulk(&self) -> bool { self.attributes & 3 == 2 }
    pub fn is_interrupt(&self) -> bool { self.attributes & 3 == 3 }
}

/// A claimed interface as `claim` describes it at the buffer's start.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Interface {
    pub class: u8, pub subclass: u8, pub protocol: u8, pub number: u8, pub speed: u8,
    pub vendor: u16, pub product: u16, pub count: u8, pub endpoints: [EndpointInfo; MAX_ENDPOINTS],
}

pub const INTERFACE_BYTES: usize = 10 + 5 * MAX_ENDPOINTS;

impl Interface {
    pub fn encode(&self, out: &mut [u8]) {
        out[..10].copy_from_slice(&[self.class, self.subclass, self.protocol, self.number, self.speed, self.vendor as u8, (self.vendor >> 8) as u8, self.product as u8, (self.product >> 8) as u8, self.count]);
        for (i, e) in self.endpoints.iter().enumerate() {
            out[10 + 5 * i..15 + 5 * i].copy_from_slice(&[e.address, e.attributes, e.packet as u8, (e.packet >> 8) as u8, e.interval]);
        }
    }
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let b = bytes.get(..INTERFACE_BYTES)?;
        let mut endpoints = [EndpointInfo::default(); MAX_ENDPOINTS];
        for (i, e) in endpoints.iter_mut().enumerate() {
            let at = 10 + 5 * i;
            *e = EndpointInfo { address: b[at], attributes: b[at + 1], packet: u16::from_le_bytes([b[at + 2], b[at + 3]]), interval: b[at + 4] };
        }
        Some(Self { class: b[0], subclass: b[1], protocol: b[2], number: b[3], speed: b[4], vendor: u16::from_le_bytes([b[5], b[6]]),
            product: u16::from_le_bytes([b[7], b[8]]), count: b[9].min(MAX_ENDPOINTS as u8), endpoints })
    }
    pub fn endpoints(&self) -> &[EndpointInfo] { &self.endpoints[..self.count as usize] }
}

/// A class driver's connection to `usb_host`. The buffer is a lease (revoked on re-attach and on drop); after the host
/// restarts every handle is gone, so the client claims its interfaces again.
pub struct Host { endpoint: Endpoint, buffer: Pages, lease: usize, attached: bool }

impl Host {
    pub fn new(endpoint: Endpoint) -> Result<Self> {
        let buffer = Pages::new(BUFFER).ok_or(Error::NoMemory)?;
        let lease = buffer.share()?;
        Ok(Self { endpoint, buffer, lease, attached: false })
    }
    pub fn buffer(&self) -> &[u8] { self.buffer.as_slice() }
    pub fn buffer_mut(&mut self) -> &mut [u8] { self.buffer.as_mut_slice() }

    // A lost host instance (ERR_PEER) or one without our buffer (not-found) needs the buffer again.
    fn check<T>(&mut self, result: Result<T>) -> Result<T> {
        if matches!(result, Err(Error::Peer)) { self.attached = false; }
        result
    }

    /// The next interface of this client's class, attaching the buffer first if the host is new.
    pub fn claim(&mut self) -> Result<(u32, Interface)> {
        if !self.attached {
            let _ = ipc::revoke(self.lease);
            usb::attach(self.endpoint, self.lease)?;
            self.attached = true;
        }
        let result = usb::claim(self.endpoint);
        // Invalid: a new host instance without our buffer; attach again next time.
        if matches!(result, Err(Error::Invalid)) { self.attached = false; }
        let handle = self.check(result)?;
        Ok((handle, Interface::decode(self.buffer()).ok_or(Error::Invalid)?))
    }
    pub fn release(&mut self, handle: u32) -> Result<()> { let r = usb::release(self.endpoint, handle); self.check(r) }
    /// A control transfer; the data stage is at the buffer's start.
    pub fn control(&mut self, handle: u32, request_type: u8, request: u8, value: u16, index: u16, length: u16) -> Result<usize> {
        let r = usb::control(self.endpoint, handle, request_type, request, value, index, length);
        self.check(r).map(|n| n as usize)
    }
    /// A bulk transfer of `length` bytes at `offset` in the buffer.
    pub fn bulk(&mut self, handle: u32, address: u8, offset: usize, length: usize) -> Result<usize> {
        let r = usb::bulk(self.endpoint, handle, address, offset as u32, length as u32);
        self.check(r).map(|n| n as usize)
    }
    /// The interrupt reports received since the last call: each is a length byte and the report at the buffer's start.
    pub fn reports(&mut self, handle: u32, address: u8, mut each: impl FnMut(&[u8])) -> Result<usize> {
        let r = usb::reports(self.endpoint, handle, address);
        let count = self.check(r)? as usize;
        let buffer = self.buffer.as_slice();
        let mut at = 0;
        for _ in 0..count {
            let Some(&length) = buffer.get(at) else { break };
            let Some(report) = buffer.get(at + 1..at + 1 + length as usize) else { break };
            each(report);
            at += 1 + length as usize;
        }
        Ok(count)
    }
}

impl Drop for Host { fn drop(&mut self) { let _ = ipc::revoke(self.lease); let _ = ipc::drop_cap(self.lease); } }
