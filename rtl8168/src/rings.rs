use core::ptr::{self, NonNull};
use core::sync::atomic::{fence, Ordering};

#[cfg(not(target_arch = "x86_64"))]
compile_error!("RTL8168 DMA rings currently require coherent x86_64 DMA memory");

// One frame per descriptor, no VLAN insertion/stripping or checksum/segmentation offload.
pub const SLOTS: usize = 256;
pub const DESCRIPTOR_BYTES: usize = 16;
pub const ALIGNMENT: usize = 256;
pub const BUFFER_BYTES: usize = 2048;
pub const MIN_FRAME: usize = 14;
pub const MAX_FRAME: usize = 1514;
pub const TX_RING_OFFSET: usize = 0;
pub const RX_RING_OFFSET: usize = SLOTS * DESCRIPTOR_BYTES;
pub const TX_BUFFERS_OFFSET: usize = RX_RING_OFFSET + SLOTS * DESCRIPTOR_BYTES;
pub const RX_BUFFERS_OFFSET: usize = TX_BUFFERS_OFFSET + SLOTS * BUFFER_BYTES;
pub const DMA_BYTES: usize = RX_BUFFERS_OFFSET + SLOTS * BUFFER_BYTES;

const OWN: u32 = 1 << 31;
const END: u32 = 1 << 30;
const FIRST: u32 = 1 << 29;
const LAST: u32 = 1 << 28;
const RX_ERRORS: u32 = (1 << 22) | (1 << 21) | (1 << 20) | (1 << 19);
const RX_LENGTH: u32 = 0x3fff;
const FCS_BYTES: usize = 4;
const WIRE_MIN_FRAME: usize = 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegionError {
    Null,
    Alignment,
    TooSmall,
    AddressRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendError {
    FrameLength,
    Full,
    DeviceOwned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropReason {
    Hardware,
    Fragment,
    Length,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Received {
    Empty,
    Frame(usize),
    Dropped { reason: DropReason, status: u32 },
    OutputTooSmall { needed: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Completion {
    pub slot: usize,
    pub status: u32,
}

// Memory is borrowed from the platform; dropping Rings neither stops DMA nor releases memory.
pub struct Rings {
    base: NonNull<u8>,
    physical: u64,
    tx_next: usize,
    tx_done: usize,
    tx_used: usize,
    rx_next: usize,
}

impl Rings {
    /// Initialize a stopped device's coherent descriptor and buffer region below 4 GiB.
    /// # Safety
    /// `base` must map at least `bytes` writable bytes at the contiguous DMA address `physical`.
    /// The mapping must be coherent, stable and exclusively used by this object and its NIC.
    /// No other CPU references or accesses to this region may exist while this object is used.
    /// The NIC must be stopped now, obey descriptor ownership later, and be stopped before reuse.
    /// Keep the mapping alive until DMA is quiesced, including after this object is dropped.
    /// Do not enable offloads, VLAN stripping or jumbo frames with these descriptors.
    pub unsafe fn initialize(
        base: *mut u8,
        bytes: usize,
        physical: u64,
    ) -> Result<Self, RegionError> {
        let base = NonNull::new(base).ok_or(RegionError::Null)?;
        if base.as_ptr() as usize % ALIGNMENT != 0 || physical % ALIGNMENT as u64 != 0 {
            return Err(RegionError::Alignment);
        }
        if bytes < DMA_BYTES {
            return Err(RegionError::TooSmall);
        }
        if (base.as_ptr() as usize).checked_add(DMA_BYTES).is_none()
            || physical == 0
            || physical
                .checked_add(DMA_BYTES as u64)
                .filter(|end| *end <= 1 << 32)
                .is_none()
        {
            return Err(RegionError::AddressRange);
        }
        let mut rings = Self {
            base,
            physical,
            tx_next: 0,
            tx_done: 0,
            tx_used: 0,
            rx_next: 0,
        };
        ptr::write_bytes(base.as_ptr(), 0, DMA_BYTES);
        for slot in 0..SLOTS {
            rings.address(TX_RING_OFFSET, slot, TX_BUFFERS_OFFSET);
            rings.write(TX_RING_OFFSET, slot, 0, end(slot));
            rings.rearm(slot);
        }
        Ok(rings)
    }

    pub fn tx_address(&self) -> u64 {
        self.physical + TX_RING_OFFSET as u64
    }

    pub fn rx_address(&self) -> u64 {
        self.physical + RX_RING_OFFSET as u64
    }

    pub fn tx_pending(&self) -> usize {
        self.tx_used
    }

    // Publish one frame; the caller must kick the NIC after this returns successfully.
    pub fn send(&mut self, frame: &[u8]) -> Result<usize, SendError> {
        if !(MIN_FRAME..=MAX_FRAME).contains(&frame.len()) {
            return Err(SendError::FrameLength);
        }
        if self.tx_used == SLOTS {
            return Err(SendError::Full);
        }
        let slot = self.tx_next;
        if self.read(TX_RING_OFFSET, slot, 0) & OWN != 0 {
            return Err(SendError::DeviceOwned);
        }
        dma_barrier();
        let length = frame.len().max(WIRE_MIN_FRAME);
        unsafe {
            let buffer = self
                .base
                .as_ptr()
                .add(TX_BUFFERS_OFFSET + slot * BUFFER_BYTES);
            ptr::copy_nonoverlapping(frame.as_ptr(), buffer, frame.len());
            ptr::write_bytes(buffer.add(frame.len()), 0, length - frame.len());
        }
        self.address(TX_RING_OFFSET, slot, TX_BUFFERS_OFFSET);
        self.write(TX_RING_OFFSET, slot, 1, 0);
        dma_barrier();
        self.write(
            TX_RING_OFFSET,
            slot,
            0,
            OWN | end(slot) | FIRST | LAST | length as u32,
        );
        dma_barrier();
        self.tx_next = (slot + 1) % SLOTS;
        self.tx_used += 1;
        Ok(slot)
    }

    // Reclaim at most one completed descriptor; completion is not evidence of peer delivery.
    pub fn reclaim(&mut self) -> Option<Completion> {
        if self.tx_used == 0 {
            return None;
        }
        let slot = self.tx_done;
        let status = self.read(TX_RING_OFFSET, slot, 0);
        if status & OWN != 0 {
            return None;
        }
        dma_barrier();
        self.write(TX_RING_OFFSET, slot, 0, end(slot));
        self.tx_done = (slot + 1) % SLOTS;
        self.tx_used -= 1;
        Some(Completion { slot, status })
    }

    // Inspect exactly one descriptor, including malformed input; the service budgets repeated calls.
    pub fn receive(&mut self, output: &mut [u8]) -> Received {
        let slot = self.rx_next;
        let status = self.read(RX_RING_OFFSET, slot, 0);
        if status & OWN != 0 {
            return Received::Empty;
        }
        dma_barrier();
        let wire_length = (status & RX_LENGTH) as usize;
        let drop = if status & RX_ERRORS != 0 {
            Some(DropReason::Hardware)
        } else if status & (FIRST | LAST) != (FIRST | LAST) {
            Some(DropReason::Fragment)
        } else if !(WIRE_MIN_FRAME + FCS_BYTES..=MAX_FRAME + FCS_BYTES).contains(&wire_length) {
            Some(DropReason::Length)
        } else {
            None
        };
        let result = if let Some(reason) = drop {
            Received::Dropped { reason, status }
        } else {
            let length = wire_length - FCS_BYTES;
            if output.len() < length {
                return Received::OutputTooSmall { needed: length };
            }
            unsafe {
                let buffer = self
                    .base
                    .as_ptr()
                    .add(RX_BUFFERS_OFFSET + slot * BUFFER_BYTES);
                ptr::copy_nonoverlapping(buffer, output.as_mut_ptr(), length);
            }
            Received::Frame(length)
        };
        self.rearm(slot);
        self.rx_next = (slot + 1) % SLOTS;
        result
    }

    fn rearm(&mut self, slot: usize) {
        self.address(RX_RING_OFFSET, slot, RX_BUFFERS_OFFSET);
        self.write(RX_RING_OFFSET, slot, 1, 0);
        dma_barrier();
        self.write(
            RX_RING_OFFSET,
            slot,
            0,
            OWN | end(slot) | BUFFER_BYTES as u32,
        );
        dma_barrier();
    }

    fn address(&mut self, ring: usize, slot: usize, buffers: usize) {
        let physical = self.physical + (buffers + slot * BUFFER_BYTES) as u64;
        self.write(ring, slot, 2, physical as u32);
        self.write(ring, slot, 3, (physical >> 32) as u32);
    }

    fn read(&self, ring: usize, slot: usize, word: usize) -> u32 {
        unsafe { u32::from_le(ptr::read_volatile(self.word(ring, slot, word))) }
    }

    fn write(&mut self, ring: usize, slot: usize, word: usize, value: u32) {
        unsafe { ptr::write_volatile(self.word(ring, slot, word), value.to_le()) }
    }

    fn word(&self, ring: usize, slot: usize, word: usize) -> *mut u32 {
        unsafe {
            self.base
                .as_ptr()
                .add(ring + slot * DESCRIPTOR_BYTES + word * 4)
                .cast()
        }
    }
}

fn end(slot: usize) -> u32 {
    if slot == SLOTS - 1 {
        END
    } else {
        0
    }
}

// A full compiler/hardware fence orders coherent RAM accesses around volatile ownership changes.
fn dma_barrier() {
    fence(Ordering::SeqCst);
}
