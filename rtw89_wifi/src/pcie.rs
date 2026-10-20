// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2020-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! RTL8852B PCIe preparation from pinned pci.c/pci.h/rtw8852be.c, stopping before host DMA enable.

use crate::fwcmd::{self, Bus, Queue, State};
use crate::io::{self, Fault, FaultKind, RegisterIo, TimedIo};
use crate::{mac, registers::*};

const DBI_FLAG: usize = 0x1090;
const DBI_WRITE: usize = 0x1094;
const DBI_READ: usize = 0x1098;
const MDIO_CONFIG: usize = 0x10a0;
const MDIO_WRITE: usize = 0x10a4;
const MDIO_READ: usize = 0x10a6;
const L1_CONTROL: u16 = 0x719;
const HCI_ENABLE: u32 = 1 << 11 | 1 << 13;
const STOP_ALL: u32 = 0x001f_0f03;
const EXP_CONTROL: usize = 0x13f0;
const INDEX_REGISTERS: [usize; 9] = [0x1050, 0x1054, 0x1058, 0x105c, 0x1060, 0x1064, 0x1078, 0x107c, 0x1080];
const BDRAM_TABLE: [(usize, u32); 7] = [
    (0x1200, 0x020500), (0x1204, 0x020505), (0x1208, 0x02050a), (0x120c, 0x02050f),
    (0x1220, 0x010414), (0x1224, 0x010418), (0x1228, 0x01041c),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage { Guard, Phy, Refclock, Link, Indices, Modes, Ring, Bdram }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Failure { pub stage: Stage, pub cause: Fault, pub l1_restore: Option<Fault>, pub stop: Option<Fault> }

#[derive(Debug)]
pub struct Prepared { _private: () }

#[derive(Clone, Copy)]
enum Speed { Gen1, Gen2 }

fn state(at: usize, value: u32) -> Fault { Fault::new(FaultKind::PowerState, at, value) }

fn require(io: &mut impl RegisterIo, at: usize, mask: u32, value: u32) -> io::Result<()> {
    let raw = io::read32(io, at)?;
    if raw & mask != value { return Err(state(at, raw)); }
    Ok(())
}

fn modify(io: &mut impl RegisterIo, at: usize, clear: u32, set: u32) -> io::Result<()> {
    io::modify32(io, at, clear, set)?;
    require(io, at, clear | set, set)
}

fn poll(io: &mut impl RegisterIo, at: usize, width: usize, mask: u16, timeout: u32) -> io::Result<()> {
    let start = io.now_us();
    let mut raw = 0;
    for _ in 0..=timeout / 10 + 1 {
        let elapsed = io.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock, at, raw as u32))?;
        if elapsed >= u64::from(timeout) { break; }
        raw = if width == 1 { io.read8(at)? as u16 } else { io.read16(at)? };
        let elapsed = io.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock, at, raw as u32))?;
        if elapsed >= u64::from(timeout) { break; }
        if raw & mask == 0 { return Ok(()); }
        io.delay_us(10.min((u64::from(timeout) - elapsed) as u32))?;
    }
    Err(Fault::new(FaultKind::Timeout, at, raw as u32))
}

fn mdio_command(io: &mut impl RegisterIo, address: u8, speed: Speed, flag: u16) -> io::Result<()> {
    let idle = io.read16(MDIO_CONFIG)?;
    if idle & 0x300 != 0 { return Err(state(MDIO_CONFIG, idle as u32)); }
    io.write8(MDIO_CONFIG, address & 31)?;
    let page = (address >= 32) as u16 | if matches!(speed, Speed::Gen2) { 2 } else { 0 };
    let value = io.read16(MDIO_CONFIG)? & !0x3000 | page << 12;
    io.write16(MDIO_CONFIG, value)?;
    io.write16(MDIO_CONFIG, value | flag)?;
    poll(io, MDIO_CONFIG, 2, 0x300, 2_000)
}

fn mdio_modify(io: &mut impl RegisterIo, address: u8, speed: Speed, clear: u16, set: u16) -> io::Result<()> {
    mdio_command(io, address, speed, 1 << 9)?;
    let original = io.read16(MDIO_READ)?;
    if address == 0x30 && original & clear == set { return Ok(()); }
    let value = (original & !clear) | set;
    io.write16(MDIO_WRITE, value)?;
    mdio_command(io, address, speed, 1 << 8)?;
    mdio_command(io, address, speed, 1 << 9)?;
    let actual = io.read16(MDIO_READ)?;
    if actual & (clear | set) != set { return Err(state(MDIO_READ, actual as u32)); }
    Ok(())
}

