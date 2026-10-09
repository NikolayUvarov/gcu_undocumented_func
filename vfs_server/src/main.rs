#![no_std]
#![no_main]
// vfs_server v2 (idl/vfs.wit): FAT volumes from the block drivers — the boot disk, its log partition `log` and the RAM
// disk `ram` — read and written through handles, and the model disk `models` (251), read only. A handle belongs to the
// client that opened it (PID and badge) and carries a zone: what it may change. A client's badge decides the zone of a
// root: applications get read-only roots; the user's badge (the shell's client) writes anywhere on `ram` and `log` and
// in the boot disk's `data` directory only, so boot files and models are never writable. A handle opened from another
// never has a wider zone (MC-3.4); `..` is refused (paths stay below a handle). The boot disk's `system/` holds the
// private directories of services (351-NET-0005): only the client with the directory's badge may open, read or write
// it. Each boot's system log goes to the log volume (journal.rs).
extern crate alloc;
mod disk;
mod fat;
mod journal;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use disk::{Disk, Shared};
use fat::{Node, Volume};
use journal::Journal;
use mind::abi::*;
use mind::fs::{BADGE_KEYSTORE, BADGE_NETPOLICY, BADGE_USER, ENTRY_ARCHIVE, ENTRY_DIR, ENTRY_HIDDEN, ENTRY_READ_ONLY, ENTRY_SYSTEM, MODE_CREATE, MODE_NEW, MODE_TRUNCATE, MODE_WRITE};
use mind::idl::codec::Text;
use mind::idl::vfs::{self, Error, Request};
use mind::idl::wire::Call;
use mind::idl::{rtc, wire};
use mind::ipc::{self, Endpoint};

const RECEIVED: usize = 9;
const MODELS_LABEL: &str = "MIND MODELS";
const HANDLES: usize = 96;

/// What a handle may change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Zone { ReadOnly, Writable, BootRoot, BootReadOnly, System, Hidden }

// The private directories of the boot disk's `system/`, each with the only badge that may open, read or write it.
const PRIVATE: [(&str, u16); 2] = [("keystore", BADGE_KEYSTORE), ("netpolicy", BADGE_NETPOLICY)];

impl Zone {
    // The zone of `name` below a directory of this zone, for a client with `badge`: below the boot root `data` is the
    // user's to write and `system` holds private directories; below `system` everything is hidden but the client's own.
    fn below(self, name: &str, badge: u16) -> Zone {
        match self {
            Zone::BootRoot if fat::same_name(name, "data") => Zone::Writable,
            Zone::BootRoot | Zone::BootReadOnly if fat::same_name(name, "system") => Zone::System,
            Zone::BootRoot | Zone::BootReadOnly => Zone::ReadOnly,
            Zone::System => match PRIVATE.iter().find(|(dir, _)| fat::same_name(name, dir)) {
                Some(&(_, owner)) if owner == badge => Zone::Writable,
                _ => Zone::Hidden,
            },
            zone => zone,
        }
    }
}

struct Handle { owner: u64, badge: u16, volume: usize, node: Node, name: String, zone: Zone }

