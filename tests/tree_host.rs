//! Host tests of libmind/src/tree.rs (issue 311-STO-0027; MC-4.2, 4.5, 4.13): directories as DAG-CBOR nodes and the
//! walk of a tree of them. Node bytes are checked against an encoding written here from the schema; every non-canonical
//! node, bad entry and node over a block is refused; a walk visits exactly the blocks of a tree, and refuses a missing
//! block, a changed one, an entry that disagrees with its object and a tree past the depth. Two roots compared
//! (311-STO-0039) against a comparison of the trees' models, random ones among them, an equal subtree never read. A tree's
//! files listed with their paths (351-STO-0006) as the models list them, each directory checked, within the steps, depth
//! and path given.
#![allow(dead_code)]
#[path = "../libmind/src/sha256.rs"]
mod sha256;
#[path = "../libmind/src/cid.rs"]
mod cid;
#[path = "../libmind/src/dag.rs"]
mod dag;
#[path = "../libmind/src/tree.rs"]
mod tree;

use cid::{Cid, Codec};
use dag::{Blocks, Builder, Error, CHUNK, FANOUT};
use std::collections::{HashMap, HashSet};
use std::collections::BTreeMap;
use tree::{decode, diff, encode, files, size, walk, Change, Entry, Kind, Side, DEPTH, NAME_MAX, PATH_MAX};

/// Blocks in memory; `replace` makes the store answer a CID with other bytes; `reads` counts the blocks read.
#[derive(Default)]
struct Memory { blocks: HashMap<Cid, Vec<u8>>, replace: HashMap<Cid, Vec<u8>>, reads: usize }
impl Blocks for Memory {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, Error> {
        let cid = Cid::of(codec, data);
        self.blocks.insert(cid, data.to_vec());
        Ok(cid)
    }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> {
        self.reads += 1;
        let data = self.replace.get(cid).or_else(|| self.blocks.get(cid)).ok_or(Error::NotFound)?;
        out[..data.len()].copy_from_slice(data);
        Ok(data.len())
    }
    fn has(&mut self, cid: &Cid) -> Result<bool, Error> { Ok(self.blocks.contains_key(cid)) }
}

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
fn unhex(text: &str) -> Vec<u8> { (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect() }
fn pattern(seed: usize, len: usize) -> Vec<u8> { (0..len).map(|i| ((i * 31 + seed * 7 + 1) % 251) as u8).collect() }

/// A directory node written here from the schema, not by tree.rs, so that nodes the encoder refuses can be made too:
/// each entry is (name, kind, size, link).
fn written(entries: &[(&[u8], u64, u64, Cid)]) -> Vec<u8> {
    fn head(out: &mut Vec<u8>, major: u8, value: u64) {
        let m = major << 5;
        match value {
            0..=23 => out.push(m | value as u8),
            24..=0xff => out.extend([m | 24, value as u8]),
            0x100..=0xffff => { out.push(m | 25); out.extend((value as u16).to_be_bytes()) }
            0x1_0000..=0xffff_ffff => { out.push(m | 26); out.extend((value as u32).to_be_bytes()) }
            _ => { out.push(m | 27); out.extend(value.to_be_bytes()) }
        }
    }
    // {"v": 1, "entries": [...]}
    let mut out = unhex("a261760167656e7472696573");
    head(&mut out, 4, entries.len() as u64);
    for (name, kind, size, cid) in entries {
        out.push(0x84);
        head(&mut out, 3, name.len() as u64);
        out.extend_from_slice(name);
        head(&mut out, 0, *kind);
        head(&mut out, 0, *size);
        out.extend(unhex("d82a582500"));
        out.extend(cid.to_bytes());
    }
    out
}

fn tuples<'a>(entries: &[Entry<'a>]) -> Vec<(&'a [u8], u64, u64, Cid)> {
    entries.iter().map(|e| (e.name, if e.kind == Kind::File { 0 } else { 1 }, e.size, e.cid)).collect()
}

/// Every entry of a node, in order.
fn listed(bytes: &[u8]) -> Result<Vec<Entry<'_>>, Error> {
    let dir = decode(bytes)?;
    Ok((0..dir.len()).map(|i| dir.entry(i)).collect())
}

/// Stores `data` as an object; its root and size.
fn store(blocks: &mut Memory, data: &[u8]) -> (Cid, u64) {
    let mut builder = Box::new(Builder::new());
    builder.write(blocks, data).unwrap();
    builder.finish(blocks).unwrap()
}

/// Stores a directory of `entries`; its CID.
fn put_dir(blocks: &mut Memory, entries: &[Entry]) -> Cid {
    let mut out = Box::new([0u8; CHUNK]);
    let len = encode(entries, &mut out).unwrap();
    blocks.put(Codec::DagCbor, &out[..len]).unwrap()
}

/// An entry for a file of `data`, stored.
fn file<'a>(blocks: &mut Memory, name: &'a str, data: &[u8]) -> Entry<'a> {
    let (cid, size) = store(blocks, data);
    Entry { name: name.as_bytes(), kind: Kind::File, size, cid }
}

/// An entry for a directory of `entries`, stored.
fn dir<'a>(blocks: &mut Memory, name: &'a str, entries: &[Entry]) -> Entry<'a> {
    Entry { name: name.as_bytes(), kind: Kind::Directory, size: entries.len() as u64, cid: put_dir(blocks, entries) }
}

/// A tree: files of one chunk, of several and empty, a file named twice, a subdirectory with one of its own, an empty
/// directory. Its blocks, its root, the bytes of its files, and the CIDs of `sub`, of `a.txt` and of `big.bin`.
fn sample() -> (Memory, Cid, u64, [Cid; 3]) {
    let mut blocks = Memory::default();
    let small = pattern(1, 100);
    let deeper = [file(&mut blocks, "c.txt", &pattern(2, 10)), file(&mut blocks, "nothing", b"")];
    let sub = [file(&mut blocks, "b.txt", &pattern(3, CHUNK)), dir(&mut blocks, "deeper", &deeper)];
    let entries = [
        file(&mut blocks, "a.txt", &small),
        file(&mut blocks, "big.bin", &pattern(4, 3 * CHUNK + 5)),
        dir(&mut blocks, "empty", &[]),
        file(&mut blocks, "same.txt", &small),
        dir(&mut blocks, "sub", &sub),
    ];
    let root = put_dir(&mut blocks, &entries);
    (blocks, root, (100 + 10 + CHUNK + 3 * CHUNK + 5 + 100) as u64, [entries[4].cid, entries[0].cid, entries[1].cid])
}

