//! Host tests of storefs/src/fs.rs (issue 311-STO-0032; MC-4.1, 4.7, 4.9): `vfs.wit`'s files and directories over the
//! store's names `fs/<volume>`, through the block store's own logic (blockstore/src/store.rs on a simulated medium).
//! Open, read, write, truncate, list page by page, remove, rename, stat and volume; a file written, flushed and read
//! back by SHA-256, and random writes against a model; the flush of a directory publishes what is under it at once; of
//! two writers from one version the second is told of the conflict and nothing of it is published; a publication stopped
//! at each write of the medium leaves the old root or the new one; a client's handles refused to another, a read-only
//! volume refused a change, and a handle allowing no more than its parent; each bound refused with nothing changed; the
//! errors as `vfs.wit` 2.5 has them.
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
#[path = "../storefs/src/fs.rs"]
mod fs;

/// `vfs.wit` 2.5's errors, as fs.rs maps to them: checked against the generated libmind/src/idl/vfs.rs below.
mod vfs {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Error { NotFound = 0, Exists = 1, NotEmpty = 2, Invalid = 3, Denied = 4, NoSpace = 5, ReadOnly = 6, Io = 7, Handles = 8, Name = 9, NotDirectory = 10, IsDirectory = 11 }
}

use cid::Cid;
use dag::{Builder, CHUNK};
use fs::{Client, Error, Fs, CHUNKS, CREATE, FILES, HANDLES, NEW, PAGE, PATH, TREES, TRUNCATE, WRITE};
use std::collections::BTreeMap;
use store::{hmac_sha256, Crypto, Device, Entry, Extent, Head, Pin, Store, BUFFER, SECTOR};
use tree::Kind::{self, Directory, File};

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

/// Records sealed by a tag alone (their bytes left plain): storefs needs the store's logic, not its cipher, which
/// tests/blockstore_host.rs covers.
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

/// The badge storefs publishes with.
const OWNER: u16 = 9;

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

/// A name's current root, as `root` reads it.
impl<D: Device> fs::Roots for Store<'_, D> {
    fn current(&mut self, name: &str) -> Result<(u64, Option<Cid>), dag::Error> { self.snapshot(name.as_bytes()).map_err(|_| dag::Error::Store) }
}

/// The shell's client, which may write; another client with the same badge; an application, which may not.
const USER: Client = Client { pid: 10, badge: 1 };
const OTHER: Client = Client { pid: 11, badge: 1 };
const APP: Client = Client { pid: 12, badge: 0 };

/// The service's predicate: the user's badge writes every volume, others none.
fn writable(badge: u16, _: &str) -> bool { badge == 1 }

/// Runs a test on a thread whose stack holds the service's state while it is made (786 KiB).
fn run(test: impl FnOnce() + Send + 'static) {
    if let Err(panic) = std::thread::Builder::new().stack_size(64 << 20).spawn(test).unwrap().join() { std::panic::resume_unwind(panic); }
}

/// An entry as `listed` gives it.
fn entry(name: &str, kind: Kind, size: u64) -> (String, Kind, u64) { (name.to_string(), kind, size) }

fn pattern(seed: usize, len: usize) -> Vec<u8> { (0..len).map(|i| ((i * 31 + seed * 7 + 1) % 251) as u8).collect() }

/// Writes `data` at `offset` as a client of `vfs.wit` does: 16 KiB a call at most.
fn write_all(fs: &mut Fs, store: &mut Local, client: Client, file: u32, offset: u64, data: &[u8]) {
    for (k, part) in data.chunks(16384).enumerate() { assert_eq!(fs.write(store, client, file, offset + (k * 16384) as u64, part), Ok(part.len())); }
}

/// A file's bytes, read as a client of `vfs.wit` does, to the size `stat` gives.
fn read_all(fs: &mut Fs, store: &mut Local, client: Client, file: u32) -> Vec<u8> {
    let size = fs.stat(store, client, file).unwrap().size as usize;
    let mut out = vec![0; size];
    let mut done = 0;
    while done < size {
        let n = fs.read(store, client, file, done as u64, &mut out[done..(done + 16384).min(size)]).unwrap();
        assert!(n > 0);
        done += n;
    }
    assert_eq!(fs.read(store, client, file, size as u64, &mut [0; 1]), Ok(0));
    out
}

/// Every entry of a directory, page by page: name, kind, size.
fn listed(fs: &mut Fs, store: &mut Local, client: Client, dir: u32) -> Vec<(String, Kind, u64)> {
    let mut out = Vec::new();
    loop {
        let start = out.len() as u32;
        let n = fs.list(store, client, dir, start, |name, kind, size| out.push((String::from_utf8(name.to_vec()).unwrap(), kind, size))).unwrap();
        assert!(n <= PAGE);
        if n == 0 { return out; }
    }
}

/// Every path under a directory as its tree reads it, the handles opened on the way closed again.
fn seen(fs: &mut Fs, store: &mut Local, client: Client, dir: u32) -> BTreeMap<String, (Kind, u64)> {
    let mut out = BTreeMap::new();
    for (name, kind, size) in listed(fs, store, client, dir) {
        if kind == Directory {
            let sub = fs.open_dir(store, client, dir, &name, false).unwrap();
            for (path, entry) in seen(fs, store, client, sub) { out.insert(format!("{name}/{path}"), entry); }
            fs.close(store, client, sub).unwrap();
        }
        out.insert(name, (kind, size));
    }
    out
}

