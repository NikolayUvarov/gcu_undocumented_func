#![allow(dead_code)]
// A ring of TEXT_QUEUE bytes: a task's text input (SPAWN_TEXT, 000-KRN-0094), and the console channel's both ways
// (100000-KRN-0222). Shared by the kernel and the host tests.
use crate::abi::TEXT_QUEUE;

/// Bytes queued, whether the input was closed, and whether its reader waits in TEXT_READ (only then does a write wake it).
pub struct Text { bytes: [u8; TEXT_QUEUE], head: usize, pub len: usize, pub closed: bool, pub waiting: bool }

impl Text {
    pub const fn new() -> Self { Self { bytes: [0; TEXT_QUEUE], head: 0, len: 0, closed: false, waiting: false } }
    /// Appends what fits of `data`; how much.
    pub fn push(&mut self, data: &[u8]) -> usize {
        let n = data.len().min(TEXT_QUEUE - self.len);
        for (i, &byte) in data[..n].iter().enumerate() { self.bytes[(self.head + self.len + i) % TEXT_QUEUE] = byte; }
        self.len += n; n
    }
    /// Takes up to `out.len()` bytes from the front; how many.
    pub fn pop(&mut self, out: &mut [u8]) -> usize {
        let n = out.len().min(self.len);
        for (i, byte) in out[..n].iter_mut().enumerate() { *byte = self.bytes[(self.head + i) % TEXT_QUEUE]; }
        self.head = (self.head + n) % TEXT_QUEUE; self.len -= n; n
    }
}
