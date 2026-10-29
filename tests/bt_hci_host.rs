//! bt_hci's parts that need no controller (700-BLT-0001): HCI commands, events joined from USB packets, the devices
//! heard, and a Realtek patch made from its files. With `proprietary/firmware/rtl_bt/` fetched, the real RTL8821C
//! files too. Evidence for these encodings only: what the controller does with them is the DEXP's to show (MC-12.1).
#[path = "../bt_hci/src/hci.rs"] #[allow(dead_code)] mod hci;
#[path = "../bt_hci/src/rtl.rs"] #[allow(dead_code)] mod rtl;
use std::path::Path;

#[test]
fn a_command_packet() {
    let mut out = [0u8; hci::COMMAND_MAX];
    let n = hci::command(hci::LE_SET_SCAN_ENABLE, &[1, 1], &mut out);
    assert_eq!(&out[..n], &[0x0C, 0x20, 2, 1, 1]);
    assert_eq!(hci::command(hci::RESET, &[], &mut out), 3);
}

#[test]
fn events_are_joined_from_interrupt_packets() {
    let mut ev = hci::Events::new();
    let mut got = Vec::new();
    // Read Local Version's Command Complete, 14 bytes, then an event in two 16-byte packets and a short one.
    ev.feed(&[0x0E, 12, 1, 0x01, 0x10, 0, 0x0A, 0x0C, 0x00, 0x0A, 0x5D, 0x00, 0x21, 0x88], |c, p| got.push((c, p.to_vec())));
    let long: Vec<u8> = [0x3E, 30].into_iter().chain(0..30).collect();
    for packet in long.chunks(16) { ev.feed(packet, |c, p| got.push((c, p.to_vec()))); }
    assert_eq!(got.len(), 2);
    let (op, ret) = hci::complete(&got[0].1).unwrap();
    assert_eq!(op, hci::READ_LOCAL_VERSION);
    assert_eq!(hci::Version::parse(ret), Some(hci::Version { hci: 0x0A, hci_rev: 0x000C, lmp: 0x0A, manufacturer: 0x5D, lmp_subver: 0x8821 }));
    assert_eq!((got[1].0, got[1].1.len()), (0x3E, 30));
    // Bytes past an event's end in the same packet are dropped, not taken as another event's start.
    ev.feed(&[0x0F, 4, 0, 1, 0x03, 0x0C, 0xEE], |c, p| got.push((c, p.to_vec())));
    assert_eq!((ev.dropped, hci::status(&got[2].1)), (1, Some((0, hci::RESET))));
}

#[test]
fn the_address_and_the_devices_heard() {
    assert_eq!(hci::bd_addr(&[0, 0xA5, 0x6B, 0xF0, 0x2D, 0x79, 0x1C]), Some([0x1C, 0x79, 0x2D, 0xF0, 0x6B, 0xA5]));
    // One advertising report: connectable, public address, flags then a complete name, RSSI -60.
    let mut report = vec![1, 0x00, 0x00, 6, 5, 4, 3, 2, 1, 11, 2, 0x01, 0x06, 7, 0x09];
    report.extend_from_slice(b"Mouse1");
    report.push((-60i8) as u8);
    let mut heard = Vec::new();
    hci::le_reports(&report, |h| heard.push((h.addr, h.rssi, h.name.map(|n| n.to_vec()))));
    assert_eq!(heard, vec![([1, 2, 3, 4, 5, 6], -60, Some(b"Mouse1".to_vec()))]);
    // An extended inquiry result: a keyboard (peripheral, 0x002540), RSSI -70, a shortened name.
    let mut eir = vec![1, 6, 5, 4, 3, 2, 1, 1, 0, 0x40, 0x25, 0x00, 0, 0, (-70i8) as u8, 3, 0x08, b'K', b'B'];
    eir.resize(15 + 240, 0);
    let h = hci::extended_inquiry(&eir).unwrap();
    assert_eq!((h.addr, h.rssi, h.name, h.kind, hci::major_class(h.kind)), ([1, 2, 3, 4, 5, 6], -70, Some(&b"KB"[..]), 0x002540, "PERIPHERAL"));
    // Malformed data ends the walk without a name.
    assert_eq!(hci::ad_fields(&[9, 0x09, b'x']), (None, None));
}

