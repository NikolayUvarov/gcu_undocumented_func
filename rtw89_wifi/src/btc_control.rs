// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Bounded no-link coexistence register transitions; none of these operations alone grants RFK.

use crate::io::{self, Fault, FaultKind, RegisterIo, TimedIo};

const SCOREBOARD: usize = 0xac;
const LTE_CTRL: usize = 0xdaf0;
const LTE_WDATA: usize = 0xdaf4;
const LTE_RDATA: usize = 0xdaf8;
const WL_ACTIVE: u32 = 1;
const WL_ON: u32 = 1 << 1;
const WL_RFK: u32 = 1 << 11;
const WL_BTLOG: u32 = 1 << 14;
const BASEBAND: usize = 0x10000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BtState { pub enabled: bool, pub patched: bool, pub running_rfk: bool, pub requests_rfk: bool }
impl BtState {
    pub fn read(bus: &mut impl RegisterIo) -> io::Result<Self> {
        let raw = io::read32(bus, SCOREBOARD)?;
        Ok(Self { enabled: raw & 2 != 0, patched: raw & 1 << 30 != 0,
            running_rfk: raw & 1 << 5 != 0, requests_rfk: raw & 1 << 6 != 0 })
    }
}

/// Lower bits are a WLAN-owned write shadow; the same address reads Bluetooth's independent state.
pub struct WlanShadow { value: u32, failed: bool }
impl WlanShadow {
    /// Only for a fresh verified firmware session with exclusive scoreboard ownership; never reconstruct on live firmware.
    pub fn new() -> Self { Self { value: 0, failed: false } }
    pub fn value(&self) -> u32 { self.value }
    pub(crate) fn clear(&mut self,mask:u32){self.value&=!mask;}
    pub fn initialize(&mut self) { self.value = WL_ACTIVE | WL_ON | WL_BTLOG; }
    pub fn set_rfk(&mut self, active: bool) { if active { self.value |= WL_RFK; } else { self.value &= !WL_RFK; } }
    /// An ambiguous write poisons this shadow; it must not be reused as a successfully applied state.
    pub fn publish(&mut self, bus: &mut impl RegisterIo, timeout_us: u32) -> io::Result<()> {
        if self.failed || timeout_us <= 1_000 || timeout_us > 100_000 { return Err(Fault::new(FaultKind::Access, SCOREBOARD, timeout_us)); }
        self.failed = true;
        let mut timed = TimedIo::new(bus, timeout_us as u64);
        let bt = io::read32(&mut timed, SCOREBOARD)?;
        // Preserve only firmware-owned upper bits and select the powered-on TP notification, with the source toggle.
        let firmware = ((bt >> 24) & 0x7f & !1) | 1;
        timed.write32(SCOREBOARD, 1 << 31 | firmware << 24 | self.value)?;
        timed.delay_us(1_000)?;
        timed.delay_us(0)?;
        self.failed = false;
        Ok(())
    }
}
impl Default for WlanShadow { fn default() -> Self { Self::new() } }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Antenna { Initial, NoLink, Rfk, WlanOnly }

/// Actual pre-transition registers, retained for a failed transition's independent restoration attempt.
#[derive(Debug)]
pub struct SavedAntenna { grants: u32, control: u8, pollution: u16 }

