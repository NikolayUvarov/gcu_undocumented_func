// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2018-2019 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw88.txt.
// Adapted from Linux v6.12 rtw88 phy.c (rtw_phy_setup_phy_cond, check_positive, rtw_parse_tbl_phy_cond), main.h (struct rtw_phy_cond).
//! The condition words of rtw88's tables: an entry whose first word has bit 31 opens an if, elif, else or endif; one
//! with bit 30 closes the condition just read; the entries between apply when the chip matches it.

/// The interface field for PCIe.
pub const INTF_PCIE: u8 = 1;

/// What the chip and its board are, as the tables' conditions test them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chip { pub cut: u8, pub pkg: u8, pub intf: u8, pub rfe: u8 }

impl Chip {
    /// As rtw_phy_setup_phy_cond fills it: a cut or package of 0 is "any" (15), the interface PCIe.
    pub fn pcie(cut: u8, pkg: u8, rfe: u8) -> Self {
        Self { cut: if cut == 0 { 15 } else { cut & 0xf }, pkg: if pkg == 0 { 15 } else { pkg & 0xf }, intf: INTF_PCIE, rfe }
    }
}

const POS: u32 = 1 << 31;
const NEG: u32 = 1 << 30;
const BRANCH_ELSE: u32 = 2;
const BRANCH_ENDIF: u32 = 3;

fn branch(word: u32) -> u32 { word >> 28 & 3 }

/// Whether the condition in `word` holds for `chip` (check_positive): its cut, package and interface when it names them,
/// and its RFE type always.
pub fn holds(word: u32, chip: &Chip) -> bool {
    let (rfe, intf, pkg, cut) = (word as u8, (word >> 8 & 0xf) as u8, (word >> 12 & 0xf) as u8, (word >> 24 & 0xf) as u8);
    (cut == 0 || cut == chip.cut) && (pkg == 0 || pkg == chip.pkg) && (intf == 0 || intf == chip.intf) && rfe == chip.rfe
}

/// Calls `apply` with each (address, value) of `table` that applies to `chip`, in order (rtw_parse_tbl_phy_cond).
pub fn select<E>(table: &[(u32, u32)], chip: &Chip, mut apply: impl FnMut(u32, u32) -> Result<(), E>) -> Result<(), E> {
    let (mut condition, mut matched, mut skipped) = (0u32, true, false);
    for &(word, value) in table {
        if word & POS != 0 {
            match branch(word) {
                BRANCH_ENDIF => { matched = true; skipped = false; }
                BRANCH_ELSE => matched = !skipped,
                _ => condition = word,
            }
        } else if word & NEG != 0 {
            if skipped {
                matched = false;
            } else if holds(condition, chip) {
                (matched, skipped) = (true, true);
            } else {
                (matched, skipped) = (false, false);
            }
        } else if matched {
            apply(word, value)?;
        }
    }
    Ok(())
}
