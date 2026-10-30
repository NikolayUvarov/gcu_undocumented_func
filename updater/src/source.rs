// Where releases come from (351-UPD-0007): an HTTPS server, trusted by its pinned key or the system's roots, over the
// flow grant the network policy gives the updater; a directory on a mounted volume (a USB stick); or a bundle the
// Effector gateway handed over (751-UPD-0032). All read the layout of scripts/release.py, and nothing read here is
// trusted until the updater has checked it.
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{ERR_TIMEOUT, SLOT_NETWORK, SLOT_TLS};
use mind::fs::{self, File};
use mind::http::{self, Head, Parser as _, Sink, Transport, Url};
use mind::idl::{socket, tls, wire};
use mind::ipc::Endpoint;
use mind::parse::Session as Parser;
use mind::idl::update::Error;
use crate::bundle;

const STACK: Endpoint = Endpoint(SLOT_NETWORK);
const TLS: Endpoint = Endpoint(SLOT_TLS);
const IDLE_MS: usize = 15_000; // no progress for this long: the connection counts as cut
const TRIES: u32 = 5; // connections per file before giving up
const STAGE: usize = 64 * 1024; // a file is written to the slot in pieces of this size
// No call to the stack, the TLS service or a parser waits longer, so a stalled connection ends in another try or an
// error rather than a silence (351-UPD-0018, as download does since 351-NET-0029). Above the services' own 10 s.
const CALL_MS: u32 = 60_000;

pub enum Source { Https { base: String, pin: Option<[u8; 32]> }, Directory(String), Bundle(Bundle) }

/// A bundle in the updater's staging: its file, and each entry's name, start and size in it.
pub struct Bundle { file: File, entries: Vec<(String, u64, u64)> }

impl Bundle {
    /// The bundle at `path`, its directory and every local header read and checked (`bundle`): else `malformed`.
    pub fn open(path: &str) -> Result<Self, Error> {
        let file = File::open(path).map_err(|_| Error::Write)?;
        let len = file.size() as u64;
        let read = |at: u64, size: usize| -> Result<Vec<u8>, Error> {
            let mut data = alloc::vec![0u8; size];
            if file.read_at(at as usize, &mut data) != Ok(size) { return Err(Error::Malformed); }
            Ok(data)
        };
        let tail_len = (len as usize).min(bundle::TAIL_MAX);
        let (directory, size, count) = bundle::end(&read(len - tail_len as u64, tail_len)?, len).ok_or(Error::Malformed)?;
        let entries = bundle::entries(&read(directory, size)?, count, directory).ok_or(Error::Malformed)?;
        let mut ranges = Vec::with_capacity(entries.len());
        for entry in entries {
            let header = read(entry.local, bundle::LOCAL_LEN + entry.name.len())?;
            let start = bundle::start(&header, &entry, directory).ok_or(Error::Malformed)?;
            ranges.push((entry.name, start, entry.size));
        }
        Ok(Self { file, entries: ranges })
    }

    pub fn len(&self) -> usize { self.entries.len() }

    fn find(&self, path: &str) -> Result<(u64, u64), Error> {
        self.entries.iter().find(|(name, _, _)| name == path).map(|&(_, start, size)| (start, size)).ok_or(Error::Missing)
    }
}

fn now() -> usize { mind::time::uptime_ms() }

fn timed_out<T>(result: &mind::sys::Result<T>) -> bool { matches!(result, Err(mind::sys::Error::Other(ERR_TIMEOUT))) }

// A TLS session of the TLS service over the flow grant.
struct Session(u32);

