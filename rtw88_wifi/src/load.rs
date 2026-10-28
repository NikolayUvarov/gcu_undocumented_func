// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2018-2019 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw88.txt.
// Adapted from Linux v6.12 rtw88 phy.c (rtw_phy_cfg_mac, _agc, _bb, _rf, rtw_phy_load_tables).
//! The tables written to the chip: MAC entries as bytes, BB and AGC entries as words, RF entries through the RF path;
//! some addresses are delays, not registers.

use crate::cond::{select, Chip};
use crate::tables;

/// What loading the tables needs of the device.
pub trait Io {
    type Error;
    fn write8(&mut self, at: u32, value: u8) -> Result<(), Self::Error>;
    fn write32(&mut self, at: u32, value: u32) -> Result<(), Self::Error>;
    /// The RF register `at` of path A written whole (RFREG_MASK, 20 bits).
    fn write_rf(&mut self, at: u32, value: u32) -> Result<(), Self::Error>;
    fn delay_us(&mut self, us: u32) -> Result<(), Self::Error>;
}

/// A BB entry: addresses 0xf9 to 0xfe are delays (rtw_phy_cfg_bb).
fn bb<I: Io>(io: &mut I, at: u32, value: u32) -> Result<(), I::Error> {
    match at {
        0xfe => io.delay_us(50_000),
        0xfd => io.delay_us(5_000),
        0xfc => io.delay_us(1_000),
        0xfb => io.delay_us(50),
        0xfa => io.delay_us(5),
        0xf9 => io.delay_us(1),
        _ => io.write32(at, value),
    }
}

/// An RF entry: 0xffe and 0xfe are delays; a write waits 1 µs (rtw_phy_cfg_rf).
fn rf<I: Io>(io: &mut I, at: u32, value: u32) -> Result<(), I::Error> {
    match at {
        0xffe => io.delay_us(50_000),
        0xfe => io.delay_us(100),
        _ => { io.write_rf(at, value)?; io.delay_us(1) }
    }
}

/// The MAC, BB, AGC and RF tables in rtw_phy_load_tables' order (MAC entries a byte each); `btg` adds the AGC table of
/// an RFE type with a BTG front end. The 8821C has no RF calibration table.
pub fn all<I: Io>(io: &mut I, chip: &Chip, btg: bool) -> Result<(), I::Error> {
    select(&tables::MAC, chip, |at, value| io.write8(at, value as u8))?;
    select(&tables::BB, chip, |at, value| bb(io, at, value))?;
    select(&tables::AGC, chip, |at, value| io.write32(at, value))?;
    if btg { select(&tables::AGC_BTG_TYPE2, chip, |at, value| io.write32(at, value))?; }
    select(&tables::RF_A, chip, |at, value| rf(io, at, value))
}
