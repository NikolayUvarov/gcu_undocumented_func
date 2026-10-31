// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2018-2019 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw88.txt.
// Adapted from Linux v6.12 rtw88 util.c (ltecoex_read_reg, ltecoex_reg_write), coex.c (rtw_coex_power_on_setting,
// __rtw_coex_init_hw_config with wifi_only, rtw_coex_set_ant_path for COEX_SET_ANT_WONLY and COEX_SET_ANT_2G_FREERUN,
// rtw_coex_action_freerun's table, rtw_coex_set_gnt_wl/bt, rtw_coex_set_wl_pri_mask, rtw_coex_table) and rtw8821c.c
// (rtw8821c_coex_cfg_init, _cfg_ant_switch, _cfg_gnt_debug, table_nsant_8821c).
//! The antenna the Wi-Fi shares with the card's Bluetooth: the Wi-Fi's alone on a card without Bluetooth, as rtw88's
//! Wi-Fi-only path does; on a card whose Bluetooth has an antenna of its own, rtw88's free run (700-DRV-0046).

use crate::io::{Error, Regs};

fn lte_ready<R: Regs>(r: &mut R) -> Result<(), Error> { r.poll32(0x1700, 1 << 29, 1 << 29, 1000, 10, "the LTE coexistence register") }

/// An LTE coexistence register, read through its window.
pub fn lte_read<R: Regs>(r: &mut R, at: u32) -> Result<u32, Error> {
    lte_ready(r)?;
    r.w32(0x1700, 0x800f_0000 | at);
    Ok(r.r32(0x1708))
}

pub fn lte_write<R: Regs>(r: &mut R, at: u32, value: u32) -> Result<(), Error> {
    lte_ready(r)?;
    r.w32(0x1704, value);
    r.w32(0x1700, 0xc00f_0000 | at);
    Ok(())
}

/// What the board says of the Bluetooth: its RFE type, whether the card has one (`btcoex`), whether the antenna is shared.
pub struct Board { pub rfe: u8, pub bt: bool, pub shared: bool }

/// How the antenna is set at start: the Wi-Fi's alone, or rtw88's free run beside a Bluetooth on its own antenna. A
/// shared antenna needs rtw88's arbitration by the Bluetooth's state, which this driver does not run: the Wi-Fi keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { WifiOnly, FreeRun }

impl Board {
    pub fn mode(&self) -> Mode { if self.bt && !self.shared { Mode::FreeRun } else { Mode::WifiOnly } }
}

/// rtw_coex_power_on_setting, the parts that write.
pub fn power_on<R: Regs>(r: &mut R, b: &Board) {
    r.set8(0x02, 0x03);
    if b.shared {
        r.w32(0x6c0, 0x5555_5555);
        r.w32(0x6c4, 0x5555_5555);
        r.w32(0x6c8, 0xf0ff_ffff);
    }
    r.w8(0xff1a, 0);
    r.clr32(0x64, 1 << 20);
    r.clr32(0x64, 1 << 24);
    r.clr32(0x40, 1 << 19);
    r.clr32(0x64, 1 << 15);
    r.clr32(0x70, 1 << 18);
    r.clr32(0x70, 1 << 27);
}

/// The board's RF calibration and the Bluetooth's not under way, on boards with a BTG front end (rtw_coex_check_rfk).
fn rfk_idle<R: Regs>(r: &mut R, b: &Board) {
    if !matches!(b.rfe, 2 | 4) { return; }
    for _ in 0..60 {
        if r.r16(0xaa) & 1 << 5 == 0 && r.r8(0x49c) & 1 == 0 { return; }
        r.msleep(10);
    }
}

/// __rtw_coex_init_hw_config with the board's mode. Wi-Fi alone: the Wi-Fi granted the antenna and its switch set to it.
/// Free run (COEX_SET_ANT_2G_FREERUN with table 100): the Wi-Fi still granted by software, the Bluetooth by the PTA, the
/// switch at WLG_BT, and the PTA's tables open to both. The H2C command it sends (BT_WIFI_CONTROL) is the caller's.
pub fn init<R: Regs>(r: &mut R, b: &Board) -> Result<(), Error> {
    let free = b.mode() == Mode::FreeRun;
    r.set8(0x550, 1 << 3);
    r.mask8(0x790, 0x3f, 5);
    r.w8(0x778, 1);
    r.set32(0x40, 1 << 5);
    r.set32(0x40, 1 << 9);
    r.set8(0x4c6, 1 << 4);
    r.clr8(0x4c6, 1 << 5);
    r.set16(0x762, 1 << 12);
    r.mask8(0x6cf, 1 << 3, 1);
    r.set8(0x6cc, 1 << 3 | 1 << 4);
    r.set8(0x6cf, 1 << 3);
    rfk_idle(r, b);
    // GNT_WL software high in both its fields; GNT_BT software low, or the PTA's (0) in free run.
    let v = lte_read(r, 0x38)?;
    lte_write(r, 0x38, v & !0xff00 | if free { 0x3300 } else { 0x7700 })?;
    r.set8(0x73, 1 << 2);
    if !matches!(b.rfe, 5 | 6 | 13 | 14) {
        r.clr32(0x4c, 1 << 23);
        r.set32(0x4c, 1 << 24);
        r.mask8(0xcb4, 0xff, 0x77);
        // WLG, or WLG_BT in free run (3 on module types other than 2 and 4, whose switch is the other way).
        r.mask32(0xcb4, 0xf000_0000, if !free { 2 } else if matches!(b.rfe, 2 | 4) { 2 } else { 3 });
        r.set8(0x67, 1 << 5);
        r.set8(0x67, 1 << 4);
    }
    r.w16(0xaa, 0x8003);
    if free {
        r.w32(0x6c0, 0xffff_ffff);
        r.w32(0x6c4, 0xffff_ffff);
        r.w32(0x6c8, 0xf0ff_ffff);
    }
    Ok(())
}

/// The H2C mailbox command BT_WIFI_CONTROL (0x69) with WL_LEAKAP's opcode and DIS_5MS, as the Wi-Fi-only set-up sends.
pub const H2C_BT_WIFI_CONTROL: [u8; 8] = [0x69, 0x0c, 0x01, 0, 0, 0, 0, 0];
