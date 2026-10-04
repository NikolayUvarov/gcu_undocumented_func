//! LEGACY: the legacy (pre-1.0) VirtIO PCI interface: registers in I/O BAR0, queues at page frame numbers, the legacy
//! interrupt line acknowledged by reading the ISR. Only for cards without the modern interface; delete this file, the
//! `legacy` feature and the places marked `LEGACY:` to drop it (docs/legacy.md).
use crate::{Queue, F_MAC, F_STATUS, QUEUES_BYTES, RX_BUFFERS};
use mind::abi::SLOT_DEV0;
use mind::dev::{Dma, Ports};

// Register offsets from BAR0; device-specific configuration follows at 0x14 (no MSI-X).
const FEATURES: u16 = 0x00; const GUEST_FEATURES: u16 = 0x04; const QUEUE_PFN: u16 = 0x08; const QUEUE_SIZE: u16 = 0x0C;
const QUEUE_SELECT: u16 = 0x0E; const QUEUE_NOTIFY: u16 = 0x10; const STATUS: u16 = 0x12; const ISR: u16 = 0x13;
const CONFIG_MAC: u16 = 0x14; const CONFIG_STATUS: u16 = 0x1A;
const STATUS_ACK: u8 = 1; const STATUS_DRIVER: u8 = 2; const STATUS_DRIVER_OK: u8 = 4; const STATUS_FAILED: u8 = 128;
/// struct virtio_net_hdr without mergeable buffers.
pub const HEADER: usize = 10;

pub struct Legacy { ports: Ports, io: u16 }

impl Legacy {
    /// Reset, features, both queues at their page frame numbers (the size is the device's); returns the queues, the
    /// MAC address and the negotiated features.
    pub fn setup(dma: &Dma) -> Option<(Self, [Queue; 2], u64, u32)> {
        let ports = Ports(SLOT_DEV0);
        let (io, count) = ports.range()?;
        if count < 0x20 { return None; }
        ports.out8(io + STATUS, 0);
        ports.out8(io + STATUS, STATUS_ACK);
        ports.out8(io + STATUS, STATUS_ACK | STATUS_DRIVER);
        let features = ports.in32(io + FEATURES) & (F_MAC | F_STATUS);
        ports.out32(io + GUEST_FEATURES, features);
        let mut queues = [Queue::new(0, 0), Queue::new(0, 0)];
        let mut base = 0;
        for (index, queue) in queues.iter_mut().enumerate() {
            ports.out16(io + QUEUE_SELECT, index as u16);
            let size = ports.in16(io + QUEUE_SIZE) as usize;
            if size < RX_BUFFERS || base + Queue::bytes(size) > QUEUES_BYTES { ports.out8(io + STATUS, STATUS_FAILED); return None; }
            *queue = Queue::new(size, base);
            ports.out32(io + QUEUE_PFN, (dma.physical(base) >> 12) as u32);
            base += Queue::bytes(size);
        }
        let mac = if features & F_MAC != 0 { (0..6).fold(0u64, |mac, i| mac << 8 | ports.in8(io + CONFIG_MAC + i) as u64) } else { 0 };
        Some((Self { ports, io }, queues, mac, features))
    }
    pub fn ready(&self) { self.ports.out8(self.io + STATUS, STATUS_ACK | STATUS_DRIVER | STATUS_DRIVER_OK) }
    pub fn notify(&self, queue: usize) { self.ports.out16(self.io + QUEUE_NOTIFY, queue as u16) }
    /// Reading the ISR acknowledges the interrupt.
    pub fn acknowledge(&self) { let _ = self.ports.in8(self.io + ISR); }
    pub fn link(&self) -> bool { self.ports.in16(self.io + CONFIG_STATUS) & 1 != 0 }
}