struct Mounted { name: &'static str, volume: Volume<Shared> }

// A client confined to one directory (`scope`): the capability vfs_server minted with the scope's badge stays here, so
// ending the scope revokes every copy of it. The first task that opens a root with the badge is its only user.
struct Scope { badge: u16, volume: usize, node: Node, name: String, zone: Zone, user: Option<u64>, made_ms: u64, cap: usize }

struct Server { volumes: Vec<Mounted>, handles: Vec<Option<Handle>>, scopes: Vec<Option<Scope>>, next_badge: u16, journal: Option<(usize, Journal)> }

const SCOPES: usize = 8;
const SCOPE_UNUSED_MS: u64 = 60_000; // a scope nobody took up ends after a minute
// Badges of scoped clients start here (below: applications 0, the user 1).
const SCOPE_BADGE_FIRST: u16 = 0x100;
// The label of the boot disk's log partition, which every computer can read and write (211-PRT-0006).
const LOG_LABEL: &str = "MIND LOG";

fn error(e: fat::Error) -> Error {
    match e {
        fat::Error::NotFound => Error::NotFound, fat::Error::Exists => Error::Exists, fat::Error::NotEmpty => Error::NotEmpty, fat::Error::Invalid => Error::Invalid,
        fat::Error::NoSpace => Error::NoSpace, fat::Error::ReadOnly => Error::ReadOnly, fat::Error::Io => Error::Io, fat::Error::Name => Error::Name,
        fat::Error::NotDirectory => Error::NotDirectory, fat::Error::IsDirectory => Error::IsDirectory,
    }
}

// The checked parts of a relative path: no `.` or `..`, nothing empty but separators.
fn parts(path: &str) -> Result<Vec<&str>, Error> {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.iter().any(|p| *p == "." || *p == "..") { return Err(Error::Invalid); }
    Ok(parts)
}

// Calendar time for new and changed entries, from the RTC (none: 2000-01-01).
fn now() -> u32 {
    let rtc = Endpoint(SLOT_VFS_RTC);
    let days = rtc::date(rtc).ok().flatten().unwrap_or(0);
    let seconds = rtc::now(rtc).ok().flatten().unwrap_or(0);
    fat::stamp(days, seconds)
}

impl Server {
    // The system log's new records to this boot's file, when due; open handles of the file see its new size.
    fn save_journal(&mut self) {
        let now_ms = mind::time::uptime_ms() as u64;
        let Some((index, journal)) = self.journal.as_mut() else { return };
        if now_ms < journal.due { return; }
        journal.due = now_ms + journal::SAVE_MS;
        let index = *index;
        if let Some(node) = journal.save(&mut self.volumes[index].volume, Endpoint(SLOT_LOG), now()) { self.refresh(index, node); }
    }

    fn get(&self, id: u32, sender: u64, badge: u16) -> Result<&Handle, Error> {
        match self.handles.get(id as usize) { Some(Some(h)) if h.owner == sender && h.badge == badge => Ok(h), _ => Err(Error::Invalid) }
    }

    fn add(&mut self, handle: Handle) -> Result<u32, Error> {
        if !self.handles.iter().any(Option::is_none) {
            // Handles of dead clients are freed when the table is full.
            for slot in self.handles.iter_mut() { if slot.as_ref().is_some_and(|h| !mind::process::alive(h.owner)) { *slot = None; } }
        }
        let id = self.handles.iter().position(Option::is_none).ok_or(Error::Handles)?;
        self.handles[id] = Some(handle);
        Ok(id as u32)
    }

    // From directory node `node` (zone `zone`) along `parts`; `create` makes missing directories where allowed.
    fn walk(&mut self, volume: usize, mut node: Node, mut zone: Zone, parts: &[&str], create: bool, badge: u16) -> Result<(Node, Zone), Error> {
        for part in parts {
            let next = zone.below(part, badge);
            if next == Zone::Hidden { return Err(Error::Denied); }
            let v = &mut self.volumes[volume].volume;
            node = match v.find(&node, part) {
                Ok(entry) if entry.node.is_dir() => entry.node,
                Ok(_) => return Err(Error::NotDirectory),
                Err(fat::Error::NotFound) if create => {
                    // `system` itself is made by the first service that makes its private directory.
                    let owner = next == Zone::System && PRIVATE.iter().any(|&(_, b)| b == badge);
                    if next != Zone::Writable && !owner { return Err(if v.writable() { Error::Denied } else { Error::ReadOnly }); }
                    v.create(&node, part, true, now()).map_err(error)?
                }
                Err(e) => return Err(error(e)),
            };
            zone = next;
        }
        Ok((node, zone))
    }

    // The directory of `path` below handle `dir` and the last name of the path.
    fn parent<'p>(&mut self, dir: u32, sender: u64, badge: u16, path: &'p str) -> Result<(usize, Node, Zone, &'p str), Error> {
        let parts = parts(path)?;
        let (&name, folders) = parts.split_last().ok_or(Error::Invalid)?;
        let h = self.get(dir, sender, badge)?;
        let (volume, node, zone) = (h.volume, h.node, h.zone);
        if !node.is_dir() { return Err(Error::NotDirectory); }
        let (node, zone) = self.walk(volume, node, zone, folders, false, badge)?;
        Ok((volume, node, zone, name))
    }

    // After a change to a file, other handles of the same entry see its new size and clusters.
    fn refresh(&mut self, volume: usize, node: Node) {
        for h in self.handles.iter_mut().flatten() { if h.volume == volume && node.entry.is_some() && h.node.entry == node.entry { h.node = node; } }
    }

