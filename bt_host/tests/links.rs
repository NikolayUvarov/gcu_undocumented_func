//! Links (700-BLT-0005, 0006): ACL packets joined and reassembled, the controller's credits, L2CAP channels both ways,
//! and a BR/EDR link made, paired and secured against a simulated controller. Evidence for these state machines only:
//! a real controller and device are the DEXP's and the Ryzen's to show (MC-12.1).
mod common;
use bt_host::acl::{self, Pdu, Stream};
use bt_host::buf::{Addr, Queue};
use bt_host::ctl::{Ctl, Pool};
use bt_host::hci;
use bt_host::l2cap::{self, Admit, Change, Changes, Channels};
use common::*;

#[test]
fn acl_packets_are_joined_from_usb_packets() {
    let mut s = Stream::new(64);
    let mut got = Vec::new();
    // A 300-byte packet in 64-byte USB packets, then two small ones in one USB packet.
    let mut big = acl::header(0x002A, acl::PB_FIRST, 300).to_vec();
    big.extend((0..300).map(|i| i as u8));
    for usb in big.chunks(64) { s.feed(usb, |p| got.push(p.to_vec())); }
    let mut two = acl::header(0x002A, acl::PB_CONTINUING, 2).to_vec();
    two.extend_from_slice(&[7, 8]);
    two.extend_from_slice(&acl::header(0x002B, acl::PB_FIRST, 1));
    two.push(9);
    s.feed(&two, |p| got.push(p.to_vec()));
    // An empty USB packet (a transfer's end) changes nothing.
    s.feed(&[], |p| got.push(p.to_vec()));
    assert_eq!(got.len(), 3);
    assert_eq!(acl::parse(&got[0]).map(|(h, pb, d)| (h, pb, d.len(), d[299])), Some((0x2A, acl::PB_FIRST, 300, 43)));
    assert_eq!(acl::parse(&got[1]).map(|(h, pb, d)| (h, pb, d.to_vec())), Some((0x2A, acl::PB_CONTINUING, vec![7, 8])));
    assert_eq!(acl::parse(&got[2]).map(|(h, _, d)| (h, d.to_vec())), Some((0x2B, vec![9])));
    // A header naming more than the buffer takes: skipped to its transfer's end, then the stream goes on.
    let mut bad = acl::header(1, acl::PB_FIRST, 4000).to_vec();
    bad.resize(64, 0);
    s.feed(&bad, |p| got.push(p.to_vec()));
    s.feed(&[0; 64], |p| got.push(p.to_vec()));
    s.feed(&[0; 10], |p| got.push(p.to_vec()));
    let mut ok = acl::header(3, acl::PB_FIRST, 1).to_vec();
    ok.push(5);
    s.feed(&ok, |p| got.push(p.to_vec()));
    assert_eq!((s.dropped, got.len()), (1, 4));
}

#[test]
fn l2cap_pdus_are_reassembled() {
    let mut pdu = Pdu::new();
    // 10 bytes of payload on CID 0x0040 in a start of 6 bytes and two continuations.
    let whole: Vec<u8> = [10, 0, 0x40, 0].into_iter().chain(1..=10).collect();
    assert!(pdu.feed(true, &whole[..6]).is_none());
    assert!(pdu.feed(false, &whole[6..10]).is_none());
    assert_eq!(pdu.feed(false, &whole[10..]).map(|p| p.to_vec()), Some(whole.clone()));
    // A continuation without a start, a start cut by another, a PDU over the limit: dropped and counted.
    assert!(pdu.feed(false, &[1, 2]).is_none());
    assert!(pdu.feed(true, &whole[..6]).is_none());
    assert_eq!(pdu.feed(true, &whole).map(|p| p.len()), Some(14));
    assert!(pdu.feed(true, &[0xFF, 0xFF, 0x40, 0]).is_none());
    assert_eq!(pdu.dropped, 3);
}

