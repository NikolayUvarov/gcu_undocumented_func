//! Host tests of the boot records the updater writes (updater/src/plan.rs, 351-UPD-0007; MC-9.3): each goes into the
//! file that does not hold the newer valid record, and the bootloader (bootloader/src/slots.rs) follows it as meant.
//! The releases it keeps in the block store (updater/src/kept.rs, 351-UPD-0033): which ones stay, the tree of each, read
//! back by the layout's paths with every block checked, and a tree out of shape or damaged refused.
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

#[allow(dead_code)]
#[path = "../libmind/src/sha256.rs"]
mod sha256;
#[allow(dead_code)]
#[path = "../libmind/src/cid.rs"]
mod cid;
#[allow(dead_code)]
#[path = "../libmind/src/dag.rs"]
mod dag;
#[allow(dead_code)]
#[path = "../libmind/src/tree.rs"]
mod tree;
#[path = "../updater/src/kept.rs"]
mod kept;

use plan::{confirm, expired, floor, in_slot, keeps, last_good, manifest_version, kind, may_stage, newest, other, period, raise, trial_of_other, Kind};
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
    // 351-UPD-0036: the release's period goes into the slot; the root's update.txt stays the root's.
    assert_eq!(kind("period.txt"), Kind::Period);
    assert_eq!(kind("PERIOD.TXT"), Kind::Period);
    assert_eq!(in_slot("period.txt"), "period.txt");
    for shared in ["EFI/BOOT/BOOTAA64.EFI", "EFI/BOOT/BOOTX64.EFI.old", "BOOTX64.EFI", "LICENSES/MIT.txt", "data/models/x.elf", "MANIFEST", "elf", ".elf",
                   "update.txt", "data/period.txt", "period.txt.old"] {
        assert_eq!(kind(shared), Kind::Shared, "{}", shared);
    }
}

#[test]
fn a_slots_period_holds_every_alone() {
    // 351-UPD-0036: one `every SECONDS`, with comments and blank lines around it.
    assert_eq!(period("every 60\n"), Ok(60));
    assert_eq!(period("# the maintainer's minute\n\n  every   7  # seconds\r\n"), Ok(7));
    assert_eq!(period("every 0"), Ok(0), "0: once at start, as update.txt's");
    // What is the root's alone, a second period, a bad number or none: refused, with the line or why.
    assert_eq!(period("every 60\nsource https://elsewhere.example\n"), Err("source https://elsewhere.example"));
    assert_eq!(period("pin 00\nevery 60\n"), Err("pin 00"));
    for line in ["automatic apply", "channel stable", "tries 3"] { assert_eq!(period(&format!("every 9\n{}\n", line)), Err(line)); }
    assert_eq!(period("every 60\nevery 30\n"), Err("every 30"));
    for bad in ["every", "every -1", "every 1.5", "every 60 s", "every 99999999999999999999", "EVERY 60"] {
        assert_eq!(period(bad), Err(bad.trim()), "{}", bad);
    }
    assert_eq!(period(""), Err("NO `every` LINE"));
    assert_eq!(period("# nothing\n\n"), Err("NO `every` LINE"));
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
    // The key file's history first (351-UPD-0027), then the key file, the channel and the manifest.
    assert_eq!(&names[..5], ["keys/1", "keys/current", "channels/stable", "releases/1/x86_64/MANIFEST", "releases/1/x86_64/MANIFEST.SIG"]);
    assert!(names.len() > 5 && names[5..].iter().all(|n| n.starts_with("blobs/")), "{names:?}");
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

#[test]
fn the_store_keeps_the_running_the_last_known_good_and_a_staged_release() {
    // The last known good: the newest below the running one that the minimum allows.
    assert_eq!(last_good(&[3, 4, 5, 6, 7], 6, 1), Some(5));
    assert_eq!(last_good(&[3, 4, 6], 6, 4), Some(4));
    assert_eq!(last_good(&[5, 6], 6, 6), None, "a release below the minimum boots no more");
    assert_eq!(last_good(&[6, 7], 6, 1), None);
    assert_eq!(last_good(&[5, 6], 0, 1), None, "the running version not known");
    let versions = [3, 4, 5, 6, 7];
    let kept = |running, minimum, staged| versions.iter().copied().filter(|&v| keeps(v, &versions, running, minimum, staged)).collect::<Vec<_>>();
    assert_eq!(kept(6, 1, 7), [5, 6, 7]);
    assert_eq!(kept(6, 1, 0), [5, 6], "a release above the running one that is not staged goes");
    assert_eq!(kept(6, 6, 7), [6, 7], "the minimum raised past the last known good");
    assert_eq!(kept(3, 1, 0), [3]);
    assert_eq!(kept(0, 1, 0), versions, "all, while the running version is not known");
    // After a confirmed update from 5 to 6, both stay; after the next, 7 and 6.
    assert!(keeps(5, &[5, 6], 6, 5, 0) && keeps(6, &[5, 6], 6, 5, 0));
    assert_eq!([5, 6, 7].iter().copied().filter(|&v| keeps(v, &[5, 6, 7], 7, 5, 0)).collect::<Vec<_>>(), [6, 7]);
}

// Blocks in memory, by CID, each checked by the reader as the store's are.
#[derive(Default)]
struct Blocks { blocks: std::collections::HashMap<cid::Cid, Vec<u8>> }

impl dag::Blocks for Blocks {
    fn put(&mut self, codec: cid::Codec, data: &[u8]) -> Result<cid::Cid, dag::Error> {
        let cid = cid::Cid::of(codec, data);
        self.blocks.entry(cid).or_insert_with(|| data.to_vec());
        Ok(cid)
    }
    fn get(&mut self, cid: &cid::Cid, out: &mut [u8]) -> Result<usize, dag::Error> {
        let data = self.blocks.get(cid).ok_or(dag::Error::NotFound)?;
        out[..data.len()].copy_from_slice(data);
        Ok(data.len())
    }
    fn has(&mut self, cid: &cid::Cid) -> Result<bool, dag::Error> { Ok(self.blocks.contains_key(cid)) }
}

fn bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    (0..len).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x as u8 }).collect()
}

