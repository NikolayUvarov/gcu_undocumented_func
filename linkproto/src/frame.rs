//! A frame: header (version, channel, kind, flags, sequence, payload length; little-endian), payload, CRC-32C of both;
//! COBS-encoded and followed by a zero byte on the line. Version 2 (800-LNK-0026) adds an address after the version, so
//! several slaves can share one line: a version 1 receiver drops a version 2 frame, so it stays silent on such a line.
use crate::{cobs, crc};

pub const VERSION: u8 = 1;
/// The version of a frame with an address.
pub const VERSION_ADDRESSED: u8 = 2;
pub const HEADER: usize = 8;
/// A version 2 header: the address byte more.
pub const HEADER_ADDRESSED: usize = HEADER + 1;
pub const CRC: usize = 4;
/// The largest payload a frame carries; slow lines agree on less in their hello.
pub const MAX_PAYLOAD: usize = 1024;
pub const MAX_FRAME: usize = HEADER_ADDRESSED + MAX_PAYLOAD + CRC;
/// The most bytes a frame takes on the line, its separating zero included.
pub const MAX_WIRE: usize = cobs::max_encoded(MAX_FRAME) + 1;
/// The addresses a slave may have on a shared line; 0 is none, a version 1 frame.
pub const ADDRESS_MAX: u8 = 247;

/// Where a frame goes and what it is. `seq` pairs a request with its answer. `address` is the slave's on a shared line,
/// 1 to ADDRESS_MAX, or 0 for a line with one slave (a version 1 frame).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header { pub address: u8, pub channel: u8, pub kind: u8, pub flags: u8, pub seq: u16 }

/// `payload` framed into `out`, the separating zero last; the bytes to send, or None when it does not fit.
pub fn encode(h: &Header, payload: &[u8], out: &mut [u8]) -> Option<usize> {
    if payload.len() > MAX_PAYLOAD || h.address > ADDRESS_MAX { return None; }
    let mut raw = [0u8; MAX_FRAME];
    let rest = [h.channel, h.kind, h.flags, h.seq as u8, (h.seq >> 8) as u8, payload.len() as u8, (payload.len() >> 8) as u8];
    let header = if h.address == 0 {
        raw[0] = VERSION;
        HEADER
    } else {
        raw[0] = VERSION_ADDRESSED;
        raw[1] = h.address;
        HEADER_ADDRESSED
    };
    raw[header - rest.len()..header].copy_from_slice(&rest);
    raw[header..header + payload.len()].copy_from_slice(payload);
    let end = header + payload.len();
    let crc = crc::crc32c(&raw[..end]);
    raw[end..end + CRC].copy_from_slice(&crc.to_le_bytes());
    let n = cobs::encode(&raw[..end + CRC], out)?;
    *out.get_mut(n)? = 0;
    Some(n + 1)
}

/// Frames gathered from the line's bytes. A frame that is malformed, too long or fails its CRC is counted and dropped;
/// the next zero byte starts afresh.
pub struct Decoder { wire: [u8; MAX_WIRE], len: usize, overflow: bool, raw: [u8; MAX_FRAME], header: usize, payload_len: usize, pub dropped: u32 }

impl Default for Decoder { fn default() -> Self { Self::new() } }

impl Decoder {
    pub const fn new() -> Self { Self { wire: [0; MAX_WIRE], len: 0, overflow: false, raw: [0; MAX_FRAME], header: HEADER, payload_len: 0, dropped: 0 } }

    /// One byte from the line; a frame's header when it completes one, its payload then in `payload`.
    pub fn push(&mut self, byte: u8) -> Option<Header> {
        if byte != 0 {
            if self.len < MAX_WIRE { self.wire[self.len] = byte; self.len += 1; } else { self.overflow = true; }
            return None;
        }
        let (len, overflow) = (self.len, self.overflow);
        self.len = 0;
        self.overflow = false;
        if len == 0 { return None; }
        let header = if overflow { None } else { self.check(len) };
        if header.is_none() { self.dropped += 1; }
        header
    }

    fn check(&mut self, len: usize) -> Option<Header> {
        let n = cobs::decode(&self.wire[..len], &mut self.raw)?;
        if n < HEADER + CRC { return None; }
        let r = &self.raw;
        let (header, address) = match r[0] {
            VERSION => (HEADER, 0),
            VERSION_ADDRESSED if n >= HEADER_ADDRESSED + CRC && (1..=ADDRESS_MAX).contains(&r[1]) => (HEADER_ADDRESSED, r[1]),
            _ => return None,
        };
        let h = &r[header - 7..header];
        let payload_len = u16::from_le_bytes([h[5], h[6]]) as usize;
        if payload_len > MAX_PAYLOAD || n != header + payload_len + CRC { return None; }
        let end = header + payload_len;
        if crc::crc32c(&r[..end]).to_le_bytes() != r[end..end + CRC] { return None; }
        (self.header, self.payload_len) = (header, payload_len);
        Some(Header { address, channel: h[0], kind: h[1], flags: h[2], seq: u16::from_le_bytes([h[3], h[4]]) })
    }

    /// The payload of the frame `push` last returned.
    pub fn payload(&self) -> &[u8] { &self.raw[self.header..self.header + self.payload_len] }
}
