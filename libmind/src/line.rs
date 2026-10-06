//! A line of formatted output collected before it is written, so one line goes out in one write and lines of tasks
//! printing at once on several CPUs do not cut into each other on the console (issue 209).

/// Bytes of one write; a longer line goes out in pieces of this size.
pub const LINE_MAX: usize = 256;

/// Collects formatted text and hands it to `sink` a full line (or LINE_MAX bytes) at a time.
pub struct Line<F: FnMut(&[u8])> { bytes: [u8; LINE_MAX], len: usize, sink: F }

impl<F: FnMut(&[u8])> Line<F> {
    pub fn new(sink: F) -> Self { Self { bytes: [0; LINE_MAX], len: 0, sink } }
    pub fn push(&mut self, mut text: &[u8]) {
        while !text.is_empty() {
            let take = text.len().min(LINE_MAX - self.len);
            self.bytes[self.len..self.len + take].copy_from_slice(&text[..take]);
            self.len += take; text = &text[take..];
            if self.len == LINE_MAX { self.flush(); }
        }
    }
    pub fn flush(&mut self) { if self.len > 0 { (self.sink)(&self.bytes[..self.len]); self.len = 0; } }
}

impl<F: FnMut(&[u8])> core::fmt::Write for Line<F> {
    fn write_str(&mut self, text: &str) -> core::fmt::Result { self.push(text.as_bytes()); Ok(()) }
}

impl<F: FnMut(&[u8])> Drop for Line<F> {
    fn drop(&mut self) { self.flush(); }
}
