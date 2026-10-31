//! Host tests of libmind/src/worktree.rs (issue 311-STO-0029; MC-4.9, 4.10): a directory tree changed in memory and
//! published by compare-and-swap, through the block store's own logic (blockstore/src/store.rs on a simulated medium).
//! Every edit, its refusals and the reads that see it; the bounds refused with `Full` and nothing changed; a
//! write-out's root walks, and what no edit touched keeps its CID; of two writers from one version the second is told
//! of the conflict and publishes nothing; a medium stopped at each write of a publication leaves the old root or the
//! new one, never a mix. A file replaced in one edit, for storefs (311-STO-0032).
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

use cid::{Cid, Codec};
use dag::{Blocks, Builder, CHUNK};
use std::collections::BTreeMap;
use store::{hmac_sha256, Crypto, Device, Entry, Extent, Head, Pin, Store, BUFFER, LEASE_NS, SECTOR};
use tree::Kind::{self, Directory, File};
use tree::DEPTH;
use worktree::{Error, Found, Worktree, BYTES, DIRS};

const SECTORS: usize = 8192;

/// A medium in memory. From write `stop` on (counted from the first), no write is made and no flush done; with `torn`,
/// that write makes its first sector alone.
struct Memory { data: Vec<u8>, writes: usize, stop: Option<usize>, torn: bool }
impl Memory {
    fn new() -> Self { Memory { data: vec![0; SECTORS * SECTOR], writes: 0, stop: None, torn: false } }
    fn stopped(&self) -> bool { self.stop.is_some_and(|s| self.writes >= s) }
}
impl Device for &mut Memory {
    fn sectors(&self) -> u64 { (self.data.len() / SECTOR) as u64 }
    fn writable(&self) -> bool { true }
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool { let at = lba as usize * SECTOR; out.copy_from_slice(&self.data[at..at + out.len()]); true }
    fn write(&mut self, lba: u64, data: &[u8]) -> bool {
        self.writes += 1;
        let at = lba as usize * SECTOR;
        if self.stopped() {
            if self.torn && self.stop == Some(self.writes) { self.data[at..at + SECTOR].copy_from_slice(&data[..SECTOR]); }
            return false;
        }
        self.data[at..at + data.len()].copy_from_slice(data);
        true
    }
    fn flush(&mut self) -> bool { !self.stopped() }
}

/// Records sealed by a tag alone (their bytes left plain): the working tree needs the store's logic, not its cipher,
/// which tests/blockstore_host.rs covers.
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

