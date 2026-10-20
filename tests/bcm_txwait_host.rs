// Host tests of bcm_wifi's transmit-status wait and ring room (550-DRV-0027, the audit's H12): the production logic,
// driven by a scripted status FIFO and a clock that advances on every look.
#[path = "../bcm_wifi/src/txwait.rs"]
mod txwait;

use std::cell::Cell;
use txwait::{cookie, room, wait, INTERMEDIATE, VALID};

const END: usize = 100;

// A final status word for `cookie`: sent once, acknowledged.
fn status(c: u16) -> u32 { (c as u32) << 16 | 1 << 12 | 2 | VALID }

// Runs the wait over `entries` (None: the FIFO is empty at that look), each look of the clock 1 ms later; the result,
// the entries taken and the last time seen.
fn run(entries: Vec<Option<u32>>, mine: u16) -> (Option<u32>, usize, usize) {
    let clock = Cell::new(0usize);
    let taken = Cell::new(0usize);
    let mut queue = entries.into_iter();
    let result = wait(|| { taken.set(taken.get() + 1); queue.next().flatten().map(|first| (first, 0)) }, mine,
                      || { let t = clock.get(); clock.set(t + 1); t }, END, || {});
    (result, taken.get(), clock.get())
}

#[test]
fn other_statuses_without_end_still_stop_at_the_deadline() {
    let mine = cookie(7, 3);
    let others: Vec<Option<u32>> = (0..10_000).map(|i| Some(if i % 2 == 0 { status(cookie(6, 3)) } else { status(mine) | INTERMEDIATE })).collect();
    let (result, taken, clock) = run(others, mine);
    assert_eq!(result, None);
    assert!(taken <= END && clock <= END + 2, "taken {taken}, clock {clock}");
}

#[test]
fn no_status_ends_at_the_deadline() {
    let (result, _, clock) = run(vec![None; 1000], cookie(0, 0));
    assert_eq!(result, None);
    assert!(clock <= END + 2);
}

#[test]
fn a_matching_status_before_the_deadline_is_taken() {
    let mine = cookie(1, 15);
    let (result, taken, _) = run(vec![None, Some(status(cookie(0, 15))), Some(status(mine))], mine);
    assert_eq!(result, Some(status(mine) & 0xFFFF));
    assert_eq!(taken, 3);
}

#[test]
fn a_matching_status_after_the_deadline_does_not_count() {
    let mine = cookie(2, 0);
    let mut entries = vec![None; 150];
    entries.push(Some(status(mine)));
    assert_eq!(run(entries, mine).0, None);
}

#[test]
fn a_gone_chip_reads_as_empty() {
    let mine = cookie(3, 1);
    assert_eq!(run(vec![Some(u32::MAX); 500], mine).0, None);
}

#[test]
fn cookies_differ_by_generation_and_are_never_0_or_ffff() {
    for g in 0..=255u8 {
        for s in 0..16 {
            let c = cookie(g, s);
            assert!(c != 0 && c != 0xFFFF && c >> 12 == 2);
            assert_ne!(c, cookie(g.wrapping_add(1), s));
        }
    }
}

#[test]
fn the_ring_keeps_one_slot_free_and_waits_for_the_engine() {
    assert!(room(0, 0, 16));
    assert!(room(14, 0, 16));
    assert!(!room(15, 0, 16), "a full ring would make the engine's index equal its current one");
    assert!(room(15, 1, 16));
    assert!(!room(3, 4, 16));
}
