//! MIND IDL wire format (docs/idl/README.md). Word calls: word 0 = method:8 | major:8 | fields from bit 16, word 1 =
//! fields; a reply has the status in bits 0..8 of word 0 and the result from bit 16. Buffer calls (v0.2): word 0 =
//! method | major << 8 | request length << 16, the client's buffer travels as the memory capability, the reply is
//! status | reply length << 16 and its payload is in the same buffer.
use crate::abi::CAP_KIND_MEMORY;
use crate::ipc::{self, Endpoint, Message, Received};
use crate::mem::{Mapping, Pages};
use crate::sys::{Error, Result};

pub const STATUS_OK: usize = 0;
pub const STATUS_NONE: usize = 1; // option result: none
pub const STATUS_ERROR: usize = 2; // result<_, error-code> or result<_, enum>: the error code or case is in word 1
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

/// A case of an enumerated error; a system error code instead (the server could not encode its reply) is that error.
pub fn enum_code(code: usize) -> Result<usize> {
    if code >= crate::abi::ERR_FIRST { return Err(crate::sys::check(code).err().unwrap_or(Error::Invalid)); }
    Ok(code)
}

/// Errors of `result<T, E>` with an enum E: Some(case) for an error reply, None for any other status.
pub fn enum_error(reply: &[usize; 2]) -> Result<Option<usize>> {
    if reply[0] & 0xFF != STATUS_ERROR { return Ok(None); }
    if reply[0] >> 8 != 0 { return Err(Error::Invalid); }
    enum_code(reply[1]).map(Some)
}

/// A word call whose result is a capability: it lands in the caller's fixed slot `receive`. Returns the reply words
/// and whether a capability arrived.
pub fn call_receiving(endpoint: Endpoint, words: [usize; 2], cap: Option<(usize, bool)>, receive: usize) -> Result<([usize; 2], bool)> {
    let message = match cap {
        None => Message::new(words[0], words[1]),
        Some((handle, true)) => Message::new(words[0], words[1]).with_cap_moved(handle, u8::MAX),
        Some((handle, false)) => Message::new(words[0], words[1]).with_cap(handle, u8::MAX),
    };
    endpoint.call(&message, receive).map(|reply| (reply.data, reply.cap_received))
}

/// Checks the reply of a capability result: None when the capability arrived with an empty ok reply, Some(code) for an
/// error reply. A capability that came with anything but success is dropped.
pub fn check_cap_reply(reply: &[usize; 2], received: bool, receive: usize) -> Result<Option<usize>> {
    let outcome = match reply[0] & 0xFF {
        STATUS_OK if reply[0] == 0 && reply[1] == 0 && received => Ok(None),
        STATUS_ERROR if reply[0] >> 8 == 0 => Ok(Some(reply[1])),
        _ => Err(Error::Invalid),
    };
    if received && !matches!(outcome, Ok(None)) { let _ = ipc::drop_cap(receive); }
    outcome
}

/// Errors of `result<T, error-code>`: the code (a system error code) is in word 1.
pub fn check_error(reply: &[usize; 2]) -> Result<()> {
    if reply[0] & 0xFF == STATUS_ERROR { return Err(crate::sys::check(reply[1]).err().unwrap_or(Error::Invalid)); }
    Ok(())
}

/// A call whose data travels in `buffer` (request already encoded, `length` bytes). The server's access to the buffer
/// ends before the reply is read (revoke, MC-2.6): it cannot change the reply while the client decodes it.
pub fn call_buffer(endpoint: Endpoint, method: usize, buffer: &Pages, length: usize) -> Result<[usize; 2]> {
    let cap = buffer.share()?;
    let reply = endpoint.call(&Message::new(method | length << 16, 0).with_cap(cap, 0), 0);
    let _ = ipc::revoke(cap); let _ = ipc::drop_cap(cap);
    Ok(reply?.data)
}

