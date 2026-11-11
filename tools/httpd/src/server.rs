//! The server: a thread a connection within the limits (MC-5.1, 5.2), keep-alive, GET and HEAD of the release layout.

use crate::head::{self, Method, Request};
use crate::layout::{self, Kind};
use crate::log::Log;
use crate::range::{self, Asked};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// What the server serves and its limits; each refusal names the limit it met.
#[derive(Clone, Debug)]
pub struct Config {
    pub root: PathBuf,
    pub connections: usize,
    pub per_client: usize,
    pub per_minute: u32,
    pub head_timeout: Duration,
    pub idle_timeout: Duration,
    pub requests_per_connection: usize,
}

impl Config {
    pub fn new(root: PathBuf) -> Config {
        Config { root, connections: 64, per_client: 8, per_minute: 600, head_timeout: Duration::from_secs(10),
                 idle_timeout: Duration::from_secs(10), requests_per_connection: 1000 }
    }
}

#[derive(Default)]
struct Client { connections: usize, window: Option<Instant>, requests: u32 }

struct Server { config: Config, log: Log, total: Mutex<(usize, HashMap<IpAddr, Client>)> }

/// A connection's place in the counts, given back when it ends.
struct Admitted { server: Arc<Server>, client: IpAddr }
impl Drop for Admitted {
    fn drop(&mut self) {
        let mut counts = self.server.total.lock().unwrap();
        counts.0 -= 1;
        if let Some(c) = counts.1.get_mut(&self.client) { c.connections -= 1; }
    }
}

/// Serves `listener` until it fails; `config.root` must be canonical.
pub fn run(listener: TcpListener, config: Config, log: Log) -> io::Result<()> {
    let server = Arc::new(Server { config, log, total: Mutex::new((0, HashMap::new())) });
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let Ok(peer) = stream.peer_addr() else { continue };
        let client = peer.ip();
        let refused = {
            let mut counts = server.total.lock().unwrap();
            let (connections, per_client) = (server.config.connections, server.config.per_client);
            if counts.0 >= connections { Some(format!("the server holds its limit of {connections} connections")) }
            else if counts.1.get(&client).map_or(0, |c| c.connections) >= per_client { Some(format!("this address holds its limit of {per_client} connections")) }
            else { counts.0 += 1; counts.1.entry(client).or_default().connections += 1; None }
        };
        if let Some(why) = refused {
            let mut stream = stream;
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let _ = refuse(&mut stream, 503, &why, &[]);
            server.log.line(SystemTime::now(), &client.to_string(), "-", "-", 503, 0, Duration::ZERO, &why);
            continue;
        }
        let admitted = Admitted { server: server.clone(), client };
        std::thread::spawn(move || connection(&admitted.server, stream, client));
    }
    Ok(())
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK", 206 => "Partial Content", 304 => "Not Modified", 400 => "Bad Request", 404 => "Not Found",
        405 => "Method Not Allowed", 408 => "Request Timeout", 416 => "Range Not Satisfiable", 429 => "Too Many Requests",
        431 => "Request Header Fields Too Large", 503 => "Service Unavailable", 505 => "HTTP Version Not Supported", _ => "Error",
    }
}

// A refusal's whole response: the reason as text, the connection closed after it.
fn refuse(stream: &mut TcpStream, status: u16, why: &str, extra: &[(&str, String)]) -> io::Result<u64> {
    let body = format!("{why}\n");
    let mut head = format!("HTTP/1.1 {status} {}\r\nDate: {}\r\nServer: mind-httpd\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n",
                           reason(status), crate::date::http(SystemTime::now()), body.len());
    for (name, value) in extra { head.push_str(&format!("{name}: {value}\r\n")); }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    Ok(body.len() as u64)
}

fn connection(server: &Server, mut stream: TcpStream, client: IpAddr) {
    let config = &server.config;
    let _ = stream.set_read_timeout(Some(config.idle_timeout));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
    let _ = stream.set_nodelay(true);
    let who = client.to_string();
    let mut buf: Vec<u8> = Vec::new();
    for _ in 0..config.requests_per_connection {
        let started = Instant::now();
        // The head, within its size and its deadline; an idle kept-alive connection just ends.
        let end = loop {
            if let Some(end) = head::end(&buf) { break end; }
            let late = started.elapsed() > config.head_timeout;
            if buf.len() > head::HEAD_MAX || late {
                let (status, why) = if late { (408, format!("the request head took longer than {} s", config.head_timeout.as_secs())) }
                                    else { (431, format!("the request head is larger than {} bytes", head::HEAD_MAX)) };
                let sent = refuse(&mut stream, status, &why, &[]).unwrap_or(0);
                server.log.line(SystemTime::now(), &who, "-", "-", status, sent, started.elapsed(), &why);
                return;
            }
            let mut chunk = [0u8; 4096];
            match stream.read(&mut chunk) {
                Ok(0) => return,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
                    if buf.is_empty() { return; }
                    let why = format!("no byte of the request head for {} s", config.idle_timeout.as_secs());
                    let sent = refuse(&mut stream, 408, &why, &[]).unwrap_or(0);
                    server.log.line(SystemTime::now(), &who, "-", "-", 408, sent, started.elapsed(), &why);
                    return;
                }
                Err(_) => return,
            }
        };
        let request = head::parse(&buf[..end]);
        buf.drain(..end);
        let request = match request {
            Ok(r) => r,
            Err(bad) => {
                let allow = if bad == head::Bad::Method { vec![("Allow", "GET, HEAD".to_string())] } else { vec![] };
                let sent = refuse(&mut stream, bad.status(), bad.text(), &allow).unwrap_or(0);
                server.log.line(SystemTime::now(), &who, "-", "-", bad.status(), sent, started.elapsed(), bad.text());
                return;
            }
        };
        let method = if request.method == Method::Get { "GET" } else { "HEAD" };
        // A request with a body is not read: it is refused, and the connection with it.
        if request.header("content-length").is_some_and(|v| v.trim() != "0") || request.header("transfer-encoding").is_some() {
            let why = "a request with a body";
            let sent = refuse(&mut stream, 400, why, &[]).unwrap_or(0);
            server.log.line(SystemTime::now(), &who, method, &request.target, 400, sent, started.elapsed(), why);
            return;
        }
        if let Some((window, limit)) = over_rate(server, client) {
            let why = format!("more than {limit} requests a minute from this address");
            let sent = refuse(&mut stream, 429, &why, &[("Retry-After", window.to_string())]).unwrap_or(0);
            server.log.line(SystemTime::now(), &who, method, &request.target, 429, sent, started.elapsed(), &why);
            return;
        }
        match respond(server, &mut stream, &request) {
            Ok((status, sent, why, keep)) => {
                server.log.line(SystemTime::now(), &who, method, &request.target, status, sent, started.elapsed(), why);
                if !keep || !request.keep_alive() { return; }
            }
            Err(_) => return,
        }
    }
}