/// The store, counting the blocks put through it.
struct Counted<'s, 'a>(&'s mut Local<'a>, usize);
impl Blocks for Counted<'_, '_> {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, dag::Error> { self.1 += 1; Blocks::put(self.0, codec, data) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> { Blocks::get(self.0, cid, out) }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> { Blocks::has(self.0, cid) }
}

fn pattern(seed: usize, len: usize) -> Vec<u8> { (0..len).map(|i| ((i * 31 + seed * 7 + 1) % 251) as u8).collect() }
fn unhex(text: &str) -> Vec<u8> { (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect() }

/// Stores `data` as an object: its root and size.
fn object(store: &mut Local, data: &[u8]) -> (Cid, u64) {
    let mut builder = Box::new(Builder::new());
    builder.write(store, data).unwrap();
    builder.finish(store).unwrap()
}

/// Every path under the directory `root` with its kind, size and CID, read with `tree::decode` alone.
fn contents(store: &mut Local, root: &Cid) -> BTreeMap<String, (Kind, u64, Cid)> {
    let (mut out, mut todo, mut node) = (BTreeMap::new(), vec![(String::new(), *root)], vec![0u8; CHUNK]);
    while let Some((path, cid)) = todo.pop() {
        let len = store.get(&cid, &mut node).unwrap();
        let dir = tree::decode(&node[..len]).unwrap();
        for i in 0..dir.len() {
            let e = dir.entry(i);
            let at = format!("{path}/{}", std::str::from_utf8(e.name).unwrap());
            if e.kind == Directory { todo.push((at.clone(), e.cid)); }
            out.insert(at, (e.kind, e.size, e.cid));
        }
    }
    out
}

/// Whether every block under `root` is stored, by `tree::walk`; the bytes of its files.
fn complete(store: &mut Local, root: &Cid) -> u64 {
    let mut buffer = Box::new([0u8; CHUNK]);
    tree::walk(store, root, &mut buffer, 100_000, |s, cid| if Store::has(s, cid) { Ok(()) } else { Err(dag::Error::NotFound) }).unwrap()
}

/// What a working tree holds and reads: every path under its root.
fn state(wt: &mut Worktree, store: &mut Local) -> ((usize, usize), BTreeMap<String, Found>) {
    let (mut out, mut todo) = (BTreeMap::new(), vec![String::new()]);
    while let Some(dir) = todo.pop() {
        let mut found = Vec::new();
        wt.list(store, &dir, |name, f| found.push((format!("{dir}/{}", std::str::from_utf8(name).unwrap()), f))).unwrap();
        for (path, f) in found {
            if f.kind == Directory { todo.push(path.clone()); }
            out.insert(path, f);
        }
    }
    (wt.held(), out)
}

/// An edit; refused, it has changed nothing the tree holds or reads.
fn attempt(wt: &mut Worktree, store: &mut Local, edit: impl FnOnce(&mut Worktree, &mut Local) -> Result<(), Error>) -> Result<(), Error> {
    let before = state(wt, store);
    let done = edit(wt, store);
    if let Err(e) = done { assert_eq!(state(wt, store), before, "{e:?}"); }
    done
}

fn refused(wt: &mut Worktree, store: &mut Local, error: Error, edit: impl FnOnce(&mut Worktree, &mut Local) -> Result<(), Error>) {
    assert_eq!(attempt(wt, store, edit), Err(error));
}

fn file(cid: Cid, size: u64) -> Found { Found { kind: File, size, cid: Some(cid) } }
fn held(size: u64) -> Found { Found { kind: Directory, size, cid: None } }

#[test]
fn each_edit_is_read_at_once_and_refused_against_its_rules() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let (a, _) = object(&mut store, b"alpha");
    let (big, big_size) = object(&mut store, &pattern(1, 3 * CHUNK + 1));
    let mut wt = Box::new(Worktree::new());
    // The empty tree: a root of no entries, nothing held.
    assert_eq!(wt.lookup(&mut store, "/"), Ok(held(0)));
    assert_eq!(state(&mut wt, &mut store), ((0, 0), BTreeMap::new()));
    assert_eq!(wt.lookup(&mut store, "docs"), Err(Error::NotFound));
    // Each edit is read at once; the directories it changed are held, with no CID until written out.
    wt.mkdir(&mut store, "docs").unwrap();
    wt.put(&mut store, "docs/a.txt", &a, 5).unwrap();
    wt.put(&mut store, "/big.bin", &big, big_size).unwrap();
    wt.mkdir(&mut store, "docs/deep").unwrap();
    wt.mkdir(&mut store, "docs/deep/deeper").unwrap();
    assert_eq!(wt.lookup(&mut store, "docs/a.txt"), Ok(file(a, 5)));
    assert_eq!(wt.lookup(&mut store, "/big.bin"), Ok(file(big, big_size)));
    assert_eq!(wt.lookup(&mut store, "docs"), Ok(held(2)));
    assert_eq!(wt.lookup(&mut store, ""), Ok(held(2)));
    let mut listed = Vec::new();
    assert_eq!(wt.list(&mut store, "docs", |name, f| listed.push((name.to_vec(), f))), Ok(2));
    assert_eq!(listed, [(b"a.txt".to_vec(), file(a, 5)), (b"deep".to_vec(), held(1))]);
    assert_eq!(wt.list(&mut store, "docs/a.txt", |_, _| {}), Err(Error::NotDirectory));
    assert_eq!(wt.held().0, 4);

    // Refused, each with nothing changed: a missing parent, a file where a directory belongs, a name taken.
    let ghost = Cid::raw(b"never stored");
    refused(&mut wt, &mut store, Error::NotFound, |w, s| w.put(s, "none/x", &a, 5));
    refused(&mut wt, &mut store, Error::NotDirectory, |w, s| w.put(s, "docs/a.txt/x", &a, 5));
    refused(&mut wt, &mut store, Error::Exists, |w, s| w.put(s, "docs/a.txt", &a, 5));
    refused(&mut wt, &mut store, Error::Exists, |w, s| w.put(s, "docs/deep", &a, 5));
    // A file the store does not hold, or not of the size given.
    refused(&mut wt, &mut store, Error::Store(dag::Error::NotFound), |w, s| w.put(s, "docs/b", &ghost, 12));
    refused(&mut wt, &mut store, Error::Store(dag::Error::Shape), |w, s| w.put(s, "docs/b", &a, 6));
    refused(&mut wt, &mut store, Error::Store(dag::Error::Shape), |w, s| w.put(s, "docs/b", &big, big_size - 1));
    refused(&mut wt, &mut store, Error::Exists, |w, s| w.mkdir(s, "docs"));
    refused(&mut wt, &mut store, Error::Exists, |w, s| w.mkdir(s, "big.bin"));
    refused(&mut wt, &mut store, Error::NotFound, |w, s| w.mkdir(s, "none/x"));
    refused(&mut wt, &mut store, Error::NotDirectory, |w, s| w.mkdir(s, "big.bin/x"));
    refused(&mut wt, &mut store, Error::NotEmpty, |w, s| w.remove(s, "docs"));
    refused(&mut wt, &mut store, Error::NotFound, |w, s| w.remove(s, "docs/none"));
    refused(&mut wt, &mut store, Error::NotDirectory, |w, s| w.remove(s, "big.bin/x"));
    refused(&mut wt, &mut store, Error::Inside, |w, s| w.rename(s, "docs", "docs/docs"));
    refused(&mut wt, &mut store, Error::Inside, |w, s| w.rename(s, "docs", "docs/deep/docs"));
    refused(&mut wt, &mut store, Error::Exists, |w, s| w.rename(s, "docs", "docs"));
    refused(&mut wt, &mut store, Error::Exists, |w, s| w.rename(s, "big.bin", "docs"));
    refused(&mut wt, &mut store, Error::NotFound, |w, s| w.rename(s, "none", "x"));
    refused(&mut wt, &mut store, Error::NotFound, |w, s| w.rename(s, "big.bin", "none/big.bin"));
    refused(&mut wt, &mut store, Error::NotDirectory, |w, s| w.rename(s, "docs/a.txt", "big.bin/a.txt"));
    // Names `tree` refuses, and the root where an edit needs an entry.
    let long = "n".repeat(256);
    for bad in ["", "/", "a//b", "docs/", "//docs", ".", "docs/..", "a\0b", "docs/../x", &long] {
        refused(&mut wt, &mut store, Error::Name, |w, s| w.put(s, bad, &a, 5));
        refused(&mut wt, &mut store, Error::Name, |w, s| w.mkdir(s, bad));
        refused(&mut wt, &mut store, Error::Name, |w, s| w.remove(s, bad));
        refused(&mut wt, &mut store, Error::Name, |w, s| w.rename(s, bad, "x"));
        refused(&mut wt, &mut store, Error::Name, |w, s| w.rename(s, "big.bin", bad));
    }
    for bad in ["a//b", "docs/", ".", "a\0b"] { assert_eq!(wt.lookup(&mut store, bad), Err(Error::Name)); }

    // Moves: within a directory, across directories, and a held directory with what it holds.
    wt.rename(&mut store, "docs/a.txt", "docs/b.txt").unwrap();
    assert_eq!((wt.lookup(&mut store, "docs/a.txt"), wt.lookup(&mut store, "docs/b.txt")), (Err(Error::NotFound), Ok(file(a, 5))));
    wt.rename(&mut store, "big.bin", "docs/deep/big.bin").unwrap();
    assert_eq!((wt.lookup(&mut store, ""), wt.lookup(&mut store, "docs/deep")), (Ok(held(1)), Ok(held(2))));
    wt.mkdir(&mut store, "other").unwrap();
    wt.rename(&mut store, "docs/deep", "other/moved").unwrap();
    assert_eq!(wt.lookup(&mut store, "other/moved/big.bin"), Ok(file(big, big_size)));
    assert_eq!((wt.lookup(&mut store, "other/moved/deeper"), wt.lookup(&mut store, "docs")), (Ok(held(0)), Ok(held(1))));
    assert_eq!(wt.lookup(&mut store, "docs/deep"), Err(Error::NotFound));
    // Removals: an empty directory, whose held node is let go, and a file.
    let count = wt.held().0;
    wt.remove(&mut store, "other/moved/deeper").unwrap();
    assert_eq!((wt.held().0, wt.lookup(&mut store, "other/moved")), (count - 1, Ok(held(1))));
    wt.remove(&mut store, "docs/b.txt").unwrap();
    assert_eq!(wt.lookup(&mut store, "docs"), Ok(held(0)));

    // Published, the tree reads back from the store alone as the edits left it.
    let (version, root) = wt.publish(&mut store, "home").unwrap();
    assert_eq!((version, store.resolve(b"home")), (1, Ok((1, root))));
    let tree = contents(&mut store, &root);
    assert_eq!(tree.keys().map(String::as_str).collect::<Vec<_>>(), ["/docs", "/other", "/other/moved", "/other/moved/big.bin"]);
    assert_eq!((tree["/docs"].1, tree["/other"].1, tree["/other/moved"].1, tree["/other/moved/big.bin"]), (0, 1, 1, (File, big_size, big)));
    // Nothing is held then: reads go to the store, and give every CID.
    assert_eq!(wt.held(), (0, 0));
    assert_eq!(wt.base(), (Some(root), 1));
    assert_eq!(wt.lookup(&mut store, ""), Ok(Found { kind: Directory, size: 2, cid: Some(root) }));
    assert_eq!(wt.lookup(&mut store, "other/moved"), Ok(Found { kind: Directory, size: 1, cid: Some(tree["/other/moved"].2) }));
    // Refused deep in a stored tree, an edit lets go of the directories it held on the way: a put, a move down two paths.
    refused(&mut wt, &mut store, Error::Exists, |w, s| w.put(s, "other/moved/big.bin", &a, 5));
    refused(&mut wt, &mut store, Error::NotFound, |w, s| w.rename(s, "other/moved/big.bin", "docs/none/big.bin"));
    assert_eq!(wt.held(), (0, 0));
    // An edit in a stored tree holds the directories down to it, and no others; a stored empty directory is removed.
    wt.put(&mut store, "other/moved/a.txt", &a, 5).unwrap();
    assert_eq!(wt.held().0, 3);
    wt.remove(&mut store, "docs").unwrap();
    assert_eq!((wt.held().0, wt.lookup(&mut store, "docs")), (3, Err(Error::NotFound)));
    // A directory is not a file to put.
    refused(&mut wt, &mut store, Error::Store(dag::Error::Format), |w, s| w.put(s, "x", &root, 2));
    // A tree whose root is a file has no directory to read or change.
    for (cid, size) in [(a, 5), (big, big_size)] {
        let mut other = Box::new(Worktree::new());
        other.start(Some(cid), 1);
        assert_eq!(other.list(&mut store, "", |_, _| {}), Err(Error::NotDirectory), "{size}");
        assert_eq!(other.lookup(&mut store, "x"), Err(Error::NotDirectory));
        assert_eq!((other.mkdir(&mut store, "x"), other.held()), (Err(Error::NotDirectory), (0, 0)));
    }
}

#[test]
fn a_replace_puts_a_new_version_of_a_file_in_one_edit() {
    // 311-STO-0032: storefs stores a written file in place of the one it read.
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let (a, _) = object(&mut store, b"alpha");
    let (big, big_size) = object(&mut store, &pattern(2, 2 * CHUNK + 9));
    let mut wt = Box::new(Worktree::new());
    for dir in ["docs", "docs/deep"] { wt.mkdir(&mut store, dir).unwrap(); }
    wt.put(&mut store, "docs/a", &a, 5).unwrap();
    wt.put(&mut store, "docs/z", &a, 5).unwrap();
    // In place of a file: the entries keep their number and order.
    wt.replace(&mut store, "docs/a", &big, big_size).unwrap();
    assert_eq!((wt.lookup(&mut store, "docs/a"), wt.lookup(&mut store, "docs")), (Ok(file(big, big_size)), Ok(held(3))));
    let mut names = Vec::new();
    wt.list(&mut store, "docs", |name, _| names.push(name.to_vec())).unwrap();
    assert_eq!(names, [b"a".to_vec(), b"deep".to_vec(), b"z".to_vec()]);
    // Under a name not taken, as a put.
    wt.replace(&mut store, "docs/deep/new", &a, 5).unwrap();
    assert_eq!((wt.lookup(&mut store, "docs/deep/new"), wt.lookup(&mut store, "docs/deep")), (Ok(file(a, 5)), Ok(held(1))));
    // Refused, each with nothing changed: a directory there, a missing parent, a file where a directory belongs, an
    // object not stored or not of the size given, a name refused, the root.
    refused(&mut wt, &mut store, Error::Exists, |w, s| w.replace(s, "docs/deep", &a, 5));
    refused(&mut wt, &mut store, Error::NotFound, |w, s| w.replace(s, "none/a", &a, 5));
    refused(&mut wt, &mut store, Error::NotDirectory, |w, s| w.replace(s, "docs/a/x", &a, 5));
    refused(&mut wt, &mut store, Error::Store(dag::Error::NotFound), |w, s| w.replace(s, "docs/a", &Cid::raw(b"never stored"), 12));
    refused(&mut wt, &mut store, Error::Store(dag::Error::Shape), |w, s| w.replace(s, "docs/a", &a, 4));
    refused(&mut wt, &mut store, Error::Name, |w, s| w.replace(s, "docs/..", &a, 5));
    refused(&mut wt, &mut store, Error::Name, |w, s| w.replace(s, "", &a, 5));
    // Published, then replaced in the stored tree: the directories down to it are written, and every other CID stays.
    let (_, root) = wt.publish(&mut store, "home").unwrap();
    let before = contents(&mut store, &root);
    wt.replace(&mut store, "docs/deep/new", &big, big_size).unwrap();
    assert_eq!(wt.held().0, 3);
    let (_, root) = wt.publish(&mut store, "home").unwrap();
    let after = contents(&mut store, &root);
    assert_eq!(after["/docs/deep/new"], (File, big_size, big));
    for (path, entry) in &before {
        if !["/docs", "/docs/deep", "/docs/deep/new"].contains(&path.as_str()) { assert_eq!(after[path], *entry, "{path}"); }
    }
    assert_eq!(complete(&mut store, &root), 2 * big_size + 5);
    // Past DIRS, a stored directory is not held for a replace either.
    let mut wt = Box::new(Worktree::new());
    wt.start(Some(root), 2);
    for k in 0..DIRS - 1 { wt.mkdir(&mut store, &format!("d{k:02}")).unwrap(); }
    refused(&mut wt, &mut store, Error::Full, |w, s| w.replace(s, "docs/z", &big, big_size));
    wt.replace(&mut store, "d00/z", &big, big_size).unwrap();
}

#[test]
fn the_bounds_are_refused_with_full_and_nothing_changed() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let (a, _) = object(&mut store, b"alpha");

