//! rtw88_wifi's parts that need no chip (550-DRV-0042): the eFuse's decoding, the firmware's header, the TX and RX
//! descriptors, the firmware's TX report, the receive filter, the power sequences, the TX power indices, the rings.
//! Evidence for these encodings only: what the chip does with them is the DEXP's to show (MC-12.1).
#[path = "../rtw88_wifi/src/coex.rs"] mod coex;
#[path = "../rtw88_wifi/src/cond.rs"] mod cond;
#[path = "../rtw88_wifi/src/dma.rs"] mod dma;
#[path = "../rtw88_wifi/src/efuse.rs"] mod efuse;
#[path = "../rtw88_wifi/src/fw.rs"] mod fw;
#[path = "../rtw88_wifi/src/h2c.rs"] #[allow(dead_code)] mod h2c;
#[path = "../rtw88_wifi/src/io.rs"] mod io;
#[path = "../rtw88_wifi/src/load.rs"] mod load;
#[path = "../rtw88_wifi/src/mac.rs"] #[allow(dead_code)] mod mac;
#[path = "../rtw88_wifi/src/phy.rs"] #[allow(dead_code)] mod phy;
#[path = "../rtw88_wifi/src/power.rs"] mod power;
#[path = "../rtw88_wifi/src/radio.rs"] #[allow(dead_code)] mod radio;
#[path = "../rtw88_wifi/src/tables.rs"] #[allow(dead_code)] mod tables;
#[path = "../rtw88_wifi/src/txrx.rs"] mod txrx;

use std::collections::HashMap;

/// Registers as bytes; every write recorded. `fixed` registers read as given whatever was written.
#[derive(Default)]
struct Fake { mem: HashMap<u32, u8>, fixed: HashMap<u32, u8>, writes: Vec<(u32, u32, u8)>, waited: u64 }

impl Fake {
    fn get(&self, at: u32) -> u8 { self.fixed.get(&at).or(self.mem.get(&at)).copied().unwrap_or(0) }
    fn put(&mut self, at: u32, v: u32, n: u8) {
        self.writes.push((at, v, n));
        for i in 0..n as u32 { self.mem.insert(at + i, (v >> (8 * i)) as u8); }
    }
}

impl io::Regs for Fake {
    fn r8(&mut self, at: u32) -> u8 { self.get(at) }
    fn r16(&mut self, at: u32) -> u16 { u16::from_le_bytes([self.get(at), self.get(at + 1)]) }
    fn r32(&mut self, at: u32) -> u32 { u32::from_le_bytes([self.get(at), self.get(at + 1), self.get(at + 2), self.get(at + 3)]) }
    fn w8(&mut self, at: u32, v: u8) { self.put(at, v as u32, 1) }
    fn w16(&mut self, at: u32, v: u16) { self.put(at, v as u32, 2) }
    fn w32(&mut self, at: u32, v: u32) { self.put(at, v, 4) }
    fn udelay(&mut self, us: u32) { self.waited += us as u64; }
    fn msleep(&mut self, ms: u32) { self.waited += ms as u64 * 1000; }
}

fn word(d: &[u8], i: usize) -> u32 { u32::from_le_bytes(d[i * 4..i * 4 + 4].try_into().unwrap()) }

#[test]
fn the_efuse_decodes_into_the_boards_fields() {
    let mut phy = [0xffu8; efuse::PHYSICAL];
    let mut at = 0;
    let mut put = |bytes: &[u8]| { phy[at..at + bytes.len()].copy_from_slice(bytes); at += bytes.len(); };
    // Block 1 (0x08..0x0f) by a one-byte header, word 0 only: 0x08, 0x09.
    put(&[0x1e, 0xaa, 0xbb]);
    // Block 25 (0xc8..0xcf), word 1 (0xca, 0xcb) by a two-byte header: RFE 4 with package bit 5, then the country's 'U'.
    put(&[0x2f, 0x3d, 0x24, b'U']);
    // Block 26 (0xd0..0xd7), words 0-2: the MAC address and two more bytes.
    put(&[0x4f, 0x38, 0x00, 0xe0, 0x4c, 0x12, 0x34, 0x56, 0xc8, 0x21]);
    let mut log = [0u8; efuse::LOGICAL];
    efuse::decode(&phy, &mut log).unwrap();
    assert_eq!((log[0x08], log[0x09], log[0x0a]), (0xaa, 0xbb, 0xff));
    assert_eq!((log[0xca], log[0xcb]), (0x24, b'U'));
    let b = efuse::Board::from(&log);
    assert_eq!((b.rfe, b.pkg), (4, 1));
    assert_eq!(b.mac, [0x00, 0xe0, 0x4c, 0x12, 0x34, 0x56]);
    assert!(b.mac_valid());
    assert_eq!(b.crystal, 0, "a blank crystal byte reads as 0, as Linux leaves it");
    // RFE 4 takes 2.4 GHz's power from path B's block (0x3a), blank here.
    assert_eq!(b.power.cck, [0xff; 6]);
    // A record running past the parsed region (416 bytes) is refused: 46 whole records of 9 bytes, then one whose
    // second word would end past it.
    let mut bad = [0u8; efuse::PHYSICAL];
    bad[414] = 0x10;
    bad[416..].fill(0xff);
    assert!(efuse::decode(&bad, &mut log).is_err());
}

