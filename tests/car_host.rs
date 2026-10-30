//! Host tests of libmind/src/car.rs (issue 309-STO-0031; MC-4.4, 4.8, 6.8): copies of named roots in a CAR v1 file,
//! through the block store's own logic (blockstore/src/store.rs on a simulated medium). A copy written here by hand from
//! the CAR v1 and DAG-CBOR specifications is what an export writes and what a check reads; every truncation, every
//! changed byte, a CID of another hash or codec, a length past a block and a names node not exactly its encoding are
//! refused; export then import into an empty store gives the same names, roots and blocks; an import into a store
//! holding a damaged block repairs it, and a scrub then finds nothing; a name at another root is left as it is and
//! reported; nothing is put from a copy with one bad section, nor published from one that changed between its reads.
#![allow(dead_code)]
#[path = "../libmind/src/sha256.rs"]
mod sha256;
#[path = "../libmind/src/cid.rs"]
mod cid;
#[path = "../libmind/src/dag.rs"]
mod dag;
#[path = "../libmind/src/tree.rs"]
mod tree;
#[path = "../blockstore/src/store.rs"]
mod store;
#[path = "../libmind/src/worktree.rs"]
mod worktree;
#[path = "../libmind/src/car.rs"]
mod car;

use car::{Checked, Error, Exported, Imported, Named, Outcome, Refused, Source, Target, NAMES, NODE_MAX, SECTION_MAX};
use cid::{Cid, Codec};
use dag::{Blocks, Builder, CHUNK};
use store::{hmac_sha256, Crypto, Device, Entry, Extent, Head, Pin, Scrubbed, Store, BUFFER, SECTOR};
use worktree::Worktree;

const SECTORS: usize = 8192;

/// A medium in memory that counts its writes.
struct Memory { data: Vec<u8>, writes: usize }
impl Memory {
    fn new() -> Self { Memory { data: vec![0; SECTORS * SECTOR], writes: 0 } }
}
impl Device for &mut Memory {
    fn sectors(&self) -> u64 { (self.data.len() / SECTOR) as u64 }
    fn writable(&self) -> bool { true }
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool { let at = lba as usize * SECTOR; out.copy_from_slice(&self.data[at..at + out.len()]); true }
    fn write(&mut self, lba: u64, data: &[u8]) -> bool {
        self.writes += 1;
        let at = lba as usize * SECTOR;
        self.data[at..at + data.len()].copy_from_slice(data);
        true
    }
    fn flush(&mut self) -> bool { true }
}

/// Records sealed by a tag alone, their bytes left plain, so a test finds a block on the medium: copies need the store's
/// logic, not its cipher, which tests/blockstore_host.rs covers.
struct Tagged;
impl Crypto for Tagged {
    fn seal(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8]) -> [u8; 16] { hmac_sha256(key, &[nonce, aad, data])[..16].try_into().unwrap() }
    fn open(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8], tag: &[u8; 16]) -> bool { hmac_sha256(key, &[nonce, aad, data])[..16] == *tag }
    fn random(&self, _: &mut [u8]) -> bool { false }
}

struct Room { index: Vec<Entry>, heads: Vec<Head>, pins: Vec<Pin>, holes: Vec<Extent>, buffer: Box<[u8; BUFFER]>, scratch: Box<[u8; CHUNK]> }
impl Room {
    fn new() -> Self {
        Room { index: vec![Entry::EMPTY; store::slots_for(SECTORS as u64)], heads: vec![Head::EMPTY; 16], pins: vec![Pin::EMPTY; 4], holes: vec![Extent::EMPTY; 64], buffer: Box::new([0; BUFFER]), scratch: Box::new([0; CHUNK]) }
    }
}

type Local<'a> = Store<'a, &'a mut Memory>;

fn mount<'a>(medium: &'a mut Memory, room: &'a mut Room) -> Local<'a> {
    Store::mount(medium, &Tagged, None, &mut room.index, &mut room.heads, &mut room.pins, &mut room.holes, &mut room.buffer, &mut room.scratch, 0).unwrap()
}

/// The badge the tests publish with (the shell's on the platform).
const OWNER: u16 = 7;

/// The store's names as a publication sees them.
impl<D: Device> worktree::Names for Store<'_, D> {
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<Option<u64>, dag::Error> {
        match Store::publish(self, name.as_bytes(), expected, root, OWNER) {
            Ok(version) => Ok(Some(version)),
            Err(store::Error::Conflict) => Ok(None),
            Err(store::Error::Incomplete) => Err(dag::Error::NotFound),
            Err(store::Error::Invalid) => Err(dag::Error::Shape),
            Err(_) => Err(dag::Error::Store),
        }
    }
    fn version(&mut self, name: &str) -> Result<u64, dag::Error> { self.snapshot(name.as_bytes()).map(|(v, _)| v).map_err(|_| dag::Error::Store) }
}

/// The store as copies see it: names read at one point (requests are served one at a time), and a collection.
impl<D: Device> car::Store for Store<'_, D> {
    fn snapshot(&mut self, names: &[&str], heads: &mut [(u64, Option<Cid>)]) -> Result<(), dag::Error> {
        for (name, head) in names.iter().zip(heads.iter_mut()) { *head = Store::snapshot(self, name.as_bytes()).map_err(|_| dag::Error::Store)?; }
        Ok(())
    }
    fn collect(&mut self) -> Result<u64, dag::Error> { Store::collect(self).map(|c| c.blocks as u64).map_err(|_| dag::Error::Store) }
}

