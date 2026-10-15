// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022, 2024 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Bounded RFE-1/cut-1 table stages from pinned phy.c; these do not calibrate or authorize transmission.

use crate::board::Identity;
use crate::io::{self, Fault, FaultKind, RegisterIo, TimedIo};
use crate::phy_tables as tables;
use crate::power;
use crate::registers::*;

pub const BAR_BYTES: usize = 0x20000;
pub const PHY_BASE: usize = 0x10000;
const RF_MASK: u32 = 0xfffff;
const SWSI_DATA: usize = PHY_BASE + 0x370;
const SWSI_STATUS: usize = PHY_BASE + 0x174c;
const SWSI_BUSY: u32 = 3 << 24;
const STAGE_US: u64 = 100_000;
const DATA_STOP: u32 = 0x000b_0f03;
const DATA_BUSY: u32 = 0x006b_0f03;

/// All reads and the H2C transport refer to the same exclusively owned RTL8852BE function.
/// The transport copies the payload and drains hardware command consumption/PCIe TX busy before returning.
/// All transport work shares `timeout_us`; command consumption does not establish firmware application.
/// Send H2C type=0/category=2/class=8+path/function=page, with explicit receive/done ACK flags false.
/// Preserve the normal H2C sequence and its periodic receive-ACK policy; DMA drain is not a firmware ACK.
/// It may operate only the preinstalled command ring; it must retain DMA memory after every failure.
pub trait PhyIo: RegisterIo {
    fn pci_id(&mut self) -> io::Result<u32>;
    fn pci_command(&mut self) -> io::Result<u16>;
    fn bar_len(&self) -> usize;
    fn send_rf_page(&mut self, path: u8, page: u8, payload: &[u8], timeout_us: u32) -> io::Result<()>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage { Baseband, TxPowerUnit, BasebandReset, RadioA, RadioB, Nctl, Dack, DpkBackoff, RckA, RckB }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State { Fresh, BasebandLoaded, RadioLoaded, TablesLoaded, RckDone, DackDone, Failed }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Failure { pub stage: Stage, pub cause: Fault, pub disable_bb_rf: Option<Fault>, pub restore_rf: Option<Fault> }

/// Successful table writes only; RFK, board trims, channel setup and regulatory/SAR limits remain mandatory.
#[derive(Debug)]
pub struct TablesLoaded { _private: () }

/// RCK results only; DACK/RXDCK, channel calibration and regulatory limits are still mandatory.
#[derive(Debug, Eq, PartialEq)]
pub struct RckResult { pub values: [u8; 2], pub dpk_gain: u8 }

/// Gain defaults decoded into software state, distinct from board-specific EFUSE trims.
#[derive(Debug, Eq, PartialEq)]
pub struct Gain {
    pub lna: [[[i8; 7]; 2]; 4],
    pub tia: [[[i8; 2]; 2]; 4],
    pub offset20: [[i8; 2]; 4],
    pub offset40: [[[i8; 3]; 2]; 4],
    pub offset80: [[[i8; 11]; 2]; 4],
}

pub fn gain_defaults() -> Gain {
    let mut gain = Gain { lna: [[[0; 7]; 2]; 4], tia: [[[0; 2]; 2]; 4], offset20: [[0; 2]; 4],
        offset40: [[[0; 3]; 2]; 4], offset80: [[[0; 11]; 2]; 4] };
    for &(at, data) in tables::GAIN {
        let band = ((at >> 16) & 255) as usize;
        let path = ((at >> 8) & 255) as usize;
        let bytes = data.to_le_bytes().map(|value| value as i8);
        match (at >> 24, at & 255) {
            (0, 0) => gain.lna[band][path][..4].copy_from_slice(&bytes),
            (0, 1) => gain.lna[band][path][4..].copy_from_slice(&bytes[..3]),
            (0, 2) => gain.tia[band][path].copy_from_slice(&bytes[..2]),
            (1, 0) => gain.offset20[band][path] = bytes[0],
            (1, 0x10) => gain.offset40[band][path][0] = bytes[0],
            (1, 0x11) => gain.offset40[band][path][1..].copy_from_slice(&bytes[..2]),
            (1, 0x20) => gain.offset80[band][path][0] = bytes[0],
            (1, 0x21) => gain.offset80[band][path][1..5].copy_from_slice(&bytes),
            (1, 0x29) => gain.offset80[band][path][9..11].copy_from_slice(&bytes[..2]),
            _ => unreachable!(),
        }
    }
    gain
}

fn state(at: usize, value: u32) -> Fault { Fault::new(FaultKind::PowerState, at, value) }

fn require(io: &mut impl RegisterIo, at: usize, mask: u32, expected: u32) -> io::Result<()> {
    let raw = io::read32(io, at)?;
    if raw & mask != expected { return Err(state(at, raw)); }
    Ok(())
}

fn remaining(io: &mut impl RegisterIo, start: u64) -> io::Result<u32> {
    let elapsed = io.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock, 0, 0))?;
    STAGE_US.checked_sub(elapsed).filter(|left| *left != 0).map(|left| left as u32)
        .ok_or(Fault::new(FaultKind::Timeout, 0, 0))
}

