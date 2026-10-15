// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! RTL8852B download-memory preparation from pinned mac.c and rtw8852b.c; no host DMA startup.

use crate::fwcmd::{self, Bus};
use crate::io::{self, Fault, FaultKind, RegisterIo, TimedIo};
use crate::registers::*;

pub const DMAC_CLOCK: usize = 0x8404;
pub const HFC_CONTROL: usize = 0x8a00;
pub const CHANNEL_PAGE: usize = 0x8a04;
pub const WDE_CONFIG: usize = 0x8c08;
pub const PLE_CONFIG: usize = 0x9008;
pub const WDE_READY: usize = 0x8d00;
pub const PLE_READY: usize = 0x9100;
const DLE_ENABLE: u32 = B_AX_DLE_WDE_EN | B_AX_DLE_PLE_EN;
const HFC_ENABLE: u32 = 1 | 1 << 3;
const MEMORY_FIELDS: u32 = 0x1fff_3f03;
const DMAC_BASE: u32 = B_AX_MAC_FUNC_EN | B_AX_DMAC_FUNC_EN | B_AX_DISPATCHER_EN | B_AX_PKT_BUF_EN;
const DISPATCH_CLOCK: u32 = 1 << 18;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage { Guard, Engines, Quotas, Ready, FlowControl }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Failure { pub stage: Stage, pub cause: Fault, pub hfc_disable: Option<Fault>, pub dle_disable: Option<Fault> }

fn require(io: &mut impl RegisterIo, at: usize, mask: u32, expected: u32) -> io::Result<()> {
    let value = io::read32(io, at)?;
    if value & mask != expected { return Err(Fault::new(FaultKind::PowerState, at, value)); }
    Ok(())
}

fn write(io: &mut impl RegisterIo, at: usize, value: u32) -> io::Result<()> {
    io.write32(at, value)?;
    require(io, at, u32::MAX, value)
}

fn modify(io: &mut impl RegisterIo, at: usize, clear: u32, set: u32) -> io::Result<()> {
    let value = io::read32(io, at)?;
    write(io, at, (value & !clear) | set)
}

fn disable(bus: &mut impl RegisterIo, at: usize, mask: u32) -> io::Result<()> {
    let mut timed = TimedIo::new(bus, 1_000);
    modify(&mut timed, at, mask, 0)?;
    timed.delay_us(0)
}

pub(crate) fn guard(bus: &mut impl Bus) -> io::Result<()> {
    let command = bus.pci_command()?;
    if command & 7 != 2 { return Err(Fault::new(FaultKind::Access, 4, command as u32)); }
    let raw = io::read32(bus, SYS_CFG1)?;
    if chip_cut(raw) != Some(1) { return Err(Fault::new(FaultKind::UnsupportedCut, SYS_CFG1, raw)); }
    require(bus, R_AX_IC_PWR_STATE, B_AX_WLMAC_PWR_STE_MASK, 1 << 8)?;
    require(bus, R_AX_PLATFORM_ENABLE, 1 << 1, 0)?;
    let bb = bus.read8(R_AX_SYS_FUNC_EN)?;
    if bb & 3 != 0 { return Err(Fault::new(FaultKind::PowerState, R_AX_SYS_FUNC_EN, bb as u32)); }
    require(bus, fwcmd::INIT_CONFIG, 1 << 11 | 1 << 13, 0)?;
    require(bus, fwcmd::DMA_BUSY, fwcmd::BUSY_MASK, 0)
}

/// Prepare internal MAC memory on the verified RTL8852BE grant after board power-on, with BB/WCPU held stopped.
/// The caller retains PCI MEM-only ownership and must power the MAC off after a failed preparation.
/// This neither performs PCIe pre-initialization nor enables DMA, WCPU, BB/RF or any packet channel.
pub fn prepare_download(bus: &mut impl Bus) -> core::result::Result<(), Failure> {
    let failure = |cause| Failure { stage: Stage::Guard, cause, hfc_disable: None, dle_disable: None };
    guard(bus).map_err(failure)?;
    let mut stage = Stage::Engines;
    let result = (|| {
        let mut timed = TimedIo::new(bus, 10_000);
        write(&mut timed, R_AX_DMAC_FUNC_EN, DMAC_BASE)?;
        write(&mut timed, DMAC_CLOCK, DISPATCH_CLOCK)?;
        modify(&mut timed, R_AX_DMAC_FUNC_EN, DLE_ENABLE, 0)?;
        modify(&mut timed, DMAC_CLOCK, 0, DLE_ENABLE)?;
        stage = Stage::Quotas;
        // DLFW uses 1024 unlinked 64-byte WDE pages and 64 linked + 960 unlinked 128-byte PLE pages.
        modify(&mut timed, WDE_CONFIG, MEMORY_FIELDS, 0)?;
        modify(&mut timed, PLE_CONFIG, MEMORY_FIELDS, 64 << 16 | 8 << 8 | 1)?;
        // The download WDE quota inherits SCC's WCPU minimum of 48 while its maximum remains zero upstream.
        for (at, value) in [(0x8c40, 0), (0x8c44, 48), (0x8c4c, 0), (0x8c50, 0)] {
            write(&mut timed, at, value)?;
        }
        for index in 0..11 {
            let pages = match index { 2 => 16, 3 => 48, _ => 0 };
            write(&mut timed, 0x9040 + index * 4, pages | pages << 16)?;
        }
        modify(&mut timed, R_AX_DMAC_FUNC_EN, 0, DLE_ENABLE)?;
        stage = Stage::Ready;
        io::poll32(&mut timed, WDE_READY, 3, 3, 1, 2_000)?;
        io::poll32(&mut timed, PLE_READY, 3, 3, 1, 2_000)?;
        stage = Stage::FlowControl;
        modify(&mut timed, HFC_CONTROL, HFC_ENABLE, 0)?;
        write(&mut timed, CHANNEL_PAGE, 40 << 16)?;
        modify(&mut timed, HFC_CONTROL, 3 << 10, 0)?;
        modify(&mut timed, HFC_CONTROL, HFC_ENABLE, 1 << 3)?;
        timed.delay_us(0)
    })();
    if let Err(cause) = result {
        let hfc_disable = disable(bus, HFC_CONTROL, HFC_ENABLE).err();
        let dle_disable = disable(bus, R_AX_DMAC_FUNC_EN, DLE_ENABLE).err();
        return Err(Failure { stage, cause, hfc_disable, dle_disable });
    }
    Ok(())
}
