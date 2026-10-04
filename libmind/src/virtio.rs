//! VirtIO over PCI, modern interface (VirtIO 1.x): where the configuration structures are (vendor capabilities in
//! configuration space, read through `DEVICE_CONFIG`) and the common configuration registers. Used by init to grant a
//! driver exactly the BAR it needs, and by the drivers.
use crate::dev::{device_config, Mmio};

pub const COMMON: u8 = 1; pub const NOTIFY: u8 = 2; pub const ISR: u8 = 3; pub const DEVICE: u8 = 4;
// Common configuration registers (offsets in the COMMON structure).
pub const DEVICE_FEATURE_SELECT: usize = 0x00; pub const DEVICE_FEATURE: usize = 0x04; pub const DRIVER_FEATURE_SELECT: usize = 0x08;
pub const DRIVER_FEATURE: usize = 0x0C; pub const MSIX_CONFIG: usize = 0x10; pub const DEVICE_STATUS: usize = 0x14; pub const QUEUE_SELECT: usize = 0x16;
pub const QUEUE_SIZE: usize = 0x18; pub const QUEUE_MSIX_VECTOR: usize = 0x1A; pub const QUEUE_ENABLE: usize = 0x1C; pub const QUEUE_NOTIFY_OFF: usize = 0x1E;
pub const QUEUE_DESC: usize = 0x20; pub const QUEUE_DRIVER: usize = 0x28; pub const QUEUE_DEVICE: usize = 0x30;
pub const STATUS_ACK: u8 = 1; pub const STATUS_DRIVER: u8 = 2; pub const STATUS_DRIVER_OK: u8 = 4; pub const STATUS_FEATURES_OK: u8 = 8; pub const STATUS_FAILED: u8 = 128;
pub const F_VERSION_1: u64 = 1 << 32;
pub const NO_VECTOR: u16 = 0xFFFF;

/// One configuration structure: its BAR, offset and length (and for NOTIFY the queue offset multiplier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Region { pub bar: u8, pub offset: u32, pub length: u32, pub multiplier: u32 }

/// The common, notification, ISR and device-specific structures of a modern device; None for a legacy-only device.
#[derive(Clone, Copy, Debug, Default)]
pub struct Layout { pub common: Region, pub notify: Region, pub isr: Region, pub device: Region }

impl Layout {
    /// Reads the vendor capabilities through `bar_slot`, any BAR capability of the device.
    pub fn read(bar_slot: usize) -> Option<Self> {
        let config = |offset: usize| device_config(bar_slot, offset).ok();
        if config(0x04)? >> 16 & 0x10 == 0 { return None; }
        let (mut layout, mut found, mut at) = (Self::default(), 0u8, (config(0x34)? & 0xFC) as usize);
        for _ in 0..48 {
            if at < 0x40 { break; }
            let header = config(at)?;
            if header & 0xFF == 0x09 {
                let kind = (header >> 24) as u8;
                let region = Region { bar: config(at + 4)? as u8, offset: config(at + 8)?, length: config(at + 12)?, multiplier: if kind == NOTIFY { config(at + 16)? } else { 0 } };
                let target = match kind { COMMON => Some(&mut layout.common), NOTIFY => Some(&mut layout.notify), ISR => Some(&mut layout.isr), DEVICE => Some(&mut layout.device), _ => None };
                if let Some(target) = target { if found & 1 << kind == 0 { *target = region; found |= 1 << kind; } }
            }
            at = (header >> 8 & 0xFC) as usize;
        }
        (found & 0b11110 == 0b11110).then_some(layout)
    }
    /// The BAR holding all four structures, if they share one (the layout the drivers support).
    pub fn single_bar(&self) -> Option<u8> {
        let bar = self.common.bar;
        (bar < 6 && [self.notify.bar, self.isr.bar, self.device.bar].iter().all(|&b| b == bar)).then_some(bar)
    }
}

/// The modern transport over the mapped BAR.
pub struct Modern { pub bar: Mmio, pub layout: Layout }

