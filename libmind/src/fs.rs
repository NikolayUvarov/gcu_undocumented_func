//! vfs_server client (idl/vfs.wit): files and directories through handles. A path may name a volume first (`ram:notes`);
//! without one it is on the boot disk. Data travels in one buffer the process lends with every call. What a program may
//! write depends on the badge of its VFS capability: applications read only; the shell writes on `ram:` and in `data/`.
use crate::abi::*;
use crate::idl::vfs;
use crate::idl::wire;
use crate::ipc::Endpoint;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Bytes moved per call.
pub const CHUNK: usize = 16 * 1024;
const BUFFER: usize = CHUNK + 4096;

/// Why a file operation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { NotFound, Exists, NotEmpty, Invalid, Denied, NoSpace, ReadOnly, Io, Handles, Name, NotDirectory, IsDirectory, NoMemory, NoService }
pub type Result<T> = core::result::Result<T, Error>;

impl From<vfs::Error> for Error {
    fn from(e: vfs::Error) -> Self {
        match e {
            vfs::Error::NotFound => Error::NotFound, vfs::Error::Exists => Error::Exists, vfs::Error::NotEmpty => Error::NotEmpty, vfs::Error::Invalid => Error::Invalid,
            vfs::Error::Denied => Error::Denied, vfs::Error::NoSpace => Error::NoSpace, vfs::Error::ReadOnly => Error::ReadOnly, vfs::Error::Io => Error::Io,
            vfs::Error::Handles => Error::Handles, vfs::Error::Name => Error::Name, vfs::Error::NotDirectory => Error::NotDirectory, vfs::Error::IsDirectory => Error::IsDirectory,
        }
    }
}

impl From<crate::sys::Error> for Error {
    fn from(e: crate::sys::Error) -> Self { if e == crate::sys::Error::NoMemory { Error::NoMemory } else { Error::NoService } }
}

impl From<Error> for crate::sys::Error {
    fn from(e: Error) -> Self {
        use crate::sys::Error as S;
        match e { Error::NotFound => S::NotFound, Error::Denied | Error::ReadOnly => S::Rights, Error::NoSpace | Error::NoMemory => S::NoMemory, Error::Handles => S::NoSlot, Error::NoService => S::Peer, _ => S::Invalid }
    }
}

// A generated call: the IPC failure and the service's error both become `Error`.
fn call<T>(result: crate::sys::Result<core::result::Result<T, vfs::Error>>) -> Result<T> { result?.map_err(Error::from) }

struct Client { shared: wire::Shared, roots: [(u32, bool); 2] } // root handles of "" and "ram", opened once
struct State(UnsafeCell<Option<Client>>);
unsafe impl Sync for State {} // processes are single-threaded
static STATE: State = State(UnsafeCell::new(None));

// The VFS capability calls go to: SLOT_VFS, or the one a launcher lent (`use_endpoint`).
static ENDPOINT: AtomicUsize = AtomicUsize::new(SLOT_VFS);
fn endpoint() -> Endpoint { Endpoint(ENDPOINT.load(Ordering::Relaxed)) }

/// Sends all later calls to `endpoint` (e.g. a VFS capability a launcher lent in `SLOT_FILE`, which may write where the
/// program's own may not). Handles are bound to the capability they were opened with, so the roots are opened again.
pub fn use_endpoint(endpoint: Endpoint) {
    ENDPOINT.store(endpoint.0, Ordering::Relaxed);
    if let Some(c) = unsafe { &mut *STATE.0.get() } { c.roots = [(0, false); 2]; }
}

fn client() -> Result<&'static mut Client> {
    let slot = unsafe { &mut *STATE.0.get() };
    if slot.is_none() { *slot = Some(Client { shared: wire::Shared::new(BUFFER).map_err(|_| Error::NoMemory)?, roots: [(0, false); 2] }); }
    Ok(slot.as_mut().unwrap())
}

/// Creates the buffer up front (services do this at startup so they don't grow while running).
pub fn prepare() -> Result<()> { client().map(drop) }

/// The volume and the path on it: `ram:docs/a` -> ("ram", "docs/a"); a path without a volume is on the boot disk.
pub fn split(path: &str) -> (&str, &str) {
    match path.find(':') { Some(i) if !path[..i].contains('/') => (&path[..i], path[i + 1..].trim_start_matches('/')), _ => ("", path.trim_start_matches('/')) }
}

fn root(volume: &str) -> Result<u32> {
    let index = if volume.is_empty() { 0 } else if volume.eq_ignore_ascii_case("ram") { 1 } else { return Err(Error::NotFound) };
    let c = client()?;
    if !c.roots[index].1 { let handle = call(vfs::root(endpoint(), c.shared.buffer(), volume))?; c.roots[index] = (handle, true); }
    Ok(c.roots[index].0)
}

/// What `metadata` and `File::metadata` report: size, FAT modification stamp (see `fat_time`), `VFS_ENTRY_*` bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metadata { pub size: u32, pub modified: u32, pub attributes: u8, pub is_dir: bool }

