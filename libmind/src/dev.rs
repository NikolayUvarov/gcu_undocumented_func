//! Ring 3 drivers access devices strictly through capabilities: ports, IRQ lines, input, framebuffer.
use crate::abi::*;
use crate::ipc::Endpoint;
use crate::sys::{call, check, syscall, Result};

/// What a capability slot holds: (CAP_KIND_* kind, base, size).
pub fn cap_info(slot: usize) -> (usize, usize, usize) { let raw = syscall(SYSCALL_CAP_INFO, slot, 0, [0; 4]); (raw.result, raw.arg2, raw.msg[2]) }

/// I/O port range in a capability slot (port numbers are absolute).
#[derive(Clone, Copy)]
pub struct Ports(pub usize);

impl Ports {
    /// Base and port count of the granted range (for PCI devices with a BAR).
    pub fn range(&self) -> Option<(u16, u16)> {
        match cap_info(self.0) { (CAP_KIND_PORTS, base, count) => Some((base as u16, count as u16)), _ => None }
    }
    fn read(&self, port: u16, width: usize) -> usize { syscall(SYSCALL_PORT_IN, self.0, port as usize, [0, width, 0, 0]).result }
    fn write(&self, port: u16, width: usize, value: usize) { syscall(SYSCALL_PORT_OUT, self.0, port as usize, [value, width, 0, 0]); }
    pub fn in8(&self, port: u16) -> u8 { self.read(port, 1) as u8 }
    pub fn in16(&self, port: u16) -> u16 { self.read(port, 2) as u16 }
    pub fn in32(&self, port: u16) -> u32 { self.read(port, 4) as u32 }
    pub fn out8(&self, port: u16, value: u8) { self.write(port, 1, value as usize) }
    pub fn out16(&self, port: u16, value: u16) { self.write(port, 2, value as usize) }
    pub fn out32(&self, port: u16, value: u32) { self.write(port, 4, value as usize) }
    /// Bulk read of 16-bit words (ATA data) directly into a buffer.
    pub fn read_words(&self, port: u16, buffer: &mut [u16]) -> Result<usize> {
        check(syscall(SYSCALL_PORT_IN_BLOCK, self.0, port as usize, [0, 0, buffer.as_mut_ptr() as usize, buffer.len()]).result)
    }
    /// Bulk write of 16-bit words (ATA data) from a buffer.
    pub fn write_words(&self, port: u16, buffer: &[u16]) -> Result<usize> {
        check(syscall(SYSCALL_PORT_OUT_BLOCK, self.0, port as usize, [0, 0, buffer.as_ptr() as usize, buffer.len()]).result)
    }
}

/// Interrupt line in a capability slot. After it fires, the kernel masks it until wait/ack.
#[derive(Clone, Copy)]
pub struct Irq(pub usize);

impl Irq {
    /// Unmasks the line and sleeps until the next interrupt.
    pub fn wait(&self) -> Result<()> { check(call(SYSCALL_IRQ_WAIT, self.0, 0)).map(drop) }
    /// Deliver interrupts as messages to an IPC endpoint (`irq` flag in `Received`).
    pub fn bind(&self, endpoint: Endpoint) -> Result<()> { check(call(SYSCALL_IRQ_BIND, self.0, endpoint.0)).map(drop) }
    /// Acknowledges handling and unmasks the line again.
    pub fn ack(&self) -> Result<()> { check(call(SYSCALL_IRQ_ACK, self.0, 0)).map(drop) }
}

/// Keyboard event from a driver holding the input capability.
pub fn input_event(app: u8, shell: u8, background: bool) -> Result<()> {
    check(syscall(SYSCALL_INPUT_EVENT, app as usize, shell as usize, [background as usize, 0, 0, 0]).result).map(drop)
}