/// A file's bytes read from the store alone, every block checked.
fn bytes(store: &mut Local, cid: &Cid) -> Vec<u8> {
    let (mut buffer, mut chunk, mut out) = (Box::new([0u8; CHUNK]), Box::new([0u8; CHUNK]), Vec::new());
    dag::each_chunk(store, cid, &mut buffer, &mut chunk, |_, data| { out.extend_from_slice(data); Ok(()) }).unwrap();
    out
}

/// The name's current version, and every path under its root read from the store alone: a directory's entries, or a
/// file's bytes.
fn published(store: &mut Local, name: &str) -> (u64, BTreeMap<String, Result<u64, Vec<u8>>>) {
    let (version, root) = store.snapshot(name.as_bytes()).unwrap();
    let (mut out, mut todo, mut node) = (BTreeMap::new(), root.into_iter().map(|r| (String::new(), r)).collect::<Vec<_>>(), vec![0u8; CHUNK]);
    while let Some((path, cid)) = todo.pop() {
        let len = store.get(&cid, &mut node).unwrap();
        let dir = tree::decode(&node[..len]).unwrap();
        let entries: Vec<_> = (0..dir.len()).map(|i| { let e = dir.entry(i); (String::from_utf8(e.name.to_vec()).unwrap(), e.kind, e.size, e.cid) }).collect();
        for (name, kind, size, cid) in entries {
            let at = if path.is_empty() { name } else { format!("{path}/{name}") };
            match kind {
                Directory => { out.insert(at.clone(), Ok(size)); todo.push((at, cid)); }
                File => { let data = bytes(store, &cid); assert_eq!(data.len() as u64, size); out.insert(at, Err(data)); }
            }
        }
    }
    (version, out)
}

/// Whether every block under `root` is stored, by `tree::walk`.
fn complete(store: &mut Local, root: &Cid) -> u64 {
    let mut buffer = Box::new([0u8; CHUNK]);
    tree::walk(store, root, &mut buffer, 100_000, |s, cid| if Store::has(s, cid) { Ok(()) } else { Err(dag::Error::NotFound) }).unwrap()
}

