//! MIND IDL v0.2 wire format (docs/idl/README.md): word 0 = method:8 | major:8 | fields from bit 16, word 1 = fields;
//! a reply has the status in bits 0..8 of word 0 and the result from bit 16. Strings, bytes, lists and records travel
//! in a memory buffer lent with the call (`Buffer`), encoded little-endian with explicit lengths (`Writer`, `Reader`).
use crate::abi::ERR_LIMIT;
use crate::ipc::{self, Endpoint, Message, Received};
use crate::sys::{Error, Result};
use core::marker::PhantomData;

pub const STATUS_OK: usize = 0;
pub const STATUS_NONE: usize = 1; // option result: none
pub const STATUS_FAILED: usize = 2; // result<T, E>: the error code is in bits 16..24
pub const STATUS_OVERFLOW: usize = 3; // the result does not fit the client's buffer
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

/// `call` whose reply may carry a capability into the caller's fixed slot `receive`: the reply words, and whether a
/// capability came.
pub fn call_receiving(endpoint: Endpoint, words: [usize; 2], cap: Option<(usize, bool)>, receive: usize) -> Result<([usize; 2], bool)> {
    let message = match cap {
        None => Message::new(words[0], words[1]),
        Some((handle, true)) => Message::new(words[0], words[1]).with_cap_moved(handle, u8::MAX),
        Some((handle, false)) => Message::new(words[0], words[1]).with_cap(handle, u8::MAX),
    };
    endpoint.call(&message, receive).map(|reply| (reply.data, reply.cap_received))
}

/// What a reply said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status { Ok, None, Failed(u8) }

/// Validates a reply: status, and no bits outside the declared result.
pub fn check_reply(reply: &[usize; 2], used: [usize; 2], optional: bool, fallible: bool) -> Result<Status> {
    let (status, used) = match reply[0] & 0xFF {
        STATUS_OK => (Status::Ok, used),
        STATUS_NONE if optional => (Status::None, [0, 0]),
        STATUS_FAILED if fallible => (Status::Failed((reply[0] >> 16) as u8), [0xFF << 16, 0]),
        STATUS_OVERFLOW => return Err(Error::Other(ERR_LIMIT)),
        _ => return Err(Error::Invalid),
    };
    if reply[0] & !0xFF & !used[0] != 0 || reply[1] & !used[1] != 0 { return Err(Error::Invalid); }
    Ok(status)
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

/// A reply carrying capability `handle`: moved, or copied so that the server can still revoke what it gave.
pub fn reply_cap(words: [usize; 2], handle: usize, moved: bool) -> Result<()> {
    let message = Message::new(words[0], words[1]);
    ipc::reply(&if moved { message.with_cap_moved(handle, u8::MAX) } else { message.with_cap(handle, u8::MAX) })
}

/// Answers a request that failed `decode`.
pub fn reject(reason: Reject) -> Result<()> { reply([if reason == Reject::Version { STATUS_VERSION } else { STATUS_INVALID }, 0]) }

/// A buffer lent to the server with a call: the capability and the client's view of the same memory.
pub struct Buffer<'b> { pub cap: usize, pub bytes: &'b mut [u8] }

/// A page block shared once and lent with every call (clients keep one per service).
pub struct Shared { pages: crate::mem::Pages, cap: usize }

impl Shared {
    pub fn new(bytes: usize) -> Result<Self> {
        let pages = crate::mem::Pages::new(bytes).ok_or(Error::NoMemory)?;
        let cap = pages.share()?;
        Ok(Self { pages, cap })
    }
    pub fn buffer(&mut self) -> Buffer<'_> { Buffer { cap: self.cap, bytes: self.pages.as_mut_slice() } }
}

impl Drop for Shared { fn drop(&mut self) { let _ = ipc::drop_cap(self.cap); } }

/// Encodes values into a buffer: integers little-endian, `bool` as 0/1, strings as u16 length + UTF-8, bytes as u32
/// length + bytes, lists as u32 count + items.
pub struct Writer<'a> { buf: &'a mut [u8], at: usize }

impl<'a> Writer<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self { Self { buf, at: 0 } }
    pub fn len(&self) -> usize { self.at }
    pub fn is_empty(&self) -> bool { self.at == 0 }
    pub fn into_inner(self) -> &'a mut [u8] { self.buf }
    fn put(&mut self, bytes: &[u8]) -> Result<()> {
        let end = self.at.checked_add(bytes.len()).filter(|&end| end <= self.buf.len()).ok_or(Error::Other(ERR_LIMIT))?;
        self.buf[self.at..end].copy_from_slice(bytes); self.at = end;
        Ok(())
    }
    pub fn u8(&mut self, value: u8) -> Result<()> { self.put(&[value]) }
    pub fn u16(&mut self, value: u16) -> Result<()> { self.put(&value.to_le_bytes()) }
    pub fn u32(&mut self, value: u32) -> Result<()> { self.put(&value.to_le_bytes()) }
    pub fn u64(&mut self, value: u64) -> Result<()> { self.put(&value.to_le_bytes()) }
    pub fn bool(&mut self, value: bool) -> Result<()> { self.put(&[value as u8]) }
    pub fn str(&mut self, value: &str, max: usize) -> Result<()> {
        if value.len() > max { return Err(Error::Invalid); }
        self.u16(value.len() as u16)?; self.put(value.as_bytes())
    }
    pub fn bytes(&mut self, value: &[u8], max: usize) -> Result<()> {
        if value.len() > max { return Err(Error::Invalid); }
        self.u32(value.len() as u32)?; self.put(value)
    }
    pub fn list<'i, T: Item<'i>>(&mut self, items: &[T], max: usize) -> Result<()> {
        if items.len() > max { return Err(Error::Invalid); }
        self.u32(items.len() as u32)?;
        for item in items { item.encode(self)?; }
        Ok(())
    }
}

