#![no_std]
#![no_main]
// get (000-NET-0048): an analog of wget and curl, a process of its own. One request, its method, headers and body
// (`-d TEXT`, `-d @FILE`, `-F` multipart), redirects followed by their rules, the body to the console or a file (`-o`,
// `-O` into ram:, `-c` resuming it), `-I` the head alone, `-w` the status and the time, `-f` failing on an HTTP error,
// `--max-time`; exit statuses as curl's (args.rs). HTTPS through the launcher's TLS client (BADGE_TLS_NO_DEVICE: never
// as the device); plain HTTP only with --insecure-http, said on the console. It reaches only what its flow grant names,
// a line of the network policy for `get` (a host asked for at run time is 000-NET-0049's remainder), and writes only the
// user's files its launcher lends (REQUEST_FILES). `-I` asks with GET and reads no body (HEAD waits for 000-NET-0047).
extern crate alloc;

mod args;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use args::{Data, Form, Options, Output};
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_FILE, SLOT_NETWORK, SLOT_PARSE, SLOT_TLS};
use mind::fs::File;
use mind::http::{self, Connection, Method, Source, Transport, Url};
use mind::idl::{socket, tls};
use mind::ipc::Endpoint;
use mind::multipart::{Multipart, Part};

mind::request!(REQUEST_CONSOLE | REQUEST_NETWORK | REQUEST_PARSE | REQUEST_TLS | REQUEST_FILES);

const STACK: Endpoint = Endpoint(SLOT_NETWORK);
const TLS: Endpoint = Endpoint(SLOT_TLS);
const IDLE_MS: usize = 30_000; // a receive with nothing for this long fails
const BODY_MAX: u64 = 1 << 40; // the decoded body's bound (a coded body that grows without end stops here)

// An exit status and why.
struct Fail(i32, String);

fn now() -> usize { mind::time::uptime_ms() }

// TCP of the flow grant, or a TLS session over it; each wait ends at the idle limit or the overall deadline.
enum Link { Tcp(u32), Tls(u32) }
struct Timed { link: Link, deadline: usize }

impl Timed {
    fn until(&self, ms: usize) -> usize { (now() + ms).min(self.deadline) }
}

