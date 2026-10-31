#![no_std]
#![no_main]
// webcheck URL [--post JSON] [--repeat N] [--stream] [--idle SECONDS] [--pin leaf|spki:HEX]... [--name NAME] [--client-cert]: the network
// track's check of HTTP for services (750-NET-0019). Over one connection (TCP, or a session of the TLS service for
// https) it posts JSON to URL and reads the response, chunked or not; then GETs URL N more times on the same
// connection; with --stream it GETs URL once more and reads its body to its end, however long the server holds it, up
// to --idle seconds without a byte; with none of these, one GET. With one or two --pin, https trusts the server by
// them and the TLS service still checks its certificate for NAME (default: the host) (tls.wit 1.2, 750-NET-0020).
// Response heads go to the parser service, as download's do; with --json, a JSON body too, one line per value
// (parse.wit 1.4, 750-NET-0021). With --client-cert, https asks the TLS service to present the device certificate, which a
// client it lends without that right is refused (351-NET-0022). With --api, the checks of HTTP for API clients against
// tests/http_api_stub.py at URL (000-NET-0047): a JSON POST, a multipart upload, redirect chains, gzip and deflate
// bodies and a bomb, 1000 Server-Sent Events, a chunked upload, Retry-After. One line per step.
extern crate alloc;

use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_NETWORK, SLOT_PARSE, SLOT_TLS};
use alloc::string::String;
use alloc::vec::Vec;
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

// A connection of its own for each API check, over a TLS session for https.
fn open(url: &Url) -> Connection<Timed> {
    let Some(address) = ipv4(url.host) else { fail(format_args!("A.B.C.D HOSTS ONLY")) };
    let link = if url.https {
        let session = match tls::attach(TLS, SLOT_NETWORK) { Ok(Ok(s)) => s, other => fail(format_args!("TLS: {:?}", other.map_err(|_| ()))) };
        match tls::connect(TLS, session, url.host, address, url.port, false, 10_000) {
            Ok(Ok(_)) => Link::Tls(session),
            other => { let _ = tls::close(TLS, session); fail(format_args!("TLS CONNECT: {:?}", other.map_err(|_| ()))) }
        }
    } else {
        match socket::tcp_connect(STACK, address, url.port, 10_000) { Ok(Ok(s)) => Link::Tcp(s), other => fail(format_args!("CONNECT: {:?}", other.map_err(|_| ()))) }
    };
    Connection::new(Timed { link, idle_ms: 15_000 })
}

fn hex(digest: &[u8]) -> String {
    use core::fmt::Write;
    let mut text = String::new();
    for byte in digest { let _ = write!(text, "{:02x}", byte); }
    text
}

// The whole body as text, up to 4 KiB.
fn text<T: Transport>(response: &mut http::Response<'_, T>) -> String {
    let mut first = [0u8; 4096];
    let (_, kept) = body(response, &mut first).unwrap_or_else(|e| fail(format_args!("BODY: {:?}", e)));
    String::from(core::str::from_utf8(&first[..kept]).unwrap_or("(not UTF-8)"))
}

// GET or POST `path` and follow its redirects within this host, a new connection for each: the status that ended it,
// how many redirects were followed, and the last body.
fn follow(base: &Url, method: Method, path: &str, parser: &mut mind::parse::Service) -> Result<(u16, u32, String), http::Error> {
    let mut current = String::from(path);
    let (mut method, mut followed) = (method, 0u32);
    loop {
        let url = if current.starts_with('/') { Url { path: &current, ..*base } } else { Url::parse(&current)? };
        let mut connection = open(&url);
        let mut empty: &[u8] = b"";
        let body = (method == Method::Post).then_some((0u64, &mut empty as &mut dyn http::Source));
        let mut response = connection.request(method, &url, &[], body, parser, false)?;
        let head = response.head;
        let answer = text(&mut response);
        let mut next = [0u8; 2048];
        match http::redirect(&url, method, &head, followed, &mut next)? {
            None => return Ok((head.status, followed, answer)),
            Some(r) => {
                if !r.same_host { return Err(http::Error::Redirect); }
                let to = core::str::from_utf8(&next[..r.length]).unwrap_or("");
                let path = to.split_once("://").map_or(to, |(_, rest)| &rest[rest.find('/').unwrap_or(rest.len())..]);
                mind::println!("WEBCHECK API REDIRECT {} {} TO {}", head.status, r.method.name(), path);
                current = String::from(to);
                method = r.method;
                followed += 1;
            }
        }
    }
}