impl Target for Vec<u8> {
    fn write(&mut self, data: &[u8]) -> Result<(), Error> { self.extend_from_slice(data); Ok(()) }
}

fn pattern(seed: usize, len: usize) -> Vec<u8> { (0..len).map(|i| ((i * 31 + seed * 7 + 1) % 251) as u8).collect() }
fn unhex(text: &str) -> Vec<u8> { (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect() }
fn chunks() -> (Box<[u8; CHUNK]>, Box<[u8; CHUNK]>) { (Box::new([0; CHUNK]), Box::new([0; CHUNK])) }

/// Stores `data` as an object: its root and size.
fn object(store: &mut Local, data: &[u8]) -> (Cid, u64) {
    let mut builder = Box::new(Builder::new());
    builder.write(store, data).unwrap();
    builder.finish(store).unwrap()
}

/// An object's bytes, every block checked.
fn read(store: &mut Local, root: &Cid) -> Vec<u8> {
    let (mut buffer, mut chunk) = chunks();
    let mut out = Vec::new();
    dag::each_chunk(store, root, &mut buffer, &mut chunk, |_, bytes| { out.extend_from_slice(bytes); Ok(()) }).unwrap();
    out
}

/// Every block under `root` in the order the walk visits it; each one the store holds.
fn blocks(store: &mut Local, root: &Cid) -> Vec<Cid> {
    let (mut buffer, _) = chunks();
    let mut visited = Vec::new();
    tree::walk(store, root, &mut buffer, 1 << 20, |s, cid| { assert!(Store::has(s, cid), "{cid}"); visited.push(*cid); Ok(()) }).unwrap();
    visited
}

fn export(store: &mut Local, names: &[&str]) -> (Vec<u8>, Exported) {
    let (mut buffer, mut block) = chunks();
    car::export(store, names, Vec::new(), 1 << 20, &mut buffer, &mut block, |_, _, _| {}).unwrap()
}

/// A check of `copy`: what it found and each name in it.
fn check(copy: &[u8]) -> Result<(Checked, Vec<(String, u64, Cid)>), Refused> {
    let (mut buffer, mut block) = chunks();
    let mut names = Vec::new();
    car::check(copy, &mut buffer, &mut block, |n| names.push((n.name.to_string(), n.version, n.root))).map(|c| (c, names))
}

fn refusal(copy: &[u8]) -> Error { check(copy).expect_err("refused").error }

/// An import of `copy` into `store`: what it did and what became of each name.
fn import(store: &mut impl car::Store, copy: impl Source) -> Result<(Imported, Vec<(String, Outcome)>), Refused> {
    let (mut buffer, mut block) = chunks();
    let mut names = Vec::new();
    car::import(store, copy, &mut buffer, &mut block, |n, o| names.push((n.name.to_string(), o))).map(|i| (i, names))
}

fn varint(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 { out.push(byte); return out; }
        out.push(byte | 0x80);
    }
}

// A CID's binary form, written here from multiformats: version 1, the codec, sha2-256, 32 bytes, the digest.
fn cid_bytes(codec: u8, data: &[u8]) -> Vec<u8> { [&[0x01, codec, 0x12, 0x20][..], &sha256::digest(data)].concat() }
fn section(cid: &[u8], block: &[u8]) -> Vec<u8> { [varint((cid.len() + block.len()) as u64), cid.to_vec(), block.to_vec()].concat() }
// A header naming the names node `node`, as CAR v1 writes it: `{"roots": [link], "version": 1}`.
fn header_of(node: &[u8]) -> Vec<u8> {
    let head = [unhex("a265726f6f747381d82a582500"), cid_bytes(0x71, node), unhex("6776657273696f6e01")].concat();
    [varint(head.len() as u64), head].concat()
}
// A copy of the names node `node` and the sections after it.
fn copy_of(node: &[u8], sections: &[Vec<u8>]) -> Vec<u8> { [header_of(node), section(&cid_bytes(0x71, node), node)].into_iter().chain(sections.iter().cloned()).collect::<Vec<_>>().concat() }

const NOTE_LEN: usize = 100;

/// The copy of one name, `note` at version 1, a raw block of 100 bytes, written here byte by byte from the CAR v1 and
/// DAG-CBOR specifications: the header, the names node's section and the block's section (its length a varint of two
/// bytes). The names node and the block, too.
fn by_hand() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let block = pattern(9, NOTE_LEN);
    // {"v": 1, "names": [["note", 1, link]]}: map(2) "v" 1 "names" array(1) array(3) "note" 1, tag 42 over bytes(37), 0, CID.
    let node = [unhex("a2617601656e616d65738183646e6f746501d82a582500"), cid_bytes(0x55, &block)].concat();
    let copy = [
        unhex("3a"), unhex("a265726f6f747381d82a582500"), cid_bytes(0x71, &node), unhex("6776657273696f6e01"),
        unhex("5f"), cid_bytes(0x71, &node), node.clone(),
        unhex("8801"), cid_bytes(0x55, &block), block.clone(),
    ].concat();
    (copy, node, block)
}

