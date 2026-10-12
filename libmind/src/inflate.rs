//! DEFLATE (RFC 1951) and its zlib wrapper (RFC 1950), decompressed as a stream: what comes out is handed to a sink a
//! piece at a time, so a large picture is never held whole (000-APP-0050's PNG backgrounds). Only a 32 KiB window
//! of the output is kept for back-references. No system calls: host-tested in tests/image_host.rs.
use alloc::vec;
use alloc::vec::Vec;

const WINDOW: usize = 32 * 1024;
const LENGTH_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DIST_EXTRA: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

/// Why a stream could not be decompressed.
pub type Error = &'static str;

// The input: one or more pieces read as one stream (a PNG's IDAT chunks), least significant bit first.
struct Bits<'a> { parts: &'a [&'a [u8]], part: usize, at: usize, bit: u32, held: u32 }

impl Bits<'_> {
    fn byte(&mut self) -> Option<u8> {
        while self.part < self.parts.len() {
            if let Some(&byte) = self.parts[self.part].get(self.at) { self.at += 1; return Some(byte); }
            self.part += 1;
            self.at = 0;
        }
        None
    }
    fn need(&mut self, n: u32) -> Result<(), Error> {
        while self.held < n {
            let byte = self.byte().ok_or("the stream ends early")?;
            self.bit |= (byte as u32) << self.held;
            self.held += 8;
        }
        Ok(())
    }
    fn take(&mut self, n: u32) -> Result<u32, Error> {
        if n == 0 { return Ok(0); }
        self.need(n)?;
        let value = self.bit & ((1u32 << n) - 1);
        self.bit >>= n; self.held -= n;
        Ok(value)
    }
    fn align(&mut self) { let drop = self.held % 8; self.bit >>= drop; self.held -= drop; }
}

// A canonical Huffman code: how many codes each length has, and the symbols in code order.
struct Huffman { counts: [u16; 16], symbols: Vec<u16> }

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Self, Error> {
        let mut counts = [0u16; 16];
        for &l in lengths { counts[l as usize] += 1; }
        counts[0] = 0;
        let mut offsets = [0u16; 16];
        for i in 1..16 { offsets[i] = offsets[i - 1] + counts[i - 1]; }
        let mut symbols = vec![0u16; lengths.len()];
        for (symbol, &l) in lengths.iter().enumerate() { if l != 0 { symbols[offsets[l as usize] as usize] = symbol as u16; offsets[l as usize] += 1; } }
        Ok(Self { counts, symbols })
    }
    fn decode(&self, bits: &mut Bits) -> Result<u16, Error> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= bits.take(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first { return self.symbols.get((index + code - first) as usize).copied().ok_or("a bad code"); }
            index += count; first += count; first <<= 1; code <<= 1;
        }
        Err("a code longer than 15 bits")
    }
}

// The output: a window for back-references, flushed to the sink in pieces.
struct Out<'a> { window: Vec<u8>, at: usize, filled: usize, pending: Vec<u8>, sink: &'a mut dyn FnMut(&[u8]) -> bool, stopped: bool }

impl Out<'_> {
    fn push(&mut self, byte: u8) {
        self.window[self.at] = byte;
        self.at = (self.at + 1) % WINDOW;
        self.filled = (self.filled + 1).min(WINDOW);
        self.pending.push(byte);
        if self.pending.len() >= 4096 { self.flush(); }
    }
    fn copy(&mut self, distance: usize, length: usize) -> Result<(), Error> {
        if distance > self.filled { return Err("a distance before the start"); }
        for _ in 0..length { let byte = self.window[(self.at + WINDOW - distance) % WINDOW]; self.push(byte); }
        Ok(())
    }
    fn flush(&mut self) {
        if !self.stopped && !self.pending.is_empty() { self.stopped = !(self.sink)(&self.pending); }
        self.pending.clear();
    }
}

/// Decompresses a raw DEFLATE stream, handing the bytes to `sink` in pieces (`sink` returns false to stop early).
pub fn inflate(data: &[u8], sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), Error> {
    deflate(&mut Bits { parts: &[data], part: 0, at: 0, bit: 0, held: 0 }, sink).map(|_| ())
}