// The checks of HTTP for API clients against tests/http_api_stub.py at `base` (000-NET-0047), each in a function of
// its own so that their connections' buffers share the stack.
fn api(base: &Url, parser: &mut mind::parse::Service) {
    api_json(base, parser);
    api_multipart(base, parser);
    api_streamed(base, parser);
    api_redirects(base, parser);
    for path in ["/api/gzip", "/api/deflate"] { api_coded(base, path, parser); }
    api_bomb(base, parser);
    api_events(base, parser);
    for path in ["/api/busy", "/api/limited"] { api_retry(base, path, parser); }
    api_methods(base, parser);
}

// A JSON POST, as a language model's API takes one.
#[inline(never)]
fn api_json(base: &Url, parser: &mut mind::parse::Service) {
    let chat = "{\"model\":\"mind-test\",\"messages\":[{\"role\":\"user\",\"content\":\"hello from MIND\"}],\"max_tokens\":16}";
    let url = Url { path: "/api/chat", ..*base };
    let mut connection = open(&url);
    let mut source = chat.as_bytes();
    let mut response = connection.request(Method::Post, &url, &[("Content-Type", "application/json"), ("Accept", "application/json")], Some((chat.len() as u64, &mut source)), parser, false)
        .unwrap_or_else(|e| fail(format_args!("API JSON: {:?}", e)));
    let (status, media) = (response.status, response.head.media);
    mind::println!("WEBCHECK API JSON {} {} {}", status, media.as_str(), text(&mut response));
}

// A file's bytes made from their offsets, given in pieces of at most 1500.
struct Pattern { at: usize, length: usize }
impl http::Source for Pattern {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, http::Error> {
        let n = buffer.len().min(self.length - self.at).min(1500);
        for (i, b) in buffer[..n].iter_mut().enumerate() { *b = ((self.at + i) * 7 % 251) as u8; }
        self.at += n;
        Ok(n)
    }
}

// A multipart upload: a field and a 100 000-byte file streamed in pieces.
#[inline(never)]
fn api_multipart(base: &Url, parser: &mut mind::parse::Service) {
    let length = 100_000;
    let data: Vec<u8> = (0..length).map(|i| (i * 7 % 251) as u8).collect();
    mind::println!("WEBCHECK API FILE SHA256 {}", hex(&mind::sha256::digest(&data)));
    let mut random = [0u8; 16];
    if !mind::random::fill(&mut random) { random = (mind::time::monotonic_ns() as u128).to_le_bytes(); }
    let mut file = Pattern { at: 0, length };
    let mut parts = [
        mind::multipart::Part::Field { name: "chat_id", value: b"42" },
        mind::multipart::Part::File { name: "document", filename: "data.bin", media: "application/octet-stream", length: length as u64, source: &mut file },
    ];
    let mut multipart = mind::multipart::Multipart::new(&mut parts, random).unwrap_or_else(|e| fail(format_args!("API MULTIPART: {:?}", e)));
    let mut kind = String::from("multipart/form-data; boundary=");
    kind.push_str(multipart.boundary());
    let url = Url { path: "/api/upload", ..*base };
    let mut connection = open(&url);
    let total = multipart.length();
    let mut response = connection.request(Method::Post, &url, &[("Content-Type", &kind)], Some((total, &mut multipart)), parser, false)
        .unwrap_or_else(|e| fail(format_args!("API MULTIPART: {:?}", e)));
    let status = response.status;
    mind::println!("WEBCHECK API MULTIPART {} {} BYTES {}", status, total, text(&mut response));
}

// A body of unknown length, sent chunked.
#[inline(never)]
fn api_streamed(base: &Url, parser: &mut mind::parse::Service) {
    let url = Url { path: "/api/stream", ..*base };
    let mut connection = open(&url);
    let mut file = Pattern { at: 0, length: 50_000 };
    let mut response = connection.request_streamed(Method::Post, &url, &[("Content-Type", "application/octet-stream")], &mut file, parser, false)
        .unwrap_or_else(|e| fail(format_args!("API STREAMED: {:?}", e)));
    let status = response.status;
    mind::println!("WEBCHECK API STREAMED {} {}", status, text(&mut response));
}