#[test]
fn the_controllers_buffers_limit_what_is_sent() {
    let mut ctl = Ctl::new();
    ctl.bredr = Pool { len: 27, total: 2, free: 2 };
    ctl.link_up(0x10, false);
    // 60 bytes of payload: a 64-byte PDU in three fragments of at most 27 bytes.
    assert!(ctl.l2cap(0x10, 0x40, &[&[0xAB; 40], &[0xCD; 20]]));
    let mut out = [0u8; 64];
    let first = ctl.next_acl(&mut out).map(|n| out[..n].to_vec()).unwrap();
    assert_eq!(acl::parse(&first).map(|(h, pb, d)| (h, pb, d.len(), d[0], d[2])), Some((0x10, acl::PB_FIRST, 27, 60, 0x40)));
    assert_eq!(ctl.next_acl(&mut out).map(|n| acl::parse(&out[..n]).unwrap().1), Some(acl::PB_CONTINUING));
    // Both buffers held: the third waits for Number Of Completed Packets.
    assert!(ctl.next_acl(&mut out).is_none());
    assert_eq!(ctl.backlog(0x10), 3);
    ctl.completed(0x10, 1);
    assert_eq!(ctl.next_acl(&mut out).map(|n| acl::parse(&out[..n]).unwrap().2.len()), Some(10));
    // A link gone: the packets the controller held are free again, and its queued ones dropped.
    assert!(ctl.l2cap(0x10, 0x40, &[&[1; 30]]));
    ctl.link_down(0x10);
    assert_eq!(ctl.bredr.free, 2);
    assert!(ctl.next_acl(&mut out).is_none());
    // Commands wait for the controller's credit; one never answered gives it back after its time.
    ctl.command(hci::RESET, &[]);
    ctl.command(hci::READ_BD_ADDR, &[]);
    assert_eq!(ctl.next_command(&mut out), Some(3));
    assert_eq!(ctl.next_command(&mut out), None);
    ctl.now += bt_host::ctl::COMMAND_MS + 1;
    assert_eq!(ctl.next_command(&mut out).map(|_| u16::from_le_bytes([out[0], out[1]])), Some(hci::READ_BD_ADDR));
    assert_eq!(ctl.lost_commands, 1);
}

#[test]
fn queues_keep_their_records_in_order() {
    let mut q: Queue<16> = Queue::new();
    assert!(q.push(&[&[1, 2], &[3]]));
    assert!(q.push(&[&[4; 5]]));
    assert!(!q.push(&[&[5; 8]]));
    assert_eq!(q.front(), Some(&[1, 2, 3][..]));
    q.pop();
    assert!(q.push(&[&[6; 6]]));
    let mut out = [0u8; 8];
    assert_eq!(q.take(&mut out), Some(5));
    assert_eq!(q.take(&mut out).map(|n| out[..n].to_vec()), Some(vec![6; 6]));
    assert!(q.is_empty());
    assert_eq!(q.dropped, 1);
}

fn link() -> (Ctl, Channels) {
    let mut ctl = Ctl::new();
    ctl.bredr = Pool { len: 1021, total: 8, free: 8 };
    ctl.link_up(0x20, false);
    (ctl, Channels::new())
}

/// The signalling commands the controller was given, decoded: code, identifier, data.
fn signals(ctl: &mut Ctl) -> Vec<(u8, u8, Vec<u8>)> {
    let mut out = [0u8; 2100];
    let mut got = Vec::new();
    while let Some(n) = ctl.next_acl(&mut out) {
        let (_, _, d) = acl::parse(&out[..n]).unwrap();
        assert_eq!(le16(d, 2), l2cap::SIGNALLING);
        let mut c = &d[4..];
        while c.len() >= 4 { let len = le16(c, 2) as usize; got.push((c[0], c[1], c[4..4 + len].to_vec())); c = &c[4 + len..]; }
        ctl.completed(0x20, 1);
    }
    got
}