#[test]
fn a_copy_is_the_car_v1_written_here_by_hand() {
    let (copy, node, block) = by_hand();
    assert_eq!(copy.len(), 1 + 58 + 1 + 95 + 2 + 136);
    assert_eq!(header_of(&node), copy[..59]);
    // An export of the same name writes exactly these bytes.
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let (root, _) = object(&mut store, &block);
    store.publish(b"note", 0, &root, OWNER).unwrap();
    let (written, exported) = export(&mut store, &["note"]);
    assert_eq!(written, copy);
    assert_eq!(exported, Exported { names: 1, blocks: 1, bytes: copy.len() as u64, digest: sha256::digest(&copy) });
    // A check reads it: the name, one block, the copy's bytes and their SHA-256.
    let (checked, names) = check(&copy).unwrap();
    assert_eq!(checked, Checked { names: 1, blocks: 1, bytes: copy.len() as u64, digest: sha256::digest(&copy) });
    assert_eq!(names, [("note".to_string(), 1, Cid::from_bytes(&cid_bytes(0x55, &block)).unwrap())]);
    // The reader: the header's root, then each section checked, then the end.
    let mut reader = car::Reader::open(&copy[..]).unwrap();
    assert_eq!((reader.root(), reader.at()), (Cid::of(Codec::DagCbor, &node), 59));
    let mut out = Box::new([0u8; CHUNK]);
    assert_eq!(reader.next(&mut out), Ok(Some((Cid::of(Codec::DagCbor, &node), node.len()))));
    assert_eq!(car::decode_names(&out[..node.len()]).unwrap().get(0), Named { name: "note", version: 1, root });
    assert_eq!(reader.next(&mut out), Ok(Some((root, NOTE_LEN))));
    assert_eq!(&out[..NOTE_LEN], &block[..]);
    assert_eq!((reader.next(&mut out), reader.at(), reader.digest()), (Ok(None), copy.len() as u64, sha256::digest(&copy)));
    // The names node encodes as written by hand.
    let mut encoded = [0u8; NODE_MAX];
    let len = car::encode_names(&[Named { name: "note", version: 1, root }], &mut encoded).unwrap();
    assert_eq!(&encoded[..len], &node[..]);
}

/// A store with a tree under `home` (two directories, a file of three chunks under a node, an empty file, a small one),
/// an object of 20 chunks under `big`, a file under `note`, and `big`'s object in `home` as well: its blocks met twice.
fn stocked(store: &mut Local) -> Vec<(&'static str, Cid)> {
    let small = object(store, b"a small file\n");
    let empty = object(store, b"");
    let three = object(store, &pattern(1, 2 * CHUNK + 5));
    let big = object(store, &pattern(2, 20 * CHUNK - 3));
    let note = object(store, &pattern(9, NOTE_LEN));
    let mut wt = Box::new(Worktree::new());
    for dir in ["docs", "docs/deep"] { wt.mkdir(store, dir).unwrap(); }
    for (path, (cid, size)) in [("docs/small.txt", small), ("docs/deep/empty", empty), ("three.bin", three), ("big.bin", big)] { wt.put(store, path, &cid, size).unwrap(); }
    let (_, home) = wt.publish(store, "home").unwrap();
    store.publish(b"big", 0, &big.0, OWNER).unwrap();
    store.publish(b"note", 0, &note.0, OWNER).unwrap();
    vec![("big", big.0), ("home", home), ("note", note.0)]
}

#[test]
fn export_then_import_into_an_empty_store_gives_the_same_names_roots_and_blocks() {
    let (mut a_medium, mut b_medium) = (Memory::new(), Memory::new());
    let (mut a_room, mut b_room) = (Room::new(), Room::new());
    let mut a = mount(&mut a_medium, &mut a_room);
    let names = stocked(&mut a);
    // Given in any order, listed sorted; each name's blocks in the walk's order, `big`'s twice.
    let (copy, exported) = export(&mut a, &["note", "home", "big"]);
    let walked: Vec<Vec<Cid>> = names.iter().map(|(_, root)| blocks(&mut a, root)).collect();
    let total = walked.iter().map(Vec::len).sum::<usize>() as u64;
    assert_eq!((exported.names, exported.blocks, exported.bytes), (3, total, copy.len() as u64));
    assert_eq!(walked[0].len(), 21);
    assert!(walked[1].windows(21).any(|w| w == &walked[0][..]));
    let (checked, listed) = check(&copy).unwrap();
    assert_eq!(checked, Checked { names: 3, blocks: total, bytes: copy.len() as u64, digest: exported.digest });
    assert_eq!(listed, names.iter().map(|(n, root)| (n.to_string(), 1, *root)).collect::<Vec<_>>());
    // The sections are the walks' blocks in order, after the names node.
    let mut reader = car::Reader::open(&copy[..]).unwrap();
    let mut out = Box::new([0u8; CHUNK]);
    reader.next(&mut out).unwrap();
    let sections: Vec<Cid> = std::iter::from_fn(|| reader.next(&mut out).unwrap().map(|(cid, _)| cid)).collect();
    assert_eq!(sections, walked.concat());

    // Into an empty store: each name published at its root, each block stored once.
    let mut b = mount(&mut b_medium, &mut b_room);
    let distinct = { let mut all = walked.concat(); all.sort(); all.dedup(); all.len() as u64 };
    let (imported, outcomes) = import(&mut b, &copy[..]).unwrap();
    assert_eq!(imported, Imported { checked, stored: distinct, repaired: 0, held: total - distinct, collected: None });
    assert_eq!(outcomes, names.iter().map(|(n, _)| (n.to_string(), Outcome::Published(1))).collect::<Vec<_>>());
    assert_eq!(b.stats().blocks as u64, distinct);
    for ((name, root), walk) in names.iter().zip(&walked) {
        assert_eq!(b.resolve(name.as_bytes()), Ok((1, *root)));
        assert_eq!(&blocks(&mut b, root), walk);
    }
    for (_, root) in names.iter().filter(|(n, _)| *n != "home") { assert_eq!(read(&mut b, root), read(&mut a, root)); }
    // The same copy again: every name at its root already, nothing published, nothing written.
    let writes = b.device().writes;
    let (again, outcomes) = import(&mut b, &copy[..]).unwrap();
    assert_eq!((again.stored, again.repaired, again.held), (0, 0, total));
    assert_eq!(outcomes, names.iter().map(|(n, _)| (n.to_string(), Outcome::Same(1))).collect::<Vec<_>>());
    assert_eq!(b.device().writes, writes);
    // And an export of the imported store is the same copy.
    assert_eq!(export(&mut b, &["big", "home", "note"]).0, copy);
}

