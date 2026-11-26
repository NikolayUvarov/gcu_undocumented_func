//! The LE central (700-BLT-0009) against a simulated peripheral that computes its side of SMP with the same
//! functions (themselves checked against the specification's sample data in smp.rs): LE Secure Connections and legacy
//! Just Works, the keys distributed and kept, a later link encrypted with them, GATT discovery of the HID service and a
//! boot keyboard's notification handed up, a report map's keyboard ID. Evidence for these exchanges only (MC-12.1).
mod common;
use bt_host::buf::Addr;
use bt_host::hci;
use bt_host::l2cap;
use bt_host::le::{self, keyboard_id};
use bt_host::smp_crypto as sc;
use common::*;

const MOUSE: Addr = Addr([0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xCA]);
const LOCAL: Addr = Addr([0xA6, 0x6B, 0xF0, 0x2D, 0x79, 0x1C]);
const H: u16 = 0x0040;

fn a56(random: bool, a: Addr) -> [u8; 7] { let mut v = [0u8; 7]; v[0] = random as u8; for i in 0..6 { v[1 + i] = a.0[5 - i]; } v }

/// A stack with the LE device listed, its link up: our Pairing Request's frame.
fn linked() -> (Sim, Vec<u8>) {
    let mut s = Sim::new("CA:BB:CC:DD:EE:FF le-hid random\n");
    s.stack.le.local = LOCAL;
    s.stack.le.rng.seed(&[7; 32]);
    let c = s.tick(10);
    assert_eq!(ops(&c), vec![hci::LE_CREATE_CONNECTION]);
    assert_eq!((c[0].1[5], &c[0].1[6..12]), (1, &MOUSE.0[..]));
    let mut ev = vec![hci::LE_CONNECTION_COMPLETE, 0, H as u8, 0, 0, 1];
    ev.extend_from_slice(&MOUSE.0);
    ev.extend_from_slice(&[0x18, 0, 0, 0, 0xF4, 0x01, 0]);
    s.event(hci::EV_LE_META, &ev);
    let f = s.frames();
    assert_eq!((f[0].0, f[0].1), (H, l2cap::SMP));
    (s, f[0].2.clone())
}

fn smp_out(s: &mut Sim) -> Vec<Vec<u8>> { s.frames().into_iter().filter(|f| f.1 == l2cap::SMP).map(|f| f.2).collect() }
fn att_out(s: &mut Sim) -> Vec<Vec<u8>> { s.frames().into_iter().filter(|f| f.1 == l2cap::ATT).map(|f| f.2).collect() }

