// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2020, 2024 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Bounded OTP reads and logical-map decoding from pinned efuse.c and rtw8852b_common.h; no OTP writes.

use crate::io::{modify8, modify16, poll32, Fault, FaultKind, RegisterIo, Result, TimedIo};
use crate::power::{xtal_read, xtal_write};
use crate::registers::*;

pub const PHYSICAL_SIZE: usize = 1216;
pub const LOGICAL_SIZE: usize = 2048;
pub const DAV_PHYSICAL_SIZE: usize = 96;
pub const DAV_LOGICAL_SIZE: usize = 16;
pub const PHYCAP_AT: usize = 0x580;
pub const PHYCAP_SIZE: usize = 128;
pub const MAC_AT: usize = 0x400;
pub const CHANNEL_PLAN_AT: usize = 0x2b8;
pub const XTAL_AT: usize = 0x2b9;
pub const RFE_AT: usize = 0x2ca;
pub const COUNTRY_AT: usize = 0x2cb;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadError { pub cause: Fault, pub restore: Option<Fault> }

impl From<Fault> for ReadError {
    fn from(cause: Fault) -> Self { Self { cause, restore: None } }
}

pub struct Maps {
    pub logical: [u8; LOGICAL_SIZE],
    pub dav_logical: [u8; DAV_LOGICAL_SIZE],
    pub phycap: [u8; PHYCAP_SIZE],
}

fn enable_ddv(io: &mut impl RegisterIo) -> Result<()> {
    modify8(io, R_AX_PMC_DBG_CTRL2, 0, B_AX_SYSON_DIS_PMCR_AX_WRMSK as u8)?;
    modify16(io, R_AX_SYS_ISO_CTRL, 0, B_AX_PWC_EV2EF_B14 as u16)?;
    io.delay_us(1_000)?;
    modify16(io, R_AX_SYS_ISO_CTRL, 0, B_AX_PWC_EV2EF_B15 as u16)?;
    modify16(io, R_AX_SYS_ISO_CTRL, B_AX_ISO_EB2CORE as u16, 0)
}

fn disable_ddv(io: &mut impl RegisterIo) -> Result<()> {
    crate::power::identify(io)?;
    modify16(io, R_AX_SYS_ISO_CTRL, 0, B_AX_ISO_EB2CORE as u16)?;
    modify16(io, R_AX_SYS_ISO_CTRL, B_AX_PWC_EV2EF_B15 as u16, 0)?;
    io.delay_us(1_000)?;
    modify16(io, R_AX_SYS_ISO_CTRL, B_AX_PWC_EV2EF_B14 as u16, 0)?;
    modify8(io, R_AX_PMC_DBG_CTRL2, B_AX_SYSON_DIS_PMCR_AX_WRMSK as u8, 0)
}

pub fn read_ddv(io: &mut impl RegisterIo, start: usize, output: &mut [u8]) -> core::result::Result<(), ReadError> {
    if output.is_empty() || start.checked_add(output.len()).is_none_or(|end| end > 0x800) {
        return Err(Fault::new(FaultKind::Efuse, R_AX_EFUSE_CTRL, start as u32).into());
    }
    let read = (|| {
        let mut timed = TimedIo::new(io, 4_000_000);
        enable_ddv(&mut timed)?;
        for (i, byte) in output.iter_mut().enumerate() {
            let command = ((start + i) as u32) << B_AX_EF_ADDR_MASK.trailing_zeros();
            timed.write32(R_AX_EFUSE_CTRL, command & !B_AX_EF_RDY)?;
            timed.delay_us(1)?;
            *byte = poll32(&mut timed, R_AX_EFUSE_CTRL, B_AX_EF_RDY, B_AX_EF_RDY, 1, 1_000_000)? as u8;
        }
        Ok(())
    })();
    let restore = disable_ddv(&mut TimedIo::new(io, 100_000)).err();
    match (read, restore) {
        (Ok(()), None) => Ok(()),
        (Ok(()), Some(cause)) => Err(ReadError { cause, restore: Some(cause) }),
        (Err(cause), restore) => Err(ReadError { cause, restore }),
    }
}