fn hex(digest: &[u8; 32]) -> String { digest.iter().map(|b| format!("{:02x}", b)).collect() }

// A file put as an object, as the updater puts one: its blob and its bytes.
fn blob(store: &mut Blocks, builder: &mut dag::Builder, data: &[u8]) -> kept::Blob {
    builder.reset();
    builder.write(store, data).unwrap();
    let (cid, size) = builder.finish(store).unwrap();
    kept::Blob { digest: sha256::digest(data), size, cid }
}

// The object `cid` of `size` bytes read whole, every block checked.
fn read(store: &mut Blocks, cid: &cid::Cid, size: u64) -> Vec<u8> {
    let (mut out, mut buffer) = (vec![0u8; size as usize], Box::new([0u8; dag::CHUNK]));
    let mut at = 0;
    while at < out.len() { at += dag::read_at(store, cid, at as u64, &mut out[at..], &mut buffer).unwrap(); }
    out
}

fn release<'a>(version: u64, channel_file: &'a [u8], manifest: &'a [u8]) -> kept::Release<'a> {
    kept::Release { keys_file: b"{\"keys\": 1}\n", channel: "stable", channel_file, version, arch: "x86_64", manifest, signature: &[7u8; 64] }
}

#[test]
fn a_kept_release_is_the_release_layout_read_back_by_its_paths() {
    let (mut store, mut builder, mut buffer) = (Blocks::default(), Box::new(dag::Builder::new()), Box::new([0u8; dag::CHUNK]));
    let files: Vec<Vec<u8>> = [0, 1, dag::CHUNK, 3 * dag::CHUNK + 5, 300_000].iter().enumerate().map(|(k, &len)| bytes(k as u64, len)).collect();
    let mut blobs: Vec<kept::Blob> = files.iter().map(|f| blob(&mut store, &mut builder, f)).collect();
    // A digest given twice is one blob.
    blobs.push(blobs[2]);
    let (channel, manifest) = (b"{\"name\": \"stable\"}\ned25519 00\n".to_vec(), bytes(99, 20_000));
    let root = kept::put(&mut store, &mut builder, &release(7, &channel, &manifest), &blobs, &mut buffer).unwrap();
    assert_eq!(kept::version(&mut store, &root, "x86_64", &mut buffer), Ok(7));
    assert_eq!(kept::version(&mut store, &root, "aarch64", &mut buffer), Err(dag::Error::Shape), "no manifest for another processor");
    assert_eq!(kept::channel(&mut store, &root, &mut buffer).as_deref(), Ok("stable"));
    for (path, want) in [("keys/current", &b"{\"keys\": 1}\n"[..]), ("channels/stable", &channel[..]), ("releases/7/x86_64/MANIFEST", &manifest[..]),
                         ("releases/7/x86_64/MANIFEST.SIG", &[7u8; 64][..])] {
        let (cid, size) = kept::find(&mut store, &root, path, &mut buffer).unwrap().unwrap();
        assert_eq!(read(&mut store, &cid, size), want, "{}", path);
    }
    for (data, b) in files.iter().zip(&blobs) {
        let path = format!("blobs/{}", hex(&b.digest));
        assert_eq!(kept::stored(&path), format!("blobs/{}/{}", &hex(&b.digest)[..2], hex(&b.digest)));
        let (cid, size) = kept::find(&mut store, &root, &path, &mut buffer).unwrap().unwrap();
        assert_eq!((cid, size), (b.cid, data.len() as u64));
        assert_eq!(&read(&mut store, &cid, size), data);
    }
    // What the layout does not hold, or a directory where a file is asked for.
    for path in [format!("blobs/{}", hex(&[0xab; 32])), String::from("blobs"), String::from("releases/7"), String::from("channels/beta"), String::from("keys/other"), String::from("releases/7/x86_64/MANIFEST/x")] {
        assert_eq!(kept::find(&mut store, &root, &path, &mut buffer), Ok(None), "{}", path);
    }
    let mut listed = Vec::new();
    let mut left = 100;
    kept::blobs(&mut store, &root, &mut buffer, &mut left, |b| listed.push(b)).unwrap();
    listed.sort_by_key(|b| b.digest);
    let mut want = blobs[..5].to_vec();
    want.sort_by_key(|b| b.digest);
    assert_eq!(listed, want);
    // The steps are bounded: fewer than the entries is TooLarge.
    let mut left = 3;
    assert_eq!(kept::blobs(&mut store, &root, &mut buffer, &mut left, |_| {}), Err(dag::Error::TooLarge));
}

