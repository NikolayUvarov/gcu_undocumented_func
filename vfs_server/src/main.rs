#![no_std]
#![no_main]
// vfs_server: an isolated process that receives sectors from block drivers over IPC, parses FAT
// and hands clients descriptors bound to their PID. Data goes through the client's buffer.
mod disk;
mod fat;

use mind::abi::*;
use mind::idl::codec::Text;
use mind::idl::wire::{self, Call};
use mind::idl::vfs;
use mind::ipc::Endpoint;
use mind::sys::Error;

const RECEIVED_CAP: usize = 9;
const MAX_OPEN: usize = 32;

#[derive(Clone, Copy)]
struct Open { owner: u64, file: fat::Node, cursor: Option<(usize, u32)> } // cursor: (cluster index in the chain, cluster)

struct Server { volume: Option<fat::Volume>, open: [Option<Open>; MAX_OPEN] }

impl Server {
    // Descriptors of dead clients are freed when the table is full.
    fn allocate(&mut self, open: Open) -> Option<usize> {
        if self.open.iter().all(Option::is_some) {
            for slot in self.open.iter_mut() { if slot.is_some_and(|o| !mind::process::alive(o.owner)) { *slot = None; } }
        }
        let fd = self.open.iter().position(Option::is_none)?;
        self.open[fd] = Some(open);
        Some(fd)
    }

    fn handle(&mut self, request: vfs::Request, sender: u64, call: Call) -> mind::sys::Result<()> {
        let Some(volume) = self.volume.as_mut() else { return wire::reply_error(call, Error::NotFound) };
        let owned = |open: &Option<Open>| open.filter(|o| o.owner == sender);
        match request {
            vfs::Request::Open { path } => {
                let node = match volume.resolve(path.as_str().as_bytes()) { Some(node) if !node.is_dir => node, Some(_) => return vfs::reply_open(call, Err(Error::Invalid)), None => return vfs::reply_open(call, Err(Error::NotFound)) };
                let result = self.allocate(Open { owner: sender, file: node, cursor: None }).ok_or(Error::NoSlot).map(|fd| vfs::File { fd: fd as u32, size: node.size as u64 });
                vfs::reply_open(call, result.as_ref().map_err(|e| *e))
            }
            vfs::Request::Read { fd, offset, length } => {
                let Some(mut open) = self.open.get(fd as usize).and_then(owned) else { return vfs::reply_read(call, Err(Error::Invalid)) };
                let mut data = [0u8; 4096];
                let want = (length as usize).min(data.len());
                let got = volume.read(&open.file, offset as usize, &mut data[..want], &mut open.cursor);
                self.open[fd as usize] = Some(open);
                vfs::reply_read(call, Ok(&data[..got]))
            }
            vfs::Request::Size { fd } => vfs::reply_size(call, self.open.get(fd as usize).and_then(owned).map(|o| o.file.size as u64).ok_or(Error::Invalid)),
            vfs::Request::Close { fd } => {
                let result = self.open.get(fd as usize).and_then(owned).map(|_| ()).ok_or(Error::Invalid);
                if result.is_ok() { self.open[fd as usize] = None; }
                vfs::reply_close(call, result)
            }
            vfs::Request::List { path, start } => {
                let Some(dir) = volume.resolve(path.as_str().as_bytes()) else { return vfs::reply_list(call, Err(Error::NotFound)) };
                if !dir.is_dir { return vfs::reply_list(call, Err(Error::Invalid)); }
                let mut page = vfs::Page::default(); let mut index = 0u32;
                volume.walk(&dir, |entry| {
                    if index < start { index += 1; return true; }
                    let name = Text::new(core::str::from_utf8(entry.name).unwrap_or("?")).unwrap_or_default();
                    if !page.entries.push(vfs::Entry { name, size: entry.node.size, directory: entry.node.is_dir }) { page.next = index; return false; }
                    index += 1; true
                });
                vfs::reply_list(call, Ok(&page))
            }
        }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    // The first drive with a FAT volume (order: ata, ahci, usb_storage) becomes the root.
    let volume = (SLOT_BLOCK_FIRST..SLOT_BLOCK_FIRST + BLOCK_DEVICES)
        .filter(|&slot| mind::dev::cap_info(slot).0 == CAP_KIND_ENDPOINT)
        .filter_map(|slot| mind::block::Device::open(Endpoint(slot)).ok())
        .filter_map(disk::Disk::new)
        .find_map(fat::Volume::mount);
    match &volume {
        Some(v) => mind::println!("[VFS] MOUNTED FAT{} FROM {} AT LBA {}", v.bits(), match v.kind() { BLOCK_KIND_ATA => "ATA", BLOCK_KIND_AHCI => "AHCI", BLOCK_KIND_USB => "USB", _ => "?" }, v.start()),
        None => mind::println!("[VFS] NO FAT VOLUME ON ANY BLOCK DEVICE; REQUESTS WILL FAIL"),
    }
    let mut server = Server { volume, open: [None; MAX_OPEN] };
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        // idl/vfs.wit; descriptors belong to the sender's PID.
        match vfs::decode(&request, RECEIVED_CAP) {
            Ok((decoded, call)) => { let _ = server.handle(decoded, request.sender, call); }
            Err(reason) => if request.is_call { let _ = wire::reject(reason); },
        }
    }
}