fn dbi_command(io: &mut impl RegisterIo, address: u16, write: bool) -> io::Result<()> {
    poll(io, DBI_FLAG + 2, 1, 0xff, 200)?;
    let lane = address & 3;
    let value = address & 0x0ffc | if write { 1 << (12 + lane) } else { 0 };
    io.write16(DBI_FLAG, value)?;
    io.write8(DBI_FLAG + 2, if write { 1 } else { 2 })?;
    poll(io, DBI_FLAG + 2, 1, 0xff, 200)
}

fn dbi_read(io: &mut impl RegisterIo, address: u16) -> io::Result<u8> {
    if !matches!(address, 0x82 | L1_CONTROL) { return Err(Fault::new(FaultKind::Access, address as usize, 0)); }
    dbi_command(io, address, false)?;
    io.read8(DBI_READ + (address & 3) as usize)
}

fn dbi_write_l1(io: &mut impl RegisterIo, value: u8) -> io::Result<()> {
    poll(io, DBI_FLAG + 2, 1, 0xff, 200)?;
    io.write8(DBI_WRITE + (L1_CONTROL & 3) as usize, value)?;
    dbi_command(io, L1_CONTROL, true)?;
    let actual = dbi_read(io, L1_CONTROL)?;
    if actual != value { return Err(state(L1_CONTROL as usize, actual as u32)); }
    Ok(())
}

fn stop(bus: &mut impl RegisterIo) -> io::Result<()> {
    let mut timed = TimedIo::new(bus, 1_000);
    modify(&mut timed, fwcmd::DMA_STOP, 0, STOP_ALL)?;
    modify(&mut timed, fwcmd::INIT_CONFIG, HCI_ENABLE, 0)?;
    io::poll32(&mut timed, fwcmd::DMA_BUSY, fwcmd::BUSY_MASK, 0, 10, 100)?;
    timed.delay_us(0)
}

fn guard(bus: &mut impl Bus) -> io::Result<()> {
    mac::guard(bus)?;
    require(bus, R_AX_DMAC_FUNC_EN, 0x64c4_0000, 0x64c4_0000)?;
    require(bus, mac::DMAC_CLOCK, 0x0484_0000, 0x0484_0000)?;
    require(bus, mac::WDE_CONFIG, 0x1fff_3f03, 0)?;
    require(bus, mac::PLE_CONFIG, 0x1fff_3f03, 0x0040_0801)?;
    require(bus, 0x8c44, u32::MAX, 48)?;
    require(bus, mac::HFC_CONTROL, 0xc09, 8)?;
    require(bus, mac::CHANNEL_PAGE, u32::MAX, 40 << 16)?;
    require(bus, mac::WDE_READY, 3, 3)?;
    require(bus, mac::PLE_READY, 3, 3)?;
    let mdio = bus.read16(MDIO_CONFIG)?;
    let dbi = bus.read8(DBI_FLAG + 2)?;
    if mdio & 0x300 != 0 { return Err(state(MDIO_CONFIG, mdio as u32)); }
    if dbi != 0 { return Err(state(DBI_FLAG + 2, dbi as u32)); }
    Ok(())
}

