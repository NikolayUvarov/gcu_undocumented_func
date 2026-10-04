//! The frame ring a network card driver shares with the network stack (idl/net.wit 1.2, issue 107): transmit and
//! receive rings of fixed slots in one memory region the stack lends, with head and tail counters at fixed offsets.
//! Each side writes only its own counters and the slots it owns, and checks everything it reads from the region: the
//! other side may be broken or hostile. No file here depends on the kernel: the host tests build it too.
use core::sync::atomic::{AtomicU32, Ordering};

pub const SLOTS: usize = 32;
pub const SLOT: usize = 2048;
pub const FRAME_MAX: usize = 1514;
pub const BYTES: usize = HEADER + 2 * SLOTS * SLOT;
const HEADER: usize = 4096;
const DATA: usize = 8; // u16 length, u16 checksum start, u16 checksum offset, padding
const TX_HEAD: usize = 0;
const TX_TAIL: usize = 64;
const RX_HEAD: usize = 128;
const RX_TAIL: usize = 192;

/// A frame read from the ring: its length and, for a sent frame, where a checksum is to be completed (0, 0: none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame { pub len: usize, pub start: u16, pub offset: u16 }

/// The ring at `base`; copying it copies the address only.
#[derive(Clone, Copy)]
pub struct Ring { base: *mut u8 }

// The two directions: where the producer's and the consumer's counters are and where the slots begin.
#[derive(Clone, Copy)]
struct Lane { head: usize, tail: usize, slots: usize }
const TRANSMIT: Lane = Lane { head: TX_HEAD, tail: TX_TAIL, slots: HEADER };
const RECEIVE: Lane = Lane { head: RX_HEAD, tail: RX_TAIL, slots: HEADER + SLOTS * SLOT };

impl Ring {
    /// # Safety
    /// `base` must map at least `BYTES` writable bytes, aligned to 64, for as long as the ring is used.
    pub unsafe fn new(base: *mut u8) -> Self { Self { base } }

    /// Sets every counter to 0 (the lender, before it lends the region).
    pub fn reset(&self) { for at in [TX_HEAD, TX_TAIL, RX_HEAD, RX_TAIL] { self.counter(at).store(0, Ordering::Release); } }

    fn counter(&self, at: usize) -> &AtomicU32 { unsafe { AtomicU32::from_ptr(self.base.add(at).cast::<u32>()) } }
    fn slot(&self, lane: Lane, n: u32) -> *mut u8 { unsafe { self.base.add(lane.slots + (n as usize % SLOTS) * SLOT) } }

    fn push(&self, lane: Lane, frame: &[u8], start: u16, offset: u16) -> bool {
        if frame.len() > FRAME_MAX { return false; }
        let head = self.counter(lane.head).load(Ordering::Relaxed);
        let tail = self.counter(lane.tail).load(Ordering::Acquire);
        if head.wrapping_sub(tail) as usize >= SLOTS { return false; } // full (or a tail from the future: wait)
        let slot = self.slot(lane, head);
        unsafe {
            for (i, value) in [frame.len() as u16, start, offset].into_iter().enumerate() { core::ptr::write_volatile(slot.add(2 * i).cast::<u16>(), value); }
            core::ptr::copy_nonoverlapping(frame.as_ptr(), slot.add(DATA), frame.len());
        }
        self.counter(lane.head).store(head.wrapping_add(1), Ordering::Release);
        true
    }

    // The oldest frame of `lane`, copied into `out`. A slot with an impossible length is skipped (`len` 0).
    fn pop(&self, lane: Lane, out: &mut [u8; FRAME_MAX]) -> Option<Frame> {
        let tail = self.counter(lane.tail).load(Ordering::Relaxed);
        let head = self.counter(lane.head).load(Ordering::Acquire);
        let queued = head.wrapping_sub(tail) as usize;
        if queued == 0 || queued > SLOTS { return None; } // empty, or a head nobody could have written: ignored
        let slot = self.slot(lane, tail);
        let [len, start, offset] = [0, 1, 2].map(|i| unsafe { core::ptr::read_volatile(slot.add(2 * i).cast::<u16>()) });
        let len = len as usize;
        let frame = if (14..=FRAME_MAX).contains(&len) {
            unsafe { core::ptr::copy_nonoverlapping(slot.add(DATA), out.as_mut_ptr(), len); }
            Frame { len, start, offset }
        } else { Frame { len: 0, start: 0, offset: 0 } };
        self.counter(lane.tail).store(tail.wrapping_add(1), Ordering::Release);
        Some(frame)
    }

    /// The stack queues a frame to send; false while the transmit ring is full.
    pub fn send(&self, frame: &[u8], start: u16, offset: u16) -> bool { self.push(TRANSMIT, frame, start, offset) }
    /// The driver takes the oldest frame to send.
    pub fn take_sent(&self, out: &mut [u8; FRAME_MAX]) -> Option<Frame> { self.pop(TRANSMIT, out) }
    /// Frames queued to send and not taken yet.
    pub fn sending(&self) -> bool { self.counter(TX_HEAD).load(Ordering::Acquire) != self.counter(TX_TAIL).load(Ordering::Acquire) }
    /// The driver queues a received frame; false while the receive ring is full.
    pub fn deliver(&self, frame: &[u8]) -> bool { self.push(RECEIVE, frame, 0, 0) }
    /// The stack takes the oldest received frame.
    pub fn receive(&self, out: &mut [u8; FRAME_MAX]) -> Option<Frame> { self.pop(RECEIVE, out) }
}
