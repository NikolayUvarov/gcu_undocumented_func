//! The machines' consoles (800-LNK-0039): what channel 1 printed and the cable's bytes outside frames, in a ring with
//! absolute offsets, each piece with the time it came; input with sequence numbers, a number already taken not written
//! again (MC-7.5). docs/remote-serial.md, section 4.3, is the session an agent builds on it.
use linkproto::frame::{Decoder, MAX_RUN};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

/// The bytes kept for each machine; the oldest go first.
pub const KEPT: usize = 1 << 20;
/// The most one input call carries, and the most waiting to be sent.
pub const INPUT_MAX: usize = 4096;
const PENDING_MAX: usize = 4 * INPUT_MAX;

/// Milliseconds since the epoch, the host's clock.
pub fn now_ms() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }

/// `first`: the offset of the first byte kept; `pieces`: where each piece began and when it came; `seq`: the highest
/// input number taken; `pending`: input not sent yet; `offered`: the machine answers channel 1 (until it refuses).
pub struct Console { first: u64, data: VecDeque<u8>, pieces: VecDeque<(u64, u64)>, seq: Option<u64>, pub pending: VecDeque<u8>, pub offered: bool }

/// A read: from where (the oldest kept, if the bytes asked for were dropped), how many were dropped, the bytes, the
/// pieces' offsets and times within them, and the offset after them.
pub struct Read { pub from: u64, pub gap: u64, pub data: Vec<u8>, pub pieces: Vec<(u64, u64)>, pub next: u64 }

impl Default for Console { fn default() -> Self { Self::new() } }

impl Console {
    pub fn new() -> Self { Console { first: 0, data: VecDeque::new(), pieces: VecDeque::new(), seq: None, pending: VecDeque::new(), offered: true } }

    pub fn next(&self) -> u64 { self.first + self.data.len() as u64 }

    pub fn push(&mut self, bytes: &[u8], time_ms: u64) {
        if bytes.is_empty() { return; }
        self.pieces.push_back((self.next(), time_ms));
        self.data.extend(bytes);
        let excess = self.data.len().saturating_sub(KEPT);
        if excess == 0 { return; }
        self.data.drain(..excess);
        self.first += excess as u64;
        // A piece cut at its start begins at the first byte kept.
        while self.pieces.len() > 1 && self.pieces[1].0 <= self.first { self.pieces.pop_front(); }
        if let Some(p) = self.pieces.front_mut() { p.0 = p.0.max(self.first); }
    }

    /// Up to `max` bytes from `from`.
    pub fn read(&self, from: u64, max: usize) -> Read {
        let start = from.clamp(self.first, self.next());
        let gap = self.first.saturating_sub(from);
        let at = (start - self.first) as usize;
        let data: Vec<u8> = self.data.range(at..(at + max).min(self.data.len())).copied().collect();
        let end = start + data.len() as u64;
        // The piece the read begins in, then those that begin inside it.
        let before = self.pieces.iter().rposition(|p| p.0 <= start);
        let pieces = self.pieces.iter().enumerate().filter(|&(i, p)| Some(i) == before || (p.0 > start && p.0 < end)).map(|(_, &(o, t))| (o.max(start), t)).collect();
        Read { from: start, gap, data, pieces, next: end }
    }

    /// Input numbered `seq`: queued and true, or false when that number was taken already.
    pub fn input(&mut self, seq: u64, bytes: &[u8]) -> Result<bool, String> {
        if bytes.is_empty() || bytes.len() > INPUT_MAX { return Err(format!("an input carries 1 to {INPUT_MAX} bytes")); }
        if self.seq.is_some_and(|last| seq <= last) { return Ok(false); }
        if self.pending.len() + bytes.len() > PENDING_MAX { return Err("the console's input waits to be sent: try again".into()); }
        self.seq = Some(seq);
        self.pending.extend(bytes);
        Ok(true)
    }

    /// The highest input number taken.
    pub fn seq(&self) -> Option<u64> { self.seq }
}

/// The cable's bytes that are no frame: a boot's lines before `link` answers, or a console beside it. Each run of bytes
/// between zeros that does not decode is one; so is a run longer than any frame, and one the line left idle.
pub struct Stray { decoder: Decoder, run: Vec<u8>, long: bool, pub found: Vec<(u64, Vec<u8>)> }