// Seconds until the client's minute ends and the limit, when this request is over it.
fn over_rate(server: &Server, client: IpAddr) -> Option<(u64, u32)> {
    let mut counts = server.total.lock().unwrap();
    let c = counts.1.entry(client).or_default();
    let now = Instant::now();
    if c.window.map_or(true, |w| now.duration_since(w) >= Duration::from_secs(60)) { c.window = Some(now); c.requests = 0; }
    c.requests += 1;
    let limit = server.config.per_minute;
    (c.requests > limit).then(|| 60u64.saturating_sub(now.duration_since(c.window.unwrap()).as_secs()).max(1)).map(|s| (s, limit))
}

fn matches_tag(header: Option<&str>, tag: &str) -> bool {
    header.is_some_and(|h| h.split(',').map(|t| t.trim().trim_start_matches("W/")).any(|t| t == "*" || t == tag))
}

// The response to a parsed request: its status, the body bytes sent, the refusal's reason, whether to keep the connection.
fn respond(server: &Server, stream: &mut TcpStream, request: &Request) -> io::Result<(u16, u64, &'static str, bool)> {
    let (relative, kind) = match layout::resolve(&request.target) {
        Ok(found) => found,
        Err(why) => { let sent = refuse(stream, why.status(), why.text(), &[])?; return Ok((why.status(), sent, why.text(), false)); }
    };
    let (mut file, meta) = match layout::open(&server.config.root, &relative) {
        Ok(opened) => opened,
        Err(why) => { let sent = refuse(stream, why.status(), why.text(), &[])?; return Ok((why.status(), sent, why.text(), false)); }
    };
    let len = meta.len();
    let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let tag = match kind {
        Kind::Blob => format!("\"{}\"", relative.rsplit('/').next().unwrap_or("")),
        _ => format!("\"{:x}-{:x}{:08x}\"", len, meta.mtime(), meta.mtime_nsec()),
    };
    let cache = if kind == Kind::Channel { "no-cache" } else { "public, max-age=31536000, immutable" };
    let keep = if request.keep_alive() { "keep-alive" } else { "close" };
    let mut head = format!("Date: {}\r\nServer: mind-httpd\r\nAccept-Ranges: bytes\r\nETag: {tag}\r\nLast-Modified: {}\r\nCache-Control: {cache}\r\nConnection: {keep}\r\n",
                           crate::date::http(SystemTime::now()), crate::date::http(modified));
    if matches_tag(request.header("if-none-match"), &tag) {
        stream.write_all(format!("HTTP/1.1 304 Not Modified\r\n{head}\r\n").as_bytes())?;
        return Ok((304, 0, "", true));
    }
    let (status, start, count) = match range::asked(request.header("range"), len) {
        Asked::Whole => (200, 0, len),
        Asked::Part(first, last) => { head.push_str(&format!("Content-Range: bytes {first}-{last}/{len}\r\n")); (206, first, last - first + 1) }
        Asked::Unsatisfiable => {
            let why = "the range is outside the file";
            let sent = refuse(stream, 416, why, &[("Content-Range", format!("bytes */{len}"))])?;
            return Ok((416, sent, why, false));
        }
    };
    stream.write_all(format!("HTTP/1.1 {status} {}\r\n{head}Content-Type: application/octet-stream\r\nContent-Length: {count}\r\n\r\n", reason(status)).as_bytes())?;
    if request.method == Method::Head { return Ok((status, 0, "", true)); }
    let sent = copy(&mut file, start, count, stream)?;
    // A file cut shorter while it was read cannot give the length promised: the connection ends there.
    if sent < count { return Ok((status, sent, "the file was shorter than when it was opened", false)); }
    Ok((status, sent, "", true))
}

// The bytes `start..start+count` of the file opened for this request, as they were when it was opened.
fn copy(file: &mut File, start: u64, count: u64, out: &mut TcpStream) -> io::Result<u64> {
    file.seek(SeekFrom::Start(start))?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut sent = 0;
    while sent < count {
        let want = (count - sent).min(buf.len() as u64) as usize;
        let n = file.read(&mut buf[..want])?;
        if n == 0 { break; }
        out.write_all(&buf[..n])?;
        sent += n as u64;
    }
    Ok(sent)
}
