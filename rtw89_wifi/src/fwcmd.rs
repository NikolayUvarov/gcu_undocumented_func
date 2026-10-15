// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2020 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! RTL8852BE channel-12 command transport, adapted from pinned pci.c/pci.h and core.c/core.h/txrx.h.

use core::{marker::PhantomData, ptr::NonNull, sync::atomic::{fence, Ordering}};
use crate::io::{self, Fault, RegisterIo, TimedIo};

pub const DESCRIPTORS: usize = 32;
pub const RETAIN_COMPLETED: usize = 8;
pub const BUFFERS: usize = DESCRIPTORS + RETAIN_COMPLETED;
pub const TXWD_BYTES: usize = 24;
pub const MAX_PAYLOAD: usize = 2020;
pub const BUFFER_BYTES: usize = 2048;
pub const BUFFERS_AT: usize = 4096;
pub const DMA_BYTES: usize = BUFFERS_AT + BUFFERS * BUFFER_BYTES;

pub const INDEX: usize = 0x1080;
pub const NUMBER: usize = 0x1038;
pub const BASE_LOW: usize = 0x1160;
pub const BASE_HIGH: usize = 0x1164;
pub const BDRAM: usize = 0x1228;
pub const CLEAR_INDEX: usize = 0x1014;
pub const DMA_STOP: usize = 0x1010;
pub const DMA_BUSY: usize = 0x101c;
pub const INIT_CONFIG: usize = 0x1000;
const STOP_CHANNEL: u32 = 1 << 18;
const TX_HCI: u32 = 1 << 11;
const RX_HCI: u32 = 1 << 13;
const BUSY_MASK: u32 = 0x007f_0f03;
const BDRAM_VALUE: u32 = 28 | 4 << 8 | 1 << 16;
const INDEX_MASK: u32 = 0x0fff_0fff;

/// Configuration reads must refer to the same PCI function as the MMIO mapping.
pub trait Bus: RegisterIo { fn pci_command(&mut self) -> io::Result<u16>; }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error { Architecture, Region, State, Length, Full, Index, Count, Io(Fault) }
impl From<Fault> for Error { fn from(value: Fault) -> Self { Self::Io(value) } }
type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State { Allocated, Installed, Running, Faulted }

/// CPU ownership is separate from the hardware read index: eight completed payloads remain immutable.
pub struct Queue<'a> {
    base: NonNull<u8>, physical: u64, issued: u64, completed: u64, state: State,
    _exclusive: PhantomData<&'a mut [u8]>,
}

impl<'a> Queue<'a> {
    /// Initialize while PCI mastering is disabled and no earlier device transaction can access this region.
    ///
    /// # Safety
    /// `base` must name `len` exclusive, mapped coherent DMA bytes at `physical`, aligned to 4096, for this device.
    /// This implementation requires x86-64 coherent WB memory and strongly ordered UC MMIO, with no WC aliases.
    /// Release/Acquire fences constrain compiler ordering; x86 WB/UC ordering supplies the device-visible ordering.
    /// The allocation and mapping must outlive this object AND all device access, including eight retired payloads.
    /// No CPU alias may access it; only this queue and the device may do so until verified external DMA quiescence.
    /// A failure/drop does not quiesce hardware or release ownership; the caller retains the region on every fault.
    pub unsafe fn new(bus: &mut impl Bus, base: *mut u8, len: usize, physical: u64) -> Result<Self> {
        if !cfg!(target_arch = "x86_64") { return Err(Error::Architecture); }
        if bus.pci_command()? & 7 != 2 { return Err(Error::State); }
        let Some(base) = NonNull::new(base) else { return Err(Error::Region); };
        if len < DMA_BYTES || len > isize::MAX as usize || base.as_ptr() as usize & 4095 != 0 || physical & 4095 != 0 || physical == 0
            || physical.checked_add(DMA_BYTES as u64).is_none_or(|end| end > 1u64 << 32)
            || (base.as_ptr() as usize).checked_add(DMA_BYTES).is_none() {
            return Err(Error::Region);
        }
        for at in 0..DMA_BYTES { core::ptr::write_volatile(base.as_ptr().add(at), 0); }
        Ok(Self { base, physical, issued: 0, completed: 0, state: State::Allocated, _exclusive: PhantomData })
    }

