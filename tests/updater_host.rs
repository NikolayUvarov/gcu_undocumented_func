//! Host tests of the boot records the updater writes (updater/src/plan.rs, 351-UPD-0007; MC-9.3): each goes into the
//! file that does not hold the newer valid record, and the bootloader (bootloader/src/slots.rs) follows it as meant.
#[allow(dead_code)]
#[path = "../bootloader/src/slots.rs"]
mod slots;
#[path = "../updater/src/plan.rs"]
mod plan;

use plan::{confirm, may_stage, newest, other, trial_of_other};
use slots::{Record, SEQUENCE_LAST};

fn rec(sequence: u64, slot: u8, fallback: u8, tries: u8, confirmed: bool) -> Record {
    Record { sequence, slot, fallback, tries, confirmed }
}

// The records after `write` lands in its file.
fn after(mut records: [Option<Record>; 2], write: (usize, Record)) -> [Option<Record>; 2] {
    records[write.0] = Record::parse(&write.1.encode());
    records
}

#[test]
fn the_newer_record_is_the_bootloaders() {
    let (a, b) = (rec(4, b'A', 0, 0, true), rec(5, b'B', b'A', 2, false));
    assert_eq!(newest([Some(a), Some(b)]), Some((1, b)));
    assert_eq!(newest([Some(b), Some(a)]), Some((0, b)));
    assert_eq!(newest([None, Some(a)]), Some((1, a)));
    assert_eq!(newest([None, None]), None);
    // Equal sequences: the bootloader takes the first file.
    assert_eq!(newest([Some(a), Some(rec(4, b'B', 0, 0, true))]), Some((0, a)));
    assert_eq!((other(b'A'), other(b'B')), (b'B', b'A'));
}

#[test]
fn an_update_boots_the_other_slot_on_trial_then_the_bootloader_counts_it() {
    // Slot A confirmed in BOOT0, as boot_slots.py layout leaves it; BOOT1 empty.
    let records = [Some(rec(1, b'A', 0, 0, true)), None];
    let write = trial_of_other(records, b'A', 3).unwrap();
    assert_eq!(write, (1, rec(2, b'B', b'A', 3, false)));
    let records = after(records, write);
    let boot = slots::plan(records);
    assert_eq!((boot.order, boot.trial), ([b'B', b'A'], true));
    // The bootloader counts the try in the other file, then the updater confirms it there again.
    let (file, counted) = boot.write.unwrap();
    assert_eq!((file, counted), (0, rec(3, b'B', b'A', 2, false)));
    let records = after(records, (file, counted));
    assert!(!may_stage(records, b'B', true), "the fallback is kept while the trial runs");
    let write = confirm(records, b'B').unwrap();
    assert_eq!(write, (1, rec(4, b'B', b'A', 0, true)));
    let records = after(records, write);
    assert_eq!(slots::plan(records).order, [b'B', b'A']);
    assert!(!slots::plan(records).trial);
    assert!(may_stage(records, b'B', true));
    assert_eq!(confirm(records, b'B'), None, "a confirmed slot is not confirmed again");
}

#[test]
fn a_failed_trial_leaves_the_fallback_running_and_its_slot_to_fill() {
    // B's trial used its tries: A boots, not on trial, and B may be filled again.
    let records = [Some(rec(6, b'B', b'A', 0, false)), Some(rec(5, b'B', b'A', 1, false))];
    assert_eq!(slots::plan(records).order, [b'A', 0]);
    assert_eq!(confirm(records, b'A'), None, "the record names the failed slot, not the running one");
    assert!(may_stage(records, b'A', false));
    assert_eq!(trial_of_other(records, b'A', 2), Some((1, rec(7, b'B', b'A', 2, false))));
}

#[test]
fn a_rollback_is_a_trial_of_the_other_slot() {
    let records = [Some(rec(9, b'B', b'A', 0, true)), Some(rec(8, b'B', b'A', 2, false))];
    let write = trial_of_other(records, b'B', 1).unwrap();
    assert_eq!(write, (1, rec(10, b'A', b'B', 1, false)));
    let boot = slots::plan(after(records, write));
    assert_eq!((boot.order, boot.trial), ([b'A', b'B'], true));
}

#[test]
fn without_a_valid_record_the_first_file_gets_sequence_one() {
    assert_eq!(trial_of_other([None, None], b'A', 3), Some((0, rec(1, b'B', b'A', 3, false))));
    assert_eq!(confirm([None, None], b'A'), None);
    assert!(may_stage([None, None], b'A', false));
    assert!(!may_stage([None, None], b'A', true));
}

#[test]
fn a_sequence_that_could_not_count_on_is_not_written() {
    let last = SEQUENCE_LAST - 2;
    assert_eq!(trial_of_other([Some(rec(last - 1, b'A', 0, 0, true)), None], b'A', 3), Some((1, rec(last, b'B', b'A', 3, false))));
    assert_eq!(trial_of_other([Some(rec(last, b'A', 0, 0, true)), None], b'A', 3), None);
    assert_eq!(confirm([Some(rec(last, b'A', b'B', 1, false)), None], b'A'), None);
}