#[test]
fn every_truncation_and_every_changed_byte_is_refused() {
    let (copy, _, _) = by_hand();
    for len in 0..copy.len() { assert!(check(&copy[..len]).is_err(), "cut to {len}"); }
    for at in 0..copy.len() {
        for bit in 0..8 {
            let mut changed = copy.clone();
            changed[at] ^= 1 << bit;
            assert!(check(&changed).is_err(), "byte {at}, bit {bit}");
        }
    }
    // Cut at a section's end the copy is a CAR v1 still, of fewer blocks: a walk under a root finds the next one missing.
    assert_eq!(check(&copy[..59]).unwrap_err(), Refused { error: Error::Missing, at: 59 });
    assert_eq!(check(&copy[..155]).unwrap_err(), Refused { error: Error::Missing, at: 155 });
    // A copy of a tree: cuts at and around each section's start and every 97 bytes, and a changed byte in each
    // section's length and CID and in some of each block.
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let three = object(&mut store, &pattern(1, 2 * CHUNK + 5));
    let small = object(&mut store, b"a small file\n");
    let mut wt = Box::new(Worktree::new());
    wt.mkdir(&mut store, "docs").unwrap();
    wt.put(&mut store, "docs/small.txt", &small.0, small.1).unwrap();
    wt.put(&mut store, "three.bin", &three.0, three.1).unwrap();
    wt.publish(&mut store, "tree").unwrap();
    let (copy, _) = export(&mut store, &["tree"]);
    let mut starts = vec![59];
    let mut reader = car::Reader::open(&copy[..]).unwrap();
    let mut out = Box::new([0u8; CHUNK]);
    while reader.next(&mut out).unwrap().is_some() { starts.push(reader.at() as usize); }
    starts.pop();
    for len in (0..copy.len()).filter(|len| starts.iter().any(|s| len.abs_diff(*s) <= 40) || len % 97 == 0) {
        assert!(check(&copy[..len]).is_err(), "cut to {len}");
    }
    for &start in &starts {
        for at in (start..start + 40).chain([start + 45, start + 200, start + 5000]).filter(|&at| at < copy.len()) {
            let mut changed = copy.clone();
            changed[at] ^= 0x04;
            assert!(check(&changed).is_err(), "byte {at}");
        }
    }
    assert!(check(&copy).is_ok());
    // A byte more at the end: a section after the last, cut short.
    assert_eq!(refusal(&[&copy[..], &[0x01]].concat()), Error::Truncated);
}

