//! usb_host's hub facts (211-DRV-0035): USB 2 and SuperSpeed hub descriptors, the speed after a port reset, the
//! change bits to clear. QEMU has no SuperSpeed hub, so these and the MacBook Pro's dock are the evidence.
#[path = "../usb_host/src/hub.rs"]
#[allow(dead_code)]
mod hub;

#[test]
fn descriptors() {
    // A Genesys Logic USB 3 hub's SuperSpeed side (05E3:0626 kind): 4 ports, 100 ms to power.
    let ss = [12, 0x2A, 4, 0x09, 0x00, 50, 0, 0x04, 0x00, 0x00, 0, 0];
    assert_eq!(hub::superspeed(&ss), Some(hub::Hub { ports: 4, think: 0, power_ms: 100 }));
    assert_eq!(hub::usb2(&ss), None);
    // Its USB 2 side: 4 ports, TT think time 8 FS bit times (code 0), 50 ms.
    let hs = [9, 0x29, 4, 0xE0, 0x00, 25, 100, 0, 0xFF];
    assert_eq!(hub::usb2(&hs), Some(hub::Hub { ports: 4, think: 3, power_ms: 50 }));
    assert_eq!(hub::superspeed(&hs), None);
    assert_eq!(hub::superspeed(&ss[..11]), None);
    // More ports than a route string's digit holds are cut to 15.
    let wide = [12, 0x2A, 20, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    assert_eq!(hub::superspeed(&wide).unwrap().ports, 15);
}

#[test]
fn speeds() {
    // A SuperSpeed hub's port: powered (bit 9), enabled, in U0: SuperSpeed, not low speed.
    assert_eq!(hub::speed_after_reset(true, 0x0203), 4);
    assert_eq!(hub::speed_after_reset(false, 0x0203), 2);
    assert_eq!(hub::speed_after_reset(false, 0x0503), 3);
    assert_eq!(hub::speed_after_reset(false, 0x0103), 1);
}

#[test]
fn change_bits() {
    let ss: Vec<u16> = hub::other_changes(true, 0b1111_1001).collect();
    assert_eq!(ss, vec![19, 29, 25, 26]);
    let usb2: Vec<u16> = hub::other_changes(false, 0b0000_1111).collect();
    assert_eq!(usb2, vec![17, 18, 19]);
    assert_eq!(hub::other_changes(true, hub::CHANGED_CONNECTION | hub::CHANGED_RESET).count(), 0);
}
