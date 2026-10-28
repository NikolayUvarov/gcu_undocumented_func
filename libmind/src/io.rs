//! A program's text input (000-KRN-0094, ABI 9): the bytes its launcher writes, read to their end — what a shell's `|`
//! or `<` gives a program. A program started without one gets `Error::NotFound` from `read`, and keys stay its input
//! (`mind::input`). The launcher holds a `Writer`, which `commit-with-input` in idl/loader.wit gives it. `Lines` makes
//! no system calls of its own: tests/io_host.rs.

/// The longest line `Lines` gives whole; a longer one comes in pieces of this size.
pub const LINE_MAX: usize = 4096;

/// The lines of a stream of bytes, without their ends (`\n`, or `\r\n`); the last one may lack its end.
pub struct Lines { buffer: [u8; LINE_MAX], start: usize, end: usize, done: bool }

impl Default for Lines { fn default() -> Self { Self::new() } }

impl Lines {
    pub const fn new() -> Self { Self { buffer: [0; LINE_MAX], start: 0, end: 0, done: false } }

    /// The next line, its bytes taken from `read` (0 from it: the end); None at the end.
    pub fn next_from<E>(&mut self, mut read: impl FnMut(&mut [u8]) -> Result<usize, E>) -> Result<Option<&[u8]>, E> {
        loop {
            if let Some(at) = self.buffer[self.start..self.end].iter().position(|&b| b == b'\n') {
                let (from, to) = (self.start, self.start + at);
                self.start = to + 1;
                let line = &self.buffer[from..to];
                return Ok(Some(line.strip_suffix(b"\r").unwrap_or(line)));
            }
            if self.done || self.end - self.start == LINE_MAX {
                if self.start == self.end { return Ok(None); }
                let from = self.start;
                self.start = self.end;
                return Ok(Some(&self.buffer[from..self.end]));
            }
            // What is left moves to the front, and the input fills the room behind it.
            self.buffer.copy_within(self.start..self.end, 0);
            self.end -= self.start;
            self.start = 0;
            match read(&mut self.buffer[self.end..])? { 0 => self.done = true, n => self.end += n }
        }
    }

    /// The next line of this program's text input; None at its end.
    #[cfg(target_os = "none")]
    pub fn next(&mut self) -> crate::sys::Result<Option<&[u8]>> { self.next_from(read) }
}

#[cfg(target_os = "none")]
mod system {
    use crate::abi::*;
    use crate::sys::{call, check, syscall, Error, Result};

    /// Whether this program was started with a text input.
    pub fn has_input() -> bool { call(SYSCALL_TEXT_READ, 0, 0) != ERR_NOT_FOUND }

    /// Up to `buffer.len()` bytes of the input, waiting for them: 0 at its end. `Error::NotFound` without one.
    pub fn read(buffer: &mut [u8]) -> Result<usize> {
        loop { if let Some(n) = read_ready(buffer)? { return Ok(n); } }
    }

    /// `read` that waits at most `TEXT_WAIT_MS`, or until a key or other input event comes: None when no byte came.
    pub fn read_ready(buffer: &mut [u8]) -> Result<Option<usize>> {
        if buffer.is_empty() { return Ok(Some(0)); }
        match check(call(SYSCALL_TEXT_READ, buffer.as_mut_ptr() as usize, buffer.len())) {
            Err(Error::Other(ERR_BUSY)) => Ok(None),
            other => other.map(Some),
        }
    }

    /// The whole input, to its end, appended to `out`; how many bytes.
    #[cfg(feature = "alloc")]
    pub fn read_to_end(out: &mut alloc::vec::Vec<u8>) -> Result<usize> {
        let mut chunk = [0u8; TEXT_QUEUE];
        let before = out.len();
        loop {
            match read(&mut chunk)? { 0 => return Ok(out.len() - before), n => out.extend_from_slice(&chunk[..n]) }
        }
    }

    /// The writer of another program's text input (a `CAP_KIND_TEXT` capability). Dropping it drops the capability;
    /// once no copy is left, the reader comes to the end after the bytes already queued.
    pub struct Writer(usize);

    impl Writer {
        /// Takes the handle of a writer (from `commit-with-input` or `mind::process::spawn_fed`); None for another kind.
        pub fn new(handle: usize) -> Option<Self> { (crate::dev::cap_info(handle).0 == CAP_KIND_TEXT).then_some(Self(handle)) }

        /// The PID of the program that reads it.
        pub fn reader(&self) -> u64 { crate::dev::cap_info(self.0).1 as u64 }

        pub fn handle(&self) -> usize { self.0 }

        /// Queues what fits of `bytes` (at most `TEXT_QUEUE` wait to be read): how many, 0 while the queue is full.
        /// `Error::Peer` once the reader ended.
        pub fn write_some(&self, bytes: &[u8]) -> Result<usize> { send(self.0, bytes, 0) }

        /// Queues all of `bytes`, waiting while the queue is full; `Error::Peer` once the reader ended.
        pub fn write_all(&self, mut bytes: &[u8]) -> Result<()> {
            while !bytes.is_empty() {
                match self.write_some(bytes)? { 0 => { crate::time::sleep(1); } n => bytes = &bytes[n..] }
            }
            Ok(())
        }

        /// Ends the input after what is queued: the reader reads it, then the end.
        pub fn close(self) -> Result<()> { send(self.0, &[], TEXT_CLOSE).map(drop) }
    }

    impl Drop for Writer { fn drop(&mut self) { let _ = crate::ipc::drop_cap(self.0); } }

    fn send(handle: usize, bytes: &[u8], flags: usize) -> Result<usize> {
        let bytes = &bytes[..bytes.len().min(TEXT_QUEUE)];
        check(syscall(SYSCALL_TEXT_WRITE, handle, flags, [bytes.as_ptr() as usize, bytes.len(), 0, 0]).result)
    }
}
#[cfg(target_os = "none")]
pub use system::*;