#[test]
fn two_releases_share_their_blobs_and_hundreds_fit_under_their_prefixes() {
    let (mut store, mut builder, mut buffer) = (Blocks::default(), Box::new(dag::Builder::new()), Box::new([0u8; dag::CHUNK]));
    // More blobs than one directory holds: 400 entries of 64 hex digits are over a block.
    let files: Vec<Vec<u8>> = (0..400).map(|k| bytes(1000 + k, 100 + k as usize)).collect();
    let blobs: Vec<kept::Blob> = files.iter().map(|f| blob(&mut store, &mut builder, f)).collect();
    let flat: Vec<(String, kept::Blob)> = { let mut f: Vec<_> = blobs.iter().map(|b| (hex(&b.digest), *b)).collect(); f.sort_by(|a, b| a.0.cmp(&b.0)); f };
    let entries: Vec<tree::Entry> = flat.iter().map(|(n, b)| tree::Entry { name: n.as_bytes(), kind: tree::Kind::File, size: b.size, cid: b.cid }).collect();
    assert_eq!(tree::encode(&entries, &mut buffer), Err(dag::Error::TooLarge), "one flat directory would not hold them");
    let five = kept::put(&mut store, &mut builder, &release(5, b"five", b"manifest 5"), &blobs, &mut buffer).unwrap();
    let held = store.blocks.len();
    // Release 6 changes one file: the others are the same objects, and its tree adds only what differs.
    let mut six_blobs = blobs.clone();
    six_blobs[17] = blob(&mut store, &mut builder, b"release 6's file");
    let six = kept::put(&mut store, &mut builder, &release(6, b"six", b"manifest 6"), &six_blobs, &mut buffer).unwrap();
    assert!(store.blocks.len() - held < 12, "release 6 added {} blocks", store.blocks.len() - held);
    for (root, version, set) in [(five, 5, &blobs), (six, 6, &six_blobs)] {
        assert_eq!(kept::version(&mut store, &root, "x86_64", &mut buffer), Ok(version));
        let mut left = 10_000;
        let mut listed = 0;
        kept::blobs(&mut store, &root, &mut buffer, &mut left, |b| { assert!(set.contains(&b)); listed += 1; }).unwrap();
        assert_eq!(listed, 400);
        for b in set.iter() {
            assert_eq!(kept::find(&mut store, &root, &format!("blobs/{}", hex(&b.digest)), &mut buffer), Ok(Some((b.cid, b.size))));
        }
    }
}

