//! The block store as a volume of fm, `store:` (300-APP-0019). Published names are files, and a `/` in a name makes
//! directories; the objects the caller's owner pinned are in `.pins`, by CID. An object's bytes are read through
//! mind::dag, every block checked against its CID. A file copied to `store:` is stored as an object and published
//! under its path; a deleted name is unpublished; a rename moves both names at once. The store is a `Store`: the
//! service in the system, memory in tests/fm_host.rs.
use crate::cid::Cid;
use crate::dag::{self, Blocks, Builder, CHUNK};
use crate::fm::{Disk, Failure, Sink, Started, VolumeInfo};
use crate::panel::{self, Entry, VFS_ENTRY_READ_ONLY};
use crate::tui::viewer::Source;
use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

/// The volume's prefix in fm's paths.
pub const VOLUME: &str = "store:";
/// The directory of pinned objects.
pub const PINS: &str = ".pins";

/// What fm asks of the block store besides its blocks.
pub trait Store: Blocks + Clone {
    /// A name's current version and the root it points at; None if it is not published.
    fn resolve(&mut self, name: &str) -> Result<Option<(u64, Cid)>, String>;
    /// Points `name` at `root` if its current version is `expected` (0: a new name).
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<(), Failure>;
    /// Removes `name` if its current version is `expected`.
    fn unpublish(&mut self, name: &str, expected: u64) -> Result<(), Failure>;
    /// Several names at once, all or none: each with the version expected and its new root (None removes it).
    fn commit(&mut self, updates: &[(&str, u64, Option<Cid>)]) -> Result<(), Failure>;
    /// The objects the caller's owner pinned, with their sizes.
    fn pins(&mut self) -> Result<Vec<(Cid, u64)>, String>;
    /// Sectors of the medium used and in all, and the names published.
    fn stats(&mut self) -> Result<(u64, u64, u32), String>;
    /// Every published name, where the store can list them (blockstore.wit 1.3 cannot: fm shows the names it saw).
    fn names(&mut self) -> Option<Vec<String>>;
}

/// A name the store takes: 1 to 64 bytes of `A-Z a-z 0-9 . _ / -`.
pub fn valid(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._/-".contains(&b))
}

const NAMES: &str = "a name in the store is 1 to 64 of A-Z a-z 0-9 . _ / -";

fn failure(error: dag::Error) -> Failure {
    match error { dag::Error::Full => Failure::NoSpace, dag::Error::NotFound => Failure::NotFound, other => Failure::Other(format!("{:?}", other)) }
}

// A buffer of one block, on the heap.
fn chunk() -> Box<[u8; CHUNK]> { alloc::vec![0u8; CHUNK].into_boxed_slice().try_into().unwrap() }

// A builder (about 110 KiB) made on the heap: all zeros is `Builder::new()`.
fn builder() -> Box<Builder> {
    let layout = core::alloc::Layout::new::<Builder>();
    unsafe {
        let memory = alloc::alloc::alloc_zeroed(layout) as *mut Builder;
        if memory.is_null() { alloc::alloc::handle_alloc_error(layout); }
        Box::from_raw(memory)
    }
}

/// The volume over a store, with the names fm saw published, opened or listed.
pub struct Volume<S: Store> { store: S, seen: Rc<RefCell<Vec<String>>> }