/// A mounted volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeInfo { pub label: [u8; 11], pub fat_bits: u8, pub bytes: u64, pub free: u64, pub cluster: u32, pub writable: bool }
impl VolumeInfo { pub fn label(&self) -> &str { core::str::from_utf8(&self.label).unwrap_or("").trim_end() } }

fn stat(handle: u32) -> Result<Metadata> {
    let c = client()?;
    let e = call(vfs::stat(endpoint(), c.shared.buffer(), handle))?;
    Ok(Metadata { size: e.size, modified: e.modified, attributes: e.attributes, is_dir: e.directory })
}

fn close(handle: u32) { let _ = vfs::close(endpoint(), handle); }

/// A directory handle.
pub struct Dir { handle: u32, owned: bool }

impl Dir {
    /// The root of a volume (`""`: the boot disk, `"ram"`).
    pub fn root(volume: &str) -> Result<Self> { Ok(Self { handle: root(volume)?, owned: false }) }
    /// A directory by path (`ram:docs`, `data/notes`).
    pub fn open(path: &str) -> Result<Self> { let (volume, rest) = split(path); Self::root(volume)?.dir(rest, false) }
    /// The same, made with its parents if missing.
    pub fn create(path: &str) -> Result<Self> { let (volume, rest) = split(path); Self::root(volume)?.dir(rest, true) }
    /// A directory below this one.
    pub fn dir(&self, path: &str, create: bool) -> Result<Self> {
        if path.is_empty() { return Ok(Self { handle: self.handle, owned: false }); }
        let c = client()?;
        Ok(Self { handle: call(vfs::open_dir(endpoint(), c.shared.buffer(), self.handle, create, path))?, owned: true })
    }
    /// A file below this one with `VFS_MODE_*` bits.
    pub fn file(&self, path: &str, mode: u8) -> Result<File> {
        let c = client()?;
        let handle = call(vfs::open(endpoint(), c.shared.buffer(), self.handle, mode, path))?;
        let size = stat(handle).map(|m| m.size as usize).unwrap_or(0);
        Ok(File { handle, size, position: 0 })
    }
    /// Calls `visit` for every entry; returns their number.
    pub fn list(&self, mut visit: impl FnMut(&DirEntry)) -> Result<usize> {
        let mut start = 0u32;
        loop {
            let c = client()?;
            let entries = call(vfs::list(endpoint(), c.shared.buffer(), self.handle, start))?;
            if entries.is_empty() { return Ok(start as usize); }
            for e in entries.iter() { visit(&DirEntry { name: e.name.as_bytes(), size: e.size, is_dir: e.directory, flags: e.attributes, modified: e.modified }); }
            start += entries.len() as u32;
        }
    }
    /// Removes a file or an empty directory below this one.
    pub fn remove(&self, path: &str) -> Result<()> { let c = client()?; call(vfs::remove(endpoint(), c.shared.buffer(), self.handle, path)) }
    /// Renames or moves `from` (below this directory) to `to` below `target` (the same volume).
    pub fn rename(&self, from: &str, target: &Dir, to: &str) -> Result<()> { let c = client()?; call(vfs::rename(endpoint(), c.shared.buffer(), self.handle, target.handle, from, to)) }
    pub fn metadata(&self) -> Result<Metadata> { stat(self.handle) }
    /// The volume this directory is on.
    pub fn volume(&self) -> Result<VolumeInfo> {
        let c = client()?;
        let v = call(vfs::volume(endpoint(), c.shared.buffer(), self.handle))?;
        let mut label = [b' '; 11];
        for (i, b) in v.label.bytes().take(11).enumerate() { label[i] = b; }
        Ok(VolumeInfo { label, fat_bits: v.fat_bits, bytes: v.bytes, free: v.free, cluster: v.cluster, writable: v.writable })
    }
    /// Writes what is cached for this volume to the disk.
    pub fn flush(&self) -> Result<()> { call(vfs::flush(endpoint(), self.handle)) }
    /// Checks this directory's volume without changing it; `visit` sees the report (idl/vfs.wit `report`).
    pub fn check<T>(&self, visit: impl FnOnce(&vfs::Report) -> T) -> Result<T> {
        let c = client()?;
        Ok(visit(&call(vfs::check(endpoint(), c.shared.buffer(), self.handle))?))
    }
}

impl Drop for Dir { fn drop(&mut self) { if self.owned { close(self.handle); } } }

/// An open file (the handle belongs to the process and is closed in Drop).
pub struct File { handle: u32, size: usize, position: usize }