impl Transport for Timed {
    fn send(&mut self, data: &[u8]) -> Result<(), http::Error> {
        let (mut sent, until) = (0, self.until(IDLE_MS));
        while sent < data.len() {
            let end = data.len().min(sent + 4096);
            let n = match self.link {
                Link::Tcp(s) => match socket::tcp_send(STACK, s, &data[sent..end]) { Ok(Ok(n)) => n as usize, _ => return Err(http::Error::Transport) },
                Link::Tls(s) => match tls::send(TLS, s, &data[sent..end]) { Ok(Ok(n)) => n as usize, Ok(Err(tls::Error::Again)) => 0, _ => return Err(http::Error::Transport) },
            };
            if n == 0 { if now() >= until { return Err(http::Error::Transport); } mind::time::sleep(5); }
            sent += n;
        }
        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, http::Error> {
        let (until, length) = (self.until(IDLE_MS), buffer.len().min(4096));
        loop {
            let got = match self.link {
                Link::Tcp(s) => match socket::tcp_receive(STACK, s, length as u32, &mut buffer[..length]) {
                    Ok(Ok(n)) => n as usize, Ok(Err(socket::Error::Closed)) => return Ok(0), Ok(Err(socket::Error::Again)) => 0, _ => return Err(http::Error::Transport),
                },
                Link::Tls(s) => match tls::receive(TLS, s, length as u32, &mut buffer[..length]) {
                    Ok(Ok(n)) => n as usize, Ok(Err(tls::Error::Closed)) => return Ok(0), Ok(Err(tls::Error::Again)) => 0, _ => return Err(http::Error::Transport),
                },
            };
            if got > 0 { return Ok(got); }
            if now() >= until { return Err(http::Error::Transport); }
            mind::time::sleep(5);
        }
    }
}

impl Drop for Timed {
    fn drop(&mut self) { let _ = match self.link { Link::Tcp(s) => socket::close(STACK, s).map(drop), Link::Tls(s) => tls::close(TLS, s).map(drop) }; }
}

fn ipv4(text: &str) -> Option<u32> {
    let mut address = [0u8; 4];
    let mut parts = text.split('.');
    for byte in address.iter_mut() { *byte = parts.next()?.parse().ok()?; }
    parts.next().is_none().then_some(u32::from_be_bytes(address))
}

// The host's address: as written, or the stack's DNS answer (the grant must allow DNS).
fn resolve(host: &str) -> Result<u32, Fail> {
    if let Some(address) = ipv4(host) { return Ok(address); }
    match socket::resolve(STACK, host, 0, 0, 5000) {
        Ok(Ok(address)) => Ok(address),
        Ok(Err(error)) => Err(Fail(args::NO_HOST, format!("{}: NOT RESOLVED ({:?})", host, error))),
        Err(_) => Err(Fail(args::NO_HOST, format!("{}: NO NETWORK GRANT", host))),
    }
}

fn connect(url: &Url, deadline: usize) -> Result<Connection<Timed>, Fail> {
    let address = resolve(url.host)?;
    let wait = (deadline.saturating_sub(now())).clamp(1, 10_000) as u32;
    let link = if url.https {
        let session = match tls::attach(TLS, SLOT_NETWORK) {
            Ok(Ok(session)) => session,
            Ok(Err(error)) => return Err(Fail(args::TLS_ERROR, format!("TLS: {:?}", error))),
            Err(_) => return Err(Fail(args::TLS_ERROR, String::from("NO TLS CLIENT (THE LAUNCHER LENT NONE)"))),
        };
        let failed = match tls::connect(TLS, session, url.host, address, url.port, false, wait) {
            Ok(Ok(_)) => None,
            Ok(Err(tls::Error::Certificate)) => Some(Fail(args::CERTIFICATE, format!("{}: CERTIFICATE REFUSED", url.host))),
            Ok(Err(tls::Error::Timeout)) => Some(Fail(args::TIMEOUT, format!("{}: NO ANSWER", url.host))),
            Ok(Err(error @ (tls::Error::Refused | tls::Error::Denied | tls::Error::NoNetwork))) => Some(Fail(args::NO_CONNECTION, format!("{}:{}: {:?}", url.host, url.port, error))),
            Ok(Err(error)) => Some(Fail(args::TLS_ERROR, format!("{}: TLS {:?}", url.host, error))),
            Err(error) => Some(Fail(args::TLS_ERROR, format!("{}: TLS {:?}", url.host, error))),
        };
        if let Some(fail) = failed { let _ = tls::close(TLS, session); return Err(fail); }
        Link::Tls(session)
    } else {
        match socket::tcp_connect(STACK, address, url.port, wait) {
            Ok(Ok(socket)) => Link::Tcp(socket),
            Ok(Err(socket::Error::Timeout)) => return Err(Fail(args::TIMEOUT, format!("{}: NO ANSWER", url.host))),
            Ok(Err(error)) => return Err(Fail(args::NO_CONNECTION, format!("{}:{}: {:?}", url.host, url.port, error))),
            Err(_) => return Err(Fail(args::NO_CONNECTION, format!("{}: NO NETWORK GRANT", url.host))),
        }
    };
    Ok(Connection::new(Timed { link, deadline }))
}

fn method(name: &str) -> Method { match name { "POST" => Method::Post, "PUT" => Method::Put, _ => Method::Get } }

// A file to send, read a piece at a time.
struct FileSource { file: File, at: usize }

impl Source for FileSource {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, http::Error> {
        let n = self.file.read_at(self.at, buffer).map_err(|_| http::Error::Body)?;
        self.at += n;
        Ok(n)
    }
}

fn open(path: &str) -> Result<File, Fail> { File::open(path).map_err(|e| Fail(args::READ_ERROR, format!("{}: {:?}", path, e))) }

fn media(path: &str) -> &'static str {
    let lower = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match lower.as_str() {
        "txt" => "text/plain", "html" | "htm" => "text/html", "json" => "application/json", "pdf" => "application/pdf",
        "png" => "image/png", "jpg" | "jpeg" => "image/jpeg", "csv" => "text/csv", _ => "application/octet-stream",
    }
}

// What came back: its head and how much of its body.
struct Answer { head: http::Head, bytes: u64 }

// Headers that carry the user's credentials: not sent on to another host after a redirect (as curl without
// --location-trusted).
const CREDENTIALS: [&str; 3] = ["authorization", "cookie", "proxy-authorization"];

