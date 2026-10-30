// blocks tree (311-STO-0029, docs/storage): a directory tree under a name, read and changed through a working tree
// (mind::worktree). Each change is one publication by compare-and-swap, from the name's current version or, given as the
// last word, a version it keeps (0: none, the empty tree); a test plays a second writer so.
use mind::cid::Cid;
use mind::dag::{self, Builder, CHUNK};
use mind::fs::File;
use mind::idl::blockstore::{self, Error};
use mind::idl::codec::Text;
use mind::sha256::Sha256;
use mind::tree::Kind;
use mind::worktree::{self, Found, Worktree};

use crate::models::hex;
use crate::{report, store, store_silent, Remote};

// About 85 KiB: kept here rather than on the stack.
static mut TREE: Worktree = Worktree::new();

const USAGE: &str = "Usage: blocks tree ls <name> [path] | cat <name> <path> <file> | put <name> <file> <path> [expected] | mkdir <name> <path> [expected] | rm <name> <path> [expected] | mv <name> <from> <to> [expected]";

/// The store's names, as a publication of the working tree needs them.
impl worktree::Names for Remote {
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<Option<u64>, dag::Error> {
        match blockstore::publish(store(), name, expected, &root.to_bytes()) {
            Ok(Ok(done)) => {
                self.durability = done.durability as u8;
                Ok(Some(done.version))
            }
            Ok(Err(Error::Conflict)) => Ok(None),
            Ok(Err(e)) => Err(self.fail(e)),
            Err(_) => Err(dag::Error::Store),
        }
    }
    fn version(&mut self, name: &str) -> Result<u64, dag::Error> {
        let Some(text) = Text::new(name) else { return Err(dag::Error::Store) };
        match blockstore::snapshot(store(), &[text]) {
            Ok(Ok(heads)) => heads.as_slice().first().map(|h| h.version).ok_or(dag::Error::Store),
            Ok(Err(e)) => Err(self.fail(e)),
            Err(_) => Err(dag::Error::Store),
        }
    }
}

pub fn command<'a>(mut words: impl Iterator<Item = &'a str>, builder: &mut Builder, buffer: &mut [u8; CHUNK], data: &mut [u8; CHUNK], remote: &mut Remote) {
    let tree = unsafe { &mut *core::ptr::addr_of_mut!(TREE) };
    let w: [Option<&str>; 6] = core::array::from_fn(|_| words.next());
    match w {
        [Some("ls"), Some(name), path, None, ..] => ls(tree, remote, name, path.unwrap_or("")),
        [Some("cat"), Some(name), Some(path), Some(file), None, _] => cat(tree, remote, name, path, file, buffer, data),
        [Some("put"), Some(name), Some(file), Some(path), expected, None] => {
            if !read(tree, name, expected) { return; }
            let Ok(mut f) = File::open(file) else { mind::println!("blocks: cannot open {}", file); return };
            loop {
                let n = match f.read(data) { Ok(n) => n, Err(e) => { mind::println!("blocks: reading {}: {:?}", file, e); return } };
                if n == 0 { break; }
                if let Err(e) = builder.write(remote, &data[..n]) { return report("tree put", e, remote); }
            }
            let (cid, size) = match builder.finish(remote) { Ok(done) => done, Err(e) => return report("tree put", e, remote) };
            let done = tree.put(remote, path, &cid, size);
            publish(tree, remote, name, "tree put", done);
        }
        [Some("mkdir"), Some(name), Some(path), expected, None, _] => {
            if !read(tree, name, expected) { return; }
            let done = tree.mkdir(remote, path);
            publish(tree, remote, name, "tree mkdir", done);
        }
        [Some("rm"), Some(name), Some(path), expected, None, _] => {
            if !read(tree, name, expected) { return; }
            let done = tree.remove(remote, path);
            publish(tree, remote, name, "tree rm", done);
        }
        [Some("mv"), Some(name), Some(from), Some(to), expected, None] => {
            if !read(tree, name, expected) { return; }
            let done = tree.rename(remote, from, to);
            publish(tree, remote, name, "tree mv", done);
        }
        _ => mind::println!("{}", USAGE),
    }
}