impl Default for Stray { fn default() -> Self { Self::new() } }

impl Stray {
    pub fn new() -> Self { Stray { decoder: Decoder::new(), run: Vec::new(), long: false, found: Vec::new() } }

    pub fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            let frame = self.decoder.push(b).is_some();
            if b != 0 {
                self.run.push(b);
                if self.run.len() > MAX_RUN { self.long = true; self.flush(); }
                continue;
            }
            if frame && !self.long { self.run.clear(); } else { self.flush(); }
            self.long = false;
        }
    }

    /// The run so far, when the line has gone quiet: no frame is written with a pause in it.
    pub fn idle(&mut self) {
        if self.run.is_empty() { return; }
        self.flush();
        self.decoder = Decoder::new();
        self.long = false;
    }

    fn flush(&mut self) { if !self.run.is_empty() { self.found.push((now_ms(), std::mem::take(&mut self.run))); } }
}

/// Base64 text to bytes; None when it is not base64.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c { b'A'..=b'Z' => Some(c - b'A'), b'a'..=b'z' => Some(c - b'a' + 26), b'0'..=b'9' => Some(c - b'0' + 52), b'+' => Some(62), b'/' => Some(63), _ => None };
    let text = text.trim().trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut bits, mut n) = (0u32, 0);
    for &c in text {
        bits = bits << 6 | value(c)? as u32;
        n += 6;
        if n >= 8 { n -= 8; out.push((bits >> n) as u8); }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_and_a_gap() {
        let mut c = Console::new();
        c.push(b"abc", 1);
        c.push(&vec![b'x'; KEPT], 2);
        assert_eq!(c.next(), 3 + KEPT as u64);
        let r = c.read(0, 4);
        assert_eq!((r.from, r.gap, &r.data[..], r.next), (3, 3, &b"xxxx"[..], 7));
        assert_eq!(r.pieces, vec![(3, 2)]);
        let r = c.read(c.next(), 10);
        assert!(r.data.is_empty() && r.gap == 0);
    }

    #[test]
    fn pieces_within_a_read() {
        let mut c = Console::new();
        c.push(b"one ", 10);
        c.push(b"two ", 20);
        c.push(b"three", 30);
        let r = c.read(2, 8);
        assert_eq!(&r.data[..], b"e two th");
        assert_eq!(r.pieces, vec![(2, 10), (4, 20), (8, 30)]);
    }

    #[test]
    fn an_input_sent_twice_is_written_once() {
        let mut c = Console::new();
        assert_eq!(c.input(1, b"ls\r"), Ok(true));
        assert_eq!(c.input(1, b"ls\r"), Ok(false));
        assert_eq!(c.input(0, b"x"), Ok(false));
        assert_eq!(c.input(2, b"pwd\r"), Ok(true));
        assert_eq!(c.pending.iter().copied().collect::<Vec<u8>>(), b"ls\rpwd\r");
        assert!(c.input(3, &[]).is_err());
    }

    #[test]
    fn stray_lines_and_frames() {
        let mut wire = [0u8; linkproto::frame::MAX_WIRE];
        let h = linkproto::frame::Header { address: 0, channel: 0, kind: 1, flags: 0, seq: 7 };
        let n = linkproto::frame::encode(&h, b"hi", &mut wire).unwrap();
        let mut s = Stray::new();
        s.feed(b"\0U-Boot 2025.10\r\n");
        s.feed(&wire[..n]);
        // The frame's leading zero ends the boot's line, which goes as text; the frame does not.
        assert_eq!(s.found.len(), 1);
        assert_eq!(s.found[0].1, b"U-Boot 2025.10\r\n");
        s.found.clear();
        s.feed(&wire[..n]);
        assert!(s.found.is_empty());
        s.feed(b"login: ");
        assert!(s.found.is_empty());
        s.idle();
        assert_eq!(s.found[0].1, b"login: ");
    }

    #[test]
    fn base64_both_ways() {
        let data = b"\x00\xffline\r\n";
        assert_eq!(base64_decode(&crate::ws::base64(data)).unwrap(), data);
        assert!(base64_decode("not*base64").is_none());
    }
}
