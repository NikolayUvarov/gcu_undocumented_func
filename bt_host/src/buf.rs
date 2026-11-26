//! Fixed-size queues of byte records and of log lines: the stack allocates nothing.

/// Records in a ring of `N` bytes, each kept as two length bytes and the record; full when the next does not fit.
pub struct Queue<const N: usize> { data: [u8; N], start: usize, end: usize, count: usize, pub dropped: u32 }

impl<const N: usize> Default for Queue<N> { fn default() -> Self { Self::new() } }

impl<const N: usize> Queue<N> {
    pub const fn new() -> Self { Self { data: [0; N], start: 0, end: 0, count: 0, dropped: 0 } }
    pub fn len(&self) -> usize { self.count }
    pub fn is_empty(&self) -> bool { self.count == 0 }
    /// Bytes a new record may still take.
    pub fn room(&self) -> usize { (N - (self.end - self.start)).saturating_sub(2) }

    /// Appends the parts as one record; false (and counted) when it does not fit.
    pub fn push(&mut self, parts: &[&[u8]]) -> bool {
        let len: usize = parts.iter().map(|p| p.len()).sum();
        if len > u16::MAX as usize || 2 + len > N - (self.end - self.start) { self.dropped += 1; return false; }
        if self.end + 2 + len > N { self.data.copy_within(self.start..self.end, 0); self.end -= self.start; self.start = 0; }
        self.data[self.end..self.end + 2].copy_from_slice(&(len as u16).to_le_bytes());
        let mut at = self.end + 2;
        for p in parts { self.data[at..at + p.len()].copy_from_slice(p); at += p.len(); }
        self.end = at;
        self.count += 1;
        true
    }

    /// The oldest record.
    pub fn front(&self) -> Option<&[u8]> {
        if self.count == 0 { return None; }
        let len = u16::from_le_bytes([self.data[self.start], self.data[self.start + 1]]) as usize;
        Some(&self.data[self.start + 2..self.start + 2 + len])
    }

    pub fn pop(&mut self) {
        if self.count == 0 { return; }
        let len = u16::from_le_bytes([self.data[self.start], self.data[self.start + 1]]) as usize;
        self.start += 2 + len;
        self.count -= 1;
        if self.count == 0 { (self.start, self.end) = (0, 0); }
    }

    /// The oldest record copied into `out`, and taken; its length.
    pub fn take(&mut self, out: &mut [u8]) -> Option<usize> {
        let n = { let r = self.front()?; let n = r.len().min(out.len()); out[..n].copy_from_slice(&r[..n]); n };
        self.pop();
        Some(n)
    }

    pub fn clear(&mut self) { (self.start, self.end, self.count) = (0, 0, 0); }
}

/// A line being formatted, cut at its capacity.
pub struct Line { pub bytes: [u8; LINE], pub len: usize }
pub const LINE: usize = 240;

impl Default for Line { fn default() -> Self { Self { bytes: [0; LINE], len: 0 } } }

impl core::fmt::Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let n = s.len().min(LINE - self.len);
        self.bytes[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

/// The log's lines, for the driver to print; the oldest are kept when it overflows.
pub struct Log { pub lines: Queue<8192> }

impl Default for Log { fn default() -> Self { Self::new() } }

impl Log {
    pub const fn new() -> Self { Self { lines: Queue::new() } }
    pub fn say(&mut self, args: core::fmt::Arguments) {
        let mut line = Line::default();
        let _ = core::fmt::write(&mut line, args);
        self.lines.push(&[&line.bytes[..line.len]]);
    }
    /// The next line, copied out.
    pub fn take<'a>(&mut self, out: &'a mut [u8; LINE]) -> Option<&'a str> {
        let n = self.lines.take(out)?;
        Some(core::str::from_utf8(&out[..n]).unwrap_or("?"))
    }
}

/// A device address as HCI carries it (least significant byte first), shown most significant first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Addr(pub [u8; 6]);

impl Addr {
    pub fn from_wire(b: &[u8]) -> Self { let mut a = [0; 6]; a.copy_from_slice(&b[..6]); Self(a) }
    /// From the text form "AA:BB:CC:DD:EE:FF".
    pub fn parse(text: &str) -> Option<Self> {
        let mut a = [0u8; 6];
        let mut parts = text.split(':');
        for i in (0..6).rev() { a[i] = u8::from_str_radix(parts.next()?, 16).ok()?; }
        parts.next().is_none().then_some(Self(a))
    }
}

impl core::fmt::Display for Addr {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let a = self.0;
        write!(f, "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", a[5], a[4], a[3], a[2], a[1], a[0])
    }
}

/// Bytes as hex, for keys in the keys file and the log.
pub struct Hex<'a>(pub &'a [u8]);

impl core::fmt::Display for Hex<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { for b in self.0 { write!(f, "{:02X}", b)?; } Ok(()) }
}

/// Hex text into `out`; false when it is not exactly that many bytes.
pub fn unhex(text: &str, out: &mut [u8]) -> bool {
    let t = text.as_bytes();
    if t.len() != out.len() * 2 { return false; }
    for (i, o) in out.iter_mut().enumerate() {
        let Ok(s) = core::str::from_utf8(&t[2 * i..2 * i + 2]) else { return false };
        let Ok(v) = u8::from_str_radix(s, 16) else { return false };
        *o = v;
    }
    true
}

/// Little-endian reads that give 0 past the end, for parsers that check lengths first.
pub fn le16(b: &[u8], at: usize) -> u16 { u16::from_le_bytes([b.get(at).copied().unwrap_or(0), b.get(at + 1).copied().unwrap_or(0)]) }
pub fn le32(b: &[u8], at: usize) -> u32 { (le16(b, at) as u32) | (le16(b, at + 2) as u32) << 16 }
pub fn be16(b: &[u8], at: usize) -> u16 { u16::from_be_bytes([b.get(at).copied().unwrap_or(0), b.get(at + 1).copied().unwrap_or(0)]) }
pub fn be32(b: &[u8], at: usize) -> u32 { (be16(b, at) as u32) << 16 | be16(b, at + 2) as u32 }
