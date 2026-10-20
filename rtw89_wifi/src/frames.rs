// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Initial legacy-rate raw receive frames for wlanhw 1.1; association and filtering remain separate.

use crate::rxdesc;

pub(crate) const FRAME_MAX: usize = 2346;
pub(crate) const HEADER_BYTES: usize = 8;
const RATES: [u8; 12] = [2, 4, 11, 22, 12, 18, 24, 36, 48, 72, 96, 108];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error { Descriptor(rxdesc::Error), Kind, Integrity, Decrypted, Aggregate, Rate, Frame, Channel, Output }

#[derive(Debug)]
pub(crate) struct Frame<'a> { mpdu: &'a [u8], rate: u8 }
impl Frame<'_> {
    pub(crate) fn mpdu(&self) -> &[u8] { self.mpdu }
    pub(crate) fn rate(&self) -> u8 { self.rate }

    /// The admitted channel must remain stable from reception through delivery; drain old frames before retuning.
    pub(crate) fn write(&self, channel: u8, all_beacons: bool, output: &mut [u8]) -> Result<usize, Error> {
        if !(1..=14).contains(&channel) { return Err(Error::Channel); }
        let length = HEADER_BYTES + self.mpdu.len();
        if output.len() < length { return Err(Error::Output); }
        output[..HEADER_BYTES].copy_from_slice(&[(self.mpdu.len() & 255) as u8, (self.mpdu.len() >> 8) as u8,
            0, channel, self.rate, u8::from(all_beacons), 0, 0]);
        output[HEADER_BYTES..length].copy_from_slice(self.mpdu);
        Ok(length)
    }
}

fn header_bytes(frame: &[u8]) -> Result<usize, Error> {
    if frame.len() < 2 || frame[0] & 3 != 0 { return Err(Error::Frame); }
    let fc = u16::from_le_bytes([frame[0], frame[1]]);
    let size = match (fc >> 2) & 3 {
        0 => 24 + if fc & 0x8000 != 0 { 4 } else { 0 },
        1 => if matches!((fc >> 4) & 15, 12 | 13) { 10 } else { 16 },
        2 => 24 + if fc & 0x300 == 0x300 { 6 } else { 0 }
            + if fc & 0x80 != 0 { 2 + if fc & 0x8000 != 0 { 4 } else { 0 } } else { 0 },
        _ => return Err(Error::Frame),
    };
    if frame.len() < size { return Err(Error::Frame); }
    Ok(size)
}

/// Decode one retained full PCI receive buffer; hardware CRC/ICV status remains part of the device trust contract.
/// AX advertises RX_INCLUDES_FCS; remove exactly four bytes and retain protected MPDUs for the station's software crypto.
pub(crate) fn decode(bytes: &[u8]) -> Result<Frame<'_>, Error> {
    let packet = rxdesc::parse(bytes).map_err(Error::Descriptor)?;
    from_packet(packet, bytes)
}

/// Use the same validated descriptor and retained bytes from the sole RX owner; trailing RXBD padding may be omitted.
pub(crate) fn from_packet(packet: rxdesc::Packet, bytes: &[u8]) -> Result<Frame<'_>, Error> {
    if packet.kind != rxdesc::TYPE_WIFI { return Err(Error::Kind); }
    if packet.crc_error || packet.icv_error { return Err(Error::Integrity); }
    if packet.hardware_decrypted { return Err(Error::Decrypted); }
    let flags = u32::from_le_bytes(bytes.get(16..20).ok_or(Error::Descriptor(rxdesc::Error::Truncated))?.try_into().unwrap());
    if flags & ((1 << 5) | (1 << 6)) != 0 { return Err(Error::Aggregate); }
    let info = u32::from_le_bytes(bytes.get(8..12).ok_or(Error::Descriptor(rxdesc::Error::Truncated))?.try_into().unwrap());
    let rate = *RATES.get(((info >> 16) & 0x1ff) as usize).ok_or(Error::Rate)?;
    let payload = packet.bytes(bytes).ok_or(Error::Frame)?;
    let length = payload.len().checked_sub(4).filter(|n| *n <= FRAME_MAX).ok_or(Error::Frame)?;
    let mpdu = &payload[..length];
    header_bytes(mpdu)?;
    Ok(Frame { mpdu, rate })
}