// The blocks of a stream; true when it was read to its end (the sink did not stop it).
fn deflate(bits: &mut Bits, sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<bool, Error> {
    let mut out = Out { window: vec![0; WINDOW], at: 0, filled: 0, pending: Vec::with_capacity(4096), sink, stopped: false };
    loop {
        let last = bits.take(1)? == 1;
        match bits.take(2)? {
            0 => {
                bits.align();
                let len = bits.take(16)? as usize;
                if bits.take(16)? as usize != !len & 0xFFFF { return Err("a stored block's length is damaged"); }
                for _ in 0..len { let byte = bits.take(8)? as u8; out.push(byte); }
            }
            kind @ (1 | 2) => {
                let (lit, dist) = if kind == 1 {
                    let mut lengths = [0u8; 288];
                    for (i, l) in lengths.iter_mut().enumerate() { *l = match i { 0..=143 => 8, 144..=255 => 9, 256..=279 => 7, _ => 8 }; }
                    (Huffman::new(&lengths)?, Huffman::new(&[5u8; 30])?)
                } else { dynamic(bits)? };
                loop {
                    let symbol = lit.decode(bits)? as usize;
                    if symbol < 256 { out.push(symbol as u8); continue; }
                    if symbol == 256 { break; }
                    let i = symbol - 257;
                    if i >= 29 { return Err("a bad length"); }
                    let length = LENGTH_BASE[i] as usize + bits.take(LENGTH_EXTRA[i] as u32)? as usize;
                    let d = dist.decode(bits)? as usize;
                    if d >= 30 { return Err("a bad distance"); }
                    let distance = DIST_BASE[d] as usize + bits.take(DIST_EXTRA[d] as u32)? as usize;
                    out.copy(distance, length)?;
                    if out.stopped { return Ok(false); }
                }
            }
            _ => return Err("a block of an unknown kind"),
        }
        if out.stopped { return Ok(false); }
        if last { break; }
    }
    out.flush();
    Ok(!out.stopped)
}

// A dynamic block's codes (RFC 1951 3.2.7).
fn dynamic(bits: &mut Bits) -> Result<(Huffman, Huffman), Error> {
    let (hlit, hdist, hclen) = (bits.take(5)? as usize + 257, bits.take(5)? as usize + 1, bits.take(4)? as usize + 4);
    let mut code_lengths = [0u8; 19];
    for &i in ORDER.iter().take(hclen) { code_lengths[i] = bits.take(3)? as u8; }
    let codes = Huffman::new(&code_lengths)?;
    let mut lengths = vec![0u8; hlit + hdist];
    let mut i = 0;
    while i < hlit + hdist {
        let symbol = codes.decode(bits)?;
        let (value, repeat) = match symbol {
            0..=15 => (symbol as u8, 1),
            16 => (*lengths.get(i.wrapping_sub(1)).ok_or("a repeat with nothing before it")?, 3 + bits.take(2)? as usize),
            17 => (0, 3 + bits.take(3)? as usize),
            _ => (0, 11 + bits.take(7)? as usize),
        };
        if i + repeat > lengths.len() { return Err("too many code lengths"); }
        for l in &mut lengths[i..i + repeat] { *l = value; }
        i += repeat;
    }
    Ok((Huffman::new(&lengths[..hlit])?, Huffman::new(&lengths[hlit..])?))
}

/// A zlib stream (a two-byte header, DEFLATE, the Adler-32 of the output), decompressed as `inflate` does; the
/// checksum is checked when the whole stream was read.
pub fn zlib(data: &[u8], sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), Error> { zlib_parts(&[data], sink) }

/// A zlib stream held in several pieces, one after another (a PNG's IDAT chunks), without joining them.
pub fn zlib_parts(parts: &[&[u8]], sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), Error> {
    let mut bits = Bits { parts, part: 0, at: 0, bit: 0, held: 0 };
    let header = (bits.take(8)? as u16) << 8 | bits.take(8)? as u16;
    if header >> 8 & 0x0F != 8 || header % 31 != 0 || header & 0x20 != 0 { return Err("not a zlib stream"); }
    let (mut a, mut b) = (1u32, 0u32);
    let whole = deflate(&mut bits, &mut |piece| {
        for &byte in piece { a = (a + byte as u32) % 65_521; b = (b + a) % 65_521; }
        sink(piece)
    })?;
    if whole {
        bits.align();
        let mut check = 0u32;
        for _ in 0..4 { check = check << 8 | bits.take(8).map_err(|_| "the checksum is missing")?; }
        if check != (b << 16 | a) { return Err("the checksum does not match"); }
    }
    Ok(())
}
