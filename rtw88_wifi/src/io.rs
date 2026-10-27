// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2018-2019 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw88.txt.
// Adapted from Linux v6.12 rtw88 hci.h (rtw_write32_mask, _set, _clr), util.c (check_hw_ready).
//! Register access to BAR2 and the waits the chip's sequences need.

/// Why a step failed: which wait ran out and the register's last value, or what was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Timeout { what: &'static str, at: u32, value: u32 },
    Firmware(&'static str),
    Efuse(&'static str),
    Chip(&'static str),
    Dma(&'static str),
}

/// The chip's registers. The provided methods are rtw88's helpers.
pub trait Regs {
    fn r8(&mut self, at: u32) -> u8;
    fn r16(&mut self, at: u32) -> u16;
    fn r32(&mut self, at: u32) -> u32;
    fn w8(&mut self, at: u32, value: u8);
    fn w16(&mut self, at: u32, value: u16);
    fn w32(&mut self, at: u32, value: u32);
    /// A short wait, spun.
    fn udelay(&mut self, us: u32);
    /// A long wait, slept.
    fn msleep(&mut self, ms: u32);

    fn set8(&mut self, at: u32, bits: u8) { let v = self.r8(at); self.w8(at, v | bits); }
    fn clr8(&mut self, at: u32, bits: u8) { let v = self.r8(at); self.w8(at, v & !bits); }
    fn set16(&mut self, at: u32, bits: u16) { let v = self.r16(at); self.w16(at, v | bits); }
    fn set32(&mut self, at: u32, bits: u32) { let v = self.r32(at); self.w32(at, v | bits); }
    fn clr32(&mut self, at: u32, bits: u32) { let v = self.r32(at); self.w32(at, v & !bits); }
    /// The field `mask` of a word set to `value`, shifted to the mask's lowest bit (rtw_write32_mask).
    fn mask32(&mut self, at: u32, mask: u32, value: u32) {
        let v = self.r32(at);
        self.w32(at, v & !mask | (value << mask.trailing_zeros()) & mask);
    }
    fn mask8(&mut self, at: u32, mask: u8, value: u8) {
        let v = self.r8(at);
        self.w8(at, v & !mask | (value << mask.trailing_zeros()) & mask);
    }
    /// Waits until `(r32(at) & mask) == want`, reading every `us` µs at most `tries` times.
    fn poll32(&mut self, at: u32, mask: u32, want: u32, tries: u32, us: u32, what: &'static str) -> Result<(), Error> {
        let mut value = 0;
        for _ in 0..tries {
            value = self.r32(at);
            if value & mask == want { return Ok(()); }
            self.udelay(us);
        }
        Err(Error::Timeout { what, at, value })
    }
    fn poll8(&mut self, at: u32, mask: u8, want: u8, tries: u32, us: u32, what: &'static str) -> Result<(), Error> {
        let mut value = 0;
        for _ in 0..tries {
            value = self.r8(at);
            if value & mask == want { return Ok(()); }
            self.udelay(us);
        }
        Err(Error::Timeout { what, at, value: value as u32 })
    }
}
