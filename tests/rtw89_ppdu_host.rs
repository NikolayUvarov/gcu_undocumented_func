// 213-DRV-0087: the signal of a received PPDU from its status report, against reports captured on the Ryzen.
#![allow(dead_code)]
#[path = "../rtw89_wifi/src/ppdu.rs"] mod ppdu;
use ppdu::{parse, weakness, Report};

// A whole RX buffer as the bench print showed it (2026-11-21 16:29-16:33 UTC): the PCIe word, the descriptor, the report.
fn split(hex: &str) -> ([u32; 3], Vec<u8>) {
    let b: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
    let w = |at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let (w0, w1, w3) = (w(4), w(8), w(16));
    let start = 4 + if w0 >> 31 != 0 { 32 } else { 16 };
    (([w0, w1, w3]), b[start..start + (w0 & 0x3fff) as usize].to_vec())
}
// CoolNet2's 5 GHz beacon on channel 100 (A0:36:BC:11:F8:8C, OFDM 6 Mb/s): an OFDM_BRK report, paths A 91 and B 95.
const FIVE: &str = "54c0cb0440008001120004000000000001002000010060111f0001001800000000000000020000000b2c00008305005d5b5f00000158303000bacffb18c05e080000000000000000007fc05f0000000000000000";
// Its 2.4 GHz beacon on channel 8 heard from 7 (CCK 1 Mb/s): a CCK report, RPL 90 above the average 88.
const CCK: &str = "6cc0b400580080014000000000000000010020000100001000000200180000000000000000000000a0f30a04b00bacc7000000008b07005858580000805a210000df6ffe5a5db00b20410030011e0730000d800400c01f0800000000000000000017c05f0000000000000000";
// The same beacon earlier, an OFDM_BRK report at CCK: paths 92 and 89.
const BREAK: &str = "5cc06100480080013000000000000000010020000100001000000200180000000000000000000000a0f30a04b00bacc7000000008305005a5c5900000100073000fb144e00c00b080000000000000000001bc05f0000000000000000";
// A data frame from the access point to this radio (OFDM, rate 9): a legacy OFDM report with the frequency-domain RPL 97.
const DATA: &str = "54c0c71240008001320009000000000001002000010066111f000100030000000000000002000000cd2c00008c05006a686c00000161083000e0bffd16c05d0872feffff0000000000ff284c0000000000000000";
// Another station's CCK frame: RPL 69 above the average 63.
const OTHER: &str = "6cc0c812580080010000000000000000000020000100000000000200180000000000000000000000a0f30a04e006becf000000008b07003f413c000080459f020031f0035445e006206300300124083000ea8eec0000000872feffff00000000007fc05f0000000000000000";

fn signal(hex: &str) -> Option<Report> { let (d, p) = split(hex); parse(d, &p) }

#[test]
fn reports_captured_on_the_ryzen_give_linux_s_signal() {
    assert_eq!(signal(FIVE), Some(Report { count: 1, rate: 4, signal_dbm: -63 }), "5 GHz: no RPL conversion for a break");
    assert_eq!(signal(CCK), Some(Report { count: 4, rate: 0, signal_dbm: -65 }), "2.4 GHz CCK: paths 88 moved by +2");
    assert_eq!(signal(BREAK).unwrap().signal_dbm, -64);
    assert_eq!(signal(DATA).unwrap().signal_dbm, -61, "paths 104/108 moved by RPL 97 less the average 106");
    assert_eq!(signal(OTHER).unwrap().signal_dbm, -75);
    // Linux on the same spot: its beacons' average at -63 dBm on 2.4 GHz and -67 dBm on 5 GHz (other hours).
    assert_eq!((weakness(-63), weakness(-110), weakness(0), weakness(17)), (63, 110, 1, 1));
}

#[test]
fn untrusted_report_bytes_are_bounded_and_refused_whole() {
    for hex in [FIVE, CCK, BREAK, DATA, OTHER] {
        let (d, p) = split(hex);
        for cut in 0..p.len() { assert_eq!(parse(d, &p[..cut]), None, "a report cut at {cut} disagrees with its length"); }
        let mut longer = p.clone(); longer.extend_from_slice(&[0; 8]); assert_eq!(parse(d, &longer), None);
        assert_eq!(parse([d[0] & !(15 << 24), d[1], d[2]], &p), None, "a Wi-Fi descriptor is no report");
        // The PHY status follows the MAC info, its one user padded to eight bytes and the PLCP (no RX counters here).
        let users = usize::from(p[0] & 15); let at = 8 + users * 4 + (users & 1) * 4 + usize::from(p[6]) * 8;
        let mut invalid = p.clone(); invalid[at] &= !0x80;
        assert_eq!(parse(d, &invalid), None, "the PHY status not valid");
        let mut users = p.clone(); users[0] = (users[0] & 0xf0) | 5; assert_eq!(parse(d, &users), None, "five users");
    }
    // An IE of no length (type 23) ends the IE walk unparsed: the header's paths stand, as Linux's failed parse leaves them.
    let (d, mut p) = split(CCK); p[32 + 8] = (p[32 + 8] & 0xe0) | 23;
    assert_eq!(parse(d, &p).unwrap().signal_dbm, -66);
}