#[test]
fn secure_connections_just_works_then_hid_over_gatt() {
    let (mut s, req) = linked();
    assert_eq!(req, vec![le::PAIRING_REQUEST, 3, 0, 0x09, 16, 0, 3]);
    // The peripheral: KeyboardOnly would be 2; this one is NoInputNoOutput too, with SC, bonding; it gives its identity.
    let rsp = [le::PAIRING_RESPONSE, 3, 0, 0x09, 16, 0, 2];
    s.send(H, l2cap::SMP, &rsp, 27);
    let out = smp_out(&mut s);
    assert_eq!((out[0][0], out[0].len()), (le::PUBLIC_KEY, 65));
    let pka_x: [u8; 32] = sc::rev(out[0][1..33].try_into().unwrap());
    let pka_y: [u8; 32] = sc::rev(out[0][33..65].try_into().unwrap());
    let (b, pkb_x, pkb_y) = sc::keypair(&[0x5A; 32]).unwrap();
    let dh = sc::dhkey(&b, &pka_x, &pka_y).unwrap();
    s.send(H, l2cap::SMP, &[&[le::PUBLIC_KEY][..], &sc::rev(pkb_x), &sc::rev(pkb_y)].concat(), 27);
    assert!(smp_out(&mut s).is_empty());
    // Its confirm, then the randoms.
    let nb: sc::Key = [0x3C; 16];
    let cb = sc::f4(&pkb_x, &pka_x, &nb, 0);
    s.send(H, l2cap::SMP, &[&[le::PAIRING_CONFIRM][..], &sc::rev(cb)].concat(), 27);
    let out = smp_out(&mut s);
    assert_eq!(out[0][0], le::PAIRING_RANDOM);
    let na: sc::Key = sc::rev(out[0][1..17].try_into().unwrap());
    s.send(H, l2cap::SMP, &[&[le::PAIRING_RANDOM][..], &sc::rev(nb)].concat(), 27);
    let out = smp_out(&mut s);
    // Our DHKey check is what the peripheral computes for us.
    let (a, bb) = (a56(false, LOCAL), a56(true, MOUSE));
    let (mackey, ltk) = sc::f5(&dh, &na, &nb, &a, &bb);
    assert_eq!(out[0], [&[le::DHKEY_CHECK][..], &sc::rev(sc::f6(&mackey, &na, &nb, &[0; 16], &[0x09, 0, 3], &a, &bb))].concat());
    let eb = sc::f6(&mackey, &nb, &na, &[0; 16], &[0x09, 0, 3], &bb, &a);
    s.send(H, l2cap::SMP, &[&[le::DHKEY_CHECK][..], &sc::rev(eb)].concat(), 27);
    let c = s.run();
    assert_eq!(c[0].0, hci::LE_START_ENCRYPTION);
    assert_eq!((&c[0].1[2..12], &c[0].1[12..]), (&[0u8; 10][..], &sc::rev(ltk)[..]));
    assert!(s.logged("SECURE CONNECTIONS") && s.logged("JUST WORKS"));
    s.event(hci::EV_ENCRYPTION_CHANGE, &[0, H as u8, 0, 1]);
    assert_eq!(s.ups(), vec!["keys"]);
    assert_eq!(s.stack.devices.get(MOUSE).unwrap().ltk.map(|k| k.key), Some(sc::rev(ltk)));
    // Its identity: then pairing is done and GATT starts.
    s.send(H, l2cap::SMP, &[le::IDENTITY_INFORMATION, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16], 27);
    assert!(att_out(&mut s).is_empty());
    s.send(H, l2cap::SMP, &[le::IDENTITY_ADDRESS, 1, 0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xCA], 27);
    assert!(s.logged("PAIRING DONE"));
    let out = att_out(&mut s);
    assert_eq!(out[0], vec![le::MTU_REQUEST, 247, 0]);
    // A server's answers: MTU 64, the HID service 0x0010-0x0020, its characteristics, one descriptor, the map.
    s.send(H, l2cap::ATT, &[le::MTU_RESPONSE, 64, 0], 27);
    assert_eq!(att_out(&mut s)[0], vec![le::FIND_BY_TYPE, 1, 0, 0xFF, 0xFF, 0x00, 0x28, 0x12, 0x18]);
    s.send(H, l2cap::ATT, &[le::FIND_BY_TYPE_RESPONSE, 0x10, 0, 0x20, 0], 27);
    assert_eq!(att_out(&mut s)[0], vec![le::READ_BY_TYPE, 0x10, 0, 0x20, 0, 0x03, 0x28]);
    s.send(H, l2cap::ATT, &[le::READ_BY_TYPE_RESPONSE, 7, 0x11, 0, 0x12, 0x12, 0, 0x22, 0x2A, 0x14, 0, 0x02, 0x15, 0, 0x4B, 0x2A, 0x16, 0, 0x04, 0x17, 0, 0x4E, 0x2A], 27);
    assert_eq!(att_out(&mut s)[0], vec![le::READ_BY_TYPE, 0x17, 0, 0x20, 0, 0x03, 0x28]);
    s.send(H, l2cap::ATT, &[le::ERROR, le::READ_BY_TYPE, 0x17, 0, le::ATTRIBUTE_NOT_FOUND], 27);
    assert_eq!(att_out(&mut s)[0], vec![le::FIND_INFORMATION, 0x13, 0, 0x20, 0]);
    s.send(H, l2cap::ATT, &[le::FIND_INFORMATION_RESPONSE, 1, 0x13, 0, 0x02, 0x29], 27);
    assert_eq!(att_out(&mut s)[0], vec![le::FIND_INFORMATION, 0x14, 0, 0x20, 0]);
    s.send(H, l2cap::ATT, &[le::ERROR, le::FIND_INFORMATION, 0x14, 0, le::ATTRIBUTE_NOT_FOUND], 27);
    assert_eq!(att_out(&mut s)[0], vec![le::READ, 0x15, 0]);
    s.send(H, l2cap::ATT, &[le::READ_RESPONSE, 0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0x85, 0x01, 0xC0], 27);
    // Boot protocol (written without an answer), then the boot keyboard's notifications turned on.
    let out = att_out(&mut s);
    assert_eq!(out, vec![vec![le::WRITE_COMMAND, 0x17, 0, 0], vec![le::WRITE_REQUEST, 0x13, 0, 1, 0]]);
    s.send(H, l2cap::ATT, &[le::WRITE_RESPONSE], 27);
    assert!(s.logged("HID READY") && s.logged("BOOT PROTOCOL, REPORT MAP 9 BYTES"));
    // "b" pressed.
    s.send(H, l2cap::ATT, &[le::NOTIFICATION, 0x12, 0, 0, 0, 0x05, 0, 0, 0, 0, 0], 27);
    assert_eq!(s.ups(), vec!["keyboard [00, 00, 05, 00, 00, 00, 00, 00]"]);
    // The peripheral asks us as a server: an MTU answered, discovery finds nothing.
    s.send(H, l2cap::ATT, &[le::MTU_REQUEST, 100, 0], 27);
    s.send(H, l2cap::ATT, &[le::READ_BY_GROUP, 1, 0, 0xFF, 0xFF, 0x00, 0x28], 27);
    assert_eq!(att_out(&mut s), vec![vec![le::MTU_RESPONSE, 247, 0], vec![le::ERROR, le::READ_BY_GROUP, 1, 0, le::ATTRIBUTE_NOT_FOUND]]);
    // The link goes and comes back: encrypted with the key kept, no pairing.
    s.event(hci::EV_DISCONNECTION_COMPLETE, &[0, H as u8, 0, 0x13]);
    s.tick(3_000);
    let mut ev = vec![hci::LE_CONNECTION_COMPLETE, 0, 0x41, 0, 0, 1];
    ev.extend_from_slice(&MOUSE.0);
    ev.extend_from_slice(&[0x18, 0, 0, 0, 0xF4, 0x01, 0]);
    let c = s.event(hci::EV_LE_META, &ev);
    assert_eq!((c[0].0, &c[0].1[12..]), (hci::LE_START_ENCRYPTION, &sc::rev(ltk)[..]));
    assert!(smp_out(&mut s).is_empty());
}