fn ready(bus: &mut impl RegisterIo) -> io::Result<()> {
    let mut timed = TimedIo::new(bus, 50_000);
    for _ in 0..=1_000 {
        let raw = timed.read8(LTE_CTRL + 3)?;
        if raw == u8::MAX { return Err(Fault::new(FaultKind::Unavailable, LTE_CTRL + 3, raw as u32)); }
        timed.delay_us(0)?;
        if raw & 0x20 != 0 { return Ok(()); }
        timed.delay_us(50)?;
    }
    Err(Fault::new(FaultKind::Timeout, LTE_CTRL, 0))
}
fn lte_read(bus: &mut impl RegisterIo) -> io::Result<u32> {
    ready(bus)?; bus.write32(LTE_CTRL, 0x800f0038)?;
    let value = io::read32(bus, LTE_RDATA)?;
    ready(bus)?; Ok(value)
}
fn lte_write(bus: &mut impl RegisterIo, value: u32) -> io::Result<()> {
    ready(bus)?; bus.write32(LTE_WDATA, value)?; bus.write32(LTE_CTRL, 0xc00f0038)?; ready(bus)
}
fn require(bus: &mut impl RegisterIo, at: usize, mask: u32, expected: u32) -> io::Result<()> {
    let raw = io::read32(bus, at)?;
    if raw & mask != expected { return Err(Fault::new(FaultKind::PowerState, at, raw)); } Ok(())
}
fn control(bus: &mut impl RegisterIo, wl: bool) -> io::Result<()> {
    let value = bus.read8(0x73)?;
    if value == u8::MAX { return Err(Fault::new(FaultKind::Unavailable, 0x73, value as u32)); }
    bus.write8(0x73, (value & !4) | if wl { 4 } else { 0 })?;
    let actual = bus.read8(0x73)?;
    if actual == u8::MAX { return Err(Fault::new(FaultKind::Unavailable, 0x73, actual as u32)); }
    if actual & 4 != if wl { 4 } else { 0 } { return Err(Fault::new(FaultKind::PowerState, 0x73, actual as u32)); } Ok(())
}

impl SavedAntenna {
    pub fn read(bus: &mut impl RegisterIo, timeout_us: u32) -> io::Result<Self> {
        if timeout_us == 0 || timeout_us > 200_000 { return Err(Fault::new(FaultKind::Access, LTE_CTRL, timeout_us)); }
        let mut timed = TimedIo::new(bus, timeout_us as u64);
        let control = timed.read8(0x73)?; let pollution = timed.read16(0xc67c)?;
        if control == u8::MAX || pollution == u16::MAX { return Err(Fault::new(FaultKind::Unavailable, 0x73, control as u32)); }
        let grants = lte_read(&mut timed)?; timed.delay_us(0)?;
        Ok(Self { grants, control, pollution })
    }
    /// Restore known prior grant/control state; never retry a still-pending indirect command.
    pub fn restore(&self, bus: &mut impl RegisterIo, timeout_us: u32) -> io::Result<()> {
        if timeout_us == 0 || timeout_us > 200_000 { return Err(Fault::new(FaultKind::Access, LTE_CTRL, timeout_us)); }
        let mut timed = TimedIo::new(bus, timeout_us as u64);
        lte_write(&mut timed, self.grants)?;
        control(&mut timed, self.control & 4 != 0)?;
        timed.write16(0xc67c, self.pollution)?;
        let grants = lte_read(&mut timed)?;
        if grants != self.grants { return Err(Fault::new(FaultKind::PowerState, 0x38, grants)); }
        let pollution = timed.read16(0xc67c)?;
        if pollution != self.pollution { return Err(Fault::new(FaultKind::PowerState, 0xc67c, pollution as u32)); }
        timed.delay_us(0)
    }
}

/// The caller owns a freshly validated MAC/PHY/RX session; ordinary TX stays stopped throughout this transition.
/// A prior snapshot and independent radio-disable path are mandatory if any applied write fails.
pub fn antenna(bus: &mut impl RegisterIo, desired: Antenna, timeout_us: u32) -> io::Result<BtState> {
    if timeout_us == 0 || timeout_us > 100_000 { return Err(Fault::new(FaultKind::Access, LTE_CTRL, timeout_us)); }
    let mut timed = TimedIo::new(bus, timeout_us as u64);
    let bt = BtState::read(&mut timed)?;
    if bt.running_rfk || bt.requests_rfk { return Err(Fault::new(FaultKind::PowerState, SCOREBOARD, 0x60)); }
    let grants = match desired { Antenna::NoLink => 0,
        Antenna::Initial if bt.enabled => 0xdd00dd00, _ => 0x77007700 };
    let pollution = if matches!(desired,Antenna::Rfk|Antenna::WlanOnly) { 0x100 } else { 0x166 };
    // The initial source path writes grants first; no-link and RFK select WLAN control first.
    if matches!(desired,Antenna::Initial|Antenna::WlanOnly) { lte_write(&mut timed, grants)?; control(&mut timed, true)?; }
    else { control(&mut timed, true)?; lte_write(&mut timed, grants)?; }
    timed.write16(0xc67c, pollution)?;
    let actual = lte_read(&mut timed)?;
    if actual != grants { return Err(Fault::new(FaultKind::PowerState, 0x38, actual)); }
    let actual = timed.read16(0xc67c)?;
    if actual != pollution { return Err(Fault::new(FaultKind::PowerState, 0xc67c, actual as u32)); }
    timed.delay_us(0)?; Ok(bt)
}