#[test]
fn each_call_does_what_vfs_wit_says() {
    run(|| {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let mut fs = Box::new(Fs::new());
        let s = &mut store;
        // A volume no name holds yet: the empty tree, its case ignored.
        let r = fs.root(s, USER, "Home", writable).unwrap();
        let v = fs.volume(USER, r).unwrap();
        assert_eq!((v.name(), v.writable, v.version, v.root), ("home", true, 0, None));
        let e = fs.stat(s, USER, r).unwrap();
        assert_eq!((e.name(), e.kind, e.size), (&b""[..], Directory, 0));
        for bad in ["", "a/b", "seventeen-bytes-x", "a b"] { assert_eq!(fs.root(s, USER, bad, writable), Err(Error::NotFound), "{bad:?}"); }

        // Directories: made on the way with `create`, a path without names the directory itself.
        let deep = fs.open_dir(s, USER, r, "/docs//deep/", true).unwrap();
        assert_eq!(fs.stat(s, USER, deep).map(|e| (e.name().to_vec(), e.kind, e.size)), Ok((b"deep".to_vec(), Directory, 0)));
        let docs = fs.open_dir(s, USER, r, "docs", false).unwrap();
        let same = fs.open_dir(s, USER, docs, "", false).unwrap();
        assert_eq!(fs.stat(s, USER, same).unwrap().name(), b"docs");
        assert_eq!(fs.open_dir(s, USER, r, "none", false), Err(Error::NotFound));
        assert_eq!(fs.open_dir(s, USER, r, "docs/..", false), Err(Error::Invalid));
        assert_eq!(fs.open_dir(s, USER, r, "./docs", false), Err(Error::Invalid));

        // A file: written, read, grown with a gap of zeros, cut and grown again.
        let f = fs.open(s, USER, docs, "a.txt", WRITE | CREATE).unwrap();
        assert_eq!(fs.write(s, USER, f, 0, b"hello world"), Ok(11));
        let mut out = [0u8; 32];
        assert_eq!(fs.read(s, USER, f, 6, &mut out), Ok(5));
        assert_eq!(&out[..5], b"world");
        assert_eq!(fs.write(s, USER, f, 20, b"!"), Ok(1));
        assert_eq!(read_all(&mut fs, s, USER, f), b"hello world\0\0\0\0\0\0\0\0\0!");
        fs.truncate(USER, f, 5).unwrap();
        fs.truncate(USER, f, 8).unwrap();
        assert_eq!(read_all(&mut fs, s, USER, f), b"hello\0\0\0");
        let e = fs.stat(s, USER, f).unwrap();
        assert_eq!((e.name(), e.kind, e.size), (&b"a.txt"[..], File, 8));
        // Another handle of the file reads what was written, before it is stored; so does a listing.
        let g = fs.open(s, USER, r, "docs/a.txt", 0).unwrap();
        assert_eq!(read_all(&mut fs, s, USER, g), b"hello\0\0\0");
        assert_eq!(listed(&mut fs, s, USER, docs), [entry("a.txt", File, 8), entry("deep", Directory, 0)]);
        // Refusals.
        assert_eq!(fs.open(s, USER, docs, "a.txt", WRITE | CREATE | NEW), Err(Error::Exists));
        assert_eq!(fs.open(s, USER, r, "docs", 0), Err(Error::IsDirectory));
        assert_eq!(fs.open(s, USER, r, "docs", NEW), Err(Error::Exists));
        assert_eq!(fs.open_dir(s, USER, r, "docs/a.txt", false), Err(Error::NotDirectory));
        assert_eq!(fs.open(s, USER, r, "docs/a.txt/x", 0), Err(Error::NotDirectory));
        assert_eq!(fs.open(s, USER, r, "none/x", WRITE | CREATE), Err(Error::NotFound));
        assert_eq!(fs.open(s, USER, r, "x", NEW), Err(Error::NotFound));
        assert_eq!(fs.open(s, USER, r, "docs/../x", 0), Err(Error::Invalid));
        assert_eq!(fs.open(s, USER, r, "/", 0), Err(Error::Invalid));
        assert_eq!(fs.open(s, USER, r, "a\0b", 0), Err(Error::Name));
        assert_eq!(fs.open(s, USER, f, "x", 0), Err(Error::NotDirectory));
        assert_eq!(fs.read(s, USER, docs, 0, &mut out), Err(Error::IsDirectory));
        assert_eq!(fs.write(s, USER, docs, 0, b"x"), Err(Error::IsDirectory));
        assert_eq!(fs.truncate(USER, docs, 0), Err(Error::IsDirectory));
        assert_eq!(fs.list(s, USER, f, 0, |_, _, _| {}), Err(Error::NotDirectory));
        // Nothing is published until a flush or a close of what was written.
        assert_eq!(s.snapshot(b"fs/home"), Ok((0, None)));
        fs.close(s, USER, f).unwrap();
        let (version, tree) = published(s, "fs/home");
        assert_eq!(version, 1);
        assert_eq!(tree.get("docs/a.txt"), Some(&Err(b"hello\0\0\0".to_vec())));
        assert_eq!(tree.get("docs/deep"), Some(&Ok(0)));
        assert_eq!(fs.volume(USER, g).map(|v| v.version), Ok(1));

        // Forty files made by `open` alone (handles that may not write), then published by one flush of the root.
        let p = fs.open_dir(s, USER, r, "p", true).unwrap();
        for k in 0..40 {
            let h = fs.open(s, USER, p, &format!("{k:02}"), CREATE).unwrap();
            assert_eq!(fs.write(s, USER, h, 0, b"x"), Err(Error::Denied));
            fs.close(s, USER, h).unwrap();
        }
        assert_eq!(s.snapshot(b"fs/home").unwrap().0, 1);
        fs.flush(s, USER, r).unwrap();
        assert_eq!(s.snapshot(b"fs/home").unwrap().0, 2);
        // Listed sixteen at a time, in the order of the names; an empty page is the end.
        let mut names = Vec::new();
        for (start, n) in [(0, 16), (16, 16), (32, 8), (40, 0), (99, 0)] {
            assert_eq!(fs.list(s, USER, p, start, |name, kind, size| names.push((String::from_utf8(name.to_vec()).unwrap(), kind, size))), Ok(n), "from {start}");
        }
        assert_eq!(names, (0..40).map(|k| (format!("{k:02}"), File, 0)).collect::<Vec<_>>());

        // Removals: a file, whose handles are closed; a directory with entries refused; and an empty one.
        fs.remove(s, USER, r, "docs/a.txt").unwrap();
        assert_eq!(fs.stat(s, USER, g), Err(Error::Invalid));
        assert_eq!(fs.remove(s, USER, r, "docs/a.txt"), Err(Error::NotFound));
        assert_eq!(fs.remove(s, USER, r, "p"), Err(Error::NotEmpty));
        assert_eq!(fs.remove(s, USER, r, ""), Err(Error::Invalid));
        fs.remove(s, USER, p, "00").unwrap();
        assert_eq!(listed(&mut fs, s, USER, p).len(), 39);

        // Renames across directory handles; the handles of what moves follow it.
        let f = fs.open(s, USER, docs, "b.txt", WRITE | CREATE).unwrap();
        write_all(&mut fs, s, USER, f, 0, b"bravo");
        fs.rename(s, USER, docs, "b.txt", deep, "c.txt").unwrap();
        assert_eq!(fs.stat(s, USER, f).unwrap().name(), b"c.txt");
        fs.rename(s, USER, r, "docs/deep", p, "moved").unwrap();
        assert_eq!((fs.stat(s, USER, deep).unwrap().name(), fs.stat(s, USER, f).unwrap().name()), (&b"moved"[..], &b"c.txt"[..]));
        assert_eq!(listed(&mut fs, s, USER, deep), [entry("c.txt", File, 5)]);
        assert_eq!(fs.rename(s, USER, r, "p", deep, "p"), Err(Error::Invalid));
        assert_eq!(fs.rename(s, USER, r, "p/01", r, "docs"), Err(Error::Exists));
        assert_eq!(fs.rename(s, USER, r, "none", r, "x"), Err(Error::NotFound));
        assert_eq!(fs.rename(s, USER, docs, "", r, "x"), Err(Error::Invalid));
        // Its close stores it where it is now, and publishes every change.
        fs.close(s, USER, f).unwrap();
        let (version, tree) = published(s, "fs/home");
        assert_eq!(version, 3);
        assert_eq!(tree.get("p/moved/c.txt"), Some(&Err(b"bravo".to_vec())));
        assert_eq!((tree.get("docs"), tree.get("p"), tree.contains_key("p/00")), (Some(&Ok(0)), Some(&Ok(40)), false));
        // A later root of the volume reads it as published.
        let other = fs.root(s, OTHER, "home", writable).unwrap();
        let read = seen(&mut fs, s, OTHER, other);
        assert_eq!(read.len(), tree.len());
        assert!(tree.iter().all(|(path, entry)| read[path] == match entry { Ok(n) => (Directory, *n), Err(data) => (File, data.len() as u64) }));
        for h in [r, deep, docs, same, p] { fs.close(s, USER, h).unwrap(); }
        fs.close(s, OTHER, other).unwrap();
        assert_eq!(fs.held(), (0, 0, 0, 0));
    });
}