/// Prepare only the observed RTL8852BE/cut-1 path after MAC download-memory preparation, with PCI mastering off.
/// The caller owns the verified BAR2 grant and powers the MAC off after failure; this never enables host DMA.
pub fn prepare(bus: &mut impl Bus) -> core::result::Result<Prepared, Failure> {
    guard(bus).map_err(|cause| Failure { stage: Stage::Guard, cause, l1_restore: None, stop: None })?;
    let mut stage = Stage::Phy;
    let mut l1_original = None;
    let result = (|| {
        let mut timed = TimedIo::new(bus, 50_000);
        modify(&mut timed, 0x1008, 1 << 5, 0)?;
        modify(&mut timed, R_AX_SYS_PW_CTRL, 1 << 14, 0)?;
        modify(&mut timed, R_AX_SYS_SDIO_CTRL, 0, 1 << 15)?;
        modify(&mut timed, R_AX_SYS_SDIO_CTRL, 1 << 14, 0)?;
        mdio_modify(&mut timed, 0x1b, Speed::Gen1, 0xf000, 0x1000)?;
        mdio_modify(&mut timed, 0x1d, Speed::Gen1, 0x000c, 0x000c)?;
        stage = Stage::Refclock;
        let raw = dbi_read(&mut timed, 0x82)?;
        let speed = match raw & 3 { 1 => Speed::Gen1, 2 => Speed::Gen2, _ => return Err(state(0x82, raw as u32)) };
        let l1 = dbi_read(&mut timed, L1_CONTROL)?;
        if l1 & 8 != 0 { l1_original = Some(l1); dbi_write_l1(&mut timed, l1 & !8)?; }
        mdio_modify(&mut timed, 0x30, speed, 1 << 13, 0)?;
        if let Some(original) = l1_original { dbi_write_l1(&mut timed, original)?; l1_original = None; }
        stage = Stage::Link;
        modify(&mut timed, 0x74, 0, 1 << 5)?;
        modify(&mut timed, EXP_CONTROL, 1 << 4, 0)?;
        io::modify32(&mut timed, 0x11d8, 0xf0, 0x83)?;
        require(&mut timed, 0x11d8, 0xf1, 0x81)?;
        modify(&mut timed, 0x11c0, 0, 3)?;
        modify(&mut timed, EXP_CONTROL, 3, 1)?;
        modify(&mut timed, fwcmd::INIT_CONFIG, 0, 3 << 22)?;
        modify(&mut timed, fwcmd::DMA_STOP, 0, STOP_ALL)?;
        modify(&mut timed, fwcmd::INIT_CONFIG, HCI_ENABLE, 0)?;
        io::poll32(&mut timed, fwcmd::DMA_BUSY, fwcmd::BUSY_MASK, 0, 10, 100)?;
        stage = Stage::Indices;
        io::modify32(&mut timed, fwcmd::CLEAR_INDEX, 0, 0x70f)?;
        io::modify32(&mut timed, 0x1018, 0, 3)?;
        for at in INDEX_REGISTERS { io::poll32(&mut timed, at, 0x0fff_0fff, 0, 10, 100)?; }
        stage = Stage::Modes;
        modify(&mut timed, fwcmd::INIT_CONFIG, 1 << 18, 0)?;
        modify(&mut timed, fwcmd::INIT_CONFIG, 7 << 8, 7 << 8)?;
        modify(&mut timed, fwcmd::INIT_CONFIG, 7 << 14, 3 << 14)?;
        modify(&mut timed, fwcmd::INIT_CONFIG, 0, 1 << 12)?;
        modify(&mut timed, EXP_CONTROL, 7 << 16, 7 << 16)?;
        modify(&mut timed, 0x1004, 15 << 24 | 15 << 16, 1 << 24 | 1 << 16)?;
        modify(&mut timed, 0x8810, 0, 1)?;
        modify(&mut timed, 0x9a00, 2, 0)?;
        timed.delay_us(0)
    })();
    if let Err(cause) = result {
        let l1_restore = l1_original.and_then(|original| {
            let mut timed = TimedIo::new(bus, 1_000);
            dbi_write_l1(&mut timed, original).and_then(|()| timed.delay_us(0)).err()
        });
        let stopped = stop(bus).err();
        return Err(Failure { stage, cause, l1_restore, stop: stopped });
    }
    Ok(Prepared { _private: () })
}

impl Prepared {
    /// Complete BDRAM setup after this device's command queue is installed; leave all channels and HCI stopped.
    /// No ordinary RX/TX rings exist here, so a later caller must not enable their channels or claim a working radio.
    pub fn finish(self, bus: &mut impl Bus, queue: &Queue<'_>) -> core::result::Result<(), Failure> {
        let mut stage = Stage::Ring;
        let mut modified = false;
        let result = (|| {
            guard(bus)?;
            if bus.pci_command()? & 7 != 2 || queue.state() != State::Installed { return Err(state(4, 0)); }
            let mut timed = TimedIo::new(bus, 5_000);
            require(&mut timed, fwcmd::INIT_CONFIG, HCI_ENABLE, 0)?;
            require(&mut timed, fwcmd::DMA_STOP, STOP_ALL, STOP_ALL)?;
            require(&mut timed, fwcmd::BASE_LOW, u32::MAX, queue.physical() as u32)?;
            require(&mut timed, fwcmd::BASE_HIGH, u32::MAX, 0)?;
            let count = timed.read16(fwcmd::NUMBER)?;
            if count != fwcmd::DESCRIPTORS as u16 { return Err(state(fwcmd::NUMBER, count as u32)); }
            require(&mut timed, fwcmd::INDEX, 0x0fff_0fff, 0)?;
            io::poll32(&mut timed, fwcmd::DMA_BUSY, fwcmd::BUSY_MASK, 0, 10, 100)?;
            stage = Stage::Bdram;
            modified = true;
            for (at, value) in BDRAM_TABLE { timed.write32(at, value)?; require(&mut timed, at, u32::MAX, value)?; }
            io::modify32(&mut timed, fwcmd::INIT_CONFIG, 0, 8)?;
            io::poll32(&mut timed, fwcmd::INIT_CONFIG, 8, 0, 1, 100)?;
            timed.delay_us(0)
        })();
        if let Err(cause) = result {
            let stopped = if modified { stop(bus).err() } else { None };
            return Err(Failure { stage, cause, l1_restore: None, stop: stopped });
        }
        Ok(())
    }
}
