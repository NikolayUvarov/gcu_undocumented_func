//! storefs's logic (issue 311-STO-0032; MC-3.4, 4.1, 4.3, 4.9): `vfs.wit`'s files and directories over the store's
//! names `fs/<volume>`, each a directory tree of `tree`. `root` opens a volume's tree at the name's current version as a
//! working tree (`worktree`) of its own; the handles opened from it belong to its client (PID and badge) and never allow
//! more than the one they came from. Reads go to the store, every block checked against its CID. A change of a
//! directory (a directory made, a file made or emptied, a removal, a move) is one publication by the name's
//! compare-and-swap. A file written is held as the chunks it changed, and at its `flush` or `close` stored as an object
//! of `dag`, put into the tree and published. If another writer published first, nothing is published (Conflict): the
//! tree drops what it did not publish and follows the name to its current version, never merging. A tree that holds no
//! unpublished data follows the name before each call. A scope (311-STO-0037; MC-3.1, 3.4) is a client confined to one
//! directory, by a badge of its own: its root of the directory's volume is that directory, by its path, and every other
//! root is refused; it may write only if asked and its maker's handle may; its first task, or the one its maker names
//! (`adopt`), is its only user, and it ends with that task, or after a minute if nobody took it up. No heap and no
//! system calls (the clock and whether a task is alive are given); every bound is fixed and named, a client holds at
//! most half of each table, and a request past one is refused with nothing changed: tests/storefs_host.rs.
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
/// Scopes at once: a scope keeps a path and no tree until its user opens its root, and one nobody took up ends after
/// SCOPE_UNUSED_MS; 16 is four for each of the TREES that can be open, for the shell to lend one to each program it starts.
pub const SCOPES: usize = 16;
/// What one client (PID and badge) holds at most of each, and makes at most of SCOPES: half.
pub const TREES_EACH: usize = TREES / 2;
pub const HANDLES_EACH: usize = HANDLES / 2;
pub const FILES_EACH: usize = FILES / 2;
pub const CHUNKS_EACH: usize = CHUNKS / 2;
pub const SCOPES_EACH: usize = SCOPES / 2;
/// A scope's badge is one from here to u16::MAX, as vfs_server's: init's badges are below it.
pub const SCOPE_BADGE_FIRST: u16 = 0x100;
/// Milliseconds a scope nobody took up lasts: a minute, as vfs_server's.
pub const SCOPE_UNUSED_MS: u64 = 60_000;
// How many badges scopes take in turn.
const SCOPE_BADGES: u16 = u16::MAX - SCOPE_BADGE_FIRST + 1;
/// Bytes a file is at most after a write or a truncation: 64 MiB.
pub const FILE_MAX: u64 = 64 << 20;
/// Bytes of a handle's path from its tree's root at most.
pub const PATH: usize = 512;
/// Entries a `list` gives at most (`vfs.wit`'s `list<entry, 16>`).
pub const PAGE: usize = 16;
/// Bytes of a volume's name at most (`vfs.wit`'s `string<16>`).
pub const VOLUME_MAX: usize = 16;
/// The volumes vfs_server holds, never storefs's: a files client asks vfs_server first, and storefs for a volume
/// vfs_server answers `not-found` (the kernel track's way (b), 2026-10-30). So these, and the boot disk's empty name, are
/// NotFound here.
pub const VFS_VOLUMES: [&str; 3] = ["ram", "log", "models"];
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
    /// No entry at the path, a name that is no volume, or no scope of the badge `adopt` names.
    NotFound,
    /// The name is taken.
    Exists,
    /// A directory removed while it holds entries.
    NotEmpty,
    /// A handle not the client's, two handles of two trees, `.` or `..` in a path, no name where an entry is needed, a
    /// directory moved into itself or under it.
    Invalid,
    /// A change through a handle that may not write; a scope's root asked by another task than its user, or of another
    /// volume, or with the badge of no scope; an `adopt` not by the scope's maker, or of a scope another task uses.
    Denied,
    /// A name a directory refuses.
    Name,
    /// A file where a directory belongs.
    NotDirectory,
    /// A directory where a file belongs.
    IsDirectory,
    /// HANDLES, TREES, FILES or SCOPES are all in use, or the client holds its share of them, or a scope's capability
    /// was not made: nothing was changed.
    Handles,
    /// Past CHUNKS, the client's share of them, or what the working tree holds (`worktree::DIRS`, `BYTES`): nothing was
    /// changed, and a flush lets the chunks go.
    Full,
    /// A file past FILE_MAX, a path past PATH, a directory over a block, a tree past `tree::DEPTH`: nothing was changed.
    TooLarge,
    /// The name is at another version than the one the tree read: nothing was published. Also the answer of a file
    /// whose unpublished data such a refusal dropped.
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
    /// The error of `vfs.wit` 2.6 that says it: a conflict is `conflict` (311-KRN-0189), so the client reads the file
    /// again and writes once more. The bounds are `no-space`, the tables' `handles`.
    pub fn wire(self) -> vfs::Error {
        match self {
            Error::NotFound => vfs::Error::NotFound, Error::Exists => vfs::Error::Exists, Error::NotEmpty => vfs::Error::NotEmpty,
            Error::Invalid => vfs::Error::Invalid, Error::Denied => vfs::Error::Denied, Error::Name => vfs::Error::Name,
            Error::NotDirectory => vfs::Error::NotDirectory, Error::IsDirectory => vfs::Error::IsDirectory, Error::Handles => vfs::Error::Handles,
            Error::Full | Error::TooLarge | Error::Store(dag::Error::Full) => vfs::Error::NoSpace,
            Error::Conflict { .. } => vfs::Error::Conflict,
            Error::Store(_) => vfs::Error::Io,
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

/// What `volume` gives: the volume's name, whether the handle may write, and the version and root its tree is at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Volume { pub writable: bool, pub version: u64, pub root: Option<Cid>, len: usize, name: [u8; VOLUME_MAX] }
impl Volume { pub fn name(&self) -> &str { core::str::from_utf8(&self.name[..self.len]).unwrap_or("") } }

/// A scope as `scoped` gives it: whether it may write, its maker, its user once taken up, its volume and its directory's
/// path from the volume's root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scoped<'s> { pub writable: bool, pub maker: Client, pub user: Option<u64>, pub volume: &'s str, pub path: &'s str }

// A tree's client, its volume in lower case, and the handles open in it (none: free).
#[derive(Clone, Copy)]
struct View { users: usize, pid: u64, badge: u16, len: usize, volume: [u8; VOLUME_MAX] }

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
// its size now, and whether it changed since. A file whose change a conflict dropped is `stale`, with that conflict's
// versions.
#[derive(Clone, Copy)]
struct File { users: usize, tree: usize, base: [u8; cid::BYTES], stored: u64, size: u64, changed: bool, stale: bool, read: u64, current: u64 }

// Whose a chunk is: which file, and the chunk's index in it.
#[derive(Clone, Copy)]
struct Slot { used: bool, file: usize, index: u64 }

// A scope: its badge and the capability minted with it, its maker, its user once `taken`, when it was made (ms),
// whether it may write, its volume in lower case, and its directory's path from the volume's root (`depth` bytes).
#[derive(Clone, Copy)]
struct Scope { used: bool, badge: u16, cap: usize, maker: Client, taken: bool, user: u64, made: u64, writable: bool, len: usize, volume: [u8; VOLUME_MAX], depth: usize, path: [u8; PATH] }

impl Scope {
    fn volume(&self) -> &str { core::str::from_utf8(&self.volume[..self.len]).unwrap_or("") }
    fn path(&self) -> &str { core::str::from_utf8(&self.path[..self.depth]).unwrap_or("") }
}

/// The service's state: 796 KiB, all zeros when new, so a program keeps it in a static in `.bss` (`new` is `const`).
pub struct Fs {
    trees: [Worktree; TREES],
    views: [View; TREES],
    handles: [Handle; HANDLES],
    files: [File; FILES],
    slots: [Slot; CHUNKS],
    chunks: [[u8; CHUNK]; CHUNKS],
    scopes: [Scope; SCOPES],
    // The next scope's badge, from SCOPE_BADGE_FIRST.
    turn: u16,
    builder: Builder,
    buffer: [u8; CHUNK],
    data: [u8; CHUNK],
}

impl Fs {
    pub const fn new() -> Self {
        const TREE: Worktree = Worktree::new();
        const VIEW: View = View { users: 0, pid: 0, badge: 0, len: 0, volume: [0; VOLUME_MAX] };
        const HANDLE: Handle = Handle { used: false, pid: 0, badge: 0, tree: 0, directory: false, writable: false, file: 0, len: 0, path: [0; PATH] };
        const FILE: File = File { users: 0, tree: 0, base: [0; cid::BYTES], stored: 0, size: 0, changed: false, stale: false, read: 0, current: 0 };
        const SLOT: Slot = Slot { used: false, file: 0, index: 0 };
        const SCOPE: Scope = Scope { used: false, badge: 0, cap: 0, maker: Client { pid: 0, badge: 0 }, taken: false, user: 0, made: 0, writable: false, len: 0, volume: [0; VOLUME_MAX], depth: 0, path: [0; PATH] };
        Fs { trees: [TREE; TREES], views: [VIEW; TREES], handles: [HANDLE; HANDLES], files: [FILE; FILES], slots: [SLOT; CHUNKS],
             chunks: [[0; CHUNK]; CHUNKS], scopes: [SCOPE; SCOPES], turn: 0, builder: Builder::new(), buffer: [0; CHUNK], data: [0; CHUNK] }
    }

    /// What it holds: handles, trees, files and chunks (at most HANDLES, TREES, FILES and CHUNKS).
    pub fn held(&self) -> (usize, usize, usize, usize) {
        (self.handles.iter().filter(|h| h.used).count(), self.views.iter().filter(|v| v.users > 0).count(),
         self.files.iter().filter(|f| f.users > 0).count(), self.slots.iter().filter(|c| c.used).count())
    }

    /// What `client` holds of them: handles, trees, files and chunks (at most HANDLES_EACH, TREES_EACH, FILES_EACH and
    /// CHUNKS_EACH).
    pub fn held_by(&self, client: Client) -> (usize, usize, usize, usize) {
        let own = |t: usize| self.views[t].users > 0 && self.views[t].pid == client.pid && self.views[t].badge == client.badge;
        (self.handles.iter().filter(|h| h.used && h.pid == client.pid && h.badge == client.badge).count(), (0..TREES).filter(|&t| own(t)).count(),
         self.files.iter().filter(|f| f.users > 0 && own(f.tree)).count(), self.slots.iter().filter(|c| c.used && own(self.files[c.file].tree)).count())
    }

    /// The root of `volume` for `client`: a tree of its own, read at the current version of the name `fs/<volume>` (none:
    /// the empty tree), that may write if `writable` says so of the client's badge and the volume. A volume is 1 to
    /// VOLUME_MAX bytes of `A-Z a-z 0-9 . _ -`, its case ignored, and none of VFS_VOLUMES; another name is NotFound. A
    /// scope's client (a badge from SCOPE_BADGE_FIRST) gets the scope's directory instead, on its volume alone.
    pub fn root<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, volume: &str, writable: impl Fn(u16, &str) -> bool) -> Result<u32, Error> {
        if client.badge >= SCOPE_BADGE_FIRST { return self.scoped_root(store, client, volume); }
        let bytes = volume.as_bytes();
        if bytes.is_empty() || bytes.len() > VOLUME_MAX || !bytes.iter().all(|&c| c.is_ascii_alphanumeric() || b"._-".contains(&c)) { return Err(Error::NotFound); }
        if VFS_VOLUMES.iter().any(|v| v.eq_ignore_ascii_case(volume)) { return Err(Error::NotFound); }
        let mut view = View { users: 0, pid: client.pid, badge: client.badge, len: bytes.len(), volume: [0; VOLUME_MAX] };
        for (to, c) in view.volume.iter_mut().zip(bytes) { *to = c.to_ascii_lowercase(); }
        let may = writable(client.badge, view.volume());
        self.open_root(store, client, view, b"", may)
    }

    /// A client confined to directory `dir`, a handle of `client`'s (`scope`): its root of the directory's volume is that
    /// directory, by its path from the volume's root, opened at the name's current version as any root; every other root
    /// is Denied. It may write only if `writable` and `dir` may (never more than `dir`, MC-3.4). Its badge is the next
    /// from SCOPE_BADGE_FIRST that no scope holds, wrapping, and `mint` makes the service's capability with that badge
    /// (none: Handles), kept with the scope so that its end revokes every copy. SCOPES at most, SCOPES_EACH made by a
    /// client: past them Handles. A refusal changes nothing. Returns the badge and the capability.
    pub fn scope<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, dir: u32, writable: bool, now: u64, mint: impl FnOnce(u16) -> Option<usize>) -> Result<(u16, usize), Error> {
        let d = self.directory(store, client, dir)?;
        if self.scopes_held_by(client) >= SCOPES_EACH { return Err(Error::Handles); }
        let s = self.scopes.iter().position(|s| !s.used).ok_or(Error::Handles)?;
        let mut turn = self.turn;
        let badge = loop {
            let badge = SCOPE_BADGE_FIRST + turn;
            turn = (turn + 1) % SCOPE_BADGES;
            if self.scope_of(badge).is_none() { break badge; }
        };
        let cap = mint(badge).ok_or(Error::Handles)?;
        let (handle, view) = (&self.handles[d], &self.views[self.handles[d].tree]);
        let mut scope = Scope { used: true, badge, cap, maker: client, taken: false, user: 0, made: now, writable: writable && handle.writable, len: view.len, volume: view.volume, depth: handle.len, path: [0; PATH] };
        scope.path[..handle.len].copy_from_slice(handle.path());
        (self.scopes[s], self.turn) = (scope, turn);
        Ok((badge, cap))
    }

    /// Makes scope `badge` task `task`'s alone, as if that task had used it first (`adopt`): it then lasts as long as the
    /// task. Only the task that made it may ask, and a scope another task uses already is Denied; no such scope is
    /// NotFound.
    pub fn adopt(&mut self, client: Client, badge: u16, task: u64) -> Result<(), Error> {
        let s = self.scope_of(badge).ok_or(Error::NotFound)?;
        let scope = &mut self.scopes[s];
        if scope.maker.pid != client.pid || (scope.taken && scope.user != task) { return Err(Error::Denied); }
        (scope.taken, scope.user) = (true, task);
        Ok(())
    }

    /// Ends each scope whose user has ended, by `alive` of its PID, and each nobody took up within SCOPE_UNUSED_MS of
    /// `now` (ms): its badge and capability go to `ended`, for the service to revoke, and its client's handles are
    /// closed unpublished, so they are Invalid. Returns how many.
    pub fn sweep_scopes(&mut self, now: u64, alive: impl Fn(u64) -> bool, mut ended: impl FnMut(u16, usize)) -> usize {
        let mut gone = 0;
        for s in 0..SCOPES {
            let Scope { used, badge, cap, taken, user, made, .. } = self.scopes[s];
            if !used { continue; }
            let over = if taken { !alive(user) } else { now.saturating_sub(made) > SCOPE_UNUSED_MS };
            if !over { continue; }
            self.scopes[s].used = false;
            for h in 0..HANDLES {
                if self.handles[h].used && self.handles[h].badge == badge { self.release(h); }
            }
            ended(badge, cap);
            gone += 1;
        }
        gone
    }

    /// Scope `badge` as it is now, if there is one.
    pub fn scoped(&self, badge: u16) -> Option<Scoped<'_>> {
        let scope = &self.scopes[self.scope_of(badge)?];
        Some(Scoped { writable: scope.writable, maker: scope.maker, user: scope.taken.then_some(scope.user), volume: scope.volume(), path: scope.path() })
    }

    /// Scopes, at most SCOPES.
    #[allow(dead_code)] // the host tests use it
    pub fn scopes_held(&self) -> usize { self.scopes.iter().filter(|s| s.used).count() }

    /// Scopes `client` made, at most SCOPES_EACH.
    pub fn scopes_held_by(&self, client: Client) -> usize { self.scopes.iter().filter(|s| s.used && s.maker == client).count() }

    /// A directory below directory `dir`; with `create`, each one missing on the way is made if `dir` may write, all of
    /// them one publication.
    pub fn open_dir<S: Blocks + Names + Roots>(&mut self, store: &mut S, client: Client, dir: u32, path: &str, create: bool) -> Result<u32, Error> {
        let d = self.directory(store, client, dir)?;
        let (t, may, from) = (self.handles[d].tree, self.handles[d].writable, self.handles[d].len);
        let mut out = [0; PATH];
        let len = join(self.handles[d].path(), path, &mut out)?;
        let h = self.free(client)?;
        // Down the path, each name a directory; a refusal drops those made.
        let mut made = false;
        for end in (from + 1..len).filter(|&i| out[i] == b'/').chain([len]).filter(|&end| end > from) {
            let at = text(&out[..end])?;
            let step = match self.trees[t].lookup(store, at) {
                Ok(found) if found.kind == Kind::Directory => Ok(()),
                Ok(_) => Err(Error::NotDirectory),
                Err(worktree::Error::NotFound) if create && !may => Err(Error::Denied),
                Err(worktree::Error::NotFound) if create => self.trees[t].mkdir(store, at).map_err(Error::from),
                Err(e) => Err(e.into()),
            };
            match step {
                Ok(()) => made |= self.trees[t].held().0 > 0,
                Err(e) => { self.drop_edits(t); return Err(e); }
            }
        }
        if made { self.commit(store, t)?; }
        self.add(h, client, t, true, may, 0, &out[..len]);
        Ok(h as u32)
    }

    /// A file below directory `dir`, by `mode` (WRITE, CREATE, TRUNCATE and NEW): the handles of one file in a tree share
    /// what is written through them. WRITE, CREATE and TRUNCATE need a `dir` that may write, and the handle may write with
    /// WRITE alone. A file made or emptied here is one publication.
    pub fn open<S: Blocks + Names + Roots>(&mut self, store: &mut S, client: Client, dir: u32, path: &str, mode: u8) -> Result<u32, Error> {
        let d = self.directory(store, client, dir)?;
        let (t, from) = (self.handles[d].tree, self.handles[d].len);
        if mode & (WRITE | CREATE | TRUNCATE) != 0 && !self.handles[d].writable { return Err(Error::Denied); }
        let mut out = [0; PATH];
        let len = join(self.handles[d].path(), path, &mut out)?;
        if len == from { return Err(Error::Invalid); }
        let (path, h) = (&out[..len], self.free(client)?);
        let shared = self.shared(t, path);
        let f = match shared {
            Some(f) => f,
            None if self.held_by(client).2 >= FILES_EACH => return Err(Error::Handles),
            None => self.files.iter().position(|f| f.users == 0).ok_or(Error::Handles)?,
        };
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
        if mode & TRUNCATE != 0 && size != 0 {
            let empty = empty(store)?;
            tree.replace(store, at, &empty, 0)?;
            (base, size) = (empty, 0);
        }
        self.commit(store, t)?;
        // What its other handles wrote goes too.
        if let (true, Some(f)) = (mode & TRUNCATE != 0, shared) {
            self.drop_chunks(f, 0);
            let file = &mut self.files[f];
            (file.base, file.stored, file.size, file.changed) = (base.to_bytes(), 0, 0, false);
        }
        if shared.is_none() { self.files[f] = File { users: 0, tree: t, base: base.to_bytes(), stored: size, size, changed: false, stale: false, read: 0, current: 0 }; }
        self.add(h, client, t, false, mode & WRITE != 0, f, path);
        Ok(h as u32)
    }

    /// Up to `out.len()` bytes of a file from `offset`, as written through its handles; fewer at its end.
    pub fn read<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, file: u32, offset: u64, out: &mut [u8]) -> Result<usize, Error> {
        let h = self.now(store, client, file)?;
        if self.handles[h].directory { return Err(Error::IsDirectory); }
        let f = self.handles[h].file;
        self.live(f)?;
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

    /// Writes `data` into a file at `offset`, through a handle that may write; the file grows, a gap reading as zeros,
    /// to FILE_MAX at most. The chunks it changes are held until the file is stored; a write that needs more than are
    /// free, or than the client's share, is Full.
    pub fn write<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, file: u32, offset: u64, data: &[u8]) -> Result<usize, Error> {
        let f = self.writer(store, client, file)?;
        let end = offset.checked_add(data.len() as u64).ok_or(Error::TooLarge)?;
        if data.is_empty() { return Ok(0); }
        if end.max(self.files[f].size) > FILE_MAX { return Err(Error::TooLarge); }
        let (first, last) = (offset / CHUNK as u64, (end - 1) / CHUNK as u64);
        // Every chunk it needs is taken first, or none.
        let needed = (first..=last).filter(|&i| self.chunk(f, i).is_none()).count();
        let free = self.slots.iter().filter(|c| !c.used).count().min(CHUNKS_EACH.saturating_sub(self.held_by(client).3));
        if needed > free { return Err(Error::Full); }
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

    /// Sets the size of a file, FILE_MAX at most, through a handle that may write: what lies past it goes, and a longer
    /// file reads zeros.
    pub fn truncate<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, file: u32, size: u64) -> Result<(), Error> {
        let f = self.writer(store, client, file)?;
        if size == self.files[f].size { return Ok(()); }
        if size > FILE_MAX { return Err(Error::TooLarge); }
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
    pub fn stat<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, handle: u32) -> Result<Entry, Error> {
        let h = self.now(store, client, handle)?;
        if !self.handles[h].directory { self.live(self.handles[h].file)?; }
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
    pub fn list<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, dir: u32, start: u32, mut visit: impl FnMut(&[u8], Kind, u64)) -> Result<usize, Error> {
        let d = self.directory(store, client, dir)?;
        let (trees, handles, files) = (&mut self.trees, &self.handles, &self.files);
        let (t, path) = (handles[d].tree, handles[d].path());
        let (mut index, mut given) = (0, 0);
        trees[t].list(store, text(path)?, |name, found| {
            if index >= start as usize && given < PAGE {
                let open = handles.iter().find(|h| h.used && h.tree == t && !h.directory && !files[h.file].stale && child(h.path(), path, name));
                visit(name, found.kind, open.map_or(found.size, |h| files[h.file].size));
                given += 1;
            }
            index += 1;
        })?;
        Ok(given)
    }

    /// Removes a file or an empty directory below directory `dir`, which must be able to write: one publication. Its
    /// handles are closed.
    pub fn remove<S: Blocks + Names + Roots>(&mut self, store: &mut S, client: Client, dir: u32, path: &str) -> Result<(), Error> {
        let d = self.directory(store, client, dir)?;
        let (t, from) = (self.handles[d].tree, self.handles[d].len);
        let mut out = [0; PATH];
        let len = join(self.handles[d].path(), path, &mut out)?;
        if len == from { return Err(Error::Invalid); }
        if !self.handles[d].writable { return Err(Error::Denied); }
        self.trees[t].remove(store, text(&out[..len])?)?;
        self.commit(store, t)?;
        for h in 0..HANDLES {
            if self.handles[h].used && self.handles[h].tree == t && self.handles[h].path() == &out[..len] { self.release(h); }
        }
        Ok(())
    }

    /// Moves `from` below directory `dir` to `to` below directory `target`, both of one tree and able to write: one
    /// publication. The handles of what moves follow it.
    pub fn rename<S: Blocks + Names + Roots>(&mut self, store: &mut S, client: Client, dir: u32, from: &str, target: u32, to: &str) -> Result<(), Error> {
        let d = self.directory(store, client, dir)?;
        let e = self.directory(store, client, target)?;
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
        self.commit(store, t)?;
        for h in self.handles.iter_mut().filter(|h| h.used && h.tree == t && under(h.path(), from)) {
            h.path.copy_within(la..h.len, lb);
            h.path[..lb].copy_from_slice(to);
            h.len = h.len - la + lb;
        }
        Ok(())
    }

    /// The volume a handle is on.
    pub fn volume<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, handle: u32) -> Result<Volume, Error> {
        let h = self.now(store, client, handle)?;
        let (view, (root, version)) = (&self.views[self.handles[h].tree], self.trees[self.handles[h].tree].base());
        Ok(Volume { writable: self.handles[h].writable, version, root, len: view.len, name: view.volume })
    }

    /// Through a handle that may write, stores what was written (a file's handle: its file; a directory's: every file
    /// written below it) and publishes it, by the name's compare-and-swap, at once. Another writer's version is Conflict,
    /// nothing published: the tree drops what it did not publish and follows the name. A file the working tree cannot
    /// take for its bounds is put after the files before it are published. A handle that may not write has nothing to
    /// flush.
    pub fn flush<S: Blocks + Names + Roots>(&mut self, store: &mut S, client: Client, handle: u32) -> Result<(), Error> {
        let h = self.handle(client, handle)?;
        self.publish(store, h)
    }

    /// Closes a handle: a file's that may write is flushed first, and the handle is closed whatever that answers. The
    /// last handle of a file lets its chunks go, and the last of a tree frees it.
    pub fn close<S: Blocks + Names + Roots>(&mut self, store: &mut S, client: Client, handle: u32) -> Result<(), Error> {
        let h = self.handle(client, handle)?;
        let published = if self.handles[h].directory { Ok(()) } else { self.publish(store, h) };
        self.release(h);
        published
    }

    /// Closes every handle of the clients that have ended, by `ended` of their PIDs, without publishing; returns how many.
    pub fn forget(&mut self, ended: impl Fn(u64) -> bool) -> usize {
        let mut gone = 0;
        for h in 0..HANDLES {
            if self.handles[h].used && ended(self.handles[h].pid) { self.release(h); gone += 1; }
        }
        gone
    }

    // The root of a scope's client: the scope's directory, on its volume alone. The first task to ask is its only user,
    // whatever the root answers, as vfs_server's.
    fn scoped_root<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, volume: &str) -> Result<u32, Error> {
        let s = self.scope_of(client.badge).ok_or(Error::Denied)?;
        let scope = &mut self.scopes[s];
        if !scope.taken { (scope.taken, scope.user) = (true, client.pid); }
        if scope.user != client.pid || !scope.volume().eq_ignore_ascii_case(volume) { return Err(Error::Denied); }
        let view = View { users: 0, pid: client.pid, badge: client.badge, len: scope.len, volume: scope.volume };
        let (path, depth, may) = (scope.path, scope.depth, scope.writable);
        self.open_root(store, client, view, &path[..depth], may)
    }

    // A tree of `client`'s for `view` at its name's current version, and a handle of the directory at `path` in it.
    fn open_root<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, view: View, path: &[u8], may: bool) -> Result<u32, Error> {
        let h = self.free(client)?;
        if self.held_by(client).1 >= TREES_EACH { return Err(Error::Handles); }
        let t = self.views.iter().position(|v| v.users == 0).ok_or(Error::Handles)?;
        let (version, root) = store.current(view.name(&mut [0; NAME]))?;
        self.trees[t].start(root, version);
        // A directory: the root's block, and each on the way to it, read and checked against its CID.
        if self.trees[t].lookup(store, text(path)?)?.kind != Kind::Directory { return Err(Error::NotDirectory); }
        self.views[t] = view;
        self.add(h, client, t, true, may, 0, path);
        Ok(h as u32)
    }

    // The scope whose badge is `badge`.
    fn scope_of(&self, badge: u16) -> Option<usize> {
        if badge < SCOPE_BADGE_FIRST { return None; }
        self.scopes.iter().position(|s| s.used && s.badge == badge)
    }

    // A free handle for `client`, within its share.
    fn free(&self, client: Client) -> Result<usize, Error> {
        if self.held_by(client).0 >= HANDLES_EACH { return Err(Error::Handles); }
        self.handles.iter().position(|h| !h.used).ok_or(Error::Handles)
    }

    // Handle `id` if it is `client`'s.
    fn handle(&self, client: Client, id: u32) -> Result<usize, Error> {
        match self.handles.get(id as usize) { Some(h) if h.used && h.pid == client.pid && h.badge == client.badge => Ok(id as usize), _ => Err(Error::Invalid) }
    }

    // Handle `id` of `client` once its tree followed the name: one the move closed is Invalid.
    fn now<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, id: u32) -> Result<usize, Error> {
        let h = self.handle(client, id)?;
        self.refresh(store, self.handles[h].tree)?;
        self.handle(client, id)
    }

    fn directory<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, id: u32) -> Result<usize, Error> {
        let h = self.now(store, client, id)?;
        if self.handles[h].directory { Ok(h) } else { Err(Error::NotDirectory) }
    }

    // The file of a handle that may write.
    fn writer<S: Blocks + Roots>(&mut self, store: &mut S, client: Client, id: u32) -> Result<usize, Error> {
        let h = self.now(store, client, id)?;
        match self.handles[h] { Handle { writable: false, .. } => return Err(Error::Denied), Handle { directory: true, .. } => return Err(Error::IsDirectory), _ => {} }
        self.live(self.handles[h].file)?;
        Ok(self.handles[h].file)
    }

    // A file a conflict dropped answers that conflict.
    fn live(&self, f: usize) -> Result<(), Error> {
        let file = &self.files[f];
        if file.stale { Err(Error::Conflict { read: file.read, current: file.current }) } else { Ok(()) }
    }

    // The file open at `path` in tree `t`, but one a conflict dropped.
    fn shared(&self, t: usize, path: &[u8]) -> Option<usize> {
        self.handles.iter().find(|h| h.used && h.tree == t && !h.directory && !self.files[h.file].stale && h.path() == path).map(|h| h.file)
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

    // Drops what tree `t` holds unpublished.
    fn drop_edits(&mut self, t: usize) {
        let (root, version) = self.trees[t].base();
        self.trees[t].start(root, version);
    }

    // Before a call on tree `t`: if it holds no unpublished data and its name has moved, it follows the name.
    fn refresh<S: Blocks + Roots>(&mut self, store: &mut S, t: usize) -> Result<(), Error> {
        if self.files.iter().any(|f| f.users > 0 && f.tree == t && f.changed) { return Ok(()); }
        let (version, root) = store.current(self.views[t].name(&mut [0; NAME]))?;
        if version != self.trees[t].base().1 { self.follow(store, t, version, root); }
        Ok(())
    }

    // Moves tree `t` to `version` of its name, whose root is `root`. A file whose data was not published is dropped and
    // answers Conflict until closed; any other open file is read again at its path; a handle whose entry is gone, or no
    // longer of its kind, is closed, as a removal closes it.
    fn follow<S: Blocks>(&mut self, store: &mut S, t: usize, version: u64, root: Option<Cid>) {
        let read = self.trees[t].base().1;
        self.trees[t].start(root, version);
        for f in 0..FILES {
            if self.files[f].users == 0 || self.files[f].tree != t || self.files[f].stale { continue; }
            if self.files[f].changed {
                self.drop_chunks(f, 0);
                let file = &mut self.files[f];
                (file.changed, file.stale, file.read, file.current) = (false, true, read, version);
                continue;
            }
            let Some(h) = self.handles.iter().position(|h| h.used && !h.directory && h.file == f) else { continue };
            let found = match text(self.handles[h].path()) { Ok(at) => self.trees[t].lookup(store, at).ok(), Err(_) => None };
            match found {
                Some(worktree::Found { kind: Kind::File, size, cid: Some(cid) }) => {
                    let file = &mut self.files[f];
                    (file.base, file.stored, file.size) = (cid.to_bytes(), size, size);
                }
                _ => for h in 0..HANDLES { if self.handles[h].used && !self.handles[h].directory && self.handles[h].file == f { self.release(h); } },
            }
        }
        for h in 0..HANDLES {
            if !self.handles[h].used || self.handles[h].tree != t || !self.handles[h].directory { continue; }
            let found = match text(self.handles[h].path()) { Ok(at) => self.trees[t].lookup(store, at).ok(), Err(_) => None };
            if !found.is_some_and(|found| found.kind == Kind::Directory) { self.release(h); }
        }
    }

    // Publishes what tree `t` holds by its name's compare-and-swap. Refused, the tree drops it; after a conflict it
    // follows the name to its current version.
    fn commit<S: Blocks + Names + Roots>(&mut self, store: &mut S, t: usize) -> Result<(), Error> {
        if self.trees[t].held().0 == 0 { return Ok(()); }
        let mut name = [0; NAME];
        let name = self.views[t].name(&mut name);
        let Err(e) = self.trees[t].publish(store, name) else { return Ok(()) };
        self.drop_edits(t);
        if matches!(e, worktree::Error::Conflict { .. }) {
            if let Ok((version, root)) = store.current(name) { self.follow(store, t, version, root); }
        }
        Err(e.into())
    }

    // What `flush` does for handle `h`.
    fn publish<S: Blocks + Names + Roots>(&mut self, store: &mut S, h: usize) -> Result<(), Error> {
        let Handle { tree: t, directory, writable, file, .. } = self.handles[h];
        if !writable { return Ok(()); }
        if !directory { self.live(file)?; }
        // Each file stored and put into the tree, until it is published: the file, its object and size.
        let (mut put, mut n) = ([(0, [0; cid::BYTES], 0); FILES], 0);
        for f in 0..FILES {
            let below = self.handles.iter().any(|o| o.used && !o.directory && o.file == f && under(o.path(), self.handles[h].path()));
            let wanted = if directory { self.files[f].users > 0 && self.files[f].tree == t && self.files[f].changed && below } else { f == file && self.files[f].changed };
            if !wanted { continue; }
            if let Err(e) = self.put_file(store, t, f, &mut put, &mut n) { self.drop_edits(t); return Err(e); }
        }
        self.commit(store, t)?;
        for &(f, cid, size) in &put[..n] { self.published_as(f, cid, size); }
        Ok(())
    }

    // Stores file `f` and puts it into tree `t`, noted in `put` until published. Past what the tree holds, the files
    // noted before it are published first, which lets them go.
    fn put_file<S: Blocks + Names + Roots>(&mut self, store: &mut S, t: usize, f: usize, put: &mut [(usize, [u8; cid::BYTES], u64); FILES], n: &mut usize) -> Result<(), Error> {
        let (cid, size) = self.store_file(store, f)?;
        let at = self.handles.iter().position(|o| o.used && !o.directory && o.file == f).ok_or(Error::Invalid)?;
        let (mut path, len) = ([0; PATH], self.handles[at].len);
        path[..len].copy_from_slice(self.handles[at].path());
        let path = text(&path[..len])?;
        match self.trees[t].replace(store, path, &cid, size) {
            Err(worktree::Error::Full) => {
                self.commit(store, t)?;
                for &(f, cid, size) in &put[..*n] { self.published_as(f, cid, size); }
                *n = 0;
                self.trees[t].replace(store, path, &cid, size)?;
            }
            done => done?,
        }
        put[*n] = (f, cid.to_bytes(), size);
        *n += 1;
        Ok(())
    }

    // File `f` is published as object `cid` of `size` bytes: its chunks go.
    fn published_as(&mut self, f: usize, cid: [u8; cid::BYTES], size: u64) {
        self.drop_chunks(f, 0);
        let file = &mut self.files[f];
        (file.base, file.stored, file.changed) = (cid, size, false);
    }

    // Stores file `f` as an object of `dag`: its root and size.
    fn store_file<S: Blocks>(&mut self, store: &mut S, f: usize) -> Result<(Cid, u64), Error> {
        let File { base, stored, size, .. } = self.files[f];
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
        Ok(self.builder.finish(store)?)
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
