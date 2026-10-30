//! A working tree (issue 311-STO-0029; MC-4.9, 4.10): a directory tree of `tree` changed in memory and published as a
//! new root under a name. It starts from the root a name's version gave (none: the empty tree). Each directory an edit
//! changes is held here as its node, with every directory above it; the rest stays in the store, and reads go there
//! for it. A write-out puts the held directories, deepest first, and gives the new root: a subtree no edit touched keeps
//! its CID. A publication makes that root the name's next version only if the name is still at the version read (the
//! store's compare-and-swap), so a published root is never changed in place, and a writer learns when another one
//! published first: a conflict, never a merge. No heap and no system calls; what it holds is bounded by DIRS and
//! BYTES, and an edit past them is refused with nothing changed: tests/worktree_host.rs.
use crate::cid::{self, Cid, Codec};
use crate::dag::{self, fetch, Blocks, CHUNK};
use crate::tree::{self, Dir, Entry, Kind, DEPTH};

/// Directories a working tree holds at most: each one an edit changed, and the root.
pub const DIRS: usize = 64;
/// Bytes it holds at most: the held directories' nodes and names.
pub const BYTES: usize = 64 * 1024;
// The empty directory, `{"v": 1, "entries": []}`.
static EMPTY: [u8; 13] = *b"\xa2\x61v\x01\x67entries\x80";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// A path with a name `tree` refuses; the root, where an edit needs an entry.
    Name,
    /// No entry at the path, or no directory where its parent belongs.
    NotFound,
    /// A file where a directory belongs.
    NotDirectory,
    /// The name is taken.
    Exists,
    /// A directory removed while it holds entries.
    NotEmpty,
    /// A directory moved into itself or under it.
    Inside,
    /// Past DIRS or BYTES: nothing was changed, and a publication lets the held directories go.
    Full,
    /// A directory over a block, or more than `tree::DEPTH` directories down from the root: nothing was changed.
    TooLarge,
    /// The name is at another version than the one read: nothing was published.
    Conflict { read: u64, current: u64 },
    /// The store, or a block read from it (`dag::Error`): a file whose size is not the one given is Shape.
    Store(dag::Error),
}

impl From<dag::Error> for Error {
    fn from(e: dag::Error) -> Self { Error::Store(e) }
}

/// What a path names, as the edits leave it: its kind, its size (a file's bytes, a directory's entries) and its CID;
/// none for a held directory, which has one once written out, and for the empty tree's root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found { pub kind: Kind, pub size: u64, pub cid: Option<Cid> }

/// A name's versions, as a publication needs them: the block store's client, or its logic in tests.
pub trait Names {
    /// Makes `root` the next version of `name` if `expected` is its current one (0: no such name); returns the new
    /// version, or none when the name is at another (a conflict), nothing then published.
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<Option<u64>, dag::Error>;
    /// The current version of `name`: 0 for none, a removal's for a removed one.
    fn version(&mut self, name: &str) -> Result<u64, dag::Error>;
}

// A held directory: its parent's slot (the root, slot 0, is its own), the bytes of its name and of its node, its
// entries, and the CID the last write-out gave it. Names and nodes lie in `bytes` one after another, in slot order.
#[derive(Clone, Copy)]
struct Held { parent: usize, name: usize, node: usize, count: usize, cid: [u8; cid::BYTES] }

// A CID kept as its binary form, zeros for none (no CID starts with 0), so a new tree is zeros and a static of one
// takes no room in a program's file.
fn keep(cid: Option<Cid>) -> [u8; cid::BYTES] { cid.map_or([0; cid::BYTES], |c| c.to_bytes()) }
fn kept(bytes: &[u8; cid::BYTES]) -> Option<Cid> { Cid::from_bytes(bytes).ok() }

struct Dirs { held: [Held; DIRS], len: usize, bytes: [u8; BYTES] }

impl Dirs {
    fn start(&self, k: usize) -> usize { self.held[..k].iter().map(|h| h.name + h.node).sum() }
    fn used(&self) -> usize { self.start(self.len) }
    fn node(&self, k: usize) -> &[u8] {
        let at = self.start(k) + self.held[k].name;
        &self.bytes[at..at + self.held[k].node]
    }