// One request on `url` with the options' headers and body (built anew for each attempt, as a redirect may resend it),
// and what came back; a redirect is returned for the caller to follow. `trusted`: still the host the user named.
fn exchange(o: &Options, url: &Url, method_now: Method, with_body: bool, trusted: bool, offset: u64, deadline: usize,
            sink: &mut dyn FnMut(&http::Head, &[u8]) -> Result<(), Fail>, parser: &mut mind::parse::Service, next: &mut [u8], followed: u32)
            -> Result<(Answer, Option<http::Redirected>), Fail> {
    let mut connection = connect(url, deadline)?;
    let mut headers: Vec<(&str, &str)> = o.headers.iter().filter(|(n, _)| trusted || !CREDENTIALS.iter().any(|c| n.eq_ignore_ascii_case(c)))
        .map(|(n, v)| (n.as_str(), v.as_str())).collect();
    let range = format!("bytes={}-", offset);
    if offset > 0 { headers.push(("Range", &range)); }
    let has_type = o.headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("content-type"));
    let fail_http = |e: http::Error| -> Fail {
        match e {
            _ if now() >= deadline => Fail(args::TIMEOUT, String::from("THE TIME IS UP (--max-time)")),
            http::Error::Body => Fail(args::READ_ERROR, String::from("A FILE TO SEND ENDED EARLY")),
            http::Error::Redirect => Fail(args::TOO_MANY_REDIRECTS, String::from("A REDIRECT NOT FOLLOWED (NO LOCATION, OR FROM HTTPS TO HTTP)")),
            http::Error::Closed | http::Error::Transport => Fail(args::RECEIVE_ERROR, String::from("THE CONNECTION ENDED BEFORE THE WHOLE ANSWER (-c GOES ON WITH A SAVED FILE)")),
            other => Fail(args::RECEIVE_ERROR, format!("THE ANSWER: {:?}", other)),
        }
    };
    // The body, when this attempt carries one.
    let mut text_body: &[u8] = b"";
    let mut file_source = None;
    let mut file_sources: Vec<(String, FileSource)> = Vec::new();
    let mut body: Option<(u64, &mut dyn Source)> = None;
    let mut boundary_type = String::new();
    let mut parts: Vec<Part> = Vec::new();
    let mut multipart = None;
    if with_body {
        match &o.data {
            Some(Data::Text(t)) => text_body = t.as_bytes(),
            Some(Data::File(path)) => { let file = open(path)?; file_source = Some((file.size() as u64, FileSource { file, at: 0 })); }
            None => {}
        }
        for part in &o.form { if let Form::File { path, .. } = part { let file = open(path)?; file_sources.push((path.clone(), FileSource { file, at: 0 })); } }
        if !o.form.is_empty() {
            let mut files = file_sources.iter_mut();
            for part in &o.form {
                parts.push(match part {
                    Form::Field { name, value } => Part::Field { name, value: value.as_bytes() },
                    Form::File { name, .. } => {
                        let (path, source) = files.next().unwrap();
                        let length = source.file.size() as u64;
                        let filename = path.rsplit(['/', ':']).next().unwrap_or(path);
                        Part::File { name, filename, media: media(path), length, source }
                    }
                });
            }
            let mut random = [0u8; 16];
            mind::random::fill(&mut random);
            multipart = Some(Multipart::new(&mut parts, random).map_err(|_| Fail(args::USAGE, String::from("-F: A NAME OR FILE NAME THAT CANNOT BE SENT")))?);
        }
        if let Some(m) = multipart.as_mut() {
            boundary_type = format!("multipart/form-data; boundary={}", m.boundary());
            body = Some((m.length(), m as &mut dyn Source));
        } else if let Some((length, source)) = file_source.as_mut() {
            body = Some((*length, source as &mut dyn Source));
        } else if o.data.is_some() {
            body = Some((text_body.len() as u64, &mut text_body as &mut dyn Source));
        }
    }
    if !boundary_type.is_empty() { headers.push(("Content-Type", &boundary_type)); }
    else if o.data.is_some() && with_body && !has_type { headers.push(("Content-Type", "application/x-www-form-urlencoded")); }
    let mut response = connection.request(method_now, url, &headers, body, parser, true).map_err(fail_http)?;
    let head = response.head;
    if o.follow && matches!(head.status, 301 | 302 | 303 | 307 | 308) {
        if followed >= o.max_redirs { return Err(Fail(args::TOO_MANY_REDIRECTS, format!("MORE THAN {} REDIRECTS (--max-redirs)", o.max_redirs))); }
        if let Some(r) = http::redirect(url, method_now, &head, followed, next).map_err(fail_http)? { return Ok((Answer { head, bytes: 0 }, Some(r))); }
    }
    // The head alone, an error body -f does not want, or the end of a file already whole.
    if o.head || (o.fail && head.status >= 400) || (head.status == 416 && offset > 0) { return Ok((Answer { head, bytes: 0 }, None)); }
    let mut failed = None;
    let bytes = mind::http_coding::decode(&mut response, BODY_MAX, &mut |piece| match sink(&head, piece) { Ok(()) => true, Err(f) => { failed = Some(f); false } })
        .map_err(fail_http)?;
    if let Some(f) = failed { return Err(f); }
    Ok((Answer { head, bytes }, None))
}

