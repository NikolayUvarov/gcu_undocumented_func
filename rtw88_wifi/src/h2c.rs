// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2018-2019 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw88.txt.
// Adapted from Linux v6.12 rtw88 fw.c (rtw_fw_send_h2c_command, rtw_fw_send_h2c_packet, rtw_fw_send_general_info,
// rtw_fw_send_phydm_info, rtw_fw_do_iqk) and fw.h (the commands' formats).
//! Commands to the firmware: 8-byte ones through the mailboxes, 32-byte packets through the H2C ring.

use crate::dma::{Region, Tx, TX_DESC};
use crate::io::{Error, Regs};
use crate::txrx;

/// The mailbox next used and the packets' sequence: both start at 0 after each firmware download.
pub struct H2c { pub box_: u8, pub seq: u16 }

impl H2c {
    pub fn new() -> Self { Self { box_: 0, seq: 0 } }

    /// An 8-byte command through the next mailbox.
    pub fn command<R: Regs>(&mut self, r: &mut R, c: &[u8; 8]) -> Result<(), Error> {
        let b = self.box_ as u32;
        let mut free = false;
        for _ in 0..30 {
            if r.r8(0x1cc) & 1 << b == 0 { free = true; break; }
            r.udelay(100);
        }
        if !free { return Err(Error::Timeout { what: "an H2C mailbox", at: 0x1cc, value: r.r8(0x1cc) as u32 }); }
        r.w32(0x1f0 + 4 * b, u32::from_le_bytes(c[4..8].try_into().unwrap()));
        r.w32(0x1d0 + 4 * b, u32::from_le_bytes(c[0..4].try_into().unwrap()));
        self.box_ = (self.box_ + 1) & 3;
        Ok(())
    }

    /// A packet command `sub` with `payload` (at most 24 bytes) through the H2C ring.
    pub fn packet<R: Regs>(&mut self, r: &mut R, region: &mut Region, ring: &mut Tx, sub: u8, payload: &[u8]) -> Result<(), Error> {
        if !ring.room(r) { return Err(Error::Chip("the H2C ring is full")); }
        let slot = ring.slot(region);
        txrx::h2c_desc(&mut slot[..TX_DESC]);
        let p = &mut slot[TX_DESC..TX_DESC + 32];
        p.fill(0);
        p[0..4].copy_from_slice(&(0x01 | 0xff << 8 | (sub as u32) << 16).to_le_bytes());
        p[4..8].copy_from_slice(&((8 + payload.len()) as u32 | (self.seq as u32) << 16).to_le_bytes());
        p[8..8 + payload.len()].copy_from_slice(payload);
        self.seq = self.seq.wrapping_add(1);
        ring.post(r, region, 32);
        Ok(())
    }
}

/// The mailbox command RECOVER_BT_DEV (0xd1), sent after each download to an 8821C on PCIe.
pub const RECOVER_BT_DEV: [u8; 8] = [0xd1, 1, 0, 0, 0, 0, 0, 0];

/// GENERAL_INFO (0x0d): the firmware's TX buffer, in pages above the reserved boundary (508 − 460).
pub fn general_info() -> [u8; 4] { [0, 0, 48, 0] }

/// PHYDM_INFO (0x11): the RFE type, one TX and one RX path, the cut, then two fields rtw88 sets to 1.
pub fn phydm_info(rfe: u8, cut: u8) -> [u8; 8] { [rfe, 4, cut, 0x11, 0, 0, 0, 0] }

/// IQK (0x0e): neither cleared nor a segment.
pub fn iqk() -> [u8; 1] { [0] }