#[test]
fn the_power_differences_are_signed_nibbles() {
    let mut log = [0xffu8; efuse::LOGICAL];
    log[0xca] = 0;
    log[0x10..0x16].copy_from_slice(&[0x2a, 0x2a, 0x2b, 0x2b, 0x2c, 0x2d]);
    log[0x16..0x1b].copy_from_slice(&[0x28, 0x29, 0x2a, 0x2b, 0x2c]);
    log[0x1b] = 0xf2; // OFDM +2, BW20 -1
    let b = efuse::Board::from(&log);
    assert_eq!((b.power.ofdm_diff, b.power.bw20_diff), (2, -1));
    // Channel 1: group 0, CCK 0x2a + min(by rate, the world-wide −4).
    let (cck, ofdm) = phy::indices(1, &b);
    assert_eq!(cck, [0x26; 4]);
    // OFDM 0x28 + 2: 6-18 Mb/s min(+8, +2), 24 Mb/s +2, 36 Mb/s +2, 48 Mb/s +2, 54 Mb/s 0.
    assert_eq!(ofdm, [0x2c, 0x2c, 0x2c, 0x2c, 0x2c, 0x2c, 0x2c, 0x2a]);
    // Channel 13: CCK at most −16, OFDM at most −26, never below 0.
    let (cck, ofdm) = phy::indices(13, &b);
    assert_eq!(cck[0], 0x2c - 16, "channel 13 is in CCK's group 4");
    assert_eq!(ofdm[7], 0x2c + 2 - 26);
    // A blank eFuse's base is modest, not the maximum.
    let blank = efuse::Board::from(&[0xffu8; efuse::LOGICAL]);
    assert_eq!(phy::indices(6, &blank).0, [0x20 - 4; 4]);
}

#[test]
fn the_firmwares_header_is_read_and_its_sizes_checked() {
    let mut f = vec![0u8; 64 + 100 + 8 + 200 + 8];
    f[0..2].copy_from_slice(&0x8821u16.to_le_bytes());
    f[4..6].copy_from_slice(&24u16.to_le_bytes());
    f[6] = 11;
    f[0x20..0x24].copy_from_slice(&0x8020_0000u32.to_le_bytes());
    f[0x24..0x28].copy_from_slice(&100u32.to_le_bytes());
    f[0x30..0x34].copy_from_slice(&200u32.to_le_bytes());
    f[0x3c..0x40].copy_from_slice(&0x0000_0000u32.to_le_bytes());
    let image = fw::parse(&f).unwrap();
    assert_eq!((image.signature, image.version, image.sub), (0x8821, 24, 11));
    assert_eq!(image.dmem, fw::Part { addr: 0x0020_0000, at: 64, len: 108 });
    assert_eq!(image.imem, fw::Part { addr: 0, at: 172, len: 208 });
    assert!(image.emem.is_none());
    f.push(0);
    assert!(fw::parse(&f).is_err());
    // The real file, when fetched into proprietary/ (scripts/firmware/rtl8821ce.py).
    if let Ok(real) = std::fs::read("proprietary/firmware/rtw88/rtw8821c_fw.bin") {
        let image = fw::parse(&real).unwrap();
        assert_eq!((image.signature, image.version, image.sub), (0x8821, 24, 11));
        println!("rtw8821c_fw.bin: {:?}", image);
    }
}

