//! A bulk IN stream's spill (158-DRV-0082): the transfers usb_host took off the ring before its client asked for them,
//! kept in order as the client gets them (two length bytes, bit 15 for an error, then the bytes), up to a bound. With it
//! the ring's TDs go back to the controller at usb_host's own pace (every STREAM_MS), not the client's: a camera's
//! burst of a frame no longer waits on the gateway once its 32 pages are full. No system calls: tests/usb_spill_host.rs.
use alloc::collections::VecDeque;

pub struct Spill { bytes: VecDeque<u8>, limit: usize, transfers: usize, pub most: usize, pub refused: u64 }

impl Spill {
    /// A spill of `limit` bytes at most, its two length bytes a transfer included.
    pub fn new(limit: usize) -> Self { Self { bytes: VecDeque::new(), limit, transfers: 0, most: 0, refused: 0 } }
    pub fn is_empty(&self) -> bool { self.bytes.is_empty() }
    /// The bytes it holds, and the transfers.
    pub fn len(&self) -> (usize, usize) { (self.bytes.len(), self.transfers) }

    /// One transfer kept (at most 32 KiB less one: its length takes 15 bits). False, and nothing kept, when it would
    /// pass the bound or the memory is not there: the transfer then waits on its TD, as without a spill.
    pub fn push(&mut self, data: &[u8], damaged: bool) -> bool {
        let need = 2 + data.len();
        if data.len() > 0x7FFF || self.bytes.len() + need > self.limit || self.bytes.try_reserve(need).is_err() { self.refused += 1; return false; }
        let head = (data.len() as u16 | if damaged { 0x8000 } else { 0 }).to_le_bytes();
        self.bytes.extend(head.iter());
        self.bytes.extend(data.iter());
        self.transfers += 1;
        self.most = self.most.max(self.bytes.len());
        true
    }

    /// As many whole transfers as fit in `out`, oldest first; the bytes written.
    pub fn take(&mut self, out: &mut [u8]) -> usize {
        let mut written = 0;
        while self.bytes.len() >= 2 {
            let head = u16::from_le_bytes([self.bytes[0], self.bytes[1]]);
            let n = 2 + (head & 0x7FFF) as usize;
            if written + n > out.len() { break; }
            let (a, b) = self.bytes.as_slices();
            let k = a.len().min(n);
            out[written..written + k].copy_from_slice(&a[..k]);
            out[written + k..written + n].copy_from_slice(&b[..n - k]);
            self.bytes.drain(..n);
            self.transfers -= 1;
            written += n;
        }
        written
    }
}