fn guard(bus: &mut impl PhyIo, identity: &Identity) -> io::Result<()> {
    if bus.bar_len() < BAR_BYTES { return Err(Fault::new(FaultKind::Access, BAR_BYTES, bus.bar_len().min(u32::MAX as usize) as u32)); }
    let id = bus.pci_id()?;
    let command = bus.pci_command()?;
    if id != 0xb852_10ec || command == u16::MAX || command & 6 != 6 { return Err(Fault::new(FaultKind::Access, 4, command as u32)); }
    if identity.cut != 1 { return Err(Fault::new(FaultKind::UnsupportedCut, SYS_CFG1, identity.cut as u32)); }
    if identity.rfe != 1 { return Err(Fault::new(FaultKind::Rfe, crate::efuse::RFE_AT, identity.rfe as u32)); }
    require(bus, SYS_CFG1, CHIP_VERSION_MASK, 1 << 12)?;
    require(bus, R_AX_IC_PWR_STATE, B_AX_WLMAC_PWR_STE_MASK, 1 << 8)?;
    require(bus, R_AX_PLATFORM_ENABLE, 3, 3)?;
    require(bus, 0x1e0, 0xe0, 0xe0)?;
    require(bus, R_AX_DMAC_FUNC_EN, 0xfffd_0000, 0xfffd_0000)?;
    require(bus, 0x8404, 0x0f9f_0000, 0x0f9f_0000)?;
    require(bus, R_AX_CMAC_FUNC_EN, 0xf000_003f, 0xf000_003f)?;
    require(bus, 0xc004, 0x4000_003f, 0x4000_003f)?;
    require(bus, 0xe000, 1 << 30, 0)?;
    let bb = bus.read8(R_AX_SYS_FUNC_EN)?;
    if bb == u8::MAX || bb & 3 != 3 { return Err(state(R_AX_SYS_FUNC_EN, bb as u32)); }
    require(bus, R_AX_SPS_DIG_ON_CTRL0, B_AX_REG_ZCDC_H_MASK, 1 << 17)?;
    require(bus, R_AX_WLRF_CTRL, B_AX_AFC_AFEDIG, B_AX_AFC_AFEDIG)?;
    let cycle = bus.read8(0x8040)?;
    if cycle == u8::MAX || cycle & 15 != 14 { return Err(state(0x8040, cycle as u32)); }
    require(bus, 0x1000, 1 << 11 | 1 << 13, 1 << 11)?;
    require(bus, 0x1010, DATA_STOP | 1 << 18 | 1 << 20, DATA_STOP)?;
    require(bus, 0x101c, DATA_BUSY, 0)?;
    require(bus, 0xc348, 0xffff, 0)?;
    require(bus, 0xc6ec, 1 << 7, 0)?;
    Ok(())
}

pub(crate) fn disable_bb_rf(bus: &mut impl RegisterIo) -> io::Result<()> {
    // Two XTAL paths share the eight 50 ms idle/read/write/readback ceilings.
    let mut io = TimedIo::new(bus, 400_000);
    let mut first = None;
    let mut remember = |result: io::Result<()>| { if let Err(error) = result { if first.is_none() { first = Some(error); } } };
    remember((|| {
        io::modify32(&mut io, R_AX_WLRF_CTRL, B_AX_AFC_AFEDIG, 0)?;
        require(&mut io, R_AX_WLRF_CTRL, B_AX_AFC_AFEDIG, 0)?;
        io.delay_us(0)
    })());
    remember((|| {
        io::modify8(&mut io, R_AX_SYS_FUNC_EN, 3, 0)?;
        let value = io.read8(R_AX_SYS_FUNC_EN)?;
        if value & 3 != 0 { return Err(state(R_AX_SYS_FUNC_EN, value as u32)); }
        io.delay_us(0)
    })());
    for offset in [0x80u8, 0x81] {
        remember((|| {
            // Do not replace an XTAL command still pending after an earlier failed read or write.
            io::poll32(&mut io, R_AX_WLAN_XTAL_SI_CTRL, B_AX_WL_XTAL_SI_CMD_POLL, 0, 50, 50_000)?;
            let original = power::xtal_read(&mut io, offset)?;
            power::xtal_write(&mut io, offset, original & !7, 255)?;
            let actual = power::xtal_read(&mut io, offset)?;
            if actual != original & !7 { return Err(state(offset as usize, actual as u32)); }
            io.delay_us(0)
        })());
    }
    first.map_or(Ok(()), Err)
}