#[test]
fn a_channel_we_open_is_configured_both_ways() {
    let (mut ctl, mut chans) = link();
    let mut ch = Changes::default();
    let local = chans.connect(&mut ctl, 0x20, l2cap::PSM_AVDTP).unwrap();
    let s = signals(&mut ctl);
    assert_eq!((s[0].0, s[0].2.clone()), (l2cap::CONNECTION_REQUEST, vec![0x19, 0, local as u8, (local >> 8) as u8]));
    let ident = s[0].1;
    // Pending, then success with the peer's CID 0x0071.
    chans.signal(&mut ctl, 0x20, &sig(l2cap::CONNECTION_RESPONSE, ident, &[0x71, 0, local as u8, 0, 1, 0, 0, 0]), |_| Admit::Refuse, &mut ch);
    assert!(signals(&mut ctl).is_empty());
    chans.signal(&mut ctl, 0x20, &sig(l2cap::CONNECTION_RESPONSE, ident, &[0x71, 0, local as u8, 0, 0, 0, 0, 0]), |_| Admit::Refuse, &mut ch);
    let s = signals(&mut ctl);
    // Our configuration: their CID, no flags, our MTU.
    assert_eq!((s[0].0, s[0].2.clone()), (l2cap::CONFIGURATION_REQUEST, vec![0x71, 0, 0, 0, 1, 2, (l2cap::MTU & 0xFF) as u8, (l2cap::MTU >> 8) as u8]));
    let our_conf = s[0].1;
    // Theirs, with an MTU of 895, a hint we do not know and a flush timeout; ours accepted.
    let theirs = [local as u8, 0, 0, 0, 0x01, 2, 0x7F, 0x03, 0x80 | 0x30, 1, 0, 0x02, 2, 0xFF, 0xFF];
    chans.signal(&mut ctl, 0x20, &sig(l2cap::CONFIGURATION_REQUEST, 9, &theirs), |_| Admit::Refuse, &mut ch);
    let s = signals(&mut ctl);
    assert_eq!((s[0].0, s[0].1, s[0].2.clone()), (l2cap::CONFIGURATION_RESPONSE, 9, vec![0x71, 0, 0, 0, 0, 0]));
    assert!(ch.iter().next().is_none());
    chans.signal(&mut ctl, 0x20, &sig(l2cap::CONFIGURATION_RESPONSE, our_conf, &[local as u8, 0, 0, 0, 0, 0]), |_| Admit::Refuse, &mut ch);
    assert_eq!(ch.iter().collect::<Vec<_>>(), vec![Change::Opened { handle: 0x20, local, psm: l2cap::PSM_AVDTP, initiator: true }]);
    assert_eq!(chans.get(local).map(|c| (c.remote, c.mtu, c.state)), Some((0x71, 895, l2cap::State::Open)));
    // Data goes to their CID; longer than their MTU is refused.
    assert!(chans.send(&mut ctl, 0x20, local, &[&[1, 2, 3]]));
    assert!(!chans.send(&mut ctl, 0x20, local, &[&[0; 896]]));
    let mut out = [0u8; 64];
    let n = ctl.next_acl(&mut out).unwrap();
    assert_eq!(&out[4..n], &[3, 0, 0x71, 0, 1, 2, 3]);
    // We close it: Disconnection Request, closed when answered.
    ctl.completed(0x20, 1);
    chans.disconnect(&mut ctl, 0x20, local);
    let s = signals(&mut ctl);
    assert_eq!((s[0].0, s[0].2.clone()), (l2cap::DISCONNECTION_REQUEST, vec![0x71, 0, local as u8, 0]));
    let mut ch = Changes::default();
    chans.signal(&mut ctl, 0x20, &sig(l2cap::DISCONNECTION_RESPONSE, s[0].1, &[0x71, 0, local as u8, 0]), |_| Admit::Refuse, &mut ch);
    assert_eq!(ch.iter().collect::<Vec<_>>(), vec![Change::Closed { handle: 0x20, local, psm: l2cap::PSM_AVDTP }]);
    assert!(chans.get(local).is_none());
}

