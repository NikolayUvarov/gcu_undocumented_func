//! A frame: header (version, channel, kind, flags, sequence, payload length; little-endian), payload, CRC-32C of both;
//! COBS-encoded and followed by a zero byte on the line.
use crate::{cobs, crc};

pub const VERSION: u8 = 1;
pub const HEADER: usize = 8;
pub const CRC: usize = 4;
/// The largest payload a frame carries; slow lines agree on less in their hello.
pub const MAX_PAYLOAD: usize = 1024;
pub const MAX_FRAME: usize = HEADER + MAX_PAYLOAD + CRC;
/// The most bytes a frame takes on the line, its separating zero included.
pub const MAX_WIRE: usize = cobs::max_encoded(MAX_FRAME) + 1;

/// Where a frame goes and what it is. `seq` pairs a request with its answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header { pub channel: u8, pub kind: u8, pub flags: u8, pub seq: u16 }

/// `payload` framed into `out`, the separating zero last; the bytes to send, or None when it does not fit.
pub fn encode(h: &Header, payload: &[u8], out: &mut [u8]) -> Option<usize> {
    if payload.len() > MAX_PAYLOAD { return None; }
    let mut raw = [0u8; MAX_FRAME];
    raw[..HEADER].copy_from_slice(&[VERSION, h.channel, h.kind, h.flags, h.seq as u8, (h.seq >> 8) as u8, payload.len() as u8, (payload.len() >> 8) as u8]);
    raw[HEADER..HEADER + payload.len()].copy_from_slice(payload);
    let end = HEADER + payload.len();
    let crc = crc::crc32c(&raw[..end]);
    raw[end..end + CRC].copy_from_slice(&crc.to_le_bytes());
    let n = cobs::encode(&raw[..end + CRC], out)?;
    *out.get_mut(n)? = 0;
    Some(n + 1)
}

/// Frames gathered from the line's bytes. A frame that is malformed, too long or fails its CRC is counted and dropped;
/// the next zero byte starts afresh.
pub struct Decoder { wire: [u8; MAX_WIRE], len: usize, overflow: bool, raw: [u8; MAX_FRAME], payload_len: usize, pub dropped: u32 }

impl Default for Decoder { fn default() -> Self { Self::new() } }

impl Decoder {
    pub const fn new() -> Self { Self { wire: [0; MAX_WIRE], len: 0, overflow: false, raw: [0; MAX_FRAME], payload_len: 0, dropped: 0 } }

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
        let payload_len = u16::from_le_bytes([r[6], r[7]]) as usize;
        if r[0] != VERSION || payload_len > MAX_PAYLOAD || n != HEADER + payload_len + CRC { return None; }
        let end = HEADER + payload_len;
        if crc::crc32c(&r[..end]).to_le_bytes() != r[end..end + CRC] { return None; }
        self.payload_len = payload_len;
        Some(Header { channel: r[1], kind: r[2], flags: r[3], seq: u16::from_le_bytes([r[4], r[5]]) })
    }

    /// The payload of the frame `push` last returned.
    pub fn payload(&self) -> &[u8] { &self.raw[HEADER..HEADER + self.payload_len] }
}
