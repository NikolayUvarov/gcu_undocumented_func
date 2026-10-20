// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Hardware-only RTL8852B coexistence initialization between RF tables and NCTL; no RFK grant.

use crate::board::Board;
use crate::download;
use crate::io::{self, Fault, FaultKind, RegisterIo, TimedIo};
use crate::phy::{self, Phy, PhyIo, RegisterPhase, State};

const STAGE_US: u64 = 200_000;
const LTE_CTRL: usize = 0xdaf0;
const LTE_WDATA: usize = 0xdaf4;
const LTE_RDATA: usize = 0xdaf8;
const RF_MASK: u32 = 0xfffff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage { Guard, Pta, Priorities, RfMasks, Counters }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Failure {
    pub stage: Stage,
    pub cause: Fault,
    pub close_lut: [Option<Fault>; 2],
    pub disable_bb_rf: Option<Fault>,
}

/// Hardware initcfg only; policy, antenna/grants, reports and owned WLAN scoreboard state remain mandatory.
#[derive(Debug)]
pub struct ConfigPrepared { attempt:u64,session:u64 }
impl ConfigPrepared{pub(crate) fn matches(&self,attempt:u64,session:u64)->bool{attempt!=0&&session!=0&&self.attempt==attempt&&self.session==session}}

fn fault(at: usize, value: u32) -> Fault { Fault::new(FaultKind::PowerState, at, value) }

fn remaining(bus: &mut impl RegisterIo, start: u64) -> io::Result<u64> {
    let elapsed = bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock, 0, 0))?;
    STAGE_US.checked_sub(elapsed).filter(|left| *left != 0).ok_or(Fault::new(FaultKind::Timeout, 0, 0))
}

fn modify8(bus: &mut impl RegisterIo, at: usize, clear: u8, set: u8) -> io::Result<()> {
    let raw = bus.read8(at)?;
    if raw == u8::MAX { return Err(Fault::new(FaultKind::Unavailable, at, raw as u32)); }
    bus.write8(at, (raw & !clear) | set)
}

fn lte_ready(bus: &mut impl RegisterIo) -> io::Result<()> {
    let mut timed = TimedIo::new(bus, 50_000);
    for _ in 0..=1_000 {
        let raw = timed.read8(LTE_CTRL + 3)?;
        if raw == u8::MAX { return Err(Fault::new(FaultKind::Unavailable, LTE_CTRL + 3, raw as u32)); }
        timed.delay_us(0)?;
        if raw & 0x20 != 0 { return Ok(()); }
        timed.delay_us(50)?;
    }
    Err(Fault::new(FaultKind::Timeout, LTE_CTRL, 0))
}

fn pta(bus: &mut impl RegisterIo) -> io::Result<()> {
    modify8(bus, 0x40, 0, 1 << 5)?;
    modify8(bus, 0xda20, 0, 1 << 1)?;
    modify8(bus, 0xda35, 0, 1)?;
    modify8(bus, 0xda40, 0, 1 << 2 | 1 << 3)?;
    modify8(bus, 0xda42, 0, 1)?;
    modify8(bus, 0xcc07, 1 << 1, 0)?;
    let cca = bus.read16(0xc340)?;
    if cca == u16::MAX { return Err(Fault::new(FaultKind::Unavailable, 0xc340, cca as u32)); }
    bus.write16(0xc340, (cca | 1 << 5) & !(1 << 9))?;
    // Preserve source read-command/RDATA ordering; never replace a pending LTE transaction.
    lte_ready(bus)?;
    bus.write32(LTE_CTRL, 0x800f_003c)?;
    let value = io::read32(bus, LTE_RDATA)?;
    lte_ready(bus)?;
    bus.write32(LTE_WDATA, value & 1 << 8)?;
    bus.write32(LTE_CTRL, 0xc00f_003c)?;
    // This is the same ready check that would precede a subsequent indirect transaction.
    lte_ready(bus)?;
    modify8(bus, 0x40, 3 << 6, 0)?;
    modify8(bus, 0xda4c, 0, 1)?;
    modify8(bus, 0xda6c, 0x3f, 5)?;
    modify8(bus, 0x41, 1 << 2, 1 << 1)
}