#[test]
fn a_cid_of_another_hash_or_codec_a_length_past_a_block_and_a_header_not_its_encoding_are_refused() {
    let (copy, node, block) = by_hand();
    let with = |last: Vec<u8>| [&copy[..155], &last[..]].concat();
    let refused_at = |copy: &[u8]| check(copy).unwrap_err();
    // The block's section with its CID of another hash (sha2-512), content type (dag-pb), version (CIDv0) or length.
    let sha512 = [&[0x01, 0x55, 0x13, 0x40][..], &[0xab; 64]].concat();
    assert_eq!(refused_at(&with(section(&sha512, &block))), Refused { error: Error::Cid(cid::Error::Algorithm), at: 155 });
    assert_eq!(refusal(&with(section(&cid_bytes(0x70, &block), &block))), Error::Cid(cid::Error::Codec));
    assert_eq!(refusal(&with(section(&[&[0x12, 0x20][..], &sha256::digest(&block)].concat(), &block))), Error::Cid(cid::Error::Version));
    let short = [&[0x01, 0x55, 0x12, 0x10][..], &sha256::digest(&block)[..16]].concat();
    assert_eq!(refusal(&with(section(&short, &block))), Error::Cid(cid::Error::Length));
    // A block that does not match its CID; one of the right bytes as `dag-cbor`.
    assert_eq!(refusal(&with(section(&cid_bytes(0x55, b"other"), &block))), Error::Corrupt);
    assert_eq!(refusal(&with(section(&cid_bytes(0x71, &block), &block))), Error::Order);
    // Lengths: past a block, cutting the CID, not in the shortest form, past the copy's end.
    let past = [varint(SECTION_MAX as u64 + 1), cid_bytes(0x55, &block), block.clone()].concat();
    assert_eq!(refused_at(&with(past)), Refused { error: Error::TooLarge, at: 155 });
    assert_eq!(refusal(&with([varint(20), cid_bytes(0x55, &block)[..20].to_vec()].concat())), Error::Format);
    assert_eq!(refusal(&with([vec![0x88, 0x81, 0x00], cid_bytes(0x55, &block), block.clone()].concat())), Error::Format);
    assert_eq!(refusal(&with([varint(137), cid_bytes(0x55, &block), block.clone()].concat())), Error::Truncated);
    assert_eq!(refusal(&with(vec![0xff; 9])), Error::Format);
    // A block of the most bytes is read; one more is past a block.
    let full = vec![7u8; CHUNK];
    let most = [&copy[..155], &section(&cid_bytes(0x55, &full), &full)[..]].concat();
    let mut reader = car::Reader::open(&most[..]).unwrap();
    reader.next(&mut Box::new([0u8; CHUNK])).unwrap();
    assert_eq!(reader.next(&mut Box::new([0u8; CHUNK])), Ok(Some((Cid::raw(&full), CHUNK))));
    // Sections out of order: the block before the names node, the block twice, a section after the last.
    assert_eq!(refused_at(&[&copy[..59], &copy[155..], &copy[59..155]].concat()), Refused { error: Error::Order, at: 59 });
    assert_eq!(refused_at(&[&copy[..], &copy[155..]].concat()), Refused { error: Error::Order, at: copy.len() as u64 });
    let second = [0x55u8; 10];
    assert_eq!(refusal(&with(section(&cid_bytes(0x55, &second), &second))), Error::Order);

    // The header: another root's codec, two roots, keys out of order, a length not its own, CAR v2, a version 2.
    let rest = &copy[59..];
    let head = |bytes: &str| [varint(unhex(bytes).len() as u64), unhex(bytes)].concat();
    let node_cid = hex(&cid_bytes(0x71, &node));
    let raw_root = hex(&cid_bytes(0x55, &node));
    for (bytes, error) in [
        (format!("a265726f6f747381d82a582500{raw_root}6776657273696f6e01"), Error::Format),
        (format!("a265726f6f747382d82a582500{node_cid}d82a582500{node_cid}6776657273696f6e01"), Error::Format),
        (format!("a26776657273696f6e0165726f6f747381d82a582500{node_cid}"), Error::Format),
        (format!("a265726f6f747381d82a582500{node_cid}6776657273696f6e1801"), Error::Format),
        (format!("a265726f6f747381d82a582500{node_cid}6776657273696f6e02"), Error::Version),
        ("a16776657273696f6e02".to_string(), Error::Version),
        (format!("a365726f6f747381d82a582500{node_cid}6776657273696f6e01"), Error::Format),
        (format!("a265726f6f747380{}6776657273696f6e01", ""), Error::Format),
    ] {
        assert_eq!(refused_at(&[head(&bytes), rest.to_vec()].concat()), Refused { error, at: 0 }, "{bytes}");
    }
    assert_eq!(refusal(&[vec![59], copy[1..].to_vec()].concat()), Error::Format);
    assert_eq!(refusal(&[vec![0xba, 0x00], copy[1..].to_vec()].concat()), Error::Format);
    assert_eq!(refusal(&[]), Error::Truncated);
    // The names node, not first: a header naming another root.
    let other = [unhex("a2617601656e616d65738183646e6f746502d82a582500"), cid_bytes(0x55, &block)].concat();
    assert_eq!(refused_at(&[header_of(&other), rest.to_vec()].concat()), Refused { error: Error::Order, at: 59 });
}

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }

/// The sections of `copy` after its header: where each starts and ends.
fn spans(copy: &[u8]) -> Vec<(usize, usize)> {
    let mut reader = car::Reader::open(copy).unwrap();
    let mut out = Box::new([0u8; CHUNK]);
    let mut spans = Vec::new();
    let mut at = reader.at() as usize;
    while reader.next(&mut out).unwrap().is_some() {
        spans.push((at, reader.at() as usize));
        at = reader.at() as usize;
    }
    spans
}

#[test]
fn a_check_finds_a_directory_met_long_before_and_refuses_a_block_left_out_or_moved() {
    // The root's first entry holds 40 directories, each with a file of its own: the walk reads the root again after more
    // `dag-cbor` sections than a check remembers, so it is looked for from the first.
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut wt = Box::new(Worktree::new());
    wt.mkdir(&mut store, "a").unwrap();
    for k in 0..40 {
        let (cid, size) = object(&mut store, format!("file {k}\n").as_bytes());
        wt.mkdir(&mut store, &format!("a/d{k:02}")).unwrap();
        wt.put(&mut store, &format!("a/d{k:02}/f"), &cid, size).unwrap();
    }
    let (last, size) = object(&mut store, b"last\n");
    wt.put(&mut store, "b", &last, size).unwrap();
    wt.publish(&mut store, "wide").unwrap();
    let (copy, exported) = export(&mut store, &["wide"]);
    assert_eq!(exported.blocks, 1 + 1 + 40 * 2 + 1);
    assert_eq!(check(&copy).unwrap().0.blocks, exported.blocks);
    // Each block left out, and each moved one place on: the copy is no longer the walk's, and is refused.
    let spans = spans(&copy);
    for k in 1..spans.len() {
        let (start, end) = spans[k];
        let left_out = [&copy[..start], &copy[end..]].concat();
        assert!(matches!(refusal(&left_out), Error::Order | Error::Missing), "section {k} left out");
        if k + 1 < spans.len() {
            let next = spans[k + 1].1;
            let moved = [&copy[..start], &copy[end..next], &copy[start..end], &copy[next..]].concat();
            assert!(matches!(refusal(&moved), Error::Order | Error::Missing), "section {k} moved");
        }
    }
}