impl Modern {
    pub fn common8(&self, register: usize) -> u8 { self.bar.read8(self.layout.common.offset as usize + register) }
    pub fn common16(&self, register: usize) -> u16 { self.bar.read16(self.layout.common.offset as usize + register) }
    pub fn common32(&self, register: usize) -> u32 { self.bar.read32(self.layout.common.offset as usize + register) }
    pub fn set8(&self, register: usize, value: u8) { self.bar.write8(self.layout.common.offset as usize + register, value) }
    pub fn set16(&self, register: usize, value: u16) { self.bar.write16(self.layout.common.offset as usize + register, value) }
    pub fn set32(&self, register: usize, value: u32) { self.bar.write32(self.layout.common.offset as usize + register, value) }
    pub fn set64(&self, register: usize, value: u64) { self.bar.write64(self.layout.common.offset as usize + register, value) }
    pub fn device8(&self, offset: usize) -> u8 { self.bar.read8(self.layout.device.offset as usize + offset) }
    pub fn device16(&self, offset: usize) -> u16 { self.bar.read16(self.layout.device.offset as usize + offset) }
    pub fn isr(&self) -> u8 { self.bar.read8(self.layout.isr.offset as usize) }

    /// Resets the device and negotiates `wanted` features plus VERSION_1; None if the device refuses them.
    pub fn negotiate(&self, wanted: u64) -> Option<u64> {
        self.set8(DEVICE_STATUS, 0);
        for _ in 0..1000 { if self.common8(DEVICE_STATUS) == 0 { break; } core::hint::spin_loop(); }
        self.set8(DEVICE_STATUS, STATUS_ACK); self.set8(DEVICE_STATUS, STATUS_ACK | STATUS_DRIVER);
        self.set32(DEVICE_FEATURE_SELECT, 0); let low = self.common32(DEVICE_FEATURE) as u64;
        self.set32(DEVICE_FEATURE_SELECT, 1); let high = self.common32(DEVICE_FEATURE) as u64;
        let features = (low | high << 32) & (wanted | F_VERSION_1);
        if features & F_VERSION_1 == 0 { self.set8(DEVICE_STATUS, STATUS_FAILED); return None; }
        self.set32(DRIVER_FEATURE_SELECT, 0); self.set32(DRIVER_FEATURE, features as u32);
        self.set32(DRIVER_FEATURE_SELECT, 1); self.set32(DRIVER_FEATURE, (features >> 32) as u32);
        self.set8(DEVICE_STATUS, STATUS_ACK | STATUS_DRIVER | STATUS_FEATURES_OK);
        if self.common8(DEVICE_STATUS) & STATUS_FEATURES_OK == 0 { self.set8(DEVICE_STATUS, STATUS_FAILED); return None; }
        Some(features)
    }

    /// Sets up queue `index` with up to `max` entries at the given physical addresses and MSI-X entry `vector`
    /// (NO_VECTOR: none). Returns the queue size and its notification offset, or None if the device refuses.
    pub fn queue(&self, index: u16, max: u16, desc: u64, driver: u64, device: u64, vector: u16) -> Option<(u16, usize)> {
        self.set16(QUEUE_SELECT, index);
        let size = self.common16(QUEUE_SIZE).min(max);
        if size == 0 { return None; }
        self.set16(QUEUE_SIZE, size);
        self.set16(QUEUE_MSIX_VECTOR, vector);
        if self.common16(QUEUE_MSIX_VECTOR) != vector { return None; }
        self.set64(QUEUE_DESC, desc); self.set64(QUEUE_DRIVER, driver); self.set64(QUEUE_DEVICE, device);
        let notify = self.layout.notify.offset as usize + self.common16(QUEUE_NOTIFY_OFF) as usize * self.layout.notify.multiplier as usize;
        self.set16(QUEUE_ENABLE, 1);
        Some((size, notify))
    }
    pub fn notify(&self, at: usize, queue: u16) { self.bar.write16(at, queue) }
    pub fn ready(&self) { self.set8(DEVICE_STATUS, self.common8(DEVICE_STATUS) | STATUS_DRIVER_OK) }
}