#[test]
fn a_peers_channel_is_admitted_refused_or_held_for_security() {
    let (mut ctl, mut chans) = link();
    let mut ch = Changes::default();
    let admit = |psm: u16| match psm { l2cap::PSM_SDP => Admit::Accept, l2cap::PSM_HID_CONTROL => Admit::Pending, _ => Admit::Refuse };
    // SDP from CID 0x0045: accepted, our configuration follows.
    chans.signal(&mut ctl, 0x20, &sig(l2cap::CONNECTION_REQUEST, 3, &[0x01, 0, 0x45, 0]), admit, &mut ch);
    let s = signals(&mut ctl);
    assert_eq!(s.len(), 2);
    assert_eq!((s[0].0, s[0].1), (l2cap::CONNECTION_RESPONSE, 3));
    let local = le16(&s[0].2, 0);
    assert_eq!(&s[0].2[2..], &[0x45, 0, 0, 0, 0, 0]);
    assert_eq!(s[1].0, l2cap::CONFIGURATION_REQUEST);
    // A PSM not served: refused with "PSM not supported".
    chans.signal(&mut ctl, 0x20, &sig(l2cap::CONNECTION_REQUEST, 4, &[0x17, 0, 0x46, 0]), admit, &mut ch);
    let s = signals(&mut ctl);
    assert_eq!(&s[0].2, &[0, 0, 0x46, 0, 2, 0, 0, 0]);
    // HID control: pending (authentication), then admitted once the link is secure.
    chans.signal(&mut ctl, 0x20, &sig(l2cap::CONNECTION_REQUEST, 5, &[0x11, 0, 0x47, 0]), admit, &mut ch);
    let s = signals(&mut ctl);
    let held = le16(&s[0].2, 0);
    assert_eq!(&s[0].2[2..], &[0x47, 0, 1, 0, 1, 0]);
    assert_eq!(ch.iter().collect::<Vec<_>>(), vec![Change::Waiting { handle: 0x20, local: held, psm: 0x11 }]);
    chans.admit(&mut ctl, 0x20, held, true, &mut ch);
    let s = signals(&mut ctl);
    assert_eq!((s[0].0, s[0].1, &s[0].2[4..]), (l2cap::CONNECTION_RESPONSE, 5, &[0u8, 0, 0, 0][..]));
    assert_eq!(s[1].0, l2cap::CONFIGURATION_REQUEST);
    // Their configuration asks for enhanced retransmission: unacceptable, basic mode named; an unknown option that is
    // not a hint: unknown, named.
    let ertm = [local as u8, (local >> 8) as u8, 0, 0, 0x04, 9, 3, 0, 0, 0, 0, 0, 0, 0, 0];
    chans.signal(&mut ctl, 0x20, &sig(l2cap::CONFIGURATION_REQUEST, 6, &ertm), admit, &mut ch);
    let s = signals(&mut ctl);
    assert_eq!(&s[0].2[..6], &[0x45, 0, 0, 0, 1, 0]);
    assert_eq!(&s[0].2[6..9], &[0x04, 9, 0]);
    chans.signal(&mut ctl, 0x20, &sig(l2cap::CONFIGURATION_REQUEST, 7, &[local as u8, (local >> 8) as u8, 0, 0, 0x30, 1, 9]), admit, &mut ch);
    assert_eq!(&signals(&mut ctl)[0].2, &[0x45, 0, 0, 0, 3, 0, 0x30, 1, 9]);
    // Information and echo answered; an unknown code rejected as not understood.
    chans.signal(&mut ctl, 0x20, &[sig(l2cap::INFORMATION_REQUEST, 8, &[2, 0]), sig(l2cap::ECHO_REQUEST, 9, &[1, 2]), sig(0x7E, 10, &[])].concat(), admit, &mut ch);
    let s = signals(&mut ctl);
    assert_eq!(s, vec![(l2cap::INFORMATION_RESPONSE, 8, vec![2, 0, 0, 0, 0, 0, 0, 0]), (l2cap::ECHO_RESPONSE, 9, vec![1, 2]), (l2cap::COMMAND_REJECT, 10, vec![0, 0])]);
    // The peer closes SDP before it opened: gone, answered.
    chans.signal(&mut ctl, 0x20, &sig(l2cap::DISCONNECTION_REQUEST, 11, &[local as u8, (local >> 8) as u8, 0x45, 0]), admit, &mut ch);
    assert_eq!(signals(&mut ctl)[0], (l2cap::DISCONNECTION_RESPONSE, 11, vec![local as u8, (local >> 8) as u8, 0x45, 0]));
    assert!(chans.get(local).is_none());
    // The link goes: the held channel goes with it.
    let mut ch = Changes::default();
    chans.link_down(0x20, &mut ch);
    assert!(chans.get(held).is_none());
}