#[test]
fn a_frames_tx_descriptor_asks_for_its_rate_its_tries_and_a_report() {
    let mut d = [0xffu8; 48];
    txrx::tx_desc(&mut d, &txrx::Frame { len: 60, qsel: txrx::QSEL_MGMT, rate: txrx::desc_rate(2).unwrap(), tries: 7, group: true, seq: 0x123, cookie: 5 });
    assert_eq!(word(&d, 0), 60 | 48 << 16 | 1 << 24 | 1 << 26);
    assert_eq!(word(&d, 1), 0x12 << 8 | 8 << 16);
    assert_eq!(word(&d, 2), 1 << 19);
    assert_eq!(word(&d, 3), 1 << 8 | 1 << 10);
    assert_eq!(word(&d, 4), 0 | 1 << 17 | 7 << 18);
    assert_eq!(word(&d, 6), 5 << 2);
    assert_eq!(word(&d, 9), 0x123 << 12);
    assert_eq!((word(&d, 5), word(&d, 7), word(&d, 8), word(&d, 10), word(&d, 11)), (0, 0, 0, 0, 0));
    txrx::tx_desc(&mut d, &txrx::Frame { len: 1500, qsel: txrx::QSEL_BE, rate: txrx::desc_rate(108).unwrap(), tries: 15, group: false, seq: 0, cookie: 63 });
    assert_eq!(word(&d, 1), 6 << 16);
    assert_eq!(word(&d, 4) & 0x7f, 11);
    assert_eq!(txrx::desc_rate(13), None);
    assert_eq!((0..12).map(txrx::rate_500k).collect::<Vec<_>>(), vec![2, 4, 11, 22, 12, 18, 24, 36, 48, 72, 96, 108]);
}

#[test]
fn a_received_buffer_and_the_firmwares_reports_are_read() {
    let mut b = [0u8; 64];
    // 120 bytes with FCS, 32 bytes of driver info, shift 2, PHY status, rate 54 Mb/s.
    b[0..4].copy_from_slice(&(120u32 | 4 << 16 | 2 << 24 | 1 << 26).to_le_bytes());
    b[12..16].copy_from_slice(&11u32.to_le_bytes());
    let rx = txrx::Rx::parse(&b).unwrap();
    assert_eq!((rx.len, rx.drv_info, rx.shift, rx.physt, rx.c2h, rx.crc, rx.rate), (120, 32, 2, true, false, false, 11));
    assert_eq!(rx.start(), 24 + 32 + 2);
    b[8..12].copy_from_slice(&(1u32 << 28).to_le_bytes());
    assert!(txrx::Rx::parse(&b).unwrap().c2h);
    // OFDM PHY status: PWDB 60 is −50 dBm; CCK with LNA index 2 and VGA 3 on an RFE-0 board: −6 − 6.
    let mut s = [0u8; 32];
    s[0] = 1;
    s[1] = 60;
    assert_eq!(txrx::signal(&s, false), Some(-50));
    s = [0; 32];
    s[12..16].copy_from_slice(&(3u32 << 8 | 2 << 13).to_le_bytes());
    assert_eq!(txrx::signal(&s, true), Some(-12));
    // The 11ac report: cookie in payload[8] bits 7-2, status in payload[9] bits 7-6, retries in payload[10].
    let mut m = [0u8; 14];
    m[0] = 0xff;
    m[2] = 0x0f;
    m[2 + 8] = 9 << 2;
    m[2 + 10] = 2;
    assert_eq!(txrx::report(&m), Some(txrx::Report { cookie: 9, acked: true, tries: Some(3) }));
    m[2 + 9] = 0x40;
    assert!(!txrx::report(&m).unwrap().acked);
    // The older report (id 0x03).
    let mut o = [0u8; 10];
    o[0] = 3;
    o[2 + 6] = 4 << 2;
    assert_eq!(txrx::report(&o), Some(txrx::Report { cookie: 4, acked: true, tries: Some(1) }));
    assert_eq!(txrx::report(&[0x09, 0, 1, 2]), None);
}

#[test]
fn the_filter_passes_what_the_station_asked_for() {
    let own = [2, 0, 0, 0, 0, 1];
    let bss = [0x10, 0x20, 0x30, 0x40, 0x50, 0x60];
    let mut beacon = [0u8; 36];
    beacon[0] = 0x80;
    beacon[4..10].copy_from_slice(&[0xff; 6]);
    beacon[16..22].copy_from_slice(&[9; 6]);
    assert!(txrx::wanted(&beacon, true, &own, &bss));
    assert!(!txrx::wanted(&beacon, false, &own, &bss), "another network's beacon outside a scan");
    beacon[16..22].copy_from_slice(&bss);
    assert!(txrx::wanted(&beacon, false, &own, &bss), "its own network's beacon");
    let mut data = [0u8; 40];
    data[0] = 0x08;
    data[1] = 0x02; // from the DS
    data[4..10].copy_from_slice(&own);
    assert!(txrx::wanted(&data, false, &own, &[0; 6]));
    let mut ack = [0u8; 10];
    ack[0] = 0xd4;
    ack[4..10].copy_from_slice(&own);
    assert!(!txrx::wanted(&ack, false, &own, &bss), "no control frames");
}