    // The held subdirectory `name` of held `k`.
    fn child(&self, k: usize, name: &[u8]) -> Option<usize> {
        let mut at = 0;
        for c in 0..self.len {
            let h = &self.held[c];
            if c > 0 && h.parent == k && &self.bytes[at..at + h.name] == name { return Some(c); }
            at += h.name + h.node;
        }
        None
    }

    // Directories from the root down to held `k`, both counted.
    fn level(&self, mut k: usize) -> usize {
        let mut n = 1;
        while k != 0 && n <= DIRS { k = self.held[k].parent; n += 1; }
        n
    }

    // Puts `new` in place of `old` bytes at `at`; the room was checked.
    fn splice(&mut self, at: usize, old: usize, new: &[u8]) {
        let used = self.used();
        self.bytes.copy_within(at + old..used, at + new.len());
        self.bytes[at..at + new.len()].copy_from_slice(new);
    }

    fn set_node(&mut self, k: usize, node: &[u8], count: usize) {
        let at = self.start(k) + self.held[k].name;
        self.splice(at, self.held[k].node, node);
        (self.held[k].node, self.held[k].count) = (node.len(), count);
    }

    fn move_to(&mut self, k: usize, parent: usize, name: &[u8]) {
        self.splice(self.start(k), self.held[k].name, name);
        (self.held[k].name, self.held[k].parent) = (name.len(), parent);
    }

    fn add(&mut self, parent: usize, name: &[u8], node: &[u8], count: usize) {
        let at = self.used();
        self.bytes[at..at + name.len()].copy_from_slice(name);
        self.bytes[at + name.len()..at + name.len() + node.len()].copy_from_slice(node);
        self.held[self.len] = Held { parent, name: name.len(), node: node.len(), count, cid: keep(None) };
        self.len += 1;
    }

    // Lets held `k` go: a directory with no entries, so none is held under it.
    fn release(&mut self, k: usize) {
        self.splice(self.start(k), self.held[k].name + self.held[k].node, &[]);
        self.held.copy_within(k + 1..self.len, k);
        self.len -= 1;
        for h in &mut self.held[..self.len] { if h.parent > k { h.parent -= 1; } }
    }
}

// Where a directory is: held, the root as read (none: the empty tree), or in the store with the entries its parent
// gives it.
#[derive(Clone, Copy)]
enum Place { Held(usize), Root, Stored(Cid, u64) }

// A path: names `/`-separated, each one a name `tree` takes, a leading `/` allowed; the root is the empty path.
#[derive(Clone, Copy)]
struct Path<'a>(&'a str);

impl<'a> Path<'a> {
    fn parse(path: &'a str) -> Result<Self, Error> {
        let path = path.strip_prefix('/').unwrap_or(path);
        if path.is_empty() || path.split('/').all(|n| tree::valid_name(n.as_bytes())) { Ok(Path(path)) } else { Err(Error::Name) }
    }
    fn names(self) -> impl Iterator<Item = &'a [u8]> { self.0.split('/').filter(|n| !n.is_empty()).map(str::as_bytes) }
    fn depth(self) -> usize { self.names().count() }
    // The parent's path and the last name; the root has none.
    fn split(self) -> Result<(Path<'a>, &'a [u8]), Error> {
        match self.0.rsplit_once('/') {
            Some((parent, name)) => Ok((Path(parent), name.as_bytes())),
            None if !self.0.is_empty() => Ok((Path(""), self.0.as_bytes())),
            None => Err(Error::Name),
        }
    }
    // Whether this path lies under `other`.
    fn under(self, other: Path) -> bool { self.0.len() > other.0.len() && self.0.starts_with(other.0) && self.0.as_bytes()[other.0.len()] == b'/' }
}

// What an edit changes in one held node: an entry taken out, one put in, and new sizes of subdirectories by name.
#[derive(Clone, Copy)]
struct Change<'p> { slot: usize, out: Option<&'p [u8]>, add: Option<Entry<'p>>, sizes: [Option<(&'p [u8], u64)>; 2] }