const LINK_ABC: &str = "d82a58250001551220ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

#[test]
fn a_directory_is_the_schemas_encoding_and_reads_back() {
    let mut out = Box::new([0u8; CHUNK]);
    // The empty directory, and one file of 3 bytes named "a", byte for byte.
    assert_eq!(encode(&[], &mut out), Ok(13));
    assert_eq!(hex(&out[..13]), "a261760167656e747269657380");
    assert_eq!(listed(&out[..13]), Ok(vec![]));
    let abc = Cid::raw(b"abc");
    let len = encode(&[Entry { name: b"a", kind: Kind::File, size: 3, cid: abc }], &mut out).unwrap();
    assert_eq!(hex(&out[..len]), "a261760167656e747269657381846161".to_owned() + "0003" + LINK_ABC);
    // Names of one to NAME_MAX bytes, in the order of their bytes; sizes in every integer form; both kinds.
    let long = [b'z'; NAME_MAX];
    let names: [&[u8]; 9] = [b"...", b".a", b"B", b"a", b"a b", b"ab", b"z", &long, "имя".as_bytes()];
    let sizes = [0, 23, 24, 255, 256, 65_536, 1 << 32, u64::MAX, 7];
    let node = Cid::of(Codec::DagCbor, b"a directory");
    let entries: Vec<Entry> = names.iter().zip(sizes).enumerate()
        .map(|(i, (name, size))| if i % 3 == 1 { Entry { name, kind: Kind::Directory, size, cid: node } } else { Entry { name, kind: Kind::File, size, cid: Cid::raw(name) } })
        .collect();
    let len = encode(&entries, &mut out).unwrap();
    assert_eq!(out[..len], written(&tuples(&entries))[..]);
    assert_eq!(listed(&out[..len]).unwrap(), entries);
    let read = decode(&out[..len]).unwrap();
    for e in &entries { assert_eq!(read.find(e.name), Some(*e)); }
    for absent in [&b"!"[..], b"a c", b"aa", b"zz", "имя0".as_bytes()] { assert_eq!(read.find(absent), None); }
}

#[test]
fn only_the_canonical_encoding_decodes() {
    let node = written(&[
        ("a.txt".as_bytes(), 0, 300, Cid::raw(b"a")),
        ("dir".as_bytes(), 1, 2, Cid::of(Codec::DagCbor, b"d")),
        ("é".as_bytes(), 0, 70_000, Cid::of(Codec::DagCbor, b"e")),
    ]);
    assert!(decode(&node).is_ok());
    // Every byte changed to every other value: refused, or read as exactly what it encodes.
    let (mut out, mut read) = (Box::new([0u8; CHUNK]), 0);
    for at in 0..node.len() {
        for value in 0..=255u8 {
            let mut changed = node.clone();
            changed[at] = value;
            if value == node[at] { continue; }
            if let Ok(entries) = listed(&changed) {
                let len = encode(&entries, &mut out).unwrap();
                assert_eq!(out[..len], changed[..], "byte {at} as {value:#x}");
                read += 1;
            }
        }
    }
    // A digest's bytes, a name's last character: some changes are other valid directories.
    assert!(read > 0);
    for len in 0..node.len() { assert!(decode(&node[..len]).is_err(), "cut to {len}"); }
    let header = "a261760167656e7472696573";
    let entry = "846161".to_owned() + "0003" + LINK_ABC;
    for (case, bytes, error) in [
        ("another version", "a26176026765".to_owned() + "6e747269657381" + &entry, Error::Version),
        ("entries before v", "a2676".to_owned() + "56e747269657381" + &entry + "617601", Error::Format),
        ("an extra key", "a36176016765".to_owned() + "6e747269657381" + &entry + "617800", Error::Format),
        ("a map head written longer", "b802617601".to_owned() + "6765" + "6e747269657381" + &entry, Error::Format),
        ("a size written longer", header.to_owned() + "818461610018" + "03" + LINK_ABC, Error::Format),
        ("a name's length written longer", header.to_owned() + "8184780161" + "0003" + LINK_ABC, Error::Format),
        ("a name's length past any end", header.to_owned() + "81847bffffffffffffffff61" + "0003" + LINK_ABC, Error::Format),
        ("an indefinite array", header.to_owned() + "9f" + &entry + "ff", Error::Format),
        ("an entry of three", header.to_owned() + "81836161" + "00" + LINK_ABC, Error::Format),
        ("a name as a byte string", header.to_owned() + "81844161" + "0003" + LINK_ABC, Error::Format),
        ("a negative size", header.to_owned() + "8184616100" + "22" + LINK_ABC, Error::Format),
        ("a link without tag 42", header.to_owned() + "81846161" + "0003" + &LINK_ABC[4..], Error::Format),
        ("bytes after the node", header.to_owned() + "81" + &entry + "00", Error::Format),
    ] {
        assert_eq!(decode(&unhex(&bytes)).err(), Some(error), "{case}");
    }
}