/// A firmware file in the format: two patches for chip IDs 1 and 2, the extension naming `project`.
fn epatch(project: u8) -> Vec<u8> {
    let mut f = b"Realtech".to_vec();
    f.extend_from_slice(&0x75B8_F098u32.to_le_bytes());
    f.extend_from_slice(&2u16.to_le_bytes());
    for id in [1u16, 2] { f.extend_from_slice(&id.to_le_bytes()); }
    for len in [8u16, 600] { f.extend_from_slice(&len.to_le_bytes()); }
    for at in [0x30u32, 0x40] { f.extend_from_slice(&at.to_le_bytes()); }
    f.resize(0x30, 0);
    f.extend_from_slice(&[0xA1; 8]);
    f.resize(0x40, 0);
    f.extend(std::iter::repeat(0xB2).take(600));
    f.extend_from_slice(&[0xFF, project, 1, 0, 0x51, 0x04, 0xFD, 0x77]);
    f
}

#[test]
fn a_patch_is_chosen_by_the_rom_version_and_ends_with_the_files_version() {
    let config = [0x55, 0xAB, 0x23, 0x87, 4, 0, 0xD9, 0, 1, 0x0F];
    let fw = epatch(10);
    assert_eq!(rtl::parse(&fw), Ok(rtl::Epatch { version: 0x75B8_F098, patches: 2, project: 10 }));
    let mut out = vec![0u8; 4096];
    let n = rtl::image(&fw, &config, &rtl::RTL8821C, 1, &mut out).unwrap();
    assert_eq!(n, 600 + 10);
    assert!(out[..596].iter().all(|&b| b == 0xB2));
    assert_eq!(&out[596..600], &0x75B8_F098u32.to_le_bytes());
    assert_eq!(&out[600..610], &config);
    assert_eq!(rtl::image(&fw, &config, &rtl::RTL8821C, 5, &mut out), Err(rtl::Error::NoPatch(5)));
    assert_eq!(rtl::image(&epatch(8), &config, &rtl::RTL8821C, 1, &mut out), Err(rtl::Error::Project(8)));
    assert_eq!(rtl::image(&fw, &config[..5], &rtl::RTL8821C, 1, &mut out), Err(rtl::Error::Config));
    assert_eq!(rtl::parse(b"Realtekk\0\0\0\0\0\0"), Err(rtl::Error::Signature));
}

#[test]
fn fragments_of_252_bytes_with_their_index_bytes() {
    // 610 bytes: two whole fragments and the rest, the last marked.
    assert_eq!(rtl::fragments(610), 3);
    assert_eq!(rtl::fragment(610, 0), (0x00, 0..252));
    assert_eq!(rtl::fragment(610, 1), (0x01, 252..504));
    assert_eq!(rtl::fragment(610, 2), (0x82, 504..610));
    // Past 0x7F the index starts again from 1.
    let len = 200 * 252 + 7;
    assert_eq!(rtl::fragment(len, 0x7F).0, 0x7F);
    assert_eq!(rtl::fragment(len, 0x80).0, 0x01);
    assert_eq!(rtl::fragment(len, 200), (0x80 | 0x49, 200 * 252..len));
}

#[test]
fn the_rtl8821c_files_when_fetched() {
    // scripts/host_tests.sh runs from the repository's root.
    let dir = Path::new("proprietary/firmware/rtl_bt");
    let (Ok(fw), Ok(config)) = (std::fs::read(dir.join("rtl8821c_fw.bin")), std::fs::read(dir.join("rtl8821c_config.bin"))) else {
        println!("SKIP: proprietary/firmware/rtl_bt/ not fetched");
        return;
    };
    let e = rtl::parse(&fw).unwrap();
    assert_eq!((e.version, e.patches, e.project), (0x75B8_F098, 2, 10));
    let mut out = vec![0u8; 64 * 1024];
    let n1 = rtl::image(&fw, &config, &rtl::RTL8821C, 0, &mut out).unwrap();
    let n2 = rtl::image(&fw, &config, &rtl::RTL8821C, 1, &mut out).unwrap();
    assert_eq!((n1, n2), (0x4C14 + 10, 0x8864 + 10));
    assert!(rtl::fragments(n2) < 0xFF);
}