fn write_table(io: &mut impl RegisterIo, table: &[(u32, u32)]) -> io::Result<()> {
    for &(at, value) in table { io.write32(PHY_BASE + at as usize, value)?; }
    io.delay_us(0)
}

fn init_txpower_unit(io: &mut impl RegisterIo) -> io::Result<()> {
    // These are single-band CMAC0 power registers, not MCU-offset PHY registers.
    for (at, value) in [(0xd248, 0x07763333), (0xd220, 0x01ebf000), (0xd240, 0x0002f8ff)] {
        io.write32(at, value)?;
    }
    io::modify32(io, 0xd288, 0, 1 << 31)?;
    io::modify32(io, 0xd28c, 31, 0)?;
    io::modify32(io, 0xd290, 31, 0x1d)?;
    io.delay_us(0)
}

fn reset_baseband(io: &mut impl RegisterIo) -> io::Result<()> {
    let tssi = [0x58dc, 0x5818, 0x78dc, 0x7818];
    for at in tssi { io::modify32(io, PHY_BASE + at, 0, 1 << 30)?; }
    for at in [0x1200, 0x3200] { io::modify32(io, PHY_BASE + at, 7 << 28, 7 << 28)?; }
    io.delay_us(1)?;
    io::modify32(io, PHY_BASE + 0x704, 0, 2)?;
    io::modify32(io, PHY_BASE + 0x704, 2, 0)?;
    for at in [0x1200, 0x3200] { io::modify32(io, PHY_BASE + at, 7 << 28, 0)?; }
    io::modify32(io, PHY_BASE + 0x704, 0, 2)?;
    for at in tssi { io::modify32(io, PHY_BASE + at, 1 << 30, 0)?; }
    io.delay_us(0)
}

fn rf_field(path: u8, at: u32, mask: u32) -> io::Result<u32> {
    if path > 1 || at & !0x100ff != 0 || mask == 0 || mask & !RF_MASK != 0 {
        return Err(Fault::new(FaultKind::Access, at as usize, mask));
    }
    Ok(mask.trailing_zeros())
}

pub(crate) fn rf_read(io: &mut impl RegisterIo, path: u8, at: u32, mask: u32) -> io::Result<u32> {
    let shift = rf_field(path, at, mask)?;
    let value = if at & 0x10000 != 0 {
        io::read32(io, PHY_BASE + 0xe000 + (path as usize * 0x1000) + ((at & 255) as usize * 4))?
    } else {
        io::poll32(io, SWSI_STATUS, SWSI_BUSY, 0, 1, 30)?;
        io::modify32(io, PHY_BASE + 0x378, 0x7ff, (path as u32) << 8 | at)?;
        io.delay_us(2)?;
        io::poll32(io, SWSI_STATUS, 1 << 26, 1 << 26, 1, 30)?;
        io::read32(io, SWSI_STATUS)?
    };
    io.delay_us(0)?;
    Ok((value & mask) >> shift)
}

pub(crate) fn rf_write_masked(io: &mut impl RegisterIo, path: u8, at: u32, mask: u32, value: u32) -> io::Result<()> {
    let shift = rf_field(path, at, mask)?;
    if value > (mask >> shift) || (value << shift) & !mask != 0 {
        return Err(Fault::new(FaultKind::Access, at as usize, value));
    }
    let bits = value << shift;
    if at & 0x10000 != 0 {
        let address = PHY_BASE + 0xe000 + (path as usize * 0x1000) + ((at & 255) as usize * 4);
        io::modify32(io, address, mask, bits)?;
        io.delay_us(1)
    } else {
        io::poll32(io, SWSI_STATUS, SWSI_BUSY, 0, 1, 30)?;
        let masked = mask != RF_MASK;
        if masked { io::modify32(io, PHY_BASE + 0x374, RF_MASK, mask)?; }
        io.write32(SWSI_DATA, (masked as u32) << 31 | (path as u32) << 28 | at << 20 | bits)?;
        io.delay_us(0)
    }
}

fn rf_write(io: &mut impl RegisterIo, path: u8, at: u32, value: u32) -> io::Result<()> {
    rf_write_masked(io, path, at, RF_MASK, value)
}

