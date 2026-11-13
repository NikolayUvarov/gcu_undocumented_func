#![no_std]
#![no_main]
// httpd (110-NET-0070): the release layout over HTTPS from a MIND machine, by the rules of tools/httpd (stage 1,
// mind::http_serve). init starts it only when the owner's policy gives program `httpd` a flow, and gives it that flow
// (whose listening grant must name TCP 443), a TLS client with the right to the device certificate (the server role,
// tls.wit 1.3: the key stays in keystore), the parser for request heads (parse.wit 2.1, MC-11.11), the clock, and a
// read-only scope of the owner's data/releases/. Nothing else: no other files, no keys, no devices (MC-11.4).
// One loop serves every connection, and no call waits for a peer, so a slow client holds up no other. Each request is
// one line in the system log: the client, the method, the path without a query, the status, the bytes, the time.
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{BootInfo, SLOT_NETWORK, SLOT_PARSE, SLOT_TLS, SLOT_VFS};
use mind::http_serve::{self, Asked, Bad, Kind, Method, Refusal, Request, HEAD_MAX};
use mind::idl::{parse, tls};
use mind::ipc::Endpoint;

const TLS: Endpoint = Endpoint(SLOT_TLS);
const PARSER: Endpoint = Endpoint(SLOT_PARSE);
const ROOT: &str = "data/releases";
const PORT: u16 = 443;
const BACKLOG: u8 = 8;
const CONNECTIONS: usize = 7; // beside the listener: one client's 8 sessions of tls and 8 sockets of netstack
const PER_ADDRESS: usize = 4; // connections at once from one address
const PER_MINUTE: u32 = 600; // requests a minute from one address
const HEAD_MS: u64 = 10_000; // a request head's deadline, from its first byte
const IDLE_MS: u64 = 10_000; // longest a response waits for the client to take a byte; a kept connection's idle time
const REQUESTS: u32 = 100; // on one connection
const CHUNK: usize = 4096; // tls.wit's largest send and receive
const AGAIN_MS: usize = 10_000; // between tries to listen

fn now() -> u64 { mind::time::uptime_ms() as u64 }

fn dotted(address: u32) -> String { let [a, b, c, d] = address.to_be_bytes(); format!("{a}.{b}.{c}.{d}") }

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK", 206 => "Partial Content", 304 => "Not Modified", 400 => "Bad Request", 404 => "Not Found",
        405 => "Method Not Allowed", 408 => "Request Timeout", 414 => "URI Too Long", 416 => "Range Not Satisfiable",
        429 => "Too Many Requests", 431 => "Request Header Fields Too Large", 505 => "HTTP Version Not Supported", _ => "Error",
    }
}

// The Date header (RFC 9110 5.6.7) when the clock is set; none without one, as a server without a clock sends none.
fn date() -> String {
    let Some(unix) = mind::rtc::unix_time() else { return String::new() };
    let days = unix / 86400;
    let (year, month, day) = mind::rtc::civil_from_days(days.saturating_sub(10957) as u32);
    let s = unix % 86400;
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("Date: {}, {:02} {} {} {:02}:{:02}:{:02} GMT\r\n", DAYS[(days % 7) as usize], day, MONTHS[(month as usize).clamp(1, 12) - 1], year, s / 3600, s / 60 % 60, s % 60)
}

// A refusal's whole response: the reason as text; the connection closes after it.
fn refusal(status: u16, why: &str, extra: &str) -> Vec<u8> {
    format!("HTTP/1.1 {status} {}\r\n{}Server: mind-httpd\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{why}\n",
            reason(status), date(), why.len() + 1).into_bytes()
}

#[derive(PartialEq)]
enum Phase { Handshake, Head, Send, Done }

// The request being answered, for its log line.
struct Line { method: &'static str, path: String, status: u16, body: u64, why: &'static str }

struct Conn {
    session: u32, address: u32, phase: Phase,
    input: Vec<u8>, output: Vec<u8>, sent: usize,
    file: Option<(mind::fs::File, u64, u64)>, // the body still to read: the file, its next byte, the byte after the last
    since: u64, moved: u64, keep: bool, requests: u32, line: Option<Line>,
}

// What a client address did this minute, and how many connections it holds.
struct Client { address: u32, minute: u64, requests: u32 }

