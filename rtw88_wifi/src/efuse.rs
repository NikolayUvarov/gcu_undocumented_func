// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2018-2019 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw88.txt.
// Adapted from Linux v6.12 rtw88 efuse.c (rtw_dump_physical_efuse_map, rtw_dump_logical_efuse_map), rtw8821c.h
// (struct rtw8821c_efuse, rtw8821ce_efuse), rtw8821c.c (rtw8821c_read_efuse) and main.h (struct rtw_txpwr_idx).
//! The eFuse: its physical bytes read, decoded into the logical map, and what the board's set-up needs from it.

use crate::io::{Error, Regs};

pub const PHYSICAL: usize = 512;
pub const LOGICAL: usize = 512;
/// The physical map's protected tail, not parsed.
const PROTECTED: usize = 96;

/// The physical map read byte by byte, with the 2.5 V LDO off (read only).
pub fn read<R: Regs>(r: &mut R, phy: &mut [u8; PHYSICAL]) -> Result<(), Error> {
    r.mask32(0x34, 0x300, 0);
    r.clr8(0x37, 1 << 7);
    let mut c = r.r32(0x30);
    for (addr, byte) in phy.iter_mut().enumerate() {
        c = c & !(0xff | 0x3ff << 8) | (addr as u32 & 0x3ff) << 8;
        r.w32(0x30, c & !(1 << 31));
        let mut tries = 0;
        loop {
            r.udelay(1);
            c = r.r32(0x30);
            if c & 1 << 31 != 0 { break; }
            tries += 1;
            if tries > 100_000 { return Err(Error::Timeout { what: "an eFuse byte", at: 0x30, value: c }); }
        }
        *byte = c as u8;
    }
    Ok(())
}

/// The logical map from the physical one: blocks of 8 bytes, each word present unless its enable bit is set.
pub fn decode(phy: &[u8; PHYSICAL], log: &mut [u8; LOGICAL]) -> Result<(), Error> {
    log.fill(0xff);
    let end = PHYSICAL - PROTECTED;
    let mut i = 0;
    while i < end {
        let (h1, h2) = (phy[i], phy[i + 1]);
        if h1 == 0xff || (h1 & 0x1f == 0x0f && h2 == 0xff) { break; }
        let (block, enable) = if h1 & 0x1f == 0x0f {
            i += 2;
            (((h2 & 0xf0) >> 1 | (h1 >> 5) & 7) as usize, h2 & 0xf)
        } else {
            i += 1;
            (((h1 & 0xf0) >> 4) as usize, h1 & 0xf)
        };
        for w in 0..4 {
            if enable & 1 << w != 0 { continue; }
            let at = block * 8 + w * 2;
            if i + 1 > end || at + 1 >= LOGICAL { return Err(Error::Efuse("a word past the map's end")); }
            log[at] = phy[i];
            log[at + 1] = phy[i + 1];
            i += 2;
        }
    }
    Ok(())
}

/// The 2.4 GHz TX power of one path: the CCK and 40 MHz bases by channel group, and the OFDM and 20 MHz differences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Power2g { pub cck: [u8; 6], pub bw40: [u8; 5], pub ofdm_diff: i8, pub bw20_diff: i8 }

/// What the board's set-up takes from the logical map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    pub mac: [u8; 6], pub rfe: u8, pub pkg: u8, pub crystal: u8, pub btcoex: bool, pub shared: bool,
    pub swing_2g: u8, pub swing_5g: u8, pub country: [u8; 2], pub power: Power2g,
}

/// A signed 4-bit difference.
fn nibble(v: u8) -> i8 { if v & 8 != 0 { (v | 0xf0) as i8 } else { v as i8 } }

impl Board {
    pub fn from(log: &[u8; LOGICAL]) -> Board {
        let rfe = log[0xca] & 0x1f;
        // Boards of RFE type 2 or 4 keep 2.4 GHz's power in path B's block, used for path A (rtw8821c_read_efuse).
        let block = if matches!(rfe, 2 | 4) { 0x3a } else { 0x10 };
        let p = &log[block..block + 18];
        let power = Power2g { cck: p[0..6].try_into().unwrap(), bw40: p[6..11].try_into().unwrap(), ofdm_diff: nibble(p[11] & 0xf), bw20_diff: nibble(p[11] >> 4) };
        let or0 = |v: u8| if v == 0xff { 0 } else { v };
        Board {
            mac: log[0xd0..0xd6].try_into().unwrap(), rfe, pkg: (log[0xca] >> 5) & 1, crystal: or0(log[0xb9]),
            btcoex: log[0xc1] & 0xe0 == 0x20, shared: log[0xc3] & 1 != 0, swing_2g: or0(log[0xc6]), swing_5g: or0(log[0xc7]),
            country: [log[0xcb], log[0xcc]], power,
        }
    }

    /// A MAC address the board can use: not group, not all zeros or ones.
    pub fn mac_valid(&self) -> bool { self.mac[0] & 1 == 0 && self.mac != [0; 6] && self.mac != [0xff; 6] }
}