impl Transport for Session {
    fn send(&mut self, data: &[u8]) -> Result<(), http::Error> {
        let (mut sent, deadline) = (0, now() + IDLE_MS);
        while sent < data.len() {
            let end = data.len().min(sent + 4096);
            match tls::send(TLS, self.0, &data[sent..end]) {
                Ok(Ok(n)) if n > 0 => sent += n as usize,
                Ok(Ok(_)) | Ok(Err(tls::Error::Again)) if now() < deadline => { mind::time::sleep(5); }
                _ => return Err(http::Error::Transport),
            }
        }
        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, http::Error> {
        let deadline = now() + IDLE_MS;
        let length = buffer.len().min(4096);
        loop {
            match tls::receive(TLS, self.0, length as u32, &mut buffer[..length]) {
                Ok(Ok(n)) if n > 0 => return Ok(n),
                Ok(Err(tls::Error::Closed)) => return Ok(0),
                Ok(Ok(_)) | Ok(Err(tls::Error::Again)) if now() < deadline => { mind::time::sleep(2); }
                _ => return Err(http::Error::Transport),
            }
        }
    }
}

impl Drop for Session { fn drop(&mut self) { let _ = tls::close(TLS, self.0); } }

fn ipv4(text: &str) -> Option<u32> {
    let mut address = [0u8; 4]; let mut parts = text.split('.');
    for byte in address.iter_mut() { *byte = parts.next()?.parse().ok()?; }
    parts.next().is_none().then_some(u32::from_be_bytes(address))
}

// A session to the server, verified by its key or the roots; None when it is worth another try.
fn connect(url: &Url, pin: Option<&[u8; 32]>) -> Result<Option<Session>, Error> {
    let address = match ipv4(url.host) {
        Some(address) => address,
        None => match socket::resolve(STACK, url.host, 0, 0, 5000) { Ok(Ok(address)) => address, _ => return Ok(None) },
    };
    // The TLS service gets a child of the flow grant: the policy that applies to the updater applies to the session.
    let attached = tls::attach(TLS, SLOT_NETWORK);
    if timed_out(&attached) { mind::println!("[UPDATER] TLS: NO ANSWER IN {} S", CALL_MS / 1000); return Ok(None); }
    let session = match attached { Ok(Ok(session)) => Session(session), _ => return Err(Error::Network) };
    let connected = match pin {
        Some(pin) => tls::connect_pinned(TLS, session.0, url.host, address, url.port, pin, 10_000),
        None => tls::connect(TLS, session.0, url.host, address, url.port, false, 10_000),
    };
    if timed_out(&connected) { mind::println!("[UPDATER] TLS: NO ANSWER IN {} S", CALL_MS / 1000); return Ok(None); }
    match connected {
        Ok(Ok(_)) => Ok(Some(session)),
        Ok(Err(tls::Error::Refused | tls::Error::Timeout | tls::Error::Closed | tls::Error::NoNetwork)) => Ok(None),
        // A server that is not the one trusted: asking again changes nothing.
        Ok(Err(error)) => { mind::println!("[UPDATER] TLS: {:?}", error); Err(Error::Network) }
        Err(_) => Err(Error::Network),
    }
}

// A resume's request and its head in the log, so that a stall shows the step it stopped at.
struct Traced<'a, T> { inner: &'a mut T, path: &'a str, offset: u64, on: bool }

impl<T: Transport> Transport for Traced<'_, T> {
    fn send(&mut self, data: &[u8]) -> Result<(), http::Error> {
        let sent = self.inner.send(data);
        if core::mem::take(&mut self.on) { mind::println!("[UPDATER] {}: ASKED FOR THE REST FROM BYTE {}{}", self.path, self.offset, if sent.is_ok() { "" } else { ": NOT SENT" }); }
        sent
    }
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, http::Error> { self.inner.receive(buffer) }
}

impl Traced<'_, Parser> {
    fn parse(&mut self, head: &[u8]) -> Result<Head, http::Error> {
        let parsed = self.inner.head(head);
        if self.on {
            match &parsed { Ok(h) => mind::println!("[UPDATER] {}: ITS HEAD PARSED: STATUS {}", self.path, h.status), Err(e) => mind::println!("[UPDATER] {}: ITS HEAD NOT PARSED: {:?}", self.path, e) }
        }
        parsed
    }
}

impl http::Parser for Traced<'_, Parser> {
    fn head(&mut self, head: &[u8]) -> Result<Head, http::Error> { self.parse(head) }
}

// A body kept in memory, up to `limit` bytes.
struct Memory { data: Vec<u8>, limit: usize }

