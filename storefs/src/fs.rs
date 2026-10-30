//! storefs's logic (issue 311-STO-0032; MC-3.4, 4.1, 4.3, 4.9): `vfs.wit`'s files and directories over the store's
//! names `fs/<volume>`, each a directory tree of `tree`. `root` opens a volume's tree at the name's current version as a
//! working tree (`worktree`) of its own; the handles opened from it belong to its client (PID and badge) and never allow
//! more than the one they came from. Reads go to the store, every block checked against its CID. A file written is held
//! as the chunks it changed, at most CHUNKS for every file at once, and at its `flush` or `close` stored as an object of
//! `dag` and put into the tree. The flush or close of a file that may be written, and the flush of a directory that may,
//! publish the tree by the name's compare-and-swap: if another writer published first, nothing is published (Conflict)
//! and the tree keeps its changes, never merged. No heap and no system calls; every bound is fixed and named, and a
//! request past one is refused with nothing changed: tests/storefs_host.rs.
use crate::cid::{self, Cid, Codec};
use crate::dag::{self, Blocks, Builder, CHUNK};
use crate::tree::{self, Kind, NAME_MAX};
use crate::vfs;
use crate::worktree::{self, Names, Worktree};

/// Trees open at once: each `root` opens one, and the handles opened from it share it.
pub const TREES: usize = 4;
/// Handles open at once, of every client.
pub const HANDLES: usize = 64;
/// Files open at once: the handles of one path in one tree share one.
pub const FILES: usize = 32;
/// Chunks of written files held at once, of CHUNK bytes each (256 KiB): a write past them is Full until a flush.
pub const CHUNKS: usize = 16;
/// Bytes of a handle's path from its tree's root at most.
pub const PATH: usize = 512;
/// Entries a `list` gives at most (`vfs.wit`'s `list<entry, 16>`).
pub const PAGE: usize = 16;
/// Bytes of a volume's name at most (`vfs.wit`'s `string<16>`).
pub const VOLUME_MAX: usize = 16;
/// `open`'s mode bits, as `vfs.wit` has them: write, create, truncate, refuse an existing file.
pub const WRITE: u8 = 1;
pub const CREATE: u8 = 2;
pub const TRUNCATE: u8 = 4;
pub const NEW: u8 = 8;
// A volume's name in the store: the prefix, then the volume.
const PREFIX: &[u8] = b"fs/";
const NAME: usize = PREFIX.len() + VOLUME_MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// No entry at the path, or a name that is no volume.
    NotFound,
    /// The name is taken.
    Exists,
    /// A directory removed while it holds entries.
    NotEmpty,
    /// A handle not the client's, two handles of two trees, `.` or `..` in a path, no name where an entry is needed, a
    /// directory moved into itself or under it.
    Invalid,
    /// A change through a handle that may not write.
    Denied,
    /// A name a directory refuses.
    Name,
    /// A file where a directory belongs.
    NotDirectory,
    /// A directory where a file belongs.
    IsDirectory,
    /// HANDLES, TREES or FILES are all in use: nothing was changed.
    Handles,
    /// Past CHUNKS, or past what the working tree holds (`worktree::DIRS`, `BYTES`): nothing was changed, and a flush
    /// lets them go.
    Full,
    /// A path past PATH, a directory over a block, a tree past `tree::DEPTH`, a file past u64 bytes: nothing was changed.
    TooLarge,
    /// The name is at another version than the one the tree read: nothing was published.
    Conflict { read: u64, current: u64 },
    /// The store, or a block read from it.
    Store(dag::Error),
}

impl From<dag::Error> for Error {
    fn from(e: dag::Error) -> Self { Error::Store(e) }
}

impl From<worktree::Error> for Error {
    fn from(e: worktree::Error) -> Self {
        match e {
            worktree::Error::Name => Error::Name, worktree::Error::NotFound => Error::NotFound, worktree::Error::NotDirectory => Error::NotDirectory,
            worktree::Error::Exists => Error::Exists, worktree::Error::NotEmpty => Error::NotEmpty, worktree::Error::Inside => Error::Invalid,
            worktree::Error::Full => Error::Full, worktree::Error::TooLarge => Error::TooLarge,
            worktree::Error::Conflict { read, current } => Error::Conflict { read, current }, worktree::Error::Store(e) => Error::Store(e),
        }
    }
}