// An edit: the held nodes it changes (a move: both parents, and theirs), and the held directory it moves (to a new
// parent and name), makes (under a parent) or lets go.
struct Plan<'p> { changes: [Option<Change<'p>>; 4], moved: Option<(usize, usize, &'p [u8])>, made: Option<(usize, &'p [u8])>, dropped: Option<usize> }

impl<'p> Plan<'p> {
    fn new() -> Self { Plan { changes: [None; 4], moved: None, made: None, dropped: None } }
    fn at(&mut self, slot: usize) -> &mut Change<'p> {
        let i = self.changes.iter().position(|c| c.is_none_or(|c| c.slot == slot)).unwrap();
        self.changes[i].get_or_insert(Change { slot, out: None, add: None, sizes: [None; 2] })
    }
}

/// A directory tree changed in memory: about 85 KiB, so a program keeps one in a static (`new` is `const`, all zeros).
pub struct Worktree { root: [u8; cid::BYTES], version: u64, dirs: Dirs, buffer: [u8; CHUNK] }

impl Default for Worktree { fn default() -> Self { Self::new() } }

impl Worktree {
    /// The empty tree, as of a name that does not exist (version 0).
    pub const fn new() -> Self {
        const NONE: Held = Held { parent: 0, name: 0, node: 0, count: 0, cid: [0; cid::BYTES] };
        Worktree { root: [0; cid::BYTES], version: 0, dirs: Dirs { held: [NONE; DIRS], len: 0, bytes: [0; BYTES] }, buffer: [0; CHUNK] }
    }

    /// Starts again from `root` (none: the empty tree), read at a name's `version`; every edit is dropped.
    pub fn start(&mut self, root: Option<Cid>, version: u64) { (self.root, self.version, self.dirs.len) = (keep(root), version, 0); }

    /// The root and the version the tree was read at.
    pub fn base(&self) -> (Option<Cid>, u64) { (kept(&self.root), self.version) }

    /// What it holds: directories (at most DIRS) and bytes (at most BYTES).
    pub fn held(&self) -> (usize, usize) { (self.dirs.len, self.dirs.used()) }

    /// What `path` names, as the edits leave it.
    pub fn lookup<B: Blocks>(&mut self, blocks: &mut B, path: &str) -> Result<Found, Error> {
        let Ok((parent, name)) = Path::parse(path)?.split() else {
            let (place, root) = (self.top(), kept(&self.root));
            let count = dir_at(blocks, &self.dirs, &mut self.buffer, root, place)?.len() as u64;
            return Ok(Found { kind: Kind::Directory, size: count, cid: if self.dirs.len > 0 { None } else { root } });
        };
        let place = self.find(blocks, parent)?;
        if let Place::Held(k) = place {
            if let Some(c) = self.dirs.child(k, name) { return Ok(Found { kind: Kind::Directory, size: self.dirs.held[c].count as u64, cid: None }); }
        }
        let (kind, size, cid) = self.get(blocks, place, name)?.ok_or(Error::NotFound)?;
        Ok(Found { kind, size, cid: Some(cid) })
    }

    /// Each entry of the directory at `path`, as the edits leave it, in the order of the names; returns how many.
    pub fn list<B: Blocks>(&mut self, blocks: &mut B, path: &str, mut visit: impl FnMut(&[u8], Found)) -> Result<usize, Error> {
        let (place, root) = (self.find(blocks, Path::parse(path)?)?, kept(&self.root));
        let dir = dir_at(blocks, &self.dirs, &mut self.buffer, root, place)?;
        for i in 0..dir.len() {
            let e = dir.entry(i);
            let held = matches!(place, Place::Held(k) if e.kind == Kind::Directory && self.dirs.child(k, e.name).is_some());
            visit(e.name, Found { kind: e.kind, size: e.size, cid: if held { None } else { Some(e.cid) } });
        }
        Ok(dir.len())
    }

