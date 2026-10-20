//! usb_serial's FTDI logic on the host (800-LNK-0002): the baud rate divisor as AN232B-05 codes it, and the two status
//! bytes stripped from each packet.
#[path = "../usb_serial/src/ftdi.rs"]
#[allow(dead_code)]
mod ftdi;

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
