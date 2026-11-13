// The boot records the updater writes (351-UPD-0007, docs/update/slots.md): always into the file that does not hold the
// newer valid record, one past its sequence. Free of the system so the host tests (tests/updater_host.rs) run it with
// the bootloader's own encoding.
use super::slots::{Record, SEQUENCE_LAST};

/// What a file a manifest lists is to a slot (351-UPD-0023): the kernel or a boot service the bootloader loads, the
/// loader the slot carries for itself (351-UPD-0010), an application the slot carries with them, the updater's period
/// the release carries (351-UPD-0036), or a file of the root shared by both slots (licences, data, another processor's
/// loader).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind { Boot, Loader, Application, Period, Shared }

/// A release's period for the updater, in its slot (351-UPD-0036): not `update.txt`, which is the root's.
pub const PERIOD: &str = "period.txt";

/// `boot` is the bootloader's list of boot services (BOOT_FILES), `loader` the loader's path for this processor
/// (`EFI/BOOT/BOOTX64.EFI`); a volume's names may come back in another case.
pub fn kind(path: &str, boot: &[&str], loader: &str) -> Kind {
    if path.eq_ignore_ascii_case("kernel.elf") || boot.iter().any(|name| name.eq_ignore_ascii_case(path)) { return Kind::Boot; }
    if path.eq_ignore_ascii_case(loader) { return Kind::Loader; }
    if path.eq_ignore_ascii_case(PERIOD) { return Kind::Period; }
    let elf = path.len() > 4 && path.as_bytes()[path.len() - 4..].eq_ignore_ascii_case(b".elf");
    if elf && !path.contains('/') { Kind::Application } else { Kind::Shared }
}

/// The seconds a slot's period.txt names (351-UPD-0036): one line `every SECONDS`, with `#` comments and blank lines.
/// Any other line, a second `every` or none is refused: the line, or why. `source`, `pin` and the rest are the root's.
pub fn period(text: &str) -> Result<u64, &str> {
    let mut every = None;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        let mut words = line.split_whitespace();
        match (words.next(), words.next().map(str::parse::<u64>), words.next()) {
            (Some("every"), Some(Ok(seconds)), None) if every.is_none() => every = Some(seconds),
            _ => return Err(line),
        }
    }
    every.ok_or("NO `every` LINE")
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

/// The version floor the records hold (351-UPD-0024): the newer valid one's, as the bootloader reads it; 0 with none.
pub fn floor(records: [Option<Record>; 2]) -> u64 { newest(records).map_or(0, |(_, r)| r.floor) }

/// The lowest key file version a slot may hold (351-UPD-0029): the newer valid record's, as the bootloader reads it.
pub fn keys(records: [Option<Record>; 2]) -> u64 { newest(records).map_or(0, |(_, r)| r.keys) }

/// `record` one past the newer valid one, and the file it goes to; file 0 with sequence 1 when none is valid. It
/// carries the newer record's floor and key file version, or `record`'s when higher: no write of the updater lowers
/// them. None when the sequence could not count on: a trial needs two more, which the bootloader writes.
fn next(records: [Option<Record>; 2], record: Record) -> Option<(usize, Record)> {
    let (file, sequence) = newest(records).map_or((0, 1), |(k, r)| (1 - k, r.sequence + 1));
    let (floor, keys) = (floor(records).max(record.floor), keys(records).max(record.keys));
    (sequence <= SEQUENCE_LAST - 2).then_some((file, Record { sequence, floor, keys, ..record }))
}

/// The record that confirms `running` once its trial boot was confirmed: the same slot and fallback, confirmed, the key
/// file version raised to `keys`, the running slot's (351-UPD-0029): from then a slot whose key file is below it, which
/// may name keys that version revoked, is not booted. None when the newer record already confirms it, or names another
/// slot.
pub fn confirm(records: [Option<Record>; 2], running: u8, keys: u64) -> Option<(usize, Record)> {
    let (_, current) = newest(records)?;
    if current.slot != running || current.confirmed { return None; }
    next(records, Record { slot: running, fallback: current.fallback, tries: 0, confirmed: true, sequence: 0, floor: 0, keys })
}

/// Whether the other slot may be written: not while the running slot is on trial and not yet confirmed on the disk,
/// since the other slot is then its fallback.
pub fn may_stage(records: [Option<Record>; 2], running: u8, trial: bool) -> bool {
    !trial || newest(records).is_some_and(|(_, r)| r.slot == running && r.confirmed)
}

/// The record that boots the other slot on trial with `tries`, falling back to the running one: an update's activation
/// point, and a rollback's.
pub fn trial_of_other(records: [Option<Record>; 2], running: u8, tries: u8) -> Option<(usize, Record)> {
    next(records, Record { slot: other(running), fallback: running, tries, confirmed: false, sequence: 0, floor: 0, keys: 0 })
}

/// The record a recovery order writes (351-UPD-0030): the other slot on trial with one try, falling back to the running
/// one, the floor lowered to `floor`. The one write that lowers the floor, and only for an order the recovery keys'
/// threshold signed; the key file version stays (351-UPD-0029). A trial needs two more sequence numbers, which the
/// bootloader writes.
pub fn recover(records: [Option<Record>; 2], running: u8, floor: u64) -> Option<(usize, Record)> {
    let (file, sequence) = newest(records).map_or((0, 1), |(k, r)| (1 - k, r.sequence + 1));
    let keys = keys(records);
    (sequence <= SEQUENCE_LAST - 2).then_some((file, Record { slot: other(running), fallback: running, tries: 1, confirmed: false, sequence, floor, keys }))
}

/// The record that raises the version floor (351-UPD-0024) to `minimum`, a checked channel's, once the newer record
/// confirms `running` and its manifest names `version` at or above it: the newer record again, one on, with that
/// floor. None leaves the floor as it is: it is never lowered, and a confirmed trial alone raises nothing, so a
/// rollback to an earlier release at or above the minimum stays possible.
pub fn raise(records: [Option<Record>; 2], running: u8, minimum: u64, version: u64) -> Option<(usize, Record)> {
    let (_, current) = newest(records)?;
    let confirmed = current.slot == running && current.confirmed;
    (minimum > floor(records) && confirmed && version >= minimum).then(|| next(records, Record { floor: minimum, ..current })).flatten()
}

/// The last known good release among those the block store keeps (351-UPD-0033): the newest below the running one
/// and not below `minimum`, which a rollback may boot; none while the running version is not known.
pub fn last_good(versions: &[u64], running: u64, minimum: u64) -> Option<u64> {
    versions.iter().copied().filter(|&v| v < running && v >= minimum).max()
}

/// Whether the store keeps release `version` of `versions` (351-UPD-0033, MC-9.3): the running one, the last known
/// good, and one staged above the running one. While the running version is not known, it keeps all of them.
pub fn keeps(version: u64, versions: &[u64], running: u64, minimum: u64, staged: u64) -> bool {
    running == 0 || version == running || Some(version) == last_good(versions, running, minimum) || (staged > running && version == staged)
}

/// The version a boot manifest's `version` header line names, 0 without one: as the bootloader reads it (351-UPD-0024).
pub fn manifest_version(text: &[u8]) -> u64 {
    let line = text.split(|&b| b == b'\n').take_while(|l| !l.starts_with(b"file ")).find(|l| l.starts_with(b"version "));
    line.and_then(|l| core::str::from_utf8(&l[8..]).ok()).and_then(|v| v.parse().ok()).unwrap_or(0)
}