    fn scope_of(&self, badge: u16) -> Option<usize> {
        if badge < SCOPE_BADGE_FIRST { return None; }
        self.scopes.iter().position(|s| s.as_ref().is_some_and(|s| s.badge == badge))
    }

    fn end_scope(&mut self, index: usize) {
        if let Some(scope) = self.scopes[index].take() {
            let _ = ipc::revoke(scope.cap);
            let _ = ipc::drop_cap(scope.cap);
            mind::println!("[VFS] SCOPE {:#x} ENDED", scope.badge);
        }
    }

    // Scopes whose task has ended, or that nobody took up, are revoked.
    fn sweep_scopes(&mut self) {
        let now = mind::time::uptime_ms() as u64;
        for index in 0..self.scopes.len() {
            let over = self.scopes[index].as_ref().is_some_and(|s| match s.user { Some(pid) => !mind::process::alive(pid), None => now.saturating_sub(s.made_ms) > SCOPE_UNUSED_MS });
            if over { self.end_scope(index); }
        }
    }

    fn writable(&self, zone: Zone, volume: usize) -> Result<(), Error> {
        if !self.volumes[volume].volume.writable() { return Err(Error::ReadOnly); }
        if zone != Zone::Writable { return Err(Error::Denied); }
        Ok(())
    }