    /// Puts the file `cid` of `size` bytes, an object the store holds, at `path`: in a directory that exists, under a
    /// name not taken.
    pub fn put<B: Blocks>(&mut self, blocks: &mut B, path: &str, cid: &Cid, size: u64) -> Result<(), Error> {
        let (parent, name) = Path::parse(path)?.split()?;
        // The object's root is read: a file, of the size given.
        if dag::size(blocks, cid, &mut self.buffer)? != size { return Err(dag::Error::Shape.into()); }
        self.edit(|w| {
            let k = w.hold(blocks, parent)?;
            if w.get(blocks, Place::Held(k), name)?.is_some() { return Err(Error::Exists); }
            let mut plan = Plan::new();
            plan.at(k).add = Some(Entry { name, kind: Kind::File, size, cid: *cid });
            w.grow(&mut plan, k, parent, 1);
            w.apply(&plan)
        })
    }

    /// Puts the file `cid` of `size` bytes, an object the store holds, at `path` in place of the file there, or under a
    /// name not taken: one edit, so a new version of a file never leaves it missing (311-STO-0032). A directory there is
    /// Exists.
    pub fn replace<B: Blocks>(&mut self, blocks: &mut B, path: &str, cid: &Cid, size: u64) -> Result<(), Error> {
        let (parent, name) = Path::parse(path)?.split()?;
        if dag::size(blocks, cid, &mut self.buffer)? != size { return Err(dag::Error::Shape.into()); }
        self.edit(|w| {
            let k = w.hold(blocks, parent)?;
            let mut plan = Plan::new();
            plan.at(k).add = Some(Entry { name, kind: Kind::File, size, cid: *cid });
            match w.get(blocks, Place::Held(k), name)? {
                Some((Kind::File, ..)) => plan.at(k).out = Some(name),
                Some(_) => return Err(Error::Exists),
                None => w.grow(&mut plan, k, parent, 1),
            }
            w.apply(&plan)
        })
    }

    /// Makes an empty directory at `path`: in a directory that exists, under a name not taken, at most `tree::DEPTH`
    /// directories down from the root.
    pub fn mkdir<B: Blocks>(&mut self, blocks: &mut B, path: &str) -> Result<(), Error> {
        let path = Path::parse(path)?;
        let (parent, name) = path.split()?;
        if path.depth() >= DEPTH { return Err(Error::TooLarge); }
        self.edit(|w| {
            let k = w.hold(blocks, parent)?;
            if w.get(blocks, Place::Held(k), name)?.is_some() { return Err(Error::Exists); }
            let mut plan = Plan::new();
            plan.at(k).add = Some(Entry { name, kind: Kind::Directory, size: 0, cid: Cid::of(Codec::DagCbor, &EMPTY) });
            plan.made = Some((k, name));
            w.grow(&mut plan, k, parent, 1);
            w.apply(&plan)
        })
    }

    /// Removes the entry at `path`: a file, or a directory with no entries.
    pub fn remove<B: Blocks>(&mut self, blocks: &mut B, path: &str) -> Result<(), Error> {
        let (parent, name) = Path::parse(path)?.split()?;
        self.edit(|w| {
            let k = w.hold(blocks, parent)?;
            let (kind, size, _) = w.get(blocks, Place::Held(k), name)?.ok_or(Error::NotFound)?;
            if kind == Kind::Directory && size > 0 { return Err(Error::NotEmpty); }
            let mut plan = Plan::new();
            plan.at(k).out = Some(name);
            plan.dropped = w.dirs.child(k, name);
            w.grow(&mut plan, k, parent, -1);
            w.apply(&plan)
        })
    }

    /// Moves the entry at `from` to `to`: in a directory that exists, under a name not taken; a directory not into
    /// itself, and at most `tree::DEPTH` directories down from the root.
    pub fn rename<B: Blocks>(&mut self, blocks: &mut B, from: &str, to: &str) -> Result<(), Error> {
        let (from, to) = (Path::parse(from)?, Path::parse(to)?);
        let ((fp, fname), (tp, tname)) = (from.split()?, to.split()?);
        self.edit(|w| {
            let f = w.hold(blocks, fp)?;
            let (kind, size, cid) = w.get(blocks, Place::Held(f), fname)?.ok_or(Error::NotFound)?;
            if kind == Kind::Directory && to.under(from) { return Err(Error::Inside); }
            let t = w.hold(blocks, tp)?;
            if w.get(blocks, Place::Held(t), tname)?.is_some() { return Err(Error::Exists); }
            if kind == Kind::Directory && to.depth() >= DEPTH { return Err(Error::TooLarge); }
            let mut plan = Plan::new();
            plan.at(f).out = Some(fname);
            plan.at(t).add = Some(Entry { name: tname, kind, size, cid });
            plan.moved = w.dirs.child(f, fname).map(|c| (c, t, tname));
            if f != t {
                w.grow(&mut plan, f, fp, -1);
                w.grow(&mut plan, t, tp, 1);
            }
            w.apply(&plan)
        })
    }