#[test]
fn entries_out_of_order_or_badly_named_are_refused() {
    let abc = Cid::raw(b"abc");
    let node = Cid::of(Codec::DagCbor, b"a directory");
    let long = [b'n'; NAME_MAX + 1];
    let file = |name: &'static [u8]| (name, 0u64, 3u64, abc);
    for (case, entries) in [
        ("unsorted", vec![file(b"b"), file(b"a")]),
        ("a name twice", vec![file(b"a"), file(b"a")]),
        ("an empty name", vec![file(b"")]),
        ("a slash", vec![file(b"a/b")]),
        ("a NUL", vec![file(b"a\0b")]),
        (".", vec![file(b".")]),
        ("..", vec![file(b"..")]),
        ("not UTF-8", vec![file(b"\xff")]),
        ("an overlong slash", vec![file(b"\xc0\xaf")]),
        ("a lone continuation byte", vec![file(b"a\x80")]),
        ("a surrogate", vec![file(b"\xed\xa0\x80")]),
        ("a directory linked as raw", vec![(&b"d"[..], 1, 0, abc)]),
    ] {
        assert_eq!(decode(&written(&entries)).err(), Some(Error::Format), "{case}");
        let entries: Vec<Entry> = entries.iter().map(|&(name, kind, size, cid)| Entry { name, kind: if kind == 0 { Kind::File } else { Kind::Directory }, size, cid }).collect();
        assert_eq!(encode(&entries, &mut Box::new([0u8; CHUNK])), Err(Error::Format), "{case}");
    }
    // A name too long, a kind of neither, both only as bytes.
    assert_eq!(decode(&written(&[(&long, 0, 3, abc)])).err(), Some(Error::Format));
    assert_eq!(encode(&[Entry { name: &long, kind: Kind::File, size: 3, cid: abc }], &mut Box::new([0u8; CHUNK])), Err(Error::Format));
    assert_eq!(decode(&written(&[(b"a", 2, 3, abc)])).err(), Some(Error::Format));
    // A file may be a node, a directory's link may not be raw: the same entries with the right links are a directory.
    assert!(decode(&written(&[(b"d", 1, 0, node), (b"f", 0, 3, node)])).is_ok());
}

#[test]
fn a_directory_over_a_block_is_refused() {
    let abc = Cid::raw(b"abc");
    let names: Vec<Vec<u8>> = (0..60).map(|i| format!("{i:03}{}", "x".repeat(NAME_MAX - 3)).into_bytes()).collect();
    let entries: Vec<Entry> = names.iter().map(|name| Entry { name, kind: Kind::File, size: 3, cid: abc }).collect();
    // The most entries of the longest names a block holds, and one more.
    let fits = (0..=entries.len()).rev().find(|&k| written(&tuples(&entries[..k])).len() <= CHUNK).unwrap();
    assert!(fits > 0 && fits < entries.len());
    let mut out = Box::new([0u8; CHUNK]);
    assert_eq!(encode(&entries[..fits], &mut out), Ok(written(&tuples(&entries[..fits])).len()));
    assert_eq!(encode(&entries[..fits + 1], &mut out), Err(Error::TooLarge));
    assert_eq!(decode(&written(&tuples(&entries[..fits + 1]))).err(), Some(Error::TooLarge));
}

#[test]
fn a_walk_visits_exactly_the_blocks_of_a_tree() {
    let (mut blocks, inner, inner_total, _) = sample();
    // The sample and a file of height 2 (two nodes of height 1 under its root) side by side, under one more root.
    let huge = file(&mut blocks, "huge.bin", &pattern(5, CHUNK * FANOUT + 3));
    let root = put_dir(&mut blocks, &[huge, Entry { name: b"tree", kind: Kind::Directory, size: 5, cid: inner }]);
    let total = inner_total + huge.size;
    let mut buffer = Box::new([0u8; CHUNK]);
    let mut seen = HashSet::new();
    assert_eq!(walk(&mut blocks, &root, &mut buffer, u64::MAX, |_, cid| { seen.insert(*cid); Ok(()) }), Ok(total));
    // Every block stored, and nothing else.
    assert_eq!(seen, blocks.blocks.keys().copied().collect());
    assert_eq!(size(&mut blocks, &root, &mut buffer, u64::MAX), Ok(total));
    // A file is a root too, walked as `dag` walks it.
    let (mut by_tree, mut by_dag) = (Vec::new(), Vec::new());
    assert_eq!(walk(&mut blocks, &huge.cid, &mut buffer, u64::MAX, |_, cid| { by_tree.push(*cid); Ok(()) }), Ok(huge.size));
    assert_eq!(dag::walk(&mut blocks, &huge.cid, &mut buffer, |_, cid| { by_dag.push(*cid); Ok(()) }), Ok(huge.size));
    assert_eq!(by_tree, by_dag);
    assert_eq!(size(&mut blocks, &huge.cid, &mut buffer, u64::MAX), Ok(huge.size));
    // A visit that refuses stops the walk with its error.
    assert_eq!(walk(&mut blocks, &root, &mut buffer, u64::MAX, |_, _| Err(Error::Full)), Err(Error::Full));
}

#[test]
fn a_missing_block_is_not_found_and_a_changed_one_corrupt() {
    let (mut blocks, root, total, [sub, small, big]) = sample();
    let mut buffer = Box::new([0u8; CHUNK]);
    let has = |b: &mut Memory, cid: &Cid| if b.has(cid)? { Ok(()) } else { Err(Error::NotFound) };
    assert_eq!(walk(&mut blocks, &root, &mut buffer, u64::MAX, has), Ok(total));
    // Each block missing in turn, the chunks too.
    let all: Vec<Cid> = blocks.blocks.keys().copied().collect();
    for cid in &all {
        let data = blocks.blocks.remove(cid).unwrap();
        assert_eq!(walk(&mut blocks, &root, &mut buffer, u64::MAX, has), Err(Error::NotFound), "{cid}");
        blocks.blocks.insert(*cid, data);
    }
    // The root, a directory below it, a file of one chunk and a file's root node, each answered with other bytes.
    for cid in [root, sub, small, big] {
        let mut forged = blocks.blocks[&cid].clone();
        let last = forged.len() - 1;
        forged[last] ^= 1;
        blocks.replace.insert(cid, forged);
        assert_eq!(walk(&mut blocks, &root, &mut buffer, u64::MAX, |_, _| Ok(())), Err(Error::Corrupt), "{cid}");
        assert_eq!(size(&mut blocks, &root, &mut buffer, u64::MAX), Err(Error::Corrupt), "{cid}");
        blocks.replace.clear();
    }
}

