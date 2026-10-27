// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2018-2019 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw88.txt.
// Adapted from Linux v6.12 rtw88 tx.h/tx.c (struct rtw_tx_desc, rtw_tx_fill_tx_desc, the report cookie),
// rx.h/rx.c (struct rtw_rx_desc), rtw8821c.c (query_phy_status, query_rx_desc), fw.c/fw.h (C2H, the CCX report).
// The per-frame retry limit (TX descriptor word 4, bits 17-23) and the report's retry count come from Realtek's halmac
// headers (halmac_tx_desc_nic.h, halmac_fw_offload_c2h_nic.h), not rtw88: to be confirmed on the chip.
//! The TX descriptor of a frame sent at a fixed rate, the RX descriptor and PHY status of a frame received, and the
//! firmware's report of a frame sent.

/// The rates the station asks for (500 kb/s units), as the chip's descriptor rates (DESC_RATE1M to DESC_RATE54M).
pub fn desc_rate(rate: u8) -> Option<u8> {
    Some(match rate { 2 => 0, 4 => 1, 11 => 2, 22 => 3, 12 => 4, 18 => 5, 24 => 6, 36 => 7, 48 => 8, 72 => 9, 96 => 10, 108 => 11, _ => return None })
}

/// A descriptor rate as 500 kb/s units; 0 for an HT or VHT rate.
pub fn rate_500k(desc: u8) -> u8 { [2, 4, 11, 22, 12, 18, 24, 36, 48, 72, 96, 108].get(desc as usize).copied().unwrap_or(0) }

pub const QSEL_MGMT: u8 = 0x12;
pub const QSEL_BE: u8 = 0;
pub const QSEL_BEACON: u8 = 0x10;
pub const QSEL_H2C: u8 = 0x13;

fn put(d: &mut [u8], word: usize, value: u32) {
    let v = u32::from_le_bytes(d[word * 4..word * 4 + 4].try_into().unwrap()) | value;
    d[word * 4..word * 4 + 4].copy_from_slice(&v.to_le_bytes());
}

/// What a frame needs of its descriptor.
pub struct Frame { pub len: usize, pub qsel: u8, pub rate: u8, pub tries: u8, pub group: bool, pub seq: u16, pub cookie: u8 }

/// The 48-byte TX descriptor of a frame sent at one fixed rate, no fallback, at most `tries` times, its sequence
/// number as the station set it, and a report asked for under `cookie` (6 bits).
pub fn tx_desc(d: &mut [u8], f: &Frame) {
    d[..48].fill(0);
    // RATEID_B_20M (8) for CCK, RATEID_BG (6) for OFDM on 2.4 GHz.
    let rate_id: u32 = if f.rate < 4 { 8 } else { 6 };
    put(d, 0, f.len as u32 & 0xffff | 48 << 16 | (f.group as u32) << 24 | 1 << 26);
    put(d, 1, (f.qsel as u32 & 0x1f) << 8 | rate_id << 16);
    put(d, 2, 1 << 19);
    put(d, 3, 1 << 8 | 1 << 10);
    let tries = f.tries.clamp(1, 63) as u32;
    put(d, 4, f.rate as u32 & 0x7f | 1 << 17 | tries << 18);
    put(d, 6, (f.cookie as u32 & 0x3f) << 2);
    put(d, 9, (f.seq as u32 & 0xfff) << 12);
}

/// The beacon ring's descriptor of a reserved-page chunk (a firmware chunk): 6 Mb/s, the hardware's sequence.
pub fn page_desc(d: &mut [u8], len: usize) {
    d[..48].fill(0);
    put(d, 0, len as u32 & 0xffff | 48 << 16 | 1 << 26 | 1 << 31);
    put(d, 1, (QSEL_BEACON as u32) << 8 | 7 << 16);
    put(d, 3, 1 << 8 | 1 << 10);
    put(d, 4, 4);
    put(d, 8, 1 << 15);
}

/// The H2C ring's descriptor of a 32-byte command packet.
pub fn h2c_desc(d: &mut [u8]) {
    d[..48].fill(0);
    put(d, 0, 32);
    put(d, 1, (QSEL_H2C as u32) << 8);
}

