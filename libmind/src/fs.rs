//! vfs_server client: files are opened by path, data goes through the process's shared buffer.
use crate::abi::*;
use crate::ipc::{Endpoint, Message};
use crate::mem::Pages;
use crate::sys::{check, Error, Result};
use core::cell::UnsafeCell;

pub const CHUNK: usize = 4096;

struct Channel { pages: Pages, cap: usize }
struct Shared(UnsafeCell<Option<Channel>>);
unsafe impl Sync for Shared {} // processes are single-threaded
static CHANNEL: Shared = Shared(UnsafeCell::new(None));

// The exchange buffer with the server is created once and passed as a capability in every request.
fn channel() -> Result<&'static mut Channel> {
    let slot = unsafe { &mut *CHANNEL.0.get() };
    if slot.is_none() { let pages = Pages::new(CHUNK).ok_or(Error::NoMemory)?; let cap = pages.share()?; *slot = Some(Channel { pages, cap }); }
    Ok(slot.as_mut().unwrap())
}

/// Creates the vfs_server exchange buffer up front (services do this at startup so they don't grow while running).
pub fn prepare() -> Result<()> { channel().map(drop) }

fn request(op: usize, fd: usize, len: usize, offset: usize) -> Result<[usize; 2]> {
    let channel = channel()?;
    let reply = Endpoint::VFS.call(&Message::new(op | fd << 8 | len << 16, offset).with_cap(channel.cap, 0), 0)?;
    check(reply.data[0])?;
    Ok(reply.data)
}

fn put_path(path: &str) -> Result<usize> {
    if path.len() > 255 { return Err(Error::Invalid); }
    let channel = channel()?; channel.pages.as_mut_slice()[..path.len()].copy_from_slice(path.as_bytes());
    Ok(path.len())
}

/// Open file (the descriptor belongs to the process and is closed in Drop).
pub struct File { fd: usize, size: usize, position: usize }

impl File {
    pub fn open(path: &str) -> Result<Self> {
        if path.len() > 255 { return Err(Error::Invalid); }
        let len = put_path(path)?;
        let [fd, size] = request(VFS_OPEN, 0, len, 0)?;
        Ok(Self { fd, size, position: 0 })
    }
    pub fn size(&self) -> usize { self.size }
    pub fn position(&self) -> usize { self.position }
    pub fn seek(&mut self, position: usize) { self.position = position.min(self.size); }
    /// Reads at `offset` without changing the current position.
    pub fn read_at(&self, offset: usize, buffer: &mut [u8]) -> Result<usize> {
        let mut done = 0;
        while done < buffer.len() {
            let want = (buffer.len() - done).min(CHUNK);
            let [got, _] = request(VFS_READ, self.fd, want, offset + done)?;
            buffer[done..done + got].copy_from_slice(&channel()?.pages.as_slice()[..got]);
            done += got;
            if got < want { break; }
        }
        Ok(done)
    }
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize> {
        let got = self.read_at(self.position, buffer)?; self.position += got; Ok(got)
    }
}

impl Drop for File { fn drop(&mut self) { let _ = request(VFS_CLOSE, self.fd, 0, 0); } }

/// Directory entry.
pub struct DirEntry<'a> { pub name: &'a [u8], pub size: u32, pub is_dir: bool }

/// Iterates a directory (`""` or `"/"` is the root); returns the number of entries.
pub fn list(path: &str, mut visit: impl FnMut(&DirEntry)) -> Result<usize> {
    let mut index = 0; let mut total = 0;
    loop {
        let len = if path.is_empty() { 0 } else { put_path(path)? };
        let [count, next] = request(VFS_LIST, 0, len, index)?;
        let buffer = channel()?.pages.as_slice(); let mut at = 0;
        for _ in 0..count {
            let size = u32::from_le_bytes(buffer[at..at + 4].try_into().unwrap());
            let (flags, name_len) = (buffer[at + 4], buffer[at + 5] as usize);
            visit(&DirEntry { name: &buffer[at + 6..at + 6 + name_len], size, is_dir: flags & 1 != 0 });
            at += 6 + name_len;
        }
        total += count;
        if next == 0 { return Ok(total); }
        index = next;
    }
}