#[test]
fn entries_that_disagree_with_their_objects_are_refused() {
    let mut blocks = Memory::default();
    let (small, _) = store(&mut blocks, b"abc");
    let (big, big_size) = store(&mut blocks, &pattern(1, CHUNK + 1));
    let empty = put_dir(&mut blocks, &[]);
    let one = put_dir(&mut blocks, &[Entry { name: b"x", kind: Kind::File, size: 3, cid: small }]);
    let mut buffer = Box::new([0u8; CHUNK]);
    for (case, kind, claimed, cid) in [
        ("a file's size other than its bytes", Kind::File, 4, small),
        ("a larger file's size other than its bytes", Kind::File, big_size - 1, big),
        ("a directory's size other than its entries", Kind::Directory, 0, one),
        ("an empty directory said to hold one", Kind::Directory, 1, empty),
        ("a file's node named as a directory", Kind::Directory, 2, big),
        ("a directory named as a file", Kind::File, 1, one),
    ] {
        let root = put_dir(&mut blocks, &[Entry { name: b"e", kind, size: claimed, cid }]);
        assert_eq!(walk(&mut blocks, &root, &mut buffer, u64::MAX, |_, _| Ok(())), Err(Error::Shape), "{case}");
        assert_eq!(size(&mut blocks, &root, &mut buffer, u64::MAX), Err(Error::Shape), "{case}");
    }
    // A directory of another version, as the root or below it; a node of neither schema.
    let other = blocks.put(Codec::DagCbor, &unhex("a261760267656e747269657380")).unwrap();
    let below = put_dir(&mut blocks, &[Entry { name: b"d", kind: Kind::Directory, size: 0, cid: other }]);
    let neither = blocks.put(Codec::DagCbor, &unhex("a1617601")).unwrap();
    for (root, error) in [(other, Error::Version), (below, Error::Version), (neither, Error::Format)] {
        assert_eq!(walk(&mut blocks, &root, &mut buffer, u64::MAX, |_, _| Ok(())), Err(error));
        assert_eq!(size(&mut blocks, &root, &mut buffer, u64::MAX), Err(error));
    }
}

#[test]
fn a_tree_past_the_depth_is_refused() {
    let mut blocks = Memory::default();
    // A chain of directories, each holding the next; the last holds a file of 3 bytes.
    let last = file(&mut blocks, "f", b"abc");
    let mut chain = vec![put_dir(&mut blocks, &[last])];
    while chain.len() <= DEPTH {
        let below = *chain.last().unwrap();
        chain.push(put_dir(&mut blocks, &[Entry { name: b"d", kind: Kind::Directory, size: 1, cid: below }]));
    }
    let mut buffer = Box::new([0u8; CHUNK]);
    // DEPTH directories from the root down, and one more.
    assert_eq!(walk(&mut blocks, &chain[DEPTH - 1], &mut buffer, u64::MAX, |_, _| Ok(())), Ok(3));
    assert_eq!(walk(&mut blocks, &chain[DEPTH], &mut buffer, u64::MAX, |_, _| Ok(())), Err(Error::Shape));
    assert_eq!(size(&mut blocks, &chain[DEPTH], &mut buffer, u64::MAX), Err(Error::Shape));
}

#[test]
fn files_past_u64_bytes_are_refused() {
    // A file of 2^62 bytes from one chunk: at each of 6 levels a node whose 256 links are the same node.
    let mut blocks = Memory::default();
    let mut link = blocks.put(Codec::Raw, &[7; CHUNK]).unwrap();
    let mut node = vec![0u8; dag::NODE_MAX];
    for h in 1..=6 {
        let len = dag::encode((CHUNK as u64) << (8 * h), &[link; FANOUT], &mut node);
        link = blocks.put(Codec::DagCbor, &node[..len]).unwrap();
    }
    let mut buffer = Box::new([0u8; CHUNK]);
    assert_eq!(dag::size(&mut blocks, &link, &mut buffer), Ok(1 << 62));
    let names: [&[u8]; 4] = [b"a", b"b", b"c", b"d"];
    let entries: Vec<Entry> = names.iter().map(|name| Entry { name, kind: Kind::File, size: 1 << 62, cid: link }).collect();
    // Three of them are 3 * 2^62 bytes; four are more than u64.
    let three = put_dir(&mut blocks, &entries[..3]);
    let four = put_dir(&mut blocks, &entries);
    assert_eq!(size(&mut blocks, &three, &mut buffer, u64::MAX), Ok(3 << 62));
    assert_eq!(size(&mut blocks, &four, &mut buffer, u64::MAX), Err(Error::TooLarge));
}

#[test]
fn a_walk_takes_at_most_the_steps_it_is_given() {
    // A directory of 300 entries that all name the same subdirectory of one file: few blocks, many steps.
    let mut blocks = Memory::default();
    let inner = [file(&mut blocks, "f", b"x")];
    let sub = dir(&mut blocks, "s", &inner);
    let names: Vec<String> = (0..300).map(|k| format!("e{k:03}")).collect();
    let wide: Vec<Entry> = names.iter().map(|n| Entry { name: n.as_bytes(), ..sub }).collect();
    let root = put_dir(&mut blocks, &wide);
    let mut buffer = [0u8; CHUNK];
    // Each of the 300 entries is a step, its subdirectory a block visited, that one's entry a step and its file a block.
    let needed = 1 + 300 * 4;
    assert_eq!(walk(&mut blocks, &root, &mut buffer, needed, |_, _| Ok(())), Ok(300));
    assert_eq!(walk(&mut blocks, &root, &mut buffer, needed - 1, |_, _| Ok(())), Err(Error::TooLarge));
    // Sizing reads no file's blocks: its steps are the entries and the directories.
    assert_eq!(size(&mut blocks, &root, &mut buffer, 1 + 300 * 3), Ok(300));
    assert_eq!(size(&mut blocks, &root, &mut buffer, 300 * 3), Err(Error::TooLarge));
    // A file's walk is counted too: each block it visits.
    let (big, _) = store(&mut blocks, &pattern(9, 3 * CHUNK));
    assert_eq!(walk(&mut blocks, &big, &mut buffer, 4, |_, _| Ok(())), Ok(3 * CHUNK as u64));
    assert_eq!(walk(&mut blocks, &big, &mut buffer, 3, |_, _| Ok(())), Err(Error::TooLarge));
}


