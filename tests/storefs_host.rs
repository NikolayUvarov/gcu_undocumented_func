//! Host tests of storefs/src/fs.rs (issue 311-STO-0032; MC-4.1, 4.7, 4.9): `vfs.wit`'s files and directories over the
//! store's names `fs/<volume>`, through the block store's own logic (blockstore/src/store.rs on a simulated medium).
//! Open, read, write, truncate, list page by page, remove, rename, stat and volume, each change of a directory one
//! publication; a file written, flushed and read back by SHA-256, and random writes against a model; the flush of a
//! directory publishes what is under it at once; a reader's open tree follows other writers' publications; of two
//! writers from one version the second is told of the conflict, nothing of it is published, and it writes again and
//! publishes; a publication stopped at each write of the medium leaves the old root or the new one; a client's handles
//! refused to another, a read-only volume refused a change, and a handle allowing no more than its parent; each bound,
//! and each client's share, refused with nothing changed; the errors as `vfs.wit` 2.6 has them.
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

/// `vfs.wit` 2.6's errors, as fs.rs maps to them: checked against the generated libmind/src/idl/vfs.rs below.
mod vfs {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Error { NotFound = 0, Exists = 1, NotEmpty = 2, Invalid = 3, Denied = 4, NoSpace = 5, ReadOnly = 6, Io = 7, Handles = 8, Name = 9, NotDirectory = 10, IsDirectory = 11, Conflict = 12 }
}

use cid::Cid;
use dag::{Builder, CHUNK};
use fs::{Client, Error, Fs, CHUNKS, CHUNKS_EACH, CREATE, FILES, FILES_EACH, FILE_MAX, HANDLES, HANDLES_EACH, NEW, PAGE, PATH, TREES, TREES_EACH, TRUNCATE, WRITE};
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

/// The current version of `name`.
fn version(store: &mut Local, name: &str) -> u64 { store.snapshot(name.as_bytes()).unwrap().0 }

/// Writes `data` at `offset` as a client of `vfs.wit` does: 16 KiB a call at most.
fn write_all(fs: &mut Fs, store: &mut Local, client: Client, file: u32, offset: u64, data: &[u8]) {
    for (k, part) in data.chunks(16384).enumerate() { assert_eq!(fs.write(store, client, file, offset + (k * 16384) as u64, part), Ok(part.len())); }
}