    // DIRS: the root and 63 directories made; one more is refused, an edit in those held is not.
    let mut wt = Box::new(Worktree::new());
    for k in 0..DIRS - 1 { wt.mkdir(&mut store, &format!("d{k:02}")).unwrap(); }
    assert_eq!(wt.held().0, DIRS);
    refused(&mut wt, &mut store, Error::Full, |w, s| w.mkdir(s, "one-more"));
    wt.put(&mut store, "d00/a", &a, 5).unwrap();
    // A publication lets them go, and the edit refused is made.
    assert_eq!(wt.publish(&mut store, "dirs").map(|p| p.0), Ok(1));
    wt.mkdir(&mut store, "one-more").unwrap();
    assert_eq!(wt.held().0, 2);
    // Directories held to be changed count as well: past DIRS, a stored one is not held.
    for k in 0..DIRS - 2 { wt.put(&mut store, &format!("d{k:02}/b"), &a, 5).unwrap(); }
    assert_eq!(wt.held().0, DIRS);
    refused(&mut wt, &mut store, Error::Full, |w, s| w.put(s, "d62/b", &a, 5));

    // BYTES: directories of names of 255 bytes. A directory takes 54 before it would pass a block (TooLarge); the fifth
    // directory's first is past BYTES (Full).
    let mut wt = Box::new(Worktree::new());
    let (mut made, mut full) = (0, None);
    while full.is_none() {
        let dir = format!("b{made}");
        wt.mkdir(&mut store, &dir).unwrap();
        made += 1;
        for k in 0.. {
            let path = format!("{dir}/{k:03}{}", "x".repeat(252));
            match attempt(&mut wt, &mut store, |w, s| w.put(s, &path, &a, 5)) {
                Ok(()) => {}
                Err(Error::TooLarge) => { assert_eq!(k, 54); break; }
                Err(Error::Full) => { full = Some(path); break; }
                Err(e) => panic!("{e:?}"),
            }
        }
    }
    let bytes = wt.held().1;
    assert!(made == 5 && bytes <= BYTES && bytes + 301 > BYTES, "{made} directories, {bytes} bytes");
    wt.publish(&mut store, "bytes").unwrap();
    wt.put(&mut store, &full.unwrap(), &a, 5).unwrap();

