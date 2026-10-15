// SPDX-License-Identifier: BSD-4-Clause
// RTL8168G register sequences adapted from FreeBSD re/rgephy at 16a79708; full notices: LICENSES/BSD-4-Clause-realtek.txt.
// This product includes software developed by Bill Paul.

use crate::identity::{Identity, PCI_ID};
use crate::rings::{Rings, ALIGNMENT, DMA_BYTES};

pub const REGISTER_BYTES: usize = 0xf4;
pub const STARTUP_MS: u64 = 1500;
const COMMAND: usize = 0x37;
const TX_POLL: usize = 0x38;
const IMR: usize = 0x3c;
const ISR: usize = 0x3e;
const TX_CONFIG: usize = 0x40;
const RX_CONFIG: usize = 0x44;
const CONFIG_LOCK: usize = 0x50;
const PHYAR: usize = 0x60;
const MEDIA: usize = 0x6c;
const MISC: usize = 0xf0;
const RXDV_GATE: u32 = 1 << 19;
const IRQ_MASK: u16 = 0x803f;

/// Register accesses must be volatile, ordered and restricted to this granted, powered device's MMIO window.
pub trait Registers {
    fn len(&self) -> usize;
    fn read8(&mut self, offset: usize) -> u8;
    fn read16(&mut self, offset: usize) -> u16;
    fn read32(&mut self, offset: usize) -> u32;
    fn write8(&mut self, offset: usize, value: u8);
    fn write16(&mut self, offset: usize, value: u16);
    fn write32(&mut self, offset: usize, value: u32);
    fn now_ms(&self) -> u64;
    fn sleep_ms(&mut self, milliseconds: u32);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { WrongDevice, UnknownXid(u16), Mmio, Mac, Phy(u32), Reset, Mdio, PhyReset, Budget, Dma, State, Gone }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State { Stopped, Prepared, Armed, Running, Failed }

/// Only the observed RTL8168G XID is selected; a timeout never authorizes releasing its DMA region.
pub struct Controller<R> { io: R, pub identity: Identity, pub mac: u64, pub phy: u32, state: State, started: u64 }

impl<R: Registers> Controller<R> {
    /// Reads identity and station address before any device write.
    pub fn new(mut io: R, pci_id: u32) -> Result<Self, Error> {
        if pci_id != PCI_ID { return Err(Error::WrongDevice); }
        if io.len() < REGISTER_BYTES { return Err(Error::Mmio); }
        let identity = Identity::decode(pci_id, io.read32(TX_CONFIG)).map_err(|_| Error::Gone)?;
        if identity.xid != 0x4c0 { return Err(Error::UnknownXid(identity.xid)); }
        let mut mac = 0u64;
        for offset in 0..6 { mac = (mac << 8) | io.read8(offset) as u64; }
        if mac == 0 || mac >> 40 & 1 != 0 { return Err(Error::Mac); }
        let started = io.now_ms();
        Ok(Self { io, identity, mac, phy: 0, state: State::Stopped, started })
    }

    pub fn state(&self) -> State { self.state }

    fn change8(&mut self, at: usize, clear: u8, set: u8) { let value = self.io.read8(at); self.io.write8(at, value & !clear | set); }
    fn change32(&mut self, at: usize, clear: u32, set: u32) { let value = self.io.read32(at); self.io.write32(at, value & !clear | set); }

    fn budget(&self) -> Result<(), Error> {
        if self.io.now_ms().saturating_sub(self.started) >= STARTUP_MS { Err(Error::Budget) } else { Ok(()) }
    }

    fn wait(&mut self, timeout: u64, error: Error, mut done: impl FnMut(&mut R) -> Result<bool, Error>) -> Result<(), Error> {
        let start = self.io.now_ms();
        for _ in 0..1000 {
            self.budget()?;
            if done(&mut self.io)? { return Ok(()); }
            if self.io.now_ms().saturating_sub(start) >= timeout { return Err(error); }
            self.io.sleep_ms(1);
        }
        Err(error)
    }

    fn mdio_write(&mut self, register: u8, value: u16) -> Result<(), Error> {
        self.budget()?;
        self.io.write32(PHYAR, 1 << 31 | (register as u32) << 16 | value as u32);
        self.wait(50, Error::Mdio, |io| {
            let value = io.read32(PHYAR);
            if value == u32::MAX { Err(Error::Gone) } else { Ok(value & (1 << 31) == 0) }
        })?;
        self.io.sleep_ms(1);
        self.budget()
    }

    fn mdio_read(&mut self, register: u8) -> Result<u16, Error> {
        self.budget()?;
        self.io.write32(PHYAR, (register as u32) << 16);
        let mut result = 0;
        self.wait(50, Error::Mdio, |io| {
            let value = io.read32(PHYAR);
            if value == u32::MAX { return Err(Error::Gone); }
            result = value as u16;
            Ok(value & (1 << 31) != 0)
        })?;
        self.io.sleep_ms(1);
        self.budget()?;
        Ok(result)
    }

    /// Resets the MAC, wakes the PHY and starts negotiation without waiting for a cable or a link partner.
    pub fn prepare(&mut self) -> Result<(), Error> {
        if self.state != State::Stopped { return Err(Error::State); }
        self.started = self.io.now_ms();
        let result = self.prepare_inner();
        self.state = if result.is_ok() { State::Prepared } else { State::Failed };
        result
    }