#[test]
fn a_file_written_flushed_and_read_back_matches_by_sha256() {
    run(|| {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let mut fs = Box::new(Fs::new());
        let s = &mut store;
        let r = fs.root(s, USER, "home", writable).unwrap();
        // Three chunks and some, in pieces that do not fall on chunks.
        let data = pattern(1, 3 * CHUNK + 4321);
        let f = fs.open(s, USER, r, "big.bin", WRITE | CREATE | TRUNCATE).unwrap();
        for (k, piece) in data.chunks(5000).enumerate() { assert_eq!(fs.write(s, USER, f, (k * 5000) as u64, piece), Ok(piece.len())); }
        fs.flush(s, USER, f).unwrap();
        assert_eq!(fs.held().3, 0);
        let digest = sha256::digest(&data);
        assert_eq!(sha256::digest(&read_all(&mut fs, s, USER, f)), digest);
        // From the store alone, and through another client's root.
        let (version, tree) = published(s, "fs/home");
        assert_eq!(version, 1);
        match &tree["big.bin"] { Err(stored) => assert_eq!(sha256::digest(stored), digest), other => panic!("{other:?}") }
        let app = fs.root(s, APP, "home", writable).unwrap();
        let g = fs.open(s, APP, app, "big.bin", 0).unwrap();
        assert_eq!(sha256::digest(&read_all(&mut fs, s, APP, g)), digest);
        // Changed in place across a chunk's end: the rest is read from the object it was.
        let mut model = data.clone();
        let patch = pattern(9, 100);
        write_all(&mut fs, s, USER, f, CHUNK as u64 - 50, &patch);
        model[CHUNK - 50..CHUNK + 50].copy_from_slice(&patch);
        assert_eq!(fs.held().3, 2);
        fs.close(s, USER, f).unwrap();
        let (version, tree) = published(s, "fs/home");
        assert_eq!((version, &tree["big.bin"]), (2, &Err(model.clone())));
        // The reader's tree stays at the version it read.
        assert_eq!(sha256::digest(&read_all(&mut fs, s, APP, g)), digest);
        fs.close(s, APP, g).unwrap();
        fs.close(s, APP, app).unwrap();

        // Random writes, cuts and growths of one file against a model, stored now and then.
        let f = fs.open(s, USER, r, "big.bin", WRITE).unwrap();
        let mut rng = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = |n: u64| { rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17; rng % n };
        for step in 0..400 {
            match next(4) {
                0 | 1 => {
                    let (offset, len) = (next(model.len() as u64 + 3000), 1 + next(20000) as usize);
                    let piece = pattern(step, len);
                    match fs.write(s, USER, f, offset, &piece[..len.min(16384)]) {
                        Ok(n) => {
                            let end = offset as usize + n;
                            if model.len() < end { model.resize(end, 0); }
                            model[offset as usize..end].copy_from_slice(&piece[..n]);
                        }
                        Err(Error::Full) => fs.flush(s, USER, f).unwrap(),
                        Err(e) => panic!("step {step}: {e:?}"),
                    }
                }
                2 => {
                    let size = next(5 * CHUNK as u64);
                    fs.truncate(USER, f, size).unwrap();
                    model.resize(size as usize, 0);
                }
                _ => fs.flush(s, USER, f).unwrap(),
            }
            assert_eq!(read_all(&mut fs, s, USER, f), model, "step {step}");
        }
        fs.close(s, USER, f).unwrap();
        assert_eq!(published(s, "fs/home").1["big.bin"], Err(model));
    });
}

#[test]
fn a_flush_of_a_directory_publishes_every_change_under_it_at_once() {
    run(|| {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let mut fs = Box::new(Fs::new());
        let s = &mut store;
        let r = fs.root(s, USER, "home", writable).unwrap();
        let a = fs.open_dir(s, USER, r, "a", true).unwrap();
        let x = fs.open(s, USER, a, "x", WRITE | CREATE).unwrap();
        let y = fs.open(s, USER, r, "b/y", WRITE | CREATE);
        assert_eq!(y, Err(Error::NotFound));
        fs.open_dir(s, USER, r, "b", true).and_then(|b| fs.close(s, USER, b)).unwrap();
        let y = fs.open(s, USER, r, "b/y", WRITE | CREATE).unwrap();
        write_all(&mut fs, s, USER, x, 0, &pattern(1, CHUNK + 1));
        write_all(&mut fs, s, USER, y, 0, b"yankee");
        // The flush of `a` stores what was written below it and publishes the tree once: `b/y` as it was made, empty.
        fs.flush(s, USER, a).unwrap();
        let (version, tree) = published(s, "fs/home");
        assert_eq!(version, 1);
        assert_eq!((&tree["a/x"], &tree["b/y"]), (&Err(pattern(1, CHUNK + 1)), &Err(Vec::new())));
        // The root's flush takes the rest; a flush with nothing changed publishes nothing.
        fs.flush(s, USER, r).unwrap();
        assert_eq!(published(s, "fs/home"), (2, BTreeMap::from([("a".into(), Ok(1)), ("a/x".into(), Err(pattern(1, CHUNK + 1))), ("b".into(), Ok(1)), ("b/y".into(), Err(b"yankee".to_vec()))])));
        fs.flush(s, USER, r).unwrap();
        for h in [x, y, a, r] { fs.close(s, USER, h).unwrap(); }
        assert_eq!(s.snapshot(b"fs/home").unwrap().0, 2);
        // A directory's edits alone wait for a flush: the tree's last handle closed first drops them.
        let r = fs.root(s, USER, "home", writable).unwrap();
        fs.open_dir(s, USER, r, "c", true).and_then(|c| fs.close(s, USER, c)).unwrap();
        fs.remove(s, USER, r, "b/y").unwrap();
        fs.close(s, USER, r).unwrap();
        assert_eq!(published(s, "fs/home").0, 2);
        let r = fs.root(s, USER, "home", writable).unwrap();
        assert_eq!(listed(&mut fs, s, USER, r), [entry("a", Directory, 1), entry("b", Directory, 1)]);
    });
}

