//! vfs_server client (idl/vfs.wit 2.x): files and directories through handles. A path may name a volume first
//! (`ram:notes`, `log:boot0001.log`); without one it is on the boot disk. What a program may write depends on the
//! badge of its VFS capability: applications read only; the shell writes on `ram:`, on `log:` (the boot disk's log
//! partition, where one is) and in `data/`.
use crate::abi::*;
use crate::idl::vfs;
use crate::ipc::Endpoint;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Bytes moved per call (`bytes<16384>` in idl/vfs.wit).
pub const CHUNK: usize = 16 * 1024;

/// `open` mode bits.
pub const MODE_WRITE: u8 = 1;
pub const MODE_CREATE: u8 = 2;
pub const MODE_TRUNCATE: u8 = 4;
pub const MODE_NEW: u8 = 8;
/// Attributes of a directory entry (`DirEntry::flags`).
pub const ENTRY_DIR: u8 = 1;
pub const ENTRY_HIDDEN: u8 = 2;
pub const ENTRY_SYSTEM: u8 = 4;
pub const ENTRY_READ_ONLY: u8 = 8;
pub const ENTRY_ARCHIVE: u8 = 16;
/// Badge of the VFS client that may write on `ram` and in the boot disk's `data` directory (init gives it to the
/// shell, Appendix B.6); unbadged clients only read.
pub const BADGE_USER: u16 = 1;
/// Badges of the services with a private directory in the boot disk's `system/` (351-NET-0005): only the client with
/// the directory's badge may open, read or write `system/keystore` and `system/netpolicy`; nobody else may open them.
pub const BADGE_KEYSTORE: u16 = 2;
pub const BADGE_NETPOLICY: u16 = 3;
/// The Effector agent's private directories, `system/effector` and `system/effector_gw` (750-KRN-0119), on the same rule.
pub const BADGE_EFFECTOR: u16 = 6;
pub const BADGE_EFFECTOR_GW: u16 = 7;
/// Badge of the updater's client (351-UPD-0008): besides reading, it may fill the slot the system did not boot from
/// (`MIND/A` or `MIND/B`) and write the boot records `MIND/BOOT0` and `MIND/BOOT1` whole, in place; nothing else.
pub const BADGE_UPDATE: u16 = 4;
/// Badge of the read-only clients init lends services, and through `loader` its programs: set by init, so no holder can
/// set another on them (a badge is set once, MC-3.4) and reach a private directory or the update zone.
pub const BADGE_READER: u16 = 5;
/// Badge of the loader's client that alone opens `system/own/`, where it makes each program's own directory and scopes
/// it into the program's SLOT_OWN (000-KRN-0170); set by init.
pub const BADGE_OWNERS: u16 = 8;

/// Why a file operation failed. `Conflict`: another writer published the file first, so read it again (vfs.wit 2.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { NotFound, Exists, NotEmpty, Invalid, Denied, NoSpace, ReadOnly, Io, Handles, Name, NotDirectory, IsDirectory, Conflict, NoMemory, NoService }
pub type Result<T> = core::result::Result<T, Error>;