const PRE_AGC_DISABLED: &[(usize,u32,u32)] = &[(0x46d0,3,0),(0x4790,3,0),(0x4ad4,u32::MAX,0x60),(0x4ae0,u32::MAX,0x60),
    (0x4688,0xff000000,0x1a),(0x476c,0xff000000,0x1a),(0x4694,0xff,0x2a),(0x4694,0xff00,0x2a),
    (0x4778,0xff,0x2a),(0x4778,0xff00,0x2a),(0x4ae4,0xffffff,0x79e99e),(0x4aec,0xffffff,0x79e99e),
    (0x469c,0xfc000000,0x26),(0x49f0,0xfc000000,0x26)];
const BTG_ENABLED: &[(usize,u32,u32)] = &[(0x4738,1<<19,1),(0x4738,1<<22,0),(0x476c,0xff000000,0x20),
    (0x4778,0xff,0x30),(0x4aa4,1<<19,1),(0x4aa4,1<<22,1),(0x980,0x001e0000,0),(0x49c4,1<<14,1),
    (0x49c0,0x03c00000,2),(0x4420,1<<31,1),(0xc6c,1<<21,1)];

fn table(bus: &mut impl RegisterIo, rows: &[(usize,u32,u32)]) -> io::Result<()> {
    for &(offset, mask, value) in rows {
        let bits = value << mask.trailing_zeros();
        io::modify32(bus, BASEBAND + offset, mask, bits)?;
        require(bus, BASEBAND + offset, mask, bits)?;
    }
    Ok(())
}

/// Source default WLAN power is set before monitor/slot/init messages, separately from the later common action.
pub fn default_power(bus: &mut impl RegisterIo, timeout_us: u32) -> io::Result<()> {
    if timeout_us == 0 || timeout_us > 100_000 { return Err(Fault::new(FaultKind::Access, 0, timeout_us)); }
    let mut timed = TimedIo::new(bus, timeout_us as u64);
    io::modify32(&mut timed, 0xd200, 0x3ff, 0)?;
    require(&mut timed, 0xd200, 0x3ff, 0)?;
    io::modify32(&mut timed, 0xd220, 0xff8 | 2, 0)?;
    require(&mut timed, 0xd220, 0xff8 | 2, 0)?;
    timed.delay_us(0)
}

/// The no-link initial _action_common BB operations; TX-limit iteration has no station to visit.
/// BT RF-parameter commands require BT-info updates absent at cold init, and WLAN RX gain starts at level zero.
/// AFH, ROLE, reports and the final scoreboard flush remain separate mandatory firmware/control steps.
pub fn common_initial(bus: &mut impl RegisterIo, bt: BtState, timeout_us: u32) -> io::Result<()> {
    if timeout_us == 0 || timeout_us > 100_000 { return Err(Fault::new(FaultKind::Access, 0, timeout_us)); }
    let mut timed = TimedIo::new(bus, timeout_us as u64);
    if bt.enabled && bt.patched { table(&mut timed, BTG_ENABLED)?; }
    table(&mut timed, PRE_AGC_DISABLED)?;
    timed.delay_us(0)
}

#[cfg(test)]
#[path = "../../tests/rtw89_btc_control_cases.rs"]
mod tests;


pub(crate) fn btg(bus:&mut impl RegisterIo,enabled:bool)->io::Result<()> {
    if enabled{return table(bus,BTG_ENABLED)}
    table(bus,&[(0x4738,1<<19,0),(0x4738,1<<22,0),(0x476c,0xff000000,0x1a),
        (0x4778,0xff,0x2a),(0x4aa4,1<<19,0),(0x4aa4,1<<22,0),(0x980,0x001e0000,0xc),
        (0x49c4,1<<14,0),(0x49c0,0x03c00000,0),(0x4420,1<<31,1),(0xc6c,1<<21,0)])
}
pub(crate) fn preagc_off(bus:&mut impl RegisterIo)->io::Result<()>{table(bus,PRE_AGC_DISABLED)}