// The body on the console: text as it comes, a body that is not UTF-8 counted only.
struct Console { carry: Vec<u8>, binary: bool, bytes: u64 }

impl Console {
    fn take(&mut self, piece: &[u8]) {
        self.bytes += piece.len() as u64;
        if self.binary { return; }
        self.carry.extend_from_slice(piece);
        match core::str::from_utf8(&self.carry) {
            Ok(text) => { mind::print!("{}", text); self.carry.clear(); }
            Err(e) if e.error_len().is_none() => {
                let valid = e.valid_up_to();
                mind::print!("{}", core::str::from_utf8(&self.carry[..valid]).unwrap_or(""));
                self.carry.drain(..valid);
            }
            Err(_) => self.binary = true,
        }
    }
}

fn run(o: &Options) -> Result<(), Fail> {
    let start = now();
    let deadline = o.max_time.map_or(usize::MAX, |s| start + s as usize * 1000);
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT { mind::fs::use_endpoint(Endpoint(SLOT_FILE)); }
    let first = Url::parse(&o.url).map_err(|_| Fail(args::BAD_URL, format!("{}: NOT AN http:// OR https:// URL", o.url)))?;
    let target = match &o.output {
        Output::Console => None,
        Output::File(path) => Some(path.clone()),
        Output::Remote => Some(format!("ram:{}", args::remote_name(first.path).ok_or_else(|| Fail(args::USAGE, String::from("-O: THE URL NAMES NO FILE")))?)),
    };
    // Resuming: the bytes already in the file are asked for no more.
    let mut file = None;
    let mut offset = 0u64;
    if let Some(path) = &target {
        if o.resume { if let Ok(f) = File::open_mode(path, mind::fs::MODE_WRITE) { offset = f.size() as u64; file = Some(f); } }
        if file.is_none() { file = Some(File::create(path).map_err(|e| Fail(args::WRITE_ERROR, format!("{}: {:?}", path, e)))?); }
    }
    let mut parser = mind::parse::Service(Endpoint(SLOT_PARSE));
    let mut current = o.url.clone();
    let (mut method_now, mut with_body, mut trusted, mut followed) = (method(o.method()), true, true, 0u32);
    let mut console = Console { carry: Vec::new(), binary: false, bytes: 0 };
    let mut written = offset;
    let mut started = false;
    loop {
        let url = Url::parse(&current).map_err(|_| Fail(args::BAD_URL, format!("{}: NOT AN http:// OR https:// URL", current)))?;
        if !url.https && !o.insecure_http { return Err(Fail(args::UNSUPPORTED, format!("{}: PLAIN HTTP, NOT ENCRYPTED: ADD --insecure-http", current))); }
        if !url.https && !o.silent { mind::println!("GET: PLAIN HTTP TO {}, NOT ENCRYPTED", url.authority); }
        let mut next = [0u8; 2048];
        let mut sink = |head: &http::Head, piece: &[u8]| -> Result<(), Fail> {
            match file.as_mut() {
                None => { console.take(piece); Ok(()) }
                Some(f) => {
                    // A resumed file goes on only when the server answers with the rest (206); else it starts over.
                    if !started { started = true; if offset > 0 && head.status != 206 { written = 0; } }
                    f.write_at(written as usize, piece).map_err(|e| Fail(args::WRITE_ERROR, format!("{:?}", e)))?;
                    written += piece.len() as u64;
                    Ok(())
                }
            }
        };
        let exchanged = exchange(o, &url, method_now, with_body, trusted, offset, deadline, &mut sink, &mut parser, &mut next, followed);
        // What was written before a failure stays on the disk for -c.
        if exchanged.is_err() { if let Some(f) = file.as_ref() { let _ = f.flush(); } }
        let (answer, redirect) = exchanged?;
        let status = answer.head.status;
        if let Some(r) = redirect {
            current = String::from(core::str::from_utf8(&next[..r.length]).unwrap_or(""));
            method_now = r.method;
            with_body = with_body && r.keep_body;
            trusted = trusted && r.same_host;
            followed += 1;
            if !o.silent { mind::println!("GET: HTTP {}, ON TO {}", status, current); }
            continue;
        }
        if !console.carry.is_empty() && !console.binary { console.binary = true; }
        if console.binary { mind::println!("\nGET: A BODY THAT IS NOT TEXT, {} BYTES: SAVE IT WITH -o", console.bytes); }
        let whole = status == 416 && offset > 0;
        if let (Some(f), Some(path)) = (file.as_mut(), target.as_ref()) {
            // A file started over may have been longer.
            if f.size() as u64 > written { f.truncate(written as usize).map_err(|e| Fail(args::WRITE_ERROR, format!("{}: {:?}", path, e)))?; }
            f.flush().map_err(|e| Fail(args::WRITE_ERROR, format!("{}: {:?}", path, e)))?;
            if !o.silent && !o.head { if whole { mind::println!("GET: {} ALREADY WHOLE", path); } else { mind::println!("GET: {} BYTES IN {}", written, path); } }
        }
        if o.head { mind::println!("{}", head_text(&answer.head)); }
        if o.write_out { mind::println!("GET: HTTP {}, {} BYTES IN {} MS", status, answer.bytes, now() - start); }
        if o.fail && status >= 400 && !whole { return Err(Fail(args::HTTP_ERROR, format!("HTTP {}", status))); }
        return Ok(());
    }
}

