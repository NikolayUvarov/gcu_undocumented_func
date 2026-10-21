//! The bounded ZIP reader (750-EFF-0005) on archives made by zip, 7-Zip and Python (tests/data/zip/make.sh), and on
//! hostile ones: a bomb, names that leave the directory or repeat, an encrypted entry, a wrong CRC, cut and lying data.
#![allow(dead_code)]
extern crate alloc;
#[path = "../libmind/src/inflate.rs"] mod inflate;
#[path = "../libmind/src/zip.rs"] mod zip;
use zip::{Archive, Error, Limits};

fn data(name: &str) -> Vec<u8> { std::fs::read(format!("tests/data/zip/{name}")).unwrap() }

fn read_all(a: &Archive<'_>, name: &str) -> Result<Vec<u8>, Error> {
    let e = a.find(name).expect("entry");
    let mut out = Vec::new();
    a.read(&e, &mut |p| { out.extend_from_slice(p); true })?;
    Ok(out)
}

#[test]
fn archives_made_by_zip_and_7zip_read_back() {
    let elf = data("zip-deflate.zip");
    let a = Archive::open(&elf, Limits::default()).unwrap();
    let from_zip = read_all(&a, "mindcore/x64/demo.elf").unwrap();
    assert_eq!(from_zip.len(), 20000);
    assert!(read_all(&a, "manifest.json").unwrap().starts_with(b"{\"manifest_version\":1"));
    let seven = data("7z-deflate.zip");
    let b = Archive::open(&seven, Limits::default()).unwrap();
    assert_eq!(read_all(&b, "mindcore/x64/demo.elf").unwrap(), from_zip, "the same file from either tool");
    assert_eq!(b.find("mindcore/x64/demo.elf").unwrap().method, 8, "7-Zip deflates the large file");
}

#[test]
fn stored_entries_and_directories() {
    let bytes = data("stored.zip");
    let a = Archive::open(&bytes, Limits::default()).unwrap();
    let names: Vec<_> = a.entries().map(|e| (e.name.to_owned(), e.is_dir(), e.method)).collect();
    assert_eq!(names, [("manifest.json".to_owned(), false, 0), ("mindcore/".to_owned(), true, 0), ("mindcore/x64/demo.elf".to_owned(), false, 0)]);
    assert_eq!(read_all(&a, "mindcore/x64/demo.elf").unwrap()[..4], *b"\x7fELF");
    assert_eq!(read_all(&a, "mindcore/").unwrap(), b"");
}

#[test]
fn hostile_names_are_refused() {
    for f in ["traversal.zip", "absolute.zip", "backslash.zip", "drive.zip", "dot.zip", "duplicate.zip"] {
        assert_eq!(Archive::open(&data(f), Limits::default()).err(), Some(Error::Name), "{f}");
    }
    for ok in ["a", "a/b.txt", "dir/", "a/b/c"] { assert!(zip::name_ok(ok, 255), "{ok}"); }
    for bad in ["", "/", "../a", "a/..", "a//b", "./a", "a\\b", "c:x", "a\u{1}b", "a/b//"] { assert!(!zip::name_ok(bad, 255), "{bad:?}"); }
    assert!(!zip::name_ok(&"x".repeat(256), 255));
}

#[test]
fn a_bomb_an_encrypted_entry_and_limits() {
    assert_eq!(Archive::open(&data("bomb.zip"), Limits::default()).err(), Some(Error::Limit), "8 MiB from 8 KiB passes the ratio");
    assert_eq!(Archive::open(&data("encrypted.zip"), Limits::default()).err(), Some(Error::Encrypted));
    let bytes = data("zip-deflate.zip");
    assert_eq!(Archive::open(&bytes, Limits { entries: 2, ..Limits::default() }).err(), Some(Error::Limit), "four entries, two allowed");
    assert_eq!(Archive::open(&bytes, Limits { entry: 19999, ..Limits::default() }).err(), Some(Error::Limit));
    assert_eq!(Archive::open(&bytes, Limits { total: 1000, ..Limits::default() }).err(), Some(Error::Limit));
    assert_eq!(Archive::open(&bytes, Limits { name: 8, ..Limits::default() }).err(), Some(Error::Limit));
}

#[test]
fn a_wrong_crc_and_a_lying_size_are_corrupt() {
    let bytes = data("crc.zip");
    let a = Archive::open(&bytes, Limits::default()).unwrap();
    assert_eq!(read_all(&a, "data.bin").err(), Some(Error::Corrupt));
    // The directory says the deflated file is one byte shorter than it is: more output than announced is refused.
    let mut lying = data("zip-deflate.zip");
    let size = Archive::open(&lying, Limits::default()).unwrap().find("mindcore/x64/demo.elf").unwrap().size;
    let central = find_central(&lying, "mindcore/x64/demo.elf");
    lying[central + 24..central + 28].copy_from_slice(&((size - 1) as u32).to_le_bytes());
    let a = Archive::open(&lying, Limits::default()).unwrap();
    assert_eq!(read_all(&a, "mindcore/x64/demo.elf").err(), Some(Error::Corrupt));
}

