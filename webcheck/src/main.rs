#![no_std]
#![no_main]
// webcheck URL [--post JSON] [--repeat N] [--stream] [--idle SECONDS] [--pin leaf|spki:HEX]... [--name NAME]: the network
// track's check of HTTP for services (750-NET-0019). Over one connection (TCP, or a session of the TLS service for
// https) it posts JSON to URL and reads the response, chunked or not; then GETs URL N more times on the same
// connection; with --stream it GETs URL once more and reads its body to its end, however long the server holds it, up
// to --idle seconds without a byte; with none of these, one GET. With one or two --pin, https trusts the server by
// them and the TLS service still checks its certificate for NAME (default: the host) (tls.wit 1.2, 750-NET-0020).
// Response heads go to the parser service, as download's do; with --json, a JSON body too, one line per value
// (parse.wit 1.4, 750-NET-0021). One line per step.
extern crate alloc;

use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_NETWORK, SLOT_PARSE, SLOT_TLS};
use mind::http::{self, Connection, Method, Transport, Url};
use mind::idl::{socket, tls};
use mind::ipc::Endpoint;

mind::request!(REQUEST_CONSOLE | REQUEST_NETWORK | REQUEST_PARSE | REQUEST_TLS);

const STACK: Endpoint = Endpoint(SLOT_NETWORK);
const TLS: Endpoint = Endpoint(SLOT_TLS);

fn now() -> usize { mind::time::uptime_ms() }

// TCP of the flow grant, or a TLS session over it; a receive waits up to `idle_ms` for a byte (the caller's timeout).
enum Link { Tcp(u32), Tls(u32) }
struct Timed { link: Link, idle_ms: usize }