    /// Puts the held directories, deepest first, each once its held subdirectories have their CIDs, and returns the new
    /// root; what no edit touched keeps its CID. With nothing held: the root read, or the empty tree, put.
    pub fn write_out<B: Blocks>(&mut self, blocks: &mut B) -> Result<Cid, Error> {
        if self.dirs.len == 0 {
            if let Some(root) = kept(&self.root) { return Ok(root); }
            self.take(blocks, None, 0, b"")?;
        }
        let deepest = (0..self.dirs.len).map(|k| self.dirs.level(k)).max().unwrap_or(1);
        for level in (1..=deepest).rev() {
            for k in 0..self.dirs.len {
                if self.dirs.level(k) != level { continue; }
                let dirs = &self.dirs;
                let dir = tree::decode(dirs.node(k))?;
                // A held subdirectory's entry takes the CID it was just written as: it lies deeper.
                let entry = |i| {
                    let e = dir.entry(i);
                    match dirs.child(k, e.name) { Some(c) if e.kind == Kind::Directory => Entry { cid: kept(&dirs.held[c].cid).unwrap(), ..e }, _ => e }
                };
                let len = tree::encode_each(dir.len(), entry, &mut self.buffer)?;
                let cid = blocks.put(Codec::DagCbor, &self.buffer[..len])?;
                // The store's answer is checked, as `dag::Builder` checks it.
                if cid != Cid::of(Codec::DagCbor, &self.buffer[..len]) { return Err(dag::Error::Corrupt.into()); }
                self.dirs.held[k].cid = keep(Some(cid));
            }
        }
        Ok(kept(&self.dirs.held[0].cid).unwrap())
    }

    /// Writes the tree out and makes its root the next version of `name` if the name is still at the version read;
    /// returns that version and the root, and starts again from them. If the name is at another version, nothing is
    /// published, the tree stays as it is, and the error gives both versions; the nodes written out are retained by
    /// nothing, and a collection frees them once their lease ends.
    pub fn publish<S: Blocks + Names>(&mut self, store: &mut S, name: &str) -> Result<(u64, Cid), Error> {
        let root = self.write_out(store)?;
        match store.publish(name, self.version, &root)? {
            Some(version) => {
                self.start(Some(root), version);
                Ok((version, root))
            }
            None => Err(Error::Conflict { read: self.version, current: store.version(name)? }),
        }
    }

    // The root's place: held once an edit changed anything.
    fn top(&self) -> Place { if self.dirs.len > 0 { Place::Held(0) } else { Place::Root } }

    // The entry `name` of the directory at `place`, as its node has it.
    fn get<B: Blocks>(&mut self, blocks: &mut B, place: Place, name: &[u8]) -> Result<Option<(Kind, u64, Cid)>, Error> {
        let root = kept(&self.root);
        Ok(dir_at(blocks, &self.dirs, &mut self.buffer, root, place)?.find(name).map(|e| (e.kind, e.size, e.cid)))
    }

    // The directory at `path`, as the edits leave it.
    fn find<B: Blocks>(&mut self, blocks: &mut B, path: Path) -> Result<Place, Error> {
        let mut place = self.top();
        for name in path.names() {
            if let Place::Held(k) = place {
                if let Some(c) = self.dirs.child(k, name) { place = Place::Held(c); continue; }
            }
            place = match self.get(blocks, place, name)? {
                Some((Kind::Directory, count, cid)) => Place::Stored(cid, count),
                Some(_) => return Err(Error::NotDirectory),
                None => return Err(Error::NotFound),
            };
        }
        Ok(place)
    }