    pub fn state(&self) -> State { self.state }
    pub fn pending(&self) -> usize { (self.issued - self.completed) as usize }
    pub fn retained(&self) -> usize { self.completed.min(RETAIN_COMPLETED as u64) as usize }
    pub fn physical(&self) -> u64 { self.physical }

    fn fail<T>(&mut self, error: Error) -> Result<T> { self.state = State::Faulted; Err(error) }
    fn observe<T>(&mut self, result: Result<T>) -> Result<T> {
        match result { Ok(value) => Ok(value), Err(error) => self.fail(error) }
    }

    /// Install only channel 12 while mastering and both HCI engines are off; MAC/PCIe pre-initialization is separate.
    pub fn install(&mut self, bus: &mut impl Bus) -> Result<()> {
        if self.state != State::Allocated { return Err(Error::State); }
        let result = (|| {
            if bus.pci_command()? & 7 != 2 || io::read32(bus, INIT_CONFIG)? & (TX_HCI | RX_HCI) != 0
                || io::read32(bus, DMA_STOP)? & STOP_CHANNEL == 0 { return Err(Error::State); }
            let mut timed = TimedIo::new(bus, 10_000);
            io::poll32(&mut timed, DMA_BUSY, BUSY_MASK, 0, 10, 100)?;
            io::modify32(&mut timed, CLEAR_INDEX, 0, 1 << 10)?;
            io::poll32(&mut timed, INDEX, INDEX_MASK, 0, 10, 100)?;
            timed.write16(NUMBER, DESCRIPTORS as u16)?;
            timed.write32(BDRAM, BDRAM_VALUE)?;
            timed.write32(BASE_LOW, self.physical as u32)?;
            timed.write32(BASE_HIGH, 0)?;
            if timed.read16(NUMBER)? != DESCRIPTORS as u16 || io::read32(&mut timed, BDRAM)? != BDRAM_VALUE
                || io::read32(&mut timed, BASE_LOW)? != self.physical as u32 || io::read32(&mut timed, BASE_HIGH)? != 0 {
                return Err(Error::State);
            }
            Ok(())
        })();
        self.observe(result)?;
        self.state = State::Installed;
        Ok(())
    }

    /// Admit submissions after a separately reviewed MAC/PCIe startup; this function never enables bus mastering.
    ///
    /// # Safety
    /// The caller completed supported-cut DLE/HFC/PCIe initialization and enabled mastering only after `install`.
    /// Other DMA channels must be stopped or own valid rings; channel 12 uses the addresses installed by this queue.
    /// Its BDRAM/mode/count/indices must not have been reset or changed, and the device retains coherent DMA access.
    /// Firmware authentication, target-address/security policy and WCPU download sequencing are the caller's duties.
    pub unsafe fn start(&mut self, bus: &mut impl Bus) -> Result<()> {
        if self.state != State::Installed { return Err(Error::State); }
        let result = (|| {
            let command = bus.pci_command()?;
            if command == u16::MAX || command & 6 != 6 || io::read32(bus, INIT_CONFIG)? & TX_HCI == 0
                || io::read32(bus, DMA_STOP)? & STOP_CHANNEL != 0 || io::read32(bus, INDEX)? & INDEX_MASK != 0
                || io::read32(bus, BASE_LOW)? != self.physical as u32 || io::read32(bus, BASE_HIGH)? != 0
                || bus.read16(NUMBER)? != DESCRIPTORS as u16 || io::read32(bus, BDRAM)? != BDRAM_VALUE {
                return Err(Error::State);
            }
            Ok(())
        })();
        self.observe(result)?;
        self.state = State::Running;
        Ok(())
    }