#[test]
fn of_two_writers_from_one_version_the_second_is_told_of_the_conflict_and_publishes_nothing() {
    run(|| {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let mut fs = Box::new(Fs::new());
        let s = &mut store;
        let first = fs.root(s, USER, "home", writable).unwrap();
        let f = fs.open(s, USER, first, "shared", WRITE | CREATE).unwrap();
        write_all(&mut fs, s, USER, f, 0, b"one");
        fs.close(s, USER, f).unwrap();
        // Both from version 1.
        let second = fs.root(s, OTHER, "home", writable).unwrap();
        let f = fs.open(s, USER, first, "a", WRITE | CREATE).unwrap();
        write_all(&mut fs, s, USER, f, 0, b"alpha");
        let g = fs.open(s, OTHER, second, "b", WRITE | CREATE).unwrap();
        write_all(&mut fs, s, OTHER, g, 0, b"bravo");
        fs.close(s, USER, f).unwrap();
        // The second is told, and its handle closed all the same.
        let conflict = fs.close(s, OTHER, g);
        assert_eq!(conflict, Err(Error::Conflict { read: 1, current: 2 }));
        assert_eq!(conflict.unwrap_err().wire(), vfs::Error::Io);
        assert_eq!(fs.stat(s, OTHER, g), Err(Error::Invalid));
        // Nothing of it is published: the name is at the first writer's tree, with the versions it keeps.
        let (version, tree) = published(s, "fs/home");
        assert_eq!((version, tree.keys().map(String::as_str).collect::<Vec<_>>()), (2, vec!["a", "shared"]));
        assert_eq!(s.history(b"fs/home").unwrap().iter().map(|v| v.version).collect::<Vec<_>>(), [2, 1]);
        // Its tree keeps its changes, never merged: it reads them, and each flush of it is refused the same way.
        assert_eq!(listed(&mut fs, s, OTHER, second), [entry("b", File, 5), entry("shared", File, 3)]);
        let g = fs.open(s, OTHER, second, "b", 0).unwrap();
        assert_eq!(read_all(&mut fs, s, OTHER, g), b"bravo");
        assert_eq!(fs.volume(OTHER, second).map(|v| v.version), Ok(1));
        assert_eq!(fs.flush(s, OTHER, second), Err(Error::Conflict { read: 1, current: 2 }));
        assert_eq!(s.snapshot(b"fs/home").unwrap().0, 2);
        // A later root reads the new version, and its writer makes the change again.
        let again = fs.root(s, OTHER, "home", writable).unwrap();
        assert_eq!(listed(&mut fs, s, OTHER, again).iter().map(|e| e.0.as_str()).collect::<Vec<_>>(), ["a", "shared"]);
        let h = fs.open(s, OTHER, again, "b", WRITE | CREATE).unwrap();
        write_all(&mut fs, s, OTHER, h, 0, b"bravo");
        fs.close(s, OTHER, h).unwrap();
        assert_eq!(published(s, "fs/home").1.keys().map(String::as_str).collect::<Vec<_>>(), ["a", "b", "shared"]);
        // The stale tree goes with its last handle.
        let trees = fs.held().1;
        fs.close(s, OTHER, g).unwrap();
        fs.close(s, OTHER, second).unwrap();
        assert_eq!(fs.held().1, trees - 1);
    });
}