/// A file made at `path` below `dir` with `data`, closed: two publications.
fn make(fs: &mut Fs, store: &mut Local, client: Client, dir: u32, path: &str, data: &[u8]) {
    let f = fs.open(store, client, dir, path, WRITE | CREATE | TRUNCATE).unwrap();
    write_all(fs, store, client, f, 0, data);
    fs.close(store, client, f).unwrap();
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
        let v = fs.volume(s, USER, r).unwrap();
        assert_eq!((v.name(), v.writable, v.version, v.root), ("home", true, 0, None));
        let e = fs.stat(s, USER, r).unwrap();
        assert_eq!((e.name(), e.kind, e.size), (&b""[..], Directory, 0));
        for bad in ["", "a/b", "seventeen-bytes-x", "a b"] { assert_eq!(fs.root(s, USER, bad, writable), Err(Error::NotFound), "{bad:?}"); }
        // vfs_server's volumes are not storefs's, whatever their case: a files client asks vfs_server first.
        for theirs in ["ram", "log", "models", "RAM", "Models"] { assert_eq!(fs.root(s, USER, theirs, writable), Err(Error::NotFound), "{theirs}"); }
        assert_eq!(fs::VFS_VOLUMES, ["ram", "log", "models"]);
        assert_eq!(fs.held(), (1, 1, 0, 0));

        // Directories: those made on the way with `create` are one publication; a path without names is the directory.
        let deep = fs.open_dir(s, USER, r, "/docs//deep/", true).unwrap();
        assert_eq!(published(s, "fs/home"), (1, BTreeMap::from([("docs".into(), Ok(1)), ("docs/deep".into(), Ok(0))])));
        assert_eq!(fs.stat(s, USER, deep).map(|e| (e.name().to_vec(), e.kind, e.size)), Ok((b"deep".to_vec(), Directory, 0)));
        let docs = fs.open_dir(s, USER, r, "docs", true).unwrap();
        let same = fs.open_dir(s, USER, docs, "", false).unwrap();
        assert_eq!(fs.stat(s, USER, same).unwrap().name(), b"docs");
        assert_eq!(version(s, "fs/home"), 1);
        assert_eq!(fs.open_dir(s, USER, r, "none", false), Err(Error::NotFound));
        assert_eq!(fs.open_dir(s, USER, r, "docs/..", false), Err(Error::Invalid));
        assert_eq!(fs.open_dir(s, USER, r, "./docs", false), Err(Error::Invalid));

        // A file made by `open` is one publication; written, read, grown with a gap of zeros, cut and grown again.
        let f = fs.open(s, USER, docs, "a.txt", WRITE | CREATE).unwrap();
        assert_eq!(published(s, "fs/home").1.get("docs/a.txt"), Some(&Err(Vec::new())));
        assert_eq!(fs.write(s, USER, f, 0, b"hello world"), Ok(11));
        let mut out = [0u8; 32];
        assert_eq!(fs.read(s, USER, f, 6, &mut out), Ok(5));
        assert_eq!(&out[..5], b"world");
        assert_eq!(fs.write(s, USER, f, 20, b"!"), Ok(1));
        assert_eq!(read_all(&mut fs, s, USER, f), b"hello world\0\0\0\0\0\0\0\0\0!");
        fs.truncate(s, USER, f, 5).unwrap();
        fs.truncate(s, USER, f, 8).unwrap();
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
        assert_eq!(fs.truncate(s, USER, docs, 0), Err(Error::IsDirectory));
        assert_eq!(fs.list(s, USER, f, 0, |_, _, _| {}), Err(Error::NotDirectory));
        // A file's data waits for a flush or a close of what was written.
        assert_eq!(version(s, "fs/home"), 2);
        fs.close(s, USER, f).unwrap();
        let (version_now, tree) = published(s, "fs/home");
        assert_eq!(version_now, 3);
        assert_eq!(tree.get("docs/a.txt"), Some(&Err(b"hello\0\0\0".to_vec())));
        assert_eq!(tree.get("docs/deep"), Some(&Ok(0)));
        assert_eq!(fs.volume(s, USER, g).map(|v| v.version), Ok(3));

        // Forty files made by `open` alone (handles that may not write), each one publication.
        let p = fs.open_dir(s, USER, r, "p", true).unwrap();
        for k in 0..40 {
            let h = fs.open(s, USER, p, &format!("{k:02}"), CREATE).unwrap();
            assert_eq!(fs.write(s, USER, h, 0, b"x"), Err(Error::Denied));
            fs.close(s, USER, h).unwrap();
        }
        assert_eq!(version(s, "fs/home"), 44);
        // Listed sixteen at a time, in the order of the names; an empty page is the end.
        let mut names = Vec::new();
        for (start, n) in [(0, 16), (16, 16), (32, 8), (40, 0), (99, 0)] {
            assert_eq!(fs.list(s, USER, p, start, |name, kind, size| names.push((String::from_utf8(name.to_vec()).unwrap(), kind, size))), Ok(n), "from {start}");
        }
        assert_eq!(names, (0..40).map(|k| (format!("{k:02}"), File, 0)).collect::<Vec<_>>());

        // Removals, each one publication: a file, whose handles are closed; a directory with entries refused.
        fs.remove(s, USER, r, "docs/a.txt").unwrap();
        assert_eq!((fs.stat(s, USER, g), version(s, "fs/home")), (Err(Error::Invalid), 45));
        assert_eq!(fs.remove(s, USER, r, "docs/a.txt"), Err(Error::NotFound));
        assert_eq!(fs.remove(s, USER, r, "p"), Err(Error::NotEmpty));
        assert_eq!(fs.remove(s, USER, r, ""), Err(Error::Invalid));
        fs.remove(s, USER, p, "00").unwrap();
        assert_eq!((listed(&mut fs, s, USER, p).len(), version(s, "fs/home")), (39, 46));

        // Renames across directory handles, each one publication; the handles of what moves follow it, and a file's
        // data still waits for its close.
        let f = fs.open(s, USER, docs, "b.txt", WRITE | CREATE).unwrap();
        write_all(&mut fs, s, USER, f, 0, b"bravo");
        fs.rename(s, USER, docs, "b.txt", deep, "c.txt").unwrap();
        assert_eq!(fs.stat(s, USER, f).unwrap().name(), b"c.txt");
        fs.rename(s, USER, r, "docs/deep", p, "moved").unwrap();
        assert_eq!((fs.stat(s, USER, deep).unwrap().name(), fs.stat(s, USER, f).unwrap().name()), (&b"moved"[..], &b"c.txt"[..]));
        assert_eq!(listed(&mut fs, s, USER, deep), [entry("c.txt", File, 5)]);
        let (version_now, tree) = published(s, "fs/home");
        assert_eq!((version_now, tree.get("p/moved/c.txt")), (49, Some(&Err(Vec::new()))));
        assert_eq!(fs.rename(s, USER, r, "p", deep, "p"), Err(Error::Invalid));
        assert_eq!(fs.rename(s, USER, r, "p/01", r, "docs"), Err(Error::Exists));
        assert_eq!(fs.rename(s, USER, r, "none", r, "x"), Err(Error::NotFound));
        assert_eq!(fs.rename(s, USER, docs, "", r, "x"), Err(Error::Invalid));
        assert_eq!(version(s, "fs/home"), 49);
        // Its close stores it where it is now.
        fs.close(s, USER, f).unwrap();
        let (version_now, tree) = published(s, "fs/home");
        assert_eq!(version_now, 50);
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
        let (version_now, tree) = published(s, "fs/home");
        assert_eq!(version_now, 2);
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
        let (version_now, tree) = published(s, "fs/home");
        assert_eq!((version_now, &tree["big.bin"]), (3, &Err(model.clone())));
        // The reader's open file follows the name: it reads the new version.
        assert_eq!(sha256::digest(&read_all(&mut fs, s, APP, g)), sha256::digest(&model));
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
                    fs.truncate(s, USER, f, size).unwrap();
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
fn a_flush_of_a_directory_publishes_every_file_written_under_it_at_once() {
    run(|| {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let mut fs = Box::new(Fs::new());
        let s = &mut store;
        let r = fs.root(s, USER, "home", writable).unwrap();
        let a = fs.open_dir(s, USER, r, "a", true).unwrap();
        let x = fs.open(s, USER, a, "x", WRITE | CREATE).unwrap();
        assert_eq!(fs.open(s, USER, r, "b/y", WRITE | CREATE), Err(Error::NotFound));
        fs.open_dir(s, USER, r, "b", true).and_then(|b| fs.close(s, USER, b)).unwrap();
        let y = fs.open(s, USER, r, "b/y", WRITE | CREATE).unwrap();
        assert_eq!(version(s, "fs/home"), 4);
        write_all(&mut fs, s, USER, x, 0, &pattern(1, CHUNK + 1));
        write_all(&mut fs, s, USER, y, 0, b"yankee");
        // The flush of `a` stores what was written below it, one publication; `b/y` stays as it was made, empty.
        fs.flush(s, USER, a).unwrap();
        let (version_now, tree) = published(s, "fs/home");
        assert_eq!(version_now, 5);
        assert_eq!((&tree["a/x"], &tree["b/y"]), (&Err(pattern(1, CHUNK + 1)), &Err(Vec::new())));
        // The root's flush takes the rest; a flush with nothing changed publishes nothing.
        fs.flush(s, USER, r).unwrap();
        assert_eq!(published(s, "fs/home"), (6, BTreeMap::from([("a".into(), Ok(1)), ("a/x".into(), Err(pattern(1, CHUNK + 1))), ("b".into(), Ok(1)), ("b/y".into(), Err(b"yankee".to_vec()))])));
        fs.flush(s, USER, r).unwrap();
        for h in [x, y, a, r] { fs.close(s, USER, h).unwrap(); }
        assert_eq!(version(s, "fs/home"), 6);
        // A directory's edits are published at once: nothing waits for the tree's last handle.
        let r = fs.root(s, USER, "home", writable).unwrap();
        fs.open_dir(s, USER, r, "c", true).and_then(|c| fs.close(s, USER, c)).unwrap();
        fs.remove(s, USER, r, "b/y").unwrap();
        fs.close(s, USER, r).unwrap();
        assert_eq!(version(s, "fs/home"), 8);
        let r = fs.root(s, USER, "home", writable).unwrap();
        assert_eq!(listed(&mut fs, s, USER, r), [entry("a", Directory, 1), entry("b", Directory, 0), entry("c", Directory, 0)]);
    });
}

#[test]
fn a_tree_that_holds_nothing_unpublished_follows_the_name() {
    run(|| {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let mut fs = Box::new(Fs::new());
        let s = &mut store;
        let w = fs.root(s, USER, "home", writable).unwrap();
        make(&mut fs, s, USER, w, "notes", b"one");
        // A reader keeps its root, a file and a directory open.
        let rd = fs.root(s, APP, "home", writable).unwrap();
        let f = fs.open(s, APP, rd, "notes", 0).unwrap();
        assert_eq!(read_all(&mut fs, s, APP, f), b"one");
        // Each publication of the writer is read through them.
        make(&mut fs, s, USER, w, "notes", b"two!");
        assert_eq!(read_all(&mut fs, s, APP, f), b"two!");
        fs.open_dir(s, USER, w, "new", true).and_then(|d| fs.close(s, USER, d)).unwrap();
        assert_eq!(listed(&mut fs, s, APP, rd), [entry("new", Directory, 0), entry("notes", File, 4)]);
        let dir = fs.open_dir(s, APP, rd, "new", false).unwrap();
        assert_eq!(fs.volume(s, APP, rd).map(|v| v.version), Ok(version(s, "fs/home")));
        // What the writer removes, the reader's handles of it are closed, as a removal closes them.
        fs.remove(s, USER, w, "notes").unwrap();
        fs.remove(s, USER, w, "new").unwrap();
        assert_eq!((fs.stat(s, APP, f), fs.stat(s, APP, dir)), (Err(Error::Invalid), Err(Error::Invalid)));
        assert_eq!(listed(&mut fs, s, APP, rd), []);
        assert_eq!(fs.held_by(APP), (1, 1, 0, 0));
        // A tree that holds unpublished data stays at the version it read.
        let other = fs.root(s, OTHER, "home", writable).unwrap();
        let g = fs.open(s, OTHER, other, "mine", WRITE | CREATE).unwrap();
        write_all(&mut fs, s, OTHER, g, 0, b"draft");
        let before = version(s, "fs/home");
        make(&mut fs, s, USER, w, "later", b"x");
        assert_eq!(fs.volume(s, OTHER, other).map(|v| v.version), Ok(before));
        assert_eq!(listed(&mut fs, s, OTHER, other), [entry("mine", File, 5)]);
        assert_eq!(listed(&mut fs, s, APP, rd), [entry("later", File, 1), entry("mine", File, 0)]);
    });
}

#[test]
fn of_two_writers_from_one_version_the_second_is_told_of_the_conflict_and_writes_again() {
    run(|| {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let mut fs = Box::new(Fs::new());
        let s = &mut store;
        let first = fs.root(s, USER, "home", writable).unwrap();
        for (name, data) in [("a", &b"a0"[..]), ("b", b"b0"), ("shared", b"one")] { make(&mut fs, s, USER, first, name, data); }
        let read = version(s, "fs/home");
        // Both write from that version.
        let second = fs.root(s, OTHER, "home", writable).unwrap();
        let f = fs.open(s, USER, first, "a", WRITE).unwrap();
        write_all(&mut fs, s, USER, f, 0, b"alpha");
        let g = fs.open(s, OTHER, second, "b", WRITE).unwrap();
        write_all(&mut fs, s, OTHER, g, 0, b"bravo");
        let h = fs.open(s, OTHER, second, "shared", WRITE).unwrap();
        write_all(&mut fs, s, OTHER, h, 0, b"two");
        fs.close(s, USER, f).unwrap();
        // The second's tree holds unpublished data: it reads the version it read until it publishes.
        assert_eq!(listed(&mut fs, s, OTHER, second)[0], entry("a", File, 2));
        let conflict = fs.close(s, OTHER, g);
        assert_eq!(conflict, Err(Error::Conflict { read, current: read + 1 }));
        assert_eq!(conflict.unwrap_err().wire(), vfs::Error::Conflict);
        assert_eq!(fs.stat(s, OTHER, g), Err(Error::Invalid));
        // Nothing of it is published: the name is at the first writer's tree, with the versions it keeps.
        let (version_now, tree) = published(s, "fs/home");
        assert_eq!((version_now, &tree["a"], &tree["b"], &tree["shared"]), (read + 1, &Err(b"alpha".to_vec()), &Err(b"b0".to_vec()), &Err(b"one".to_vec())));
        assert_eq!(s.history(b"fs/home").unwrap().iter().map(|v| v.version).take(2).collect::<Vec<_>>(), [read + 1, read]);
        // Its tree dropped what it did not publish and follows the name, never merged: a handle that held such data
        // answers the conflict to every call until it is closed.
        assert_eq!(fs.held_by(OTHER).3, 0);
        let refused = Err(Error::Conflict { read, current: read + 1 });
        assert_eq!(fs.write(s, OTHER, h, 0, b"x"), refused.map(|()| 1));
        assert_eq!(fs.truncate(s, OTHER, h, 0), refused);
        assert_eq!(fs.flush(s, OTHER, h), refused);
        assert_eq!(fs.read(s, OTHER, h, 0, &mut [0; 4]), refused.map(|()| 0));
        assert_eq!(fs.stat(s, OTHER, h).map(|e| e.size), refused.map(|()| 0));
        assert_eq!(fs.close(s, OTHER, h), refused);
        assert_eq!(fs.stat(s, OTHER, h), Err(Error::Invalid));
        // Its root reads the new version, and it writes again and publishes.
        assert_eq!(fs.volume(s, OTHER, second).map(|v| v.version), Ok(read + 1));
        let a = fs.open(s, OTHER, second, "a", 0).unwrap();
        assert_eq!(read_all(&mut fs, s, OTHER, a), b"alpha");
        let g = fs.open(s, OTHER, second, "b", WRITE).unwrap();
        write_all(&mut fs, s, OTHER, g, 0, b"bravo");
        fs.close(s, OTHER, g).unwrap();
        let (version_now, tree) = published(s, "fs/home");
        assert_eq!((version_now, &tree["a"], &tree["b"]), (read + 2, &Err(b"alpha".to_vec()), &Err(b"bravo".to_vec())));
    });
}

#[test]
fn a_change_of_a_directory_is_one_publication_and_its_conflict_is_told_on_it() {
    run(|| {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let mut fs = Box::new(Fs::new());
        let s = &mut store;
        let first = fs.root(s, USER, "home", writable).unwrap();
        make(&mut fs, s, USER, first, "x", b"x0");
        let read = version(s, "fs/home");
        // The second holds unpublished data, so its tree stays at `read`; the first publishes.
        let second = fs.root(s, OTHER, "home", writable).unwrap();
        let g = fs.open(s, OTHER, second, "x", WRITE).unwrap();
        write_all(&mut fs, s, OTHER, g, 0, b"draft");
        fs.open_dir(s, USER, first, "m", true).and_then(|d| fs.close(s, USER, d)).unwrap();
        // Each change of a directory is refused on its own call, with nothing published.
        let refused = Err(Error::Conflict { read, current: read + 1 });
        assert_eq!(fs.open_dir(s, OTHER, second, "n", true), refused.map(|()| 0));
        assert_eq!(published(s, "fs/home").1.keys().map(String::as_str).collect::<Vec<_>>(), ["m", "x"]);
        assert_eq!(fs.write(s, OTHER, g, 0, b"x"), refused.map(|()| 1));
        // Followed to the new version, the same call is made.
        let n = fs.open_dir(s, OTHER, second, "n", true).unwrap();
        fs.rename(s, OTHER, second, "m", n, "m").unwrap();
        fs.remove(s, OTHER, second, "x").unwrap();
        assert_eq!(version(s, "fs/home"), read + 4);
        assert_eq!(published(s, "fs/home").1.keys().map(String::as_str).collect::<Vec<_>>(), ["n", "n/m"]);
        // A rename and a removal refused so.
        let h = fs.open(s, OTHER, second, "n/f", WRITE | CREATE).unwrap();
        write_all(&mut fs, s, OTHER, h, 0, b"f");
        let read = version(s, "fs/home");
        make(&mut fs, s, USER, first, "y", b"y");
        let refused = Err(Error::Conflict { read, current: read + 2 });
        assert_eq!(fs.rename(s, OTHER, second, "n/m", second, "m"), refused);
        assert_eq!(fs.remove(s, OTHER, second, "n/m"), Ok(()));
        assert_eq!(published(s, "fs/home").1.keys().map(String::as_str).collect::<Vec<_>>(), ["n", "n/f", "y"]);
    });
}

#[test]
fn a_publication_stopped_at_any_write_leaves_the_old_root_or_the_new_never_a_mix() {
    // Two files written, in `a/b` (two chunks) and `c`, while a move and the files' making were published; the root then
    // flushed on a medium that stops at write `k` of that flush (not made, or `torn`: its first sector alone). Names of
    // 200 bytes make each node changed more than a sector. Returns the writes of the flush, what it answered, and the
    // medium.
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
            make(&mut fs, s, USER, r, &format!("a/b/{}", long(0)), &pattern(0, 100));
            let f = fs.open(s, USER, r, &format!("a/b/{}", long(1)), WRITE | CREATE).unwrap();
            write_all(&mut fs, s, USER, f, 0, &pattern(1, 2 * CHUNK));
            let g = fs.open(s, USER, r, &format!("c/{}", long(2)), WRITE | CREATE).unwrap();
            write_all(&mut fs, s, USER, g, 0, &pattern(2, 300));
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
        // Not stopped: three chunks, a node over two, four directories (a/b, a, c, the root) and the name's record.
        let (writes, answer, medium) = flush(None);
        let (version_new, new, new_tree) = mounted(medium);
        assert_eq!((writes, answer), (9, Ok(())));
        let (_, answer, medium) = flush(Some((1, false)));
        let (version_old, old, old_tree) = mounted(medium);
        assert_eq!(version_new, version_old + 1);
        assert!(answer.is_err() && old != new);
        assert!(old_tree.keys().eq(new_tree.keys()) && old_tree[&format!("c/{}", long(2))] == Err(Vec::new()) && new_tree[&format!("c/{}", long(2))] == Err(pattern(2, 300)));
        let mut newer = 0;
        for k in 1..=writes {
            for torn in [false, true] {
                let (_, answer, medium) = flush(Some((k, torn)));
                match mounted(medium) {
                    (v, root, tree) if v == version_old => assert_eq!((root, tree, answer.is_ok()), (old, old_tree.clone(), false), "write {k}, torn {torn}"),
                    (v, root, tree) if v == version_new => { assert_eq!((root, tree), (new, new_tree.clone()), "write {k}, torn {torn}"); newer += 1; }
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
                assert_eq!(fs.truncate(s, stranger, h, 0), Err(Error::Invalid));
                assert_eq!(fs.list(s, stranger, h, 0, |_, _, _| {}), Err(Error::Invalid));
                assert_eq!(fs.remove(s, stranger, h, "notes"), Err(Error::Invalid));
                assert_eq!(fs.rename(s, stranger, h, "notes", h, "x"), Err(Error::Invalid));
                assert_eq!(fs.volume(s, stranger, h), Err(Error::Invalid));
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
        assert_eq!(fs.volume(s, APP, app).map(|v| v.writable), Ok(false));
        for mode in [WRITE, CREATE, TRUNCATE, WRITE | CREATE | TRUNCATE] {
            assert_eq!(fs.open(s, APP, app, "notes", mode), Err(Error::Denied));
            assert_eq!(fs.open(s, APP, app, "new", mode), Err(Error::Denied));
        }
        assert_eq!(fs.open_dir(s, APP, app, "dir", true), Err(Error::Denied));
        assert_eq!(fs.remove(s, APP, app, "notes"), Err(Error::Denied));
        assert_eq!(fs.rename(s, APP, app, "notes", app, "x"), Err(Error::Denied));
        let g = fs.open(s, APP, app, "notes", 0).unwrap();
        assert_eq!(read_all(&mut fs, s, APP, g), b"mine");
        assert_eq!((fs.write(s, APP, g, 0, b"x"), fs.truncate(s, APP, g, 0)), (Err(Error::Denied), Err(Error::Denied)));
        let d = fs.open_dir(s, APP, app, "", true).unwrap();
        assert_eq!(fs.open(s, APP, d, "notes", WRITE), Err(Error::Denied));
        // Its flush has nothing to publish.
        fs.flush(s, APP, app).unwrap();
        fs.flush(s, APP, g).unwrap();
        assert_eq!(version(s, "fs/home"), 2);

        // A writer's handle opened to read allows no write, and one below a directory no more than it.
        let read = fs.open(s, USER, r, "notes", 0).unwrap();
        assert_eq!((fs.write(s, USER, read, 0, b"x"), fs.truncate(s, USER, read, 0)), (Err(Error::Denied), Err(Error::Denied)));
        let below = fs.open_dir(s, USER, r, "below", true).unwrap();
        assert_eq!(fs.volume(s, USER, below).map(|v| v.writable), Ok(true));
        let below_app = fs.open_dir(s, APP, d, "", false).unwrap();
        assert_eq!(fs.volume(s, APP, below_app).map(|v| v.writable), Ok(false));

        // A client that ended: its handles are closed, and what it did not publish is dropped.
        write_all(&mut fs, s, USER, f, 0, b"lost");
        let before = fs.held();
        assert_eq!(fs.forget(|pid| pid == USER.pid), 5);
        assert_eq!(fs.held(), (before.0 - 5, before.1 - 2, before.2 - 1, 0));
        assert_eq!(fs.stat(s, USER, r), Err(Error::Invalid));
        assert_eq!(published(s, "fs/home"), (3, BTreeMap::from([("below".into(), Ok(0)), ("notes".into(), Err(b"mine".to_vec()))])));
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

        // TREES: two roots a client, four in all.
        let mine: Vec<u32> = (0..TREES_EACH).map(|k| fs.root(s, USER, &format!("v{k}"), writable).unwrap()).collect();
        let before = fs.held();
        assert_eq!(fs.root(s, USER, "more", writable), Err(Error::Handles));
        let theirs: Vec<u32> = (0..TREES - TREES_EACH).map(|k| fs.root(s, OTHER, &format!("w{k}"), writable).unwrap()).collect();
        assert_eq!(fs.held().1, TREES);
        assert_eq!(fs.root(s, APP, "v0", writable), Err(Error::Handles));
        assert_eq!(fs.held(), (before.0 + 2, TREES, 0, 0));
        for &h in &mine[1..] { fs.close(s, USER, h).unwrap(); }
        for &h in &theirs { fs.close(s, OTHER, h).unwrap(); }
        let r = mine[0];

        // FILES: sixteen a client, each made by its open; the one refused is not made.
        let files: Vec<u32> = (0..FILES_EACH).map(|k| fs.open(s, USER, r, &format!("f{k:02}"), CREATE).unwrap()).collect();
        let (before, tree, at) = (fs.held(), seen(&mut fs, s, USER, r), version(s, "fs/v0"));
        assert_eq!(fs.open(s, USER, r, "one-more", WRITE | CREATE), Err(Error::Handles));
        assert_eq!((fs.held(), seen(&mut fs, s, USER, r), version(s, "fs/v0")), (before, tree, at));
        // A file open already is shared, not another.
        let again = fs.open(s, USER, r, "f00", 0).unwrap();
        assert_eq!(fs.held_by(USER).2, FILES_EACH);

        // HANDLES: thirty-two a client.
        let mut dirs = Vec::new();
        while let Ok(h) = fs.open_dir(s, USER, r, "", false) { dirs.push(h); }
        let before = fs.held();
        assert_eq!(fs.held_by(USER).0, HANDLES_EACH);
        assert_eq!(fs.open_dir(s, USER, r, "", false), Err(Error::Handles));
        assert_eq!(fs.open(s, USER, r, "f00", 0), Err(Error::Handles));
        assert_eq!(fs.root(s, USER, "v1", writable), Err(Error::Handles));
        assert_eq!(fs.held(), before);
        for h in dirs.into_iter().chain(files).chain([again]) { fs.close(s, USER, h).unwrap(); }
        assert_eq!(fs.held(), (1, 1, 0, 0));

        // CHUNKS: eight a client, one for each chunk a write changed; a write that needs more than are left takes none.
        let f = fs.open(s, USER, r, "chunks", WRITE | CREATE).unwrap();
        for k in 0..CHUNKS_EACH - 1 { fs.write(s, USER, f, (k * CHUNK) as u64, &[k as u8 + 1]).unwrap(); }
        let model = read_all(&mut fs, s, USER, f);
        let before = fs.held();
        assert_eq!(before.3, CHUNKS_EACH - 1);
        assert_eq!(fs.write(s, USER, f, (CHUNKS_EACH * CHUNK - 10) as u64, &[7; 20]), Err(Error::Full));
        assert_eq!(fs.held(), before);
        assert_eq!(read_all(&mut fs, s, USER, f), model);
        // One more fits; then a flush lets them go, and the write is made.
        fs.write(s, USER, f, ((CHUNKS_EACH - 1) * CHUNK) as u64, &[1]).unwrap();
        assert_eq!(fs.write(s, USER, f, (CHUNKS_EACH * CHUNK) as u64, &[1]), Err(Error::Full));
        fs.flush(s, USER, f).unwrap();
        assert_eq!(fs.held().3, 0);
        fs.write(s, USER, f, (CHUNKS_EACH * CHUNK - 10) as u64, &[7; 20]).unwrap();
        fs.close(s, USER, f).unwrap();
        assert_eq!(published(s, "fs/v0").1["chunks"].as_ref().map_err(Vec::len), Err(CHUNKS_EACH * CHUNK + 10));

        // FILE_MAX: a write or a truncation that would make a file larger is refused, nothing changed.
        let f = fs.open(s, USER, r, "max", WRITE | CREATE).unwrap();
        fs.write(s, USER, f, FILE_MAX - 1, &[7]).unwrap();
        let before = fs.held();
        assert_eq!(fs.write(s, USER, f, FILE_MAX - 1, &[7, 7]), Err(Error::TooLarge));
        assert_eq!(fs.write(s, USER, f, FILE_MAX, &[7]), Err(Error::TooLarge));
        assert_eq!(fs.truncate(s, USER, f, FILE_MAX + 1), Err(Error::TooLarge));
        assert_eq!(Error::TooLarge.wire(), vfs::Error::NoSpace);
        assert_eq!((fs.held(), fs.stat(s, USER, f).map(|e| e.size)), (before, Ok(FILE_MAX)));
        let mut last = [0; 2];
        assert_eq!((fs.read(s, USER, f, FILE_MAX - 1, &mut last), last), (Ok(1), [7, 0]));
        fs.truncate(s, USER, f, 10).unwrap();
        fs.close(s, USER, f).unwrap();
        assert_eq!(published(s, "fs/v0").1["max"], Err(vec![0; 10]));
        assert_eq!(FILE_MAX, 64 << 20);

        // PATH: two names of 255 bytes fit a handle's path, a third does not; nothing is made.
        let name = "n".repeat(255);
        let two = fs.open_dir(s, USER, r, &format!("{name}/{name}"), true).unwrap();
        let (tree, at) = (seen(&mut fs, s, USER, r), version(s, "fs/v0"));
        assert_eq!(fs.open_dir(s, USER, two, &name, true), Err(Error::TooLarge));
        assert_eq!(fs.open_dir(s, USER, r, &format!("x/{name}/{name}"), true), Err(Error::TooLarge));
        assert_eq!((seen(&mut fs, s, USER, r), version(s, "fs/v0")), (tree.clone(), at));
        assert!(2 * 255 + 1 <= PATH && 3 * 255 + 2 > PATH);
        // A move whose handles' paths would not fit is refused before it is made.
        fs.open_dir(s, USER, r, "x", true).and_then(|x| fs.close(s, USER, x)).unwrap();
        assert_eq!(fs.rename(s, USER, r, &name, r, &format!("x/{}", "m".repeat(255))), Err(Error::TooLarge));
        assert_eq!(seen(&mut fs, s, USER, r), tree.into_iter().chain([("x".into(), (Directory, 0))]).collect());
        fs.close(s, USER, two).unwrap();

        // tree::DEPTH: a path of 32 directories is refused at the last, and none of it is made.
        let (tree, at) = (seen(&mut fs, s, USER, r), version(s, "fs/v0"));
        assert_eq!(fs.open_dir(s, USER, r, &vec!["d"; tree::DEPTH].join("/"), true), Err(Error::TooLarge));
        assert_eq!((seen(&mut fs, s, USER, r), version(s, "fs/v0")), (tree, at));

        // The working tree's DIRS: three files 31 directories down in three branches, flushed at once. The first two
        // take 63 directories; the third is put after they are published, so the flush publishes twice.
        let branches: Vec<String> = (0..3).map(|k| format!("b{k}/{}", vec!["n"; tree::DEPTH - 2].join("/"))).collect();
        let written: Vec<u32> = branches.iter().enumerate().map(|(k, branch)| {
            fs.open_dir(s, USER, r, branch, true).and_then(|d| fs.close(s, USER, d)).unwrap();
            let f = fs.open(s, USER, r, &format!("{branch}/f"), WRITE | CREATE).unwrap();
            write_all(&mut fs, s, USER, f, 0, &pattern(k, 10));
            f
        }).collect();
        let at = version(s, "fs/v0");
        fs.flush(s, USER, r).unwrap();
        let (now, tree) = published(s, "fs/v0");
        assert_eq!(now, at + 2);
        for (k, branch) in branches.iter().enumerate() { assert_eq!(tree[&format!("{branch}/f")], Err(pattern(k, 10)), "{branch}"); }
        for f in written { fs.close(s, USER, f).unwrap(); }
        fs.close(s, USER, r).unwrap();
        assert_eq!(fs.held(), (0, 0, 0, 0));
    });
}

#[test]
fn a_client_holds_at_most_half_of_each_table_and_another_still_gets_its_share() {
    run(|| {
        let mut medium = Memory::new();
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let mut fs = Box::new(Fs::new());
        let s = &mut store;
        // A client takes its share of each: trees, files, chunks, then handles.
        fn take(fs: &mut Fs, s: &mut Local, client: Client, volumes: [&str; 2]) -> ([u32; 2], u32) {
            let roots = volumes.map(|v| fs.root(s, client, v, writable).unwrap());
            let f = fs.open(s, client, roots[0], "data", WRITE | CREATE).unwrap();
            for k in 1..FILES_EACH { fs.open(s, client, roots[0], &format!("f{k:02}"), CREATE).unwrap(); }
            for k in 0..CHUNKS_EACH { fs.write(s, client, f, (k * CHUNK) as u64, &[1]).unwrap(); }
            while fs.open_dir(s, client, roots[1], "", false).is_ok() {}
            assert_eq!(fs.held_by(client), (HANDLES_EACH, TREES_EACH, FILES_EACH, CHUNKS_EACH));
            // Past it, each is refused with nothing changed.
            let before = fs.held();
            assert_eq!(fs.root(s, client, "more", writable), Err(Error::Handles));
            assert_eq!(fs.open_dir(s, client, roots[0], "", false), Err(Error::Handles));
            assert_eq!(fs.open(s, client, roots[0], "data", 0), Err(Error::Handles));
            assert_eq!(fs.write(s, client, f, (CHUNKS_EACH * CHUNK) as u64, &[1]), Err(Error::Full));
            assert_eq!(fs.held(), before);
            (roots, f)
        }
        let (_, f) = take(&mut fs, s, USER, ["v0", "v1"]);
        assert_eq!(fs.held(), (HANDLES_EACH, TREES_EACH, FILES_EACH, CHUNKS_EACH));
        // Another client still gets a root, handles, a file and chunks: as many as the first.
        let (_, theirs) = take(&mut fs, s, OTHER, ["w0", "w1"]);
        assert_eq!(fs.held(), (HANDLES, TREES, FILES, CHUNKS));
        assert_eq!(fs.root(s, APP, "v0", writable), Err(Error::Handles));
        // Chunks one client gives back are not another's past its share; what an ended client held, a third may take.
        fs.flush(s, USER, f).unwrap();
        assert_eq!(fs.held_by(USER).3, 0);
        assert_eq!(fs.write(s, OTHER, theirs, (CHUNKS_EACH * CHUNK) as u64, &[1]), Err(Error::Full));
        assert_eq!(fs.forget(|pid| pid == USER.pid), HANDLES_EACH);
        assert_eq!(fs.root(s, APP, "v0", writable).map(|_| fs.held_by(APP)), Ok((1, 1, 0, 0)));
    });
}

#[test]
fn the_errors_are_vfs_wits() {
    // The stand-in above is vfs.wit 2.6's enum as generated, `conflict` among it (311-KRN-0189).
    let generated = include_str!("../libmind/src/idl/vfs.rs");
    assert!(generated.contains("pub const VERSION: (u8, u8, u8) = (2, 6, 0);"));
    assert!(generated.contains("pub enum Error { #[default] NotFound = 0, Exists = 1, NotEmpty = 2, Invalid = 3, Denied = 4, NoSpace = 5, ReadOnly = 6, Io = 7, Handles = 8, Name = 9, NotDirectory = 10, IsDirectory = 11, Conflict = 12 }"));
    let modes = include_str!("../libmind/src/fs.rs");
    for (name, value) in [("MODE_WRITE", WRITE), ("MODE_CREATE", CREATE), ("MODE_TRUNCATE", TRUNCATE), ("MODE_NEW", NEW)] {
        assert!(modes.contains(&format!("pub const {name}: u8 = {value};")), "{name}");
    }
    use vfs::Error as V;
    for (error, wire) in [
        (Error::NotFound, V::NotFound), (Error::Exists, V::Exists), (Error::NotEmpty, V::NotEmpty), (Error::Invalid, V::Invalid),
        (Error::Denied, V::Denied), (Error::Name, V::Name), (Error::NotDirectory, V::NotDirectory), (Error::IsDirectory, V::IsDirectory),
        (Error::Handles, V::Handles), (Error::Full, V::NoSpace), (Error::TooLarge, V::NoSpace), (Error::Store(dag::Error::Full), V::NoSpace),
        (Error::Conflict { read: 1, current: 2 }, V::Conflict), (Error::Store(dag::Error::Corrupt), V::Io), (Error::Store(dag::Error::NotFound), V::Io),
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