impl Sink for Memory {
    fn begin(&mut self, start: u64, total: u64) -> Result<(), http::Error> {
        if total > self.limit as u64 { return Err(http::Error::Sink); }
        self.data.truncate(start as usize);
        Ok(())
    }
    fn write(&mut self, offset: u64, data: &[u8]) -> Result<(), http::Error> {
        if offset != self.data.len() as u64 { return Err(http::Error::Sink); }
        self.data.extend_from_slice(data);
        Ok(())
    }
}

// A body written into a file of the slot, `size` bytes at most, in pieces of STAGE bytes.
struct Output<'f> { file: &'f mut File, size: u64, stage: Vec<u8>, staged: usize, at: u64, failed: bool }

impl Output<'_> {
    fn flush(&mut self) -> Result<(), http::Error> {
        if self.staged > 0 {
            // The slot's writes keep no limit: CALL_MS is for the network's side.
            let written = wire::with_timeout(0, || self.file.write_at(self.at as usize, &self.stage[..self.staged]));
            if written != Ok(self.staged) { self.failed = true; return Err(http::Error::Sink); }
            self.at += self.staged as u64;
            self.staged = 0;
        }
        Ok(())
    }
}

impl Sink for Output<'_> {
    fn begin(&mut self, start: u64, total: u64) -> Result<(), http::Error> {
        self.flush()?;
        if total != self.size { return Err(http::Error::Sink); }
        // The server sends the whole file again: what was there goes.
        if (self.file.size() as u64) > start && self.file.truncate(start as usize).is_err() { self.failed = true; return Err(http::Error::Sink); }
        self.at = start;
        Ok(())
    }
    fn write(&mut self, offset: u64, mut data: &[u8]) -> Result<(), http::Error> {
        if offset != self.at + self.staged as u64 { return Err(http::Error::Sink); }
        while !data.is_empty() {
            let take = data.len().min(STAGE - self.staged);
            self.stage[self.staged..self.staged + take].copy_from_slice(&data[..take]);
            self.staged += take;
            data = &data[take..];
            if self.staged == STAGE { self.flush()?; }
        }
        Ok(())
    }
}

// One GET with its retries: a cut connection resumes where the body got to.
// Each head parsed by the check's own parser (109-NET-0010), started again if it ended meanwhile.
fn get(base: &str, pin: Option<&[u8; 32]>, path: &str, mut offset: u64, sink: &mut impl Sink, parser: &mut Parser) -> Result<u64, Error> {
    let address = format!("{}/{}", base.trim_end_matches('/'), path);
    let url = Url::parse(&address).map_err(|_| Error::NoSource)?;
    for connection in 1..=TRIES {
        // Every call of this attempt is bounded (CALL_MS), the closing of its session too.
        let attempt = wire::with_timeout(CALL_MS, || -> Result<Option<Result<http::Got, http::Error>>, Error> {
            let Some(mut session) = connect(&url, pin)? else { return Ok(None) };
            let resuming = offset > 0 || connection > 1;
            let mut transport = Traced { inner: &mut session, path, offset, on: resuming };
            let mut parsing = Traced { inner: &mut *parser, path, offset, on: resuming };
            Ok(Some(http::get(&mut transport, &url, offset, sink, &mut parsing)))
        })?;
        let Some(result) = attempt else { mind::time::sleep(1000); continue };
        match result {
            Ok(got) if got.complete() => return Ok(got.total),
            Ok(got) => { mind::println!("[UPDATER] {}: CONNECTION CUT AT {} OF {}, RESUMING", path, got.end, got.total); offset = got.end; }
            Err(http::Error::Transport) => mind::println!("[UPDATER] {}: NO ANSWER, TRYING AGAIN ({} OF {})", path, connection, TRIES),
            Err(http::Error::Status(404)) => return Err(Error::Missing),
            Err(http::Error::Parser) => return Err(Error::NoParser),
            Err(http::Error::Sink) => return Err(Error::Digest),
            Err(error) => { mind::println!("[UPDATER] {}: HTTP {:?}", path, error); return Err(Error::Network); }
        }
    }
    Err(Error::Network)
}