#[test]
fn a_publication_stopped_at_any_write_leaves_the_old_root_or_the_new_never_a_mix() {
    // Version 1 published; then a file of two chunks written in `a/b`, one made in `c`, a move, and the root flushed on
    // a medium that stops at write `k` of that flush (not made, or `torn`: its first sector alone). Returns the writes of
    // the flush, what it answered, and the medium.
    let long = |k: usize| format!("{}{k}", "f".repeat(200));
    let flush = move |stop: Option<(usize, bool)>| -> (usize, Result<(), Error>, Memory) {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let (answer, writes);
        {
            let mut store = mount(&mut medium, &mut room);
            let s = &mut store;
            let mut fs = Box::new(Fs::new());
            let r = fs.root(s, USER, "home", writable).unwrap();
            for dir in ["a/b", "c"] { fs.open_dir(s, USER, r, dir, true).and_then(|d| fs.close(s, USER, d)).unwrap(); }
            let f = fs.open(s, USER, r, &format!("a/b/{}", long(0)), WRITE | CREATE).unwrap();
            write_all(&mut fs, s, USER, f, 0, &pattern(0, 100));
            fs.close(s, USER, f).unwrap();
            let f = fs.open(s, USER, r, &format!("a/b/{}", long(1)), WRITE | CREATE).unwrap();
            write_all(&mut fs, s, USER, f, 0, &pattern(1, 2 * CHUNK));
            fs.open(s, USER, r, &format!("c/{}", long(2)), CREATE).and_then(|g| fs.close(s, USER, g)).unwrap();
            fs.rename(s, USER, r, &format!("a/b/{}", long(0)), r, &long(0)).unwrap();
            let start = s.device().writes;
            if let Some((k, torn)) = stop {
                s.device().stop = Some(start + k);
                s.device().torn = torn;
            }
            answer = fs.flush(s, USER, r);
            writes = s.device().writes - start;
        }
        medium.stop = None;
        (writes, answer, medium)
    };
    // A new store on the medium: the name's version and root, its tree whole.
    let mounted = |mut medium: Memory| {
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let (version, root) = store.resolve(b"fs/home").unwrap();
        complete(&mut store, &root);
        (version, root, published(&mut store, "fs/home").1)
    };
    run(move || {
        // Not stopped: two chunks, a node over them, four directories (a/b, a, c, the root) and the name's record.
        let (writes, answer, medium) = flush(None);
        let (version, new, new_tree) = mounted(medium);
        assert_eq!((writes, version, answer), (8, 2, Ok(())));
        let (_, answer, medium) = flush(Some((1, false)));
        let (version, old, old_tree) = mounted(medium);
        assert_eq!(version, 1);
        assert!(answer.is_err() && old != new && old_tree.len() + 2 == new_tree.len());
        let mut newer = 0;
        for k in 1..=writes {
            for torn in [false, true] {
                let (_, answer, medium) = flush(Some((k, torn)));
                match mounted(medium) {
                    (1, root, tree) => assert_eq!((root, tree, answer.is_ok()), (old, old_tree.clone(), false), "write {k}, torn {torn}"),
                    (2, root, tree) => { assert_eq!((root, tree), (new, new_tree.clone()), "write {k}, torn {torn}"); newer += 1; }
                    (v, ..) => panic!("version {v} at write {k}, torn {torn}"),
                }
            }
        }
        // Only the name's record, written whole in its one sector before the medium stopped, publishes the new root.
        assert_eq!(newer, 1);
    });
}

#[test]
fn a_clients_handles_are_its_own_and_none_allows_more_than_the_one_it_came_from() {
    run(|| {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let mut fs = Box::new(Fs::new());
        let s = &mut store;
        let r = fs.root(s, USER, "home", writable).unwrap();
        let f = fs.open(s, USER, r, "notes", WRITE | CREATE).unwrap();
        write_all(&mut fs, s, USER, f, 0, b"mine");
        fs.flush(s, USER, f).unwrap();
        // Another PID, or the same PID with another badge, holds none of them.
        for stranger in [OTHER, Client { pid: USER.pid, badge: 0 }, APP] {
            for h in [r, f, 63, 64, u32::MAX] {
                assert_eq!(fs.stat(s, stranger, h), Err(Error::Invalid));
                assert_eq!(fs.open(s, stranger, h, "notes", 0), Err(Error::Invalid));
                assert_eq!(fs.open_dir(s, stranger, h, "", false), Err(Error::Invalid));
                assert_eq!(fs.read(s, stranger, h, 0, &mut [0; 4]), Err(Error::Invalid));
                assert_eq!(fs.write(s, stranger, h, 0, b"theirs"), Err(Error::Invalid));
                assert_eq!(fs.truncate(stranger, h, 0), Err(Error::Invalid));
                assert_eq!(fs.list(s, stranger, h, 0, |_, _, _| {}), Err(Error::Invalid));
                assert_eq!(fs.remove(s, stranger, h, "notes"), Err(Error::Invalid));
                assert_eq!(fs.rename(s, stranger, h, "notes", h, "x"), Err(Error::Invalid));
                assert_eq!(fs.volume(stranger, h), Err(Error::Invalid));
                assert_eq!(fs.flush(s, stranger, h), Err(Error::Invalid));
                assert_eq!(fs.close(s, stranger, h), Err(Error::Invalid));
            }
        }
        assert_eq!(read_all(&mut fs, s, USER, f), b"mine");
        // Two handles of one client from two trees are not one tree.
        let r2 = fs.root(s, USER, "home", writable).unwrap();
        assert_eq!(fs.rename(s, USER, r, "notes", r2, "x"), Err(Error::Invalid));

        // A volume the badge may not write: read, never changed.
        let app = fs.root(s, APP, "home", writable).unwrap();
        assert_eq!(fs.volume(APP, app).map(|v| v.writable), Ok(false));
        for mode in [WRITE, CREATE, TRUNCATE, WRITE | CREATE | TRUNCATE] {
            assert_eq!(fs.open(s, APP, app, "notes", mode), Err(Error::Denied));
            assert_eq!(fs.open(s, APP, app, "new", mode), Err(Error::Denied));
        }
        assert_eq!(fs.open_dir(s, APP, app, "dir", true), Err(Error::Denied));
        assert_eq!(fs.remove(s, APP, app, "notes"), Err(Error::Denied));
        assert_eq!(fs.rename(s, APP, app, "notes", app, "x"), Err(Error::Denied));
        let g = fs.open(s, APP, app, "notes", 0).unwrap();
        assert_eq!(read_all(&mut fs, s, APP, g), b"mine");
        assert_eq!((fs.write(s, APP, g, 0, b"x"), fs.truncate(APP, g, 0)), (Err(Error::Denied), Err(Error::Denied)));
        let d = fs.open_dir(s, APP, app, "", true).unwrap();
        assert_eq!(fs.open(s, APP, d, "notes", WRITE), Err(Error::Denied));
        // Its flush has nothing to publish.
        fs.flush(s, APP, app).unwrap();
        fs.flush(s, APP, g).unwrap();
        assert_eq!(s.snapshot(b"fs/home").unwrap().0, 1);

        // A writer's handle opened to read allows no write, and one below a directory no more than it.
        let read = fs.open(s, USER, r, "notes", 0).unwrap();
        assert_eq!((fs.write(s, USER, read, 0, b"x"), fs.truncate(USER, read, 0)), (Err(Error::Denied), Err(Error::Denied)));
        let below = fs.open_dir(s, USER, r, "below", true).unwrap();
        assert_eq!(fs.volume(USER, below).map(|v| v.writable), Ok(true));
        let below_app = fs.open_dir(s, APP, d, "", false).unwrap();
        assert_eq!(fs.volume(APP, below_app).map(|v| v.writable), Ok(false));

        // A client that ended: its handles are closed, and what it did not publish is dropped.
        write_all(&mut fs, s, USER, f, 0, b"lost");
        let before = fs.held();
        assert_eq!(fs.forget(USER.pid), 5);
        assert_eq!(fs.held(), (before.0 - 5, before.1 - 2, before.2 - 1, 0));
        assert_eq!(fs.stat(s, USER, r), Err(Error::Invalid));
        assert_eq!(published(s, "fs/home"), (1, BTreeMap::from([("notes".into(), Err(b"mine".to_vec()))])));
    });
}

