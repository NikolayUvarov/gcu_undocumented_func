//! Host tests of libmind/src/tree.rs (issue 311-STO-0027; MC-4.2, 4.5, 4.13): directories as DAG-CBOR nodes and the
//! walk of a tree of them. Node bytes are checked against an encoding written here from the schema; every non-canonical
//! node, bad entry and node over a block is refused; a walk visits exactly the blocks of a tree, and refuses a missing
//! block, a changed one, an entry that disagrees with its object and a tree past the depth.
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
use tree::{decode, encode, size, walk, Entry, Kind, DEPTH, NAME_MAX};

/// Blocks in memory; `replace` makes the store answer a CID with other bytes.
#[derive(Default)]
struct Memory { blocks: HashMap<Cid, Vec<u8>>, replace: HashMap<Cid, Vec<u8>> }
impl Blocks for Memory {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, Error> {
        let cid = Cid::of(codec, data);
        self.blocks.insert(cid, data.to_vec());
        Ok(cid)
    }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> {
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