#[test]
fn a_sink_that_refuses_stops_the_read() {
    let bytes = data("zip-deflate.zip");
    let a = Archive::open(&bytes, Limits::default()).unwrap();
    let e = a.find("mindcore/x64/demo.elf").unwrap();
    assert_eq!(a.read(&e, &mut |_| false), Err(Error::Sink));
}

#[test]
fn cut_and_inconsistent_archives_are_malformed() {
    let bytes = data("stored.zip");
    for cut in [0, 10, bytes.len() / 2, bytes.len() - 1] {
        assert!(Archive::open(&bytes[..cut], Limits::default()).is_err(), "cut at {cut}");
    }
    assert_eq!(Archive::open(b"not a zip at all, just text", Limits::default()).err(), Some(Error::Malformed));
    // ZIP64 or several disks: the end record says so.
    let eocd = bytes.len() - 22;
    let mut zip64 = bytes.clone(); zip64[eocd + 10..eocd + 12].copy_from_slice(&0xFFFFu16.to_le_bytes());
    assert_eq!(Archive::open(&zip64, Limits::default()).err(), Some(Error::Unsupported));
    let mut disks = bytes.clone(); disks[eocd + 4..eocd + 6].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(Archive::open(&disks, Limits::default()).err(), Some(Error::Unsupported));
    // A local header that names another file than the directory does.
    let mut renamed = bytes.clone();
    let local = renamed.windows(13).position(|w| w == b"manifest.json").unwrap();
    renamed[local] = b'M';
    let a = Archive::open(&renamed, Limits::default()).unwrap();
    assert_eq!(read_all(&a, "manifest.json").err(), Some(Error::Malformed));
    // An unknown method (here 99).
    let mut method = bytes.clone();
    let central = find_central(&method, "manifest.json");
    method[central + 10..central + 12].copy_from_slice(&99u16.to_le_bytes());
    assert_eq!(Archive::open(&method, Limits::default()).err(), Some(Error::Unsupported));
}

#[test]
fn crc32_matches_the_standard_check_value() {
    assert_eq!(zip::crc32_update(0, b"123456789"), 0xCBF4_3926);
    assert_eq!(zip::crc32_update(zip::crc32_update(0, b"12345"), b"6789"), 0xCBF4_3926, "in pieces");
}

/// The central directory record of `name`.
fn find_central(bytes: &[u8], name: &str) -> usize {
    let mut at = 0;
    loop {
        let i = at + bytes[at..].windows(4).position(|w| w == [0x50, 0x4b, 0x01, 0x02]).unwrap();
        let len = u16::from_le_bytes([bytes[i + 28], bytes[i + 29]]) as usize;
        if &bytes[i + 46..i + 46 + len] == name.as_bytes() { return i; }
        at = i + 4;
    }
}

/// Seeded fuzzing (for 500's harness): mutated archives never panic, and what reads back keeps the limits.
#[test]
fn mutated_archives_never_panic() {
    let seed: u64 = std::env::var("MIND_FUZZ_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(750_0005);
    let rounds: usize = std::env::var("MIND_FUZZ_ITERATIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(4000);
    let mut state = seed | 1;
    let mut next = move || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; state };
    let limits = Limits { entries: 16, name: 64, entry: 1 << 20, total: 2 << 20, ratio: 200 };
    let originals = [data("stored.zip"), data("zip-deflate.zip"), data("7z-deflate.zip")];
    let (mut opened, mut read) = (0, 0);
    for i in 0..rounds {
        let mut bytes = originals[i % originals.len()].clone();
        for _ in 0..(1 + next() % 8) {
            let at = (next() as usize) % bytes.len();
            match next() % 4 { 0 => bytes[at] ^= 1 << (next() % 8), 1 => bytes[at] = next() as u8, 2 => bytes.truncate(at.max(1)), _ => bytes.insert(at, next() as u8) }
        }
        if let Ok(a) = Archive::open(&bytes, limits) {
            opened += 1;
            for e in a.entries() {
                let mut got = 0u64;
                if a.read(&e, &mut |p| { got += p.len() as u64; true }).is_ok() { read += 1; assert_eq!(got, e.size); }
                assert!(got <= limits.entry);
            }
        }
    }
    assert!(opened > 0 && read > 0, "the fuzzer reached the readers ({opened} opened, {read} read)");
}