    // tree::DEPTH: 31 directories below the root; a 32nd is refused, and so is a directory moved to its place.
    let mut wt = Box::new(Worktree::new());
    let mut path = String::from("n");
    wt.mkdir(&mut store, &path).unwrap();
    for _ in 1..DEPTH - 1 {
        path.push_str("/n");
        wt.mkdir(&mut store, &path).unwrap();
    }
    refused(&mut wt, &mut store, Error::TooLarge, |w, s| w.mkdir(s, &format!("{path}/n")));
    wt.mkdir(&mut store, "m").unwrap();
    refused(&mut wt, &mut store, Error::TooLarge, |w, s| w.rename(s, "m", &format!("{path}/m")));
    // A file goes there, and the store's walk takes the tree.
    wt.put(&mut store, &format!("{path}/f"), &a, 5).unwrap();
    let (_, root) = wt.publish(&mut store, "deep").unwrap();
    assert_eq!(complete(&mut store, &root), 5);
}

#[test]
fn a_write_out_walks_and_what_no_edit_touched_keeps_its_cid() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let files: Vec<(Cid, u64)> = [10, CHUNK, 2 * CHUNK + 3, 0, 700].iter().enumerate().map(|(k, &len)| object(&mut store, &pattern(k, len))).collect();
    // The empty tree writes out as the empty directory, put.
    let mut wt = Box::new(Worktree::new());
    let empty = wt.write_out(&mut store).unwrap();
    assert_eq!(empty, Cid::of(Codec::DagCbor, &unhex("a261760167656e747269657380")));
    assert!(store.has(&empty));
    // A tree of three levels.
    let mut wt = Box::new(Worktree::new());
    for dir in ["left", "left/inner", "right", "right/sub", "empty"] { wt.mkdir(&mut store, dir).unwrap(); }
    for (path, k) in [("a", 0), ("left/l", 1), ("left/inner/i", 2), ("right/r", 3), ("right/sub/s", 4)] { wt.put(&mut store, path, &files[k].0, files[k].1).unwrap(); }
    let (_, root) = wt.publish(&mut store, "home").unwrap();
    let before = contents(&mut store, &root);
    // Read with no edit, it writes out as its root, with nothing put.
    let mut counted = Counted(&mut store, 0);
    assert_eq!(wt.write_out(&mut counted), Ok(root));
    assert_eq!(counted.1, 0);
    // An edit deep in one subtree: the directories down to it are written, and no others.
    wt.put(&mut store, "right/sub/new", &files[0].0, files[0].1).unwrap();
    let mut counted = Counted(&mut store, 0);
    let new = wt.write_out(&mut counted).unwrap();
    assert_eq!(counted.1, 3);
    let after = contents(&mut store, &new);
    assert_eq!((after.len(), after["/right/sub/new"]), (before.len() + 1, (File, files[0].1, files[0].0)));
    for (path, (kind, size, cid)) in &before {
        let on_the_way = ["/right", "/right/sub"].contains(&path.as_str());
        assert_eq!(after[path].2 == *cid, !on_the_way, "{path}");
        assert_eq!((after[path].0, after[path].1), (*kind, if path == "/right/sub" { size + 1 } else { *size }), "{path}");
    }
    // The new root decodes and walks: every block under it is stored, files' chunks included, and its bytes are the files'.
    let mut buffer = Box::new([0u8; CHUNK]);
    let mut visited = Vec::new();
    let total = tree::walk(&mut store, &new, &mut buffer, 10_000, |s, cid| { assert!(Store::has(s, cid)); visited.push(*cid); Ok(()) }).unwrap();
    assert_eq!(total, files.iter().map(|f| f.1).sum::<u64>() + files[0].1);
    assert!(visited.contains(&new) && visited.contains(&after["/left"].2) && visited.contains(&files[2].0));
    // Still held, the edited directories have no CID until published; a second write-out gives the same root.
    assert_eq!(wt.lookup(&mut store, "right"), Ok(held(2)));
    assert_eq!(wt.write_out(&mut store), Ok(new));
}