    /// Reclaim descriptor positions; completed payloads retain the upstream eight-command prefetch margin.
    pub fn reclaim(&mut self, bus: &mut impl RegisterIo) -> Result<usize> {
        if self.state != State::Running { return Err(Error::State); }
        let result = (|| {
            let index = io::read32(bus, INDEX)?;
            let host = (index & 0xfff) as usize;
            let hardware = ((index >> 16) & 0xfff) as usize;
            if hardware >= DESCRIPTORS || host != (self.issued % DESCRIPTORS as u64) as usize { return Err(Error::Index); }
            let previous = (self.completed % DESCRIPTORS as u64) as usize;
            let count = (hardware + DESCRIPTORS - previous) % DESCRIPTORS;
            if count > self.pending() { return Err(Error::Index); }
            fence(Ordering::Acquire);
            self.completed = self.completed.checked_add(count as u64).ok_or(Error::Count)?;
            Ok(count)
        })();
        self.observe(result)
    }

    /// Copy one complete H2C packet or one raw firmware part, then publish its descriptor with the 16-bit doorbell.
    pub fn submit(&mut self, bus: &mut impl RegisterIo, payload: &[u8], firmware_part: bool) -> Result<()> {
        if self.state != State::Running { return Err(Error::State); }
        if payload.is_empty() || payload.len() > MAX_PAYLOAD { return Err(Error::Length); }
        self.reclaim(bus)?;
        let oldest_held = self.completed.saturating_sub(RETAIN_COMPLETED as u64);
        if self.pending() >= DESCRIPTORS - 1 || self.issued - oldest_held >= BUFFERS as u64 { return Err(Error::Full); }
        let Some(next) = self.issued.checked_add(1) else { return self.fail(Error::Count); };
        let buffer = BUFFERS_AT + (self.issued % BUFFERS as u64) as usize * BUFFER_BYTES;
        let descriptor = (self.issued % DESCRIPTORS as u64) as usize * 8;
        let length = TXWD_BYTES + payload.len();
        unsafe {
            for at in 0..TXWD_BYTES { core::ptr::write_volatile(self.base.as_ptr().add(buffer + at), 0); }
            self.word(buffer, 12 << 16 | if firmware_part { 1 << 20 } else { 0 });
            self.word(buffer + 8, payload.len() as u32);
            for (at, value) in payload.iter().copied().enumerate() {
                core::ptr::write_volatile(self.base.as_ptr().add(buffer + TXWD_BYTES + at), value);
            }
            self.word(descriptor, (length as u32) | 1 << 30);
            self.word(descriptor + 4, (self.physical + buffer as u64) as u32);
        }
        fence(Ordering::Release);
        self.issued = next;
        let result = bus.write16(INDEX, (self.issued % DESCRIPTORS as u64) as u16).map_err(Error::Io);
        self.observe(result)
    }

    /// A drained queue still retains its final eight payloads; only external quiescence permits releasing the DMA map.
    pub fn drain(&mut self, bus: &mut impl RegisterIo, timeout_us: u32) -> Result<()> {
        if self.state != State::Running { return Err(Error::State); }
        if timeout_us == 0 || timeout_us > 1_000_000 { return Err(Error::Length); }
        let mut timed = TimedIo::new(bus, timeout_us as u64);
        for _ in 0..=timeout_us / 50 + 1 {
            self.reclaim(&mut timed)?;
            if self.pending() == 0 { return Ok(()); }
            if let Err(error) = timed.delay_us(50) { return self.fail(Error::Io(error)); }
        }
        self.fail(Error::Io(Fault::new(io::FaultKind::Timeout, INDEX, self.pending() as u32)))
    }

    unsafe fn word(&mut self, at: usize, value: u32) { core::ptr::write_volatile(self.base.as_ptr().add(at).cast::<u32>(), value.to_le()); }
}
