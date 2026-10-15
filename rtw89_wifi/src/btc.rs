// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2020 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Read-only BT RFK arbitration prerequisite; this does not grant WLAN calibration or implement coexistence.

use crate::io::{self, Fault, FaultKind, RegisterIo};
pub const SCOREBOARD: usize = 0xac;
pub const BT_RFK_RUN: u32 = 1 << 5;
pub const BT_RFK_REQUEST: u32 = 1 << 6;
pub const MAX_WAIT_US: u32 = 100_000;

/// A transient BT-side observation, not an RFK START token; Bluetooth can change it after this read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClearSnapshot { raw: u32 }
impl ClearSnapshot { pub fn raw(self) -> u32 { self.raw } }

/// Require actual clear RUN/REQ bits; absent BT timeout-report state never authorizes overriding a busy BT.
/// Native callers still need initialized BTC state, owned WL scoreboard and applied antenna/grant/TDMA policy.
pub fn wait_bt_clear(bus: &mut impl RegisterIo, timeout_us: u32) -> io::Result<ClearSnapshot> {
    if timeout_us == 0 || timeout_us > MAX_WAIT_US { return Err(Fault::new(FaultKind::Access, SCOREBOARD, timeout_us)); }
    let raw = io::poll32(bus, SCOREBOARD, BT_RFK_RUN | BT_RFK_REQUEST, 0, 40, timeout_us)?;
    Ok(ClearSnapshot { raw })
}
