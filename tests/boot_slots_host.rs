//! Host tests of the boot records (bootloader/src/slots.rs, issue 351-UPD-0006; MC-9.1, 9.3): the one encoding and
//! its refusals, a torn write, and the choice the bootloader makes from two records.
#[path = "../bootloader/src/slots.rs"]
mod slots;

use slots::{crc32, plan, spent, Plan, Record, RECORD};

fn rec(sequence: u64, slot: u8, fallback: u8, tries: u8, confirmed: bool) -> Record {
    Record { sequence, slot, fallback, tries, confirmed }
}

#[test]
fn crc_is_zlibs() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(b""), 0);
}

#[test]
fn a_record_round_trips() {
    for r in [rec(1, b'A', 0, 0, true), rec(7, b'B', b'A', 3, false), rec(u64::MAX, b'B', b'B', 255, true)] {
        let data = r.encode();
        assert_eq!(data.len(), RECORD);
        assert_eq!(&data[..8], b"MINDBOOT");
        assert_eq!(Record::parse(&data), Some(r));
    }
}

#[test]
fn damaged_records_are_refused() {
    let good = rec(5, b'B', b'A', 2, false).encode();
    // Every single flipped bit: the CRC or a field check refuses it.
    for byte in 0..RECORD {
        for bit in 0..8 {
            let mut data = good;
            data[byte] ^= 1 << bit;
            assert_eq!(Record::parse(&data), None, "byte {byte} bit {bit}");
        }
    }
    // A torn write: the first half of a new record over an old one.
    let mut torn = rec(4, b'A', 0, 0, true).encode();
    torn[..256].copy_from_slice(&good[..256]);
    assert_eq!(Record::parse(&torn), None);
    assert_eq!(Record::parse(&[0u8; RECORD]), None);
    assert_eq!(Record::parse(&good[..RECORD - 1]), None);
    // Fields the CRC covers but the format does not allow.
    for (offset, value) in [(20, b'C'), (21, b'Z'), (23, 2), (100, 1)] {
        let mut data = good;
        data[offset] = value;
        let crc = crc32(&data[..RECORD - 4]);
        data[RECORD - 4..].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(Record::parse(&data), None, "offset {offset}");
    }
}

#[test]
fn the_newer_valid_record_counts() {
    let a = rec(4, b'A', 0, 0, true);
    let b = rec(5, b'B', b'A', 0, true);
    assert_eq!(plan([Some(a), Some(b)]).chosen, Some(1));
    assert_eq!(plan([Some(b), Some(a)]).chosen, Some(0));
    // A torn newer record is not read: the older one counts.
    assert_eq!(plan([Some(a), None]), Plan { chosen: Some(0), order: [b'A', 0], trial: false, write: None });
    assert_eq!(plan([None, Some(b)]), Plan { chosen: Some(1), order: [b'B', b'A'], trial: false, write: None });
}

#[test]
fn no_valid_record_tries_either_slot() {
    assert_eq!(plan([None, None]), Plan { chosen: None, order: [b'A', b'B'], trial: false, write: None });
}

#[test]
fn a_trial_counts_down_on_the_other_record() {
    let confirmed_a = rec(1, b'A', 0, 0, true);
    let staged_b = rec(2, b'B', b'A', 2, false);
    let first = plan([Some(confirmed_a), Some(staged_b)]);
    assert_eq!(first, Plan { chosen: Some(1), order: [b'B', b'A'], trial: true, write: Some((0, rec(3, b'B', b'A', 1, false))) });
    // The write leaves the staged record as it was: a cut during it loses one try at most.
    let second = plan([Some(first.write.unwrap().1), Some(staged_b)]);
    assert_eq!(second, Plan { chosen: Some(0), order: [b'B', b'A'], trial: true, write: Some((1, rec(4, b'B', b'A', 0, false))) });
    // Not confirmed and no tries left: the last confirmed slot, and nothing written.
    let third = plan([Some(rec(3, b'B', b'A', 1, false)), Some(second.write.unwrap().1)]);
    assert_eq!(third, Plan { chosen: Some(1), order: [b'A', 0], trial: false, write: None });
}

#[test]
fn a_confirmed_slot_keeps_its_fallback() {
    let p = plan([Some(rec(9, b'B', b'A', 0, true)), Some(rec(8, b'B', b'A', 0, false))]);
    assert_eq!(p, Plan { chosen: Some(0), order: [b'B', b'A'], trial: false, write: None });
    // A record that names its own slot as the fallback has none.
    assert_eq!(plan([Some(rec(2, b'A', b'A', 0, true)), None]).order, [b'A', 0]);
    // Not confirmed, no tries, no fallback: the slot itself rather than nothing.
    assert_eq!(plan([Some(rec(2, b'B', 0, 0, false)), None]).order, [b'B', 0]);
}

#[test]
fn a_trial_that_fails_is_spent() {
    let p = plan([Some(rec(1, b'A', 0, 0, true)), Some(rec(2, b'B', b'A', 3, false))]);
    let (file, record) = spent(p.write.unwrap());
    assert_eq!((file, record), (1, rec(4, b'B', b'A', 0, false)));
    assert_eq!(plan([Some(p.write.unwrap().1), Some(record)]).order, [b'A', 0]);
}