/// A tree's model: a file's bytes, or a directory's entries by name.
#[derive(Clone, PartialEq, Debug)]
enum M { F(Vec<u8>), D(BTreeMap<String, M>) }

/// Stores the directory `entries` models, its directories first; its CID.
fn build(blocks: &mut Memory, entries: &BTreeMap<String, M>) -> Cid {
    let mut sides = Vec::new();
    for (name, m) in entries { sides.push((name, side_of(blocks, m))); }
    let listed: Vec<Entry> = sides.iter().map(|(name, s)| Entry { name: name.as_bytes(), kind: s.kind, size: s.size, cid: s.cid }).collect();
    put_dir(blocks, &listed)
}

/// What an entry of the model `m` names, stored.
fn side_of(blocks: &mut Memory, m: &M) -> Side {
    match m {
        M::F(data) => { let (cid, size) = store(blocks, data); Side { kind: Kind::File, size, cid } }
        M::D(entries) => Side { kind: Kind::Directory, size: entries.len() as u64, cid: build(blocks, entries) },
    }
}

/// The comparison of two models, as `diff` should make it: each path where they differ, in the names' order.
fn model_diff(blocks: &mut Memory, a: &BTreeMap<String, M>, b: &BTreeMap<String, M>, prefix: &str, out: &mut Vec<(String, Change)>) {
    let names: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    for name in names {
        let path = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        match (a.get(name), b.get(name)) {
            (Some(x), Some(y)) if x == y => {}
            (Some(M::D(x)), Some(M::D(y))) => model_diff(blocks, x, y, &path, out),
            (Some(x), Some(y)) => { let change = Change::Changed(side_of(blocks, x), side_of(blocks, y)); out.push((path, change)) }
            (Some(x), None) => { let change = Change::Removed(side_of(blocks, x)); out.push((path, change)) }
            (None, Some(y)) => { let change = Change::Added(side_of(blocks, y)); out.push((path, change)) }
            (None, None) => unreachable!(),
        }
    }
}

/// `diff` of two roots within a million steps, more than any tree here needs, so one that never ends fails: what it visited, its count checked.
fn compared(blocks: &mut Memory, a: Option<&Cid>, b: Option<&Cid>) -> Result<Vec<(String, Change)>, Error> {
    let (mut one, mut two) = (Box::new([0u8; CHUNK]), Box::new([0u8; CHUNK]));
    let mut path = vec![0u8; PATH_MAX];
    let mut seen = Vec::new();
    let mut left = 1 << 20;
    let n = diff(blocks, a, b, [&mut one, &mut two], &mut path, &mut left, |p, c| { seen.push((String::from_utf8(p.to_vec()).unwrap(), c)); Ok(()) })?;
    assert_eq!(n as usize, seen.len());
    Ok(seen)
}

fn d(entries: &[(&str, M)]) -> BTreeMap<String, M> { entries.iter().map(|(n, m)| (n.to_string(), m.clone())).collect() }
fn f(text: &str) -> M { M::F(text.as_bytes().to_vec()) }

#[test]
fn two_roots_compared_name_each_change() {
    let mut blocks = Memory::default();
    let deep = d(&[("x.txt", f("x")), ("y.txt", f("y"))]);
    let a = d(&[("a.txt", f("one")), ("big.bin", M::F(pattern(4, 3 * CHUNK + 5))), ("docs", M::D(d(&[("deep", M::D(deep.clone())), ("n.txt", f("n"))]))),
                ("gone", M::D(d(&[("g", f("g"))]))), ("kind", f("a file")), ("same", M::D(d(&[("s", f("s"))])))]);
    let mut deep2 = deep.clone();
    deep2.insert("y.txt".into(), f("y, changed"));
    deep2.insert("z.txt".into(), f("z"));
    let b = d(&[("a.txt", f("one")), ("big.bin", M::F(pattern(5, 3 * CHUNK + 5))), ("docs", M::D(d(&[("deep", M::D(deep2)), ("n.txt", f("n"))]))),
                ("kind", M::D(d(&[]))), ("new", M::D(d(&[("w", f("w"))]))), ("same", M::D(d(&[("s", f("s"))])))]);
    let (ra, rb) = (build(&mut blocks, &a), build(&mut blocks, &b));
    let seen = compared(&mut blocks, Some(&ra), Some(&rb)).unwrap();
    let paths: Vec<&str> = seen.iter().map(|(p, _)| p.as_str()).collect();
    // A file changed at the root, a file changed and one added two levels down, a directory gone whole, a file become a
    // directory, a directory new whole; equal entries and the equal directory `same` not named.
    assert_eq!(paths, ["big.bin", "docs/deep/y.txt", "docs/deep/z.txt", "gone", "kind", "new"]);
    let kinds: Vec<&str> = seen.iter().map(|(_, c)| match c { Change::Added(_) => "added", Change::Removed(_) => "removed", Change::Changed(..) => "changed" }).collect();
    assert_eq!(kinds, ["changed", "changed", "added", "removed", "changed", "added"]);
    let mut expected = Vec::new();
    model_diff(&mut blocks, &a, &b, "", &mut expected);
    assert_eq!(seen, expected);
    // The sides: a directory's size is its entries; a kind changed keeps both kinds.
    assert_eq!(seen[3].1, Change::Removed(Side { kind: Kind::Directory, size: 1, cid: build(&mut blocks, &d(&[("g", f("g"))])) }));
    assert!(matches!(seen[4].1, Change::Changed(Side { kind: Kind::File, size: 6, .. }, Side { kind: Kind::Directory, size: 0, .. })));
    // The other way round, every change turned over.
    let back = compared(&mut blocks, Some(&rb), Some(&ra)).unwrap();
    let turned: Vec<(String, Change)> = seen.iter().map(|(p, c)| (p.clone(), match *c { Change::Added(s) => Change::Removed(s), Change::Removed(s) => Change::Added(s), Change::Changed(x, y) => Change::Changed(y, x) })).collect();
    assert_eq!(back, turned);
    // A root with itself, and the empty tree with itself: nothing, and nothing read.
    blocks.reads = 0;
    assert_eq!(compared(&mut blocks, Some(&ra), Some(&ra)), Ok(vec![]));
    assert_eq!(compared(&mut blocks, None, None), Ok(vec![]));
    assert_eq!(blocks.reads, 0);
    // From the empty tree: each entry of the root added, none gone into.
    let all = compared(&mut blocks, None, Some(&ra)).unwrap();
    assert_eq!(all.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["a.txt", "big.bin", "docs", "gone", "kind", "same"]);
    assert!(all.iter().all(|(_, c)| matches!(c, Change::Added(_))));
    let none = compared(&mut blocks, Some(&ra), None).unwrap();
    assert!(none.len() == 6 && none.iter().all(|(_, c)| matches!(c, Change::Removed(_))));
    // The empty directory as a root is the empty tree.
    let empty = put_dir(&mut blocks, &[]);
    assert_eq!(compared(&mut blocks, Some(&empty), Some(&ra)), Ok(all));
}