    fn serve(&mut self, request: Request, call: Call, sender: u64, badge: u16) -> mind::Result<()> {
        let user = badge == BADGE_USER;
        match request {
            Request::Root { name } => {
                let name = name.as_str();
                let result = (|| {
                    // A scoped client's root of its volume is the scope's directory; other roots are refused.
                    if badge >= SCOPE_BADGE_FIRST {
                        let index = self.scope_of(badge).ok_or(Error::Denied)?;
                        let scope = self.scopes[index].as_mut().unwrap();
                        match scope.user { None => scope.user = Some(sender), Some(pid) if pid != sender => return Err(Error::Denied), _ => {} }
                        let (volume, node, zone, dir_name) = (scope.volume, scope.node, scope.zone, scope.name.clone());
                        if !self.volumes[volume].name.eq_ignore_ascii_case(name) { return Err(Error::Denied); }
                        return self.add(Handle { owner: sender, badge, volume, node, name: dir_name, zone });
                    }
                    let volume = self.volumes.iter().position(|m| m.name.eq_ignore_ascii_case(name)).ok_or(Error::NotFound)?;
                    let zone = match self.volumes[volume].name { "" if user => Zone::BootRoot, "" => Zone::BootReadOnly, _ if !user => Zone::ReadOnly, "models" => Zone::ReadOnly, _ => Zone::Writable };
                    let node = self.volumes[volume].volume.root();
                    self.add(Handle { owner: sender, badge, volume, node, name: String::new(), zone })
                })();
                vfs::reply_root(call, result)
            }
            Request::OpenDir { dir, path, create } => {
                let result = (|| {
                    let parts = parts(path.as_str())?;
                    let h = self.get(dir, sender, badge)?;
                    let (volume, node, zone, name) = (h.volume, h.node, h.zone, h.name.clone());
                    if !node.is_dir() { return Err(Error::NotDirectory); }
                    let (node, zone) = self.walk(volume, node, zone, &parts, create, badge)?;
                    let name = parts.last().map_or(name, |p| String::from(*p));
                    self.add(Handle { owner: sender, badge, volume, node, name, zone })
                })();
                vfs::reply_open_dir(call, result)
            }
            Request::Open { dir, path, mode } => {
                let result = (|| {
                    let (volume, parent, zone, name) = self.parent(dir, sender, badge, path.as_str())?;
                    let zone = zone.below(name, badge);
                    if zone == Zone::Hidden { return Err(Error::Denied); }
                    let write = mode & (MODE_WRITE | MODE_CREATE | MODE_TRUNCATE) != 0;
                    if write { self.writable(zone, volume)?; }
                    let v = &mut self.volumes[volume].volume;
                    let mut node = match v.find(&parent, name) {
                        Ok(_) if mode & MODE_NEW != 0 => return Err(Error::Exists),
                        Ok(entry) => entry.node,
                        Err(fat::Error::NotFound) if mode & MODE_CREATE != 0 => v.create(&parent, name, false, now()).map_err(error)?,
                        Err(e) => return Err(error(e)),
                    };
                    if node.is_dir() { return Err(Error::IsDirectory); }
                    if mode & MODE_TRUNCATE != 0 && node.size != 0 { v.truncate(&mut node, 0, now()).map_err(error)?; }
                    let zone = if mode & MODE_WRITE != 0 { zone } else { Zone::ReadOnly };
                    let id = self.add(Handle { owner: sender, badge, volume, node, name: String::from(name), zone })?;
                    self.refresh(volume, node);
                    Ok(id)
                })();
                vfs::reply_open(call, result)
            }
            Request::Read { file, offset, length } => {
                let mut data = Vec::new();
                let result = (|| {
                    let h = self.get(file, sender, badge)?;
                    let (volume, node) = (h.volume, h.node);
                    data.resize((length as usize).min(mind::fs::CHUNK), 0);
                    let n = self.volumes[volume].volume.read(&node, offset, &mut data).map_err(error)?;
                    data.truncate(n);
                    Ok(())
                })();
                vfs::reply_read(call, result.map(|_| &data[..]))
            }
            Request::Write { file, offset, data } => {
                let result = (|| {
                    let h = self.get(file, sender, badge)?;
                    let (volume, mut node, zone) = (h.volume, h.node, h.zone);
                    self.writable(zone, volume)?;
                    let n = self.volumes[volume].volume.write(&mut node, offset, data, now()).map_err(error)?;
                    self.refresh(volume, node);
                    Ok(n as u32)
                })();
                vfs::reply_write(call, result)
            }
            Request::Truncate { file, size } => {
                let result = (|| {
                    let h = self.get(file, sender, badge)?;
                    let (volume, mut node, zone) = (h.volume, h.node, h.zone);
                    self.writable(zone, volume)?;
                    self.volumes[volume].volume.truncate(&mut node, size, now()).map_err(error)?;
                    self.refresh(volume, node);
                    Ok(())
                })();
                vfs::reply_truncate(call, result)
            }
            Request::Stat { handle } => {
                let result = self.get(handle, sender, badge).map(|h| entry(&h.name, &h.node));
                vfs::reply_stat(call, result.as_ref().map_err(|e| *e))
            }
            Request::List { dir, start } => {
                let result = (|| {
                    let h = self.get(dir, sender, badge)?;
                    let (volume, node) = (h.volume, h.node);
                    if !node.is_dir() { return Err(Error::NotDirectory); }
                    self.volumes[volume].volume.list(&node).map_err(error)
                })();
                // At most 16 entries from `start` (vfs.wit `list<entry, 16>`).
                let items: Result<Vec<vfs::Entry>, Error> = result.map(|all| all.iter().skip(start as usize).take(16).map(|e| entry(&e.name, &e.node)).collect());
                vfs::reply_list(call, items.as_deref().map_err(|e| *e))
            }
            Request::Remove { dir, path } => {
                let result = (|| {
                    let (volume, parent, zone, name) = self.parent(dir, sender, badge, path.as_str())?;
                    self.writable(zone.below(name, badge), volume)?;
                    let v = &mut self.volumes[volume].volume;
                    let entry = v.find(&parent, name).map_err(error)?;
                    v.remove(&parent, name).map_err(error)?;
                    // A scope of the removed directory ends.
                    for index in 0..self.scopes.len() {
                        if self.scopes[index].as_ref().is_some_and(|s| s.volume == volume && entry.node.cluster >= 2 && s.node.cluster == entry.node.cluster) { self.end_scope(index); }
                    }
                    // Handles of the removed entry are closed: its clusters may be reused.
                    for slot in self.handles.iter_mut() {
                        if slot.as_ref().is_some_and(|h| h.volume == volume && (h.node.entry == entry.node.entry || (entry.node.cluster >= 2 && h.node.cluster == entry.node.cluster))) { *slot = None; }
                    }
                    Ok(())
                })();
                vfs::reply_remove(call, result)
            }
            Request::Rename { dir, from, target, to } => {
                let result = (|| {
                    let (volume, source, source_zone, name) = self.parent(dir, sender, badge, from.as_str())?;
                    let (target_volume, destination, target_zone, new_name) = self.parent(target, sender, badge, to.as_str())?;
                    if volume != target_volume { return Err(Error::Invalid); }
                    self.writable(source_zone.below(name, badge), volume)?;
                    self.writable(target_zone.below(new_name, badge), volume)?;
                    let v = &mut self.volumes[volume].volume;
                    let old = v.find(&source, name).map_err(error)?.node;
                    let moved = v.rename(&source, name, &destination, new_name).map_err(error)?;
                    for h in self.handles.iter_mut().flatten() { if h.volume == volume && old.entry.is_some() && h.node.entry == old.entry { h.node = moved; h.name = String::from(new_name); } }
                    Ok(())
                })();
                vfs::reply_rename(call, result)
            }
            Request::Volume { handle } => {
                let result = (|| {
                    let h = self.get(handle, sender, badge)?;
                    let (volume, zone) = (h.volume, h.zone);
                    let m = &mut self.volumes[volume];
                    let free = m.volume.free_clusters().map_err(error)? as u64 * m.volume.cluster_bytes() as u64;
                    Ok((m.name, m.volume.label(), m.volume.bits(), m.volume.total_bytes(), free, m.volume.cluster_bytes(), m.volume.writable() && matches!(zone, Zone::Writable | Zone::BootRoot)))
                })();
                let volume = result.map(|r| vfs::Volume { name: text(r.0), label: text(&r.1), fat_bits: r.2, bytes: r.3, free: r.4, cluster: r.5, writable: r.6 });
                vfs::reply_volume(call, volume.as_ref().map_err(|e| *e))
            }
            Request::Check { handle } => {
                // A check reads the whole FAT and directory tree; any client may ask (it changes nothing).
                let result = self.get(handle, sender, badge).map(|h| h.volume).and_then(|volume| self.volumes[volume].volume.check().map_err(error));
                let report = result.map(|r| vfs::Report { files: r.files, directories: r.directories, used: r.used, free: r.free, lost: r.lost,
                    lost_chains: r.lost_chains, cross_linked: r.cross_linked, bad_chains: r.bad_chains, sizes: r.sizes, bad_entries: r.bad_entries, dirty: r.dirty,
                    first: text(&r.first) });
                vfs::reply_check(call, report.as_ref().map_err(|e| *e))
            }
            Request::Scope { dir, writable } => {
                let result = (|| {
                    let h = self.get(dir, sender, badge)?;
                    if !h.node.is_dir() { return Err(Error::NotDirectory); }
                    // Never more than the caller's handle: writable only where the handle is.
                    // A read-only scope keeps what its directory hides: the boot root's and `system`'s private directories.
                    let zone = match h.zone {
                        Zone::Writable if writable => Zone::Writable,
                        Zone::BootRoot | Zone::BootReadOnly => Zone::BootReadOnly,
                        Zone::System => Zone::System,
                        _ => Zone::ReadOnly,
                    };
                    let (volume, node, name) = (h.volume, h.node, h.name.clone());
                    self.sweep_scopes();
                    let index = self.scopes.iter().position(Option::is_none).ok_or(Error::Handles)?;
                    let badge = loop {
                        let next = self.next_badge;
                        self.next_badge = if next == u16::MAX { SCOPE_BADGE_FIRST } else { next + 1 };
                        if self.scope_of(next).is_none() { break next; }
                    };
                    let cap = ipc::mint_badged(SLOT_SERVICE, CAP_WRITE | CAP_GRANT, badge).map_err(|_| Error::Handles)?;
                    mind::println!("[VFS] SCOPE {:#x} FOR {}:/{} ({})", badge, self.volumes[volume].name, name, if zone == Zone::Writable { "WRITABLE" } else { "READ-ONLY" });
                    self.scopes[index] = Some(Scope { badge, volume, node, name, zone, user: None, made_ms: mind::time::uptime_ms() as u64, cap });
                    Ok(cap)
                })();
                vfs::reply_scope(call, result)
            }
            Request::Format { handle, label } => {
                let result = (|| {
                    let h = self.get(handle, sender, badge)?;
                    let volume = h.volume;
                    // Only the RAM disk (its contents never outlive the boot anyway), only from its root, only for a
                    // client that may write there (the user's badge).
                    if self.volumes[volume].name != "ram" || h.zone != Zone::Writable { return Err(Error::Denied); }
                    if h.node.entry.is_some() || !h.node.is_dir() { return Err(Error::Invalid); }
                    let label = label.as_str();
                    if !label.bytes().all(|b| b.is_ascii_graphic() || b == b' ') { return Err(Error::Name); }
                    // Handles below the root and scopes on the volume end; root handles stay valid (clients cache them).
                    for slot in self.handles.iter_mut() { if slot.as_ref().is_some_and(|h| h.volume == volume && h.node.entry.is_some()) { *slot = None; } }
                    for index in 0..self.scopes.len() { if self.scopes[index].as_ref().is_some_and(|s| s.volume == volume) { self.end_scope(index); } }
                    let label = if label.trim().is_empty() { "MIND RAM" } else { label };
                    self.volumes[volume].volume.reformat(label, now()).map_err(error)?;
                    let root = self.volumes[volume].volume.root();
                    for h in self.handles.iter_mut().flatten().filter(|h| h.volume == volume) { h.node = root; }
                    mind::println!("[VFS] FORMATTED RAM: AS {} (FAT{})", self.volumes[volume].volume.label(), self.volumes[volume].volume.bits());
                    Ok(())
                })();
                vfs::reply_format(call, result)
            }
            Request::Flush { handle } => {
                let result = self.get(handle, sender, badge).map(|h| h.volume).and_then(|volume| self.volumes[volume].volume.flush().map_err(error));
                vfs::reply_flush(call, result)
            }
            Request::Close { handle } => {
                // Closing a file opened for writing flushes its volume.
                let result = self.get(handle, sender, badge).map(|h| (h.volume, h.zone == Zone::Writable && !h.node.is_dir()));
                let result = result.and_then(|(volume, wrote)| {
                    self.handles[handle as usize] = None;
                    if wrote { self.volumes[volume].volume.flush().map_err(error) } else { Ok(()) }
                });
                vfs::reply_close(call, result)
            }
        }
    }
}

