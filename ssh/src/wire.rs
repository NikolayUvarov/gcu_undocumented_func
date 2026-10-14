//! The SSH data types (RFC 4251 section 5): bytes, booleans, uint32, uint64, strings, mpints and name-lists, read from a
//! received payload and written into a bounded buffer.
use crate::Error;
use alloc::vec::Vec;

pub struct Reader<'a> { data: &'a [u8], at: usize }

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self { Reader { data, at: 0 } }
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.data.len()).ok_or(Error::Protocol)?;
        let part = &self.data[self.at..end];
        self.at = end;
        Ok(part)
    }
    pub fn u8(&mut self) -> Result<u8, Error> { Ok(self.take(1)?[0]) }
    pub fn bool(&mut self) -> Result<bool, Error> { Ok(self.u8()? != 0) }
    pub fn u32(&mut self) -> Result<u32, Error> { let b = self.take(4)?; Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]])) }
    pub fn u64(&mut self) -> Result<u64, Error> { Ok((self.u32()? as u64) << 32 | self.u32()? as u64) }
    pub fn string(&mut self) -> Result<&'a [u8], Error> { let n = self.u32()? as usize; self.take(n) }
    /// The bytes not read yet.
    pub fn rest(&self) -> &'a [u8] { &self.data[self.at..] }
}

/// Whether name-list `list` holds `name`.
pub fn has(list: &[u8], name: &[u8]) -> bool { list.split(|&b| b == b',').any(|n| n == name) }

/// A payload being written.
#[derive(Default)]
pub struct Writer { pub data: Vec<u8> }

impl Writer {
    pub fn new(kind: u8) -> Self { Writer { data: alloc::vec![kind] } }
    pub fn u8(&mut self, v: u8) -> &mut Self { self.data.push(v); self }
    pub fn bool(&mut self, v: bool) -> &mut Self { self.u8(v as u8) }
    pub fn u32(&mut self, v: u32) -> &mut Self { self.data.extend_from_slice(&v.to_be_bytes()); self }
    pub fn u64(&mut self, v: u64) -> &mut Self { self.data.extend_from_slice(&v.to_be_bytes()); self }
    pub fn raw(&mut self, v: &[u8]) -> &mut Self { self.data.extend_from_slice(v); self }
    pub fn string(&mut self, v: &[u8]) -> &mut Self { self.u32(v.len() as u32).raw(v) }
    /// A non-negative integer given as big-endian bytes: no leading zero bytes, and a zero byte first when the top bit is set.
    pub fn mpint(&mut self, v: &[u8]) -> &mut Self {
        let v = &v[v.iter().position(|&b| b != 0).unwrap_or(v.len())..];
        let pad = v.first().is_some_and(|&b| b & 0x80 != 0);
        self.u32(v.len() as u32 + pad as u32);
        if pad { self.u8(0); }
        self.raw(v)
    }
}
