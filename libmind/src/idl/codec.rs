//! MIND IDL v0.2 buffer encoding: little-endian, byte-packed, in declaration order. A string is a u16 byte length and
//! UTF-8 bytes, a list a u16 count and its items. Every type has a static maximum size (`Wire::MAX`); decoding checks
//! every length against the declared bound, booleans and UTF-8, and that the whole payload is consumed.
//! No system calls here, so it is tested on the host (tests/runtime.rs).

/// A value with a bounded wire representation.
pub trait Wire: Sized {
    const MAX: usize;
    fn encode(&self, out: &mut Writer) -> Option<()>;
    fn decode(input: &mut Reader) -> Option<Self>;
}

pub struct Writer<'a> { buffer: &'a mut [u8], at: usize }
impl<'a> Writer<'a> {
    pub fn new(buffer: &'a mut [u8]) -> Self { Self { buffer, at: 0 } }
    pub fn bytes(&mut self, bytes: &[u8]) -> Option<()> {
        let end = self.at.checked_add(bytes.len()).filter(|&end| end <= self.buffer.len())?;
        self.buffer[self.at..end].copy_from_slice(bytes); self.at = end; Some(())
    }
    pub fn len(&self) -> usize { self.at }
    pub fn is_empty(&self) -> bool { self.at == 0 }
}

pub struct Reader<'a> { buffer: &'a [u8], at: usize }
impl<'a> Reader<'a> {
    pub fn new(buffer: &'a [u8]) -> Self { Self { buffer, at: 0 } }
    pub fn bytes(&mut self, count: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(count).filter(|&end| end <= self.buffer.len())?;
        let bytes = &self.buffer[self.at..end]; self.at = end; Some(bytes)
    }
    /// True when every byte was consumed (trailing bytes make a payload malformed).
    pub fn done(&self) -> bool { self.at == self.buffer.len() }
}

macro_rules! integer {
    ($($t:ty),*) => {$(
        impl Wire for $t {
            const MAX: usize = core::mem::size_of::<$t>();
            fn encode(&self, out: &mut Writer) -> Option<()> { out.bytes(&self.to_le_bytes()) }
            fn decode(input: &mut Reader) -> Option<Self> { Some(<$t>::from_le_bytes(input.bytes(<Self as Wire>::MAX)?.try_into().ok()?)) }
        }
    )*};
}
integer!(u8, u16, u32, u64);

impl Wire for bool {
    const MAX: usize = 1;
    fn encode(&self, out: &mut Writer) -> Option<()> { out.bytes(&[*self as u8]) }
    fn decode(input: &mut Reader) -> Option<Self> { match input.bytes(1)?[0] { 0 => Some(false), 1 => Some(true), _ => None } }
}

/// UTF-8 text of at most N bytes (`string<N>`), stored inline.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Text<const N: usize> { len: u16, bytes: [u8; N] }
impl<const N: usize> Default for Text<N> { fn default() -> Self { Self { len: 0, bytes: [0; N] } } }
impl<const N: usize> Text<N> {
    /// None if the text is longer than N bytes.
    pub fn new(text: &str) -> Option<Self> {
        if text.len() > N { return None; }
        let mut bytes = [0; N]; bytes[..text.len()].copy_from_slice(text.as_bytes());
        Some(Self { len: text.len() as u16, bytes })
    }
    pub fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("") }
}
impl<const N: usize> core::fmt::Debug for Text<N> { fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { write!(f, "{:?}", self.as_str()) } }
impl<const N: usize> core::fmt::Display for Text<N> { fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { f.write_str(self.as_str()) } }
impl<const N: usize> Wire for Text<N> {
    const MAX: usize = 2 + N;
    fn encode(&self, out: &mut Writer) -> Option<()> { self.len.encode(out)?; out.bytes(&self.bytes[..self.len as usize]) }
    fn decode(input: &mut Reader) -> Option<Self> {
        let len = u16::decode(input)? as usize;
        if len > N { return None; }
        Self::new(core::str::from_utf8(input.bytes(len)?).ok()?)
    }
}

/// At most N items (`list<T, N>`), stored inline.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct List<T: Copy + Default, const N: usize> { len: usize, items: [T; N] }
impl<T: Copy + Default, const N: usize> Default for List<T, N> { fn default() -> Self { Self { len: 0, items: [T::default(); N] } } }
impl<T: Copy + Default, const N: usize> List<T, N> {
    /// None if there are more than N items.
    pub fn from_slice(items: &[T]) -> Option<Self> {
        if items.len() > N { return None; }
        let mut list = Self::default(); list.items[..items.len()].copy_from_slice(items); list.len = items.len(); Some(list)
    }
    /// False when the list is full.
    pub fn push(&mut self, item: T) -> bool { if self.len == N { return false; } self.items[self.len] = item; self.len += 1; true }
    pub fn as_slice(&self) -> &[T] { &self.items[..self.len] }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}
impl<T: Copy + Default + core::fmt::Debug, const N: usize> core::fmt::Debug for List<T, N> { fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { f.debug_list().entries(self.as_slice()).finish() } }
impl<T: Wire + Copy + Default, const N: usize> Wire for List<T, N> {
    const MAX: usize = 2 + N * T::MAX;
    fn encode(&self, out: &mut Writer) -> Option<()> { encode_slice::<T, N>(self.as_slice(), out) }
    fn decode(input: &mut Reader) -> Option<Self> {
        let len = u16::decode(input)? as usize;
        if len > N { return None; }
        let mut list = Self::default();
        for item in list.items[..len].iter_mut() { *item = T::decode(input)?; }
        list.len = len; Some(list)
    }
}

/// Encodes a slice as `list<T, N>` (None past N items or past the buffer).
pub fn encode_slice<T: Wire, const N: usize>(items: &[T], out: &mut Writer) -> Option<()> {
    if items.len() > N { return None; }
    (items.len() as u16).encode(out)?;
    for item in items { item.encode(out)?; }
    Some(())
}
/// Encodes text as `string<N>` (None past N bytes or past the buffer).
pub fn encode_str<const N: usize>(text: &str, out: &mut Writer) -> Option<()> {
    if text.len() > N { return None; }
    (text.len() as u16).encode(out)?; out.bytes(text.as_bytes())
}