fn read_dav(io: &mut impl RegisterIo, output: &mut [u8; DAV_PHYSICAL_SIZE]) -> Result<()> {
    let mut timed = TimedIo::new(io, 3_000_000);
    for (address, byte) in output.iter_mut().enumerate() {
        xtal_write(&mut timed, XTAL_SI_CTRL as u8, 0x40, 0xff)?;
        xtal_write(&mut timed, XTAL_SI_LOW_ADDR as u8, address as u8, XTAL_SI_LOW_ADDR_MASK as u8)?;
        xtal_write(&mut timed, XTAL_SI_CTRL as u8, (address >> 8) as u8, XTAL_SI_HIGH_ADDR_MASK as u8)?;
        xtal_write(&mut timed, XTAL_SI_CTRL as u8, 0, XTAL_SI_MODE_SEL_MASK as u8)?;
        let mut byte_read = TimedIo::new(&mut timed, 10_000);
        let mut ready = false;
        for _ in 0..=10_001 {
            let value = xtal_read(&mut byte_read, XTAL_SI_CTRL as u8)?;
            if value & XTAL_SI_RDY as u8 != 0 { ready = true; break; }
            byte_read.delay_us(1)?;
        }
        if !ready { return Err(Fault::new(FaultKind::Timeout, XTAL_SI_CTRL as usize, address as u32)); }
        *byte = xtal_read(&mut byte_read, XTAL_SI_READ_VAL as u8)?;
    }
    Ok(())
}

/// Decode each map using its own physical and logical bounds, including the shorter DAV region.
pub fn decode(physical: &[u8], logical: &mut [u8]) -> Result<()> {
    logical.fill(0xff);
    let end = physical.len().checked_sub(4).filter(|end| *end >= 4).ok_or(Fault::new(FaultKind::Efuse, 0, physical.len() as u32))?;
    let mut at = 4;
    while at < end {
        let header = physical.get(at..at + 2).filter(|_| at + 2 <= end).ok_or(Fault::new(FaultKind::Efuse, at, 0))?;
        if header[0] == 0xff || header[1] == 0xff { return Ok(()); }
        let block = (((header[0] & 0x0f) as usize) << 4) | ((header[1] >> 4) as usize);
        let enabled = header[1] & 0x0f;
        at += 2;
        for word in 0..4 {
            if enabled & (1 << word) != 0 { continue; }
            let target = block * 8 + word * 2;
            if at + 2 > end || target + 2 > logical.len() { return Err(Fault::new(FaultKind::Efuse, at, target as u32)); }
            logical[target..target + 2].copy_from_slice(&physical[at..at + 2]);
            at += 2;
        }
    }
    Ok(())
}

pub fn read_maps(io: &mut impl RegisterIo) -> core::result::Result<Maps, ReadError> {
    let autoload = io.read16(R_AX_SYS_WL_EFUSE_CTRL)?;
    if autoload & B_AX_AUTOLOAD_SUS as u16 == 0 { return Err(Fault::new(FaultKind::Autoload, R_AX_SYS_WL_EFUSE_CTRL, autoload as u32).into()); }
    let mut physical = [0xff; PHYSICAL_SIZE];
    let mut dav = [0xff; DAV_PHYSICAL_SIZE];
    let mut maps = Maps { logical: [0xff; LOGICAL_SIZE], dav_logical: [0xff; DAV_LOGICAL_SIZE], phycap: [0xff; PHYCAP_SIZE] };
    read_ddv(io, 0, &mut physical)?;
    read_dav(io, &mut dav)?;
    decode(&physical, &mut maps.logical)?;
    decode(&dav, &mut maps.dav_logical)?;
    read_ddv(io, PHYCAP_AT, &mut maps.phycap)?;
    Ok(maps)
}