impl<S: Store + 'static> Volume<S> {
    pub fn new(store: S) -> Self { Self { store, seen: Rc::new(RefCell::new(Vec::new())) } }

    fn remember(seen: &RefCell<Vec<String>>, name: &str) {
        let mut seen = seen.borrow_mut();
        if !seen.iter().any(|n| n == name) { seen.push(String::from(name)); }
    }

    fn names(&mut self) -> Vec<String> {
        let mut names = match self.store.names() { Some(listed) => listed, None => self.seen.borrow().clone() };
        names.sort();
        names
    }

    fn size(&mut self, root: &Cid) -> u64 { dag::size(&mut self.store, root, &mut chunk()).unwrap_or(0) }

    /// Entries under `dir` (a path in the store, "" for its root).
    pub fn list(&mut self, dir: &str) -> Result<Vec<Entry>, String> {
        let dir = dir.trim_matches('/');
        if dir.eq_ignore_ascii_case(PINS) {
            return Ok(self.store.pins()?.iter().map(|(cid, size)| Entry { name: format!("{}", cid), size: *size, dir: false, flags: VFS_ENTRY_READ_ONLY, modified: 0 }).collect());
        }
        let prefix = if dir.is_empty() { String::new() } else { format!("{}/", dir) };
        let mut entries: Vec<Entry> = Vec::new();
        for name in self.names() {
            let Some(rest) = name.strip_prefix(&prefix) else { continue };
            match rest.split_once('/') {
                Some((sub, _)) => if !entries.iter().any(|e| e.dir && e.name == sub) { entries.push(Entry::directory(sub)); },
                None => {
                    let Ok(Some((_, root))) = self.store.resolve(&name) else { continue }; // removed meanwhile
                    let size = self.size(&root);
                    entries.push(Entry { name: String::from(rest), size, dir: false, flags: 0, modified: 0 });
                }
            }
        }
        if dir.is_empty() && self.store.pins().is_ok_and(|p| !p.is_empty()) { entries.push(Entry::directory(PINS)); }
        if entries.is_empty() && !dir.is_empty() { return Err(String::from("NotFound")); }
        Ok(entries)
    }

    // The root a path names: a published name, or a pinned object by its CID.
    fn root(&mut self, path: &str) -> Option<Cid> {
        let path = path.trim_matches('/');
        if let Some(cid) = path.strip_prefix(PINS).and_then(|p| p.strip_prefix('/')) { return Cid::from_text(cid.as_bytes()).ok(); }
        let (_, root) = self.store.resolve(path).ok()??;
        Self::remember(&self.seen, path);
        Some(root)
    }

    /// An object to read.
    pub fn open(&mut self, path: &str) -> Option<Box<dyn Source>> {
        let root = self.root(path)?;
        let mut buffer = chunk();
        let size = dag::size(&mut self.store, &root, &mut buffer).ok()?;
        Some(Box::new(Object { store: self.store.clone(), root, size, buffer }))
    }

    /// A new object, published under `path` when its writing ends: a name in use only if `replace`.
    pub fn create(&mut self, path: &str, replace: bool) -> Result<Box<dyn Sink>, Failure> {
        let name = path.trim_matches('/');
        if !valid(name) || name.starts_with(PINS) { return Err(Failure::Other(String::from(NAMES))); }
        let expected = match self.store.resolve(name).map_err(Failure::Other)? {
            Some(_) if !replace => return Err(Failure::Exists),
            Some((version, _)) => version,
            None => 0,
        };
        Ok(Box::new(Writing { store: self.store.clone(), builder: builder(), name: String::from(name), expected, seen: self.seen.clone() }))
    }

    /// Directories are the `/` in names: one exists once a name in it is published.
    pub fn mkdir(&mut self, path: &str) -> Result<(), Failure> {
        let name = path.trim_matches('/');
        if valid(name) && !name.starts_with(PINS) { Ok(()) } else { Err(Failure::Other(String::from(NAMES))) }
    }

    /// Unpublishes a name; a directory only while no name is in it.
    pub fn remove(&mut self, path: &str) -> Result<(), Failure> {
        let name = path.trim_matches('/');
        if name.starts_with(PINS) { return Err(Failure::Denied); }
        match self.store.resolve(name).map_err(Failure::Other)? {
            Some((version, _)) => {
                self.store.unpublish(name, version)?;
                self.seen.borrow_mut().retain(|n| n != name);
                Ok(())
            }
            None if self.names().iter().any(|n| n.starts_with(&format!("{}/", name))) => Err(Failure::NotEmpty),
            None => Err(Failure::NotFound),
        }
    }

    /// Moves a name: the new one published and the old one removed in one commit.
    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), Failure> {
        let (from, to) = (from.trim_matches('/'), to.trim_matches('/'));
        if !valid(to) || to.starts_with(PINS) { return Err(Failure::Other(String::from(NAMES))); }
        let (version, root) = self.store.resolve(from).map_err(Failure::Other)?.ok_or(Failure::NotFound)?;
        if self.store.resolve(to).map_err(Failure::Other)?.is_some() { return Err(Failure::Exists); }
        self.store.commit(&[(to, 0, Some(root)), (from, version, None)])?;
        let mut seen = self.seen.borrow_mut();
        seen.retain(|n| n != from);
        seen.push(String::from(to));
        Ok(())
    }

    /// The store as the volume menu and the information panel show it.
    pub fn volume(&mut self) -> Option<VolumeInfo> {
        let (used, sectors, names) = self.store.stats().ok()?;
        Some(VolumeInfo { label: format!("{} names", names), fat_bits: 0, bytes: sectors * 512, free: sectors.saturating_sub(used) * 512 })
    }
}

