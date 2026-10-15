// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Exact RTL8852B 0.29.29.15 startup messages; encoding does not establish policy application or RFK permission.

use crate::{board::Board, download, h2c};

pub const SLOT_COUNT: usize = 18;
pub const MAX_BODY: usize = 256;
const MONITOR: [(u16, u32); 16] = [(0,0xda24),(0,0xda28),(0,0xda2c),(0,0xda30),
    (0,0xda4c),(0,0xda10),(0,0xda20),(0,0xda34),(0,0xcef4),(0,0x8424),(0,0xd200),(0,0xd220),
    (1,0x980),(1,0x4738),(1,0x4688),(1,0x4694)];
const SLOTS: [(u16, u32, u16); SLOT_COUNT] = [(100,0x55555555,0),(5,0xea5a5a5a,1),
    (70,0xea5a5a5a,1),(15,0xea5a5a5a,1),(15,0xea5a5a5a,1),(250,0xe5555555,0),
    (7,0xea5a5a5a,0),(5,0xe5555555,0),(50,0xe5555555,0),(20,0xea5a5a5a,1),
    (500,0x55555555,0),(5,0xea5a5a5a,0),(5,0xffffffff,1),(5,0xe5555555,0),
    (5,0x55555555,0),(250,0xea5a5a5a,0),(50,0xffffffff,1),(50,0xffffdfff,1)];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error { Profile, Reports }

/// The version selector is not a firmware-session token; callers still validate Ready against the live command queue.
pub struct Profile { _private: () }
impl Profile {
    pub fn select(board: &Board, firmware: &download::Ready) -> Result<Self, Error> {
        Self::select_version(board, firmware.version())
    }
    fn select_version(board: &Board, version: [u8; 4]) -> Result<Self, Error> {
        if version != [0,29,29,15] || board.identity.cut != 1 || board.identity.rfe != 1
            || board.maps.phycap[0x6c..0x6e] != [0xff,0xff] { return Err(Error::Profile); }
        Ok(Self { _private: () })
    }

    pub fn monitor_table(&self) -> Encoded {
        let mut out = Encoded::set(2); out.put(&[2, MONITOR.len() as u8]);
        for (kind, offset) in MONITOR { out.put(&kind.to_le_bytes()); out.put(&4u16.to_le_bytes()); out.put(&offset.to_le_bytes()); }
        out
    }
    pub fn slot_defaults(&self) -> Encoded {
        let mut out = Encoded::set(1); out.put(&[1,SLOT_COUNT as u8]);
        for slot in SLOTS { out.slot(slot); } out
    }
    /// The caller owns the report-map shadow; the 105 profile uses wire fver=5 and mapping version 3.
    pub fn reports(&self, resulting_map: u32, enabled: bool) -> Result<Encoded, Error> {
        if resulting_map & !0x1ff != 0 { return Err(Error::Reports); }
        let mut out = Encoded::set(0); out.put(&[5]); out.put(&resulting_map.to_le_bytes()); out.put(&(enabled as u32).to_le_bytes()); Ok(out)
    }
    pub fn driver_init(&self) -> Encoded {
        let mut out = Encoded::driver();
        // The pinned source resets module fields before selecting RFE-1; zero adie/BTG/stream fields are source state.
        out.put(&[0,12,0,2,10,0,1,1,2,0,6,2,0,0]); out
    }
    pub fn driver_control(&self, ignore_bt: bool) -> Encoded {
        let mut out = Encoded::driver(); out.put(&[6,4,(ignore_bt as u8) << 1,0,0,0]); out
    }
    /// The startup state has no roles, no DBCC, no scan and no connection; every reserved byte is initialized.
    pub fn driver_no_link(&self) -> Encoded {
        let mut out = Encoded::driver(); out.put(&[1,64]); out.put(&[0;64]); out
    }
    /// Source no-link width starts at enum 20MHz=0 and still encodes the six-channel guard on both sides.
    pub fn afh_no_link(&self) -> Encoded { let mut out = Encoded::set(10); out.put(&[0,0,32]); out }

    /// Match source delta TLVs; RFK START forces all slots and TDMA's instant-execute bit.
    pub fn policy(&self, previous: Policy, next: Policy) -> Option<Encoded> {
        if previous == next && next != Policy::Rfk { return None; }
        let mut out = Encoded::set(3);
        let force = next == Policy::Rfk;
        if force || previous.tdma() != next.tdma() {
            out.put(&[0,12,3,0,0,0]); out.put(&next.tdma());
        }
        for index in 0..SLOT_COUNT {
            let slot = next.slot(index);
            if force || previous.slot(index) != slot {
                out.put(&[1,10,1,index as u8]); out.slot(slot);
            }
        }
        if out.len == 0 { None } else { Some(out) }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Policy { ResetDefaults, NoLink, Rfk }
impl Policy {
    fn tdma(self) -> [u8; 8] { [0,0,0,0,0,0,0,if self == Self::Rfk { 1 } else { 0 }] }
    fn slot(self, index: usize) -> (u16,u32,u16) {
        if index != 0 { return SLOTS[index]; }
        match self { Self::ResetDefaults => SLOTS[0], Self::NoLink => (100,0xe5555555,0), Self::Rfk => (100,0xaaaaaaaa,1) }
    }
}

/// Typed command plus initialized bounded payload; source CXDRVINFO and generic BTC commands request different ACKs.
pub struct Encoded { command: h2c::Command, acks: h2c::Acks, body: [u8; MAX_BODY], len: usize }
impl Encoded {
    fn set(function: u8) -> Self { Self { command: h2c::Command { category:2,class:16,function },
        acks:h2c::Acks { received:false,done:true }, body:[0;MAX_BODY],len:0 } }
    fn driver() -> Self { let mut out = Self::set(5); out.acks.done = false; out }
    fn put(&mut self, bytes: &[u8]) { self.body[self.len..self.len+bytes.len()].copy_from_slice(bytes); self.len += bytes.len(); }
    fn slot(&mut self, (duration,table,kind): (u16,u32,u16)) { self.put(&duration.to_le_bytes()); self.put(&table.to_le_bytes()); self.put(&kind.to_le_bytes()); }
    pub fn command(&self) -> h2c::Command { self.command }
    pub fn acks(&self) -> h2c::Acks { self.acks }
    pub fn bytes(&self) -> &[u8] { &self.body[..self.len] }
    pub fn submit(&self, engine: &mut h2c::Engine, transport: &mut impl h2c::Transport, timeout_us: u32) -> Result<h2c::Ticket, h2c::Error> {
        engine.submit(transport, self.command, self.bytes(), self.acks, timeout_us)
    }
}

#[cfg(test)]
#[path = "../../tests/rtw89_btc_codec_cases.rs"]
mod tests;
