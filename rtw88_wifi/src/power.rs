// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2018-2019 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw88.txt.
// Adapted from Linux v6.12 rtw88 mac.c (rtw_pre_system_cfg, rtw_pwr_seq_parser, rtw_mac_power_switch,
// rtw_mac_init_system_cfg, rtw_mac_power_on) and rtw8821c.c (card_enable_flow_8821c, card_disable_flow_8821c), the
// entries for PCIe only.
//! The MAC's power: the system set-up before it, the power sequences, and the set-up after.

use crate::io::{Error, Regs};

/// One entry of a power sequence: a byte register's field written, or waited for.
#[derive(Clone, Copy)]
pub struct Step { pub at: u16, pub mask: u8, pub value: u8, pub poll: bool }

const fn w(at: u16, mask: u8, value: u8) -> Step { Step { at, mask, value, poll: false } }
const fn p(at: u16, mask: u8, value: u8) -> Step { Step { at, mask, value, poll: true } }

/// trans_carddis_to_cardemu_8821c then trans_cardemu_to_act_8821c, PCIe entries.
pub const ENABLE: [Step; 18] = [
    w(0x0005, 0x98, 0x00), w(0x0300, 0xff, 0x00), w(0x0301, 0xff, 0x00), w(0x0005, 0x1c, 0x00),
    w(0x0075, 0x01, 0x01), p(0x0006, 0x02, 0x02), w(0x0075, 0x01, 0x00), w(0x0006, 0x01, 0x01),
    w(0x0005, 0x80, 0x00), w(0x0005, 0x18, 0x00), w(0x0005, 0x01, 0x01), p(0x0005, 0x01, 0x00),
    w(0x0020, 0x08, 0x08), w(0x0074, 0x20, 0x20), w(0x0022, 0x02, 0x00), w(0x0062, 0xe0, 0xe0),
    w(0x0061, 0xe0, 0x00), w(0x007c, 0x02, 0x00),
];

/// trans_act_to_cardemu_8821c then trans_cardemu_to_carddis_8821c, PCIe entries.
pub const DISABLE: [Step; 12] = [
    w(0x0093, 0x08, 0x00), w(0x001f, 0xff, 0x00), w(0x0049, 0x02, 0x00), w(0x0006, 0x01, 0x01),
    w(0x0002, 0x02, 0x00), w(0x0005, 0x02, 0x02), p(0x0005, 0x02, 0x00), w(0x0020, 0x08, 0x00),
    w(0x0067, 0x20, 0x00), w(0x0005, 0x04, 0x04), w(0x0081, 0xc0, 0x00), w(0x0090, 0x02, 0x00),
];

/// rtw_pwr_cmd_polling: 20000 reads 50 µs apart; on PCIe, once more after REG_SYS_PW_CTRL bit 3 toggled.
fn wait<R: Regs>(r: &mut R, s: &Step) -> Result<(), Error> {
    for again in [false, true] {
        for _ in 0..20_000 {
            if r.r8(s.at as u32) & s.mask == s.value & s.mask { return Ok(()); }
            r.udelay(50);
        }
        if !again {
            let v = r.r8(0x04);
            r.w8(0x04, v | 0x08);
            r.w8(0x04, v & !0x08);
        }
    }
    Err(Error::Timeout { what: "power sequence", at: s.at as u32, value: r.r8(s.at as u32) as u32 })
}

pub fn run<R: Regs>(r: &mut R, steps: &[Step]) -> Result<(), Error> {
    for s in steps {
        if s.poll { wait(r, s)?; } else {
            let v = r.r8(s.at as u32);
            r.w8(s.at as u32, v & !s.mask | s.value & s.mask);
        }
    }
    Ok(())
}

/// rtw_pre_system_cfg for PCIe.
fn pre_system<R: Regs>(r: &mut R) {
    r.w8(0x1c, 0);
    r.set32(0x74, 1 << 8);
    r.set32(0x64, 1 << 29 | 1 << 28);
    r.clr32(0x4c, 1 << 25 | 1 << 26);
    r.set32(0x40, 1 << 2);
    r.clr8(0x02, 0x03);
    r.clr8(0x1f, 0x07);
    r.clr32(0xec, 0x07 << 24);
}

/// The firmware's ready pattern in REG_MCUFW_CTRL.
pub const FW_READY: u16 = 0xc078;

/// rtw_mac_power_switch: Ok(false) when the MAC was already as asked.
fn switch<R: Regs>(r: &mut R, on: bool) -> Result<bool, Error> {
    let rpwm = r.r8(0x3d9);
    if r.r16(0x80) == FW_READY { r.w8(0x3d9, (rpwm ^ 0x80) & 0x80); }
    let is_on = r.r8(0x100) != 0xea;
    if is_on == on { return Ok(false); }
    run(r, if on { &ENABLE } else { &DISABLE })?;
    Ok(true)
}

/// rtw_mac_init_system_cfg for an 11ac CPU.
fn init_system<R: Regs>(r: &mut R) {
    r.set32(0x1080, 1 << 16 | 1 << 8);
    r.set8(0x03, 0xd8);
    let v = r.r8(0x1103);
    r.w8(0x1103, v & 0xf0 | 0x0c);
    if r.r32(0x80) & 1 << 20 != 0 {
        r.clr32(0x80, 1 << 20);
        r.clr32(0x40, 1 << 19);
    }
}

/// rtw_mac_power_on: a MAC found on (a firmware left running) is turned off and on again. Ok(true) when it was.
pub fn on<R: Regs>(r: &mut R) -> Result<bool, Error> {
    pre_system(r);
    let cycled = !switch(r, true)?;
    if cycled {
        switch(r, false)?;
        pre_system(r);
        switch(r, true)?;
    }
    init_system(r);
    Ok(cycled)
}

/// rtw_power_off's register part: the coexistence scoreboard told, the MAC off.
pub fn off<R: Regs>(r: &mut R) -> Result<(), Error> {
    r.w16(0xaa, 0x8000);
    switch(r, false).map(|_| ())
}