#[test]
fn each_bound_is_refused_with_nothing_changed() {
    run(|| {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let mut fs = Box::new(Fs::new());
        let s = &mut store;

        // TREES: a root each.
        let roots: Vec<u32> = (0..TREES).map(|k| fs.root(s, USER, &format!("v{k}"), writable).unwrap()).collect();
        let before = fs.held();
        assert_eq!(fs.root(s, USER, "more", writable), Err(Error::Handles));
        assert_eq!(fs.held(), before);
        for &h in &roots[1..] { fs.close(s, USER, h).unwrap(); }
        let r = roots[0];

        // FILES: a file made by each open; the one refused is not made.
        let files: Vec<u32> = (0..FILES).map(|k| fs.open(s, USER, r, &format!("f{k:02}"), CREATE).unwrap()).collect();
        let (before, tree) = (fs.held(), seen(&mut fs, s, USER, r));
        assert_eq!(fs.open(s, USER, r, "one-more", WRITE | CREATE), Err(Error::Handles));
        assert_eq!((fs.held(), seen(&mut fs, s, USER, r)), (before, tree));
        // A file open already is shared, not another.
        let again = fs.open(s, USER, r, "f00", 0).unwrap();
        assert_eq!(fs.held().2, FILES);

        // HANDLES.
        let mut dirs = Vec::new();
        while let Ok(h) = fs.open_dir(s, USER, r, "", false) { dirs.push(h); }
        let before = fs.held();
        assert_eq!(before.0, HANDLES);
        assert_eq!(fs.open_dir(s, USER, r, "", false), Err(Error::Handles));
        assert_eq!(fs.open(s, USER, r, "f00", 0), Err(Error::Handles));
        assert_eq!(fs.held(), before);
        for h in dirs.into_iter().chain(files).chain([again]) { fs.close(s, USER, h).unwrap(); }
        assert_eq!(fs.held(), (1, 1, 0, 0));

        // CHUNKS: a chunk for each one a write changed; a write that needs more than are free takes none.
        let f = fs.open(s, USER, r, "chunks", WRITE | CREATE).unwrap();
        for k in 0..CHUNKS - 1 { fs.write(s, USER, f, (k * CHUNK) as u64, &[k as u8 + 1]).unwrap(); }
        let model = read_all(&mut fs, s, USER, f);
        let before = fs.held();
        assert_eq!(before.3, CHUNKS - 1);
        assert_eq!(fs.write(s, USER, f, (CHUNKS * CHUNK - 10) as u64, &[7; 20]), Err(Error::Full));
        assert_eq!(fs.held(), before);
        assert_eq!(read_all(&mut fs, s, USER, f), model);
        // One more fits; then a flush lets them go, and the write is made.
        fs.write(s, USER, f, ((CHUNKS - 1) * CHUNK) as u64, &[1]).unwrap();
        assert_eq!(fs.write(s, USER, f, (CHUNKS * CHUNK) as u64, &[1]), Err(Error::Full));
        fs.flush(s, USER, f).unwrap();
        assert_eq!(fs.held().3, 0);
        fs.write(s, USER, f, (CHUNKS * CHUNK - 10) as u64, &[7; 20]).unwrap();
        fs.close(s, USER, f).unwrap();
        assert_eq!(published(s, "fs/v0").1["chunks"].as_ref().map_err(Vec::len), Err((CHUNKS * CHUNK + 10) as usize));

        // PATH: two names of 255 bytes fit a handle's path, a third does not; nothing is made.
        let name = "n".repeat(255);
        let two = fs.open_dir(s, USER, r, &format!("{name}/{name}"), true).unwrap();
        let tree = seen(&mut fs, s, USER, r);
        assert_eq!(fs.open_dir(s, USER, two, &name, true), Err(Error::TooLarge));
        assert_eq!(fs.open_dir(s, USER, r, &format!("x/{name}/{name}"), true), Err(Error::TooLarge));
        assert_eq!(seen(&mut fs, s, USER, r), tree);
        assert!(2 * 255 + 1 <= PATH && 3 * 255 + 2 > PATH);
        // A move whose handles' paths would not fit is refused before it is made.
        fs.open_dir(s, USER, r, "x", true).and_then(|x| fs.close(s, USER, x)).unwrap();
        assert_eq!(fs.rename(s, USER, r, &name, r, &format!("x/{}", "m".repeat(255))), Err(Error::TooLarge));
        assert_eq!(seen(&mut fs, s, USER, r), tree.into_iter().chain([("x".into(), (Directory, 0))]).collect());
        fs.close(s, USER, two).unwrap();
        fs.open(s, USER, r, "x/f", CREATE).and_then(|g| fs.close(s, USER, g)).unwrap();
        fs.flush(s, USER, r).unwrap();

        // The working tree's DIRS: directories made until it is full, the one refused not made.
        let version = s.snapshot(b"fs/v0").unwrap().0;
        let stored = fs.open(s, USER, r, "x/f", WRITE).unwrap();
        let mut made = 0;
        let refused = loop {
            match fs.open_dir(s, USER, r, &format!("d{made:02}"), true) {
                Ok(h) => { fs.close(s, USER, h).unwrap(); made += 1; }
                Err(e) => break e,
            }
        };
        assert_eq!((refused, made), (Error::Full, worktree::DIRS - 1));
        assert!(!seen(&mut fs, s, USER, r).contains_key(&format!("d{made:02}")));
        // With one free again, a path of two takes it and is refused at the second: the first is taken out again.
        fs.remove(s, USER, r, &format!("d{:02}", made - 1)).unwrap();
        let tree = seen(&mut fs, s, USER, r);
        assert_eq!(fs.open_dir(s, USER, r, "e/e", true), Err(Error::Full));
        assert_eq!(seen(&mut fs, s, USER, r), tree);
        fs.open_dir(s, USER, r, "e", true).and_then(|e| fs.close(s, USER, e)).unwrap();
        // A file stored in a directory the full tree cannot hold: the edits before it are published first.
        write_all(&mut fs, s, USER, stored, 0, b"stored");
        fs.close(s, USER, stored).unwrap();
        let (now, tree) = published(s, "fs/v0");
        assert_eq!((now, tree.contains_key("d00"), tree.contains_key("e"), &tree["x/f"]), (version + 2, true, true, &Err(b"stored".to_vec())));
        fs.close(s, USER, r).unwrap();
        assert_eq!(fs.held(), (0, 0, 0, 0));
    });
}

