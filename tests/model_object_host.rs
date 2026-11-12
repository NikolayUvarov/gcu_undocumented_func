//! Host tests of libmind/src/model_object.rs (issues 251-STO-0014, 0015; MC-4.2): a model as one object of the block
//! store, through the store's own logic on a simulated medium. The objects are written here as `blocks models import`
//! writes them, from the format: a header, then the files' bytes in its order. A file is found by its path and read
//! whole, over several chunks too, against its SHA-256; one that differs, an object that ends early, a header not of the
//! format and a path it does not name are refused.
#![allow(dead_code)]
#[path = "../libmind/src/sha256.rs"]
mod sha256;
#[path = "../libmind/src/cid.rs"]
mod cid;
#[path = "../libmind/src/dag.rs"]
mod dag;
#[path = "../libmind/src/model_object.rs"]
mod model_object;
#[path = "../blockstore/src/store.rs"]
mod store;
#[path = "../libmind/src/tree.rs"]
mod tree;
#[path = "../libmind/src/journal.rs"]
mod journal;

use cid::{Cid, Codec};
use dag::{Blocks, Builder, CHUNK};
use model_object::{find, read, Error, Located, HEADER_MAX};
use std::collections::HashMap;
use store::{hmac_sha256, Crypto, Device, Extent, Head, Pin, Store, BUFFER, SECTOR};

const SECTORS: usize = 16384;

struct Memory { data: Vec<u8> }
impl Device for &mut Memory {
    fn sectors(&self) -> u64 { (self.data.len() / SECTOR) as u64 }
    fn writable(&self) -> bool { true }
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool { let at = lba as usize * SECTOR; out.copy_from_slice(&self.data[at..at + out.len()]); true }
    fn write(&mut self, lba: u64, data: &[u8]) -> bool { let at = lba as usize * SECTOR; self.data[at..at + data.len()].copy_from_slice(data); true }
    fn flush(&mut self) -> bool { true }
}

/// Records sealed by a tag alone: the format needs the store's logic, not its cipher.
struct Tagged;
static TAGGED: Tagged = Tagged;
impl Crypto for Tagged {
    fn seal(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8]) -> [u8; 16] { hmac_sha256(key, &[nonce, aad, data])[..16].try_into().unwrap() }
    fn open(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8], tag: &[u8; 16]) -> bool { hmac_sha256(key, &[nonce, aad, data])[..16] == *tag }
    fn random(&self, _: &mut [u8]) -> bool { false }
}

struct Room { index: Vec<store::Entry>, heads: Vec<Head>, pins: Vec<Pin>, holes: Vec<Extent>, buffer: Box<[u8; BUFFER]>, scratch: Box<[u8; CHUNK]> }

fn with_store(test: impl FnOnce(&mut Store<&mut Memory>)) {
    let mut medium = Memory { data: vec![0; SECTORS * SECTOR] };
    let mut room = Room { index: vec![store::Entry::EMPTY; store::slots_for(SECTORS as u64)], heads: vec![Head::EMPTY; 8], pins: vec![Pin::EMPTY; 4], holes: vec![Extent::EMPTY; 64], buffer: Box::new([0; BUFFER]), scratch: Box::new([0; CHUNK]) };
    let mut store = Store::mount(&mut medium, &TAGGED, None, &mut room.index, &mut room.heads, &mut room.pins, &mut room.holes, &mut room.buffer, &mut room.scratch, 0).unwrap();
    test(&mut store)
}

fn buffer() -> Box<[u8; CHUNK]> { Box::new([0; CHUNK]) }
fn hex(d: &[u8]) -> String { d.iter().map(|b| format!("{b:02x}")).collect() }
fn digest(data: &[u8]) -> [u8; 32] { let mut h = sha256::Sha256::new(); h.update(data); h.finish() }

/// A model's header written here from the format, each file with the size and SHA-256 given.
fn header(id: &str, entry: &str, files: &[(&str, u64, [u8; 32])]) -> Vec<u8> {
    let mut h = format!("MIND-MODEL 1\nid {id}\nentry {}\n{entry}\n", entry.len()).into_bytes();
    for (path, size, sha) in files { h.extend(format!("file {size} {} {path}\n", hex(sha)).into_bytes()); }
    h.extend(b"data\n");
    h
}

/// The object of `header` and `bytes`, put as one object; its root.
fn object<B: Blocks>(store: &mut B, header: &[u8], bytes: &[u8]) -> Cid {
    let mut b = Box::new(Builder::new());
    b.write(store, header).unwrap();
    b.write(store, bytes).unwrap();
    b.finish(store).unwrap().0
}