impl File {
    /// Opens a file to read.
    pub fn open(path: &str) -> Result<Self> { Self::open_mode(path, 0) }
    /// Creates a file (or empties an existing one) to write.
    pub fn create(path: &str) -> Result<Self> { Self::open_mode(path, VFS_MODE_WRITE | VFS_MODE_CREATE | VFS_MODE_TRUNCATE) }
    /// Opens with `VFS_MODE_*` bits.
    pub fn open_mode(path: &str, mode: u8) -> Result<Self> { let (volume, rest) = split(path); Dir::root(volume)?.file(rest, mode) }
    pub fn size(&self) -> usize { self.size }
    pub fn position(&self) -> usize { self.position }
    pub fn seek(&mut self, position: usize) { self.position = position.min(self.size); }
    /// Reads at `offset` without changing the current position.
    pub fn read_at(&self, offset: usize, buffer: &mut [u8]) -> Result<usize> {
        let mut done = 0;
        while done < buffer.len() {
            let want = (buffer.len() - done).min(CHUNK);
            let c = client()?;
            let data = call(vfs::read(endpoint(), c.shared.buffer(), self.handle, (offset + done) as u32, want as u32))?;
            buffer[done..done + data.len()].copy_from_slice(data);
            done += data.len();
            if data.len() < want { break; }
        }
        Ok(done)
    }
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize> { let got = self.read_at(self.position, buffer)?; self.position += got; Ok(got) }
    /// Writes at `offset` without changing the current position; the file grows.
    pub fn write_at(&mut self, offset: usize, data: &[u8]) -> Result<usize> {
        let mut done = 0;
        while done < data.len() {
            let n = (data.len() - done).min(CHUNK);
            let c = client()?;
            done += call(vfs::write(endpoint(), c.shared.buffer(), self.handle, (offset + done) as u32, &data[done..done + n]))? as usize;
        }
        self.size = self.size.max(offset + done);
        Ok(done)
    }
    pub fn write(&mut self, data: &[u8]) -> Result<usize> { let n = self.write_at(self.position, data)?; self.position += n; Ok(n) }
    /// Sets the size (shorter frees space, longer reads zeros).
    pub fn truncate(&mut self, size: usize) -> Result<()> { call(vfs::truncate(endpoint(), self.handle, size as u32))?; self.size = size; self.position = self.position.min(size); Ok(()) }
    pub fn metadata(&self) -> Result<Metadata> { stat(self.handle) }
    /// Writes what is cached for this file's volume to the disk.
    pub fn flush(&self) -> Result<()> { call(vfs::flush(endpoint(), self.handle)) }
}

impl Drop for File { fn drop(&mut self) { close(self.handle); } }

/// Directory entry: `flags` are `VFS_ENTRY_*`, `modified` is FAT date << 16 | FAT time (see `fat_time`).
pub struct DirEntry<'a> { pub name: &'a [u8], pub size: u32, pub is_dir: bool, pub flags: u8, pub modified: u32 }

impl DirEntry<'_> { pub fn name_str(&self) -> &str { core::str::from_utf8(self.name).unwrap_or("?") } }

/// A FAT date and time (`DirEntry::modified`) as (year, month, day, hour, minute, second); local time, 2-second steps.
pub const fn fat_time(modified: u32) -> (u32, u32, u32, u32, u32, u32) {
    let (date, time) = (modified >> 16, modified & 0xFFFF);
    (1980 + (date >> 9), (date >> 5) & 0xF, date & 0x1F, time >> 11, (time >> 5) & 0x3F, (time & 0x1F) * 2)
}

/// Iterates a directory (`""` or `"/"` is the root of the boot disk, `"ram:"` of the RAM disk); returns the number of entries.
pub fn list(path: &str, visit: impl FnMut(&DirEntry)) -> Result<usize> { Dir::open(path)?.list(visit) }

/// Makes a directory (and missing parents).
pub fn mkdir(path: &str) -> Result<()> { Dir::create(path).map(drop) }

/// Removes a file or an empty directory.
pub fn remove(path: &str) -> Result<()> { let (volume, rest) = split(path); Dir::root(volume)?.remove(rest) }

/// Renames or moves within a volume.
pub fn rename(from: &str, to: &str) -> Result<()> {
    let ((volume, from), (target, to)) = (split(from), split(to));
    if !volume.eq_ignore_ascii_case(target) { return Err(Error::Invalid); }
    let root = Dir::root(volume)?;
    root.rename(from, &root, to)
}

/// Size, time and attributes of a file or directory.
pub fn metadata(path: &str) -> Result<Metadata> {
    if split(path).1.is_empty() { return Dir::open(path)?.metadata(); }
    match File::open(path) { Ok(file) => file.metadata(), Err(Error::IsDirectory) => Dir::open(path)?.metadata(), Err(e) => Err(e) }
}

/// The volume `name` (`""` or `"ram"`).
pub fn volume(name: &str) -> Result<VolumeInfo> { Dir::root(name)?.volume() }

/// Checks volume `name` (`""` or `"ram"`) without changing it.
pub fn check<T>(name: &str, visit: impl FnOnce(&vfs::Report) -> T) -> Result<T> { Dir::root(name)?.check(visit) }
