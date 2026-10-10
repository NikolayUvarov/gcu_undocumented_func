// Where releases come from (351-UPD-0007): an HTTPS server, trusted by its pinned key or the system's roots, over the
// flow grant the network policy gives the updater; or a directory on a mounted volume (a USB stick). Both read the
// layout of scripts/release.py, and nothing read here is trusted until the updater has checked it.
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{SLOT_NETWORK, SLOT_PARSE, SLOT_TLS};
use mind::fs::{self, File};
use mind::http::{self, Sink, Transport, Url};
use mind::idl::{socket, tls};
use mind::ipc::Endpoint;
use mind::idl::update::Error;

const STACK: Endpoint = Endpoint(SLOT_NETWORK);
const TLS: Endpoint = Endpoint(SLOT_TLS);
const IDLE_MS: usize = 15_000; // no progress for this long: the connection counts as cut
const TRIES: u32 = 5; // connections per file before giving up
const STAGE: usize = 64 * 1024; // a file is written to the slot in pieces of this size

pub enum Source { Https { base: String, pin: Option<[u8; 32]> }, Directory(String) }

fn now() -> usize { mind::time::uptime_ms() }

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
    let session = match tls::attach(TLS, SLOT_NETWORK) { Ok(Ok(session)) => Session(session), _ => return Err(Error::Network) };
    let connected = match pin {
        Some(pin) => tls::connect_pinned(TLS, session.0, url.host, address, url.port, pin, 10_000),
        None => tls::connect(TLS, session.0, url.host, address, url.port, false, 10_000),
    };
    match connected {
        Ok(Ok(_)) => Ok(Some(session)),
        Ok(Err(tls::Error::Refused | tls::Error::Timeout | tls::Error::Closed | tls::Error::NoNetwork)) => Ok(None),
        // A server that is not the one trusted: asking again changes nothing.
        Ok(Err(error)) => { mind::println!("[UPDATER] TLS: {:?}", error); Err(Error::Network) }
        Err(_) => Err(Error::Network),
    }
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
            if self.file.write_at(self.at as usize, &self.stage[..self.staged]) != Ok(self.staged) { self.failed = true; return Err(http::Error::Sink); }
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
fn get(base: &str, pin: Option<&[u8; 32]>, path: &str, mut offset: u64, sink: &mut impl Sink) -> Result<u64, Error> {
    let address = format!("{}/{}", base.trim_end_matches('/'), path);
    let url = Url::parse(&address).map_err(|_| Error::NoSource)?;
    let mut parser = mind::parse::Service(Endpoint(SLOT_PARSE));
    for connection in 1..=TRIES {
        let Some(mut session) = connect(&url, pin)? else { mind::time::sleep(1000); continue };
        match http::get(&mut session, &url, offset, sink, &mut parser) {
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
    pub fn small(&self, path: &str, limit: usize) -> Result<Vec<u8>, Error> {
        match self {
            Source::Https { base, pin } => {
                let mut memory = Memory { data: Vec::new(), limit };
                get(base, pin.as_ref(), path, 0, &mut memory)?;
                Ok(memory.data)
            }
            Source::Directory(dir) => {
                let file = File::open(&format!("{}/{}", dir, path)).map_err(|_| Error::Missing)?;
                if file.size() > limit { return Err(Error::Digest); }
                let mut data = alloc::vec![0u8; file.size()];
                if file.read_at(0, &mut data) != Ok(data.len()) { return Err(Error::Network); }
                Ok(data)
            }
        }
    }

    /// File `path` of the layout into `file`, which is to hold `size` bytes: what it holds already is kept and the rest
    /// fetched after it. Returns the bytes fetched.
    pub fn file(&self, path: &str, file: &mut File, size: u64) -> Result<u64, Error> {
        let have = file.size() as u64;
        if have > size { file.truncate(0).map_err(|_| Error::Write)?; return self.file(path, file, size); }
        if have == size { return Ok(0); }
        if have > 0 { mind::println!("[UPDATER] {}: RESUMING AT {} OF {}", path, have, size); }
        match self {
            Source::Https { base, pin } => {
                let mut output = Output { file, size, stage: alloc::vec![0; STAGE], staged: 0, at: have, failed: false };
                let result = get(base, pin.as_ref(), path, have, &mut output);
                let flushed = output.flush();
                if output.failed || flushed.is_err() { return Err(Error::Write); }
                result.map(|_| size - have)
            }
            Source::Directory(dir) => {
                let from = File::open(&format!("{}/{}", dir, path)).map_err(|_| Error::Missing)?;
                if from.size() as u64 != size { return Err(Error::Digest); }
                let mut buffer = alloc::vec![0u8; STAGE];
                let mut at = have as usize;
                while at < size as usize {
                    let got = from.read_at(at, &mut buffer).map_err(|_| Error::Network)?;
                    if got == 0 { return Err(Error::Network); }
                    if file.write_at(at, &buffer[..got]) != Ok(got) { return Err(Error::Write); }
                    at += got;
                }
                Ok(size - have)
            }
        }
    }
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