struct Server { conns: Vec<Conn>, clients: Vec<Client>, files: bool }

impl Conn {
    fn log(&mut self) {
        let Some(line) = self.line.take() else { return };
        let why = if line.why.is_empty() { String::new() } else { format!(" ({})", line.why) };
        mind::println!("[HTTPD] {} {} {} {} {} {} MS{}", dotted(self.address), line.method, line.path, line.status, line.body, now() - self.since, why);
    }

    // A refusal answered and the connection closed after it.
    fn refuse(&mut self, method: &'static str, path: String, status: u16, why: &'static str, extra: &str) {
        let body = why.len() as u64 + 1;
        self.output = refusal(status, why, extra);
        self.sent = 0;
        self.file = None;
        self.keep = false;
        self.line = Some(Line { method, path, status, body, why });
        self.phase = Phase::Send;
    }

    // One step; whether anything moved.
    fn step(&mut self, server: &mut Vec<Client>, files: bool) -> bool {
        match self.phase {
            Phase::Handshake => match tls::served(TLS, self.session) {
                Ok(Ok(_)) => { self.phase = Phase::Head; (self.since, self.moved) = (now(), now()); true }
                Ok(Err(tls::Error::Again)) => false,
                // The TLS service logged why; one line here for the connection.
                other => {
                    let why = if matches!(other, Ok(Err(tls::Error::Timeout))) { "the TLS handshake took longer than 10 s" } else { "the TLS handshake failed" };
                    self.line = Some(Line { method: "-", path: "-".into(), status: 0, body: 0, why });
                    self.log();
                    self.phase = Phase::Done;
                    true
                }
            },
            Phase::Head => self.head(server, files),
            Phase::Send => self.send(),
            Phase::Done => false,
        }
    }

    fn head(&mut self, clients: &mut Vec<Client>, files: bool) -> bool {
        let mut buffer = [0u8; CHUNK];
        let moved = match tls::receive(TLS, self.session, CHUNK as u32, &mut buffer) {
            Ok(Ok(n)) if n > 0 => {
                if self.input.is_empty() { self.since = now(); } // a head's time runs from its first byte
                self.input.extend_from_slice(&buffer[..n]);
                self.moved = now();
                true
            }
            Ok(Ok(_)) | Ok(Err(tls::Error::Again)) => false,
            _ => { self.phase = Phase::Done; return true; } // closed by the client, or broken: nothing to answer
        };
        if self.input.is_empty() {
            // A kept connection that stays idle just ends.
            if now() - self.moved > IDLE_MS { self.phase = Phase::Done; return true; }
            return moved;
        }
        let Some(end) = http_serve::end(&self.input) else {
            if self.input.len() > HEAD_MAX { self.refuse("-", "-".into(), 431, "the request head is larger than 8192 bytes", ""); return true; }
            if now() - self.since > HEAD_MS { self.refuse("-", "-".into(), 408, "the request head took longer than 10 s", ""); return true; }
            return moved;
        };
        if end > HEAD_MAX { self.refuse("-", "-".into(), 431, "the request head is larger than 8192 bytes", ""); return true; }
        let parsed = match parse::http_request(PARSER, &self.input[..end]) {
            Ok(Ok(typed)) => mind::parse::from_request(&typed),
            _ => { self.refuse("-", "-".into(), 400, "the request head could not be parsed", ""); return true; }
        };
        self.input.drain(..end);
        self.requests += 1;
        match parsed {
            Err(bad) => {
                let extra = if bad == Bad::Method { "Allow: GET, HEAD\r\n" } else { "" };
                self.refuse("-", "-".into(), bad.status(), bad.text(), extra);
            }
            Ok(request) => self.respond(&request, clients, files),
        }
        true
    }

