#![no_std]
#![no_main]
// vfs_server v2 (idl/vfs.wit): FAT volumes from the block drivers — the boot disk and the RAM disk `ram` — read and
// written through handles. A handle belongs to the client that opened it (PID and badge) and carries a zone: what it
// may change. A client's badge decides the zone of a root: applications get read-only roots; the user's badge (the
// shell's client) writes anywhere on `ram` and in the boot disk's `data` directory only, so boot files are never
// writable. A handle opened from another never has a wider zone (MC-3.4); `..` is refused (paths stay below a handle).
extern crate alloc;
mod disk;
mod fat;

use alloc::string::String;
use alloc::vec::Vec;
use disk::Disk;
use fat::{Node, Volume};
use mind::abi::*;
use mind::idl::vfs::{self, Error, Request};
use mind::idl::{rtc, wire};
use mind::ipc::{self, Endpoint};
use mind::mem::Mapping;

const RECEIVED: usize = 9;
const HANDLES: usize = 96;

/// What a handle may change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Zone { ReadOnly, Writable, BootRoot }

impl Zone {
    // The zone of `name` below a directory of this zone: below the boot root only `data` is writable.
    fn below(self, name: &str) -> Zone {
        match self { Zone::BootRoot => if fat::same_name(name, "data") { Zone::Writable } else { Zone::ReadOnly }, zone => zone }
    }
}

struct Handle { owner: u64, badge: u16, volume: usize, node: Node, name: String, zone: Zone }