// Text of at most N bytes of UTF-8 (a longer one is cut at a character).
fn text<const N: usize>(text: &str) -> Text<N> { let mut end = text.len().min(N); while !text.is_char_boundary(end) { end -= 1; } Text::new(&text[..end]).unwrap_or_default() }

fn entry(name: &str, node: &Node) -> vfs::Entry {
    let a = node.attributes;
    let attributes = node.is_dir() as u8 * ENTRY_DIR | if a & fat::ATTR_HIDDEN != 0 { ENTRY_HIDDEN } else { 0 } | if a & fat::ATTR_SYSTEM != 0 { ENTRY_SYSTEM } else { 0 }
        | if a & fat::ATTR_READ_ONLY != 0 { ENTRY_READ_ONLY } else { 0 } | if a & fat::ATTR_ARCHIVE != 0 { ENTRY_ARCHIVE } else { 0 };
    vfs::Entry { name: text(name), size: node.size, modified: node.modified, attributes, directory: node.is_dir() }
}

// This boot's log file on the log volume, when vfs_server may read the system log (init gives it logd's read badge).
fn start_journal(volumes: &mut [Mounted]) -> Option<(usize, Journal)> {
    let index = volumes.iter().position(|m| m.name == "log" && m.volume.writable())?;
    if !matches!(mind::idl::log::state(Endpoint(SLOT_LOG)), Ok(Ok(_))) { mind::println!("[VFS] LOG: NOT KEPT, THE SYSTEM LOG CANNOT BE READ"); return None; }
    let rtc = Endpoint(SLOT_VFS_RTC);
    let (date, seconds) = (rtc::date(rtc).ok().flatten().map(mind::rtc::civil_from_days), rtc::now(rtc).ok().flatten());
    let mut header = String::from("MIND CORE: THE SYSTEM LOG OF ONE BOOT (211-KRN-0019). [SECONDS SINCE BOOT] TASK(PID) LINE\n");
    if let (Some((y, m, d)), Some(s)) = (date, seconds) { let _ = core::fmt::Write::write_fmt(&mut header, format_args!("STARTED {:04}-{:02}-{:02} {:02}:{:02}:{:02} BY THE MACHINE'S CLOCK\n", y, m, d, s / 3600, s / 60 % 60, s % 60)); }
    let journal = Journal::start(&mut volumes[index].volume, &header, now())?;
    mind::println!("[VFS] LOG: THIS BOOT'S SYSTEM LOG GOES TO LOG:{}", journal.name);
    Some((index, journal))
}

