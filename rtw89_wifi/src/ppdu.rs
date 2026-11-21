// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! A received PPDU's signal from its status report (213-DRV-0087), as Linux's rtw89_core_rx_process_ppdu_sts, the 8852B's
//! convert_rpl_to_rssi and query_ppdu (pinned 05f7e89a). The device's bytes are untrusted: every length is checked.

use crate::rxdesc;

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

// The descriptor's word 1: the PPDU count (bits 4-6) and the data rate (bits 16-24), the same in a frame and its report.
fn count_rate(w1: u32) -> (u8, u16) { (((w1 >> 4) & 7) as u8, ((w1 >> 16) & 0x1ff) as u16) }

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
    let (count, rate) = count_rate(w1);
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

/// A report as the receive ring gave it: the descriptor's words after the buffer's PCIe word, its bytes after them.
pub(crate) fn from_packet(packet: rxdesc::Packet, buffer: &[u8]) -> Option<Report> {
    parse([word(buffer, 4)?, word(buffer, 8)?, word(buffer, 16)?], packet.bytes(buffer)?)
}

/// What became of the reports: `measured` gave the frame kept just before its signal; `late` came after that frame
/// was handed on, which keeps its 0; `unmatched` followed no kept frame of their count and rate, or were not reports.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Signals { pub measured: u32, pub late: u32, pub unmatched: u32 }

// The newest frame kept: its number among the receive queue's Wi-Fi frames, its slot, count and rate, and if unread.
#[derive(Clone, Copy, Debug)]
struct Kept { number: u64, at: usize, key: (u8, u16), unread: bool }

/// Puts a report's signal on the one frame its owner kept last, when that frame is the receive queue's Wi-Fi frame
/// just before the report and has its count and rate (Linux's rtw89_core_rx_ppdu_match); a report is never guessed
/// onto another frame. The owner says what it keeps and hands on; the session numbers the frames.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Matcher { kept: Option<Kept>, pub(crate) signals: Signals }
impl Matcher {
    /// The `number`-th Wi-Fi frame kept in slot `at`; `buffer` holds its descriptor.
    pub(crate) fn kept(&mut self, number: u64, at: usize, buffer: &[u8]) {
        self.kept = word(buffer, 8).map(|w1| Kept { number, at, key: count_rate(w1), unread: true });
    }
    /// The frame in slot `at` handed on.
    pub(crate) fn handed(&mut self, at: usize) { if let Some(kept) = self.kept.as_mut().filter(|k| k.at == at) { kept.unread = false; } }
    /// The kept frames dropped: a tune, a new filter, the hand-over to the data mode.
    pub(crate) fn forget(&mut self) { self.kept = None; }
    /// Whether the frame in slot `at` is kept and still waits for its report.
    pub(crate) fn waits(&self, at: usize) -> bool { self.kept.is_some_and(|k| k.unread && k.at == at) }
    /// A report after the `frame`-th Wi-Fi frame (None: after another report): the slot and the header byte to write.
    /// Only the report of the kept frame is parsed; the others, most of them on a busy channel, cost a comparison.
    pub(crate) fn report(&mut self, frame: Option<u64>, packet: rxdesc::Packet, buffer: &[u8]) -> Option<(usize, u8)> {
        let signals = &mut self.signals;
        let Some(kept) = self.kept.take().filter(|k| frame == Some(k.number) && word(buffer, 8).map(count_rate) == Some(k.key))
            else { signals.unmatched = signals.unmatched.saturating_add(1); return None };
        if !kept.unread { signals.late = signals.late.saturating_add(1); return None; }
        let Some(report) = from_packet(packet, buffer) else { signals.unmatched = signals.unmatched.saturating_add(1); return None };
        signals.measured = signals.measured.saturating_add(1);
        Some((kept.at, weakness(report.signal_dbm)))
    }
}