impl Error {
    /// The error of `vfs.wit` 2.5 that says it. A conflict is `io` until 2.6 has `conflict`: as a write that did not
    /// reach the medium, it stored nothing under the name, so no client takes its data for stored; `invalid` or `denied`
    /// would blame the request or its rights. The bounds are `no-space`, the tables' `handles`.
    pub fn wire(self) -> vfs::Error {
        match self {
            Error::NotFound => vfs::Error::NotFound, Error::Exists => vfs::Error::Exists, Error::NotEmpty => vfs::Error::NotEmpty,
            Error::Invalid => vfs::Error::Invalid, Error::Denied => vfs::Error::Denied, Error::Name => vfs::Error::Name,
            Error::NotDirectory => vfs::Error::NotDirectory, Error::IsDirectory => vfs::Error::IsDirectory, Error::Handles => vfs::Error::Handles,
            Error::Full | Error::TooLarge | Error::Store(dag::Error::Full) => vfs::Error::NoSpace,
            Error::Conflict { .. } | Error::Store(_) => vfs::Error::Io,
        }
    }
}

/// A client as the kernel names it to a service: its PID and the badge of its capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Client { pub pid: u64, pub badge: u16 }

/// A name's current version and root, as `root` reads them: the block store's `snapshot`, or its logic in tests.
pub trait Roots {
    /// The current version of `name` and its root: (0, none) for no such name, none for a removed one.
    fn current(&mut self, name: &str) -> Result<(u64, Option<Cid>), dag::Error>;
}

/// What `stat` gives: the entry's name (none for a tree's root), what it is, and its size (a file's bytes, a
/// directory's entries).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry { pub kind: Kind, pub size: u64, len: usize, name: [u8; NAME_MAX] }
impl Entry { pub fn name(&self) -> &[u8] { &self.name[..self.len] } }

/// What `volume` gives: the volume's name, whether the handle may write, and the version and root its tree was read at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Volume { pub writable: bool, pub version: u64, pub root: Option<Cid>, len: usize, name: [u8; VOLUME_MAX] }
impl Volume { pub fn name(&self) -> &str { core::str::from_utf8(&self.name[..self.len]).unwrap_or("") } }

// A tree's volume, in lower case, and the handles open in it (none: free).
#[derive(Clone, Copy)]
struct View { users: usize, len: usize, volume: [u8; VOLUME_MAX] }

impl View {
    fn volume(&self) -> &str { core::str::from_utf8(&self.volume[..self.len]).unwrap_or("") }
    // The volume's name in the store.
    fn name<'n>(&self, out: &'n mut [u8; NAME]) -> &'n str {
        out[..PREFIX.len()].copy_from_slice(PREFIX);
        out[PREFIX.len()..PREFIX.len() + self.len].copy_from_slice(&self.volume[..self.len]);
        core::str::from_utf8(&out[..PREFIX.len() + self.len]).unwrap_or("")
    }
}

// A handle: its client, its tree, a directory's or a file's (`file`), whether it may write, and its path from the
// tree's root.
#[derive(Clone, Copy)]
struct Handle { used: bool, pid: u64, badge: u16, tree: usize, directory: bool, writable: bool, file: usize, len: usize, path: [u8; PATH] }
impl Handle { fn path(&self) -> &[u8] { &self.path[..self.len] } }

// A file open in a tree, its handles' (none: free): the object it was read as (`base`, read below `stored` bytes alone),
// its size now, and whether it changed since.
#[derive(Clone, Copy)]
struct File { users: usize, tree: usize, base: [u8; cid::BYTES], stored: u64, size: u64, changed: bool }

// Whose a chunk is: which file, and the chunk's index in it.
#[derive(Clone, Copy)]
struct Slot { used: bool, file: usize, index: u64 }

