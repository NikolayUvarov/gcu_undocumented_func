// 213-DRV-0087: the signal of a received PPDU from its status report, against reports captured on the Ryzen.
#![allow(dead_code)]
#[path = "../rtw89_wifi/src/ppdu.rs"] mod ppdu;
#[path = "../rtw89_wifi/src/rxdesc.rs"] mod rxdesc;
use ppdu::{parse, weakness, Conversion, Report};

#[path = "rtw89_ppdu_captures.rs"] mod captures;
use captures::*;

fn split(hex: &str) -> ([u32; 3], Vec<u8>) {
    let b = bytes(hex);
    let w = |at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let (w0, w1, w3) = (w(4), w(8), w(16));
    let start = 4 + if w0 >> 31 != 0 { 32 } else { 16 };
    (([w0, w1, w3]), b[start..start + (w0 & 0x3fff) as usize].to_vec())
}
fn signal(hex: &str) -> Option<Report> { let (d, p) = split(hex); parse(d, &p) }

#[test]
fn reports_captured_on_the_ryzen_give_linux_s_signal() {
    assert_eq!(signal(FIVE), Some(Report { count: 1, rate: 4, signal_dbm: -63, header_dbm: -63, conversion: Conversion::Header }),
        "5 GHz: no RPL conversion for a break");
    assert_eq!(signal(CCK), Some(Report { count: 4, rate: 0, signal_dbm: -65, header_dbm: -66,
        conversion: Conversion::Rpl { header_over: -2, ofdm: false } }), "2.4 GHz CCK: paths 88 moved by +2");
    assert_eq!(signal(BREAK).unwrap().signal_dbm, -64);
    assert_eq!(signal(DATA).unwrap().signal_dbm, -61, "paths 104/108 moved by RPL 97 less the average 106");
    assert_eq!(signal(OTHER).unwrap().signal_dbm, -75);
    assert_eq!(signal(DATA).unwrap().conversion, Conversion::Rpl { header_over: 9, ofdm: true }, "the average 106 over the RPL 97");
    assert_eq!((signal(BREAK).unwrap().conversion, signal(OTHER).unwrap().conversion),
        (Conversion::Header, Conversion::Rpl { header_over: -6, ofdm: false }));
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

// 213-DRV-0102: an OFDM report not to this radio has no RPL (Linux reads IE 1's only to_self, core.c:2006); Linux
// moves its paths by 0 less the average all the same, so its signal wraps; the header's stronger path is kept beside it.
#[test]
fn an_ofdm_report_not_to_this_radio_keeps_the_headers_signal_beside_linuxs() {
    let (d, p) = split(DATA);
    let other = parse([d[0], d[1], d[2] & !1], &p).unwrap();
    // Paths 104 and 108 less the average 106: 254 and 2 as bytes, the stronger 254, +17 dBm; the header's 108 is -56.
    assert_eq!((other.signal_dbm, other.header_dbm, other.conversion), (17, -56, Conversion::NoRpl));
    assert_eq!(parse(d, &p).unwrap().header_dbm, -56, "the same header to this radio");
    // Below OFDM 6 Mb/s IE 1 gives no RPL either, as Linux returns before it reads one.
    assert_eq!(parse([d[0], d[1] & !(0x1ff << 16) | 3 << 16, d[2]], &p).unwrap().conversion, Conversion::NoRpl);
}

// A report goes on the frame kept just before it, by its count and rate, and on no other (Matcher).
#[test]
fn a_report_marks_the_kept_frame_just_before_it_and_no_other() {
    use ppdu::{Matcher, Signals};
    let beacon = frame(&[0x80; 40], 1, 4);
    let report = bytes(FIVE);
    let packet = |b: &[u8]| rxdesc::parse(b).unwrap();
    let mut m = Matcher::default();
    m.kept(7, 2, &beacon); assert!(m.waits(2) && !m.waits(1));
    assert_eq!(m.report(Some(7), packet(&report), &report), Some((2, 63)), "-63 dBm on slot 2");
    assert!(!m.waits(2));
    assert_eq!(m.report(Some(7), packet(&report), &report), None, "a second report finds no frame");
    // Handed on before its report: late, the frame keeps its 0.
    m.kept(8, 3, &beacon); m.handed(1); assert!(m.waits(3)); m.handed(3); assert!(!m.waits(3));
    assert_eq!(m.report(Some(8), packet(&report), &report), None);
    // Another Wi-Fi frame came between, another report came between, another count, another rate, a broken report,
    // the kept frames dropped: none is guessed.
    for (number, after, count, rate) in [(9, Some(10), 1, 4), (9, None, 1, 4), (9, Some(9), 2, 4), (9, Some(9), 1, 5)] {
        m.kept(number, 0, &frame(&[0x80; 40], count, rate)); assert_eq!(m.report(after, packet(&report), &report), None);
    }
    // Eight bytes short of what its PHY status says, the ring's lengths agreeing.
    let mut broken = report.clone(); broken.truncate(report.len() - 8);
    broken[..4].copy_from_slice(&(report.len() as u32 - 8 | 0xc000).to_le_bytes()); broken[4] -= 8;
    m.kept(11, 0, &beacon); assert_eq!(m.report(Some(11), packet(&broken), &broken), None);
    m.kept(12, 0, &beacon); m.forget(); assert!(!m.waits(0)); assert_eq!(m.report(Some(12), packet(&report), &report), None);
    assert_eq!(m.signals, Signals { measured: 1, late: 1, unmatched: 7 });
}
