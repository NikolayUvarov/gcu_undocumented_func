// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2018-2019 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw88.txt.
// Adapted from Linux v6.12 rtw88 phy.c (rtw_phy_read_rf, rtw_phy_write_rf_reg_sipi, the TX power index and its
// channel groups), rtw8821c.c (rtw8821c_phy_set_param, set_channel_bb, set_channel_bb_swing, set_channel_rf,
// rtw8821c_switch_rf_set, set_channel_rxdfir, rtw8821c_set_tx_power_index_by_rate, rtw8821c_do_iqk) and
// rtw8821c_table.c (power by rate); the world-wide 20 MHz power limits are the minimum over rtw8821c_txpwr_lmt_type0's
// regulatory domains, as phy.c computes RTW_REGD_WW.
//! The baseband and RF: their set-up and tables, the crystal, a 2.4 GHz channel at 20 MHz, and its TX power.

use crate::cond::Chip;
use crate::efuse::Board;
use crate::io::{Error, Regs};
use crate::load;

/// RF path A's register read whole (rtw_phy_read_rf: its direct window).
pub fn rf_read<R: Regs>(r: &mut R, at: u32) -> u32 { r.r32(0x2800 + (at & 0xff) * 4) & 0xfffff }

/// RF path A's register written through the SIPI port (rtw_phy_write_rf_reg_sipi); a field read, changed and written.
pub fn rf_write<R: Regs>(r: &mut R, at: u32, mask: u32, value: u32) {
    let data = if mask == 0xfffff { value } else {
        let old = rf_read(r, at);
        old & !mask | (value << mask.trailing_zeros()) & mask
    };
    r.w32(0xc90, ((at & 0xff) << 20 | data & 0xfffff) & 0x0fff_ffff);
    r.udelay(13);
}

struct Tables<'a, R: Regs>(&'a mut R);

impl<R: Regs> load::Io for Tables<'_, R> {
    type Error = Error;
    fn write8(&mut self, at: u32, value: u8) -> Result<(), Error> { self.0.w8(at, value); Ok(()) }
    fn write32(&mut self, at: u32, value: u32) -> Result<(), Error> { self.0.w32(at, value); Ok(()) }
    fn write_rf(&mut self, at: u32, value: u32) -> Result<(), Error> { rf_write(self.0, at, 0xfffff, value); Ok(()) }
    fn delay_us(&mut self, us: u32) -> Result<(), Error> {
        if us >= 1000 { self.0.msleep(us / 1000) } else { self.0.udelay(us) }
        Ok(())
    }
}

/// The CCK TX filter as the tables left it, kept for channels 1 to 13.
#[derive(Clone, Copy, Default)]
pub struct CckFilter([u32; 3]);

/// rtw8821c_phy_set_param: the baseband and RF enabled and reset, the tables, the crystal; the CCK filter kept.
pub fn init<R: Regs>(r: &mut R, chip: &Chip, b: &Board) -> Result<CckFilter, Error> {
    let mut v = r.r8(0x02);
    v |= 1 << 6; r.w8(0x02, v);
    v |= 0x03; r.w8(0x02, v);
    v &= !0x03; r.w8(0x02, v);
    v |= 0x03; r.w8(0x02, v);
    r.w8(0x1f, 0x07);
    r.udelay(10);
    r.w8(0xef, 0x07);
    r.udelay(10);
    r.clr32(0x808, 1 << 28 | 1 << 29);
    load::all(&mut Tables(r), chip, matches!(b.rfe, 2 | 4))?;
    let xtal = (b.crystal & 0x3f) as u32;
    r.mask32(0x24, 0x7e00_0000, xtal);
    r.mask32(0x28, 0x0000_007e, xtal);
    r.mask32(0xa2c, 1 << 18 | 1 << 22, 0);
    r.set32(0x808, 1 << 28 | 1 << 29);
    Ok(CckFilter([r.r32(0xa24), r.r32(0xa28), r.r32(0xaac)]))
}

/// rtw8821c_switch_rf_set: the RFE control for Wi-Fi on 2.4 GHz, through the BTG front end on boards that have one.
fn switch_rf<R: Regs>(r: &mut R, btg: bool) {
    r.set32(0x1080, 1 << 16);
    r.set32(0x0000, 1 << 26);
    let mut v = r.r32(0xcb8);
    if btg {
        v |= 1 << 16;
        v &= !(1 << 18 | 1 << 20 | 1 << 22 | 1 << 21 | 1 << 23);
        r.mask32(0xa84, 0x00ff_0000, 0x0e);
        r.mask32(0xa80, 0x0000_ffff, 0xfc84);
    } else {
        v |= 1 << 20 | 1 << 22 | 1 << 21;
        v &= !(1 << 16 | 1 << 18 | 1 << 23);
        r.mask32(0xa84, 0x00ff_0000, 0x12);
        r.mask32(0xa80, 0x0000_ffff, 0x7532);
    }
    r.w32(0xcb8, v);
}