    // The response to a parsed request, its head in `output`, its body (if any) in `file`.
    fn respond(&mut self, request: &Request, clients: &mut Vec<Client>, files: bool) {
        let method = if request.method == Method::Get { "GET" } else { "HEAD" };
        let target = request.target.as_str();
        let path: String = target.split('?').next().unwrap_or("").into(); // the log holds no query string
        if request.body { return self.refuse(method, path, 400, "a request with a body", ""); }
        if let Some(wait) = over_rate(clients, self.address) {
            return self.refuse(method, path, 429, "more than 600 requests a minute from this address", &format!("Retry-After: {wait}\r\n"));
        }
        let (relative, kind) = match http_serve::resolve(target) {
            Ok(found) => found,
            Err(why) => return self.refuse(method, path, why.status(), why.text(), ""),
        };
        let opened = if files { mind::fs::File::open(&format!("{ROOT}/{relative}")) } else { Err(mind::fs::Error::NotFound) };
        let file = match opened {
            Ok(file) => file,
            Err(mind::fs::Error::IsDirectory) => return self.refuse(method, path, 404, Refusal::NotAFile.text(), ""),
            Err(_) => return self.refuse(method, path, 404, Refusal::Missing.text(), ""),
        };
        let len = file.size() as u64;
        let modified = file.metadata().map(|m| m.modified).unwrap_or(0);
        let tag = match kind {
            Kind::Blob => format!("\"{}\"", relative.rsplit('/').next().unwrap_or("")),
            _ => format!("\"{len:x}-{modified:x}\""),
        };
        let cache = if kind == Kind::Channel { "no-cache" } else { "public, max-age=31536000, immutable" };
        self.keep = request.keep_alive && self.requests < REQUESTS;
        let keep = if self.keep { "keep-alive" } else { "close" };
        let mut head = format!("{}Server: mind-httpd\r\nAccept-Ranges: bytes\r\nETag: {tag}\r\nCache-Control: {cache}\r\nConnection: {keep}\r\n", date());
        self.sent = 0;
        self.file = None;
        if http_serve::matches_tag(Some(request.if_none_match.as_str()).filter(|t| !t.is_empty()), &tag) {
            self.output = format!("HTTP/1.1 304 Not Modified\r\n{head}\r\n").into_bytes();
            self.line = Some(Line { method, path, status: 304, body: 0, why: "" });
            self.phase = Phase::Send;
            return;
        }
        let range = Some(request.range.as_str()).filter(|r| !r.is_empty());
        let (status, start, count) = match http_serve::asked(range, len) {
            Asked::Whole => (200, 0, len),
            Asked::Part(first, last) => { head.push_str(&format!("Content-Range: bytes {first}-{last}/{len}\r\n")); (206, first, last - first + 1) }
            Asked::Unsatisfiable => return self.refuse(method, path, 416, "the range is outside the file", &format!("Content-Range: bytes */{len}\r\n")),
        };
        self.output = format!("HTTP/1.1 {status} {}\r\n{head}Content-Type: application/octet-stream\r\nContent-Length: {count}\r\n\r\n", reason(status)).into_bytes();
        let body = if request.method == Method::Head { 0 } else { count };
        if body > 0 { self.file = Some((file, start, start + count)); }
        self.line = Some(Line { method, path, status, body, why: "" });
        self.phase = Phase::Send;
    }

    // Sends what is pending, the head first, then the body read on the way; then the next request or the end.
    fn send(&mut self) -> bool {
        let mut moved = false;
        loop {
            if self.sent == self.output.len() {
                self.output.clear();
                self.sent = 0;
                match self.file.as_mut() {
                    Some((file, at, end)) if *at < *end => {
                        self.output.resize(((*end - *at) as usize).min(4 * CHUNK), 0);
                        match file.read_at(*at as usize, &mut self.output) {
                            Ok(n) if n > 0 => { self.output.truncate(n); *at += n as u64; }
                            // A file cut shorter while it was read cannot give the length promised: the connection ends.
                            _ => {
                                if let Some(line) = self.line.as_mut() { line.why = "the file was shorter than when it was opened"; line.body -= *end - *at; }
                                self.output.clear();
                                self.keep = false;
                                self.finish();
                                return true;
                            }
                        }
                    }
                    _ => { self.finish(); return true; }
                }
            }
            let end = (self.sent + CHUNK).min(self.output.len());
            match tls::send(TLS, self.session, &self.output[self.sent..end]) {
                Ok(Ok(n)) if n > 0 => { self.sent += n as usize; self.moved = now(); moved = true; }
                Ok(Ok(_)) | Ok(Err(tls::Error::Again)) => {
                    if now() - self.moved > IDLE_MS {
                        if let Some(line) = self.line.as_mut() { line.why = "the client took no byte for 10 s"; }
                        self.keep = false;
                        self.finish();
                        return true;
                    }
                    return moved;
                }
                _ => { self.keep = false; self.finish(); return true; }
            }
        }
    }