// Whether `volume` is the one the bootloader read the system from: on the partition the firmware named (an MBR disk's
// signature and the start; a GPT partition's start), holding the boot manifest the bootloader verified.
fn is_boot_volume(volume: &mut Volume<Shared>, first: &[u8; fat::SECTOR], info: &BootInfo) -> bool {
    let identity = &info.boot_volume;
    let start = volume.start() as u64;
    let place = match identity.kind { VOLUME_MBR => first[440..444] == identity.signature[..4] && start == identity.start, VOLUME_GPT => start == identity.start, _ => true };
    if !place { return false; }
    let expected = info.boot_slot.manifest;
    if expected == [0; 32] { return true; }
    let path = match info.boot_slot.slot { BOOT_SLOT_A => "MIND/A/MANIFEST", BOOT_SLOT_B => "MIND/B/MANIFEST", _ => "MANIFEST" };
    let root = volume.root();
    let Ok(node) = volume.lookup(&root, path) else { return false };
    let mut hash = mind::sha256::Sha256::new();
    let (mut at, mut chunk) = (0u32, [0u8; 4096]);
    while at < node.size {
        let Ok(n) = volume.read(&node, at, &mut chunk) else { return false };
        if n == 0 { return false; }
        hash.update(&chunk[..n]);
        at += n as u32;
    }
    hash.finish() == expected
}