/// The service's state: 786 KiB, all zeros when new, so a program keeps it in a static in `.bss` (`new` is `const`).
pub struct Fs {
    trees: [Worktree; TREES],
    views: [View; TREES],
    handles: [Handle; HANDLES],
    files: [File; FILES],
    slots: [Slot; CHUNKS],
    chunks: [[u8; CHUNK]; CHUNKS],
    builder: Builder,
    buffer: [u8; CHUNK],
    data: [u8; CHUNK],
}

impl Fs {
    pub const fn new() -> Self {
        const TREE: Worktree = Worktree::new();
        const VIEW: View = View { users: 0, len: 0, volume: [0; VOLUME_MAX] };
        const HANDLE: Handle = Handle { used: false, pid: 0, badge: 0, tree: 0, directory: false, writable: false, file: 0, len: 0, path: [0; PATH] };
        const FILE: File = File { users: 0, tree: 0, base: [0; cid::BYTES], stored: 0, size: 0, changed: false };
        const SLOT: Slot = Slot { used: false, file: 0, index: 0 };
        Fs { trees: [TREE; TREES], views: [VIEW; TREES], handles: [HANDLE; HANDLES], files: [FILE; FILES], slots: [SLOT; CHUNKS],
             chunks: [[0; CHUNK]; CHUNKS], builder: Builder::new(), buffer: [0; CHUNK], data: [0; CHUNK] }
    }

    /// What it holds: handles, trees, files and chunks (at most HANDLES, TREES, FILES and CHUNKS).
    pub fn held(&self) -> (usize, usize, usize, usize) {
        (self.handles.iter().filter(|h| h.used).count(), self.views.iter().filter(|v| v.users > 0).count(),
         self.files.iter().filter(|f| f.users > 0).count(), self.slots.iter().filter(|c| c.used).count())
    }