impl From<vfs::Error> for Error {
    fn from(e: vfs::Error) -> Self {
        match e {
            vfs::Error::NotFound => Error::NotFound, vfs::Error::Exists => Error::Exists, vfs::Error::NotEmpty => Error::NotEmpty, vfs::Error::Invalid => Error::Invalid,
            vfs::Error::Denied => Error::Denied, vfs::Error::NoSpace => Error::NoSpace, vfs::Error::ReadOnly => Error::ReadOnly, vfs::Error::Io => Error::Io,
            vfs::Error::Handles => Error::Handles, vfs::Error::Name => Error::Name, vfs::Error::NotDirectory => Error::NotDirectory, vfs::Error::IsDirectory => Error::IsDirectory,
            vfs::Error::Conflict => Error::Conflict,
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

mod volumes;
pub use volumes::{on_vfs, VFS_VOLUMES};

// A root handle, opened once per volume, and the slot of the server it belongs to.
#[derive(Clone, Copy)]
struct Root { name: [u8; 16], len: usize, handle: u32, server: usize }
const ROOTS: usize = 6; // vfs_server's four, and two of storefs's, which holds at most two trees for a client
struct Client { roots: [Option<Root>; ROOTS] }
struct State(UnsafeCell<Option<Client>>);
unsafe impl Sync for State {} // processes are single-threaded
static STATE: State = State(UnsafeCell::new(None));

// The VFS capability calls go to: SLOT_VFS (and SLOT_STOREFS for storefs's volumes), or the one a launcher lent
// (`use_endpoint`) for every volume.
static ENDPOINT: AtomicUsize = AtomicUsize::new(SLOT_VFS);

// The slot `volume`'s calls go to: a lent client's for every volume; else vfs_server's for its own, and storefs's for the
// others when the program holds its client. Without one a store volume is NotFound, and nothing is asked.
fn server(volume: &str) -> Result<usize> {
    let store = (crate::dev::cap_info(SLOT_STOREFS).0 == CAP_KIND_ENDPOINT).then_some(SLOT_STOREFS);
    volumes::route(volume, ENDPOINT.load(Ordering::Relaxed), SLOT_VFS, store).ok_or(Error::NotFound)
}

/// Sends all later calls to `endpoint` (e.g. a VFS capability a launcher lent in `SLOT_FILE`, which may write where the
/// program's own may not). Handles are bound to the capability they were opened with, so the roots are opened again.
pub fn use_endpoint(endpoint: Endpoint) {
    ENDPOINT.store(endpoint.0, Ordering::Relaxed);
    if let Some(c) = unsafe { &mut *STATE.0.get() } { c.roots = [None; ROOTS]; }
}

// With a scoped client (`use_scope`): the volume and the directory its root stands for.
struct Base { volume: [u8; 16], volume_len: usize, dir: [u8; 255], dir_len: usize }
impl Base {
    fn volume(&self) -> &str { core::str::from_utf8(&self.volume[..self.volume_len]).unwrap_or("") }
    fn dir(&self) -> &str { core::str::from_utf8(&self.dir[..self.dir_len]).unwrap_or("") }
}
struct BaseCell(UnsafeCell<Option<Base>>);
unsafe impl Sync for BaseCell {}
static BASE: BaseCell = BaseCell(UnsafeCell::new(None));

/// Sends later calls to a client confined to one directory (`Dir::scope`; the shell lends one for `REQUEST_FILE`).
/// `base` (`data`, `ram:`, `ram:notes`) is the directory its root stands for, so the program keeps using full paths;
/// paths outside it fail with `Denied` (vfs_server would refuse them anyway).
pub fn use_scope(endpoint: Endpoint, base: &str) {
    use_endpoint(endpoint);
    let (volume, dir) = split(base);
    let dir = dir.trim_end_matches('/');
    let mut b = Base { volume: [0; 16], volume_len: volume.len().min(16), dir: [0; 255], dir_len: dir.len().min(255) };
    b.volume[..b.volume_len].copy_from_slice(&volume.as_bytes()[..b.volume_len]);
    b.dir[..b.dir_len].copy_from_slice(&dir.as_bytes()[..b.dir_len]);
    unsafe { *BASE.0.get() = Some(b); }
}

/// A path as this process's client sees it: the volume, and the path below the root the client gets.
fn locate(path: &str) -> Result<(&str, &str)> {
    let (volume, rest) = split(path);
    let Some(base) = (unsafe { &*BASE.0.get() }) else { return Ok((volume, rest)) };
    if !volume.eq_ignore_ascii_case(base.volume()) { return Err(Error::Denied); }
    let dir = base.dir();
    if dir.is_empty() { return Ok((volume, rest)); }
    match rest.get(..dir.len()) {
        Some(head) if head.eq_ignore_ascii_case(dir) && (rest.len() == dir.len() || rest.as_bytes()[dir.len()] == b'/') => Ok((volume, rest[dir.len()..].trim_start_matches('/'))),
        _ => Err(Error::Denied),
    }
}

fn client() -> Result<&'static mut Client> {
    let slot = unsafe { &mut *STATE.0.get() };
    if slot.is_none() { *slot = Some(Client { roots: [None; ROOTS] }); }
    Ok(slot.as_mut().unwrap())
}

/// Kept for callers that prepared a buffer at startup; the generated calls allocate one per request.
pub fn prepare() -> Result<()> { Ok(()) }

/// The volume and the path on it: `ram:docs/a` -> ("ram", "docs/a"); a path without a volume is on the boot disk.
pub fn split(path: &str) -> (&str, &str) {
    match path.find(':') { Some(i) if !path[..i].contains('/') => (&path[..i], path[i + 1..].trim_start_matches('/')), _ => ("", path.trim_start_matches('/')) }
}

// The root of `volume` and its server's slot, opened once.
fn root(volume: &str) -> Result<(u32, usize)> {
    let c = client()?;
    if let Some(r) = c.roots.iter().flatten().find(|r| r.name[..r.len].eq_ignore_ascii_case(volume.as_bytes())) { return Ok((r.handle, r.server)); }
    if volume.len() > 16 { return Err(Error::NotFound); }
    let server = server(volume)?;
    let free = c.roots.iter().position(|r| r.is_none()).ok_or(Error::Handles)?;
    let handle = call(vfs::root(Endpoint(server), volume))?;
    let mut r = Root { name: [0; 16], len: volume.len(), handle, server };
    r.name[..r.len].copy_from_slice(volume.as_bytes());
    c.roots[free] = Some(r);
    Ok((handle, server))
}

/// What `metadata` and `File::metadata` report: size, FAT modification stamp (see `fat_time`), `ENTRY_*` bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metadata { pub size: u32, pub modified: u32, pub attributes: u8, pub is_dir: bool }

/// A mounted volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeInfo { pub label: [u8; 11], pub fat_bits: u8, pub bytes: u64, pub free: u64, pub cluster: u32, pub writable: bool }
impl VolumeInfo { pub fn label(&self) -> &str { core::str::from_utf8(&self.label).unwrap_or("").trim_end() } }

fn stat(server: usize, handle: u32) -> Result<Metadata> {
    let e = call(vfs::stat(Endpoint(server), handle))?;
    Ok(Metadata { size: e.size, modified: e.modified, attributes: e.attributes, is_dir: e.directory })
}

fn close(server: usize, handle: u32) { let _ = vfs::close(Endpoint(server), handle); }

/// A directory handle, and the slot of the server that holds it.
pub struct Dir { handle: u32, owned: bool, server: usize }

impl Dir {
    /// The root of a volume (`""`: the boot disk, `"ram"`, `"models"`: the model disk, read-only; another: storefs's).
    pub fn root(volume: &str) -> Result<Self> { let (handle, server) = root(volume)?; Ok(Self { handle, owned: false, server }) }
    /// A directory by path (`ram:docs`, `data/notes`).
    pub fn open(path: &str) -> Result<Self> { let (volume, rest) = locate(path)?; Self::root(volume)?.dir(rest, false) }
    /// The same, made with its parents if missing.
    pub fn create(path: &str) -> Result<Self> { let (volume, rest) = locate(path)?; Self::root(volume)?.dir(rest, true) }
    /// A directory below this one.
    pub fn dir(&self, path: &str, create: bool) -> Result<Self> {
        if path.is_empty() { return Ok(Self { handle: self.handle, owned: false, server: self.server }); }
        Ok(Self { handle: call(vfs::open_dir(self.endpoint(), self.handle, path, create))?, owned: true, server: self.server })
    }
    /// A file below this one with `MODE_*` bits.
    pub fn file(&self, path: &str, mode: u8) -> Result<File> {
        let handle = call(vfs::open(self.endpoint(), self.handle, path, mode))?;
        let size = stat(self.server, handle).map(|m| m.size as usize).unwrap_or(0);
        Ok(File { handle, size, position: 0, server: self.server })
    }
    /// Calls `visit` for every entry; returns their number.
    pub fn list(&self, mut visit: impl FnMut(&DirEntry)) -> Result<usize> {
        let mut start = 0u32;
        loop {
            let entries = call(vfs::list(self.endpoint(), self.handle, start))?;
            if entries.is_empty() { return Ok(start as usize); }
            for e in entries.as_slice() { visit(&DirEntry { name: e.name.as_str().as_bytes(), size: e.size, is_dir: e.directory, flags: e.attributes, modified: e.modified }); }
            start += entries.len() as u32;
        }
    }
    /// Removes a file or an empty directory below this one.
    pub fn remove(&self, path: &str) -> Result<()> { call(vfs::remove(self.endpoint(), self.handle, path)) }
    /// Renames or moves `from` (below this directory) to `to` below `target` (the same volume).
    pub fn rename(&self, from: &str, target: &Dir, to: &str) -> Result<()> {
        if target.server != self.server { return Err(Error::Invalid); }
        call(vfs::rename(self.endpoint(), self.handle, from, target.handle, to))
    }
    pub fn metadata(&self) -> Result<Metadata> { stat(self.server, self.handle) }
    fn endpoint(&self) -> Endpoint { Endpoint(self.server) }
    /// The volume this directory is on.
    pub fn volume(&self) -> Result<VolumeInfo> {
        let v = call(vfs::volume(self.endpoint(), self.handle))?;
        let mut label = [b' '; 11];
        for (i, b) in v.label.as_str().bytes().take(11).enumerate() { label[i] = b; }
        Ok(VolumeInfo { label, fat_bits: v.fat_bits, bytes: v.bytes, free: v.free, cluster: v.cluster, writable: v.writable })
    }
    /// Writes what is cached for this volume to the disk.
    pub fn flush(&self) -> Result<()> { call(vfs::flush(self.endpoint(), self.handle)) }
    /// Writes a new empty volume labelled `label` over the RAM disk (`self` is the root of `ram`; needs the user's
    /// client). Handles below the root end; roots stay valid.
    pub fn format(&self, label: &str) -> Result<()> { call(vfs::format(self.endpoint(), self.handle, label)) }
    /// A client confined to this directory (vfs.wit `scope`), received in the caller's fixed slot `receive`; it may
    /// change files only if `writable` and this handle may. For a launcher that gives a program one directory.
    pub fn scope(&self, writable: bool, receive: usize) -> Result<usize> { call(vfs::scope(self.endpoint(), self.handle, writable, receive)).map(|()| receive) }
    /// Checks this directory's volume without changing it; `visit` sees the report (idl/vfs.wit `report`).
    pub fn check<T>(&self, visit: impl FnOnce(&vfs::Report) -> T) -> Result<T> {
        Ok(visit(&call(vfs::check(self.endpoint(), self.handle))?))
    }
}

impl Drop for Dir { fn drop(&mut self) { if self.owned { close(self.server, self.handle); } } }

/// An open file (the handle belongs to the process and is closed in Drop), and the slot of the server that holds it.
pub struct File { handle: u32, size: usize, position: usize, server: usize }

impl File {
    /// Opens a file to read.
    pub fn open(path: &str) -> Result<Self> { Self::open_mode(path, 0) }
    /// Creates a file (or empties an existing one) to write.
    pub fn create(path: &str) -> Result<Self> { Self::open_mode(path, MODE_WRITE | MODE_CREATE | MODE_TRUNCATE) }
    /// Opens with `MODE_*` bits.
    pub fn open_mode(path: &str, mode: u8) -> Result<Self> { let (volume, rest) = locate(path)?; Dir::root(volume)?.file(rest, mode) }
    pub fn size(&self) -> usize { self.size }
    pub fn position(&self) -> usize { self.position }
    pub fn seek(&mut self, position: usize) { self.position = position.min(self.size); }
    /// Reads at `offset` without changing the current position.
    pub fn read_at(&self, offset: usize, buffer: &mut [u8]) -> Result<usize> {
        let mut done = 0;
        while done < buffer.len() {
            let want = (buffer.len() - done).min(CHUNK);
            let got = call(vfs::read(self.endpoint(), self.handle, (offset + done) as u32, want as u32, &mut buffer[done..done + want]))?;
            done += got;
            if got < want { break; }
        }
        Ok(done)
    }
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize> { let got = self.read_at(self.position, buffer)?; self.position += got; Ok(got) }
    /// Reads from `offset` into the memory object `target` (a capability with the write and grant rights) in one call, as
    /// much as fits or as the file has; returns the bytes. vfs_server gets a copy of a child of `target` (writable, so with
    /// the grant right: a writable child without it may only be moved), revoked once it answers (`vfs.wit` 2.4 `fill`,
    /// 262-KRN-0162).
    pub fn fill(&self, offset: u64, target: usize) -> Result<u64> {
        let lent = crate::ipc::mint(target, CAP_READ | CAP_WRITE | CAP_GRANT, 0, 0)?;
        let result = call(vfs::fill(self.endpoint(), self.handle, offset, lent));
        let _ = crate::ipc::revoke(lent);
        let _ = crate::ipc::drop_cap(lent);
        result
    }
    /// Writes at `offset` without changing the current position; the file grows.
    pub fn write_at(&mut self, offset: usize, data: &[u8]) -> Result<usize> {
        let mut done = 0;
        while done < data.len() {
            let n = (data.len() - done).min(CHUNK);
            let wrote = call(vfs::write(self.endpoint(), self.handle, (offset + done) as u32, &data[done..done + n]))? as usize;
            if wrote == 0 { return Err(Error::NoSpace); }
            done += wrote;
        }
        self.size = self.size.max(offset + done);
        Ok(done)
    }
    pub fn write(&mut self, data: &[u8]) -> Result<usize> { let n = self.write_at(self.position, data)?; self.position += n; Ok(n) }
    /// Sets the size (shorter frees space, longer reads zeros).
    pub fn truncate(&mut self, size: usize) -> Result<()> { call(vfs::truncate(self.endpoint(), self.handle, size as u32))?; self.size = size; self.position = self.position.min(size); Ok(()) }
    pub fn metadata(&self) -> Result<Metadata> { stat(self.server, self.handle) }
    /// Writes what is cached for this file's volume to the disk.
    pub fn flush(&self) -> Result<()> { call(vfs::flush(self.endpoint(), self.handle)) }
    fn endpoint(&self) -> Endpoint { Endpoint(self.server) }
}

impl Drop for File { fn drop(&mut self) { close(self.server, self.handle); } }

/// Directory entry: `flags` are `ENTRY_*`, `modified` is FAT date << 16 | FAT time (see `fat_time`).
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
pub fn remove(path: &str) -> Result<()> { let (volume, rest) = locate(path)?; Dir::root(volume)?.remove(rest) }

/// Renames or moves within a volume.
pub fn rename(from: &str, to: &str) -> Result<()> {
    let ((volume, from), (target, to)) = (locate(from)?, locate(to)?);
    if !volume.eq_ignore_ascii_case(target) { return Err(Error::Invalid); }
    let root = Dir::root(volume)?;
    root.rename(from, &root, to)
}

/// Size, time and attributes of a file or directory.
pub fn metadata(path: &str) -> Result<Metadata> {
    if locate(path)?.1.is_empty() { return Dir::open(path)?.metadata(); }
    match File::open(path) { Ok(file) => file.metadata(), Err(Error::IsDirectory) => Dir::open(path)?.metadata(), Err(e) => Err(e) }
}

/// The volume `name` (`""`, `"ram"`, `"log"`, `"models"` or a store volume).
pub fn volume(name: &str) -> Result<VolumeInfo> { Dir::root(name)?.volume() }

/// Checks volume `name` (`""`, `"ram"`, `"log"`, `"models"` or a store volume) without changing it.
pub fn check<T>(name: &str, visit: impl FnOnce(&vfs::Report) -> T) -> Result<T> { Dir::root(name)?.check(visit) }

/// A program's own directory, `system/own/<name>/` (000-KRN-0170): the loader scopes it into SLOT_OWN for a program it
/// runs by its name from the booted slot, the volume's root or data/apps/, and no other program reads it. These calls go
/// there alone, apart from the program's other files; without one they fail with `NotFound`.
pub mod own {
    use super::{call, DirEntry, Error, Result, CHUNK, MODE_CREATE, MODE_TRUNCATE, MODE_WRITE};
    use crate::abi::{CAP_KIND_ENDPOINT, SLOT_OWN};
    use crate::idl::vfs;
    use crate::ipc::Endpoint;

    const OWN: Endpoint = Endpoint(SLOT_OWN);

    /// Whether the loader gave this program its own directory.
    pub fn given() -> bool { crate::dev::cap_info(SLOT_OWN).0 == CAP_KIND_ENDPOINT }

    // `work` on the directory's root, closed after.
    fn in_root<T>(work: impl FnOnce(u32) -> Result<T>) -> Result<T> {
        if !given() { return Err(Error::NotFound); }
        let root = call(vfs::root(OWN, ""))?;
        let result = work(root);
        let _ = vfs::close(OWN, root);
        result
    }

    // `work` on file `name` opened with `mode`, closed after.
    fn in_file<T>(name: &str, mode: u8, work: impl FnOnce(u32) -> Result<T>) -> Result<T> {
        in_root(|root| {
            let file = call(vfs::open(OWN, root, name, mode))?;
            let result = work(file);
            let _ = vfs::close(OWN, file);
            result
        })
    }

    /// File `name` into `buffer`, as much as fits; returns the bytes read.
    pub fn read(name: &str, buffer: &mut [u8]) -> Result<usize> {
        in_file(name, 0, |file| {
            let mut done = 0;
            while done < buffer.len() {
                let want = (buffer.len() - done).min(CHUNK);
                let got = call(vfs::read(OWN, file, done as u32, want as u32, &mut buffer[done..done + want]))?;
                done += got;
                if got < want { break; }
            }
            Ok(done)
        })
    }

    /// File `name` made or replaced with `data`.
    pub fn write(name: &str, data: &[u8]) -> Result<()> {
        in_file(name, MODE_WRITE | MODE_CREATE | MODE_TRUNCATE, |file| {
            let mut done = 0;
            while done < data.len() {
                let n = (data.len() - done).min(CHUNK);
                let wrote = call(vfs::write(OWN, file, done as u32, &data[done..done + n]))? as usize;
                if wrote == 0 { return Err(Error::NoSpace); }
                done += wrote;
            }
            call(vfs::flush(OWN, file))
        })
    }

    /// Calls `visit` for every entry of the directory; returns their number.
    pub fn list(mut visit: impl FnMut(&DirEntry)) -> Result<usize> {
        in_root(|root| {
            let mut start = 0u32;
            loop {
                let entries = call(vfs::list(OWN, root, start))?;
                if entries.is_empty() { return Ok(start as usize); }
                for e in entries.as_slice() { visit(&DirEntry { name: e.name.as_str().as_bytes(), size: e.size, is_dir: e.directory, flags: e.attributes, modified: e.modified }); }
                start += entries.len() as u32;
            }
        })
    }

    /// Removes file `name`, or an empty directory.
    pub fn remove(name: &str) -> Result<()> { in_root(|root| call(vfs::remove(OWN, root, name))) }
}