#[test]
fn a_names_node_not_exactly_its_encoding_is_refused() {
    let (_, _, block) = by_hand();
    let link = format!("d82a582500{}", hex(&cid_bytes(0x55, &block)));
    let entry = |name: &str, version: &str| format!("83{:02x}{}{version}{link}", 0x60 + name.len(), hex(name.as_bytes()));
    let block_section = section(&cid_bytes(0x55, &block), &block);
    // Each names node in a copy whose CIDs are right, so the node alone is at fault; the walk of `note` needs the block.
    let refused = |node: String| check(&copy_of(&unhex(&node), &[block_section.clone()])).unwrap_err();
    assert!(check(&copy_of(&unhex(&format!("a2617601656e616d657381{}", entry("note", "01"))), &[block_section.clone()])).is_ok());
    for (node, error) in [
        // Unsorted, and a name twice.
        (format!("a2617601656e616d657382{}{}", entry("b", "01"), entry("a", "01")), Error::Format),
        (format!("a2617601656e616d657382{}{}", entry("a", "01"), entry("a", "01")), Error::Format),
        // A version not in its shortest form, keys out of DAG-CBOR's order, another key, another schema version.
        (format!("a2617601656e616d657381{}", entry("note", "1801")), Error::Format),
        (format!("a2656e616d657381{}617601", entry("note", "01")), Error::Format),
        (format!("a3617601656e616d657381{}6178f6", entry("note", "01")), Error::Format),
        (format!("a2617602656e616d657381{}", entry("note", "01")), Error::Version),
        // Arrays of indefinite length, an entry of four, a root that is not a link, a name as bytes.
        (format!("a2617601656e616d65739f{}ff", entry("note", "01")), Error::Format),
        (format!("a2617601656e616d657381{}00", entry("note", "01").replacen("83", "84", 1)), Error::Format),
        (format!("a2617601656e616d657381{}", entry("note", "01").replace("d82a", "d82b")), Error::Format),
        (format!("a2617601656e616d657381{}", entry("note", "01").replacen("64", "44", 1)), Error::Format),
        // Names the store does not take, version 0, none, more than NAMES.
        (format!("a2617601656e616d657381{}", entry("a:b", "01")), Error::Name),
        (format!("a2617601656e616d657381{}", entry(&"n".repeat(65), "01").replacen("83a1", "837841", 1)), Error::Name),
        (format!("a2617601656e616d657381{}", entry("note", "00")), Error::Name),
        ("a2617601656e616d657380".to_string(), Error::Name),
        (format!("a2617601656e616d657389{}", (0..9).map(|k| entry(&format!("n{k}"), "01")).collect::<String>()), Error::Name),
    ] {
        assert_eq!(refused(node.clone()).error, error, "{node}");
    }
    // Encoding refuses what decoding would: unsorted, twice, more than NAMES, version 0, a name the store does not take.
    let root = Cid::raw(&block);
    let n = |name| Named { name, version: 1, root };
    let mut out = [0u8; NODE_MAX];
    for names in [vec![n("b"), n("a")], vec![n("a"), n("a")], vec![], (0..=NAMES).map(|_| n("x")).collect(), vec![Named { version: 0, ..n("a") }], vec![n("a:b")]] {
        assert_eq!(car::encode_names(&names, &mut out), Err(Error::Name));
    }
}

/// Flips a byte of the block `data` where it lies on the medium, so its record no longer opens. Records here are sealed
/// by a tag alone: a block's record is found in the clear by its header (`MIND-BLK`, the CID at 16), its bytes after it.
fn damage(medium: &mut Memory, data: &[u8]) {
    let cid = Cid::raw(data).to_bytes();
    let at = (0..medium.data.len() - 84).find(|&at| &medium.data[at..at + 8] == b"MIND-BLK" && medium.data[at + 16..at + 52] == cid).expect("the block on the medium");
    assert_eq!(&medium.data[at + 84..at + 84 + data.len()], data);
    medium.data[at + 84 + 50] ^= 0x10;
}

fn scrub(store: &mut Local) -> (Scrubbed, Vec<Option<Cid>>) {
    let mut found = Vec::new();
    store.restart_scrub();
    let report = store.scrub(u64::MAX, &mut |_, cid| found.push(cid)).unwrap().unwrap();
    (report, found)
}