    // Holds every directory from the root down to the one at `path`; its slot. The slots it adds are the last ones.
    fn hold<B: Blocks>(&mut self, blocks: &mut B, path: Path) -> Result<usize, Error> {
        if self.dirs.len == 0 { self.take(blocks, None, 0, b"")?; }
        let mut k = 0;
        for name in path.names() {
            k = match self.dirs.child(k, name) {
                Some(c) => c,
                None => match self.get(blocks, Place::Held(k), name)? {
                    Some((Kind::Directory, count, cid)) => self.take(blocks, Some((cid, count)), k, name)?,
                    Some(_) => return Err(Error::NotDirectory),
                    None => return Err(Error::NotFound),
                },
            };
        }
        Ok(k)
    }

    // Holds the stored directory (CID, entries; none: the root as read) as `name` under held `parent`; its slot.
    fn take<B: Blocks>(&mut self, blocks: &mut B, stored: Option<(Cid, u64)>, parent: usize, name: &[u8]) -> Result<usize, Error> {
        let dir = match (stored, kept(&self.root)) {
            (Some((cid, count)), _) => read(blocks, &mut self.buffer, &cid, Some(count))?,
            (None, Some(cid)) => read(blocks, &mut self.buffer, &cid, None)?,
            (None, None) => tree::decode(&EMPTY)?,
        };
        let (node, count) = (dir.bytes(), dir.len());
        if self.dirs.len == DIRS || self.dirs.used() + name.len() + node.len() > BYTES { return Err(Error::Full); }
        self.dirs.add(parent, name, node, count);
        Ok(self.dirs.len - 1)
    }

    // The entries of held `k`, the directory at `path`, change by `delta`: its entry in its parent gets the new count.
    fn grow<'p>(&self, plan: &mut Plan<'p>, k: usize, path: Path<'p>, delta: isize) {
        let Ok((_, name)) = path.split() else { return };
        let count = (self.dirs.held[k].count as isize + delta) as u64;
        let change = plan.at(self.dirs.held[k].parent);
        let free = if change.sizes[0].is_none() { 0 } else { 1 };
        change.sizes[free] = Some((name, count));
    }

    // Runs an edit; if it is refused, the directories it began to hold are let go, so nothing is changed.
    fn edit(&mut self, edit: impl FnOnce(&mut Self) -> Result<(), Error>) -> Result<(), Error> {
        let held = self.dirs.len;
        let done = edit(self);
        if done.is_err() { self.dirs.len = held; }
        done
    }

    // Checks every change of `plan` against the bounds, then makes them all; one refused, none is made.
    fn apply(&mut self, plan: &Plan) -> Result<(), Error> {
        // Each step and the bytes it adds: a node rewritten (0 to 3), the moved directory renamed (4).
        let mut steps = [(0isize, 0usize); 5];
        let mut n = 0;
        for (i, change) in plan.changes.iter().enumerate() {
            let Some(c) = change else { continue };
            let (len, _) = rewrite(self.dirs.node(c.slot), c, &mut self.buffer)?;
            steps[n] = (len as isize - self.dirs.held[c.slot].node as isize, i);
            n += 1;
        }
        if let Some((k, _, name)) = plan.moved {
            steps[n] = (name.len() as isize - self.dirs.held[k].name as isize, 4);
            n += 1;
        }
        let mut used = self.dirs.used() as isize + steps[..n].iter().map(|s| s.0).sum::<isize>();
        if let Some((_, name)) = plan.made { used += (name.len() + EMPTY.len()) as isize; }
        if let Some(k) = plan.dropped { used -= (self.dirs.held[k].name + self.dirs.held[k].node) as isize; }
        if used as usize > BYTES || (plan.made.is_some() && self.dirs.len == DIRS) { return Err(Error::Full); }
        // What shrinks first, so the bytes held never pass BYTES on the way.
        steps[..n].sort_unstable();
        for &(_, step) in &steps[..n] {
            if let (4, Some((k, parent, name))) = (step, plan.moved) { self.dirs.move_to(k, parent, name); continue; }
            let Some(c) = plan.changes[step] else { continue };
            let (len, count) = rewrite(self.dirs.node(c.slot), &c, &mut self.buffer)?;
            self.dirs.set_node(c.slot, &self.buffer[..len], count);
        }
        if let Some((parent, name)) = plan.made { self.dirs.add(parent, name, &EMPTY, 0); }
        if let Some(k) = plan.dropped { self.dirs.release(k); }
        Ok(())
    }
}