#[test]
fn of_two_writers_from_one_version_the_second_is_told_of_the_conflict_and_publishes_nothing() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let (a, _) = object(&mut store, b"alpha");
    // Both from a name that does not exist yet: the first makes it.
    let (mut first, mut second) = (Box::new(Worktree::new()), Box::new(Worktree::new()));
    first.mkdir(&mut store, "docs").unwrap();
    second.mkdir(&mut store, "other").unwrap();
    let (v1, r1) = first.publish(&mut store, "home").unwrap();
    assert_eq!(v1, 1);
    assert_eq!(second.publish(&mut store, "home"), Err(Error::Conflict { read: 0, current: 1 }));
    assert_eq!(store.resolve(b"home"), Ok((1, r1)));
    // Both from version 1: the first publishes, the second is refused.
    second.start(Some(r1), 1);
    first.put(&mut store, "docs/a", &a, 5).unwrap();
    second.put(&mut store, "docs/b", &a, 5).unwrap();
    let (v2, r2) = first.publish(&mut store, "home").unwrap();
    assert_eq!(v2, 2);
    let lost = second.write_out(&mut store).unwrap();
    assert_eq!(second.publish(&mut store, "home"), Err(Error::Conflict { read: 1, current: 2 }));
    // Nothing of it is published: the name is at the first writer's root, with the two versions it keeps.
    assert_eq!(store.resolve(b"home"), Ok((2, r2)));
    let kept: Vec<_> = store.history(b"home").unwrap().iter().map(|v| (v.version, v.root())).collect();
    assert_eq!(kept, [(2, Some(r2)), (1, Some(r1))]);
    // The second tree stays as it was, never merged.
    assert_eq!(second.base(), (Some(r1), 1));
    assert_eq!(second.lookup(&mut store, "docs/b"), Ok(file(a, 5)));
    assert_eq!(second.lookup(&mut store, "docs/a"), Err(Error::NotFound));
    // Its nodes are retained by nothing: once their lease ends, a collection frees them; every version kept stays whole.
    assert!(store.has(&lost));
    store.set_time(LEASE_NS + 1);
    store.collect().unwrap();
    assert!(!store.has(&lost));
    assert_eq!((complete(&mut store, &r1), complete(&mut store, &r2)), (0, 5));
    // Its writer decides what to do: here it reads the new version and makes its edit again.
    second.start(Some(r2), 2);
    second.put(&mut store, "docs/b", &a, 5).unwrap();
    let (v3, r3) = second.publish(&mut store, "home").unwrap();
    assert_eq!(v3, 3);
    assert_eq!(contents(&mut store, &r3).keys().map(String::as_str).collect::<Vec<_>>(), ["/docs", "/docs/a", "/docs/b"]);
}