#[test]
fn an_equal_subtree_is_never_read() {
    // Two trees that differ in one file two levels down, beside a subtree of 1 directory, then of 200.
    let reads = |wide: usize| {
        let mut blocks = Memory::default();
        let big: BTreeMap<String, M> = (0..wide).map(|k| (format!("d{k:03}"), M::D(d(&[("f", M::F(pattern(k, 10)))])))).collect();
        let a = d(&[("big", M::D(big.clone())), ("p", M::D(d(&[("q", M::D(d(&[("r", f("one"))])))])))]);
        let b = d(&[("big", M::D(big)), ("p", M::D(d(&[("q", M::D(d(&[("r", f("two"))])))])))]);
        let (ra, rb) = (build(&mut blocks, &a), build(&mut blocks, &b));
        blocks.reads = 0;
        let seen = compared(&mut blocks, Some(&ra), Some(&rb)).unwrap();
        assert_eq!(seen.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["p/q/r"]);
        blocks.reads
    };
    // The pairs on the way down (the roots, p, q), each read again on the way back once a pair below it was: 10 blocks.
    assert_eq!(reads(1), 10);
    assert_eq!(reads(200), 10);
}

/// A small random number generator, so the trees are the same on every run.
struct Rng(u64);
impl Rng {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) % n as u64) as usize
    }
}

/// A random directory: names from a few, so the same name meets itself; files from a few contents, some equal.
fn random_dir(rng: &mut Rng, depth: usize) -> BTreeMap<String, M> {
    let mut out = BTreeMap::new();
    for _ in 0..rng.next(5) {
        let name = ["a", "b", "c", "d", "e", "f"][rng.next(6)].to_string();
        let m = if depth < 4 && rng.next(3) == 0 { M::D(random_dir(rng, depth + 1)) } else { M::F(pattern(rng.next(4), rng.next(3) * 7)) };
        out.insert(name, m);
    }
    out
}

/// One random edit somewhere in `dir`: an entry added, removed, its file changed, or its kind changed.
fn mutate(rng: &mut Rng, dir: &mut BTreeMap<String, M>, depth: usize) {
    let subs: Vec<String> = dir.iter().filter(|(_, m)| matches!(m, M::D(_))).map(|(n, _)| n.clone()).collect();
    if !subs.is_empty() && rng.next(2) == 0 {
        if let Some(M::D(sub)) = dir.get_mut(&subs[rng.next(subs.len())]) { return mutate(rng, sub, depth + 1); }
    }
    let name = ["a", "b", "c", "d", "e", "f", "g"][rng.next(7)].to_string();
    match rng.next(4) {
        0 => { dir.remove(&name); }
        1 => { dir.insert(name, M::F(pattern(rng.next(9), 1 + rng.next(30)))); }
        2 => { dir.insert(name, M::D(random_dir(rng, depth + 1))); }
        _ => { dir.insert(name, M::D(BTreeMap::new())); }
    }
}

#[test]
fn random_trees_compare_as_their_models() {
    let mut rng = Rng(311_0039);
    let mut changed = 0;
    for _ in 0..300 {
        let mut blocks = Memory::default();
        let a = random_dir(&mut rng, 0);
        let mut b = a.clone();
        for _ in 0..1 + rng.next(4) { mutate(&mut rng, &mut b, 0); }
        let (ra, rb) = (build(&mut blocks, &a), build(&mut blocks, &b));
        let mut expected = Vec::new();
        model_diff(&mut blocks, &a, &b, "", &mut expected);
        assert_eq!(compared(&mut blocks, Some(&ra), Some(&rb)).unwrap(), expected, "{a:?}\n{b:?}");
        changed += expected.len();
    }
    // The edits made changes of every sort, many of them.
    assert!(changed > 300, "{changed}");
}