// A directory read from the store into `buffer` and checked: a directory, of `count` entries if its parent gives them.
fn read<'b, B: Blocks>(blocks: &mut B, buffer: &'b mut [u8; CHUNK], cid: &Cid, count: Option<u64>) -> Result<Dir<'b>, Error> {
    // A `raw` block is a file's.
    if cid.codec() == Codec::Raw { return Err(Error::NotDirectory); }
    let len = fetch(blocks, cid, buffer)?;
    let buffer: &'b [u8; CHUNK] = buffer;
    let bytes = &buffer[..len];
    match tree::decode(bytes) {
        Ok(dir) if count.is_none_or(|n| n == dir.len() as u64) => Ok(dir),
        Ok(_) => Err(dag::Error::Shape.into()),
        // A file's node where a directory belongs.
        Err(dag::Error::Format) if dag::decode(bytes).is_ok() => Err(Error::NotDirectory),
        Err(e) => Err(e.into()),
    }
}

// The directory at `place`: a held node, the empty tree, or one read from the store.
fn dir_at<'s, B: Blocks>(blocks: &mut B, dirs: &'s Dirs, buffer: &'s mut [u8; CHUNK], root: Option<Cid>, place: Place) -> Result<Dir<'s>, Error> {
    match (place, root) {
        (Place::Held(k), _) => Ok(tree::decode(dirs.node(k))?),
        (Place::Root, None) => Ok(tree::decode(&EMPTY)?),
        (Place::Root, Some(cid)) => read(blocks, buffer, &cid, None),
        (Place::Stored(cid, count), _) => read(blocks, buffer, &cid, Some(count)),
    }
}

// Where `name` is among the sorted entries of `dir`, or where it would go.
fn position(dir: &Dir, name: &[u8]) -> Result<usize, usize> {
    let (mut low, mut high) = (0, dir.len());
    while low < high {
        let middle = (low + high) / 2;
        match dir.entry(middle).name.cmp(name) {
            core::cmp::Ordering::Less => low = middle + 1,
            core::cmp::Ordering::Greater => high = middle,
            core::cmp::Ordering::Equal => return Ok(middle),
        }
    }
    Err(low)
}

// Encodes the held `node` with `change` into `out`: its length and its entries.
fn rewrite(node: &[u8], change: &Change, out: &mut [u8; CHUNK]) -> Result<(usize, usize), Error> {
    let dir = tree::decode(node)?;
    let gone = match change.out { Some(name) => Some(position(&dir, name).map_err(|_| Error::NotFound)?), None => None };
    // Where the new entry goes among those left: in place of the one taken out under its name (a replace).
    let at = match change.add {
        Some(e) => {
            let at = match position(&dir, e.name) { Err(at) => at, Ok(at) if gone == Some(at) => at, Ok(_) => return Err(Error::Exists) };
            Some(if gone.is_some_and(|g| g < at) { at - 1 } else { at })
        }
        None => None,
    };
    let count = dir.len() - gone.is_some() as usize + at.is_some() as usize;
    let entry = |i: usize| {
        let e = match (change.add, at) {
            (Some(e), Some(at)) if i == at => e,
            _ => {
                let i = if at.is_some_and(|at| i > at) { i - 1 } else { i };
                dir.entry(if gone.is_some_and(|g| i >= g) { i + 1 } else { i })
            }
        };
        match change.sizes.iter().flatten().find(|(name, _)| *name == e.name) { Some(&(_, size)) => Entry { size, ..e }, None => e }
    };
    match tree::encode_each(count, entry, out) {
        Ok(len) => Ok((len, count)),
        // Over a block.
        Err(dag::Error::TooLarge) => Err(Error::TooLarge),
        Err(e) => Err(e.into()),
    }
}
