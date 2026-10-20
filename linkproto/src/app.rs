//! The requests above the frames, little-endian: hello; files put and got by offset and checked by SHA-256 at the end,
//! an interrupted put resumed; a directory's listing; the log from a cursor; console bytes. `Client` is the master's
//! side, `Files` and `FileService` the slave's. docs/link/protocol.md describes each payload.
use crate::endpoint::{channel, code, kind, Error, Line, Master, Service};
use crate::frame::MAX_PAYLOAD;
use crate::sha256::Sha256;

pub const NAME_MAX: usize = 255;
/// The smallest payload an end agrees to: a stat's answer and a listing's line fit.
pub const MIN_PAYLOAD: usize = 64;
/// The bytes a data request or answer carries besides the data: an offset.
const DATA_HEAD: usize = 8;

/// What an end says of itself in its hello.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hello { pub version: u8, pub role: u8, pub max_payload: u16 }
pub const ROLE_MASTER: u8 = 0;
pub const ROLE_SLAVE: u8 = 1;

fn u64_at(p: &[u8], at: usize) -> Option<u64> { Some(u64::from_le_bytes(p.get(at..at + 8)?.try_into().ok()?)) }
fn u32_at(p: &[u8], at: usize) -> Option<u32> { Some(u32::from_le_bytes(p.get(at..at + 4)?.try_into().ok()?)) }
fn u16_at(p: &[u8], at: usize) -> Option<u16> { Some(u16::from_le_bytes(p.get(at..at + 2)?.try_into().ok()?)) }

/// The master's requests. `max_payload` (MIN_PAYLOAD to MAX_PAYLOAD, set before `hello`) bounds the frames both ends
/// send: smaller frames suit a line that loses bytes or a converter with a small buffer.
pub struct Client<L: Line> { pub master: Master<L>, buf: [u8; MAX_PAYLOAD], pub chunk: usize, pub max_payload: usize }

impl<L: Line> Client<L> {
    pub fn new(line: L) -> Self { Self { master: Master::new(line), buf: [0; MAX_PAYLOAD], chunk: MAX_PAYLOAD - DATA_HEAD, max_payload: MAX_PAYLOAD } }

    /// Hello: the other end's, its name in `theirs` (the length returned), and the chunk both can carry.
    pub fn hello(&mut self, name: &str, theirs: &mut [u8]) -> Result<(Hello, usize), Error> {
        let mut p = [0u8; 4 + NAME_MAX];
        let ours = self.max_payload.clamp(MIN_PAYLOAD, MAX_PAYLOAD);
        p[..4].copy_from_slice(&[crate::frame::VERSION, ROLE_MASTER, ours as u8, (ours >> 8) as u8]);
        let n = name.len().min(NAME_MAX);
        p[4..4 + n].copy_from_slice(&name.as_bytes()[..n]);
        let mut r = [0u8; MAX_PAYLOAD];
        let len = self.master.call(channel::CONTROL, kind::HELLO, &p[..4 + n], &mut r)?;
        if len < 4 { return Err(Error::Protocol); }
        let hello = Hello { version: r[0], role: r[1], max_payload: u16::from_le_bytes([r[2], r[3]]) };
        let max = (hello.max_payload as usize).min(ours);
        if max <= DATA_HEAD { return Err(Error::Protocol); }
        self.chunk = max - DATA_HEAD;
        let n = (len - 4).min(theirs.len());
        theirs[..n].copy_from_slice(&r[4..4 + n]);
        Ok((hello, n))
    }

    /// Puts `size` bytes under `name`, read by `read(offset, out) -> n`: the SHA-256 the slave confirmed, and the offset
    /// it resumed from (what it held of an interrupted put of the same name and size).
    pub fn put(&mut self, name: &str, size: u64, mut read: impl FnMut(u64, &mut [u8]) -> usize) -> Result<([u8; 32], u64), Error> {
        if name.is_empty() || name.len() > NAME_MAX { return Err(Error::TooBig); }
        let mut p = [0u8; 8 + NAME_MAX];
        p[..8].copy_from_slice(&size.to_le_bytes());
        p[8..8 + name.len()].copy_from_slice(name.as_bytes());
        let mut r = [0u8; 64];
        let n = self.master.call(channel::FILES, kind::PUT_OPEN, &p[..8 + name.len()], &mut r)?;
        let resume = u64_at(&r[..n], 0).ok_or(Error::Protocol)?;
        if resume > size { return Err(Error::Protocol); }
        let Self { master, buf, chunk, .. } = self;
        let mut sha = Sha256::new();
        let mut offset = 0u64;
        // What the slave holds already is only hashed here.
        while offset < resume {
            let want = ((resume - offset) as usize).min(*chunk);
            let n = read(offset, &mut buf[..want]).min(want);
            if n == 0 { return Err(Error::Line); }
            sha.update(&buf[..n]);
            offset += n as u64;
        }
        while offset < size {
            let want = ((size - offset) as usize).min(*chunk);
            buf[..DATA_HEAD].copy_from_slice(&offset.to_le_bytes());
            let n = read(offset, &mut buf[DATA_HEAD..DATA_HEAD + want]).min(want);
            if n == 0 { return Err(Error::Line); }
            sha.update(&buf[DATA_HEAD..DATA_HEAD + n]);
            master.call(channel::FILES, kind::PUT_DATA, &buf[..DATA_HEAD + n], &mut r)?;
            offset += n as u64;
        }
        let digest = sha.finish();
        master.call(channel::FILES, kind::PUT_CLOSE, &digest, &mut r)?;
        Ok((digest, resume))
    }