    fn prepare_inner(&mut self) -> Result<(), Error> {
        self.io.write16(IMR, 0);
        self.change32(RX_CONFIG, 0xf, 0);
        self.change32(MISC, 0, RXDV_GATE);
        self.io.write8(COMMAND, 0x10);
        self.wait(100, Error::Reset, |io| {
            let value = io.read8(COMMAND);
            if value == u8::MAX { Err(Error::Gone) } else { Ok(value & 0x10 == 0) }
        })?;
        self.io.write16(ISR, u16::MAX);
        self.mdio_write(31, 0)?;
        self.mdio_write(14, 0)?;
        self.phy = (self.mdio_read(2)? as u32) << 16 | self.mdio_read(3)? as u32;
        if self.phy != 0x001c_c800 { return Err(Error::Phy(self.phy)); }
        self.mdio_write(0, 0x8000)?;
        let start = self.io.now_ms();
        let mut reset = true;
        for _ in 0..100 {
            if self.mdio_read(0)? & 0x8000 == 0 { reset = false; break; }
            if self.io.now_ms().saturating_sub(start) >= 500 { break; }
            self.io.sleep_ms(1);
        }
        if reset { return Err(Error::PhyReset); }
        self.io.sleep_ms(1);
        let capabilities = self.mdio_read(1)?;
        if capabilities == 0 || capabilities == u16::MAX { return Err(Error::Phy(self.phy)); }
        self.mdio_write(4, ((capabilities >> 6) & 0x01e0) | 1)?;
        self.mdio_write(9, 0x0300)?;
        self.mdio_write(0, 0x1200)?;
        self.budget()
    }

    /// # Safety
    /// Keep these initialized coherent rings and their complete DMA mapping alive until hardware DMA is quiesced.
    pub unsafe fn program(&mut self, rings: &Rings) -> Result<(), Error> {
        if self.state != State::Prepared { return Err(Error::State); }
        let tx = rings.tx_address();
        let rx = rings.rx_address();
        if tx == 0 || tx % ALIGNMENT as u64 != 0 || rx % ALIGNMENT as u64 != 0 || tx.checked_add(DMA_BYTES as u64).is_none_or(|end| end > 1 << 32) {
            return Err(Error::Dma);
        }
        if self.io.read8(COMMAND) == u8::MAX { self.state = State::Failed; return Err(Error::Gone); }
        self.io.write8(CONFIG_LOCK, 0xc0);
        self.change8(0x54, 0x04, 0);
        self.change8(0x55, 0x02, 0);
        self.io.write8(CONFIG_LOCK, 0);
        self.io.write16(0xe0, 0x0089);
        let bytes = self.mac.to_be_bytes();
        self.io.write8(CONFIG_LOCK, 0xc0);
        self.io.write32(0, u32::from_le_bytes(bytes[2..6].try_into().unwrap()));
        self.io.write32(4, u16::from_le_bytes(bytes[6..8].try_into().unwrap()) as u32);
        self.io.write8(CONFIG_LOCK, 0);
        self.io.write32(0xe8, (rx >> 32) as u32); self.io.write32(0xe4, rx as u32);
        self.io.write32(0x24, (tx >> 32) as u32); self.io.write32(0x20, tx as u32);
        self.change32(MISC, RXDV_GATE, 0);
        self.io.write32(TX_CONFIG, 0x0300_0700);
        self.io.write8(0xec, 16);
        self.io.write32(8, u32::MAX); self.io.write32(12, u32::MAX);
        self.io.write32(RX_CONFIG, 0x0000_ff0e);
        self.io.write16(0xe2, 0x5100);
        self.io.write16(0xda, 2048);
        self.io.write32(0x58, 0);
        self.io.write32(0x4c, 0);
        self.io.write16(ISR, u16::MAX);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        self.state = State::Armed;
        Ok(())
    }

    /// # Safety
    /// PCI mastering must be enabled only after program(), with the complete DMA mapping still retained.
    pub unsafe fn enable(&mut self, interrupts: bool) -> Result<(), Error> {
        if self.state != State::Armed { return Err(Error::State); }
        if self.io.read8(COMMAND) == u8::MAX { self.state = State::Failed; return Err(Error::Gone); }
        self.io.write8(COMMAND, 0x0c);
        self.io.write16(IMR, if interrupts { IRQ_MASK } else { 0 });
        self.state = State::Running;
        Ok(())
    }

    pub fn link(&mut self) -> bool {
        let status = self.io.read8(MEDIA);
        self.state == State::Running && status != u8::MAX && status & 2 != 0
    }

    pub fn kick(&mut self) { if self.state == State::Running { self.io.write8(TX_POLL, 0x40); } }

    pub fn acknowledge(&mut self) -> Result<u16, Error> {
        let status = self.io.read16(ISR);
        if status == u16::MAX { self.state = State::Failed; return Err(Error::Gone); }
        self.io.write16(ISR, status);
        Ok(status)
    }

    /// Stops accepting work after a device fault; DMA memory is retained even if the stop request never completes.
    pub fn fail(&mut self) {
        self.io.write16(IMR, 0);
        self.change32(RX_CONFIG, 0xf, 0);
        self.change32(MISC, 0, RXDV_GATE);
        self.io.write8(COMMAND, 0x8c);
        self.state = State::Failed;
    }
}