fn restore_rsv1(io: &mut impl RegisterIo, path: u8, original: u32) -> io::Result<()> {
    rf_write(io, path, 5, original)?;
    let actual = rf_read(io, path, 5, RF_MASK)?;
    if actual != original { return Err(state(5, actual)); }
    io.delay_us(0)
}

fn poll_rck(io: &mut impl RegisterIo, path: u8) -> io::Result<()> {
    let mut poll = TimedIo::new(io, 30);
    for _ in 0..=15 {
        let value = rf_read(&mut poll, path, 0x1c, 8)?;
        poll.delay_us(0)?;
        if value != 0 { return Ok(()); }
        poll.delay_us(2)?;
    }
    Err(Fault::new(FaultKind::Timeout, 0x1c, path as u32))
}

/// One startup attempt, with failed attempts poisoned until the caller destroys and powers down the device.
/// The caller has verified the pinned firmware, read this board's EFUSE and initialized full single-band MAC.
/// Only command DMA may run; ordinary RX/TX channels and all MAC scheduler queues must remain stopped.
/// No concurrent firmware calibration, scan, autonomous TX or register writer may run during these stages.
pub struct Phy<'a, B> { bus: &'a mut B, identity: Identity, state: State }

impl<'a, B: PhyIo> Phy<'a, B> {
    pub fn new(bus: &'a mut B, identity: Identity) -> Self { Self { bus, identity, state: State::Fresh } }
    pub fn state(&self) -> State { self.state }
    /// Execute the documented interleaved chip steps on this same device, retaining all startup restrictions.
    pub fn bus_between_stages(&mut self) -> &mut B { self.bus }

    fn begin(&mut self, stage: Stage, expected: State) -> core::result::Result<u64, Failure> {
        if self.state != expected { return Err(Failure { stage, cause: state(0, self.state as u32), disable_bb_rf: None, restore_rf: None }); }
        let start = self.bus.now_us();
        if let Err(cause) = guard(self.bus, &self.identity).and_then(|()| remaining(self.bus, start).map(|_| ())) {
            self.state = State::Failed;
            return Err(Failure { stage, cause, disable_bb_rf: None, restore_rf: None });
        }
        Ok(start)
    }
    fn failed(&mut self, stage: Stage, cause: Fault) -> Failure {
        self.state = State::Failed;
        Failure { stage, cause, disable_bb_rf: disable_bb_rf(self.bus).err(), restore_rf: None }
    }

    /// Apply BB, initialize the chip TX-power unit and software gain defaults, then reset BB under one deadline.
    pub fn baseband(&mut self) -> core::result::Result<Gain, Failure> {
        let start = self.begin(Stage::Baseband, State::Fresh)?;
        let mut stage = Stage::Baseband;
        let result = (|| {
            let left = remaining(self.bus, start)?;
            let mut io = TimedIo::new(self.bus, left as u64);
            write_table(&mut io, tables::BB)?;
            stage = Stage::TxPowerUnit;
            init_txpower_unit(&mut io)?;
            let gain = gain_defaults();
            stage = Stage::BasebandReset;
            reset_baseband(&mut io)?;
            remaining(self.bus, start)?;
            Ok(gain)
        })();
        let gain = result.map_err(|cause| self.failed(stage, cause))?;
        self.state = State::BasebandLoaded;
        Ok(gain)
    }

    /// Follow completed baseband initialization; failure never admits path B or another stage.
    pub fn radio(&mut self) -> core::result::Result<(), Failure> {
        let start = self.begin(Stage::RadioA, State::BasebandLoaded)?;
        let mut stage = Stage::RadioA;
        let result = (|| {
            let mut payload = [0u8; 294 * 4];
            for (path, table) in [(0u8, tables::RADIO_A), (1u8, tables::RADIO_B)] {
                stage = if path == 0 { Stage::RadioA } else { Stage::RadioB };
                let left = remaining(self.bus, start)?;
                let mut io = TimedIo::new(self.bus, left as u64);
                let mut length = 0;
                for &(at, value) in table {
                    rf_write(&mut io, path, at, value)?;
                    if at & 0x10000 != 0 {
                        let word = ((at & 255) << 20) | value;
                        payload[length..length + 4].copy_from_slice(&word.to_le_bytes());
                        length += 4;
                    }
                }
                io::poll32(&mut io, SWSI_STATUS, SWSI_BUSY, 0, 1, 30)?;
                io.delay_us(0)?;
                let left = remaining(self.bus, start)?;
                self.bus.send_rf_page(path, 0, &payload[..length], left)?;
                remaining(self.bus, start)?;
            }
            Ok(())
        })();
        if let Err(cause) = result { return Err(self.failed(stage, cause)); }
        self.state = State::RadioLoaded;
        Ok(())
    }

    /// Call after BTC initialization and the pre-NCTL prefix of PHY dynamic management, before RF calibration.
    pub fn nctl(&mut self) -> core::result::Result<TablesLoaded, Failure> {
        let start = self.begin(Stage::Nctl, State::RadioLoaded)?;
        let result = (|| {
            let left = remaining(self.bus, start)?;
            let mut io = TimedIo::new(self.bus, left as u64);
            for (at, bits) in [(0xc60, 3), (0xc6c, 1), (0x58ac, 1 << 27), (0x78ac, 1 << 27), (0xc60, 2)] {
                io::modify32(&mut io, PHY_BASE + at, 0, bits)?;
            }
            io.write32(PHY_BASE + 0x8000, 8)?;
            let mut ready = false;
            let mut poll = TimedIo::new(&mut io, 1_000);
            for _ in 0..=100 {
                poll.write32(PHY_BASE + 0x8080, 4)?;
                poll.delay_us(1)?;
                let raw = io::read32(&mut poll, PHY_BASE + 0x8080)?;
                poll.delay_us(0)?;
                if raw == 4 { ready = true; break; }
                poll.delay_us(10)?;
            }
            if !ready { return Err(Fault::new(FaultKind::Timeout, PHY_BASE + 0x8080, 0)); }
            write_table(&mut io, tables::NCTL)?;
            remaining(self.bus, start).map(|_| ())
        })();
        if let Err(cause) = result { return Err(self.failed(Stage::Nctl, cause)); }
        self.state = State::TablesLoaded;
        Ok(TablesLoaded { _private: () })
    }

    /// Follow NCTL with DPK backoff and both RCK paths; no BTC RFK notification surrounds RCK in the pinned chip source.
    /// The caller retains fresh TSSI/MCC software state and completed BTC/DM prerequisites; DACK remains a separate stage.
    pub fn rck(&mut self) -> core::result::Result<RckResult, Failure> {
        let start = self.begin(Stage::DpkBackoff, State::TablesLoaded)?;
        let mut stage = Stage::DpkBackoff;
        let mut restore = None;
        let result = (|| {
            let left = remaining(self.bus, start)?;
            let mut io = TimedIo::new(self.bus, left as u64);
            let backoff = io::read32(&mut io, PHY_BASE + 0x44a0)?;
            let dpk_gain = if ((backoff >> 12) & 31) + (backoff & 127) >= 44 {
                for at in [0x81bc, 0x82bc] { io::modify32(&mut io, PHY_BASE + at, 0x7fffff, 0x7f7f7f)?; }
                0x7f
            } else { 0x5b };
            let mut values = [0; 2];
            for path in 0..2 {
                stage = if path == 0 { Stage::RckA } else { Stage::RckB };
                let original = rf_read(&mut io, path, 5, RF_MASK)?;
                restore = Some((path, original));
                rf_write_masked(&mut io, path, 5, 1, 0)?;
                rf_write_masked(&mut io, path, 0, 0xf0000, 3)?;
                rf_write(&mut io, path, 0x1b, 0x240)?;
                poll_rck(&mut io, path)?;
                let value = rf_read(&mut io, path, 0x1b, 0x7c00)?;
                rf_write(&mut io, path, 0x1b, value)?;
                restore_rsv1(&mut io, path, original)?;
                restore = None;
                values[path as usize] = value as u8;
            }
            remaining(self.bus, start)?;
            Ok(RckResult { values, dpk_gain })
        })();
        match result {
            Ok(result) => { self.state = State::RckDone; Ok(result) }
            Err(cause) => {
                let restore_rf = restore.and_then(|(path, original)| {
                    let mut cleanup = TimedIo::new(self.bus, 200);
                    restore_rsv1(&mut cleanup, path, original).err()
                });
                let mut failure = self.failed(stage, cause);
                failure.restore_rf = restore_rf;
                Err(failure)
            }
        }
    }


    /// Run DACK only after RCK, with actual BTC arbitration and the same initialized channel-band context.
    pub fn dack(&mut self,band:crate::dack::Band)->core::result::Result<crate::dack::ResultData,crate::dack::Failure>
    where B:crate::dack::DackIo {
        let start=self.begin(Stage::Dack,State::RckDone).map_err(|failure|crate::dack::Failure::guard(failure.cause))?;
        let result=crate::dack::run(self.bus,band,start);
        self.state=if result.is_ok() {State::DackDone} else {State::Failed};
        result
    }

}