    // A response sent whole (or given up): its log line, then the next request on a kept connection.
    fn finish(&mut self) {
        self.log();
        self.file = None;
        if self.keep { self.phase = Phase::Head; (self.since, self.moved) = (now(), now()); } else { self.phase = Phase::Done; }
    }
}

// Seconds until the address's minute ends, when this request is over its limit.
fn over_rate(clients: &mut Vec<Client>, address: u32) -> Option<u64> {
    let at = now();
    clients.retain(|c| at - c.minute < 60_000);
    let client = match clients.iter().position(|c| c.address == address) {
        Some(i) => &mut clients[i],
        None => { clients.push(Client { address, minute: at, requests: 0 }); clients.last_mut()? }
    };
    client.requests += 1;
    (client.requests > PER_MINUTE).then(|| (60 - (at - client.minute) / 1000).max(1))
}

impl Server {
    // Takes the connections that wait, as long as there is room; whether any came.
    fn accept(&mut self, listener: u32) -> bool {
        let mut came = false;
        while self.conns.len() < CONNECTIONS {
            match tls::accept(TLS, listener) {
                Ok(Ok(accepted)) => {
                    came = true;
                    if self.conns.iter().filter(|c| c.address == accepted.address).count() >= PER_ADDRESS {
                        mind::println!("[HTTPD] {} - - 0 0 0 MS (more than {} connections at once from this address)", dotted(accepted.address), PER_ADDRESS);
                        let _ = tls::close(TLS, accepted.session);
                        continue;
                    }
                    self.conns.push(Conn { session: accepted.session, address: accepted.address, phase: Phase::Handshake, input: Vec::new(), output: Vec::new(), sent: 0,
                                           file: None, since: now(), moved: now(), keep: false, requests: 0, line: None });
                }
                Ok(Err(tls::Error::Again)) | Ok(Err(tls::Error::Limit)) => break,
                other => { mind::println!("[HTTPD] ACCEPT FAILED: {:?}", other.map_err(|_| ())); break; }
            }
        }
        came
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    // The releases, read-only: init's scope of data/releases/, which it does not make; without it every file is missing.
    let files = mind::dev::cap_info(SLOT_VFS).0 == mind::abi::CAP_KIND_ENDPOINT;
    if files { mind::fs::use_scope(Endpoint(SLOT_VFS), ROOT); } else { mind::println!("[HTTPD] NO {ROOT}/: EVERY PATH IS ANSWERED 404"); }
    let listener = loop {
        let result = match tls::attach(TLS, SLOT_NETWORK) {
            Ok(Ok(session)) => match tls::listen(TLS, session, PORT, BACKLOG) {
                Ok(Ok(())) => break session,
                other => { let _ = tls::close(TLS, session); other.map_err(|_| ()) }
            },
            other => other.map(|r| r.map(drop)).map_err(|_| ()),
        };
        // Denied: the policy's listening grant does not name the port; tried again, as the owner may change the policy.
        mind::println!("[HTTPD] TCP {} NOT LISTENED ON: {:?}; AGAIN IN {} S", PORT, result, AGAIN_MS / 1000);
        mind::time::sleep(AGAIN_MS);
    };
    mind::println!("[HTTPD] SERVING {}/ OVER HTTPS ON TCP {}, AT MOST {} CONNECTIONS", ROOT, PORT, CONNECTIONS);
    let mut server = Server { conns: Vec::new(), clients: Vec::new(), files };
    loop {
        let mut moved = server.accept(listener);
        let Server { conns, clients, files } = &mut server;
        for conn in conns.iter_mut() { moved |= conn.step(clients, *files); }
        conns.retain(|c| { if c.phase == Phase::Done { let _ = tls::close(TLS, c.session); } c.phase != Phase::Done });
        // Nothing moved: a short wait while connections are open, a longer one while none is.
        if !moved { mind::time::sleep(if server.conns.is_empty() { 50 } else { 5 }); }
    }
}
