//! vfs_server client (idl/vfs.wit): files are opened by path; each request carries its data in a buffer the server
//! can use only during the call.
use crate::idl::vfs;
use crate::ipc::Endpoint;
use crate::sys::{Error, Result};

pub const CHUNK: usize = 4096;

/// Kept for callers that prepared the old shared buffer at startup; buffers are now per request.
pub fn prepare() -> Result<()> { Ok(()) }

/// Open file (the descriptor belongs to the process and is closed in Drop).
pub struct File { fd: u32, size: usize, position: usize }

impl File {
    pub fn open(path: &str) -> Result<Self> {
        if path.len() > 255 { return Err(Error::Invalid); }
        let file = vfs::open(Endpoint::VFS, path)?;
        Ok(Self { fd: file.fd, size: file.size as usize, position: 0 })
    }
    pub fn size(&self) -> usize { self.size }
    pub fn position(&self) -> usize { self.position }
    pub fn seek(&mut self, position: usize) { self.position = position.min(self.size); }
    /// Reads at `offset` without changing the current position.
    pub fn read_at(&self, offset: usize, buffer: &mut [u8]) -> Result<usize> {
        let mut done = 0;
        while done < buffer.len() {
            let want = (buffer.len() - done).min(CHUNK);
            let data = vfs::read(Endpoint::VFS, self.fd, (offset + done) as u64, want as u16)?;
            let got = data.len().min(want);
            buffer[done..done + got].copy_from_slice(&data.as_slice()[..got]);
            done += got;
            if got < want { break; }
        }
        Ok(done)
    }
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize> {
        let got = self.read_at(self.position, buffer)?; self.position += got; Ok(got)
    }
}

impl Drop for File { fn drop(&mut self) { let _ = vfs::close(Endpoint::VFS, self.fd); } }

/// Directory entry.
pub struct DirEntry<'a> { pub name: &'a [u8], pub size: u32, pub is_dir: bool }

/// Iterates a directory (`""` or `"/"` is the root); returns the number of entries.
pub fn list(path: &str, mut visit: impl FnMut(&DirEntry)) -> Result<usize> {
    let (mut start, mut total) = (0, 0);
    loop {
        let page = vfs::list(Endpoint::VFS, path, start)?;
        for entry in page.entries.as_slice() { visit(&DirEntry { name: entry.name.as_str().as_bytes(), size: entry.size, is_dir: entry.directory }); }
        total += page.entries.len();
        if page.next == 0 { return Ok(total); }
        start = page.next;
    }
}