#[test]
fn le_parameter_updates_are_checked() {
    let (mut ctl, _) = link();
    ctl.link_up(0x40, true);
    ctl.le = Pool { len: 27, total: 4, free: 4 };
    let ok = l2cap::le_signal(&mut ctl, 0x40, &sig(l2cap::PARAMETER_UPDATE_REQUEST, 1, &[6, 0, 12, 0, 0, 0, 0xC8, 0]));
    assert_eq!(ok, Some(l2cap::Parameters { min: 6, max: 12, latency: 0, timeout: 200 }));
    // A timeout shorter than the interval allows: rejected.
    assert_eq!(l2cap::le_signal(&mut ctl, 0x40, &sig(l2cap::PARAMETER_UPDATE_REQUEST, 2, &[6, 0, 0x80, 0x0C, 0, 0, 10, 0])), None);
    let mut out = [0u8; 64];
    let n = ctl.next_acl(&mut out).unwrap();
    assert_eq!(&out[4..n], &[6, 0, 5, 0, l2cap::PARAMETER_UPDATE_RESPONSE, 1, 2, 0, 0, 0]);
    let n = ctl.next_acl(&mut out).unwrap();
    assert_eq!(&out[8..n], &[l2cap::PARAMETER_UPDATE_RESPONSE, 2, 2, 0, 1, 0]);
}

#[test]
fn the_setup_reads_the_buffers_and_asks_for_the_events() {
    let mut s = Sim::new("11:22:33:44:55:66 audio\n");
    let o = ops(&s.commands);
    for op in [hci::READ_LOCAL_FEATURES, hci::READ_BUFFER_SIZE, hci::LE_READ_BUFFER_SIZE, hci::SET_EVENT_MASK, hci::LE_SET_EVENT_MASK, hci::WRITE_SIMPLE_PAIRING_MODE,
               hci::WRITE_LE_HOST_SUPPORTED, hci::WRITE_CLASS_OF_DEVICE, hci::WRITE_LOCAL_NAME, hci::WRITE_SCAN_ENABLE] {
        assert!(o.contains(&op), "{:04X} not sent", op);
    }
    assert_eq!(s.stack.ctl.bredr, Pool { len: ACL_LEN, total: 8, free: 8 });
    assert_eq!(s.stack.ctl.le, Pool { len: 251, total: 4, free: 4 });
    let mask = &s.commands.iter().find(|c| c.0 == hci::SET_EVENT_MASK).unwrap().1;
    let mask = u64::from_le_bytes(mask[..8].try_into().unwrap());
    // IO Capability Request (48), User Confirmation Request (50), Simple Pairing Complete (53), LE meta (61).
    for bit in [48, 50, 53, 61] { assert!(mask >> bit & 1 == 1, "bit {}", bit); }
    assert!(s.stack.ready && s.logged("BR/EDR BUFFERS: 8 OF 310 BYTES"));
    // Without devices listed: no page scan, the radio stays quiet.
    let quiet = Sim::new("");
    assert!(!ops(&quiet.commands).contains(&hci::WRITE_SCAN_ENABLE));
}