/// Checks a buffer reply: Some(length) of the payload, None for an empty option, the error of a fallible function.
pub fn buffer_reply(reply: &[usize; 2], max: usize, optional: bool, fallible: bool) -> Result<Option<usize>> {
    match reply[0] & 0xFF {
        STATUS_OK if reply[1] == 0 && reply[0] >> 16 <= max && reply[0] & 0xFF00 == 0 => Ok(Some(reply[0] >> 16)),
        STATUS_NONE if optional && reply[0] >> 8 == 0 && reply[1] == 0 => Ok(None),
        STATUS_ERROR if fallible => { check_error(reply)?; Err(Error::Invalid) }
        _ => Err(Error::Invalid),
    }
}

/// What a server needs to answer one request: the client's buffer (mapped) for buffer calls. Dropping it unmaps the
/// buffer and frees the received capability.
pub struct Call { mapping: Option<Mapping>, cap: usize, received: bool, reply: usize }
impl Call {
    pub fn words(request: &Received, cap: usize) -> Self { Self { mapping: None, cap, received: request.cap_received, reply: 0 } }
    /// Keeps the right to answer this call later (IPC_SAVE_REPLY), so the server can receive other requests meanwhile.
    pub fn defer(&mut self) -> Result<()> { if self.reply == 0 { self.reply = ipc::save_reply()?; } Ok(()) }
}

/// Sends the reply words to the caller of `call` (the last caller, or the saved one of a deferred call).
pub fn finish(call: Call, words: [usize; 2]) -> Result<()> { finish_message(call, Message::new(words[0], words[1])) }

/// Answers a capability result with the capability `handle` (copied, or moved when `moved`).
pub fn finish_cap(call: Call, handle: usize, moved: bool) -> Result<()> {
    let message = if moved { Message::new(STATUS_OK, 0).with_cap_moved(handle, u8::MAX) } else { Message::new(STATUS_OK, 0).with_cap(handle, u8::MAX) };
    finish_message(call, message)
}

fn finish_message(call: Call, message: Message) -> Result<()> {
    let saved = call.reply; drop(call);
    if saved == 0 { ipc::reply(&message) } else { ipc::reply_saved(saved, &message) }
}
impl Drop for Call {
    fn drop(&mut self) { drop(self.mapping.take()); if self.received { let _ = ipc::drop_cap(self.cap); } }
}

/// Server side of a buffer call: checks the header and the buffer, then copies the request into private memory before
/// anything is decoded (the client could change its buffer meanwhile, MC-2.11). Returns the call and the copy length.
pub fn take_buffer<const M: usize>(request: &Received, cap: usize, max_reply: usize, copy: &mut [u8; M]) -> core::result::Result<(Call, usize), Reject> {
    let mut call = Call { mapping: None, cap, received: request.cap_received, reply: 0 };
    let length = request.data[0] >> 16;
    if request.data[1] != 0 || length > M || length >> 32 != 0 || !request.cap_received || crate::dev::cap_info(cap).0 != CAP_KIND_MEMORY { return Err(Reject::Invalid); }
    let mapping = Mapping::new(cap).map_err(|_| Reject::Invalid)?;
    if mapping.len() < length || mapping.len() < max_reply { return Err(Reject::Invalid); }
    copy[..length].copy_from_slice(&mapping.as_slice()[..length]);
    call.mapping = Some(mapping);
    Ok((call, length))
}

/// Answers a buffer call: `encode` writes the result into the client's buffer.
pub fn reply_buffer(mut call: Call, encode: impl FnOnce(&mut super::codec::Writer) -> Option<()>) -> Result<()> {
    let Some(mapping) = call.mapping.as_mut() else { return finish(call, [STATUS_INVALID, 0]) };
    let mut writer = super::codec::Writer::new(mapping.as_mut_slice());
    let words = match encode(&mut writer) { Some(()) => [writer.len() << 16, 0], None => [STATUS_ERROR, crate::abi::ERR_NO_MEMORY] };
    finish(call, words)
}
/// Answers with an empty option.
pub fn reply_none(call: Call) -> Result<()> { finish(call, [STATUS_NONE, 0]) }
/// Answers a fallible function with its error.
pub fn reply_error(call: Call, error: Error) -> Result<()> { finish(call, [STATUS_ERROR, error.code()]) }
/// Answers a function with an enumerated error with the case `code`.
pub fn reply_code(call: Call, code: usize) -> Result<()> { finish(call, [STATUS_ERROR, code]) }
