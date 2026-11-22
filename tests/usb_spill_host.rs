//! Host tests of usb_host/src/spill.rs (158-DRV-0082): a bulk stream's transfers kept in order up to a bound, an empty one
//! and an error kept as such, and given back whole as the client's buffer allows.
extern crate alloc;
#[path = "../usb_host/src/spill.rs"]
mod spill;
use spill::Spill;

// The client's form of transfers: two length bytes (bit 15 an error) and the bytes, as `isochronous` gives them.
fn parse(mut bytes: &[u8]) -> Vec<(Vec<u8>, bool)> {
    let mut out = Vec::new();
    while bytes.len() >= 2 {
        let head = u16::from_le_bytes([bytes[0], bytes[1]]);
        let n = (head & 0x7FFF) as usize;
        out.push((bytes[2..2 + n].to_vec(), head & 0x8000 != 0));
        bytes = &bytes[2 + n..];
    }
    out
}

#[test]
fn transfers_come_back_in_order_with_their_empty_and_failed_ones() {
    let mut s = Spill::new(1 << 20);
    let sent: Vec<(Vec<u8>, bool)> = vec![(vec![12, 0x8C, 1, 2, 3], false), (vec![], false), (vec![9; 4096], true), ((0..=255).collect(), false)];
    for (data, damaged) in &sent { assert!(s.push(data, *damaged)); }
    assert_eq!(s.len(), (sent.iter().map(|(d, _)| 2 + d.len()).sum(), 4));
    let mut out = vec![0u8; 64 * 1024];
    let n = s.take(&mut out);
    assert_eq!(parse(&out[..n]), sent);
    assert!(s.is_empty() && s.len() == (0, 0));
}

#[test]
fn a_full_spill_refuses_and_the_transfer_waits() {
    let mut s = Spill::new(3 * (2 + 4096));
    for _ in 0..3 { assert!(s.push(&[1; 4096], false)); }
    assert!(!s.push(&[2; 4096], false), "past the bound: refused, nothing kept");
    assert_eq!((s.len().1, s.refused, s.most), (3, 1, 3 * 4098));
    assert!(!s.push(&[0; 0x8000], false), "a transfer longer than 15 bits of length is refused");
}

#[test]
fn a_small_buffer_takes_whole_transfers_only_and_the_rest_waits() {
    let mut s = Spill::new(1 << 20);
    // Many pushes and takes, so the deque wraps: the bytes stay in order across its two halves.
    let mut next = 0u32;
    let mut expected: Vec<(Vec<u8>, bool)> = Vec::new();
    let mut got: Vec<(Vec<u8>, bool)> = Vec::new();
    for round in 0..200 {
        for _ in 0..(round % 7 + 1) {
            let len = (next * 37 % 3000) as usize;
            let data: Vec<u8> = (0..len).map(|i| (i as u32 + next) as u8).collect();
            let damaged = next % 11 == 0;
            assert!(s.push(&data, damaged));
            expected.push((data, damaged));
            next += 1;
        }
        let mut out = vec![0u8; 2500 + round * 13];
        let n = s.take(&mut out);
        assert!(n <= out.len());
        got.extend(parse(&out[..n]));
    }
    let mut out = vec![0u8; 1 << 20];
    let n = s.take(&mut out);
    got.extend(parse(&out[..n]));
    assert_eq!(got, expected, "every transfer, in order, none cut");
    let mut tiny = [0u8; 3];
    assert!(s.push(&[5; 2], false));
    assert_eq!(s.take(&mut tiny[..3]), 0, "2 + 2 bytes do not fit in 3: nothing taken");
    assert_eq!(s.take(&mut [0u8; 4]), 4);
}