#[test]
fn a_bad_node_is_refused_when_read() {
    let mut blocks = Memory::default();
    let a = d(&[("s", M::D(d(&[("f", f("one"))])))]);
    let b = d(&[("s", M::D(d(&[("f", f("two"))])))]);
    let (ra, rb) = (build(&mut blocks, &a), build(&mut blocks, &b));
    let sub = side_of(&mut blocks, &M::D(d(&[("f", f("two"))]))).cid;
    // A node missing, a node changed.
    let kept = blocks.blocks.remove(&sub).unwrap();
    assert_eq!(compared(&mut blocks, Some(&ra), Some(&rb)), Err(Error::NotFound));
    blocks.blocks.insert(sub, kept);
    blocks.replace.insert(sub, put_dir_bytes(&[]));
    assert_eq!(compared(&mut blocks, Some(&ra), Some(&rb)), Err(Error::Corrupt));
    blocks.replace.clear();
    // A root that is a file, and one that is a file's node.
    let raw = blocks.put(Codec::Raw, b"not a directory").unwrap();
    assert_eq!(compared(&mut blocks, Some(&raw), Some(&rb)), Err(Error::Shape));
    let (node, _) = store(&mut blocks, &pattern(1, 2 * CHUNK));
    assert_eq!(compared(&mut blocks, Some(&node), Some(&rb)), Err(Error::Shape));
    // An entry naming a directory with another count of entries than it has.
    let two = side_of(&mut blocks, &M::D(d(&[("f", f("two"))])));
    let lying = put_dir(&mut blocks, &[Entry { name: b"s", kind: Kind::Directory, size: 2, cid: two.cid }]);
    assert_eq!(compared(&mut blocks, Some(&ra), Some(&lying)), Err(Error::Shape));
    // Entries of one CID with two counts: gone into, and the one that disagrees with the node is refused.
    let one = side_of(&mut blocks, &M::D(d(&[("f", f("one"))])));
    let miscounted = put_dir(&mut blocks, &[Entry { name: b"s", kind: Kind::Directory, size: 3, cid: one.cid }]);
    assert_eq!(compared(&mut blocks, Some(&ra), Some(&miscounted)), Err(Error::Shape));
    // An entry naming a file's node as a directory.
    let posing = put_dir(&mut blocks, &[Entry { name: b"s", kind: Kind::Directory, size: 1, cid: node }]);
    assert_eq!(compared(&mut blocks, Some(&ra), Some(&posing)), Err(Error::Shape));
}

/// A directory's encoding, as `put_dir` stores it.
fn put_dir_bytes(entries: &[Entry]) -> Vec<u8> {
    let mut out = Box::new([0u8; CHUNK]);
    let len = encode(entries, &mut out).unwrap();
    out[..len].to_vec()
}

#[test]
fn a_comparison_is_bounded_by_its_steps_depth_and_path() {
    let mut blocks = Memory::default();
    // Two chains of DEPTH directories with a file at the bottom that differs: DEPTH pairs, the last one's file changed.
    let chain = |blocks: &mut Memory, levels: usize, text: &str| {
        let mut m = d(&[("f", f(text))]);
        for _ in 1..levels { m = d(&[("dddd", M::D(m))]); }
        build(blocks, &m)
    };
    let (a, b) = (chain(&mut blocks, DEPTH, "one"), chain(&mut blocks, DEPTH, "two"));
    let seen = compared(&mut blocks, Some(&a), Some(&b)).unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, format!("{}f", "dddd/".repeat(DEPTH - 1)));
    // One level more is past the depth.
    let (a2, b2) = (chain(&mut blocks, DEPTH + 1, "one"), chain(&mut blocks, DEPTH + 1, "two"));
    assert_eq!(compared(&mut blocks, Some(&a2), Some(&b2)), Err(Error::Shape));
    // The steps: each pair read on the way down and again on the way back (but the last), each entry compared.
    let needed = (DEPTH * 2 + (DEPTH - 1) * 2 + DEPTH) as u64;
    let (mut one, mut two) = (Box::new([0u8; CHUNK]), Box::new([0u8; CHUNK]));
    let mut path = vec![0u8; PATH_MAX];
    let mut left = needed;
    assert_eq!(diff(&mut blocks, Some(&a), Some(&b), [&mut one, &mut two], &mut path, &mut left, |_, _| Ok(())), Ok(1));
    assert_eq!(left, 0);
    let mut left = needed - 1;
    assert_eq!(diff(&mut blocks, Some(&a), Some(&b), [&mut one, &mut two], &mut path, &mut left, |_, _| Ok(())), Err(Error::TooLarge));
    // A path buffer one byte short of the changed file's path.
    let mut short = vec![0u8; seen[0].0.len() - 1];
    let mut left = u64::MAX;
    assert_eq!(diff(&mut blocks, Some(&a), Some(&b), [&mut one, &mut two], &mut short, &mut left, |_, _| Ok(())), Err(Error::TooLarge));
    let mut exact = vec![0u8; seen[0].0.len()];
    assert_eq!(diff(&mut blocks, Some(&a), Some(&b), [&mut one, &mut two], &mut exact, &mut left, |_, _| Ok(())), Ok(1));
    // PATH_MAX holds DEPTH names of NAME_MAX bytes.
    let long = "n".repeat(NAME_MAX);
    let deep = |blocks: &mut Memory, text: &str| {
        let mut m = d(&[(long.as_str(), f(text))]);
        for _ in 1..DEPTH { m = d(&[(long.as_str(), M::D(m))]); }
        build(blocks, &m)
    };
    let (la, lb) = (deep(&mut blocks, "one"), deep(&mut blocks, "two"));
    let seen = compared(&mut blocks, Some(&la), Some(&lb)).unwrap();
    assert_eq!(seen[0].0.len(), DEPTH * (NAME_MAX + 1) - 1);
    // A visit's error ends the comparison, and is its answer.
    let both = (build(&mut blocks, &d(&[("x", f("1")), ("y", f("1"))])), build(&mut blocks, &d(&[("x", f("2")), ("y", f("2"))])));
    let mut visits = 0;
    let mut left = u64::MAX;
    assert_eq!(diff(&mut blocks, Some(&both.0), Some(&both.1), [&mut one, &mut two], &mut path, &mut left, |_, _| { visits += 1; Err(Error::Store) }), Err(Error::Store));
    assert_eq!(visits, 1);
}

