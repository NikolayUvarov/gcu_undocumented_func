// The boot records the updater writes (351-UPD-0007, docs/update/slots.md): always into the file that does not hold the
// newer valid record, one past its sequence. Free of the system so the host tests (tests/updater_host.rs) run it with
// the bootloader's own encoding.
use super::slots::{Record, SEQUENCE_LAST};

/// The other slot.
pub fn other(slot: u8) -> u8 { if slot == b'A' { b'B' } else { b'A' } }

/// The newer valid record and its file, as the bootloader chooses it.
pub fn newest(records: [Option<Record>; 2]) -> Option<(usize, Record)> {
    match records {
        [Some(a), Some(b)] => Some(if b.sequence > a.sequence { (1, b) } else { (0, a) }),
        [Some(a), None] => Some((0, a)),
        [None, Some(b)] => Some((1, b)),
        [None, None] => None,
    }
}

/// `record` one past the newer valid one, and the file it goes to; file 0 with sequence 1 when none is valid. None when
/// the sequence could not count on: a trial needs two more, which the bootloader writes.
fn next(records: [Option<Record>; 2], record: Record) -> Option<(usize, Record)> {
    let (file, sequence) = newest(records).map_or((0, 1), |(k, r)| (1 - k, r.sequence + 1));
    (sequence <= SEQUENCE_LAST - 2).then_some((file, Record { sequence, ..record }))
}

/// The record that confirms `running` once its trial boot was confirmed: the same slot and fallback, confirmed. None
/// when the newer record already confirms it, or names another slot.
pub fn confirm(records: [Option<Record>; 2], running: u8) -> Option<(usize, Record)> {
    let (_, current) = newest(records)?;
    if current.slot != running || current.confirmed { return None; }
    next(records, Record { slot: running, fallback: current.fallback, tries: 0, confirmed: true, sequence: 0 })
}

/// Whether the other slot may be written: not while the running slot is on trial and not yet confirmed on the disk,
/// since the other slot is then its fallback.
pub fn may_stage(records: [Option<Record>; 2], running: u8, trial: bool) -> bool {
    !trial || newest(records).is_some_and(|(_, r)| r.slot == running && r.confirmed)
}

/// The record that boots the other slot on trial with `tries`, falling back to the running one: an update's activation
/// point, and a rollback's.
pub fn trial_of_other(records: [Option<Record>; 2], running: u8, tries: u8) -> Option<(usize, Record)> {
    next(records, Record { slot: other(running), fallback: running, tries, confirmed: false, sequence: 0 })
}
