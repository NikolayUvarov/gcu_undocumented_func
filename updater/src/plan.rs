// The boot records the updater writes (351-UPD-0007, docs/update/slots.md): always into the file that does not hold the
// newer valid record, one past its sequence. Free of the system so the host tests (tests/updater_host.rs) run it with
// the bootloader's own encoding.
use super::slots::{Record, SEQUENCE_LAST};

/// What a file a manifest lists is to a slot (351-UPD-0023): the kernel or a boot service the bootloader loads, the
/// loader the slot carries for itself (351-UPD-0010), an application the slot carries with them, or a file of the root
/// shared by both slots (licences, data, another processor's loader).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind { Boot, Loader, Application, Shared }

/// `boot` is the bootloader's list of boot services (BOOT_FILES), `loader` the loader's path for this processor
/// (`EFI/BOOT/BOOTX64.EFI`); a volume's names may come back in another case.
pub fn kind(path: &str, boot: &[&str], loader: &str) -> Kind {
    if path.eq_ignore_ascii_case("kernel.elf") || boot.iter().any(|name| name.eq_ignore_ascii_case(path)) { return Kind::Boot; }
    if path.eq_ignore_ascii_case(loader) { return Kind::Loader; }
    let elf = path.len() > 4 && path.as_bytes()[path.len() - 4..].eq_ignore_ascii_case(b".elf");
    if elf && !path.contains('/') { Kind::Application } else { Kind::Shared }
}

/// A slot's file's name in the slot: the loader's without its `EFI/BOOT/` (`BOOTX64.EFI`), the others as listed.
pub fn in_slot(path: &str) -> &str { path.rsplit('/').next().unwrap_or(path) }

/// Whether metadata expiring at `expires` (seconds since 1970) is past at `now`; a clock that cannot be read shows
/// nothing current (MC-9.4, 351-UPD-0025).
pub fn expired(expires: u64, now: Option<u64>) -> bool { now.is_none_or(|now| expires <= now) }

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

/// The version floor to write (351-UPD-0024): the highest minimum seen, once the running slot is confirmed on the disk
/// at that version or above. None leaves the floor as it is: it is never lowered, and a confirmed trial alone raises
/// nothing, so a rollback to an earlier release at or above the minimum stays possible.
pub fn floor_to(current: u64, minimum: u64, running: u64, confirmed: bool) -> Option<u64> {
    (minimum > current && confirmed && running >= minimum).then_some(minimum)
}

/// The version a boot manifest's `version` header line names, 0 without one: as the bootloader reads it (351-UPD-0024).
pub fn manifest_version(text: &[u8]) -> u64 {
    let line = text.split(|&b| b == b'\n').take_while(|l| !l.starts_with(b"file ")).find(|l| l.starts_with(b"version "));
    line.and_then(|l| core::str::from_utf8(&l[8..]).ok()).and_then(|v| v.parse().ok()).unwrap_or(0)
}