impl Transport for Timed {
    fn send(&mut self, data: &[u8]) -> Result<(), http::Error> {
        let (mut sent, deadline) = (0, now() + 15_000);
        while sent < data.len() {
            let end = data.len().min(sent + 4096);
            let n = match self.link {
                Link::Tcp(s) => match socket::tcp_send(STACK, s, &data[sent..end]) { Ok(Ok(n)) => n as usize, _ => return Err(http::Error::Transport) },
                Link::Tls(s) => match tls::send(TLS, s, &data[sent..end]) {
                    Ok(Ok(n)) => n as usize, Ok(Err(tls::Error::Again)) => 0, _ => return Err(http::Error::Transport),
                },
            };
            if n == 0 { if now() >= deadline { return Err(http::Error::Transport); } mind::time::sleep(5); }
            sent += n;
        }
        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, http::Error> {
        let deadline = now() + self.idle_ms;
        let length = buffer.len().min(4096);
        loop {
            let got = match self.link {
                Link::Tcp(s) => match socket::tcp_receive(STACK, s, length as u32, &mut buffer[..length]) {
                    Ok(Ok(n)) => n as usize, Ok(Err(socket::Error::Closed)) => return Ok(0), Ok(Err(socket::Error::Again)) => 0,
                    _ => return Err(http::Error::Transport),
                },
                Link::Tls(s) => match tls::receive(TLS, s, length as u32, &mut buffer[..length]) {
                    Ok(Ok(n)) => n as usize, Ok(Err(tls::Error::Closed)) => return Ok(0), Ok(Err(tls::Error::Again)) => 0,
                    _ => return Err(http::Error::Transport),
                },
            };
            if got > 0 { return Ok(got); }
            if now() >= deadline { return Err(http::Error::Transport); }
            mind::time::sleep(5);
        }
    }
}

impl Drop for Timed {
    fn drop(&mut self) { let _ = match self.link { Link::Tcp(s) => socket::close(STACK, s).map(drop), Link::Tls(s) => tls::close(TLS, s).map(drop) }; }
}

fn fail(what: core::fmt::Arguments) -> ! {
    mind::println!("WEBCHECK: {}", what);
    mind::process::exit_with(1)
}

fn ipv4(text: &str) -> Option<u32> {
    let mut address = [0u8; 4]; let mut parts = text.split('.');
    for byte in address.iter_mut() { *byte = parts.next()?.parse().ok()?; }
    parts.next().is_none().then_some(u32::from_be_bytes(address))
}

// Reads a body to its end: its length and as many of its first bytes as `first` holds.
fn body<T: Transport>(response: &mut http::Response<'_, T>, first: &mut [u8]) -> Result<(usize, usize), http::Error> {
    let (mut total, mut kept) = (0, 0);
    let mut buffer = [0u8; 2048];
    loop {
        let n = response.read(&mut buffer)?;
        if n == 0 { return Ok((total, kept)); }
        let take = n.min(first.len() - kept);
        first[kept..kept + take].copy_from_slice(&buffer[..take]);
        kept += take;
        total += n;
    }
}

fn shown(bytes: &[u8]) -> &str { core::str::from_utf8(&bytes[..bytes.len().min(64)]).unwrap_or("(not UTF-8)") }

// A JSON body as the parser service reads it (parse.wit 1.4, 750-NET-0021): one line per value, or its refusal.
fn show_json(text: &[u8]) {
    let mut start = 0;
    loop {
        match mind::idl::parse::json(Endpoint(SLOT_PARSE), text, start) {
            Ok(Ok(page)) => {
                for (i, v) in page.page.as_slice().iter().enumerate() {
                    let parent = if v.parent == u16::MAX { -1 } else { v.parent as i32 };
                    mind::println!("WEBCHECK JSON {} {:?} IN {} KEY {:?} TEXT {:?}", start as usize + i, v.kind, parent, v.key.as_str(), v.text.as_str());
                }
                start += page.page.as_slice().len() as u32;
                if start >= page.values || page.page.as_slice().is_empty() { mind::println!("WEBCHECK JSON {} VALUES", page.values); return; }
            }
            Ok(Err(error)) => { mind::println!("WEBCHECK JSON REFUSED: {:?}", error); return; }
            Err(_) => { mind::println!("WEBCHECK JSON: NO PARSER"); return; }
        }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("webcheck — the network track's check of HTTP for services: a POST, requests on one connection, a long stream, pinned servers.\nUsage: webcheck URL [--post JSON] [--repeat N] [--stream] [--idle SECONDS] [--pin leaf|spki:HEX]... [--name NAME] [--json]");
    let mut words = mind::process::args_str().split_whitespace();
    let Some(url) = words.next() else { fail(format_args!("USAGE: webcheck URL [--post JSON] [--repeat N] [--stream] [--idle SECONDS]")) };
    let (mut post, mut repeat, mut stream, mut idle, mut as_json) = (None, 0u32, false, 15usize, false);
    let (mut pins, mut name) = (mind::idl::codec::List::<tls::Pin, 2>::default(), None);
    while let Some(word) = words.next() {
        match word {
            "--pin" => {
                let pin = words.next().and_then(|p| p.split_once(':')).and_then(|(kind, hex)| {
                    let kind = match kind { "leaf" => tls::PinKind::Leaf, "spki" => tls::PinKind::Spki, _ => return None };
                    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) { return None; }
                    let mut digest = mind::idl::codec::List::default();
                    for i in 0..32 { digest.push(u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?); }
                    Some(tls::Pin { kind, digest })
                });
                match pin { Some(pin) if pins.push(pin) => {} _ => fail(format_args!("--pin leaf|spki:HEX, at most two")) }
            }
            "--name" => name = words.next(),
            "--post" => post = words.next(),
            "--repeat" => repeat = words.next().and_then(|n| n.parse().ok()).unwrap_or(0),
            "--stream" => stream = true,
            "--json" => as_json = true,
            "--idle" => idle = words.next().and_then(|n| n.parse().ok()).unwrap_or(15),
            _ => fail(format_args!("UNKNOWN OPTION {}", word)),
        }
    }
    let Ok(url) = Url::parse(url) else { fail(format_args!("NOT AN http:// OR https:// URL")) };
    if mind::dev::cap_info(SLOT_NETWORK).0 != CAP_KIND_ENDPOINT { fail(format_args!("NO NETWORK GRANT")); }
    if mind::dev::cap_info(SLOT_PARSE).0 != CAP_KIND_ENDPOINT { fail(format_args!("NO PARSER SERVICE")); }
    let Some(address) = ipv4(url.host) else { fail(format_args!("A.B.C.D HOSTS ONLY")) };
    let link = if url.https {
        let session = match tls::attach(TLS, SLOT_NETWORK) { Ok(Ok(s)) => s, other => fail(format_args!("TLS: {:?}", other.map_err(|_| ()))) };
        let name = name.unwrap_or(url.host);
        let connected = if pins.as_slice().is_empty() { tls::connect(TLS, session, name, address, url.port, false, 10_000) }
                        else { tls::connect_pins(TLS, session, name, address, url.port, pins.as_slice(), 10_000) };
        match connected {
            Ok(Ok(_)) => Link::Tls(session),
            other => { let _ = tls::close(TLS, session); fail(format_args!("TLS CONNECT: {:?}", other.map_err(|_| ()))) }
        }
    } else {
        match socket::tcp_connect(STACK, address, url.port, 10_000) { Ok(Ok(s)) => Link::Tcp(s), other => fail(format_args!("CONNECT: {:?}", other.map_err(|_| ()))) }
    };
    let mut connection = Connection::new(Timed { link, idle_ms: 15_000 });
    let mut parser = mind::parse::Service(Endpoint(SLOT_PARSE));
    // A body is kept up to what the parser service's `json` takes (bytes<16384>), on the heap: a page of `json`'s answer
    // takes much of the 64 KiB stack.
    let mut first = alloc::vec![0u8; 16384];
    if let Some(json) = post {
        let mut source = json.as_bytes();
        let headers = [("Content-Type", "application/json"), ("Authorization", "Bearer webcheck")];
        let mut response = connection.request(Method::Post, &url, &headers, Some((json.len() as u64, &mut source)), &mut parser, false)
            .unwrap_or_else(|e| fail(format_args!("POST: {:?}", e)));
        let status = response.status;
        let (total, kept) = body(&mut response, &mut first).unwrap_or_else(|e| fail(format_args!("POST BODY: {:?}", e)));
        mind::println!("WEBCHECK POST {} {} BYTES {}", status, total, shown(&first[..kept]));
        if as_json { show_json(&first[..kept]); }
    }
    for i in 0..repeat {
        let mut response = connection.request(Method::Get, &url, &[], None, &mut parser, false)
            .unwrap_or_else(|e| fail(format_args!("GET {}: {:?}", i + 1, e)));
        if response.status != 200 { fail(format_args!("GET {}: STATUS {}", i + 1, response.status)); }
        body(&mut response, &mut first).unwrap_or_else(|e| fail(format_args!("GET {} BODY: {:?}", i + 1, e)));
    }
    if repeat > 0 { mind::println!("WEBCHECK {} REQUESTS ON ONE CONNECTION", connection.served()); }
    if post.is_none() && repeat == 0 && !stream {
        let mut response = connection.request(Method::Get, &url, &[], None, &mut parser, true).unwrap_or_else(|e| fail(format_args!("GET: {:?}", e)));
        let status = response.status;
        let (total, kept) = body(&mut response, &mut first).unwrap_or_else(|e| fail(format_args!("GET BODY: {:?}", e)));
        mind::println!("WEBCHECK GET {} {} BYTES {}", status, total, shown(&first[..kept]).trim_end());
        if as_json { show_json(&first[..kept]); }
    }
    if stream {
        connection.transport().idle_ms = idle * 1000;
        let started = now();
        let mut response = connection.request(Method::Get, &url, &[("X-Stream", "1")], None, &mut parser, false)
            .unwrap_or_else(|e| fail(format_args!("STREAM: {:?}", e)));
        let (total, kept) = body(&mut response, &mut first).unwrap_or_else(|e| fail(format_args!("STREAM BODY: {:?}", e)));
        mind::println!("WEBCHECK STREAM {} BYTES IN {} S {}", total, (now() - started) / 1000, shown(&first[..kept]));
    }
    mind::println!("WEBCHECK DONE");
}