// A directory of `entries` (name, kind, size, cid) put as given.
fn dir(store: &mut Blocks, entries: &[(&str, tree::Kind, u64, cid::Cid)]) -> cid::Cid {
    let entries: Vec<tree::Entry> = entries.iter().map(|&(n, kind, size, cid)| tree::Entry { name: n.as_bytes(), kind, size, cid }).collect();
    let mut buffer = Box::new([0u8; dag::CHUNK]);
    let len = tree::encode(&entries, &mut buffer).unwrap();
    dag::Blocks::put(store, cid::Codec::DagCbor, &buffer[..len]).unwrap()
}

#[test]
fn a_kept_tree_out_of_shape_or_damaged_is_refused() {
    use tree::Kind::{Directory, File};
    let (mut store, mut builder, mut buffer) = (Blocks::default(), Box::new(dag::Builder::new()), Box::new([0u8; dag::CHUNK]));
    let one = blob(&mut store, &mut builder, b"one");
    let root = kept::put(&mut store, &mut builder, &release(5, b"c", b"m"), &[one], &mut buffer).unwrap();
    let (blobs, channels, releases) = {
        let top = tree::decode(&store.blocks[&root]).unwrap();
        (top.find(b"blobs").unwrap().side(), top.find(b"channels").unwrap().side(), top.find(b"releases").unwrap().side())
    };
    let numbered = tree::decode(&store.blocks[&releases.cid]).unwrap().entry(0).cid;
    // Two versions under releases/: no release is told.
    let two = dir(&mut store, &[("5", Directory, 1, numbered), ("6", Directory, 1, numbered)]);
    let bad = dir(&mut store, &[("blobs", Directory, blobs.size, blobs.cid), ("channels", Directory, 1, channels.cid), ("releases", Directory, 2, two)]);
    assert_eq!(kept::version(&mut store, &bad, "x86_64", &mut buffer), Err(dag::Error::Shape));
    // A version that is not a plain number.
    for name in ["05", "0", "x"] {
        let other = dir(&mut store, &[(name, Directory, 1, numbered)]);
        let bad = dir(&mut store, &[("blobs", Directory, blobs.size, blobs.cid), ("channels", Directory, 1, channels.cid), ("releases", Directory, 1, other)]);
        assert_eq!(kept::version(&mut store, &bad, "x86_64", &mut buffer), Err(dag::Error::Shape), "{}", name);
    }
    // A count that is not the directory's: refused on the way down.
    let bad = dir(&mut store, &[("blobs", Directory, blobs.size + 1, blobs.cid), ("channels", Directory, 1, channels.cid), ("releases", Directory, 1, releases.cid)]);
    let path = format!("blobs/{}", hex(&one.digest));
    assert_eq!(kept::find(&mut store, &bad, &path, &mut buffer), Err(dag::Error::Shape));
    assert_eq!(kept::blobs(&mut store, &bad, &mut buffer, &mut 100, |_| {}), Err(dag::Error::Shape));
    // A blob under a prefix its name does not start with is not the release's.
    let name = hex(&one.digest);
    let wrong = if &name[..2] == "00" { "01" } else { "00" };
    let group = dir(&mut store, &[(name.as_str(), File, one.size, one.cid)]);
    let moved = dir(&mut store, &[(wrong, Directory, 1, group)]);
    let bad = dir(&mut store, &[("blobs", Directory, 1, moved), ("channels", Directory, 1, channels.cid), ("releases", Directory, 1, releases.cid)]);
    assert_eq!(kept::blobs(&mut store, &bad, &mut buffer, &mut 100, |_| {}), Err(dag::Error::Shape));
    assert_eq!(kept::find(&mut store, &bad, &path, &mut buffer), Ok(None));
    // A block of the tree damaged in the store: told, never read as the tree.
    let group = tree::decode(&store.blocks[&blobs.cid]).unwrap().entry(0).cid;
    store.blocks.get_mut(&group).unwrap()[3] ^= 1;
    assert_eq!(kept::find(&mut store, &root, &path, &mut buffer), Err(dag::Error::Corrupt));
    assert_eq!(kept::blobs(&mut store, &root, &mut buffer, &mut 100, |_| {}), Err(dag::Error::Corrupt));
    assert_eq!(kept::version(&mut store, &root, "x86_64", &mut buffer), Ok(5), "the rest of the tree reads");
    store.blocks.get_mut(&root).unwrap()[5] ^= 1;
    assert_eq!(kept::version(&mut store, &root, "x86_64", &mut buffer), Err(dag::Error::Corrupt));
    // A file's block where the root belongs.
    assert_eq!(kept::version(&mut store, &one.cid, "x86_64", &mut buffer), Err(dag::Error::Shape));
}
