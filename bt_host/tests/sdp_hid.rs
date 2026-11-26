//! The SDP server (700-BLT-0007) and the classic HID host (700-BLT-0008) against constructed PDUs: searches answered
//! with the A2DP source's record and continued when the client takes less, a keyboard's and a mouse's channels opened,
//! the boot protocol set, their reports handed up. Evidence for these encodings only (MC-12.1).
mod common;
use bt_host::hci;
use bt_host::l2cap;
use bt_host::sdp::{self, answer, element, uuid32};
use common::*;

fn request(id: u8, tid: u16, params: &[u8]) -> Vec<u8> {
    let mut v = vec![id, (tid >> 8) as u8, tid as u8, (params.len() >> 8) as u8, params.len() as u8];
    v.extend_from_slice(params);
    v
}

#[test]
fn data_elements_are_read() {
    let e = element(&[0x35, 0x03, 0x19, 0x11, 0x0A, 0xFF]).unwrap();
    assert_eq!((e.kind, e.len, e.value), (sdp::SEQUENCE, 5, &[0x19, 0x11, 0x0A][..]));
    let u = element(e.value).unwrap();
    assert_eq!(uuid32(&u), Some(0x110A));
    // A 128-bit UUID on the base UUID, and one that is not.
    let long = [0x1C, 0, 0, 0x11, 0x0A, 0, 0, 0x10, 0, 0x80, 0, 0, 0x80, 0x5F, 0x9B, 0x34, 0xFB];
    assert_eq!(uuid32(&element(&long).unwrap()), Some(0x110A));
    let mut other = long;
    other[16] = 0;
    assert_eq!(uuid32(&element(&other).unwrap()), None);
    // Cut short.
    assert!(element(&[0x35, 0x05, 0x19]).is_none());
    assert!(element(&[0x0A, 1, 2]).is_none());
}

#[test]
fn a_search_for_the_a2dp_source_is_answered() {
    let mut out = [0u8; 512];
    // Service Search Attribute for Audio Source (0x110A), every attribute, up to 0x200 bytes.
    let r = request(sdp::SEARCH_ATTRIBUTE_REQUEST, 7, &[0x35, 0x03, 0x19, 0x11, 0x0A, 0x02, 0x00, 0x35, 0x05, 0x0A, 0x00, 0x00, 0xFF, 0xFF, 0x00]);
    let n = answer(&r, &mut out);
    assert_eq!(&out[..3], &[sdp::SEARCH_ATTRIBUTE_RESPONSE, 0, 7]);
    let count = u16::from_be_bytes([out[5], out[6]]) as usize;
    assert_eq!(n, 7 + count + 1);
    assert_eq!(out[n - 1], 0);
    // A list of one record's list: its handle, class, protocols (L2CAP PSM 0x0019, AVDTP 1.3), group, profile, features.
    let lists = element(&out[7..7 + count]).unwrap();
    let record = element(lists.value).unwrap();
    let mut ids = Vec::new();
    let mut rest = record.value;
    while !rest.is_empty() {
        let id = element(rest).unwrap();
        let value = element(&rest[id.len..]).unwrap();
        ids.push(u16::from_be_bytes([id.value[0], id.value[1]]));
        rest = &rest[id.len + value.len..];
    }
    assert_eq!(ids, vec![0x0000, 0x0001, 0x0004, 0x0005, 0x0009, 0x0311]);
    // A search for something we have not (HID, 0x1124): an empty list.
    let r = request(sdp::SEARCH_ATTRIBUTE_REQUEST, 8, &[0x35, 0x03, 0x19, 0x11, 0x24, 0x02, 0x00, 0x35, 0x05, 0x0A, 0x00, 0x00, 0xFF, 0xFF, 0x00]);
    let n = answer(&r, &mut out);
    assert_eq!(&out[5..n], &[0, 3, 0x36, 0, 0, 0]);
    // Service Search: the record's handle.
    let r = request(sdp::SEARCH_REQUEST, 9, &[0x35, 0x03, 0x19, 0x01, 0x00, 0x00, 0x05, 0x00]);
    let n = answer(&r, &mut out);
    assert_eq!(&out[..n], &[sdp::SEARCH_RESPONSE, 0, 9, 0, 9, 0, 1, 0, 1, 0, 1, 0, 1, 0]);
    // Service Attribute for that handle, the protocol list alone.
    let r = request(sdp::ATTRIBUTE_REQUEST, 10, &[0, 1, 0, 1, 0x01, 0x00, 0x35, 0x03, 0x09, 0x00, 0x04, 0x00]);
    let n = answer(&r, &mut out);
    assert_eq!(&out[7..n - 1], &[0x36, 0, 21, 0x09, 0, 4, 0x35, 0x10, 0x35, 0x06, 0x19, 0x01, 0x00, 0x09, 0x00, 0x19, 0x35, 0x06, 0x19, 0x00, 0x19, 0x09, 0x01, 0x03]);
    // An unknown handle; garbage.
    let r = request(sdp::ATTRIBUTE_REQUEST, 11, &[0, 9, 0, 9, 0x01, 0x00, 0x35, 0x03, 0x09, 0x00, 0x04, 0x00]);
    let n = answer(&r, &mut out);
    assert_eq!(&out[..n], &[sdp::ERROR, 0, 11, 0, 2, 0, 2]);
    let n = answer(&request(0x7F, 12, &[]), &mut out);
    assert_eq!(&out[..n], &[sdp::ERROR, 0, 12, 0, 2, 0, 3]);
}

