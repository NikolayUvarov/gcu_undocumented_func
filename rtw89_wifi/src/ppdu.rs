// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! A received PPDU's signal from its status report (213-DRV-0087), as Linux's rtw89_core_rx_process_ppdu_sts, the 8852B's
//! convert_rpl_to_rssi and query_ppdu (pinned 05f7e89a). The device's bytes are untrusted: every length is checked.

/// The RX descriptor's packet type of a PPDU status report (RTW89_CORE_RX_TYPE_PPDU_STAT).
pub(crate) const TYPE: u8 = 1;

/// One report: the PPDU count and the data rate its frame carries in the descriptor, and the signal in dBm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Report { pub count: u8, pub rate: u16, pub signal_dbm: i8 }

const MAC_INFO: usize = 8;
const MAC_USER: usize = 4;
const RX_COUNTERS: usize = 96;
const MAX_USERS: u32 = 4;
const PHY_HEADER: usize = 8;
// RTW89_CCK_PKT: lower PPDU types are breaks and spoofs, whose IEs Linux does not parse.
const CCK_PKT: u32 = 11;
const OFDM6: u16 = 4;
// physts_ie_len_tabs[RTW89_CHIP_AX]; 0xff is VAR_LEN, its length in the IE header in 8-byte units.
const IE_LENGTHS: [u8; 32] = [16, 32, 24, 24, 8, 8, 8, 8, 0xff, 8, 0xff, 176, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 16, 24, 0xff,
    0xff, 0xff, 0, 24, 24, 24, 24, 32, 32, 32, 32];

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

/// RTW89_RSSI_RAW_TO_DBM: half-dB units above -110 dBm.
fn dbm(raw: u8) -> i8 { (raw >> 1) as i8 - 110 }

/// `descriptor`: the RX descriptor's words 0, 1 and 3; `payload`: the bytes after it. None for a report that is not one,
/// is truncated, or whose PHY status length disagrees with the packet (Linux drops those too).
pub(crate) fn parse(descriptor: [u32; 3], payload: &[u8]) -> Option<Report> {
    let [w0, w1, w3] = descriptor;
    if (w0 >> 24) & 15 != u32::from(TYPE) { return None; }
    let count = ((w1 >> 4) & 7) as u8;
    let rate = ((w1 >> 16) & 0x1ff) as u16;
    let to_self = w3 & 1 != 0;
    // rtw89_core_rx_process_mac_ppdu: the MAC info, its users (padded to eight bytes), RX counters and PLCP before the PHY's.
    let mut at = 0usize;
    if w0 & (1 << 23) != 0 {
        let (r0, r1) = (word(payload, 0)?, word(payload, 4)?);
        let users = r0 & 15;
        if users > MAX_USERS { return None; }
        let users = users as usize;
        at = MAC_INFO + users * MAC_USER + if users & 1 != 0 { MAC_USER } else { 0 };
        if r0 & (1 << 29) != 0 { at += RX_COUNTERS; }
        at += ((r1 >> 16) & 255) as usize * 8;
        if at > payload.len() { return None; }
    }
    // rtw89_core_rx_process_phy_ppdu: valid, and the header's length (8-byte units) is the rest of the packet.
    let (h0, h1) = (word(payload, at)?, word(payload, at + 4)?);
    if h0 & (1 << 7) == 0 || ((h0 >> 8) & 255) as usize * 8 != payload.len() - at { return None; }
    let average = (h0 >> 24) as u8;
    let mut rssi = [(h1 & 255) as u8, ((h1 >> 8) & 255) as u8];
    // rtw89_core_rx_parse_phy_sts: the IEs of a parsed PPDU type; RPL from IE 0 (CCK) or IE 1 (OFDM, to this radio).
    if h0 & 31 >= CCK_PKT {
        let end = payload.len();
        let mut pos = at + PHY_HEADER + if h0 & (1 << 5) != 0 { PHY_HEADER } else { 0 };
        let mut rpl = 0u8;
        let mut parsed = true;
        while pos < end {
            let Some(ie) = word(payload, pos) else { parsed = false; break };
            let kind = (ie & 31) as usize;
            let length = match IE_LENGTHS[kind] { 0xff => ((ie >> 5) & 127) as usize * 8, fixed => usize::from(fixed) };
            match kind {
                0 => rpl = ((((ie >> 7) & 511) >> 1) & 255) as u8,
                1 if rate >= OFDM6 && to_self => rpl = ((ie >> 8) & 255) as u8,
                _ => {}
            }
            pos += length;
            if pos > end || length == 0 { parsed = false; break; }
        }
        // __rtw8852bx_convert_rpl_to_rssi: each path's RSSI moved by the RPL's difference from the average, wrapping as u8.
        if parsed {
            let delta = rpl.wrapping_sub(average);
            for value in &mut rssi { *value = value.wrapping_add(delta); }
        }
    }
    // __rtw8852bx_query_ppdu: the stronger path.
    Some(Report { count, rate, signal_dbm: dbm(rssi[0].max(rssi[1])) })
}

/// wlanhw.wit's receive header byte 2: the signal as minus dBm, 0 when not measured.
pub(crate) fn weakness(signal_dbm: i8) -> u8 { (-i16::from(signal_dbm)).clamp(1, 255) as u8 }