    /// A stored file's size and SHA-256, so a put of the same bytes can be skipped.
    pub fn stat(&mut self, name: &str) -> Result<(u64, [u8; 32]), Error> {
        let mut r = [0u8; 64];
        let n = self.master.call(channel::FILES, kind::STAT, name.as_bytes(), &mut r)?;
        if n != 40 { return Err(Error::Protocol); }
        Ok((u64_at(&r, 0).ok_or(Error::Protocol)?, r[8..40].try_into().map_err(|_| Error::Protocol)?))
    }

    pub fn delete(&mut self, name: &str) -> Result<(), Error> {
        self.master.call(channel::FILES, kind::DELETE, name.as_bytes(), &mut [0u8; 8]).map(|_| ())
    }

    /// Gets `name`, each chunk given to `write(offset, data)`; its size and SHA-256, checked against the slave's.
    pub fn get(&mut self, name: &str, mut write: impl FnMut(u64, &[u8]) -> bool) -> Result<(u64, [u8; 32]), Error> {
        let mut r = [0u8; MAX_PAYLOAD];
        let n = self.master.call(channel::FILES, kind::GET_OPEN, name.as_bytes(), &mut r)?;
        let size = u64_at(&r[..n], 0).ok_or(Error::Protocol)?;
        let mut sha = Sha256::new();
        let mut offset = 0u64;
        while offset < size {
            let want = ((size - offset) as usize).min(self.chunk) as u16;
            let mut q = [0u8; 10];
            q[..8].copy_from_slice(&offset.to_le_bytes());
            q[8..].copy_from_slice(&want.to_le_bytes());
            let n = self.master.call(channel::FILES, kind::GET_DATA, &q, &mut r)?;
            if n == 0 || n > want as usize { return Err(Error::Protocol); }
            sha.update(&r[..n]);
            if !write(offset, &r[..n]) { return Err(Error::Line); }
            offset += n as u64;
        }
        let n = self.master.call(channel::FILES, kind::GET_CLOSE, &[], &mut r)?;
        let digest = sha.finish();
        if n != 32 || r[..32] != digest { return Err(Error::Refused(code::MISMATCH)); }
        Ok((size, digest))
    }

    /// One page of the listing from `cursor`: "name\tsize\n" lines into `out`; the next cursor, None at the end.
    pub fn list(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), Error> {
        let mut r = [0u8; MAX_PAYLOAD];
        let n = self.master.call(channel::FILES, kind::LIST, &cursor.to_le_bytes(), &mut r)?;
        let next = u32_at(&r[..n], 0).ok_or(Error::Protocol)?;
        let text = &r[4..n];
        out.get_mut(..text.len()).ok_or(Error::TooBig)?.copy_from_slice(text);
        Ok((text.len(), (next != u32::MAX).then_some(next)))
    }

    /// The log from `cursor` into `out`: its length and the next cursor.
    pub fn log(&mut self, cursor: u64, out: &mut [u8]) -> Result<(usize, u64), Error> {
        let mut r = [0u8; MAX_PAYLOAD];
        let n = self.master.call(channel::LOG, kind::LOG, &cursor.to_le_bytes(), &mut r)?;
        let next = u64_at(&r[..n], 0).ok_or(Error::Protocol)?;
        let text = &r[8..n];
        out.get_mut(..text.len()).ok_or(Error::TooBig)?.copy_from_slice(text);
        Ok((text.len(), next))
    }

    /// Console bytes to the other end, its console's output since the last call back into `out`.
    pub fn console(&mut self, input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
        let mut r = [0u8; MAX_PAYLOAD];
        let n = self.master.call(channel::CONSOLE, kind::CONSOLE, input, &mut r)?;
        out.get_mut(..n).ok_or(Error::TooBig)?.copy_from_slice(&r[..n]);
        Ok(n)
    }
}

