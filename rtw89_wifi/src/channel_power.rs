// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022, 2024 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Pure initial 2 GHz/20 MHz power-register image; callers still need actual policy and firmware provenance.

use crate::channel_policy::{self as defaults, Error, Group};

const PHY: usize = 0x10000;
const WRITES: usize = 58;

/// Effective conducted limits already resolved from the real regulatory and platform inputs, in half-dBm.
/// No default or absent-policy interpretation is provided; per-path limits also control source reference differences.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Constraints { pub per_ntx: [i8; 2], pub per_path: [i8; 2] }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Write { pub at: usize, pub mask: u32, pub bits: u32 }
const EMPTY: Write = Write { at: 0, mask: 0, bits: 0 };

#[derive(Debug)]
pub(crate) struct Image { writes: [Write; WRITES], used: usize }
impl Image {
    pub(crate) fn writes(&self) -> &[Write] { &self.writes[..self.used] }
    fn push(&mut self, at: usize, mask: u32, bits: u32) {
        assert!(self.used < WRITES && mask != 0 && bits & !mask == 0);
        self.writes[self.used] = Write { at, mask, bits }; self.used += 1;
    }
    fn bytes(&mut self, at: usize, bytes: &[i8]) {
        for (i, value) in bytes.chunks_exact(4).enumerate() {
            self.push(at + 4 * i, u32::MAX, u32::from_le_bytes([value[0] as u8, value[1] as u8, value[2] as u8, value[3] as u8]));
        }
    }
}

// Pinned common-chip DFIR shapes; channel 14 is outside this initial OFDM/HT-capable scope.
const FLAT: [u32; 8] = [0x023d23ff,0x0029b354,0x000fc1c8,0x00fdb053,0x00f86f9a,0x06faef92,0x00fe5fcc,0x00ffdff5];
const SHARP: [u32; 8] = [0x023d83ff,0x002c636a,0x0013f204,0x00008090,0x00f87fb0,0x06f99f83,0x00fdbfba,0x00003ff5];

fn reference(decrease_eighth_dbm: u16) -> Result<u32, Error> {
    // Reject an unsupported difference before the source's unsigned nine-bit TSSI field could wrap.
    if decrease_eighth_dbm > 0xac { return Err(Error::PowerRange); }
    let raw = 0x138 - decrease_eighth_dbm;
    let bb = raw & 7;
    let rf = ((raw >> 3) & 63).clamp(15, 63);
    let cw = (rf << 3) | bb;
    Ok(((0xac - decrease_eighth_dbm) as u32) << 18 | (cw as u32) << 9)
}

/// This private builder performs no MMIO and returns no transmit authorization.
/// Dynamic antenna-gain tables must be absent under the actual verified default-table provenance.
pub(crate) fn build(group: Group, channel: u8, limits: Constraints) -> Result<Image, Error> {
    if !(1..=13).contains(&channel) { return Err(Error::Index); }
    if limits.per_ntx.iter().chain(limits.per_path.iter()).any(|v| !(-64..=63).contains(v)) { return Err(Error::PowerRange); }
    let difference = (limits.per_path[0] as i16 - limits.per_path[1] as i16) * 4;
    let ref_a = reference(if difference > 0 { 0 } else { -difference as u16 })?;
    let ref_b = reference(if difference > 0 { difference as u16 } else { 0 })?;
    let mut image = Image { writes: [EMPTY; WRITES], used: 0 };
    let mut rate_bytes = [0i8; 44]; let mut used = 0;
    for streams in 1..=2 {
        for (section, count) in [4, 8, 12, 4].into_iter().enumerate() {
            if streams == 2 && section < 2 { continue; }
            for index in 0..count {
                rate_bytes[used] = defaults::rf_to_mac(defaults::default_byrate_2g(streams, section as u8, index)?.value);
                used += 1;
            }
        }
    }
    image.bytes(0xd2c0, &rate_bytes);
    let mut offsets = 0;
    for index in 0..5 {
        let value = defaults::rf_to_mac(defaults::default_byrate_2g(1, 4, index)?.value);
        if !(-8..=7).contains(&value) { return Err(Error::PowerRange); }
        offsets |= ((value as u8 & 15) as u32) << (index * 4);
    }
    image.push(0xd204, 0x000fffff, offsets);
    let shape = if defaults::default_shape_2g(0, group)?.value == 0 { FLAT } else { SHARP };
    for (i, value) in shape.into_iter().enumerate() { image.push(PHY + 0x2300 + i * 4, u32::MAX, value); }
    image.push(PHY + 0x4494, 0x03000000, (defaults::default_shape_2g(1, group)?.value as u32) << 24);
    for streams in 1..=2 {
        let mut page = [0i8; 40];
        for (slot, bandwidth, section) in [(0,0,0), (2,1,0), (4,0,1), (6,0,2)] {
            for bf in 0..2 {
                let table = defaults::default_limit_2g(bandwidth, streams, section, bf != 0, group, channel)?;
                page[slot + bf] = defaults::rf_to_mac(table.quarter_dbm).min(limits.per_ntx[streams as usize - 1]);
            }
        }
        image.bytes(0xd2ec + 40 * (streams as usize - 1), &page);
    }
    for streams in 1..=2 {
        let mut page = [0i8; 24];
        for ru in 0..3 {
            let table = defaults::default_ru_2g(ru, streams, group, channel)?;
            page[ru as usize * 8] = defaults::rf_to_mac(table.quarter_dbm).min(limits.per_ntx[streams as usize - 1]);
        }
        image.bytes(0xd33c + 24 * (streams as usize - 1), &page);
    }
    image.push(0xd200, 0x0ffffc00, 0);
    for (at, value) in [(0x5804,ref_a),(0x7804,ref_b),(0x5808,ref_a),(0x7808,ref_b)] {
        image.push(PHY + at, 0x07ffffff, value);
    }
    Ok(image)
}