/// Xorshift: the same edits on every run.
struct Rng(u64);
impl Rng {
    fn below(&mut self, n: usize) -> usize {
        (self.0 ^= self.0 << 13, self.0 ^= self.0 >> 7, self.0 ^= self.0 << 17);
        (self.0 % n as u64) as usize
    }
    // A path of one to three names of three.
    fn path(&mut self) -> String { (0..1 + self.below(3)).map(|_| ["a", "b", "c"][self.below(3)]).collect::<Vec<_>>().join("/") }
}

// A model of a tree: every path, with a file's CID and size, or none for a directory.
type Model = BTreeMap<String, Option<(Cid, u64)>>;

// What the model refuses on the way down to `path`'s parent.
fn parent(model: &Model, path: &str) -> Result<(), Error> {
    let names: Vec<&str> = path.split('/').collect();
    for k in 1..names.len() {
        match model.get(&names[..k].join("/")) { None => return Err(Error::NotFound), Some(Some(_)) => return Err(Error::NotDirectory), Some(None) => {} }
    }
    Ok(())
}

fn free(model: &Model, path: &str) -> Result<(), Error> { parent(model, path).and_then(|()| if model.contains_key(path) { Err(Error::Exists) } else { Ok(()) }) }

// The paths under `path`.
fn below<'m>(model: &'m Model, path: &str) -> impl Iterator<Item = &'m String> {
    let prefix = format!("{path}/");
    model.keys().filter(move |k| k.starts_with(&prefix))
}