struct Mounted { name: &'static str, volume: Volume<Disk> }

struct Server { volumes: Vec<Mounted>, handles: Vec<Option<Handle>> }

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
    fn walk(&mut self, volume: usize, mut node: Node, mut zone: Zone, parts: &[&str], create: bool) -> Result<(Node, Zone), Error> {
        for part in parts {
            let next = zone.below(part);
            let v = &mut self.volumes[volume].volume;
            node = match v.find(&node, part) {
                Ok(entry) if entry.node.is_dir() => entry.node,
                Ok(_) => return Err(Error::NotDirectory),
                Err(fat::Error::NotFound) if create => {
                    if next != Zone::Writable { return Err(if v.writable() { Error::Denied } else { Error::ReadOnly }); }
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
        let (node, zone) = self.walk(volume, node, zone, folders, false)?;
        Ok((volume, node, zone, name))
    }

    // After a change to a file, other handles of the same entry see its new size and clusters.
    fn refresh(&mut self, volume: usize, node: Node) {
        for h in self.handles.iter_mut().flatten() { if h.volume == volume && node.entry.is_some() && h.node.entry == node.entry { h.node = node; } }
    }

    fn writable(&self, zone: Zone, volume: usize) -> Result<(), Error> {
        if !self.volumes[volume].volume.writable() { return Err(Error::ReadOnly); }
        if zone != Zone::Writable { return Err(Error::Denied); }
        Ok(())
    }

    fn serve(&mut self, request: Request, sender: u64, badge: u16, bytes: &mut [u8]) -> mind::Result<()> {
        let user = badge & VFS_BADGE_USER != 0;
        match request {
            Request::Root { payload, .. } => {
                let name = match vfs::args_root(bytes, payload) { Ok(name) => String::from(name), Err(reason) => return wire::reject(reason) };
                let result = (|| {
                    let volume = self.volumes.iter().position(|m| m.name.eq_ignore_ascii_case(&name)).ok_or(Error::NotFound)?;
                    let zone = if !user { Zone::ReadOnly } else if self.volumes[volume].name.is_empty() { Zone::BootRoot } else { Zone::Writable };
                    let node = self.volumes[volume].volume.root();
                    self.add(Handle { owner: sender, badge, volume, node, name: String::new(), zone })
                })();
                vfs::reply_root(result)
            }
            Request::OpenDir { payload, dir, create, .. } => {
                let path = match vfs::args_open_dir(bytes, payload) { Ok(p) => String::from(p), Err(reason) => return wire::reject(reason) };
                let result = (|| {
                    let parts = parts(&path)?;
                    let h = self.get(dir, sender, badge)?;
                    let (volume, node, zone, name) = (h.volume, h.node, h.zone, h.name.clone());
                    if !node.is_dir() { return Err(Error::NotDirectory); }
                    let (node, zone) = self.walk(volume, node, zone, &parts, create)?;
                    let name = parts.last().map_or(name, |p| String::from(*p));
                    self.add(Handle { owner: sender, badge, volume, node, name, zone })
                })();
                vfs::reply_open_dir(result)
            }
            Request::Open { payload, dir, mode, .. } => {
                let path = match vfs::args_open(bytes, payload) { Ok(p) => String::from(p), Err(reason) => return wire::reject(reason) };
                let result = (|| {
                    let (volume, parent, zone, name) = self.parent(dir, sender, badge, &path)?;
                    let zone = zone.below(name);
                    let write = mode & (VFS_MODE_WRITE | VFS_MODE_CREATE | VFS_MODE_TRUNCATE) != 0;
                    if write { self.writable(zone, volume)?; }
                    let v = &mut self.volumes[volume].volume;
                    let mut node = match v.find(&parent, name) {
                        Ok(_) if mode & VFS_MODE_NEW != 0 => return Err(Error::Exists),
                        Ok(entry) => entry.node,
                        Err(fat::Error::NotFound) if mode & VFS_MODE_CREATE != 0 => v.create(&parent, name, false, now()).map_err(error)?,
                        Err(e) => return Err(error(e)),
                    };
                    if node.is_dir() { return Err(Error::IsDirectory); }
                    if mode & VFS_MODE_TRUNCATE != 0 && node.size != 0 { v.truncate(&mut node, 0, now()).map_err(error)?; }
                    let zone = if mode & VFS_MODE_WRITE != 0 { zone } else { Zone::ReadOnly };
                    let id = self.add(Handle { owner: sender, badge, volume, node, name: String::from(name), zone })?;
                    self.refresh(volume, node);
                    Ok(id)
                })();
                vfs::reply_open(result)
            }
            Request::Read { file, offset, length, .. } => {
                let mut data = Vec::new();
                let result = (|| {
                    let h = self.get(file, sender, badge)?;
                    let (volume, node) = (h.volume, h.node);
                    // The reply must fit the buffer: 4 bytes of length, then the data.
                    data.resize((length as usize).min(bytes.len().saturating_sub(4)).min(65536), 0);
                    let n = self.volumes[volume].volume.read(&node, offset, &mut data).map_err(error)?;
                    data.truncate(n);
                    Ok(())
                })();
                vfs::reply_read(bytes, result.map(|_| &data[..]))
            }
            Request::Write { payload, file, offset, .. } => {
                let data = match vfs::args_write(bytes, payload) { Ok(d) => Vec::from(d), Err(reason) => return wire::reject(reason) };
                let result = (|| {
                    let h = self.get(file, sender, badge)?;
                    let (volume, mut node, zone) = (h.volume, h.node, h.zone);
                    self.writable(zone, volume)?;
                    let n = self.volumes[volume].volume.write(&mut node, offset, &data, now()).map_err(error)?;
                    self.refresh(volume, node);
                    Ok(n as u32)
                })();
                vfs::reply_write(result)
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
                vfs::reply_truncate(result)
            }
            Request::Stat { handle, .. } => {
                let result = self.get(handle, sender, badge).map(|h| (h.name.clone(), h.node));
                vfs::reply_stat(bytes, result.as_ref().map(|(name, node)| entry(name, node)).map_err(|e| *e))
            }
            Request::List { dir, start, .. } => {
                let result = (|| {
                    let h = self.get(dir, sender, badge)?;
                    let (volume, node) = (h.volume, h.node);
                    if !node.is_dir() { return Err(Error::NotDirectory); }
                    self.volumes[volume].volume.list(&node).map_err(error)
                })();
                match result {
                    Ok(all) => {
                        // As many entries as fit: 4 bytes of count, each entry its name and 12 bytes.
                        let (mut used, mut items) = (4usize, Vec::new());
                        for e in all.iter().skip(start as usize).take(256) {
                            let name = clip(&e.name);
                            if used + name.len() + 12 > bytes.len() { break; }
                            used += name.len() + 12;
                            items.push(entry(name, &e.node));
                        }
                        vfs::reply_list(bytes, Ok(&items))
                    }
                    Err(e) => vfs::reply_list(bytes, Err(e)),
                }
            }
            Request::Remove { payload, dir, .. } => {
                let path = match vfs::args_remove(bytes, payload) { Ok(p) => String::from(p), Err(reason) => return wire::reject(reason) };
                let result = (|| {
                    let (volume, parent, zone, name) = self.parent(dir, sender, badge, &path)?;
                    self.writable(zone.below(name), volume)?;
                    let v = &mut self.volumes[volume].volume;
                    let entry = v.find(&parent, name).map_err(error)?;
                    v.remove(&parent, name).map_err(error)?;
                    // Handles of the removed entry are closed: its clusters may be reused.
                    for slot in self.handles.iter_mut() {
                        if slot.as_ref().is_some_and(|h| h.volume == volume && (h.node.entry == entry.node.entry || (entry.node.cluster >= 2 && h.node.cluster == entry.node.cluster))) { *slot = None; }
                    }
                    Ok(())
                })();
                vfs::reply_remove(result)
            }
            Request::Rename { payload, dir, target, .. } => {
                let (from, to) = match vfs::args_rename(bytes, payload) { Ok((f, t)) => (String::from(f), String::from(t)), Err(reason) => return wire::reject(reason) };
                let result = (|| {
                    let (volume, source, source_zone, name) = self.parent(dir, sender, badge, &from)?;
                    let (target_volume, destination, target_zone, new_name) = self.parent(target, sender, badge, &to)?;
                    if volume != target_volume { return Err(Error::Invalid); }
                    self.writable(source_zone.below(name), volume)?;
                    self.writable(target_zone.below(new_name), volume)?;
                    let v = &mut self.volumes[volume].volume;
                    let old = v.find(&source, name).map_err(error)?.node;
                    let moved = v.rename(&source, name, &destination, new_name).map_err(error)?;
                    for h in self.handles.iter_mut().flatten() { if h.volume == volume && old.entry.is_some() && h.node.entry == old.entry { h.node = moved; h.name = String::from(new_name); } }
                    Ok(())
                })();
                vfs::reply_rename(result)
            }
            Request::Volume { handle, .. } => {
                let result = (|| {
                    let h = self.get(handle, sender, badge)?;
                    let (volume, zone) = (h.volume, h.zone);
                    let m = &mut self.volumes[volume];
                    let free = m.volume.free_clusters().map_err(error)? as u64 * m.volume.cluster_bytes() as u64;
                    Ok((m.name, m.volume.label(), m.volume.bits(), m.volume.total_bytes(), free, m.volume.cluster_bytes(), m.volume.writable() && zone != Zone::ReadOnly))
                })();
                vfs::reply_volume(bytes, result.as_ref().map(|r| vfs::Volume { name: r.0, label: &r.1, fat_bits: r.2, bytes: r.3, free: r.4, cluster: r.5, writable: r.6 }).map_err(|e| *e))
            }
            Request::Check { handle, .. } => {
                // A check reads the whole FAT and directory tree; any client may ask (it changes nothing).
                let result = self.get(handle, sender, badge).map(|h| h.volume).and_then(|volume| self.volumes[volume].volume.check().map_err(error));
                vfs::reply_check(bytes, result.as_ref().map(|r| vfs::Report { files: r.files, directories: r.directories, used: r.used, free: r.free, lost: r.lost,
                    lost_chains: r.lost_chains, cross_linked: r.cross_linked, bad_chains: r.bad_chains, sizes: r.sizes, bad_entries: r.bad_entries, dirty: r.dirty,
                    first: &r.first }).map_err(|e| *e))
            }
            Request::Flush { handle } => {
                let result = self.get(handle, sender, badge).map(|h| h.volume).and_then(|volume| self.volumes[volume].volume.flush().map_err(error));
                vfs::reply_flush(result)
            }
            Request::Close { handle } => {
                // Closing a file opened for writing flushes its volume.
                let result = self.get(handle, sender, badge).map(|h| (h.volume, h.zone == Zone::Writable && !h.node.is_dir()));
                let result = result.and_then(|(volume, wrote)| {
                    self.handles[handle as usize] = None;
                    if wrote { self.volumes[volume].volume.flush().map_err(error) } else { Ok(()) }
                });
                vfs::reply_close(result)
            }
        }
    }
}

// Names are sent up to 255 bytes of UTF-8 (a longer one is cut at a character).
fn clip(name: &str) -> &str { let mut end = name.len().min(255); while !name.is_char_boundary(end) { end -= 1; } &name[..end] }

fn entry<'a>(name: &'a str, node: &Node) -> vfs::Entry<'a> {
    let a = node.attributes;
    let attributes = node.is_dir() as u8 * VFS_ENTRY_DIR | if a & fat::ATTR_HIDDEN != 0 { VFS_ENTRY_HIDDEN } else { 0 } | if a & fat::ATTR_SYSTEM != 0 { VFS_ENTRY_SYSTEM } else { 0 }
        | if a & fat::ATTR_READ_ONLY != 0 { VFS_ENTRY_READ_ONLY } else { 0 } | if a & fat::ATTR_ARCHIVE != 0 { VFS_ENTRY_ARCHIVE } else { 0 };
    vfs::Entry { name: clip(name), size: node.size, modified: node.modified, attributes, directory: node.is_dir() }
}

fn device_name(kind: usize) -> &'static str { match kind { BLOCK_KIND_ATA => "ATA", BLOCK_KIND_AHCI => "AHCI", BLOCK_KIND_USB => "USB", BLOCK_KIND_RAM => "RAM", _ => "?" } }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut volumes = Vec::new();
    // The boot disk: the first drive with a FAT volume (order: ata, ahci, usb_storage).
    for slot in SLOT_BLOCK_FIRST..SLOT_BLOCK_FIRST + BLOCK_DEVICES {
        if mind::dev::cap_info(slot).0 != CAP_KIND_ENDPOINT { continue; }
        let Some(disk) = mind::block::Device::open(Endpoint(slot)).ok().and_then(Disk::new) else { continue };
        if let Ok(volume) = Volume::mount(disk) {
            mind::println!("[VFS] MOUNTED FAT{} FROM {} AT LBA {}{}", volume.bits(), device_name(volume.disk.kind()), volume.start(),
                           if volume.writable() { " (DEVICE WRITABLE)" } else { " (READ-ONLY DEVICE)" });
            volumes.push(Mounted { name: "", volume });
            break;
        }
    }
    if volumes.is_empty() { mind::println!("[VFS] NO FAT VOLUME ON ANY BLOCK DEVICE"); }
    // The RAM disk: formatted when blank (its contents never outlive the boot).
    if mind::dev::cap_info(SLOT_RAMDISK).0 == CAP_KIND_ENDPOINT {
        if let Some(mut disk) = mind::block::Device::open(Endpoint(SLOT_RAMDISK)).ok().and_then(Disk::new) {
            let mut probe = [0u8; fat::SECTOR];
            let blank = fat::Sectors::read(&mut disk, 0, &mut probe) && probe[510..512] != [0x55, 0xAA];
            if blank && fat::format(&mut disk, "MIND RAM", now()).is_err() { mind::println!("[VFS] CANNOT FORMAT THE RAM DISK"); }
            match Volume::mount(disk) {
                Ok(volume) => { mind::println!("[VFS] MOUNTED FAT{} FROM RAM AS RAM: ({} KB)", volume.bits(), volume.total_bytes() / 1024); volumes.push(Mounted { name: "ram", volume }); }
                Err(_) => mind::println!("[VFS] NO FAT VOLUME ON THE RAM DISK"),
            }
        }
    }
    let mut server = Server { volumes, handles: (0..HANDLES).map(|_| None).collect() };
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        let decoded = vfs::decode(&request, RECEIVED);
        let mut mapping = if request.cap_received { Mapping::new(RECEIVED).ok() } else { None };
        let mut empty = [0u8; 0];
        let bytes: &mut [u8] = match mapping.as_mut() { Some(m) => m.as_mut_slice(), None => &mut empty };
        let _ = match decoded {
            Ok(call) => server.serve(call, request.sender, request.badge, bytes),
            Err(reason) => wire::reject(reason),
        };
        drop(mapping);
        if request.cap_received { let _ = ipc::drop_cap(RECEIVED); }
    }
}