#[test]
fn the_device_list_and_the_keys_file() {
    let mut d = bt_host::devices::Devices::new();
    let mut bad = Vec::new();
    d.load("# ours\n11:22:33:44:55:66 audio\nAA:BB:CC:DD:EE:FF le-hid random # a mouse\n01:02:03:04:05:06 toaster\n", |n, l| bad.push((n, l.to_string())));
    assert_eq!(bad, vec![(4, "01:02:03:04:05:06 toaster".to_string())]);
    let mouse = d.get(Addr::parse("AA:BB:CC:DD:EE:FF").unwrap()).unwrap();
    assert!(mouse.random && mouse.kind == bt_host::devices::Kind::LeHid);
    assert_eq!(PEER, Addr::parse("11:22:33:44:55:66").unwrap());
    let keys = "11:22:33:44:55:66 000102030405060708090A0B0C0D0E0F 04\nle AA:BB:CC:DD:EE:FF 101112131415161718191A1B1C1D1E1F 1234 2122232425262728 1\n99:99:99:99:99:99 000102030405060708090A0B0C0D0E0F 04\n";
    assert_eq!(d.load_keys(keys), 2);
    let mut out = [0u8; 512];
    let n = d.keys_text(&mut out);
    assert_eq!(std::str::from_utf8(&out[..n]).unwrap(), &keys[..keys.find("99:").unwrap()]);
}

#[test]
fn a_device_listed_is_paged_paired_and_secured() {
    let mut s = Sim::new("11:22:33:44:55:66 audio\n");
    let page = s.tick(10);
    assert_eq!(ops(&page), vec![hci::CREATE_CONNECTION]);
    assert_eq!(&page[0].1[..6], &PEER.0);
    assert_eq!(&page[0].1[6..], &[0x18, 0xCC, 1, 0, 0, 0, 1]);
    // Connected: authentication asked at once.
    assert_eq!(ops(&s.connected(0x0B)), vec![hci::AUTHENTICATION_REQUESTED]);
    let pairing = s.pair(0x0B);
    // No key: negative reply; NoInputNoOutput with general bonding; the number confirmed; then encryption.
    assert_eq!(ops(&pairing), vec![hci::LINK_KEY_NEGATIVE_REPLY, hci::IO_CAPABILITY_REPLY, hci::USER_CONFIRMATION_REPLY, hci::SET_CONNECTION_ENCRYPTION]);
    assert_eq!(&pairing[1].1[6..], &[3, 0, 4]);
    assert!(s.logged("CONFIRMED 123456") && s.logged("LINK KEY OF TYPE 4 KEPT") && !s.log.iter().any(|l| l.contains("A0A1A2")));
    assert_eq!(s.ups(), vec!["keys"]);
    assert_eq!(s.stack.devices.get(PEER).unwrap().key.map(|k| k.0[0]), Some(0xA0));
    // Encrypted: the profile's channel requested (AVDTP).
    s.encrypted(0x0B);
    let f = s.frames();
    assert_eq!(f.len(), 1);
    assert_eq!((f[0].0, f[0].1, f[0].2[0], le16(&f[0].2, 4)), (0x0B, l2cap::SIGNALLING, l2cap::CONNECTION_REQUEST, l2cap::PSM_AVDTP));
    // Disconnected: tried again later.
    s.event(hci::EV_DISCONNECTION_COMPLETE, &[0, 0x0B, 0, 0x13]);
    assert!(s.logged("DISCONNECTED (REASON 13)"));
    assert!(s.tick(1_000).is_empty());
    assert_eq!(ops(&s.tick(5_000)), vec![hci::CREATE_CONNECTION]);
    // With the key: Link Key Request answered with it; a device that lost its key makes us pair again once.
    s.connected(0x0C);
    assert_eq!(ops(&s.event(hci::EV_LINK_KEY_REQUEST, &PEER.0)), vec![hci::LINK_KEY_REPLY]);
    assert_eq!(ops(&s.event(hci::EV_AUTHENTICATION_COMPLETE, &[0x06, 0x0C, 0])), vec![hci::AUTHENTICATION_REQUESTED]);
    assert!(s.stack.devices.get(PEER).unwrap().key.is_none() && s.logged("FORGOTTEN, PAIRING AGAIN"));
    assert_eq!(ops(&s.event(hci::EV_AUTHENTICATION_COMPLETE, &[0x05, 0x0C, 0])), vec![hci::DISCONNECT]);
}