// Redirects: a chain of four kinds, a 303 after a POST, a loop, and a way down to plain HTTP.
#[inline(never)]
fn api_redirects(base: &Url, parser: &mut mind::parse::Service) {
    match follow(base, Method::Get, "/api/r/4", parser) {
        Ok((status, n, answer)) => mind::println!("WEBCHECK API REDIRECTS {} THEN {} {}", n, status, answer),
        Err(e) => mind::println!("WEBCHECK API REDIRECTS: {:?}", e),
    }
    match follow(base, Method::Post, "/api/see-other", parser) {
        Ok((status, n, answer)) => mind::println!("WEBCHECK API SEE OTHER {} THEN {} {}", n, status, answer),
        Err(e) => mind::println!("WEBCHECK API SEE OTHER: {:?}", e),
    }
    mind::println!("WEBCHECK API LOOP {:?}", follow(base, Method::Get, "/api/loop", parser).map(|r| r.0));
    mind::println!("WEBCHECK API DOWN {:?}", follow(base, Method::Get, "/api/down", parser).map(|r| r.0));
}

// A coded body decoded as it arrives.
#[inline(never)]
fn api_coded(base: &Url, path: &'static str, parser: &mut mind::parse::Service) {
    let url = Url { path, ..*base };
    let mut connection = open(&url);
    let mut response = connection.request(Method::Get, &url, &[("Accept-Encoding", "gzip, deflate")], None, parser, false)
        .unwrap_or_else(|e| fail(format_args!("API {}: {:?}", path, e)));
    let (coded, encoding) = (response.length.unwrap_or(0), response.head.encoding);
    let mut hasher = mind::sha256::Sha256::new();
    let decoded = mind::http_coding::decode(&mut response, 1 << 20, &mut |piece| { hasher.update(piece); true });
    mind::println!("WEBCHECK API {} {:?} {} BYTES DECODED {:?} SHA256 {}", &path[5..], encoding, coded, decoded, hex(&hasher.finish()));
}

// A bomb stopped at 1 MiB.
#[inline(never)]
fn api_bomb(base: &Url, parser: &mut mind::parse::Service) {
    let url = Url { path: "/api/bomb", ..*base };
    let mut connection = open(&url);
    let mut response = connection.request(Method::Get, &url, &[("Accept-Encoding", "gzip")], None, parser, false)
        .unwrap_or_else(|e| fail(format_args!("API BOMB: {:?}", e)));
    let mut seen = 0u64;
    let decoded = mind::http_coding::decode(&mut response, 1 << 20, &mut |piece| { seen += piece.len() as u64; true });
    mind::println!("WEBCHECK API BOMB {:?} AFTER {} BYTES", decoded, seen);
}

// Server-Sent Events: 1000 in order, then the last.
#[inline(never)]
fn api_events(base: &Url, parser: &mut mind::parse::Service) {
    let url = Url { path: "/api/events", ..*base };
    let mut connection = open(&url);
    connection.transport().idle_ms = 60_000;
    let mut response = connection.request(Method::Get, &url, &[("Accept", "text/event-stream")], None, parser, false)
        .unwrap_or_else(|e| fail(format_args!("API EVENTS: {:?}", e)));
    let media = response.head.media;
    let mut stream = mind::sse::Stream::new(4096);
    let (mut deltas, mut in_order, mut done) = (0usize, true, false);
    let mut buffer = [0u8; 2048];
    loop {
        let n = response.read(&mut buffer).unwrap_or_else(|e| fail(format_args!("API EVENTS BODY: {:?}", e)));
        if n == 0 { break; }
        stream.feed(&buffer[..n], &mut |event| {
            if event.kind == "delta" {
                in_order &= event.data == alloc::format!("{{\"n\":{},\n\"text\":\"piece {}\"}}", deltas, deltas) && event.id == alloc::format!("{}", deltas);
                deltas += 1;
            } else if event.kind == "done" && event.data == "[DONE]" { done = true; }
            true
        }).unwrap_or_else(|e| fail(format_args!("API EVENTS: {:?}", e)));
    }
    mind::println!("WEBCHECK API EVENTS {} {} DELTAS IN ORDER {} DONE {} LAST ID {} RETRY {:?}", media.as_str(), deltas, in_order, done, stream.last_id(), stream.retry());
}