// Starts the tree from `name` at version `expected`: none, its current one; 0, the empty tree of a new name.
fn read(tree: &mut Worktree, name: &str, expected: Option<&str>) -> bool {
    let Some(text) = Text::<64>::new(name) else { mind::println!("blocks: tree: a name of at most 64 bytes"); return false };
    let Some(expected) = expected else {
        return match blockstore::snapshot(store(), &[text]) {
            Ok(Ok(heads)) => match heads.as_slice().first() {
                Some(head) => { tree.start(Cid::from_bytes(head.root.as_slice()).ok(), head.version); true }
                None => { mind::println!("blocks: tree {}: the store answered no head", name); false }
            },
            Ok(Err(e)) => { mind::println!("blocks: tree {}: {:?}", name, e); false }
            Err(e) => { store_silent(e); false }
        };
    };
    let Ok(version) = expected.parse::<u64>() else { mind::println!("blocks: not a version: {}", expected); return false };
    if version == 0 { tree.start(None, 0); return true; }
    match blockstore::history(store(), name) {
        Ok(Ok(kept)) => match kept.as_slice().iter().find(|k| k.version == version) {
            Some(k) => { tree.start(Cid::from_bytes(k.root.as_slice()).ok(), version); true }
            None => { mind::println!("blocks: tree {}: no version {} kept", name, version); false }
        },
        Ok(Err(e)) => { mind::println!("blocks: tree {}: {:?}", name, e); false }
        Err(e) => { store_silent(e); false }
    }
}

// Publishes an edit made: the name, its new version and root; or the conflict, with both versions and the root not
// published (the tree, unchanged by the refusal, written out again: its nodes are stored already).
fn publish(tree: &mut Worktree, remote: &mut Remote, name: &str, what: &str, done: Result<(), worktree::Error>) {
    match done.and_then(|()| tree.publish(remote, name)) {
        Ok((version, root)) => mind::println!("PUBLISHED {} VERSION {} ROOT {} ({})", name, version, root, mind::blockstore::class(remote.durability)),
        Err(worktree::Error::Conflict { read, current }) => match tree.write_out(remote) {
            Ok(root) => mind::println!("CONFLICT {}: READ AT VERSION {}, NOW AT {}; ROOT {} NOT PUBLISHED", name, read, current, root),
            Err(_) => mind::println!("CONFLICT {}: READ AT VERSION {}, NOW AT {}; NOTHING PUBLISHED", name, read, current),
        },
        Err(e) => fail(what, e, remote),
    }
}

fn fail(what: &str, error: worktree::Error, remote: &Remote) {
    match error { worktree::Error::Store(e) => report(what, e, remote), e => mind::println!("blocks: {}: {:?}", what, e) }
}

// ls: each entry, a file's bytes or a directory's entries, its CID and name; then how many.
fn ls(tree: &mut Worktree, remote: &mut Remote, name: &str, path: &str) {
    if !read(tree, name, None) { return; }
    let listed = tree.list(remote, path, |entry, found| {
        let kind = if found.kind == Kind::File { "FILE" } else { "DIR" };
        if let Some(cid) = found.cid { mind::println!("{} {} {} {}", kind, found.size, cid, core::str::from_utf8(entry).unwrap_or("?")); }
    });
    match listed {
        Ok(n) => mind::println!("{} ENTRIES IN /{} OF {} VERSION {}", n, path.trim_start_matches('/'), name, tree.base().1),
        Err(e) => fail("tree ls", e, remote),
    }
}

// cat: the file at `path` into `file`, every block checked; its bytes and their SHA-256.
fn cat(tree: &mut Worktree, remote: &mut Remote, name: &str, path: &str, file: &str, buffer: &mut [u8; CHUNK], data: &mut [u8; CHUNK]) {
    if !read(tree, name, None) { return; }
    let root = match tree.lookup(remote, path) {
        Ok(Found { kind: Kind::File, cid: Some(root), .. }) => root,
        Ok(_) => { mind::println!("blocks: tree cat: {} is a directory", path); return }
        Err(e) => return fail("tree cat", e, remote),
    };
    let size = match dag::size(remote, &root, buffer) { Ok(size) => size, Err(e) => return report("tree cat", e, remote) };
    let Ok(mut out) = File::create(file) else { mind::println!("blocks: cannot create {}", file); return };
    let (mut at, mut hash) = (0, Sha256::new());
    while at < size {
        let n = match dag::read_at(remote, &root, at, data, buffer) { Ok(n) => n, Err(e) => return report("tree cat", e, remote) };
        hash.update(&data[..n]);
        if out.write(&data[..n]).ok() != Some(n) { mind::println!("blocks: writing {} failed", file); return; }
        at += n as u64;
    }
    let _ = out.flush();
    mind::println!("READ {} BYTES SHA-256 {}", size, hex(&hash.finish()));
}