/// Decodes what `Writer` wrote; every read checks bounds and limits and fails with `None` instead of trusting the data.
#[derive(Clone, Copy)]
pub struct Reader<'a> { buf: &'a [u8], at: usize }

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self { Self { buf, at: 0 } }
    /// Everything was consumed (no trailing bytes).
    pub fn end(&self) -> bool { self.at == self.buf.len() }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n).filter(|&end| end <= self.buf.len())?;
        let bytes = &self.buf[self.at..end]; self.at = end;
        Some(bytes)
    }
    pub fn u8(&mut self) -> Option<u8> { Some(self.take(1)?[0]) }
    pub fn u16(&mut self) -> Option<u16> { Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?)) }
    pub fn u32(&mut self) -> Option<u32> { Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?)) }
    pub fn u64(&mut self) -> Option<u64> { Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?)) }
    pub fn bool(&mut self) -> Option<bool> { match self.u8()? { 0 => Some(false), 1 => Some(true), _ => None } }
    pub fn str(&mut self, max: usize) -> Option<&'a str> {
        let len = self.u16()? as usize;
        if len > max { return None; }
        core::str::from_utf8(self.take(len)?).ok()
    }
    pub fn bytes(&mut self, max: usize) -> Option<&'a [u8]> {
        let len = self.u32()? as usize;
        if len > max { return None; }
        self.take(len)
    }
    pub fn count(&mut self, max: usize) -> Option<usize> { let n = self.u32()? as usize; (n <= max).then_some(n) }
}

/// A value that can be an item of a list or a field of a record.
pub trait Item<'a>: Sized + Copy {
    fn encode(&self, w: &mut Writer) -> Result<()>;
    fn decode(r: &mut Reader<'a>) -> Option<Self>;
}
impl<'a> Item<'a> for u8 { fn encode(&self, w: &mut Writer) -> Result<()> { w.u8(*self) } fn decode(r: &mut Reader<'a>) -> Option<Self> { r.u8() } }
impl<'a> Item<'a> for u16 { fn encode(&self, w: &mut Writer) -> Result<()> { w.u16(*self) } fn decode(r: &mut Reader<'a>) -> Option<Self> { r.u16() } }
impl<'a> Item<'a> for u32 { fn encode(&self, w: &mut Writer) -> Result<()> { w.u32(*self) } fn decode(r: &mut Reader<'a>) -> Option<Self> { r.u32() } }
impl<'a> Item<'a> for u64 { fn encode(&self, w: &mut Writer) -> Result<()> { w.u64(*self) } fn decode(r: &mut Reader<'a>) -> Option<Self> { r.u64() } }
impl<'a> Item<'a> for bool { fn encode(&self, w: &mut Writer) -> Result<()> { w.bool(*self) } fn decode(r: &mut Reader<'a>) -> Option<Self> { r.bool() } }

/// A decoded list: checked once when read, then iterated without copying.
#[derive(Clone, Copy)]
pub struct List<'a, T> { bytes: &'a [u8], count: usize, item: PhantomData<T> }

impl<'a, T: Item<'a>> List<'a, T> {
    pub fn read(r: &mut Reader<'a>, max: usize) -> Option<Self> {
        let count = r.count(max)?;
        let start = r.at;
        for _ in 0..count { T::decode(r)?; }
        Some(Self { bytes: &r.buf[start..r.at], count, item: PhantomData })
    }
    pub fn len(&self) -> usize { self.count }
    pub fn is_empty(&self) -> bool { self.count == 0 }
    pub fn iter(&self) -> ListIter<'a, T> { ListIter { reader: Reader::new(self.bytes), left: self.count, item: PhantomData } }
}

impl<'a, T: Item<'a> + core::fmt::Debug> core::fmt::Debug for List<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { f.debug_list().entries(self.iter()).finish() }
}

pub struct ListIter<'a, T> { reader: Reader<'a>, left: usize, item: PhantomData<T> }

impl<'a, T: Item<'a>> Iterator for ListIter<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.left == 0 { return None; }
        self.left -= 1;
        T::decode(&mut self.reader)
    }
}