// Retry-After, in seconds or as a date: given to the caller, nothing repeated.
#[inline(never)]
fn api_retry(base: &Url, path: &'static str, parser: &mut mind::parse::Service) {
    let url = Url { path, ..*base };
    let mut connection = open(&url);
    let mut response = connection.request(Method::Get, &url, &[], None, parser, false).unwrap_or_else(|e| fail(format_args!("API {}: {:?}", path, e)));
    let (status, retry) = (response.status, response.head.retry_after);
    let _ = text(&mut response);
    mind::println!("WEBCHECK API {} {} RETRY-AFTER {:?}", &path[5..], status, retry);
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("webcheck — the network track's check of HTTP for services: a POST, requests on one connection, a long stream, pinned servers.\nUsage: webcheck URL [--post JSON] [--repeat N] [--stream] [--idle SECONDS] [--pin leaf|spki:HEX]... [--name NAME] [--json] [--client-cert] [--api]");
    let mut words = mind::process::args_str().split_whitespace();
    let Some(url) = words.next() else { fail(format_args!("USAGE: webcheck URL [--post JSON] [--repeat N] [--stream] [--idle SECONDS]")) };
    let (mut post, mut repeat, mut stream, mut idle, mut as_json, mut client_certificate) = (None, 0u32, false, 15usize, false, false);
    let (mut pins, mut name, mut api_checks) = (mind::idl::codec::List::<tls::Pin, 2>::default(), None, false);
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
            "--client-cert" => client_certificate = true,
            "--api" => api_checks = true,
            "--idle" => idle = words.next().and_then(|n| n.parse().ok()).unwrap_or(15),
            _ => fail(format_args!("UNKNOWN OPTION {}", word)),
        }
    }
    let Ok(url) = Url::parse(url) else { fail(format_args!("NOT AN http:// OR https:// URL")) };
    if mind::dev::cap_info(SLOT_NETWORK).0 != CAP_KIND_ENDPOINT { fail(format_args!("NO NETWORK GRANT")); }
    if mind::dev::cap_info(SLOT_PARSE).0 != CAP_KIND_ENDPOINT { fail(format_args!("NO PARSER SERVICE")); }
    if api_checks {
        api(&url, &mut mind::parse::Service(Endpoint(SLOT_PARSE)));
        mind::println!("WEBCHECK DONE");
        return;
    }
    let Some(address) = ipv4(url.host) else { fail(format_args!("A.B.C.D HOSTS ONLY")) };
    let link = if url.https {
        let session = match tls::attach(TLS, SLOT_NETWORK) { Ok(Ok(s)) => s, other => fail(format_args!("TLS: {:?}", other.map_err(|_| ()))) };
        let name = name.unwrap_or(url.host);
        let connected = if pins.as_slice().is_empty() { tls::connect(TLS, session, name, address, url.port, client_certificate, 10_000) }
                        else { tls::connect_pins(TLS, session, name, address, url.port, pins.as_slice(), 10_000) };
        match connected {
            Ok(Ok(peer)) => { if peer.client_certificate { mind::println!("WEBCHECK TLS: THE DEVICE CERTIFICATE SENT"); } Link::Tls(session) }
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

// HEAD, PATCH and DELETE on one connection: HEAD's answers end at their heads, though they name a length and a chunked
// coding, and the requests after them are answered in order.
#[inline(never)]
fn api_methods(base: &Url, parser: &mut mind::parse::Service) {
    let url = Url { path: "/api/echo", ..*base };
    let mut connection = open(&url);
    for (method, path) in [(Method::Head, "/api/echo"), (Method::Head, "/api/events"), (Method::Get, "/api/final")] {
        let url = Url { path, ..*base };
        let mut response = connection.request(method, &url, &[], None, parser, false).unwrap_or_else(|e| fail(format_args!("API {} {}: {:?}", method.name(), path, e)));
        let (status, length, chunked) = (response.status, response.head.length, response.head.chunked);
        mind::println!("WEBCHECK API {} {} {} LENGTH {:?} CHUNKED {} BODY [{}]", method.name(), &path[5..], status, length, chunked, text(&mut response));
    }
    let patch = "{\"name\":\"mind\"}";
    let mut source = patch.as_bytes();
    let mut response = connection.request(Method::Patch, &url, &[("Content-Type", "application/merge-patch+json")], Some((patch.len() as u64, &mut source)), parser, false)
        .unwrap_or_else(|e| fail(format_args!("API PATCH: {:?}", e)));
    let status = response.status;
    mind::println!("WEBCHECK API PATCH {} {}", status, text(&mut response));
    let mut response = connection.request(Method::Delete, &url, &[], None, parser, false).unwrap_or_else(|e| fail(format_args!("API DELETE: {:?}", e)));
    let status = response.status;
    mind::println!("WEBCHECK API DELETE {} {}", status, text(&mut response));
    let item = Url { path: "/api/item", ..*base };
    let mut response = connection.request(Method::Delete, &item, &[], None, parser, false).unwrap_or_else(|e| fail(format_args!("API DELETE item: {:?}", e)));
    let status = response.status;
    let body = text(&mut response);
    mind::println!("WEBCHECK API DELETE item {} BODY [{}] ONE CONNECTION {}", status, body, connection.served());
}