// -I: what the parser read of the head (the fields this system reads, not every line the server sent).
fn head_text(h: &http::Head) -> String {
    let mut text = format!("HTTP {}", h.status);
    if let Some(length) = h.length { text.push_str(&format!("\nContent-Length: {}", length)); }
    if !h.media.is_empty() { text.push_str(&format!("\nContent-Type: {}", h.media.as_str())); }
    if !h.location.is_empty() { text.push_str(&format!("\nLocation: {}", h.location.as_str())); }
    if h.chunked { text.push_str("\nTransfer-Encoding: chunked"); }
    text
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("get — an HTTP request from the console or a script, as wget and curl make them (000-NET-0048).\nUsage: get [options] URL\n  -o FILE  save the body (the user's files: ram:, data/, log:)   -O  save it in ram: under the URL's last name\n  -c  resume a saved file   -I  the head alone   -X GET|POST|PUT   -H 'Name: value' (up to 14)\n  -d TEXT | -d @FILE  a body (POST)   -F name=value | -F name=@FILE  a multipart form (POST, up to 8)\n  -L  follow redirects (the default; --no-location stops, --max-redirs N, 10 at most)   -s  silent   -w  the status, bytes and time\n  -f  exit 22 on an HTTP error   -m, --max-time S   --insecure-http  plain http:// (not encrypted)\nExit statuses as curl's: 0, 1 not https, 2 usage, 3 URL, 6 host, 7 connection, 22 HTTP error (-f), 23 write, 26 read,\n28 time, 35 TLS, 47 redirects, 56 receive, 60 certificate.\nIt reaches only what the network policy gives `get` (a line naming the host and port, or a browsing grant).");
    let code = match args::split(mind::process::args_str()).and_then(|words| args::parse(&words)) {
        Err(why) => { mind::println!("GET: {} (help get)", why); args::USAGE }
        Ok(options) => match run(&options) {
            Ok(()) => args::OK,
            Err(Fail(code, why)) => { if !options.silent { mind::println!("GET: {}", why); } code }
        },
    };
    mind::process::exit_with(code as u32)
}