/// rtw8821c_set_channel at 20 MHz on 2.4 GHz: baseband, its swing, MAC, RF, RX DFIR; then the TX power.
pub fn channel<R: Regs>(r: &mut R, ch: u8, b: &Board, filter: &CckFilter) -> Result<(), Error> {
    if !(1..=13).contains(&ch) { return Err(Error::Chip("not a 2.4 GHz channel this driver serves")); }
    // set_channel_bb
    r.mask32(0x808, 1 << 28, 1);
    r.mask32(0x454, 1 << 7, 0);
    r.mask32(0xa80, 1 << 18, 0);
    r.mask32(0x814, 0xfc00, 15);
    r.mask32(0xc1c, 0xf00, 0);
    r.mask32(0x860, 0x1ffe_0000, 0x96a);
    r.w32(0xa24, filter.0[0]);
    r.mask32(0xa28, 0xffff, filter.0[1] & 0xffff);
    r.w32(0xaac, filter.0[2]);
    let v = r.r32(0x8ac);
    r.w32(0x8ac, v & 0xffcf_fc00 | 0x1001_0000);
    r.mask32(0x8c4, 1 << 30, 1);
    // set_channel_bb_swing
    let swing = if b.swing_2g > 9 { 0 } else { b.swing_2g };
    r.mask32(0xc1c, 0xffe0_0000, [0x200, 0x16a, 0x101, 0x0b6][swing as usize / 3]);
    crate::mac::channel(r, ch);
    // set_channel_rf
    let mut rf = rf_read(r, 0x18);
    rf &= !(1 << 16 | 1 << 9 | 1 << 8 | 0xff | 1 << 18 | 1 << 17 | 1 << 11 | 1 << 10);
    rf |= ch as u32 | 1 << 11 | 1 << 10;
    switch_rf(r, matches!(b.rfe, 2 | 4 | 7 | 0xa | 0xc | 0xf));
    rf_write(r, 0xdf, 1 << 6, 1);
    rf_write(r, 0x64, 0xf, 0xf);
    rf_write(r, 0x18, 0xfffff, rf);
    rf_write(r, 0xb8, 1 << 19, 0);
    rf_write(r, 0xb8, 1 << 19, 1);
    // set_channel_rxdfir at 20 MHz
    r.mask32(0x948, 0x3000_0000, 2);
    r.mask32(0x94c, 0x3000_0000, 2);
    r.mask32(0xc20, 1 << 31, 1);
    r.mask32(0x8f0, 1 << 31, 0);
    power(r, ch, b);
    Ok(())
}

/// The channel group of a 2.4 GHz channel (rtw_get_channel_group): CCK's, then the others'.
fn group(ch: u8) -> usize { match ch { 1..=2 => 0, 3..=5 => 1, 6..=8 => 2, 9..=11 => 3, _ => 4 } }

/// Power by rate (rtw8821c_bb_pg_type0, 2.4 GHz) above each section's base rate, in index steps: CCK 1, 2, 5.5,
/// 11 Mb/s; OFDM 6 to 54 Mb/s.
const BY_RATE_CCK: [i16; 4] = [6, 4, 2, 0];
const BY_RATE_OFDM: [i16; 8] = [8, 8, 8, 8, 6, 4, 2, 0];
/// The world-wide 20 MHz limits on channels 1 to 13, above each section's base power by rate.
const LIMIT_CCK: [i16; 13] = [-4, -4, -4, -4, -4, -4, -4, -4, -4, -4, -4, -8, -16];
const LIMIT_OFDM: [i16; 13] = [2, 2, 2, 2, 2, 2, 2, 2, 2, -2, -6, -10, -26];
/// The base used when the eFuse holds no power (0xff): a modest one, not the maximum Linux would clamp to.
const BLANK_BASE: i16 = 0x20;

/// The power indices of CCK's four rates and OFDM's eight on channel `ch`: the board's base, plus the power by rate,
/// at most the world-wide limit; 0 to 63 (phy.c's index without power tracking or SAR).
pub fn indices(ch: u8, b: &Board) -> ([u8; 4], [u8; 8]) {
    let g = group(ch);
    let base = |v: u8| if v == 0xff { BLANK_BASE } else { v as i16 };
    let cck_base = base(b.power.cck[g]);
    let ofdm_base = if b.power.bw40[g] == 0xff { BLANK_BASE } else { base(b.power.bw40[g]) + b.power.ofdm_diff as i16 };
    let at = (ch - 1) as usize;
    let idx = |base: i16, by_rate: i16, limit: i16| (base + by_rate.min(limit)).clamp(0, 0x3f) as u8;
    (core::array::from_fn(|i| idx(cck_base, BY_RATE_CCK[i], LIMIT_CCK[at])), core::array::from_fn(|i| idx(ofdm_base, BY_RATE_OFDM[i], LIMIT_OFDM[at])))
}

/// The indices written: CCK at 0x1d00, OFDM at 0x1d04 and 0x1d08, four rates a word.
fn power<R: Regs>(r: &mut R, ch: u8, b: &Board) {
    let (cck, ofdm) = indices(ch, b);
    r.w32(0x1d00, u32::from_le_bytes(cck));
    r.w32(0x1d04, u32::from_le_bytes(ofdm[0..4].try_into().unwrap()));
    r.w32(0x1d08, u32::from_le_bytes(ofdm[4..8].try_into().unwrap()));
}

/// The firmware's IQK done: RF register 0x08 reads 0xabcde (rtw8821c_do_iqk), then cleared. The H2C packet that starts
/// it is the caller's.
pub fn iqk_wait<R: Regs>(r: &mut R) -> Result<u32, Error> {
    for _ in 0..300 {
        if rf_read(r, 0x08) == 0xabcde {
            rf_write(r, 0x08, 0xfffff, 0);
            return Ok(r.r32(0x1bf0));
        }
        r.msleep(20);
    }
    Err(Error::Timeout { what: "the firmware's IQK", at: 0x08, value: rf_read(r, 0x08) })
}
