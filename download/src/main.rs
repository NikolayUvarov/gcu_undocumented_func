#![no_std]
#![no_main]
extern crate alloc;
// download FILE URL [--sha256 HEX] [--tries N] [--pin HEX]: downloads URL into FILE over HTTP/1.1 with mind::http
// (351-NET-0001), over TLS 1.3 for https (351-NET-0002). A
// file already partly there, and a connection cut midway, are resumed with `Range`. The network is the flow grant its
// launcher got from the policy broker (name lookups too, with a `download dns` line), the file a client confined to its
// directory (REQUEST_FILE). The response head is parsed by the parser service (REQUEST_PARSE, 109-NET-0009), which holds
// neither; download checks what it says against what it asked for. https runs a session of the TLS service over the
// same flow grant (REQUEST_TLS); the server is verified against the system's roots, or by its pinned key (--pin).
use alloc::vec::Vec;
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_FILE, SLOT_NETWORK, SLOT_PARSE, SLOT_TLS};
use mind::fs::{self, File, MODE_CREATE, MODE_WRITE};
use mind::http::{self, Sink, Transport, Url};
use mind::idl::{socket, tls};
use mind::ipc::Endpoint;

mind::request!(REQUEST_CONSOLE | REQUEST_NETWORK | REQUEST_FILE | REQUEST_PARSE | REQUEST_TLS);

const STACK: Endpoint = Endpoint(SLOT_NETWORK);
const TLS: Endpoint = Endpoint(SLOT_TLS);
const IDLE_MS: usize = 15_000; // no progress for this long: the connection counts as cut
const STAGE: usize = 64 * 1024; // the body is written to the file in pieces of this size

fn now() -> usize { mind::time::uptime_ms() }

// A TCP connection of the flow grant.
struct Tcp { socket: u32 }