fn files() -> Vec<(&'static str, Vec<u8>)> {
    vec![("tokens.txt", b"a\nb\nc\n".to_vec()), ("am/encoder.int8.onnx", (0..50_000u32).map(|i| (i * 7 + i / 300) as u8).collect()), ("empty.bin", Vec::new()),
         ("a name with spaces.bin", b"spaced".to_vec())]
}

fn model<B: Blocks>(store: &mut B, entry: &str) -> (Cid, Vec<(&'static str, Vec<u8>)>, usize) {
    let fs = files();
    let h = header("asr-test", entry, &fs.iter().map(|(p, d)| (*p, d.len() as u64, digest(d))).collect::<Vec<_>>());
    let root = object(store, &h, &fs.iter().flat_map(|(_, d)| d.clone()).collect::<Vec<_>>());
    (root, fs, h.len())
}

#[test]
fn every_file_is_found_by_its_path_and_read_whole_against_its_sha256() {
    with_store(|store| {
        // An entry with the format's words in it, and one long enough that the header spans two chunks.
        for entry in ["{\"id\": \"asr-test\", \"licence\": \"MIT\", \"terms\": \"file 1 data\\n\"}".to_string(), format!("{{\"terms\": \"{}\"}}", "x".repeat(20_000))] {
            let (root, fs, data) = model(store, &entry);
            let mut at = data as u64;
            for (path, bytes) in &fs {
                let file = find(store, &root, path, &mut vec![0; HEADER_MAX], &mut buffer()).unwrap();
                assert_eq!(file, Located { at, size: bytes.len() as u64, sha256: digest(bytes) }, "{path}");
                let mut out = vec![0; bytes.len()];
                assert_eq!(read(store, &root, &file, &mut out, &mut buffer()), Ok(()));
                assert_eq!(&out, bytes, "{path}");
                at += bytes.len() as u64;
            }
            // A buffer that is not the file's size.
            let file = find(store, &root, "tokens.txt", &mut vec![0; HEADER_MAX], &mut buffer()).unwrap();
            assert_eq!(read(store, &root, &file, &mut [0; 5], &mut buffer()), Err(Error::Size));
            assert_eq!(read(store, &root, &file, &mut [0; 7], &mut buffer()), Err(Error::Size));
            // A path the header does not name; a prefix or a part of one.
            for path in ["nothing", "tokens", "am", "encoder.int8.onnx", "a name"] {
                assert_eq!(find(store, &root, path, &mut vec![0; HEADER_MAX], &mut buffer()), Err(Error::NoFile), "{path}");
            }
        }
    });
}

#[test]
fn a_file_that_is_not_its_sha256_or_that_the_object_ends_before_is_refused() {
    with_store(|store| {
        let fs = files();
        let mut sums: Vec<_> = fs.iter().map(|(p, d)| (*p, d.len() as u64, digest(d))).collect();
        sums[1].2[31] ^= 1;
        let bytes: Vec<u8> = fs.iter().flat_map(|(_, d)| d.clone()).collect();
        let root = object(store, &header("m", "{}", &sums), &bytes);
        let file = find(store, &root, "am/encoder.int8.onnx", &mut vec![0; HEADER_MAX], &mut buffer()).unwrap();
        assert_eq!(read(store, &root, &file, &mut vec![0; 50_000], &mut buffer()), Err(Error::Digest));
        // The others still read.
        let file = find(store, &root, "tokens.txt", &mut vec![0; HEADER_MAX], &mut buffer()).unwrap();
        assert_eq!(read(store, &root, &file, &mut [0; 6], &mut buffer()), Ok(()));
        // The object cut before the last file ends: that file is short, the ones before it read.
        let sums: Vec<_> = fs.iter().map(|(p, d)| (*p, d.len() as u64, digest(d))).collect();
        let root = object(store, &header("m", "{}", &sums), &bytes[..bytes.len() - 1]);
        let file = find(store, &root, "a name with spaces.bin", &mut vec![0; HEADER_MAX], &mut buffer()).unwrap();
        assert_eq!(read(store, &root, &file, &mut [0; 6], &mut buffer()), Err(Error::Short));
        let file = find(store, &root, "am/encoder.int8.onnx", &mut vec![0; HEADER_MAX], &mut buffer()).unwrap();
        assert_eq!(read(store, &root, &file, &mut vec![0; 50_000], &mut buffer()), Ok(()));
    });
}

#[test]
fn a_header_not_of_the_format_is_refused() {
    with_store(|store| {
        let sha = digest(b"x");
        let good = header("m", "{}", &[("x", 1, sha)]);
        let cases: Vec<(&str, Vec<u8>, Error)> = vec![
            ("another format", { let mut h = good.clone(); h[11] = b'2'; h }, Error::Header),
            ("no data line", good[..good.len() - 5].to_vec(), Error::Header),
            ("an entry longer than it says", b"MIND-MODEL 1\nid m\nentry 1\n{}\nfile 1 00 x\ndata\n".to_vec(), Error::Header),
            ("an entry's length not a number", b"MIND-MODEL 1\nid m\nentry two\n{}\ndata\n".to_vec(), Error::Header),
            ("no entry line", b"MIND-MODEL 1\nid m\ndata\n".to_vec(), Error::Header),
            ("a line before data that is no file line", { let mut h = header("m", "{}", &[]); h.truncate(h.len() - 5); h.extend(format!("other\nfile 1 {} x\ndata\n", hex(&sha)).bytes()); h }, Error::NoFile),
            ("a digest not of 64 digits", { let mut h = header("m", "{}", &[]); h.truncate(h.len() - 5); h.extend(b"file 1 abcd x\ndata\n"); h }, Error::NoFile),
            ("a size not a number", { let mut h = header("m", "{}", &[]); h.truncate(h.len() - 5); h.extend(format!("file one {} x\ndata\n", hex(&sha)).bytes()); h }, Error::NoFile),
        ];
        for (case, h, error) in cases {
            let root = object(store, &h, b"x");
            assert_eq!(find(store, &root, "x", &mut vec![0; HEADER_MAX], &mut buffer()), Err(error), "{case}");
        }
        // A header past the buffer it is read into, and an object that ends before its header does.
        let long = header("m", &format!("{{\"t\": \"{}\"}}", "y".repeat(HEADER_MAX)), &[("x", 1, sha)]);
        let root = object(store, &long, b"x");
        assert_eq!(find(store, &root, "x", &mut vec![0; HEADER_MAX], &mut buffer()), Err(Error::Header));
        assert_eq!(find(store, &root, "x", &mut vec![0; 2 * HEADER_MAX], &mut buffer()).map(|f| f.size), Ok(1));
        let root = object(store, &good[..20], b"");
        assert_eq!(find(store, &root, "x", &mut vec![0; HEADER_MAX], &mut buffer()), Err(Error::Header));
    });
}

/// A store that answers a block with other bytes than it was asked for.
struct Lying<'s, 'a> { store: &'s mut Store<'a, &'a mut Memory>, swap: HashMap<Cid, Vec<u8>> }
impl Blocks for Lying<'_, '_> {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, dag::Error> { Blocks::put(self.store, codec, data) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> {
        match self.swap.get(cid) { Some(bytes) => { out[..bytes.len()].copy_from_slice(bytes); Ok(bytes.len()) } None => Blocks::get(self.store, cid, out) }
    }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> { Blocks::has(self.store, cid) }
}

#[test]
fn a_store_that_lies_about_a_chunk_is_not_believed() {
    let mut medium = Memory { data: vec![0; SECTORS * SECTOR] };
    let mut room = Room { index: vec![store::Entry::EMPTY; store::slots_for(SECTORS as u64)], heads: vec![Head::EMPTY; 8], pins: vec![Pin::EMPTY; 4], holes: vec![Extent::EMPTY; 64], buffer: Box::new([0; BUFFER]), scratch: Box::new([0; CHUNK]) };
    let mut store = Store::mount(&mut medium, &TAGGED, None, &mut room.index, &mut room.heads, &mut room.pins, &mut room.holes, &mut room.buffer, &mut room.scratch, 0).unwrap();
    let (root, _, data) = model(&mut store, "{}");
    // The chunk the encoder's bytes start in, answered changed: the read is refused, not the digest.
    let file = find(&mut store, &root, "am/encoder.int8.onnx", &mut vec![0; HEADER_MAX], &mut buffer()).unwrap();
    let mut b = buffer();
    let n = Blocks::get(&mut store, &root, &mut b[..]).unwrap();
    let node = dag::decode(&b[..n]).unwrap();
    let chunk = node.link(data / CHUNK);
    let n = Blocks::get(&mut store, &chunk, &mut b[..]).unwrap();
    let mut changed = b[..n].to_vec();
    changed[data % CHUNK + 10] ^= 1;
    let mut lying = Lying { store: &mut store, swap: HashMap::from([(chunk, changed)]) };
    assert_eq!(read(&mut lying, &root, &file, &mut vec![0; 50_000], &mut buffer()), Err(Error::Store(dag::Error::Corrupt)));
}