/// Decoded key event (see `mind::input::KeyEvent`) for the focused application and for the focus owner; `attention`
/// returns the focus to the owner instead.
pub fn input_key(app: usize, owner: usize, attention: bool) -> Result<()> {
    check(syscall(SYSCALL_INPUT_EVENT, 0, 0, [attention as usize, app, owner, 0]).result).map(drop)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Frame { Unchanged, Dirty, NewSource }

/// Compositor: active screen state; on NewSource the capability for the new screen is in `slot`.
pub fn compositor_pull(slot: usize) -> Result<Frame> {
    check(call(SYSCALL_COMPOSITOR_PULL, slot, 0)).map(|state| match state { 0 => Frame::Unchanged, 1 => Frame::Dirty, _ => Frame::NewSource })
}

/// A dword of the PCI configuration space of the function whose BAR capability is `slot` (read only, offset < 256).
pub fn device_config(slot: usize, offset: usize) -> Result<u32> { check(call(SYSCALL_DEVICE_CONFIG, slot, offset)).map(|v| v as u32) }

/// Device registers (MMIO), mapped uncached; accessed by offset.
pub struct Mmio { map: crate::mem::Mapping }

impl Mmio {
    pub fn map(slot: usize) -> Result<Self> { crate::mem::Mapping::new(slot).map(|map| Self { map }) }
    pub fn len(&self) -> usize { self.map.len() }
    pub fn is_empty(&self) -> bool { self.map.is_empty() }
    fn at<T>(&self, offset: usize) -> *mut T { (self.map.address() + offset) as *mut T }
    pub fn read8(&self, offset: usize) -> u8 { unsafe { core::ptr::read_volatile(self.at(offset)) } }
    pub fn read16(&self, offset: usize) -> u16 { unsafe { core::ptr::read_volatile(self.at(offset)) } }
    pub fn read32(&self, offset: usize) -> u32 { unsafe { core::ptr::read_volatile(self.at(offset)) } }
    pub fn write8(&self, offset: usize, value: u8) { unsafe { core::ptr::write_volatile(self.at(offset), value) } }
    pub fn write16(&self, offset: usize, value: u16) { unsafe { core::ptr::write_volatile(self.at(offset), value) } }
    pub fn write32(&self, offset: usize, value: u32) { unsafe { core::ptr::write_volatile(self.at(offset), value) } }
    /// 64-bit registers are written as two dwords: low, then high.
    pub fn write64(&self, offset: usize, value: u64) { self.write32(offset, value as u32); self.write32(offset + 4, (value >> 32) as u32); }
    pub fn read64(&self, offset: usize) -> u64 { self.read32(offset) as u64 | (self.read32(offset + 4) as u64) << 32 }
}

/// Driver DMA region: virtual address for the CPU and physical address for the device.
pub struct Dma { map: crate::mem::Mapping, physical: u64 }

impl Dma {
    pub fn map(slot: usize) -> Result<Self> {
        let physical = crate::mem::dma_physical(slot)? as u64;
        crate::mem::Mapping::new(slot).map(|map| Self { map, physical })
    }
    pub fn len(&self) -> usize { self.map.len() }
    pub fn is_empty(&self) -> bool { self.map.is_empty() }
    pub fn physical(&self, offset: usize) -> u64 { self.physical + offset as u64 }
    pub fn bytes(&mut self, offset: usize, len: usize) -> &mut [u8] { &mut self.map.as_mut_slice()[offset..offset + len] }
    pub fn zero(&mut self, offset: usize, len: usize) { for byte in self.bytes(offset, len) { unsafe { core::ptr::write_volatile(byte, 0) } } }
    pub fn read32(&self, offset: usize) -> u32 { unsafe { core::ptr::read_volatile((self.map.address() + offset) as *const u32) } }
    pub fn write32(&mut self, offset: usize, value: u32) { unsafe { core::ptr::write_volatile((self.map.address() + offset) as *mut u32, value) } }
    pub fn write64(&mut self, offset: usize, value: u64) { self.write32(offset, value as u32); self.write32(offset + 4, (value >> 32) as u32); }
}
