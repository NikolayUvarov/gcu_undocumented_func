//! MIND IDL v0 wire format (docs/idl/README.md): word 0 = method:8 | major:8 | fields from bit 16, word 1 = fields;
//! a reply has the status in bits 0..8 of word 0 and the result from bit 16.
use crate::ipc::{self, Endpoint, Message, Received};
use crate::sys::{Error, Result};

pub const STATUS_OK: usize = 0;
pub const STATUS_NONE: usize = 1; // option result: none
pub const STATUS_INVALID: usize = 0x80; // the request failed the receiver's schema check
pub const STATUS_VERSION: usize = 0x81; // the receiver serves another major version

/// Why the receiver rejected a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject { Invalid, Version }

pub fn field(words: &[usize; 2], word: usize, shift: u32, bits: u32) -> usize {
    let mask = if bits >= 64 { usize::MAX } else { (1 << bits) - 1 };
    (words[word] >> shift) & mask
}

pub fn call(endpoint: Endpoint, words: [usize; 2], cap: Option<(usize, bool)>) -> Result<[usize; 2]> {
    let message = match cap {
        None => Message::new(words[0], words[1]),
        Some((handle, true)) => Message::new(words[0], words[1]).with_cap_moved(handle, u8::MAX),
        Some((handle, false)) => Message::new(words[0], words[1]).with_cap(handle, u8::MAX),
    };
    endpoint.call(&message, 0).map(|reply| reply.data)
}

/// Validates a reply; returns true for an empty option result.
pub fn check_reply(reply: &[usize; 2], used: [usize; 2], optional: bool) -> Result<bool> {
    let none = match reply[0] & 0xFF {
        STATUS_OK => false,
        STATUS_NONE if optional => true,
        _ => return Err(Error::Invalid),
    };
    let used = if none { [0, 0] } else { used };
    if reply[0] & !0xFF & !used[0] != 0 || reply[1] & !used[1] != 0 { return Err(Error::Invalid); }
    Ok(none)
}

/// A capability that is not accepted is dropped right away (MC-2.12).
pub fn discard(request: &Received, cap: usize) { if request.cap_received { let _ = ipc::drop_cap(cap); } }

pub fn header(request: &Received, cap: usize, major: usize) -> core::result::Result<(), Reject> {
    if !request.is_call || request.irq.is_some() { discard(request, cap); return Err(Reject::Invalid); }
    if (request.data[0] >> 8) & 0xFF != major { discard(request, cap); return Err(Reject::Version); }
    Ok(())
}

/// Unused bits must be zero and the capability must be present exactly when declared, of the declared kind.
pub fn body(request: &Received, cap: usize, used: [usize; 2], kind: usize, expects_cap: bool) -> core::result::Result<(), Reject> {
    let words = request.data;
    let clean = words[0] & !0xFFFF & !used[0] == 0 && words[1] & !used[1] == 0;
    let cap_ok = request.cap_received == expects_cap && (!expects_cap || crate::dev::cap_info(cap).0 == kind);
    if clean && cap_ok { Ok(()) } else { discard(request, cap); Err(Reject::Invalid) }
}

pub fn reply(words: [usize; 2]) -> Result<()> { ipc::reply(&Message::new(words[0], words[1])) }

/// Answers a request that failed `decode`.
pub fn reject(reason: Reject) -> Result<()> { reply([if reason == Reject::Version { STATUS_VERSION } else { STATUS_INVALID }, 0]) }