impl Transport for Tcp {
    fn send(&mut self, data: &[u8]) -> Result<(), http::Error> {
        let (mut sent, deadline) = (0, now() + IDLE_MS);
        while sent < data.len() {
            let end = data.len().min(sent + 4096);
            match socket::tcp_send(STACK, self.socket, &data[sent..end]) {
                Ok(Ok(0)) if now() < deadline => { mind::time::sleep(5); }
                Ok(Ok(n)) if n > 0 => sent += n as usize,
                _ => return Err(http::Error::Transport),
            }
        }
        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, http::Error> {
        let deadline = now() + IDLE_MS;
        let length = buffer.len().min(4096);
        loop {
            match socket::tcp_receive(STACK, self.socket, length as u32, &mut buffer[..length]) {
                Ok(Ok(n)) if n > 0 => return Ok(n),
                Ok(Err(socket::Error::Closed)) => return Ok(0),
                Ok(Ok(_)) | Ok(Err(socket::Error::Again)) if now() < deadline => { mind::time::sleep(2); }
                _ => return Err(http::Error::Transport),
            }
        }
    }
}

impl Drop for Tcp { fn drop(&mut self) { let _ = socket::close(STACK, self.socket); } }

// A TLS session of the TLS service over the flow grant.
struct Tls { session: u32 }

impl Transport for Tls {
    fn send(&mut self, data: &[u8]) -> Result<(), http::Error> {
        let (mut sent, deadline) = (0, now() + IDLE_MS);
        while sent < data.len() {
            let end = data.len().min(sent + 4096);
            match tls::send(TLS, self.session, &data[sent..end]) {
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
            match tls::receive(TLS, self.session, length as u32, &mut buffer[..length]) {
                Ok(Ok(n)) if n > 0 => return Ok(n),
                Ok(Err(tls::Error::Closed)) => return Ok(0),
                Ok(Ok(_)) | Ok(Err(tls::Error::Again)) if now() < deadline => { mind::time::sleep(2); }
                _ => return Err(http::Error::Transport),
            }
        }
    }
}

impl Drop for Tls { fn drop(&mut self) { let _ = tls::close(TLS, self.session); } }

// The connection of one GET: plain TCP, or TLS over it.
enum Link { Tcp(Tcp), Tls(Tls) }

impl Transport for Link {
    fn send(&mut self, data: &[u8]) -> Result<(), http::Error> { match self { Link::Tcp(t) => t.send(data), Link::Tls(t) => t.send(data) } }
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, http::Error> { match self { Link::Tcp(t) => t.receive(buffer), Link::Tls(t) => t.receive(buffer) } }
}

// What a connection attempt came to: a link, a failure worth another try, or one that asking again will not change.
enum Connected { Link(Link), Retry, Fatal(tls::Error) }

fn connect(url: &Url, address: u32, pin: Option<&[u8; 32]>) -> Connected {
    if !url.https {
        return match socket::tcp_connect(STACK, address, url.port, 10_000) {
            Ok(Ok(socket)) => Connected::Link(Link::Tcp(Tcp { socket })),
            Ok(Err(error)) => { mind::println!("DOWNLOAD: CONNECT: {:?}", error); Connected::Retry }
            Err(_) => fail(format_args!("NO NETWORK GRANT")),
        };
    }
    // The TLS service gets a child of the flow grant: the policy that applies to download applies to the session.
    let session = match tls::attach(TLS, SLOT_NETWORK) {
        Ok(Ok(session)) => session,
        Ok(Err(error)) => return Connected::Fatal(error),
        Err(_) => fail(format_args!("NO TLS SERVICE")),
    };
    let link = Tls { session };
    let connected = match pin {
        Some(pin) => tls::connect_pinned(TLS, session, url.host, address, url.port, pin, 10_000),
        None => tls::connect(TLS, session, url.host, address, url.port, false, 10_000),
    };
    match connected {
        Ok(Ok(_)) => Connected::Link(Link::Tls(link)),
        Ok(Err(error @ (tls::Error::Refused | tls::Error::Timeout | tls::Error::Closed | tls::Error::NoNetwork))) => { mind::println!("DOWNLOAD: CONNECT: {:?}", error); Connected::Retry }
        Ok(Err(error)) => Connected::Fatal(error),
        Err(_) => fail(format_args!("NO TLS SERVICE")),
    }
}

// The file, written in pieces of STAGE bytes (on the heap: the stack is 64 KiB).
struct Output { file: File, stage: Vec<u8>, staged: usize, at: u64, failed: bool }

impl Output {
    fn flush(&mut self) -> Result<(), http::Error> {
        if self.staged > 0 {
            let written = self.file.write_at(self.at as usize, &self.stage[..self.staged]);
            if written != Ok(self.staged) { self.failed = true; return Err(http::Error::Sink); }
            self.at += self.staged as u64;
            self.staged = 0;
        }
        Ok(())
    }
}

impl Sink for Output {
    fn begin(&mut self, start: u64, _total: u64) -> Result<(), http::Error> {
        self.flush()?;
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

fn ipv4(text: &str) -> Option<u32> {
    let mut address = [0u8; 4]; let mut parts = text.split('.');
    for byte in address.iter_mut() { *byte = parts.next()?.parse().ok()?; }
    parts.next().is_none().then_some(u32::from_be_bytes(address))
}

fn hex(digest: &[u8; 32]) -> [u8; 64] {
    let mut out = [0u8; 64];
    for (k, b) in digest.iter().enumerate() { out[2 * k] = b"0123456789abcdef"[(b >> 4) as usize]; out[2 * k + 1] = b"0123456789abcdef"[(b & 15) as usize]; }
    out
}

// The SHA-256 of the file as it is on the disk.
fn sha256(file: &File) -> Option<[u8; 32]> {
    let mut hash = mind::sha256::Sha256::new();
    let mut buffer = [0u8; fs::CHUNK];
    let mut offset = 0;
    while offset < file.size() {
        let got = file.read_at(offset, &mut buffer).ok()?;
        if got == 0 { return None; }
        hash.update(&buffer[..got]);
        offset += got;
    }
    Some(hash.finish())
}

fn fail(what: core::fmt::Arguments) -> ! {
    mind::println!("DOWNLOAD: {}", what);
    mind::process::exit_with(1)
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("download — downloads a file over HTTP/1.1 or HTTPS, resuming one already partly there and a connection cut midway (Range).\nUsage: download FILE URL [--sha256 HEX] [--tries N] [--pin HEX]\n  URL: http[s]://HOST[:PORT]/PATH; HOST is A.B.C.D or a name (looked up with a `download dns` policy line).\n  --sha256: the file's expected SHA-256; --tries: connections before giving up (default 5).\n  --pin: https only, the SHA-256 of the server's public key (SubjectPublicKeyInfo): the server is trusted by it alone.\nIt reaches only what netpolicy.txt grants download; https is verified against tlsroots.pem without --pin.");
    let mut words = mind::process::args_str().split_whitespace();
    const USAGE: &str = "USAGE: download FILE URL [--sha256 HEX] [--tries N] [--pin HEX]";
    let (Some(path), Some(url)) = (words.next(), words.next()) else { fail(format_args!("{}", USAGE)) };
    let (mut expected, mut tries, mut pin) = (None, 5u32, None);
    let hex64 = |h: &str| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit());
    while let Some(word) = words.next() {
        match (word, words.next()) {
            ("--sha256", Some(h)) if hex64(h) => expected = Some(h),
            ("--tries", Some(n)) if n.parse::<u32>().is_ok_and(|n| n > 0) => tries = n.parse().unwrap_or(5),
            ("--pin", Some(h)) if hex64(h) => pin = Some(core::array::from_fn::<u8, 32, _>(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap_or(0))),
            _ => fail(format_args!("{}", USAGE)),
        }
    }
    let Ok(url) = Url::parse(url) else { fail(format_args!("NOT AN http:// OR https:// URL: {}", url)) };
    if pin.is_some() && !url.https { fail(format_args!("--pin IS FOR https:// URLS")); }
    if url.https && mind::dev::cap_info(SLOT_TLS).0 != CAP_KIND_ENDPOINT { fail(format_args!("HTTPS NEEDS A TLS CLIENT, AND NONE WAS LENT")); }
    if mind::dev::cap_info(SLOT_NETWORK).0 != CAP_KIND_ENDPOINT { fail(format_args!("NO NETWORK GRANT")); }
    // It does not parse response heads itself: without the parser service it does not download (MC-11.11).
    if mind::dev::cap_info(SLOT_PARSE).0 != CAP_KIND_ENDPOINT { fail(format_args!("NO PARSER SERVICE")); }
    let mut parser = mind::parse::Service(Endpoint(SLOT_PARSE));
    // The client the launcher lent is confined to the file's directory.
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT {
        let (volume, rest) = fs::split(path);
        let parent = rest.rfind('/').map_or("", |i| &rest[..i]);
        let mut base = [0u8; 300];
        let len = volume.len() + 1 + parent.len();
        if len <= base.len() {
            base[..volume.len()].copy_from_slice(volume.as_bytes());
            base[volume.len()] = b':';
            base[volume.len() + 1..len].copy_from_slice(parent.as_bytes());
            let base = core::str::from_utf8(&base[..len]).unwrap_or("");
            fs::use_scope(Endpoint(SLOT_FILE), if volume.is_empty() { parent } else { base });
        }
    }
    let file = match File::open_mode(path, MODE_WRITE | MODE_CREATE) { Ok(file) => file, Err(error) => fail(format_args!("CANNOT OPEN {}: {:?}", path, error)) };
    let address = match ipv4(url.host) {
        Some(address) => address,
        None => match socket::resolve(STACK, url.host, 0, 0, 5000) {
            Ok(Ok(address)) => address,
            Ok(Err(error)) => fail(format_args!("CANNOT LOOK UP {}: {:?}", url.host, error)),
            Err(_) => fail(format_args!("NO NETWORK GRANT")),
        },
    };
    let mut output = Output { at: file.size() as u64, file, stage: alloc::vec![0; STAGE], staged: 0, failed: false };
    if output.at > 0 { mind::println!("DOWNLOAD: RESUMING {} AT {}", path, output.at); }
    let started = now();
    let mut connections = 0;
    let got = loop {
        if connections == tries { fail(format_args!("GAVE UP AFTER {} CONNECTIONS AT {} BYTES", connections, output.at)); }
        connections += 1;
        let offset = output.at;
        let mut link = match connect(&url, address, pin.as_ref()) {
            Connected::Link(link) => link,
            Connected::Retry => { mind::time::sleep(1000); continue; }
            // A server that is not the one trusted, or a TLS service that cannot serve: asking again changes nothing.
            Connected::Fatal(error) => fail(format_args!("TLS: {:?}", error)),
        };
        let result = http::get(&mut link, &url, offset, &mut output, &mut parser);
        drop(link);
        if output.flush().is_err() || output.failed { fail(format_args!("CANNOT WRITE {}", path)); }
        match result {
            Ok(got) if got.start == 0 && offset > 0 && got.complete() => { mind::println!("DOWNLOAD: THE SERVER SENT THE WHOLE FILE AGAIN"); break got; }
            Ok(got) if got.complete() => break got,
            Ok(got) => mind::println!("DOWNLOAD: CONNECTION CUT AT {} OF {}, RESUMING", got.end, got.total),
            // Status, length, range: the server's answer will not change by asking again.
            Err(http::Error::Transport) => mind::println!("DOWNLOAD: NO ANSWER, TRYING AGAIN"),
            Err(error) => fail(format_args!("HTTP: {:?}", error)),
        }
    };
    if got.end == got.start && got.start > 0 {
        mind::println!("DOWNLOAD: ALREADY COMPLETE, {} BYTES", got.total);
    } else {
        mind::println!("DOWNLOAD: DONE {} BYTES IN {} MS, {} CONNECTIONS", got.total, now() - started, connections);
    }
    let _ = output.file.flush();
    let Some(digest) = sha256(&output.file) else { fail(format_args!("CANNOT READ {} BACK", path)) };
    let digest = hex(&digest);
    let digest = core::str::from_utf8(&digest).unwrap_or("");
    match expected {
        None => mind::println!("DOWNLOAD: SHA256 {}", digest),
        Some(h) if h.eq_ignore_ascii_case(digest) => mind::println!("DOWNLOAD: SHA256 {} MATCHES", digest),
        Some(_) => fail(format_args!("SHA256 {} DOES NOT MATCH", digest)),
    }
}