// Every path as a read gives it: kind, size (a directory's entries) and a file's CID.
fn expected(model: &Model) -> BTreeMap<String, (Kind, u64, Option<Cid>)> {
    model.iter().map(|(path, v)| (path.clone(), match v {
        Some((cid, size)) => (File, *size, Some(*cid)),
        None => (Directory, below(model, path).filter(|k| !k[path.len() + 1..].contains('/')).count() as u64, None),
    })).collect()
}

#[test]
fn random_edits_read_and_publish_as_a_model_of_them_does() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let objects: Vec<(Cid, u64)> = (0..3).map(|k| object(&mut store, &pattern(k, 1 + k * 1000))).collect();
    let (mut model, mut rng, mut wt) = (Model::new(), Rng(0x2545_f491_4f6c_dd1d), Box::new(Worktree::new()));
    for step in 0..2400 {
        let (op, p, f) = (rng.below(5), rng.path(), objects[rng.below(3)]);
        // A move's target: any path, or a name of four in a directory that exists.
        let dirs: Vec<&String> = model.iter().filter(|(_, v)| v.is_none()).map(|(k, _)| k).collect();
        let q = match (rng.below(2), ["a", "b", "c", "d"][rng.below(4)]) {
            (0, _) => rng.path(),
            (_, name) => match rng.below(dirs.len() + 1) { 0 => name.to_string(), k => format!("{}/{name}", dirs[k - 1]) },
        };
        // What the model says, in the order the working tree checks it.
        let wanted = match op {
            0 | 1 => free(&model, &p),
            2 => parent(&model, &p).and_then(|()| match model.get(&p) {
                None => Err(Error::NotFound),
                Some(None) if below(&model, &p).next().is_some() => Err(Error::NotEmpty),
                _ => Ok(()),
            }),
            4 => parent(&model, &p).and_then(|()| if model.get(&p) == Some(&None) { Err(Error::Exists) } else { Ok(()) }),
            _ => parent(&model, &p).and_then(|()| match model.get(&p) {
                None => Err(Error::NotFound),
                Some(None) if q.starts_with(&format!("{p}/")) => Err(Error::Inside),
                _ => free(&model, &q),
            }),
        };
        let held = wt.held();
        let done = match op {
            0 => wt.put(&mut store, &p, &f.0, f.1),
            1 => wt.mkdir(&mut store, &p),
            2 => wt.remove(&mut store, &p),
            3 => wt.rename(&mut store, &p, &q),
            _ => wt.replace(&mut store, &p, &f.0, f.1),
        };
        assert_eq!(done, wanted, "step {step}: {op} {p} {q}");
        match (done, op) {
            (Err(_), _) => assert_eq!(wt.held(), held),
            (Ok(()), 0 | 4) => { model.insert(p, Some(f)); }
            (Ok(()), 1) => { model.insert(p, None); }
            (Ok(()), 2) => { model.remove(&p); }
            (Ok(()), _) => {
                let moved: Vec<String> = below(&model, &p).cloned().chain([p.clone()]).collect();
                for from in moved { let v = model.remove(&from).unwrap(); model.insert(format!("{q}{}", &from[p.len()..]), v); }
            }
        }
        let (_, read) = state(&mut wt, &mut store);
        let read: BTreeMap<_, _> = read.into_iter().map(|(k, f)| (k[1..].to_string(), (f.kind, f.size, if f.kind == File { f.cid } else { None }))).collect();
        assert_eq!(read, expected(&model), "step {step}");
        // Published now and then: the store's tree, read by `tree::decode` alone, is the model's.
        if step % 50 == 49 {
            let (version, root) = wt.publish(&mut store, "random").unwrap();
            let published: BTreeMap<_, _> = contents(&mut store, &root).into_iter().map(|(k, (kind, size, cid))| (k[1..].to_string(), (kind, size, (kind == File).then_some(cid)))).collect();
            assert_eq!(published, expected(&model), "step {step}");
            // Now and then the empty tree again, so the trees stay small and the edits many.
            if step % 400 == 399 {
                wt.start(None, version);
                model.clear();
            }
        }
    }
}