fn word(d: &[u8], i: usize) -> u32 { u32::from_le_bytes(d[i * 4..i * 4 + 4].try_into().unwrap()) }

/// A received buffer's 24-byte descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rx { pub len: usize, pub crc: bool, pub drv_info: usize, pub shift: usize, pub physt: bool, pub c2h: bool, pub rate: u8 }

impl Rx {
    pub fn parse(b: &[u8]) -> Option<Rx> {
        if b.len() < 24 { return None; }
        let (w0, w2, w3) = (word(b, 0), word(b, 2), word(b, 3));
        Some(Rx { len: (w0 & 0x3fff) as usize, crc: w0 & 1 << 14 != 0, drv_info: (w0 >> 16 & 0xf) as usize * 8,
                  shift: (w0 >> 24 & 3) as usize, physt: w0 & 1 << 26 != 0, c2h: w2 & 1 << 28 != 0, rate: (w3 & 0x7f) as u8 })
    }
    /// Where the frame (or the firmware message) starts in the buffer.
    pub fn start(&self) -> usize { 24 + self.drv_info + self.shift }
}

/// The signal of a frame from its PHY status (32 bytes after the RX descriptor and shift), in dBm; None when the page
/// is neither CCK's (0) nor OFDM's (1). `rfe0`: the board's RFE type is 0, whose CCK LNA gains differ.
pub fn signal(status: &[u8], rfe0: bool) -> Option<i16> {
    if status.len() < 16 { return None; }
    match status[0] & 0xf {
        0 => {
            let w3 = word(status, 3);
            let vga = (w3 >> 8 & 0x1f) as i16;
            let lna = ((w3 >> 23 & 1) << 3 | (w3 >> 13 & 7)) as usize;
            const RFE0: [i16; 8] = [22, 8, -6, -22, -31, -40, -46, -52];
            const OTHER: [i16; 16] = [10, 6, 2, -2, -6, -10, -14, -17, -20, -24, -28, -31, -34, -37, -40, -44];
            let gain = if rfe0 { RFE0.get(lna) } else { OTHER.get(lna) };
            Some(gain.map_or(-120, |g| g - 2 * vga))
        }
        1 => Some((status[1] as i16 - 110).max(-120)),
        _ => None,
    }
}

/// The firmware's report of a frame sent: its cookie, whether it was acknowledged, and its tries when the report says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report { pub cookie: u8, pub acked: bool, pub tries: Option<u8> }

/// A firmware message (`m`: id, sequence, payload) as a TX report: the 11ac form (id 0xff, sub-id 0x0f) or the older
/// one (id 0x03).
pub fn report(m: &[u8]) -> Option<Report> {
    let (&id, payload) = (m.first()?, m.get(2..)?);
    match id {
        0xff if payload.first() == Some(&0x0f) && payload.len() > 10 =>
            Some(Report { cookie: payload[8] >> 2 & 0x3f, acked: payload[9] & 0xc0 == 0, tries: Some((payload[10] & 0x3f) + 1) }),
        0x03 if payload.len() > 6 =>
            Some(Report { cookie: payload[6] >> 2 & 0x3f, acked: payload[0] & 0xc0 == 0, tries: Some((payload[2] & 0x3f) + 1) }),
        _ => None,
    }
}

/// Whether a received 802.11 frame passes the filter: beacons and probe responses while scanning, frames to `own`,
/// and group frames of the network `bssid`. No control or extension frames.
pub fn wanted(f: &[u8], all_beacons: bool, own: &[u8; 6], bssid: &[u8; 6]) -> bool {
    if f.len() < 10 { return false; }
    let (kind, subtype, ds) = (f[0] >> 2 & 3, f[0] >> 4, f[1] & 3);
    if kind == 1 || kind == 3 { return false; }
    if kind == 0 && (subtype == 8 || subtype == 5) && all_beacons { return true; }
    if f[4..10] == *own { return true; }
    if f.len() < 24 || *bssid == [0; 6] || f[4] & 1 == 0 { return false; }
    let bss = match (kind, ds) { (0, _) | (2, 0) => &f[16..22], (2, 2) => &f[10..16], _ => return false };
    bss == bssid
}