#[test]
fn a_long_answer_is_continued() {
    let mut out = [0u8; 512];
    let mut got = Vec::new();
    let mut cont = vec![0u8];
    // At most 16 bytes an answer.
    for _ in 0..20 {
        let mut p = vec![0x35, 0x03, 0x19, 0x11, 0x0A, 0x00, 0x10, 0x35, 0x05, 0x0A, 0x00, 0x00, 0xFF, 0xFF];
        p.extend_from_slice(&cont);
        let n = answer(&request(sdp::SEARCH_ATTRIBUTE_REQUEST, 1, &p), &mut out);
        let count = u16::from_be_bytes([out[5], out[6]]) as usize;
        assert!(count <= 16);
        got.extend_from_slice(&out[7..7 + count]);
        cont = out[7 + count..n].to_vec();
        if cont == [0] { break; }
    }
    assert_eq!(cont, vec![0]);
    let whole = element(&got).unwrap();
    assert_eq!(whole.len, got.len());
    // A continuation state past the end.
    let n = answer(&request(sdp::SEARCH_ATTRIBUTE_REQUEST, 2, &[0x35, 0x03, 0x19, 0x11, 0x0A, 0x00, 0x10, 0x35, 0x05, 0x0A, 0x00, 0x00, 0xFF, 0xFF, 2, 0x10, 0]), &mut out);
    assert_eq!(&out[..n], &[sdp::ERROR, 0, 2, 0, 2, 0, 5]);
}

/// A keyboard after pairing; the HID control channel we opened, configured.
fn keyboard() -> (Sim, u16) {
    let mut s = Sim::new("11:22:33:44:55:66 hid\n");
    s.tick(10);
    s.connected(0x0B);
    s.pair(0x0B);
    s.encrypted(0x0B);
    let f = s.frames();
    assert_eq!(le16(&f[0].2, 4), l2cap::PSM_HID_CONTROL);
    let (ident, control) = (f[0].2[1], le16(&f[0].2, 6));
    configure(&mut s, ident, control, 0x0070);
    (s, control)
}

fn configure(s: &mut Sim, ident: u8, local: u16, remote: u16) {
    let (l, r) = (local.to_le_bytes(), remote.to_le_bytes());
    s.send(0x0B, l2cap::SIGNALLING, &sig(l2cap::CONNECTION_RESPONSE, ident, &[r[0], r[1], l[0], l[1], 0, 0, 0, 0]), 64);
    let f = s.frames();
    s.send(0x0B, l2cap::SIGNALLING, &[sig(l2cap::CONFIGURATION_RESPONSE, f[0].2[1], &[l[0], l[1], 0, 0, 0, 0]), sig(l2cap::CONFIGURATION_REQUEST, 0x30, &[l[0], l[1], 0, 0])].concat(), 64);
}