    /// The root of `volume` for `client`: a tree of its own, read at the current version of the name `fs/<volume>` (none:
    /// the empty tree), that may write if `writable` says so of the client's badge and the volume. A volume is 1 to
    /// VOLUME_MAX bytes of `A-Z a-z 0-9 . _ -`, its case ignored; another name is NotFound.
    pub fn root<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, volume: &str, writable: impl Fn(u16, &str) -> bool) -> Result<u32, Error> {
        let bytes = volume.as_bytes();
        if bytes.is_empty() || bytes.len() > VOLUME_MAX || !bytes.iter().all(|&c| c.is_ascii_alphanumeric() || b"._-".contains(&c)) { return Err(Error::NotFound); }
        let h = self.free()?;
        let t = self.views.iter().position(|v| v.users == 0).ok_or(Error::Handles)?;
        let mut view = View { users: 0, len: bytes.len(), volume: [0; VOLUME_MAX] };
        for (to, c) in view.volume.iter_mut().zip(bytes) { *to = c.to_ascii_lowercase(); }
        let (version, root) = store.current(view.name(&mut [0; NAME]))?;
        self.trees[t].start(root, version);
        // Its root is read: a directory, checked against its CID.
        self.trees[t].lookup(store, "")?;
        let may = writable(client.badge, view.volume());
        self.views[t] = view;
        self.add(h, client, t, true, may, 0, b"");
        Ok(h as u32)
    }

    /// A directory below directory `dir`; with `create`, each one missing on the way is made if `dir` may write.
    pub fn open_dir<S: Blocks>(&mut self, store: &mut S, client: Client, dir: u32, path: &str, create: bool) -> Result<u32, Error> {
        let d = self.directory(client, dir)?;
        let (t, may, from) = (self.handles[d].tree, self.handles[d].writable, self.handles[d].len);
        let mut out = [0; PATH];
        let len = join(self.handles[d].path(), path, &mut out)?;
        let h = self.free()?;
        // Down the path, each name a directory; those made are taken out again if one after them is refused.
        let (tree, mut made) = (&mut self.trees[t], None);
        for end in (from + 1..len).filter(|&i| out[i] == b'/').chain([len]).filter(|&end| end > from) {
            let at = text(&out[..end])?;
            let step = match tree.lookup(store, at) {
                Ok(found) if found.kind == Kind::Directory => Ok(()),
                Ok(_) => Err(Error::NotDirectory),
                Err(worktree::Error::NotFound) if create && !may => Err(Error::Denied),
                Err(worktree::Error::NotFound) if create => tree.mkdir(store, at).map(|()| { made.get_or_insert(end); }).map_err(Error::from),
                Err(e) => Err(e.into()),
            };
            if let Err(e) = step {
                for undo in made.map_or(end..end, |first| first..end).rev().filter(|&i| out[i] == b'/') {
                    if let Ok(at) = text(&out[..undo]) { let _ = tree.remove(store, at); }
                }
                return Err(e);
            }
        }
        self.add(h, client, t, true, may, 0, &out[..len]);
        Ok(h as u32)
    }

    /// A file below directory `dir`, by `mode` (WRITE, CREATE, TRUNCATE and NEW): the handles of one file in a tree share
    /// what is written through them. WRITE, CREATE and TRUNCATE need a `dir` that may write, and the handle may write with
    /// WRITE alone. A file made or emptied here is an edit of the tree, published with the next publication.
    pub fn open<S: Blocks>(&mut self, store: &mut S, client: Client, dir: u32, path: &str, mode: u8) -> Result<u32, Error> {
        let d = self.directory(client, dir)?;
        let (t, from) = (self.handles[d].tree, self.handles[d].len);
        if mode & (WRITE | CREATE | TRUNCATE) != 0 && !self.handles[d].writable { return Err(Error::Denied); }
        let mut out = [0; PATH];
        let len = join(self.handles[d].path(), path, &mut out)?;
        if len == from { return Err(Error::Invalid); }
        let (path, h) = (&out[..len], self.free()?);
        let shared = self.shared(t, path);
        let f = match shared { Some(f) => f, None => self.files.iter().position(|f| f.users == 0).ok_or(Error::Handles)? };
        let at = text(path)?;
        let tree = &mut self.trees[t];
        let (mut base, mut size) = match tree.lookup(store, at) {
            Ok(_) if mode & NEW != 0 => return Err(Error::Exists),
            Ok(found) if found.kind == Kind::Directory => return Err(Error::IsDirectory),
            Ok(found) => (found.cid.ok_or(Error::Store(dag::Error::Format))?, found.size),
            Err(worktree::Error::NotFound) if mode & CREATE != 0 => {
                let empty = empty(store)?;
                tree.replace(store, at, &empty, 0)?;
                (empty, 0)
            }
            Err(e) => return Err(e.into()),
        };
        if mode & TRUNCATE != 0 {
            if size != 0 {
                let empty = empty(store)?;
                tree.replace(store, at, &empty, 0)?;
                (base, size) = (empty, 0);
            }
            // What its other handles wrote goes too.
            if let Some(f) = shared {
                self.drop_chunks(f, 0);
                let file = &mut self.files[f];
                (file.base, file.stored, file.size, file.changed) = (base.to_bytes(), 0, 0, false);
            }
        }
        if shared.is_none() { self.files[f] = File { users: 0, tree: t, base: base.to_bytes(), stored: size, size, changed: false }; }
        self.add(h, client, t, false, mode & WRITE != 0, f, path);
        Ok(h as u32)
    }

    /// Up to `out.len()` bytes of a file from `offset`, as written through its handles; fewer at its end.
    pub fn read<S: Blocks>(&mut self, store: &mut S, client: Client, file: u32, offset: u64, out: &mut [u8]) -> Result<usize, Error> {
        let h = self.handle(client, file)?;
        if self.handles[h].directory { return Err(Error::IsDirectory); }
        let f = self.handles[h].file;
        let File { base, stored, size, .. } = self.files[f];
        if offset >= size { return Ok(0); }
        let want = (size - offset).min(out.len() as u64) as usize;
        let mut done = 0;
        while done < want {
            let at = offset + done as u64;
            let within = (at % CHUNK as u64) as usize;
            let n = (CHUNK - within).min(want - done);
            match self.chunk(f, at / CHUNK as u64) {
                Some(c) => out[done..done + n].copy_from_slice(&self.chunks[c][within..within + n]),
                None => from_base(store, &base, stored, at, &mut out[done..done + n], &mut self.buffer)?,
            }
            done += n;
        }
        Ok(done)
    }

    /// Writes `data` into a file at `offset`, through a handle that may write; the file grows, a gap reading as zeros.
    /// The chunks it changes are held until the file is stored; a write that needs more than are free is Full.
    pub fn write<S: Blocks>(&mut self, store: &mut S, client: Client, file: u32, offset: u64, data: &[u8]) -> Result<usize, Error> {
        let f = self.writer(client, file)?;
        let end = offset.checked_add(data.len() as u64).ok_or(Error::TooLarge)?;
        if data.is_empty() { return Ok(0); }
        let (first, last) = (offset / CHUNK as u64, (end - 1) / CHUNK as u64);
        // Every chunk it needs is taken first, or none.
        let needed = (first..=last).filter(|&i| self.chunk(f, i).is_none()).count();
        if needed > self.slots.iter().filter(|c| !c.used).count() { return Err(Error::Full); }
        let File { base, stored, .. } = self.files[f];
        let (mut taken, mut n) = ([0; CHUNKS], 0);
        for index in first..=last {
            if self.chunk(f, index).is_some() { continue; }
            let c = self.slots.iter().position(|c| !c.used).unwrap();
            let at = index * CHUNK as u64;
            // A chunk the write covers is not read.
            if !(offset <= at && end - at >= CHUNK as u64) {
                if let Err(e) = from_base(store, &base, stored, at, &mut self.chunks[c], &mut self.buffer) {
                    for &c in &taken[..n] { self.slots[c].used = false; }
                    return Err(e);
                }
            }
            self.slots[c] = Slot { used: true, file: f, index };
            (taken[n], n) = (c, n + 1);
        }
        let mut done = 0;
        while done < data.len() {
            let at = offset + done as u64;
            let within = (at % CHUNK as u64) as usize;
            let n = (CHUNK - within).min(data.len() - done);
            let c = self.chunk(f, at / CHUNK as u64).unwrap();
            self.chunks[c][within..within + n].copy_from_slice(&data[done..done + n]);
            done += n;
        }
        let file = &mut self.files[f];
        (file.size, file.changed) = (file.size.max(end), true);
        Ok(done)
    }

    /// Sets the size of a file, through a handle that may write: what lies past it goes, and a longer file reads zeros.
    pub fn truncate(&mut self, client: Client, file: u32, size: u64) -> Result<(), Error> {
        let f = self.writer(client, file)?;
        if size == self.files[f].size { return Ok(()); }
        if size < self.files[f].size {
            // The chunks past it let go, the rest of its last zeroed; the object read as no longer read past it.
            self.drop_chunks(f, size.div_ceil(CHUNK as u64));
            let within = (size % CHUNK as u64) as usize;
            if within > 0 {
                if let Some(c) = self.chunk(f, size / CHUNK as u64) { self.chunks[c][within..].fill(0); }
            }
            self.files[f].stored = self.files[f].stored.min(size);
        }
        (self.files[f].size, self.files[f].changed) = (size, true);
        Ok(())
    }

    /// The entry of an open file or directory: a file's size is what was written to it, stored or not.
    pub fn stat<S: Blocks>(&mut self, store: &mut S, client: Client, handle: u32) -> Result<Entry, Error> {
        let h = self.handle(client, handle)?;
        let handle = &self.handles[h];
        let path = handle.path();
        let name = path.rsplit(|&c| c == b'/').next().unwrap_or(b"");
        let (kind, size) = match handle.directory {
            true => (Kind::Directory, self.trees[handle.tree].lookup(store, text(path)?)?.size),
            false => (Kind::File, self.files[handle.file].size),
        };
        let mut entry = Entry { kind, size, len: name.len(), name: [0; NAME_MAX] };
        entry.name[..name.len()].copy_from_slice(name);
        Ok(entry)
    }

    /// The entries of directory `dir` from index `start`, PAGE at most, in the order of their names, each to `visit` with
    /// what it is and its size; returns how many (none: the end). A file open in the tree has the size written to it.
    pub fn list<S: Blocks>(&mut self, store: &mut S, client: Client, dir: u32, start: u32, mut visit: impl FnMut(&[u8], Kind, u64)) -> Result<usize, Error> {
        let d = self.directory(client, dir)?;
        let (trees, handles, files) = (&mut self.trees, &self.handles, &self.files);
        let (t, path) = (handles[d].tree, handles[d].path());
        let (mut index, mut given) = (0, 0);
        trees[t].list(store, text(path)?, |name, found| {
            if index >= start as usize && given < PAGE {
                let open = handles.iter().find(|h| h.used && h.tree == t && !h.directory && child(h.path(), path, name));
                visit(name, found.kind, open.map_or(found.size, |h| files[h.file].size));
                given += 1;
            }
            index += 1;
        })?;
        Ok(given)
    }

    /// Removes a file or an empty directory below directory `dir`, which must be able to write; its handles are closed.
    pub fn remove<S: Blocks>(&mut self, store: &mut S, client: Client, dir: u32, path: &str) -> Result<(), Error> {
        let d = self.directory(client, dir)?;
        let (t, from) = (self.handles[d].tree, self.handles[d].len);
        let mut out = [0; PATH];
        let len = join(self.handles[d].path(), path, &mut out)?;
        if len == from { return Err(Error::Invalid); }
        if !self.handles[d].writable { return Err(Error::Denied); }
        self.trees[t].remove(store, text(&out[..len])?)?;
        for h in 0..HANDLES {
            if self.handles[h].used && self.handles[h].tree == t && self.handles[h].path() == &out[..len] { self.release(h); }
        }
        Ok(())
    }

    /// Moves `from` below directory `dir` to `to` below directory `target`, both of one tree and able to write; the
    /// handles of what moves follow it.
    pub fn rename<S: Blocks>(&mut self, store: &mut S, client: Client, dir: u32, from: &str, target: u32, to: &str) -> Result<(), Error> {
        let (d, e) = (self.directory(client, dir)?, self.directory(client, target)?);
        let t = self.handles[d].tree;
        if self.handles[e].tree != t { return Err(Error::Invalid); }
        let (mut a, mut b) = ([0; PATH], [0; PATH]);
        let (la, lb) = (join(self.handles[d].path(), from, &mut a)?, join(self.handles[e].path(), to, &mut b)?);
        if la == self.handles[d].len || lb == self.handles[e].len { return Err(Error::Invalid); }
        if !self.handles[d].writable || !self.handles[e].writable { return Err(Error::Denied); }
        let (from, to) = (&a[..la], &b[..lb]);
        // Each path that follows must fit.
        if self.handles.iter().any(|h| h.used && h.tree == t && under(h.path(), from) && h.len - la + lb > PATH) { return Err(Error::TooLarge); }
        self.trees[t].rename(store, text(from)?, text(to)?)?;
        for h in self.handles.iter_mut().filter(|h| h.used && h.tree == t && under(h.path(), from)) {
            h.path.copy_within(la..h.len, lb);
            h.path[..lb].copy_from_slice(to);
            h.len = h.len - la + lb;
        }
        Ok(())
    }

    /// The volume a handle is on.
    pub fn volume(&self, client: Client, handle: u32) -> Result<Volume, Error> {
        let h = self.handle(client, handle)?;
        let (view, (root, version)) = (&self.views[self.handles[h].tree], self.trees[self.handles[h].tree].base());
        Ok(Volume { writable: self.handles[h].writable, version, root, len: view.len, name: view.volume })
    }

    /// Through a handle that may write, stores what was written (a file's handle: its file; a directory's: every file
    /// written below it) and publishes the tree if it changed, by the name's compare-and-swap. Another writer's version
    /// is Conflict, nothing published; the tree keeps its changes and stays at the version it read. A file the working
    /// tree cannot take for its bounds is put after the edits before it are published. A handle that may not write has
    /// nothing to flush.
    pub fn flush<S: Blocks + Names>(&mut self, store: &mut S, client: Client, handle: u32) -> Result<(), Error> {
        let h = self.handle(client, handle)?;
        self.publish(store, h)
    }

    /// Closes a handle: a file's that may write is flushed first, and the handle is closed whatever that answers. The
    /// last handle of a file lets its chunks go, and the last of a tree drops what it did not publish.
    pub fn close<S: Blocks + Names>(&mut self, store: &mut S, client: Client, handle: u32) -> Result<(), Error> {
        let h = self.handle(client, handle)?;
        let published = if self.handles[h].directory { Ok(()) } else { self.publish(store, h) };
        self.release(h);
        published
    }

    /// Closes every handle of the client `pid`, one that has ended, without publishing; returns how many.
    pub fn forget(&mut self, pid: u64) -> usize {
        let gone = (0..HANDLES).filter(|&h| self.handles[h].used && self.handles[h].pid == pid).count();
        for h in 0..HANDLES { if self.handles[h].used && self.handles[h].pid == pid { self.release(h); } }
        gone
    }

    // A free handle.
    fn free(&self) -> Result<usize, Error> { self.handles.iter().position(|h| !h.used).ok_or(Error::Handles) }

    // Handle `id` if it is `client`'s.
    fn handle(&self, client: Client, id: u32) -> Result<usize, Error> {
        match self.handles.get(id as usize) { Some(h) if h.used && h.pid == client.pid && h.badge == client.badge => Ok(id as usize), _ => Err(Error::Invalid) }
    }

    fn directory(&self, client: Client, id: u32) -> Result<usize, Error> {
        let h = self.handle(client, id)?;
        if self.handles[h].directory { Ok(h) } else { Err(Error::NotDirectory) }
    }

    // The file of a handle that may write.
    fn writer(&self, client: Client, id: u32) -> Result<usize, Error> {
        let h = self.handle(client, id)?;
        match self.handles[h] { Handle { writable: false, .. } => Err(Error::Denied), Handle { directory: true, .. } => Err(Error::IsDirectory), _ => Ok(self.handles[h].file) }
    }

    // The file open at `path` in tree `t`.
    fn shared(&self, t: usize, path: &[u8]) -> Option<usize> {
        self.handles.iter().find(|h| h.used && h.tree == t && !h.directory && h.path() == path).map(|h| h.file)
    }

    #[allow(clippy::too_many_arguments)]
    fn add(&mut self, h: usize, client: Client, tree: usize, directory: bool, writable: bool, file: usize, path: &[u8]) {
        let handle = &mut self.handles[h];
        (handle.used, handle.pid, handle.badge, handle.tree, handle.directory, handle.writable, handle.file, handle.len) = (true, client.pid, client.badge, tree, directory, writable, file, path.len());
        handle.path[..path.len()].copy_from_slice(path);
        self.views[tree].users += 1;
        if !directory { self.files[file].users += 1; }
    }

    // Closes handle `h` without publishing.
    fn release(&mut self, h: usize) {
        let Handle { tree, directory, file, .. } = self.handles[h];
        self.handles[h].used = false;
        self.views[tree].users -= 1;
        if !directory {
            self.files[file].users -= 1;
            if self.files[file].users == 0 { self.drop_chunks(file, 0); }
        }
    }

    // The chunk held for file `f` at `index`.
    fn chunk(&self, f: usize, index: u64) -> Option<usize> { self.slots.iter().position(|c| c.used && c.file == f && c.index == index) }

    // Lets the chunks of file `f` from `index` on go.
    fn drop_chunks(&mut self, f: usize, index: u64) {
        for c in self.slots.iter_mut().filter(|c| c.used && c.file == f && c.index >= index) { c.used = false; }
    }

    // What `flush` does for handle `h`.
    fn publish<S: Blocks + Names>(&mut self, store: &mut S, h: usize) -> Result<(), Error> {
        let Handle { tree, directory, writable, file, .. } = self.handles[h];
        if !writable { return Ok(()); }
        if directory {
            for f in 0..FILES {
                let below = self.handles.iter().any(|o| o.used && !o.directory && o.file == f && under(o.path(), self.handles[h].path()));
                if self.files[f].users > 0 && self.files[f].tree == tree && self.files[f].changed && below { self.store_file(store, f)?; }
            }
        } else if self.files[file].changed {
            self.store_file(store, file)?;
        }
        if self.trees[tree].held().0 == 0 { return Ok(()); }
        self.trees[tree].publish(store, self.views[tree].name(&mut [0; NAME]))?;
        Ok(())
    }

    // Stores file `f` as an object of `dag`, and puts it into its tree in place of the one it was read as.
    fn store_file<S: Blocks + Names>(&mut self, store: &mut S, f: usize) -> Result<(), Error> {
        let File { tree, base, stored, size, .. } = self.files[f];
        self.builder.reset();
        let mut at = 0;
        while at < size {
            let n = (size - at).min(CHUNK as u64) as usize;
            let bytes = match self.chunk(f, at / CHUNK as u64) {
                Some(c) => &self.chunks[c][..n],
                None => {
                    from_base(store, &base, stored, at, &mut self.data[..n], &mut self.buffer)?;
                    &self.data[..n]
                }
            };
            self.builder.write(store, bytes)?;
            at += n as u64;
        }
        let (cid, _) = self.builder.finish(store)?;
        let h = self.handles.iter().position(|h| h.used && !h.directory && h.file == f).unwrap();
        let (trees, path) = (&mut self.trees, text(self.handles[h].path())?);
        match trees[tree].replace(store, path, &cid, size) {
            // Past what the tree holds: the edits before it are published first, which lets them go.
            Err(worktree::Error::Full) => {
                trees[tree].publish(store, self.views[tree].name(&mut [0; NAME]))?;
                trees[tree].replace(store, path, &cid, size)?;
            }
            done => done?,
        }
        self.drop_chunks(f, 0);
        let file = &mut self.files[f];
        (file.base, file.stored, file.changed) = (cid.to_bytes(), size, false);
        Ok(())
    }
}