// An object being read, its blocks checked as they come.
struct Object<S: Store> { store: S, root: Cid, size: u64, buffer: Box<[u8; CHUNK]> }

impl<S: Store> Source for Object<S> {
    fn size(&self) -> u64 { self.size }
    fn read(&mut self, offset: u64, out: &mut [u8]) -> usize { dag::read_at(&mut self.store, &self.root, offset, out, &mut self.buffer).unwrap_or(0) }
}

// An object being written: its blocks stored as they fill, then the root published under the name.
struct Writing<S: Store> { store: S, builder: Box<Builder>, name: String, expected: u64, seen: Rc<RefCell<Vec<String>>> }

impl<S: Store + 'static> Sink for Writing<S> {
    fn write(&mut self, data: &[u8]) -> Result<(), Failure> { self.builder.write(&mut self.store, data).map_err(failure) }
    fn finish(&mut self) -> Result<(), Failure> {
        let (root, _) = self.builder.finish(&mut self.store).map_err(failure)?;
        self.store.publish(&self.name, self.expected, &root)?;
        Volume::<S>::remember(&self.seen, &self.name);
        Ok(())
    }
}

/// A disk with the store beside it: paths on `store:` go to the store (none without a client of it), the rest to the disk.
pub struct WithStore<D: Disk, S: Store> { pub disk: D, pub store: Option<Volume<S>> }

// The path in the store of a path on `store:`.
fn on_store(path: &str) -> Option<&str> { let (volume, rest) = panel::volume(path); volume.eq_ignore_ascii_case(VOLUME).then_some(rest) }

impl<D: Disk, S: Store + 'static> WithStore<D, S> {
    fn store(&mut self) -> Result<&mut Volume<S>, Failure> { self.store.as_mut().ok_or(Failure::Other(String::from("no block store client"))) }
}

impl<D: Disk, S: Store + 'static> Disk for WithStore<D, S> {
    fn list(&mut self, path: &str) -> Result<Vec<Entry>, String> {
        match on_store(path) { Some(rest) => self.store().map_err(|_| String::from("NotFound"))?.list(rest), None => self.disk.list(path) }
    }
    fn open(&mut self, path: &str) -> Option<Box<dyn Source>> {
        match on_store(path) { Some(rest) => self.store.as_mut()?.open(rest), None => self.disk.open(path) }
    }
    fn run(&mut self, path: &str, args: &str) -> Result<Started, String> { self.disk.run(path, args) }
    fn create(&mut self, path: &str, replace: bool) -> Result<Box<dyn Sink>, Failure> {
        match on_store(path) { Some(rest) => self.store()?.create(rest, replace), None => self.disk.create(path, replace) }
    }
    fn mkdir(&mut self, path: &str) -> Result<(), Failure> {
        match on_store(path) { Some(rest) => self.store()?.mkdir(rest), None => self.disk.mkdir(path) }
    }
    fn remove(&mut self, path: &str) -> Result<(), Failure> {
        match on_store(path) { Some(rest) => self.store()?.remove(rest), None => self.disk.remove(path) }
    }
    fn rename(&mut self, from: &str, to: &str) -> Result<(), Failure> {
        match (on_store(from), on_store(to)) {
            (Some(from), Some(to)) => self.store()?.rename(from, to),
            (None, None) => self.disk.rename(from, to),
            _ => Err(Failure::Other(String::from("another volume"))),
        }
    }
    // An object is never changed in place: a new one is published.
    fn writable(&mut self, path: &str) -> bool { on_store(path).is_none() && self.disk.writable(path) }
    fn volume(&mut self, path: &str) -> Option<VolumeInfo> {
        match on_store(path) { Some(_) => self.store.as_mut()?.volume(), None => self.disk.volume(path) }
    }
    fn flush(&mut self, path: &str) { if on_store(path).is_none() { self.disk.flush(path); } }
}
