/// Decimal formatting of a number without allocation.
pub struct Decimal { bytes: [u8; 20], start: usize }

impl Decimal {
    pub fn new(mut value: usize) -> Self {
        let mut bytes = [0u8; 20]; let mut start = bytes.len();
        loop { start -= 1; bytes[start] = b'0' + (value % 10) as u8; value /= 10; if value == 0 { break; } }
        Self { bytes, start }
    }
    pub fn as_bytes(&self) -> &[u8] { &self.bytes[self.start..] }
}

/// Fixed-size buffer implementing `core::fmt::Write` (excess is discarded).
pub struct FixedBuf<const N: usize> { bytes: [u8; N], len: usize }

impl<const N: usize> FixedBuf<N> {
    pub const fn new() -> Self { Self { bytes: [0; N], len: 0 } }
    pub fn as_bytes(&self) -> &[u8] { &self.bytes[..self.len] }
    pub fn clear(&mut self) { self.len = 0; }
}
impl<const N: usize> Default for FixedBuf<N> { fn default() -> Self { Self::new() } }
impl<const N: usize> core::fmt::Write for FixedBuf<N> {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        let take = text.len().min(N - self.len);
        self.bytes[self.len..self.len + take].copy_from_slice(&text.as_bytes()[..take]); self.len += take; Ok(())
    }
}
