//! Host tests of the A64's counter workaround (common/counter.rs, 100000-PRT-0012): a read whose low 9 bits are all
//! ones or all zeros is taken again, a good one at once, and the tries are bounded.
#[path = "../common/counter.rs"]
mod counter;

use counter::{sun50i_stable, TRIES};

// A counter that answers `values` in turn, then the last one for ever; how many reads were made.
fn model(values: &[u64]) -> (u64, usize) {
    let mut reads = 0;
    let value = sun50i_stable(|| { let v = values[reads.min(values.len() - 1)]; reads += 1; v });
    (value, reads)
}

#[test]
fn a_good_read_is_taken_at_once() {
    assert_eq!(model(&[0x1234_5678]), (0x1234_5678, 1));
    assert_eq!(model(&[0x200 + 2]), (0x202, 1)); // low bits 2: (v + 1) & 0x1FF = 3
    assert_eq!(model(&[0x1FE]), (0x1FE, 1));
}

#[test]
fn the_errata_values_are_read_again() {
    // All ones in the low 9 bits (a carry the read caught), then all zeros, then a good value.
    assert_eq!(model(&[0x0000_01FF, 0x0000_0400, 0x0000_0401]), (0x401, 3));
    // A wrong read far ahead, as the A64 gives (the high bits jumped with the low ones at 0x1FF).
    assert_eq!(model(&[0x00FF_FFFF_FFFF_FFFF, 0x1000_0003]), (0x1000_0003, 2));
    assert_eq!(model(&[u64::MAX, 7]), (7, 2)); // wraps, no overflow
}

#[test]
fn the_tries_are_bounded() {
    let (value, reads) = model(&[0x600]);
    assert_eq!((value, reads), (0x600, TRIES as usize));
}