// Joins the names of `path` to `base` into `out`, empty names skipped; returns the length. `.` and `..` are Invalid,
// as a path never leaves its handle's directory; a name a directory refuses is Name.
fn join(base: &[u8], path: &str, out: &mut [u8; PATH]) -> Result<usize, Error> {
    let names = || path.split('/').filter(|n| !n.is_empty());
    if names().any(|n| n == "." || n == "..") { return Err(Error::Invalid); }
    if !names().all(|n| tree::valid_name(n.as_bytes())) { return Err(Error::Name); }
    let mut len = base.len();
    out[..len].copy_from_slice(base);
    for name in names() {
        let at = len + (len > 0) as usize;
        if at + name.len() > PATH { return Err(Error::TooLarge); }
        if len > 0 { out[len] = b'/'; }
        out[at..at + name.len()].copy_from_slice(name.as_bytes());
        len = at + name.len();
    }
    Ok(len)
}

// A path as the working tree takes it: its names are UTF-8.
fn text(path: &[u8]) -> Result<&str, Error> { core::str::from_utf8(path).map_err(|_| Error::Name) }

// Whether `path` is `top` or lies under it; every path lies under the root's.
fn under(path: &[u8], top: &[u8]) -> bool { top.is_empty() || path == top || (path.len() > top.len() && path.starts_with(top) && path[top.len()] == b'/') }