#[test]
fn legacy_just_works_and_the_keys_distributed() {
    let (mut s, req) = linked();
    // No Secure Connections bit: legacy; it distributes its LTK (EncKey) and identity.
    let rsp = [le::PAIRING_RESPONSE, 3, 0, 0x01, 16, 0, 3];
    s.send(H, l2cap::SMP, &rsp, 27);
    let out = smp_out(&mut s);
    assert_eq!(out[0][0], le::PAIRING_CONFIRM);
    let mconfirm: sc::Key = sc::rev(out[0][1..17].try_into().unwrap());
    let preq: [u8; 7] = sc::rev(req.try_into().unwrap());
    let pres: [u8; 7] = sc::rev(rsp);
    let (ia, ra) = (sc::rev(LOCAL.0), sc::rev(MOUSE.0));
    let srand: sc::Key = [0x42; 16];
    let sconfirm = sc::c1(&[0; 16], &srand, &preq, &pres, 0, &ia, 1, &ra);
    s.send(H, l2cap::SMP, &[&[le::PAIRING_CONFIRM][..], &sc::rev(sconfirm)].concat(), 27);
    let out = smp_out(&mut s);
    let mrand: sc::Key = sc::rev(out[0][1..17].try_into().unwrap());
    assert_eq!(sc::c1(&[0; 16], &mrand, &preq, &pres, 0, &ia, 1, &ra), mconfirm);
    s.send(H, l2cap::SMP, &[&[le::PAIRING_RANDOM][..], &sc::rev(srand)].concat(), 27);
    let c = s.run();
    let stk = sc::s1(&[0; 16], &srand, &mrand);
    assert_eq!((c[0].0, &c[0].1[12..]), (hci::LE_START_ENCRYPTION, &sc::rev(stk)[..]));
    s.event(hci::EV_ENCRYPTION_CHANGE, &[0, H as u8, 0, 1]);
    // Its LTK, EDIV and Rand, then its identity.
    s.send(H, l2cap::SMP, &[&[le::ENCRYPTION_INFORMATION][..], &[0x77; 16]].concat(), 27);
    s.send(H, l2cap::SMP, &[le::CENTRAL_IDENTIFICATION, 0x34, 0x12, 1, 2, 3, 4, 5, 6, 7, 8], 27);
    s.send(H, l2cap::SMP, &[le::IDENTITY_INFORMATION, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 27);
    s.send(H, l2cap::SMP, &[le::IDENTITY_ADDRESS, 1, 0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xCA], 27);
    let k = s.stack.devices.get(MOUSE).unwrap().ltk.unwrap();
    assert_eq!((k.key, k.ediv, k.rand), ([0x77; 16], 0x1234, [1, 2, 3, 4, 5, 6, 7, 8]));
    assert!(s.logged("PAIRING DONE") && s.logged("LONG TERM KEY KEPT"));
    assert_eq!(att_out(&mut s)[0][0], le::MTU_REQUEST);
}

#[test]
fn a_wrong_confirm_value_fails_the_pairing() {
    let (mut s, _) = linked();
    s.send(H, l2cap::SMP, &[le::PAIRING_RESPONSE, 3, 0, 0x01, 16, 0, 3], 27);
    smp_out(&mut s);
    s.send(H, l2cap::SMP, &[&[le::PAIRING_CONFIRM][..], &[9; 16]].concat(), 27);
    smp_out(&mut s);
    s.send(H, l2cap::SMP, &[&[le::PAIRING_RANDOM][..], &[1; 16]].concat(), 27);
    assert_eq!(smp_out(&mut s), vec![vec![le::PAIRING_FAILED, le::CONFIRM_FAILED]]);
    assert!(s.logged("PAIRING FAILED: ITS CONFIRM VALUE"));
    // A key size under 7: refused.
    let (mut s, _) = linked();
    s.send(H, l2cap::SMP, &[le::PAIRING_RESPONSE, 3, 0, 0x09, 6, 0, 3], 27);
    assert_eq!(smp_out(&mut s), vec![vec![le::PAIRING_FAILED, le::KEY_SIZE]]);
}

#[test]
fn a_report_maps_keyboard_id() {
    // A mouse (ID 1) then a keyboard (ID 2), as a combined device's map has them.
    let map = [0x05, 0x01, 0x09, 0x02, 0xA1, 0x01, 0x85, 0x01, 0x09, 0x01, 0xA1, 0x00, 0xC0, 0xC0,
               0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0x85, 0x02, 0x05, 0x07, 0x19, 0xE0, 0x29, 0xE7, 0xC0];
    assert_eq!(keyboard_id(&map), Some(2));
    assert_eq!(keyboard_id(&map[..14]), None);
    // Cut short: none.
    assert_eq!(keyboard_id(&[0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0x85]), None);
}