#[test]
fn a_page_that_fails_is_tried_again_later() {
    let mut s = Sim::new("11:22:33:44:55:66 hid\n");
    s.tick(10);
    let mut p = vec![0x04, 0, 0];
    p.extend_from_slice(&PEER.0);
    p.extend_from_slice(&[1, 0]);
    s.event(hci::EV_CONNECTION_COMPLETE, &p);
    assert!(s.logged("NOT CONNECTED (STATUS 04), TRIED AGAIN IN 15 S"));
    assert!(s.tick(10_000).is_empty());
    assert_eq!(ops(&s.tick(5_100)), vec![hci::CREATE_CONNECTION]);
    // A HID device: DisplayOnly and MITM, so a keyboard types a passkey.
    s.connected(0x0D);
    let r = s.event(hci::EV_IO_CAPABILITY_REQUEST, &PEER.0);
    assert_eq!(&r[0].1[6..], &[0, 0, 5]);
    s.event(hci::EV_USER_PASSKEY_NOTIFICATION, &[PEER.0[0], PEER.0[1], PEER.0[2], PEER.0[3], PEER.0[4], PEER.0[5], 0x39, 0x30, 0, 0]);
    assert!(s.logged("TYPE 012345 ON IT, THEN ENTER"));
}

#[test]
fn only_devices_listed_may_connect_or_pair() {
    let mut s = Sim::new("11:22:33:44:55:66 audio\n");
    let stranger = [1, 2, 3, 4, 5, 6];
    let r = s.event(hci::EV_CONNECTION_REQUEST, &[1, 2, 3, 4, 5, 6, 0x04, 0x04, 0x24, 1]);
    assert_eq!((r[0].0, r[0].1.clone()), (hci::REJECT_CONNECTION, vec![1, 2, 3, 4, 5, 6, 0x0F]));
    assert_eq!(s.event(hci::EV_IO_CAPABILITY_REQUEST, &stranger)[0].0, hci::IO_CAPABILITY_NEGATIVE_REPLY);
    assert_eq!(s.event(hci::EV_PIN_CODE_REQUEST, &stranger)[0].0, hci::PIN_CODE_NEGATIVE_REPLY);
    // Ours: accepted, staying peripheral; a legacy device gets PIN 0000.
    let mut req = PEER.0.to_vec();
    req.extend_from_slice(&[0x04, 0x04, 0x24, 1]);
    let r = s.event(hci::EV_CONNECTION_REQUEST, &req);
    assert_eq!((r[0].0, r[0].1[6]), (hci::ACCEPT_CONNECTION, 1));
    let r = s.event(hci::EV_PIN_CODE_REQUEST, &PEER.0);
    assert_eq!((r[0].0, &r[0].1[6..11]), (hci::PIN_CODE_REPLY, &[4, b'0', b'0', b'0', b'0'][..]));
    // It connected to us and asks for AVDTP before securing the link: pending, we authenticate, then it is admitted.
    s.connected(0x0E);
    s.send(0x0E, l2cap::SIGNALLING, &sig(l2cap::CONNECTION_REQUEST, 1, &[0x19, 0, 0x50, 0]), 64);
    let cmds = s.run();
    assert_eq!(ops(&cmds), vec![hci::AUTHENTICATION_REQUESTED]);
    let f = s.frames();
    assert_eq!(&f[0].2[4 + 4..4 + 8], &[1, 0, 1, 0]);
    s.pair(0x0E);
    s.encrypted(0x0E);
    let f = s.frames();
    assert_eq!((f[0].2[0], &f[0].2[4 + 2..4 + 6]), (l2cap::CONNECTION_RESPONSE, &[0x50, 0, 0, 0][..]));
    assert_eq!(f[1].2[0], l2cap::CONFIGURATION_REQUEST);
    // The device opens the profile itself: we open no AVDTP channel of our own.
    assert_eq!(f.len(), 2);
}