/// What the slave's end keeps: a directory of files, a log, a console. Each refuses with a `code`.
pub trait Files {
    /// Starts `name` afresh, dropping any put left unfinished.
    fn create(&mut self, name: &str, size: u64) -> Result<(), u8>;
    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), u8>;
    /// The file written whole and its SHA-256 matching `digest`: kept; otherwise removed.
    fn commit(&mut self, digest: &[u8; 32], matches: bool) -> Result<(), u8>;
    fn open(&mut self, name: &str) -> Result<u64, u8>;
    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> Result<usize, u8>;
    /// "name\tsize\n" lines from entry `cursor` into `out`; the next cursor, None at the end.
    fn list(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8>;
    fn stat(&mut self, _name: &str) -> Result<(u64, [u8; 32]), u8> { Err(code::UNSUPPORTED) }
    fn delete(&mut self, _name: &str) -> Result<(), u8> { Err(code::UNSUPPORTED) }
    fn log(&mut self, _cursor: u64, _out: &mut [u8]) -> Result<(usize, u64), u8> { Err(code::UNSUPPORTED) }
    fn console(&mut self, _input: &[u8], _out: &mut [u8]) -> Result<usize, u8> { Err(code::UNSUPPORTED) }
}

/// A store kept elsewhere (a static, so a large buffer is off the stack).
impl<F: Files + ?Sized> Files for &mut F {
    fn create(&mut self, name: &str, size: u64) -> Result<(), u8> { (**self).create(name, size) }
    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), u8> { (**self).write_at(offset, data) }
    fn commit(&mut self, digest: &[u8; 32], matches: bool) -> Result<(), u8> { (**self).commit(digest, matches) }
    fn open(&mut self, name: &str) -> Result<u64, u8> { (**self).open(name) }
    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> Result<usize, u8> { (**self).read_at(offset, out) }
    fn list(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> { (**self).list(cursor, out) }
    fn stat(&mut self, name: &str) -> Result<(u64, [u8; 32]), u8> { (**self).stat(name) }
    fn delete(&mut self, name: &str) -> Result<(), u8> { (**self).delete(name) }
    fn log(&mut self, cursor: u64, out: &mut [u8]) -> Result<(usize, u64), u8> { (**self).log(cursor, out) }
    fn console(&mut self, input: &[u8], out: &mut [u8]) -> Result<usize, u8> { (**self).console(input, out) }
}

/// A file's name as the slave takes it: one component, printable, none of `/ \ :` and not `.` or `..`.
pub fn plain_name(raw: &[u8]) -> Option<&str> {
    let name = core::str::from_utf8(raw).ok()?;
    let bad = |c: char| c.is_control() || matches!(c, '/' | '\\' | ':' | '\0');
    (!name.is_empty() && name.len() <= NAME_MAX && name != "." && name != ".." && !name.chars().any(bad)).then_some(name)
}

/// A put in progress: its name, size, the bytes written and their hash.
struct Put { name: [u8; NAME_MAX], name_len: usize, size: u64, done: u64, sha: Sha256 }

/// The slave's side of the requests over `Files`: SHA-256 of what is written, in order, and of what is read. A put
/// left unfinished is resumed by the next put of the same name and size.
pub struct FileService<F: Files> {
    pub files: F,
    writing: Option<Put>,
    reading: Option<(u64, u64, Sha256)>,
    /// The largest answer: the master's `max_payload` from its hello.
    limit: usize,
    /// The name this end gives in its hello.
    name: [u8; 64],
    name_len: usize,
}

impl<F: Files> FileService<F> {
    pub fn new(files: F) -> Self { Self { files, writing: None, reading: None, limit: MAX_PAYLOAD, name: [0; 64], name_len: 0 } }

    pub fn set_name(&mut self, name: &str) {
        self.name_len = name.len().min(self.name.len());
        self.name[..self.name_len].copy_from_slice(&name.as_bytes()[..self.name_len]);
    }
}