#[test]
fn the_errors_are_vfs_wits() {
    // The stand-in above is vfs.wit 2.5's enum as generated; 2.6 adds `conflict`, and this mapping changes with it.
    let generated = include_str!("../libmind/src/idl/vfs.rs");
    assert!(generated.contains("pub const VERSION: (u8, u8, u8) = (2, 5, 0);"));
    assert!(generated.contains("pub enum Error { #[default] NotFound = 0, Exists = 1, NotEmpty = 2, Invalid = 3, Denied = 4, NoSpace = 5, ReadOnly = 6, Io = 7, Handles = 8, Name = 9, NotDirectory = 10, IsDirectory = 11 }"));
    let modes = include_str!("../libmind/src/fs.rs");
    for (name, value) in [("MODE_WRITE", WRITE), ("MODE_CREATE", CREATE), ("MODE_TRUNCATE", TRUNCATE), ("MODE_NEW", NEW)] {
        assert!(modes.contains(&format!("pub const {name}: u8 = {value};")), "{name}");
    }
    use vfs::Error as V;
    for (error, wire) in [
        (Error::NotFound, V::NotFound), (Error::Exists, V::Exists), (Error::NotEmpty, V::NotEmpty), (Error::Invalid, V::Invalid),
        (Error::Denied, V::Denied), (Error::Name, V::Name), (Error::NotDirectory, V::NotDirectory), (Error::IsDirectory, V::IsDirectory),
        (Error::Handles, V::Handles), (Error::Full, V::NoSpace), (Error::TooLarge, V::NoSpace), (Error::Store(dag::Error::Full), V::NoSpace),
        (Error::Conflict { read: 1, current: 2 }, V::Io), (Error::Store(dag::Error::Corrupt), V::Io), (Error::Store(dag::Error::NotFound), V::Io),
    ] {
        assert_eq!(error.wire(), wire, "{error:?}");
    }
    // The working tree's errors, as storefs reports them.
    for (inner, outer) in [
        (worktree::Error::Inside, Error::Invalid), (worktree::Error::Full, Error::Full), (worktree::Error::TooLarge, Error::TooLarge),
        (worktree::Error::Conflict { read: 3, current: 4 }, Error::Conflict { read: 3, current: 4 }), (worktree::Error::Store(dag::Error::Shape), Error::Store(dag::Error::Shape)),
    ] {
        assert_eq!(Error::from(inner), outer);
    }
}

#[test]
fn a_volume_whose_root_is_no_directory_is_refused() {
    run(|| {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let mut fs = Box::new(Fs::new());
        let s = &mut store;
        let mut builder = Box::new(Builder::new());
        builder.write(s, b"not a directory").unwrap();
        let (file, _) = builder.finish(s).unwrap();
        s.publish(b"fs/odd", 0, &file, OWNER).unwrap();
        assert_eq!(fs.root(s, USER, "odd", writable), Err(Error::NotDirectory));
        assert_eq!(fs.held(), (0, 0, 0, 0));
    });
}