#[test]
fn an_import_repairs_a_damaged_block_and_a_scrub_then_finds_nothing() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let names = stocked(&mut store);
    let (copy, exported) = export(&mut store, &["big", "home", "note"]);
    let records = scrub(&mut store).0.records;
    assert_eq!(scrub(&mut store), (Scrubbed { records, corrupt: 0, damaged: 0 }, vec![]));
    // A chunk of `big` changes on the medium after the copy: a scrub finds it, the store still holds it in its index.
    let chunk = pattern(2, 20 * CHUNK - 3)[5 * CHUNK..6 * CHUNK].to_vec();
    damage(store.device(), &chunk);
    let damaged = Cid::raw(&chunk);
    assert_eq!(scrub(&mut store), (Scrubbed { records, corrupt: 1, damaged: 0 }, vec![Some(damaged)]));
    assert!(store.has(&damaged));
    // The import reads it, finds it corrupt, stores it again; a collection frees the damaged copy.
    let (imported, outcomes) = import(&mut store, &copy[..]).unwrap();
    assert_eq!((imported.stored, imported.repaired, imported.held, imported.collected), (0, 1, exported.blocks - 1, Some(Ok(1))));
    assert_eq!(outcomes, names.iter().map(|(n, _)| (n.to_string(), Outcome::Same(1))).collect::<Vec<_>>());
    assert_eq!(scrub(&mut store), (Scrubbed { records, corrupt: 0, damaged: 0 }, vec![]));
    assert_eq!(read(&mut store, &names[0].1), pattern(2, 20 * CHUNK - 3));
    assert_eq!(store.stats().corrupt, 0);
    drop(store);
    // A new instance mounting the medium finds every block whole.
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    assert_eq!((store.stats().corrupt, store.stats().damaged), (0, 0));
    assert_eq!(read(&mut store, &names[0].1), pattern(2, 20 * CHUNK - 3));

    // Damage a mount found: the block left the index then, so the import stores it again as a block it lacked, and the
    // next collection frees the damaged copy.
    let note = pattern(9, NOTE_LEN);
    damage(store.device(), &note);
    drop(store);
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    assert_eq!(store.stats().corrupt, 1);
    let (imported, _) = import(&mut store, &copy[..]).unwrap();
    assert_eq!((imported.stored, imported.repaired, imported.collected), (1, 0, None));
    assert_eq!(read(&mut store, &names[2].1), note);
    store.collect().unwrap();
    assert_eq!(scrub(&mut store).0, Scrubbed { records, corrupt: 0, damaged: 0 });
}