#[test]
fn a_publication_stopped_at_any_write_leaves_the_old_root_or_the_new_never_a_mix() {
    // Version 1 published; then edits in three directories, published as version 2 on a medium that stops at write
    // `k` of that publication (not made, or `torn`: its first sector alone). Names of 200 bytes make each node changed
    // more than a sector. Returns the writes of the publication, what it answered, and the medium.
    let long = |k: usize| format!("{}{k}", "f".repeat(200));
    let run = |stop: Option<(usize, bool)>| -> (usize, Result<(u64, Cid), Error>, Memory) {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let (published, writes);
        {
            let mut store = mount(&mut medium, &mut room);
            let files: Vec<(Cid, u64)> = (0..3).map(|k| object(&mut store, &pattern(k, 100 + k * CHUNK))).collect();
            let mut wt = Box::new(Worktree::new());
            for dir in ["a", "a/b", "c"] { wt.mkdir(&mut store, dir).unwrap(); }
            wt.put(&mut store, &format!("a/b/{}", long(0)), &files[0].0, files[0].1).unwrap();
            wt.put(&mut store, &format!("c/{}", long(9)), &files[0].0, files[0].1).unwrap();
            wt.publish(&mut store, "home").unwrap();
            wt.put(&mut store, &format!("a/b/{}", long(1)), &files[1].0, files[1].1).unwrap();
            wt.put(&mut store, &format!("c/{}", long(2)), &files[2].0, files[2].1).unwrap();
            wt.rename(&mut store, &format!("a/b/{}", long(0)), &long(0)).unwrap();
            let start = store.device().writes;
            if let Some((k, torn)) = stop {
                store.device().stop = Some(start + k);
                store.device().torn = torn;
            }
            published = wt.publish(&mut store, "home");
            writes = store.device().writes - start;
        }
        medium.stop = None;
        (writes, published, medium)
    };
    // A new store on the medium: the name's version and root, its tree whole.
    let mounted = |mut medium: Memory| {
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let (version, root) = store.resolve(b"home").unwrap();
        complete(&mut store, &root);
        (version, root, contents(&mut store, &root))
    };
    // Not stopped: four nodes (a/b, a, c, the root) and the name's record.
    let (writes, published, medium) = run(None);
    let (version, new, new_tree) = mounted(medium);
    assert_eq!((writes, version, published.map(|p| p.1)), (5, 2, Ok(new)));
    let (_, published, medium) = run(Some((1, false)));
    let (version, old, old_tree) = mounted(medium);
    assert_eq!(version, 1);
    assert!(published.is_err() && old != new && old_tree.len() + 2 == new_tree.len());
    let mut newer = 0;
    for k in 1..=writes {
        for torn in [false, true] {
            let (_, published, medium) = run(Some((k, torn)));
            match mounted(medium) {
                (1, root, tree) => assert_eq!((root, tree, published.is_ok()), (old, old_tree.clone(), false), "write {k}, torn {torn}"),
                (2, root, tree) => { assert_eq!((root, tree), (new, new_tree.clone()), "write {k}, torn {torn}"); newer += 1; }
                (v, ..) => panic!("version {v} at write {k}, torn {torn}"),
            }
        }
    }
    // Only the name's record, written whole in its one sector before the medium stopped, publishes the new root.
    assert_eq!(newer, 1);
}
