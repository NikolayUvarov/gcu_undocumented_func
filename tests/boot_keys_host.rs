//! Host tests of the bootloader's key file check (bootloader/src/keychain.rs, 351-UPD-0029): a volume's `KEYS` taken
//! under the root set built in, through the files of its root changes (`KEYS.1` on), and refused when the root before
//! it or its own did not sign it to their thresholds, when the versions do not rise, or when it is below the version
//! the boot records name. The files are made by scripts/release.py; signatures are checked by sign_manifest.py.
#![allow(dead_code)]
#[path = "../libmind/src/release.rs"]
mod release;
#[path = "../bootloader/src/keychain.rs"]
mod keychain;

use keychain::{built_in, check, Checked, Refusal};
use std::collections::HashMap;
use std::process::Command;

fn python(code: &str) -> Vec<u8> {
    let out = Command::new("python3").args(["-c", &format!("import sys; sys.path.insert(0, 'scripts')\n{}", code)]).output().expect("python3");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{:02x}", b)).collect() }

// Key file `version` as `release.py keys` makes one: its root the seeds `root` with `threshold`, signed by `signers`
// (Python expressions; OLD is the test root set, NEW a second set of two keys).
fn version_file(version: u64, root: &str, threshold: u8, signers: &str) -> Vec<u8> {
    python(&format!("import release, datetime, hashlib\nnow = datetime.datetime(2026, 11, 13, tzinfo=datetime.timezone.utc)\n\
                     OLD = release.TEST_ROOT_SEEDS\nNEW = [hashlib.sha256(f'another root {{i}}'.encode()).digest() for i in (1, 2)]\n\
                     whole = release.test_keys_file(now, version={version}, roots=[{signers}], root=([{root}], {threshold}))\n\
                     sys.stdout.buffer.write(whole)"))
}

fn verify(key: &[u8; 32], body: &[u8], signature: &[u8; 64]) -> bool {
    let code = format!("import sign_manifest as sm\nprint(sm.verify(bytes.fromhex({:?}), bytes.fromhex({:?}), bytes.fromhex({:?})))", hex(key), hex(body), hex(signature));
    python(&code) == b"True\n"
}

// The test root set's public keys and threshold, as the bootloader's build reads them.
fn root_set() -> release::Role {
    let text = String::from_utf8(python("import release\nk, t = release.root_set()\nprint(t, *[x.hex() for x in k])")).unwrap();
    let mut words = text.split_whitespace();
    let threshold = words.next().unwrap().parse().unwrap();
    let keys: Vec<[u8; 32]> = words.map(|w| (0..32).map(|i| u8::from_str_radix(&w[2 * i..2 * i + 2], 16).unwrap()).collect::<Vec<_>>().try_into().unwrap()).collect();
    built_in(&keys, threshold)
}

// The check of a directory holding `files`, the records naming `held`.
fn checked(files: &[(&str, Vec<u8>)], held: u64) -> Result<Checked, Refusal> {
    let files: HashMap<&str, &Vec<u8>> = files.iter().map(|(name, data)| (*name, data)).collect();
    check(root_set(), held, |name, buffer| {
        let data = files.get(name).ok_or("file not found")?;
        if data.len() >= buffer.len() { return Err("file larger than its buffer"); }
        buffer[..data.len()].copy_from_slice(data);
        Ok(data.len())
    }, |file| release::parse_keys(file).ok(), verify)
}

const OLD: (&str, u8) = ("*OLD", 2);
const NEW: (&str, u8) = ("*NEW", 2);
const TWO_OLD: &str = "OLD[0], OLD[1]";

#[test]
fn a_key_file_the_root_signed_is_taken_and_names_the_boot_key() {
    let got = checked(&[("KEYS", version_file(1, OLD.0, OLD.1, TWO_OLD))], 0).unwrap();
    assert_eq!((got.keys.version, got.rotations), (1, 0));
    let boot = python("import sign_manifest as sm\nsys.stdout.buffer.write(sm.public_key(sm.TEST_SEED))");
    assert_eq!((got.keys.roles[0].keys(), got.keys.roles[0].threshold), (&[<[u8; 32]>::try_from(&boot[..]).unwrap()][..], 1));
    // A later version under the same root needs none of the files before it.
    assert_eq!(checked(&[("KEYS", version_file(4, OLD.0, OLD.1, TWO_OLD))], 3).unwrap().keys.version, 4);
}

#[test]
fn a_key_file_short_of_the_root_threshold_is_refused() {
    assert_eq!(checked(&[("KEYS", version_file(1, OLD.0, OLD.1, "OLD[0]"))], 0), Err(Refusal::Root("KEYS", 0)));
    assert_eq!(checked(&[("KEYS", version_file(1, OLD.0, OLD.1, "OLD[0], OLD[0], *NEW"))], 0), Err(Refusal::Root("KEYS", 0)), "one key twice, others unknown");
    assert_eq!(checked(&[], 0), Err(Refusal::Read("KEYS", "file not found")));
    let mut broken = version_file(1, OLD.0, OLD.1, TWO_OLD);
    broken[3] ^= 1;
    assert_eq!(checked(&[("KEYS", broken)], 0), Err(Refusal::Malformed("KEYS")));
    assert_eq!(checked(&[("KEYS", vec![b'{'; 5000])], 0), Err(Refusal::Read("KEYS", "file larger than its buffer")));
}

#[test]
fn a_key_file_below_the_records_version_is_refused() {
    assert_eq!(checked(&[("KEYS", version_file(1, OLD.0, OLD.1, TWO_OLD))], 2), Err(Refusal::Below(1, 2)));
    assert_eq!(checked(&[("KEYS", version_file(2, OLD.0, OLD.1, TWO_OLD))], 2).unwrap().keys.version, 2);
}

#[test]
fn a_root_change_is_followed_through_its_file() {
    // Version 2 rotates the root to NEW: signed by the old root and the new; version 3 by the new alone.
    let rotation = version_file(2, NEW.0, NEW.1, "OLD[0], OLD[1], *NEW");
    let three = version_file(3, NEW.0, NEW.1, "*NEW");
    assert_eq!(checked(&[("KEYS", three.clone())], 0), Err(Refusal::Root("KEYS", 0)), "without the root change");
    let got = checked(&[("KEYS.1", rotation.clone()), ("KEYS", three.clone())], 3).unwrap();
    assert_eq!((got.keys.version, got.rotations), (3, 1));
    // The change itself as KEYS: its own new root signed it.
    assert_eq!(checked(&[("KEYS", rotation.clone())], 0).unwrap().keys.version, 2);
    // Not signed by its own root, the versions not rising, the old root after the change: each refused.
    let unsigned = version_file(2, NEW.0, NEW.1, TWO_OLD);
    assert_eq!(checked(&[("KEYS.1", unsigned), ("KEYS", three.clone())], 0), Err(Refusal::Own("KEYS.1")));
    assert_eq!(checked(&[("KEYS.1", rotation.clone()), ("KEYS", rotation.clone())], 0), Err(Refusal::Order("KEYS", 2, 2)));
    let old_again = version_file(3, OLD.0, OLD.1, TWO_OLD);
    assert_eq!(checked(&[("KEYS.1", rotation.clone()), ("KEYS", old_again)], 0), Err(Refusal::Root("KEYS", 2)));
    // A file after a gap in the names is not read: KEYS.2 without KEYS.1.
    assert_eq!(checked(&[("KEYS.2", rotation), ("KEYS", three)], 0), Err(Refusal::Root("KEYS", 0)));
}