impl Source {
    /// A file of the release layout, `limit` bytes at most, read whole into memory.
    pub fn small(&self, path: &str, limit: usize, parser: &mut Parser) -> Result<Vec<u8>, Error> {
        match self {
            Source::Https { base, pin } => {
                let mut memory = Memory { data: Vec::new(), limit };
                get(base, pin.as_ref(), path, 0, &mut memory, parser)?;
                Ok(memory.data)
            }
            Source::Directory(dir) => {
                let file = File::open(&format!("{}/{}", dir, path)).map_err(|_| Error::Missing)?;
                if file.size() > limit { return Err(Error::Digest); }
                let mut data = alloc::vec![0u8; file.size()];
                if file.read_at(0, &mut data) != Ok(data.len()) { return Err(Error::Network); }
                Ok(data)
            }
            Source::Bundle(bundle) => {
                let (start, size) = bundle.find(path)?;
                if size > limit as u64 { return Err(Error::Digest); }
                let mut data = alloc::vec![0u8; size as usize];
                if bundle.file.read_at(start as usize, &mut data) != Ok(data.len()) { return Err(Error::Write); }
                Ok(data)
            }
        }
    }

    /// File `path` of the layout into `file`, which is to hold `size` bytes: what it holds already is kept and the rest
    /// fetched after it. Returns the bytes fetched.
    pub fn file(&self, path: &str, file: &mut File, size: u64, parser: &mut Parser) -> Result<u64, Error> {
        let have = file.size() as u64;
        if have > size { file.truncate(0).map_err(|_| Error::Write)?; return self.file(path, file, size, parser); }
        if have == size { return Ok(0); }
        if have > 0 { mind::println!("[UPDATER] {}: RESUMING AT {} OF {}", path, have, size); }
        match self {
            Source::Https { base, pin } => {
                let mut output = Output { file, size, stage: alloc::vec![0; STAGE], staged: 0, at: have, failed: false };
                let result = get(base, pin.as_ref(), path, have, &mut output, parser);
                let flushed = output.flush();
                if output.failed || flushed.is_err() { return Err(Error::Write); }
                result.map(|_| size - have)
            }
            Source::Directory(dir) => {
                let from = File::open(&format!("{}/{}", dir, path)).map_err(|_| Error::Missing)?;
                if from.size() as u64 != size { return Err(Error::Digest); }
                copy(&from, file, have as usize, size as usize)?;
                Ok(size - have)
            }
            Source::Bundle(bundle) => {
                let (start, length) = bundle.find(path)?;
                if length != size { return Err(Error::Digest); }
                copy_from(&bundle.file, start as usize, file, have as usize, size as usize)?;
                Ok(size - have)
            }
        }
    }
}

/// Bytes `at..size` of `from` into the same place of `file`.
pub fn copy(from: &File, file: &mut File, at: usize, size: usize) -> Result<(), Error> { copy_from(from, 0, file, at, size) }

/// Bytes `at..size` of `from`, read `base` bytes further on, into `file` at `at..size`.
fn copy_from(from: &File, base: usize, file: &mut File, mut at: usize, size: usize) -> Result<(), Error> {
    let mut buffer = alloc::vec![0u8; STAGE];
    while at < size {
        let got = from.read_at(base + at, &mut buffer[..STAGE.min(size - at)]).map_err(|_| Error::Network)?;
        if got == 0 { return Err(Error::Network); }
        if file.write_at(at, &buffer[..got]) != Ok(got) { return Err(Error::Write); }
        at += got;
    }
    Ok(())
}

/// The SHA-256 of a file as it is on the disk.
pub fn sha256(file: &File) -> Option<[u8; 32]> {
    let mut hash = mind::sha256::Sha256::new();
    let mut buffer = alloc::vec![0u8; fs::CHUNK];
    let mut offset = 0;
    while offset < file.size() {
        let got = file.read_at(offset, &mut buffer).ok()?;
        if got == 0 { return None; }
        hash.update(&buffer[..got]);
        offset += got;
    }
    Some(hash.finish())
}