// The boot volume as the bootloader named it.
fn describe(identity: &BootVolume) -> String {
    let s = &identity.signature;
    match identity.kind {
        VOLUME_MBR => alloc::format!("MBR DISK {:02X}{:02X}{:02X}{:02X}, PARTITION {} AT LBA {}", s[3], s[2], s[1], s[0], identity.partition, identity.start),
        VOLUME_GPT => alloc::format!("GPT PARTITION {} AT LBA {}", identity.partition, identity.start),
        _ => String::from("A VOLUME THE FIRMWARE DID NOT NAME"),
    }
}

fn device_name(kind: usize) -> &'static str { match kind { BLOCK_KIND_ATA => "ATA", BLOCK_KIND_AHCI => "AHCI", BLOCK_KIND_USB => "USB", BLOCK_KIND_VIRTIO => "VIRTIO", BLOCK_KIND_NVME => "NVME", mind::block::KIND_RAM => "RAM", _ => "?" } }

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let mut volumes = Vec::new();
    // The boot volume: the one the bootloader read the system from, on whichever drive shows it (211-KRN-0012).
    let mut boot_slot = None;
    'disks: for slot in SLOT_BLOCK_FIRST..SLOT_BLOCK_FIRST + BLOCK_DEVICES {
        if mind::dev::cap_info(slot).0 != CAP_KIND_ENDPOINT { continue; }
        let Some(disk) = mind::block::Device::open(Endpoint(slot)).ok().and_then(Disk::new).map(Shared::new) else { continue };
        let mut first = [0u8; fat::SECTOR];
        if !fat::Sectors::read(&mut disk.clone(), 0, &mut first) { continue; }
        // A GPT partition is found by the start the firmware named; MBR ones are listed in the table.
        let gpt = (info.boot_volume.kind == VOLUME_GPT).then(|| u32::try_from(info.boot_volume.start).ok()).flatten();
        for start in gpt.into_iter().chain(fat::fat_starts(&mut disk.clone()).into_iter().flatten()) {
            let Ok(mut volume) = Volume::mount_at(disk.clone(), start) else { continue };
            if !is_boot_volume(&mut volume, &first, info) { continue; }
            mind::println!("[VFS] MOUNTED FAT{} FROM {} AT LBA {}{}", volume.bits(), device_name(volume.disk.kind()), volume.start(),
                           if volume.writable() { " (DEVICE WRITABLE)" } else { " (READ-ONLY DEVICE)" });
            mind::println!("[VFS] THE BOOT VOLUME: {}, AND THE MANIFEST THE BOOTLOADER VERIFIED", describe(&info.boot_volume));
            let boot = volume.start();
            volumes.push(Mounted { name: "", volume });
            // The log partition on the same disk, by its label (211-KRN-0019).
            for start in fat::fat_starts(&mut disk.clone()).into_iter().flatten().filter(|&start| start != boot) {
                let Ok(log) = Volume::mount_at(disk.clone(), start) else { continue };
                if !log.label().eq_ignore_ascii_case(LOG_LABEL) { continue; }
                mind::println!("[VFS] MOUNTED FAT{} AT LBA {} AS LOG: ({} KB){}", log.bits(), start, log.total_bytes() / 1024, if log.writable() { "" } else { " (READ-ONLY DEVICE)" });
                volumes.push(Mounted { name: "log", volume: log });
                break;
            }
            boot_slot = Some(slot);
            break 'disks;
        }
    }
    // Another volume would give programs and data/ of another system: none is mounted in its place.
    if volumes.is_empty() { mind::println!("[VFS] THE BOOT VOLUME ({}) IS ON NO BLOCK DEVICE: NONE MOUNTED", describe(&info.boot_volume)); }
    // A volume labelled MIND MODELS on another drive is the model disk (251): `models`, read-only for every client.
    for slot in (SLOT_BLOCK_FIRST..SLOT_BLOCK_FIRST + BLOCK_DEVICES).filter(|&slot| Some(slot) != boot_slot) {
        if mind::dev::cap_info(slot).0 != CAP_KIND_ENDPOINT { continue; }
        let Some(disk) = mind::block::Device::open(Endpoint(slot)).ok().and_then(Disk::new).map(Shared::new) else { continue };
        let Ok(volume) = Volume::mount(disk) else { continue };
        if volume.label() != MODELS_LABEL { continue; }
        mind::println!("[VFS] MOUNTED FAT{} FROM {} AS MODELS: ({} MB, READ-ONLY)", volume.bits(), device_name(volume.disk.kind()), volume.total_bytes() >> 20);
        volumes.push(Mounted { name: "models", volume });
        break;
    }
    // The RAM disk: formatted when blank (its contents never outlive the boot).
    if mind::dev::cap_info(SLOT_RAMDISK).0 == CAP_KIND_ENDPOINT {
        if let Some(mut disk) = mind::block::Device::open(Endpoint(SLOT_RAMDISK)).ok().and_then(Disk::new) {
            let mut probe = [0u8; fat::SECTOR];
            let blank = fat::Sectors::read(&mut disk, 0, &mut probe) && probe[510..512] != [0x55, 0xAA];
            if blank && fat::format(&mut disk, "MIND RAM", now()).is_err() { mind::println!("[VFS] CANNOT FORMAT THE RAM DISK"); }
            match Volume::mount(Shared::new(disk)) {
                Ok(volume) => { mind::println!("[VFS] MOUNTED FAT{} FROM RAM AS RAM: ({} KB)", volume.bits(), volume.total_bytes() / 1024); volumes.push(Mounted { name: "ram", volume }); }
                Err(_) => mind::println!("[VFS] NO FAT VOLUME ON THE RAM DISK"),
            }
        }
    }
    let journal = start_journal(&mut volumes);
    let mut server = Server { volumes, handles: (0..HANDLES).map(|_| None).collect(), scopes: (0..SCOPES).map(|_| None).collect(), next_badge: SCOPE_BADGE_FIRST, journal };
    // The private copy of each request (MC-2.11); the data of a write is decoded in place from it.
    let mut scratch: Box<[u8; vfs::REQUEST_MAX]> = alloc::vec![0u8; vfs::REQUEST_MAX].into_boxed_slice().try_into().unwrap();
    loop {
        // The system log is saved between requests, at least every journal::SAVE_MS.
        let request = match server.journal.as_ref() {
            Some((_, journal)) => Endpoint::SERVICE.recv_timeout(RECEIVED, journal.due.saturating_sub(mind::time::uptime_ms() as u64).clamp(1, journal::SAVE_MS) as u32),
            None => Endpoint::SERVICE.recv(RECEIVED),
        };
        server.save_journal();
        let Ok(request) = request else { continue };
        let _ = match vfs::decode(&request, RECEIVED, &mut scratch) {
            Ok((decoded, call)) => server.serve(decoded, call, request.sender, request.badge),
            Err(reason) => if request.is_call { wire::reject(reason) } else { Ok(()) },
        };
    }
}
