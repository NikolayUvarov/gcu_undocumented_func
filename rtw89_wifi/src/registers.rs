// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2020 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
// Register definitions adapted from Linux v6.19 rtw89/reg.h and core.c.

pub const VENDOR_ID: u16 = 0x10ec;
pub const DEVICE_ID: u16 = 0xb852;
pub const MMIO_BAR: usize = 2;
pub const SYS_CFG1: usize = 0x00f0;
pub const CHIP_VERSION_MASK: u32 = 0x0000_f000;

/// Extract the digital cut without performing the indirect XTAL_SI transaction.
pub fn chip_cut(sys_cfg1: u32) -> Option<u8> {
    if sys_cfg1 == u32::MAX {
        None
    } else {
        Some(((sys_cfg1 & CHIP_VERSION_MASK) >> 12) as u8)
    }
}
