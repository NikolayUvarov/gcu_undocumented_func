//! usb_serial's chip logic on the host (800-LNK-0002): the FTDI baud rate divisor as AN232B-05 codes it and the two
//! status bytes stripped from each packet; the CH340's rate registers.
#[path = "../usb_serial/src/ftdi.rs"]
#[allow(dead_code)]
mod ftdi;
#[path = "../usb_serial/src/ch340.rs"]
#[allow(dead_code)]
mod ch340;

/// The rate a divisor gives, decoded the other way: 3 MHz over the integer part and eighths.
fn rate_of(value: u16, index: u16) -> u32 {
    let code = value as u32 | (index as u32 & 1) << 16;
    let (n, sub) = (code & 0x3FFF, code >> 14);
    let eighths = [0, 4, 2, 1, 3, 5, 6, 7][sub as usize];
    match (n, eighths) { (0, 0) => 3_000_000, (1, 0) => 2_000_000, _ => (24_000_000 + (8 * n + eighths) / 2) / (8 * n + eighths) }
}

#[test]
fn common_rates() {
    assert_eq!(ftdi::divisor(115_200), Some((26, 0, 115_385)));
    assert_eq!(ftdi::divisor(921_600), Some((0x8003, 0, 923_077)));
    assert_eq!(ftdi::divisor(9_600), Some((0x4138, 0, 9_600)));
    assert_eq!(ftdi::divisor(1_000_000), Some((3, 0, 1_000_000)));
    assert_eq!(ftdi::divisor(1_500_000), Some((2, 0, 1_500_000)));
    assert_eq!(ftdi::divisor(2_000_000), Some((1, 0, 2_000_000)));
    assert_eq!(ftdi::divisor(3_000_000), Some((0, 0, 3_000_000)));
}

#[test]
fn the_third_eighths_bit_goes_to_the_index() {
    // 3/8: code 4, bit 16 of the divisor, which travels in wIndex bit 0.
    let (value, index, actual) = ftdi::divisor(24_000_000 / 211).unwrap();
    assert_eq!((value, index), (26, 1));
    assert_eq!(rate_of(value, index), actual);
}

/// The nearest rate the chip can make, searched near the exact divisor.
fn nearest(baud: u32) -> u32 {
    let exact = 24_000_000 / baud.max(1) as u64;
    let near = exact.saturating_sub(2).max(16)..=(exact + 2).min(0x3FFF * 8 + 7);
    near.map(|e| ((24_000_000 + e / 2) / e) as u32).chain([2_000_000, 3_000_000])
        .min_by_key(|&r| (r as i64 - baud as i64).unsigned_abs()).unwrap()
}

#[test]
fn every_rate_within_three_percent_or_refused() {
    for baud in (300..3_100_000).step_by(997).chain([2_200_000, 2_600_000, 4_000_000, 1_454_923, 100, 1]) {
        let best = nearest(baud);
        let reachable = (best as i64 - baud as i64).unsigned_abs() * 100 <= baud as u64 * 3 && 24_000_000 / baud as u64 <= 0x3FFF * 8 + 7;
        match ftdi::divisor(baud) {
            Some((value, index, actual)) => {
                assert_eq!(rate_of(value, index), actual, "{baud}");
                assert_eq!(actual, best, "{baud}: not the nearest rate");
                assert!(reachable, "{baud} made as {actual}");
            }
            None => assert!(!reachable, "{baud} refused, though {best} is within 3 %"),
        }
    }
    assert_eq!(ftdi::divisor(0), None);
}

#[test]
fn status_bytes_stripped() {
    let mut out = [0u8; 64];
    assert_eq!(ftdi::strip(&[0x01, 0x60], &mut out), (0, false));
    assert_eq!(ftdi::strip(&[0x01, 0x60, b'h', b'i'], &mut out), (2, false));
    assert_eq!(&out[..2], b"hi");
    assert_eq!(ftdi::strip(&[0x01, 0x62, b'x'], &mut out), (1, true)); // overrun
    assert_eq!(ftdi::strip(&[0x01], &mut out), (0, false));
    assert_eq!(ftdi::strip(&[], &mut out), (0, false));
    let mut small = [0u8; 1];
    assert_eq!(ftdi::strip(&[0, 0, 1, 2, 3], &mut small), (1, false));
}

/// The rate the CH340's registers give, decoded the other way.
fn ch340_rate(value: u16) -> u32 {
    let (ps, fact, div) = ((value & 3) as u32, ((value >> 2) & 1) as u32, 256 - (value >> 8) as u64);
    let base = 48_000_000u64 >> (12 - 3 * ps - fact);
    ((base + div / 2) / div) as u32
}

#[test]
fn ch340_common_rates() {
    // 115200: 6 MHz / 52; 460800: 6 MHz / 13; 921600: 12 MHz / 13; 2 Mbit/s: 6 MHz / 3; 9600: 750 kHz / 78.
    assert_eq!(ch340::divisor(115_200), Some((0xCC03, 115_385)));
    assert_eq!(ch340::divisor(460_800), Some((0xF303, 461_538)));
    assert_eq!(ch340::divisor(921_600), Some((0xF307, 923_077)));
    assert_eq!(ch340::divisor(2_000_000), Some((0xFD03, 2_000_000)));
    assert_eq!(ch340::divisor(9_600), Some((0xB202, 9_615)));
    assert_eq!(ch340::divisor(0), None);
}

#[test]
fn ch340_every_rate_within_three_percent_or_refused() {
    for baud in (50..3_000_000).step_by(1_013) {
        if let Some((value, actual)) = ch340::divisor(baud) {
            assert_eq!(ch340_rate(value), actual, "{baud}");
            assert!((actual as i64 - baud as i64).unsigned_abs() * 100 <= baud as u64 * 3, "{baud} made as {actual}");
            let div = 256 - (value >> 8);
            assert!((2..=255).contains(&div), "{baud}: divisor {div}");
            if value & 4 != 0 { assert!(div >= 9 && div % 2 == 1, "{baud}: the higher clock with divisor {div}"); }
        }
    }
    // The common rates are all made.
    for baud in [1_200, 2_400, 4_800, 9_600, 19_200, 38_400, 57_600, 115_200, 230_400, 460_800, 921_600, 1_000_000, 1_500_000, 2_000_000] {
        assert!(ch340::divisor(baud).is_some(), "{baud} refused");
    }
    assert_eq!(ch340::DTR_RTS_ON, 0x9F);
}