// Whether `path` is `name` in the directory `dir`.
fn child(path: &[u8], dir: &[u8], name: &[u8]) -> bool {
    if dir.is_empty() { return path == name; }
    path.len() == dir.len() + 1 + name.len() && path.starts_with(dir) && path[dir.len()] == b'/' && path.ends_with(name)
}

// The empty file, stored: a `raw` block of no bytes.
fn empty<S: Blocks>(store: &mut S) -> Result<Cid, Error> {
    let cid = store.put(Codec::Raw, &[])?;
    if cid != Cid::of(Codec::Raw, &[]) { return Err(dag::Error::Corrupt.into()); }
    Ok(cid)
}

// Bytes of a file from `at` that no chunk holds: its object's below `stored`, zeros from there.
fn from_base<S: Blocks>(store: &mut S, base: &[u8; cid::BYTES], stored: u64, at: u64, out: &mut [u8], buffer: &mut [u8; CHUNK]) -> Result<(), Error> {
    let below = stored.saturating_sub(at).min(out.len() as u64) as usize;
    if below > 0 {
        let cid = Cid::from_bytes(base).map_err(|_| dag::Error::Format)?;
        if dag::read_at(store, &cid, at, &mut out[..below], buffer)? != below { return Err(dag::Error::Shape.into()); }
    }
    out[below..].fill(0);
    Ok(())
}