/// The store, its names read as another writer left them before it published (`stale`) the first time they are read.
struct Racing<'s, 'a> { store: &'s mut Local<'a>, stale: Option<(u64, Option<Cid>)> }
impl Blocks for Racing<'_, '_> {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, dag::Error> { Blocks::put(self.store, codec, data) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> { Blocks::get(self.store, cid, out) }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> { Blocks::has(self.store, cid) }
}
impl worktree::Names for Racing<'_, '_> {
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<Option<u64>, dag::Error> { worktree::Names::publish(self.store, name, expected, root) }
    fn version(&mut self, name: &str) -> Result<u64, dag::Error> { worktree::Names::version(self.store, name) }
}
impl car::Store for Racing<'_, '_> {
    fn snapshot(&mut self, names: &[&str], heads: &mut [(u64, Option<Cid>)]) -> Result<(), dag::Error> {
        car::Store::snapshot(self.store, names, heads)?;
        if let Some(stale) = self.stale.take() { heads.fill(stale); }
        Ok(())
    }
    fn collect(&mut self) -> Result<u64, dag::Error> { car::Store::collect(self.store) }
}

/// A copy that reads as `first` until it is opened a second time, then as `second`.
struct Changing { first: Vec<u8>, second: Vec<u8>, opened: usize }
impl Source for Changing {
    fn read_at(&mut self, at: u64, out: &mut [u8]) -> Result<usize, Error> {
        if at == 0 { self.opened += 1; }
        let mut bytes = if self.opened <= 1 { &self.first[..] } else { &self.second[..] };
        bytes.read_at(at, out)
    }
}

#[test]
fn a_name_at_another_root_is_left_and_reported_and_a_bad_copy_puts_nothing() {
    let (mut a_medium, mut b_medium) = (Memory::new(), Memory::new());
    let (mut a_room, mut b_room) = (Room::new(), Room::new());
    let mut a = mount(&mut a_medium, &mut a_room);
    let names = stocked(&mut a);
    let (copy, exported) = export(&mut a, &["big", "home", "note"]);
    // In the other store: `note` at another root, `home` removed, `big` at the copy's root.
    let mut b = mount(&mut b_medium, &mut b_room);
    let (other, _) = object(&mut b, b"another note\n");
    b.publish(b"note", 0, &other, OWNER).unwrap();
    b.publish(b"home", 0, &other, OWNER).unwrap();
    b.unpublish(b"home", 1, OWNER).unwrap();
    let (big, _) = object(&mut b, &pattern(2, 20 * CHUNK - 3));
    b.publish(b"big", 0, &big, OWNER).unwrap();

    // A copy with one bad section, wherever it is: refused there, and nothing put; a copy cut short likewise.
    let mut reader = car::Reader::open(&copy[..]).unwrap();
    let mut out = Box::new([0u8; CHUNK]);
    let mut starts = vec![reader.at()];
    while reader.next(&mut out).unwrap().is_some() { starts.push(reader.at()); }
    let (writes, stats) = (b.device().writes, b.stats());
    for (k, &start) in starts[..starts.len() - 1].iter().enumerate().filter(|(k, _)| k % 7 == 0 || *k == starts.len() - 2) {
        let mut bad = copy.clone();
        bad[starts[k + 1] as usize - 1] ^= 0x01;
        assert_eq!(import(&mut b, &bad[..]).unwrap_err(), Refused { error: Error::Corrupt, at: start }, "section {k}");
    }
    assert_eq!(import(&mut b, &copy[..copy.len() - 1]).unwrap_err().error, Error::Truncated);
    assert_eq!(import(&mut b, &copy[..starts[starts.len() - 2] as usize]).unwrap_err().error, Error::Missing);
    assert_eq!((b.device().writes, b.stats().blocks, b.stats().names), (writes, stats.blocks, stats.names));

    // The copy whole: `big` the same, `home` published again from its removal, `note` left at its root and reported.
    let (imported, outcomes) = import(&mut b, &copy[..]).unwrap();
    assert_eq!(outcomes, [
        ("big".to_string(), Outcome::Same(1)),
        ("home".to_string(), Outcome::Published(3)),
        ("note".to_string(), Outcome::Conflict { version: 1, root: Some(other) }),
    ]);
    assert_eq!(imported.checked.digest, exported.digest);
    assert_eq!((b.resolve(b"note"), b.resolve(b"home"), b.resolve(b"big")), (Ok((1, other)), Ok((3, names[1].1)), Ok((1, big))));
    // The file's `note` is stored all the same, retained by nothing until it is published by hand.
    assert_eq!(read(&mut b, &names[2].1), pattern(9, NOTE_LEN));

    // Another writer publishes between the names read and the import's publication: a conflict, nothing published over it.
    let (mut c_medium, mut c_room) = (Memory::new(), Room::new());
    let mut c = mount(&mut c_medium, &mut c_room);
    let (other, _) = object(&mut c, b"published first\n");
    c.publish(b"note", 0, &other, OWNER).unwrap();
    let mut racing = Racing { store: &mut c, stale: Some((0, None)) };
    let copy_of_note = export(&mut a, &["note"]).0;
    let (_, outcomes) = import(&mut racing, &copy_of_note[..]).unwrap();
    assert_eq!(outcomes, [("note".to_string(), Outcome::Conflict { version: 1, root: Some(other) })]);
    assert_eq!(c.resolve(b"note"), Ok((1, other)));
    // One that published the copy's own root: the same, at its version.
    let mut d_medium = Memory::new();
    let mut d_room = Room::new();
    let mut d = mount(&mut d_medium, &mut d_room);
    let (note, _) = object(&mut d, &pattern(9, NOTE_LEN));
    d.publish(b"note", 0, &note, OWNER).unwrap();
    let (_, outcomes) = import(&mut Racing { store: &mut d, stale: Some((0, None)) }, &copy_of_note[..]).unwrap();
    assert_eq!(outcomes, [("note".to_string(), Outcome::Same(1))]);

    // A copy that changed between its check and its import: a section changed, or one more at its end. Nothing published.
    let mut e_medium = Memory::new();
    let mut e_room = Room::new();
    let mut e = mount(&mut e_medium, &mut e_room);
    let mut changed = copy.clone();
    changed[starts[3] as usize + 60] ^= 0x01;
    assert_eq!(import(&mut e, Changing { first: copy.clone(), second: changed, opened: 0 }).unwrap_err(), Refused { error: Error::Corrupt, at: starts[3] });
    let longer = [&copy[..], &copy[starts[1] as usize..starts[2] as usize]].concat();
    assert_eq!(import(&mut e, Changing { first: copy.clone(), second: longer, opened: 0 }).unwrap_err(), Refused { error: Error::Changed, at: copy.len() as u64 + starts[2] - starts[1] });
    assert_eq!((e.snapshot(b"big"), e.snapshot(b"home"), e.snapshot(b"note")), (Ok((0, None)), Ok((0, None)), Ok((0, None))));
}

#[test]
fn an_export_refuses_names_it_cannot_copy_and_writes_nothing() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let names = stocked(&mut store);
    store.unpublish(b"note", 1, OWNER).unwrap();
    let (mut buffer, mut block) = chunks();
    let mut heard = Vec::new();
    let mut attempt = |store: &mut Local, names: &[&str], heard: &mut Vec<(String, u64, Option<Cid>)>| {
        car::export(store, names, Vec::new(), 1 << 20, &mut buffer, &mut block, |n, v, r| heard.push((n.to_string(), v, r))).map(|(copy, _)| copy)
    };
    // No such name, a removed one: each heard, nothing written.
    assert_eq!(attempt(&mut store, &["home", "ghost", "note"], &mut heard), Err(Error::Missing));
    assert_eq!(heard, [("ghost".to_string(), 0, None), ("home".to_string(), 1, Some(names[1].1)), ("note".to_string(), 2, None)]);
    // Names the store does not take, none, too many, one twice.
    let many: Vec<String> = (0..=NAMES).map(|k| format!("n{k}")).collect();
    for bad in [vec!["a:b"], vec![], many.iter().map(String::as_str).collect(), vec!["home", "home"]] {
        assert_eq!(attempt(&mut store, &bad, &mut Vec::new()), Err(Error::Name));
    }
    // A root missing a block: the walk stops there.
    let lost = pattern(2, 20 * CHUNK - 3)[..CHUNK].to_vec();
    damage(store.device(), &lost);
    assert_eq!(attempt(&mut store, &["big"], &mut Vec::new()), Err(Error::Store(dag::Error::Corrupt)));
    assert_eq!(attempt(&mut store, &["big"], &mut Vec::new()), Err(Error::Store(dag::Error::NotFound)));
}
