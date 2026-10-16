#![allow(dead_code)]
#[path = "../rtw89_wifi/src/rxdesc.rs"] mod rxdesc;
#[path = "../rtw89_wifi/src/frames.rs"] mod frames;

fn packet(frame: &[u8], rate: u16, long: bool, shift: usize, driver: usize) -> Vec<u8> {
    let offset = 4 + if long { 32 } else { 16 } + shift + driver;
    let length = offset + frame.len() + 4;
    let mut bytes = vec![0xa5; length];
    bytes[..4].copy_from_slice(&(length as u32 | 0xc000).to_le_bytes());
    bytes[4..8].copy_from_slice(&((frame.len() as u32 + 4) | ((shift / 2) as u32) << 14
        | ((driver / 8) as u32) << 28 | if long { 1 << 31 } else { 0 }).to_le_bytes());
    bytes[8..12].copy_from_slice(&(u32::from(rate) << 16).to_le_bytes());
    bytes[16..20].fill(0);
    bytes[offset..offset + frame.len()].copy_from_slice(frame);
    bytes[offset + frame.len()..].copy_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    bytes
}
fn frame(length: usize, fc: u16) -> Vec<u8> {
    let mut bytes = vec![0x31; length];
    bytes[..2].copy_from_slice(&fc.to_le_bytes()); bytes
}

#[test]
fn short_and_long_descriptors_strip_only_fcs_and_serialize_exact_wire_header() {
    let mpdu = frame(24, 0x80);
    for long in [false, true] { for shift in [0, 2, 6] { for driver in [0, 8, 56] {
        let raw = packet(&mpdu, 4, long, shift, driver);
        let decoded = frames::decode(&raw).unwrap();
        assert_eq!(decoded.mpdu(), mpdu); assert_eq!(decoded.rate(), 12);
        let mut out = [0xcc; 40];
        assert_eq!(decoded.write(6, true, &mut out), Ok(32));
        assert_eq!(&out[..8], &[24, 0, 0, 6, 12, 1, 0, 0]);
        assert_eq!(&out[8..32], mpdu); assert_eq!(&out[32..], &[0xcc; 8]);
    }}}
}
#[test]
fn legacy_rates_are_mapped_and_mcs_or_reserved_rates_are_not_invented() {
    for (raw, expected) in [2, 4, 11, 22, 12, 18, 24, 36, 48, 72, 96, 108].into_iter().enumerate() {
        assert_eq!(frames::decode(&packet(&frame(24, 0), raw as u16, false, 0, 0)).unwrap().rate(), expected);
    }
    for rate in [12, 0x80, 0x100, 0x1ff] {
        assert_eq!(frames::decode(&packet(&frame(24, 0), rate, false, 0, 0)).unwrap_err(), frames::Error::Rate);
    }
}
#[test]
fn bad_integrity_hardware_decryption_and_cut_aggregates_cannot_be_exposed_as_raw_frames() {
    for (flag, error) in [(1 << 9, frames::Error::Integrity), (1 << 10, frames::Error::Integrity),
        (4, frames::Error::Decrypted), (1 << 5, frames::Error::Aggregate), (1 << 6, frames::Error::Aggregate)] {
        let mut raw = packet(&frame(24, 0), 0, false, 0, 0);
        raw[16..20].copy_from_slice(&(flag as u32).to_le_bytes());
        assert_eq!(frames::decode(&raw).unwrap_err(), error);
    }
    let mpdu = frame(40, 0x4008);
    assert_eq!(frames::decode(&packet(&mpdu, 4, true, 0, 0)).unwrap().mpdu(), mpdu);
}
#[test]
fn variable_mac_headers_and_maximum_frame_are_checked_after_fcs_removal() {
    for (fc, length) in [(0x80, 24), (0x8080, 28), (0xd4, 10), (0xb4, 16),
        (0x08, 24), (0x0308, 30), (0x88, 26), (0x8388, 36)] {
        frames::decode(&packet(&frame(length, fc), 1, false, 0, 0)).unwrap();
        assert_eq!(frames::decode(&packet(&frame(length - 1, fc), 1, false, 0, 0)).unwrap_err(), frames::Error::Frame);
    }
    frames::decode(&packet(&frame(2346, 0x08), 1, false, 0, 0)).unwrap();
    assert_eq!(frames::decode(&packet(&frame(2347, 0x08), 1, false, 0, 0)).unwrap_err(), frames::Error::Frame);
    for fc in [1, 12] { assert_eq!(frames::decode(&packet(&frame(24, fc), 1, false, 0, 0)).unwrap_err(), frames::Error::Frame); }
}
#[test]
fn non_wifi_and_truncated_buffers_fail_without_touching_output() {
    let mut raw = packet(&frame(24, 0), 1, false, 0, 0);
    for end in 0..raw.len() { assert!(frames::decode(&raw[..end]).is_err()); }
    let decoded = frames::decode(&raw).unwrap(); let mut out = [0xcc; 31];
    assert_eq!(decoded.write(6, false, &mut out), Err(frames::Error::Output)); assert_eq!(out, [0xcc; 31]);
    for channel in [0, 15, 255] { assert_eq!(decoded.write(channel, false, &mut out), Err(frames::Error::Channel)); }
    raw[7] |= rxdesc::TYPE_C2H;
    assert_eq!(frames::decode(&raw).unwrap_err(), frames::Error::Kind);
}
