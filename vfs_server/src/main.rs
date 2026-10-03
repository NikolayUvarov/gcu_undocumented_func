#![no_std]
#![no_main]
// vfs_server: изолированный процесс, который владеет диском (ATA PIO), разбирает FAT
// и выдаёт клиентам дескрипторы, привязанные к их PID. Данные идут через буфер клиента.
mod ata;
mod fat;

use mind::abi::*;
use mind::ipc::{self, Endpoint, Message};
use mind::mem::Mapping;

const RECEIVED_CAP: usize = 9;
const MAX_OPEN: usize = 32;

#[derive(Clone, Copy)]
struct Open { owner: u64, file: fat::Node, cursor: Option<(usize, u32)> } // cursor: (номер кластера в цепочке, кластер)

struct Server { volume: Option<fat::Volume>, open: [Option<Open>; MAX_OPEN] }

impl Server {
    // Дескрипторы умерших клиентов освобождаются, когда таблица заполнена.
    fn allocate(&mut self, open: Open) -> Option<usize> {
        if self.open.iter().all(Option::is_some) {
            for slot in self.open.iter_mut() { if slot.is_some_and(|o| !mind::process::alive(o.owner)) { *slot = None; } }
        }
        let fd = self.open.iter().position(Option::is_none)?;
        self.open[fd] = Some(open);
        Some(fd)
    }

    fn handle(&mut self, op: usize, fd: usize, len: usize, offset: usize, sender: u64, buffer: Option<&mut [u8]>) -> Result<[usize; 2], usize> {
        let volume = self.volume.as_mut().ok_or(ERR_NOT_FOUND)?;
        let owned = |open: &Option<Open>| open.filter(|o| o.owner == sender);
        match op {
            VFS_OPEN => {
                let buffer = buffer.ok_or(ERR_INVALID)?;
                let path = buffer.get(..len).ok_or(ERR_INVALID)?;
                let node = volume.resolve(path).ok_or(ERR_NOT_FOUND)?;
                if node.is_dir { return Err(ERR_INVALID); }
                let fd = self.allocate(Open { owner: sender, file: node, cursor: None }).ok_or(ERR_NO_SLOT)?;
                Ok([fd, node.size as usize])
            }
            VFS_READ => {
                let buffer = buffer.ok_or(ERR_INVALID)?;
                let mut open = self.open.get(fd).and_then(owned).ok_or(ERR_INVALID)?;
                let want = len.min(buffer.len());
                let got = volume.read(&open.file, offset, &mut buffer[..want], &mut open.cursor);
                self.open[fd] = Some(open);
                Ok([got, 0])
            }
            VFS_STAT => self.open.get(fd).and_then(owned).map(|o| [o.file.size as usize, 0]).ok_or(ERR_INVALID),
            VFS_CLOSE => { self.open.get(fd).and_then(owned).ok_or(ERR_INVALID)?; self.open[fd] = None; Ok([0, 0]) }
            VFS_LIST => {
                let buffer = buffer.ok_or(ERR_INVALID)?;
                let path = buffer.get(..len).ok_or(ERR_INVALID)?;
                let mut name = [0u8; 255]; let path_len = path.len(); name[..path_len].copy_from_slice(path);
                let dir = volume.resolve(&name[..path_len]).ok_or(ERR_NOT_FOUND)?;
                if !dir.is_dir { return Err(ERR_INVALID); }
                // Записи: размер u32, флаги u8 (1 = каталог), длина имени u8, имя.
                let (mut index, mut count, mut at, mut more) = (0usize, 0usize, 0usize, false);
                volume.walk(&dir, |entry| {
                    if index < offset { index += 1; return true; }
                    let need = 6 + entry.name.len();
                    if at + need > buffer.len() { more = true; return false; }
                    buffer[at..at + 4].copy_from_slice(&entry.node.size.to_le_bytes()); buffer[at + 4] = entry.node.is_dir as u8; buffer[at + 5] = entry.name.len() as u8;
                    buffer[at + 6..at + need].copy_from_slice(entry.name); at += need; count += 1; index += 1; true
                });
                Ok([count, if more { index } else { 0 }])
            }
            _ => Err(ERR_INVALID),
        }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let volume = ata::Disk::probe().and_then(fat::Volume::mount);
    match &volume {
        Some(v) => mind::println!("[VFS] ATA DISK MOUNTED: FAT{} AT LBA {}", v.bits(), v.start()),
        None => mind::println!("[VFS] NO ATA FAT DISK; REQUESTS WILL FAIL"),
    }
    let mut server = Server { volume, open: [None; MAX_OPEN] };
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        if !request.is_call { if request.cap_received { let _ = ipc::drop_cap(RECEIVED_CAP); } continue; }
        let (op, fd, len) = (request.data[0] & 0xFF, (request.data[0] >> 8) & 0xFF, request.data[0] >> 16);
        let mut mapping = if request.cap_received { Mapping::new(RECEIVED_CAP).ok() } else { None };
        let result = server.handle(op, fd, len, request.data[1], request.sender, mapping.as_mut().map(|m| m.as_mut_slice()));
        drop(mapping);
        if request.cap_received { let _ = ipc::drop_cap(RECEIVED_CAP); }
        let reply = match result { Ok(data) => data, Err(error) => [error, 0] };
        let _ = ipc::reply(&Message::new(reply[0], reply[1]));
    }
}
