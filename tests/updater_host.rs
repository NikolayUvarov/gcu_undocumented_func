//! Host tests of the boot records the updater writes (updater/src/plan.rs, 351-UPD-0007; MC-9.3): each goes into the
//! file that does not hold the newer valid record, and the bootloader (bootloader/src/slots.rs) follows it as meant.
extern crate alloc;
#[allow(dead_code)]
#[path = "../bootloader/src/slots.rs"]
mod slots;
#[path = "../updater/src/plan.rs"]
mod plan;
#[path = "../updater/src/bundle.rs"]
mod bundle;
#[allow(dead_code)]
#[path = "../libmind/src/zip.rs"]
mod zip;
#[allow(dead_code)]
#[path = "../libmind/src/inflate.rs"]
mod inflate;

use plan::{confirm, expired, floor, in_slot, manifest_version, kind, may_stage, newest, other, raise, trial_of_other, Kind};
use slots::{Record, SEQUENCE_LAST};

fn rec(sequence: u64, slot: u8, fallback: u8, tries: u8, confirmed: bool) -> Record {
    Record { sequence, slot, fallback, tries, confirmed, floor: 0 }
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

#[test]
fn a_manifest_splits_into_boot_files_applications_and_shared_files() {
    let (boot, loader) = (["init.elf", "shell.elf"], "EFI/BOOT/BOOTX64.EFI");
    let kind = |path| kind(path, &boot, loader);
    assert_eq!(kind("kernel.elf"), Kind::Boot);
    assert_eq!(kind("init.elf"), Kind::Boot);
    assert_eq!(kind("SHELL.ELF"), Kind::Boot, "a volume's name in another case");
    assert_eq!(kind("wm.elf"), Kind::Application);
    assert_eq!(kind("UPTIME.ELF"), Kind::Application);
    // 351-UPD-0010: this processor's loader goes into the slot, as BOOTX64.EFI; another processor's stays shared.
    assert_eq!(kind("EFI/BOOT/BOOTX64.EFI"), Kind::Loader);
    assert_eq!(kind("efi/boot/bootx64.efi"), Kind::Loader);
    assert_eq!(in_slot("EFI/BOOT/BOOTX64.EFI"), "BOOTX64.EFI");
    assert_eq!(in_slot("kernel.elf"), "kernel.elf");
    for shared in ["EFI/BOOT/BOOTAA64.EFI", "EFI/BOOT/BOOTX64.EFI.old", "BOOTX64.EFI", "LICENSES/MIT.txt", "data/models/x.elf", "MANIFEST", "elf", ".elf"] {
        assert_eq!(kind(shared), Kind::Shared, "{}", shared);
    }
}

#[test]
fn metadata_is_current_only_before_its_expiry_on_a_clock_that_reads() {
    // 351-UPD-0025: the second of its expiry is past already, and a clock that cannot be read shows nothing current.
    assert!(!expired(1_000, Some(999)));
    assert!(expired(1_000, Some(1_000)));
    assert!(expired(1_000, Some(5_000)));
    assert!(expired(u64::MAX, None));
}

#[test]
fn the_floor_rises_to_a_minimum_the_confirmed_slot_meets_and_never_falls() {
    // 351-UPD-0024: the floor is the channel's minimum, not the version confirmed here, held in the records.
    let at = |floor| [Some(Record { floor, ..rec(4, b'B', b'A', 0, true) }), Some(rec(3, b'B', b'A', 1, false))];
    assert_eq!(raise(at(0), b'B', 5, 7), Some((1, Record { floor: 5, ..rec(5, b'B', b'A', 0, true) })), "the newer record again, one on");
    assert_eq!(raise(at(5), b'B', 5, 7), None, "a confirmed trial alone raises nothing");
    assert_eq!(raise(at(6), b'B', 5, 7), None, "never lowered");
    assert_eq!(raise(at(0), b'B', 7, 6), None, "the running slot below the minimum keeps the floor");
    assert_eq!(raise(at(0), b'A', 5, 7), None, "not for a slot the newer record does not confirm");
    assert_eq!(raise([Some(rec(4, b'B', b'A', 2, false)), None], b'B', 5, 7), None, "not before the running slot is confirmed");
    // With the floor at 5, release 7 confirmed, release 6 stays at or above it: a rollback to it is allowed.
    assert!(raise(at(0), b'B', 5, 7).is_some_and(|(_, r)| 6 >= r.floor));
}

#[test]
fn no_write_lowers_the_floor() {
    // Every record the updater writes carries the higher of the records' floors.
    let records = [Some(Record { floor: 6, ..rec(4, b'A', 0, 0, true) }), Some(Record { floor: 2, ..rec(3, b'A', 0, 0, true) })];
    assert_eq!(floor(records), 6);
    assert_eq!(trial_of_other(records, b'A', 3).map(|(_, r)| r.floor), Some(6));
    let trial = [Some(Record { floor: 6, ..rec(5, b'B', b'A', 2, false) }), Some(Record { floor: 6, ..rec(4, b'A', 0, 0, true) })];
    assert_eq!(confirm(trial, b'B').map(|(_, r)| r.floor), Some(6), "confirming a trial leaves the floor where it was");
    assert_eq!(floor([None, None]), 0);
}

#[test]
fn a_manifests_version_is_its_header_line() {
    assert_eq!(manifest_version(b"MIND-MANIFEST 1\nkey ab\nversion 6\nfile kernel.elf 1 aa 0 0\n"), 6);
    assert_eq!(manifest_version(b"MIND-MANIFEST 1\nkey ab\nfile kernel.elf 1 aa 0 0\n"), 0, "none: version 0");
    assert_eq!(manifest_version(b"MIND-MANIFEST 1\nfile version 9 aa 0 0\nversion 6\n"), 0, "not after a file line");
}

// Bundles (751-UPD-0032): one release.py makes of a published release, and hostile ones Python's zipfile makes, in `dir`.
fn bundles(dir: &std::path::Path) {
    let script = r#"
import sys, zipfile
from pathlib import Path
sys.path.insert(0, "scripts"); sys.path.insert(0, "tests")
import release, release_test
d = Path(sys.argv[1])
v = release_test.volume(d / "volume", b"\x7fELF kernel 1")
(d / "server").mkdir()
release.publish(release.stage(1, d / "staged", {"x86_64": v}), d / "server")
release.bundle(d / "server", d / "bundle.zip")
for name, method, entries in [("deflated", zipfile.ZIP_DEFLATED, [("a", b"x" * 100)]), ("escape", zipfile.ZIP_STORED, [("../a", b"x")]),
                              ("twice", zipfile.ZIP_STORED, [("a", b"x"), ("a", b"y")]), ("absolute", zipfile.ZIP_STORED, [("/a", b"x")])]:
    with zipfile.ZipFile(d / f"{name}.zip", "w", method) as z:
        for path, data in entries:
            z.writestr(path, data)
"#;
    let status = std::process::Command::new("python3").args(["-W", "ignore", "-c", script]).arg(dir).status().expect("python3");
    assert!(status.success());
}

// The entries of a bundle as the updater reads them: names and bytes.
fn read_bundle(data: &[u8]) -> Option<Vec<(String, Vec<u8>)>> {
    let tail = &data[data.len().saturating_sub(bundle::TAIL_MAX)..];
    let (directory, size, count) = bundle::end(tail, data.len() as u64)?;
    let entries = bundle::entries(data.get(directory as usize..directory as usize + size)?, count, directory)?;
    entries.iter().map(|e| {
        let header = data.get(e.local as usize..(e.local as usize + bundle::LOCAL_LEN + e.name.len()).min(data.len()))?;
        let start = bundle::start(header, e, directory)? as usize;
        Some((e.name.clone(), data[start..start + e.size as usize].to_vec()))
    }).collect()
}

#[test]
fn a_bundle_of_release_py_is_read_in_place() {
    let dir = std::env::temp_dir().join(format!("mind-bundle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    bundles(&dir);
    let data = std::fs::read(dir.join("bundle.zip")).unwrap();
    let read = read_bundle(&data).expect("the bundle reads");
    let names: Vec<&str> = read.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(&names[..3], ["channels/stable", "releases/1/x86_64/MANIFEST", "releases/1/x86_64/MANIFEST.SIG"]);
    assert!(names.len() > 3 && names[3..].iter().all(|n| n.starts_with("blobs/")), "{names:?}");
    for (name, bytes) in &read { assert_eq!(bytes, &std::fs::read(dir.join("server").join(name)).unwrap(), "{name}"); }
    // A byte past the end record, a count one more, a compressed entry, one of a name changed in its local header: refused.
    let mut longer = data.clone();
    longer.push(0);
    assert!(read_bundle(&longer).is_none());
    let end = data.len() - 22;
    let mut more = data.clone();
    for at in [end + 8, end + 10] { more[at] += 1; }
    assert!(read_bundle(&more).is_none());
    let central = u32::from_le_bytes(data[end + 16..end + 20].try_into().unwrap()) as usize;
    let mut packed = data.clone();
    packed[central + 10] = 8;
    assert!(read_bundle(&packed).is_none());
    let mut renamed = data.clone();
    renamed[30] ^= 1;
    assert!(read_bundle(&renamed).is_none());
    let mut sealed = data.clone();
    sealed[central + 8] |= 1;
    assert!(read_bundle(&sealed).is_none());
    // Python's own archives: deflated, a name that leaves the tree, one met twice, an absolute one.
    for name in ["deflated", "escape", "twice", "absolute"] {
        assert!(read_bundle(&std::fs::read(dir.join(format!("{name}.zip"))).unwrap()).is_none(), "{name}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
