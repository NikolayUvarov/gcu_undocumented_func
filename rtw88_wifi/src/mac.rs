// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2018-2019 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw88.txt.
// Adapted from Linux v6.12 rtw88 mac.c (txdma_queue_mapping, set_trx_fifo_info, __priority_queue_cfg, init_h2c,
// rtw_drv_info_cfg, rtw_set_channel_mac), rtw8821c.c (rtw8821c_mac_init, rqpn_table_8821c[1], page_table_8821c[1]),
// main.c and mac80211.c (the port's address, BSSID, network type and beacon control), the values resolved for PCIe.
//! The MAC's set-up after the firmware runs, and the station's address, BSSID and receive configuration.

use crate::io::{Error, Regs};

/// The reserved pages' boundary: 512 pages of 128 bytes, the top 52 reserved.
pub const BOUNDARY: u16 = 460;
/// The H2C queue's place in the TX FIFO, and its size.
const H2CQ: u32 = 500 << 7;
const H2CQ_BYTES: u32 = 8 << 7;

/// rtw_mac_init for the 8821C on PCIe: queue mapping, FIFO pages, the H2C queue, the MAC's own set-up, the RX driver
/// info. `rx_limit`: REG_RX_PKT_LIMIT in 512-byte units, for the RX buffers this driver has.
pub fn init<R: Regs>(r: &mut R, rx_limit: u8) -> Result<(), Error> {
    // txdma_queue_mapping
    r.w16(0x10c, 0xc5a0);
    r.w8(0x100, 0);
    r.w8(0x100, 0xff);
    r.w32(0x1330, 1 << 31);
    // __priority_queue_cfg: HPQ, LPQ, NPQ, EXQ, PUBQ pages
    r.w16(0x230, 16);
    r.w16(0x234, 16);
    r.w16(0x238, 16);
    r.w16(0x23c, 14);
    r.w16(0x240, 397);
    r.set32(0x22c, 1 << 31);
    r.w16(0x204, BOUNDARY);
    r.set8(0x422, 1 << 4);
    r.w16(0x424, BOUNDARY);
    r.w16(0x206, BOUNDARY);
    r.w16(0x456, BOUNDARY);
    r.w32(0x11c, 16384 - 256 - 1);
    r.set8(0x208, 1);
    r.poll32(0x208, 1, 0, 1000, 10, "the LLT's set-up")?;
    r.w8(0x103, 0);
    // init_h2c
    for at in [0x244, 0x24c] {
        let v = r.r32(at);
        r.w32(at, v & 0xfffc_0000 | H2CQ);
    }
    let v = r.r32(0x248);
    r.w32(0x248, v & 0xfffc_0000 | (H2CQ + H2CQ_BYTES));
    r.mask8(0x254, 0x03, 1);
    r.mask8(0x254, 0x04, 1);
    r.mask8(0x20d, 0x80, 1);
    let (wp, rp) = (r.r32(0x10d4) & 0x3ffff, r.r32(0x10d0) & 0x3ffff);
    let free = if wp >= rp { H2CQ_BYTES - (wp - rp) } else { rp - wp };
    if free != H2CQ_BYTES { return Err(Error::Chip("the H2C queue is not empty after its set-up")); }
    // rtw8821c_mac_init
    r.w8(0x455, 0x70);
    r.set8(0x45e, 1 << 2);
    r.w8(0x4e5, 0xe4);
    r.w8(0x4e6, 0x09);
    r.w32(0x4c8, 0x2020_08ff);
    r.w16(0x4ce, 0x0801);
    for at in [0x1448, 0x144a, 0x144c, 0x144e] { r.w8(at, 6); }
    r.set8(0x480, 1 << 5);
    r.clr8(0x5b4, 0x70);
    r.w16(0x522, 0);
    r.w8(0x51b, 0x09);
    r.w8(0x512, 0x19);
    r.w32(0x514, 0x1010_0e0a);
    r.w16(0x502, 0x0186);
    r.w16(0x506, 0x03bc);
    r.w32(0x544, 0x001b_0005);
    r.w16(0x55e, 0x3030);
    r.set8(0x550, 1 << 3);
    r.w32(0x540, 0x0000_6404);
    r.w8(0x558, 0x04);
    r.w8(0x559, 0x02);
    r.clr8(0x521, 1 << 4);
    r.w32(0x6a0, 0x0fff_ffff);
    r.w16(0x6a4, 0xffff);
    r.w32(0x608, 0xe400_220e);
    r.w8(0x60c, rx_limit);
    r.w8(0x606, 0x30);
    r.w8(0x605, 0x30);
    r.w8(0x639, 0x40);
    r.set8(0x66c, 1 << 1);
    r.set8(0x718, 1 << 6);
    r.w32(0x7d8, 0xb081_0041);
    r.w8(0x7d4, 0x98);
    // rtw_drv_info_cfg
    r.w8(0x60f, 4);
    r.mask8(0x115, 0x0f, 0x0f);
    r.set32(0x608, 1 << 28);
    r.clr32(0x7d4, 1 << 8 | 1 << 9);
    Ok(())
}

/// The RCR as rtw88 leaves it after start (FCS, PHY status appended; broadcast, multicast and own unicast accepted);
/// with `bssid_beacons`, beacons only from the BSSID.
pub fn rcr<R: Regs>(r: &mut R, bssid_beacons: bool) { r.w32(0x608, 0xf410_400e | (bssid_beacons as u32) << 7); }

/// Port 0's own address.
pub fn set_mac<R: Regs>(r: &mut R, mac: &[u8; 6]) { for (i, b) in mac.iter().enumerate() { r.w8(0x610 + i as u32, *b); } }

/// Port 0's BSSID, and its network type: 2 (a station linked) with one, 0 without.
pub fn set_bssid<R: Regs>(r: &mut R, bssid: &[u8; 6]) {
    for (i, b) in bssid.iter().enumerate() { r.w8(0x618 + i as u32, *b); }
    r.mask32(0x100, 0x30000, if *bssid == [0; 6] { 0 } else { 2 });
}

/// The station's port as rtw88's add_interface leaves it: no link, beacon function on.
pub fn station<R: Regs>(r: &mut R, mac: &[u8; 6]) {
    set_mac(r, mac);
    r.mask32(0x100, 0x30000, 0);
    r.w8(0x550, 0x08);
}

/// rtw_set_channel_mac for 20 MHz.
pub fn channel<R: Regs>(r: &mut R, ch: u8) {
    r.w8(0x483, 0);
    r.clr32(0x668, 1 << 7 | 1 << 8);
    r.clr32(0x24, 1 << 20 | 1 << 21);
    r.w8(0x55c, 80);
    r.w8(0x638, 80);
    let v = r.r8(0x454) & !(1 << 7);
    r.w8(0x454, if ch > 35 { v | 1 << 7 } else { v });
}