#[test]
fn the_power_sequence_writes_its_fields_and_waits_for_its_bits() {
    let mut r = Fake::default();
    // The polled bits as the chip sets them: 0x06 bit 1 up, 0x05 bit 0 down.
    r.fixed.insert(0x06, 0x02);
    r.mem.insert(0x05, 0xff);
    r.fixed.insert(0x05, 0x00);
    power::run(&mut r, &power::ENABLE).unwrap();
    assert_eq!(r.writes.first(), Some(&(0x05, 0x00, 1)));
    assert!(r.writes.contains(&(0x0062, 0xe0, 1)));
    assert_eq!(r.waited, 0);
    // A bit that never comes: 20000 reads, the PCIe toggle of 0x04 bit 3, 20000 more, then the failure.
    let mut r = Fake::default();
    let e = power::run(&mut r, &[power::Step { at: 0x06, mask: 2, value: 2, poll: true }]).unwrap_err();
    assert!(matches!(e, io::Error::Timeout { at: 0x06, .. }));
    assert_eq!(r.waited, 2 * 20_000 * 50);
    assert_eq!(r.writes, vec![(0x04, 0x08, 1), (0x04, 0x00, 1)]);
}

#[test]
fn a_frame_posted_on_a_ring_gets_its_two_buffer_descriptors() {
    let layout = std::alloc::Layout::from_size_align(dma::END, 4096).unwrap();
    let base = unsafe { std::alloc::alloc_zeroed(layout) };
    let mut region = unsafe { dma::Region::new(base, 0x1000_0000, dma::END) }.unwrap();
    let mut r = Fake::default();
    let (mut mgmt, mut be, mut h2c) = (dma::MGMT, dma::BE, dma::H2C);
    dma::program(&mut r, &mut region, &mut [&mut mgmt, &mut be, &mut h2c]);
    assert!(r.writes.contains(&(0x310, 0x1000_0100, 4)), "the management ring's base");
    assert!(r.writes.contains(&(0x380, 16, 2)));
    assert!(r.writes.contains(&(0x382, dma::RX_SLOTS as u32, 2)));
    assert!(mgmt.room(&mut r));
    mgmt.slot(&mut region)[48..48 + 100].fill(7);
    mgmt.post(&mut r, &mut region, 100);
    let bd = region.bytes(0x100, 16).to_vec();
    assert_eq!(u16::from_le_bytes([bd[0], bd[1]]), 48);
    assert_eq!(u16::from_le_bytes([bd[2], bd[3]]), 2, "(48 + 100) / 128 pages, rounded up");
    let dma0 = u32::from_le_bytes(bd[4..8].try_into().unwrap());
    assert_eq!(u16::from_le_bytes([bd[8], bd[9]]), 100);
    assert_eq!(u32::from_le_bytes(bd[12..16].try_into().unwrap()), dma0 + 48);
    assert_eq!(r.writes.last(), Some(&(0x3b0, 1, 2)), "the host's write index");
    // The beacon ring (a firmware chunk): both segments, the owner bit, and the ring kicked.
    dma::post_beacon(&mut r, &mut region, 48 + 4096);
    let bd = region.bytes(0, 16).to_vec();
    assert_eq!(u16::from_le_bytes([bd[0], bd[1]]), 48);
    assert_eq!(u16::from_le_bytes([bd[2], bd[3]]), 33 | 1 << 15, "(48 + 4096) / 128 pages, rounded up, and the owner bit");
    assert_eq!(u32::from_le_bytes(bd[4..8].try_into().unwrap()), 0x1000_1000);
    assert_eq!(u16::from_le_bytes([bd[8], bd[9]]), 4096);
    assert_eq!(u32::from_le_bytes(bd[12..16].try_into().unwrap()), 0x1000_1000 + 48);
    assert_eq!(r.writes.last(), Some(&(0x383, 1 << 4, 1)), "the beacon queue's flag");
    // The RX ring: the chip's write index at 3 means three buffers to take.
    let mut rx = dma::Rx { next: 0 };
    r.fixed.insert(0x3b6, 3);
    assert_eq!(rx.ready(&mut r), 3);
    rx.done(&mut r, &mut region);
    assert_eq!(rx.ready(&mut r), 2);
    unsafe { std::alloc::dealloc(base, layout) };
}