/// The files `files` lists under `root`: each path, size and CID; or its error.
fn listing(blocks: &mut Memory, root: &Cid) -> Result<Vec<(String, u64, Cid)>, Error> {
    let mut buffer = Box::new([0u8; CHUNK]);
    let mut path = vec![0u8; PATH_MAX];
    let mut left = u64::MAX;
    let mut out = Vec::new();
    files(blocks, root, &mut buffer, &mut path, &mut left, |_, path, size, cid| { out.push((String::from_utf8(path.to_vec()).unwrap(), size, *cid)); Ok(()) })?;
    Ok(out)
}

/// The files of a model, as `files` should list them: depth first, in the names' order.
fn model_files(blocks: &mut Memory, dir: &BTreeMap<String, M>, prefix: &str, out: &mut Vec<(String, u64, Cid)>) {
    for (name, m) in dir {
        let path = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        match m {
            M::F(_) => { let s = side_of(blocks, m); out.push((path, s.size, s.cid)) }
            M::D(entries) => model_files(blocks, entries, &path, out),
        }
    }
}

#[test]
fn a_trees_files_are_listed_with_their_paths_and_read_back() {
    let (mut blocks, root, _, [_, small, big]) = sample();
    let listed = listing(&mut blocks, &root).unwrap();
    let paths: Vec<&str> = listed.iter().map(|(p, _, _)| p.as_str()).collect();
    // The empty directory lists nothing; a file named twice is listed at each path.
    assert_eq!(paths, ["a.txt", "big.bin", "same.txt", "sub/b.txt", "sub/deeper/c.txt", "sub/deeper/nothing"]);
    assert_eq!((listed[0].2, listed[2].2, listed[1].2), (small, small, big));
    // Each file read through its CID gives its bytes, the size its entry gave.
    let expected = [pattern(1, 100), pattern(4, 3 * CHUNK + 5), pattern(1, 100), pattern(3, CHUNK), pattern(2, 10), Vec::new()];
    let mut buffer = Box::new([0u8; CHUNK]);
    for ((path, size, cid), want) in listed.iter().zip(expected) {
        let mut out = vec![0u8; *size as usize];
        let mut done = 0;
        while done < out.len() { done += dag::read_at(&mut blocks, cid, done as u64, &mut out[done..], &mut buffer).unwrap(); }
        assert_eq!(out, want, "{path}");
    }
    // A visit's error ends the listing, and is its answer.
    let mut path = vec![0u8; PATH_MAX];
    let (mut left, mut visits) = (u64::MAX, 0);
    assert_eq!(files(&mut blocks, &root, &mut buffer, &mut path, &mut left, |_, _, _, _| { visits += 1; Err(Error::Store) }), Err(Error::Store));
    assert_eq!(visits, 1);
}

#[test]
fn random_trees_list_their_files_as_their_models() {
    let mut rng = Rng(351_0006);
    let mut seen = 0;
    for _ in 0..300 {
        let mut blocks = Memory::default();
        let model = random_dir(&mut rng, 0);
        let root = build(&mut blocks, &model);
        let mut expected = Vec::new();
        model_files(&mut blocks, &model, "", &mut expected);
        assert_eq!(listing(&mut blocks, &root).unwrap(), expected, "{model:?}");
        seen += expected.len();
    }
    assert!(seen > 300, "{seen}");
}

#[test]
fn a_listing_checks_each_directory_and_keeps_to_its_bounds() {
    let (mut blocks, root, _, [sub, _, _]) = sample();
    // A directory that is missing, or answered with other bytes.
    let mut gone = Memory { blocks: blocks.blocks.clone(), ..Default::default() };
    gone.blocks.remove(&sub);
    assert_eq!(listing(&mut gone, &root), Err(Error::NotFound));
    blocks.replace.insert(sub, put_dir_bytes(&[]));
    assert_eq!(listing(&mut blocks, &root), Err(Error::Corrupt));
    blocks.replace.clear();
    // An entry that gives a directory another count than it has, and a file's object where the root belongs.
    let inner = put_dir(&mut blocks, &[]);
    let wrong = put_dir(&mut blocks, &[Entry { name: b"d", kind: Kind::Directory, size: 1, cid: inner }]);
    assert_eq!(listing(&mut blocks, &wrong), Err(Error::Shape));
    let (raw, _) = store(&mut blocks, b"a file");
    assert_eq!(listing(&mut blocks, &raw), Err(Error::Shape));
    // DEPTH directories hold a file at the bottom; one more is past the depth.
    let chain = |blocks: &mut Memory, levels: usize| {
        let mut m = d(&[("f", f("bottom"))]);
        for _ in 1..levels { m = d(&[("dddd", M::D(m))]); }
        build(blocks, &m)
    };
    let deep = chain(&mut blocks, DEPTH);
    let listed = listing(&mut blocks, &deep).unwrap();
    assert_eq!(listed[0].0, format!("{}f", "dddd/".repeat(DEPTH - 1)));
    let past = chain(&mut blocks, DEPTH + 1);
    assert_eq!(listing(&mut blocks, &past), Err(Error::Shape));
    // The steps: each directory of n entries read n + 1 times, and each entry.
    let needed = (DEPTH * 3) as u64;
    let mut buffer = Box::new([0u8; CHUNK]);
    let mut path = vec![0u8; PATH_MAX];
    let mut left = needed;
    assert_eq!(files(&mut blocks, &deep, &mut buffer, &mut path, &mut left, |_, _, _, _| Ok(())), Ok(1));
    assert_eq!(left, 0);
    let mut left = needed - 1;
    assert_eq!(files(&mut blocks, &deep, &mut buffer, &mut path, &mut left, |_, _, _, _| Ok(())), Err(Error::TooLarge));
    // A path buffer one byte short of the file's path, and one that just holds it.
    let mut short = vec![0u8; listed[0].0.len() - 1];
    let mut left = u64::MAX;
    assert_eq!(files(&mut blocks, &deep, &mut buffer, &mut short, &mut left, |_, _, _, _| Ok(())), Err(Error::TooLarge));
    let mut exact = vec![0u8; listed[0].0.len()];
    assert_eq!(files(&mut blocks, &deep, &mut buffer, &mut exact, &mut left, |_, _, _, _| Ok(())), Ok(1));
}