impl<F: Files> Service for FileService<F> {
    fn handle(&mut self, ch: u8, k: u8, p: &[u8], reply: &mut [u8]) -> Result<usize, u8> {
        let limit = self.limit.min(reply.len());
        let reply = &mut reply[..limit];
        match (ch, k) {
            (channel::CONTROL, kind::POLL) => Ok(0),
            (channel::CONTROL, kind::HELLO) => {
                let theirs = u16_at(p, 2).ok_or(code::BAD_REQUEST)? as usize;
                self.limit = theirs.clamp(MIN_PAYLOAD, MAX_PAYLOAD);
                reply[..4].copy_from_slice(&[crate::frame::VERSION, ROLE_SLAVE, MAX_PAYLOAD as u8, (MAX_PAYLOAD >> 8) as u8]);
                reply[4..4 + self.name_len].copy_from_slice(&self.name[..self.name_len]);
                Ok(4 + self.name_len)
            }
            (channel::FILES, kind::PUT_OPEN) => {
                let size = u64_at(p, 0).ok_or(code::BAD_REQUEST)?;
                let name = plain_name(&p[8..]).ok_or(code::DENIED)?;
                let same = |w: &Put| w.size == size && &w.name[..w.name_len] == name.as_bytes();
                let resume = match &self.writing {
                    Some(w) if same(w) => w.done,
                    _ => {
                        self.writing = None;
                        self.files.create(name, size)?;
                        let mut put = Put { name: [0; NAME_MAX], name_len: name.len(), size, done: 0, sha: Sha256::new() };
                        put.name[..name.len()].copy_from_slice(name.as_bytes());
                        self.writing = Some(put);
                        0
                    }
                };
                reply[..8].copy_from_slice(&resume.to_le_bytes());
                Ok(8)
            }
            (channel::FILES, kind::PUT_DATA) => {
                let offset = u64_at(p, 0).ok_or(code::BAD_REQUEST)?;
                let data = &p[DATA_HEAD..];
                let w = self.writing.as_mut().ok_or(code::BAD_REQUEST)?;
                if offset != w.done || offset + data.len() as u64 > w.size { return Err(code::BAD_REQUEST); }
                self.files.write_at(offset, data)?;
                w.sha.update(data);
                w.done += data.len() as u64;
                Ok(0)
            }
            (channel::FILES, kind::PUT_CLOSE) => {
                let digest: [u8; 32] = p.try_into().map_err(|_| code::BAD_REQUEST)?;
                let w = self.writing.take().ok_or(code::BAD_REQUEST)?;
                let matches = w.done == w.size && w.sha.finish() == digest;
                self.files.commit(&digest, matches)?;
                if matches { Ok(0) } else { Err(code::MISMATCH) }
            }
            (channel::FILES, kind::GET_OPEN) => {
                let name = plain_name(p).ok_or(code::DENIED)?;
                let size = self.files.open(name)?;
                self.reading = Some((size, 0, Sha256::new()));
                reply[..8].copy_from_slice(&size.to_le_bytes());
                Ok(8)
            }
            (channel::FILES, kind::GET_DATA) => {
                let (offset, want) = (u64_at(p, 0).ok_or(code::BAD_REQUEST)?, u16_at(p, 8).ok_or(code::BAD_REQUEST)? as usize);
                let (size, done, sha) = self.reading.as_mut().ok_or(code::BAD_REQUEST)?;
                if offset != *done || offset >= *size { return Err(code::BAD_REQUEST); }
                let want = want.min(reply.len()).min((*size - offset) as usize);
                let n = self.files.read_at(offset, &mut reply[..want])?;
                sha.update(&reply[..n]);
                *done += n as u64;
                Ok(n)
            }
            (channel::FILES, kind::GET_CLOSE) => {
                let (_, _, sha) = self.reading.take().ok_or(code::BAD_REQUEST)?;
                reply[..32].copy_from_slice(&sha.finish());
                Ok(32)
            }
            (channel::FILES, kind::STAT) => {
                let (size, digest) = self.files.stat(plain_name(p).ok_or(code::DENIED)?)?;
                reply[..8].copy_from_slice(&size.to_le_bytes());
                reply[8..40].copy_from_slice(&digest);
                Ok(40)
            }
            (channel::FILES, kind::DELETE) => self.files.delete(plain_name(p).ok_or(code::DENIED)?).map(|_| 0),
            (channel::FILES, kind::LIST) => {
                let cursor = u32_at(p, 0).ok_or(code::BAD_REQUEST)?;
                let (n, next) = self.files.list(cursor, &mut reply[4..])?;
                reply[..4].copy_from_slice(&next.unwrap_or(u32::MAX).to_le_bytes());
                Ok(4 + n)
            }
            (channel::LOG, kind::LOG) => {
                let cursor = u64_at(p, 0).ok_or(code::BAD_REQUEST)?;
                let (n, next) = self.files.log(cursor, &mut reply[8..])?;
                reply[..8].copy_from_slice(&next.to_le_bytes());
                Ok(8 + n)
            }
            (channel::CONSOLE, kind::CONSOLE) => self.files.console(p, reply),
            _ => Err(code::UNSUPPORTED),
        }
    }
}