#[test]
fn a_keyboard_and_its_reports() {
    let (mut s, control) = keyboard();
    assert_eq!(s.ups(), vec!["keys"]);
    // Control open: Set Protocol (boot), then the interrupt channel asked for.
    let f = s.frames();
    let set = f.iter().find(|f| f.1 == 0x70).unwrap();
    assert_eq!(set.2, vec![0x70]);
    let req = f.iter().find(|f| f.1 == l2cap::SIGNALLING && f.2[0] == l2cap::CONNECTION_REQUEST).unwrap();
    assert_eq!(le16(&req.2, 4), l2cap::PSM_HID_INTERRUPT);
    let interrupt = le16(&req.2, 6);
    s.send(0x0B, control, &[0x00], 64);
    assert!(s.logged("BOOT PROTOCOL SET"));
    configure(&mut s, req.2[1], interrupt, 0x0071);
    s.frames();
    assert!(s.logged("HID 00B: READY"));
    // "a" pressed then released, as boot reports with ID 1.
    s.send(0x0B, interrupt, &[0xA1, 0x01, 0x00, 0x00, 0x04, 0, 0, 0, 0, 0], 64);
    s.send(0x0B, interrupt, &[0xA1, 0x01, 0x00, 0x00, 0x00, 0, 0, 0, 0, 0], 64);
    // A report of another type, and one too short: left out.
    s.send(0x0B, interrupt, &[0xA2, 0x01, 0x00], 64);
    s.send(0x0B, interrupt, &[0xA1, 0x01, 0x00], 64);
    assert_eq!(s.ups(), vec!["keyboard [00, 00, 04, 00, 00, 00, 00, 00]", "keyboard [00, 00, 00, 00, 00, 00, 00, 00]"]);
    // The lights go out on the interrupt channel.
    s.stack.hid.lights(&mut s.stack.ctl, &s.stack.chans, 0x02);
    assert_eq!(s.frames().last().map(|f| (f.1, f.2.clone())), Some((0x71, vec![0xA2, 0x01, 0x02])));
    // The link goes with a key held: released.
    s.send(0x0B, interrupt, &[0xA1, 0x01, 0x02, 0x00, 0x05, 0, 0, 0, 0, 0], 64);
    s.ups();
    s.event(hci::EV_DISCONNECTION_COMPLETE, &[0, 0x0B, 0, 0x08]);
    assert_eq!(s.ups(), vec!["keyboard [00, 00, 00, 00, 00, 00, 00, 00]"]);
}

#[test]
fn a_mouse_that_reconnects_opens_its_own_channels() {
    let mut s = Sim::new("11:22:33:44:55:66 hid\n");
    let mut req = PEER.0.to_vec();
    req.extend_from_slice(&[0x80, 0x25, 0x00, 1]);
    s.event(hci::EV_CONNECTION_REQUEST, &req);
    s.connected(0x0B);
    // It has the key: Link Key Request answered, it authenticates and encrypts, then opens control and interrupt.
    s.stack.devices.get_mut(PEER).unwrap().key = Some(([7; 16], 4));
    assert_eq!(ops(&s.event(hci::EV_LINK_KEY_REQUEST, &PEER.0)), vec![hci::LINK_KEY_REPLY]);
    s.encrypted(0x0B);
    for (ident, psm, cid) in [(1u8, l2cap::PSM_HID_CONTROL, 0x40u16), (2, l2cap::PSM_HID_INTERRUPT, 0x41)] {
        let (p, c) = (psm.to_le_bytes(), cid.to_le_bytes());
        s.send(0x0B, l2cap::SIGNALLING, &sig(l2cap::CONNECTION_REQUEST, ident, &[p[0], p[1], c[0], c[1]]), 64);
        let f = s.frames();
        let local = le16(&f[0].2, 4);
        assert_eq!(&f[0].2[8..12], &[0, 0, 0, 0], "accepted at once: the link is secure");
        let l = local.to_le_bytes();
        s.send(0x0B, l2cap::SIGNALLING, &[sig(l2cap::CONFIGURATION_RESPONSE, f[1].2[1], &[l[0], l[1], 0, 0, 0, 0]), sig(l2cap::CONFIGURATION_REQUEST, 9, &[l[0], l[1], 0, 0])].concat(), 64);
        s.frames();
    }
    let interrupt = s.stack.chans.find(0x0B, l2cap::PSM_HID_INTERRUPT).unwrap().local;
    // A boot mouse report: buttons, X, Y, wheel.
    s.send(0x0B, interrupt, &[0xA1, 0x02, 0x01, 0x05, 0xFB, 0x00], 64);
    assert_eq!(s.ups(), vec!["pointer [01, 05, FB, 00]"]);
}