fn trx_mask(bus: &mut impl RegisterIo, path: u8, group: u32, value: u32) -> io::Result<()> {
    phy::rf_write_masked(bus, path, 0xef, RF_MASK, 0x20000)?;
    phy::rf_write_masked(bus, path, 0x33, RF_MASK, group)?;
    phy::rf_write_masked(bus, path, 0x3f, RF_MASK, value)?;
    phy::rf_write_masked(bus, path, 0xef, RF_MASK, 0)
}

fn close_lut(bus: &mut impl RegisterIo, path: u8) -> io::Result<()> {
    let mut timed = TimedIo::new(bus, 1_000);
    phy::rf_write_masked(&mut timed, path, 0xef, RF_MASK, 0)?;
    let actual = phy::rf_read(&mut timed, path, 0xef, RF_MASK)?;
    if actual != 0 { return Err(fault(0xef, actual)); }
    timed.delay_us(0)
}

/// Use the same exclusively owned function and actual board as the successful firmware and PHY stages.
/// Owned receive-only DMA may run, ordinary TX remains stopped; a failed modified attempt requires retained-DMA shutdown, never retry.
pub fn prepare<B: PhyIo>(phy: &mut Phy<'_, B>, board: &Board, firmware: &download::Ready) -> core::result::Result<ConfigPrepared, Failure> {
    prepare_inner(phy, board, firmware.version(),firmware.session())
}

pub(crate) fn prepare_inner<B: PhyIo>(phy: &mut Phy<'_, B>, board: &Board, version: [u8; 4],session:u64) -> core::result::Result<ConfigPrepared, Failure> {
    let fail = |cause| Failure { stage: Stage::Guard, cause, close_lut: [None; 2], disable_bb_rf: None };
    if session==0||phy.attempt()==0||phy.bus_between_stages().command_session()!=session{return Err(fail(fault(0,0)));}
    let attempt=phy.attempt();
    if phy.state() != State::RadioLoaded { return Err(fail(fault(0, phy.state() as u32))); }
    if board.identity.cut != 1 || board.identity.rfe != 1 || version != [0, 29, 29, 15]
        || board.maps.phycap[0x6c..0x6e] != [0xff, 0xff] {
        phy.mark_failed();
        return Err(fail(Fault::new(FaultKind::Rfe, crate::efuse::RFE_AT, board.identity.rfe as u32)));
    }
    let bus = phy.bus_between_stages();
    let start = bus.now_us();
    if let Err(cause) = phy::guard(bus, &board.identity, RegisterPhase::BeforeDack).and_then(|()| remaining(bus, start).map(|_| ())) {
        phy.mark_failed();
        return Err(fail(cause));
    }
    let mut stage = Stage::Pta;
    let result = (|| {
        let left = remaining(bus, start)?;
        let mut timed = TimedIo::new(bus, left);
        pta(&mut timed)?;
        stage = Stage::Priorities;
        io::modify32(&mut timed, 0xda30, 0, 1 << 3)?;
        io::modify32(&mut timed, 0xda10, 0, 1 << 8)?;
        stage = Stage::RfMasks;
        for path in 0..2 { phy::rf_write_masked(&mut timed, path, 2, RF_MASK, 0)?; }
        for (path, group, value) in [(0, 0, 0x5ff), (1, 0, 0x5ff), (0, 2, 0x5ff), (1, 2, 0x55f)] {
            trx_mask(&mut timed, path, group, value)?;
        }
        for path in 0..2 {
            let actual = phy::rf_read(&mut timed, path, 0xef, RF_MASK)?;
            if actual != 0 { return Err(fault(0xef, actual)); }
        }
        stage = Stage::Counters;
        timed.write32(0xda2c, 0xf0ff_ffff)?;
        io::modify32(&mut timed, 0xda40, 0, 1 << 16 | 1 << 2)?;
        timed.delay_us(0)?;
        remaining(bus, start).map(|_| ())
    })();
    if let Err(cause) = result {
        let close_lut = [close_lut(bus, 0).err(), close_lut(bus, 1).err()];
        let disable_bb_rf = phy::disable_bb_rf(bus).err();
        phy.mark_failed();
        return Err(Failure { stage, cause, close_lut, disable_bb_rf });
    }
    Ok(ConfigPrepared { attempt,session })
}

#[cfg(test)]
#[path = "../../tests/rtw89_btc_init_cases.rs"]
mod tests;
