//! httpd over a real socket: the layout and every refusal, ranges, HEAD and 304, keep-alive, the limits, a channel
//! replaced while it is read, and the log (110, stage 1).

use httpd::log::Log;
use httpd::server::{self, Config};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Site { root: PathBuf, logs: PathBuf, addr: SocketAddr }
impl Drop for Site { fn drop(&mut self) { let _ = fs::remove_dir_all(self.root.parent().unwrap()); } }

fn blob() -> Vec<u8> { (0..100_000u32).map(|i| (i % 251) as u8).collect() }

fn site(tune: impl FnOnce(&mut Config)) -> Site {
    let base = std::env::temp_dir().join(format!("httpd-test-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
    let (root, logs) = (base.join("root"), base.join("logs"));
    for dir in ["blobs", "releases/6/x86_64", "channels/dirchan"] { fs::create_dir_all(root.join(dir)).unwrap(); }
    fs::create_dir_all(&logs).unwrap();
    fs::write(root.join("blobs").join(DIGEST), blob()).unwrap();
    fs::write(root.join("releases/6/x86_64/MANIFEST"), b"manifest\n").unwrap();
    fs::write(root.join("channels/stable"), b"{\"version\":6}\ned25519 00\n").unwrap();
    fs::write(base.join("outside"), b"secret\n").unwrap();
    std::os::unix::fs::symlink(base.join("outside"), root.join("channels/link")).unwrap();
    std::os::unix::fs::symlink(root.join("releases/6"), root.join("releases/7")).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let mut config = Config::new(root.canonicalize().unwrap());
    tune(&mut config);
    let log = Log::new(Some(logs.clone()), 30);
    std::thread::spawn(move || server::run(listener, config, log));
    Site { root, logs, addr }
}

struct Response { status: u16, headers: Vec<(String, String)>, body: Vec<u8> }
impl Response {
    fn header(&self, name: &str) -> Option<&str> { self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()) }
    fn text(&self) -> String { String::from_utf8_lossy(&self.body).into_owned() }
}

fn read_response(reader: &mut BufReader<TcpStream>, head_only: bool) -> Response {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let status = line.split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or_else(|| panic!("no status in {line:?}"));
    let mut headers = Vec::new();
    loop {
        line.clear();
        reader.read_line(&mut line).unwrap();
        let l = line.trim_end();
        if l.is_empty() { break; }
        let (n, v) = l.split_once(':').unwrap();
        headers.push((n.to_string(), v.trim().to_string()));
    }
    let mut r = Response { status, headers, body: Vec::new() };
    if !head_only && status != 304 {
        let len: usize = r.header("content-length").unwrap().parse().unwrap();
        r.body = vec![0; len];
        reader.read_exact(&mut r.body).unwrap();
    }
    r
}

fn connect(site: &Site) -> BufReader<TcpStream> {
    let s = TcpStream::connect(site.addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    BufReader::new(s)
}

fn ask(site: &Site, request: &str) -> Response {
    let mut c = connect(site);
    c.get_mut().write_all(request.as_bytes()).unwrap();
    read_response(&mut c, request.starts_with("HEAD "))
}

fn get(site: &Site, path: &str, extra: &str) -> Response { ask(site, &format!("GET {path} HTTP/1.1\r\nHost: t\r\n{extra}Connection: close\r\n\r\n")) }

#[test]
fn the_layout_and_every_refusal() {
    let site = site(|_| {});
    let b = get(&site, &format!("/blobs/{DIGEST}"), "");
    assert_eq!((b.status, b.body.clone()), (200, blob()));
    assert_eq!(b.header("etag"), Some(format!("\"{DIGEST}\"").as_str()));
    assert_eq!(b.header("cache-control"), Some("public, max-age=31536000, immutable"));
    assert_eq!(b.header("accept-ranges"), Some("bytes"));
    let c = get(&site, "/channels/stable", "");
    assert_eq!((c.status, c.header("cache-control")), (200, Some("no-cache")));
    assert_eq!(get(&site, "/releases/6/x86_64/MANIFEST", "").text(), "manifest\n");
    let refused = [
        ("/channels/stable?x=1", 400, "a query string"), ("/channels/st%61ble", 400, "a byte outside"), ("/../outside", 404, "a dot"),
        ("/blobs", 404, "not a file of the release layout"), ("/releases/6/x86_64", 404, "not a file of the release layout"),
        ("/channels/dirchan", 404, "not a regular file"), ("/channels/link", 404, "a symbolic link"),
        ("/releases/7/x86_64/MANIFEST", 404, "a symbolic link"), ("/channels/missing", 404, "no such file"),
        (&format!("/blobs/{}", DIGEST.replace('0', "1")), 404, "no such file"), ("/channels//stable", 404, "an empty name"),
    ];
    for (path, status, why) in refused {
        let r = get(&site, path, "");
        assert_eq!(r.status, status, "{path}");
        assert!(r.text().contains(why), "{path}: {}", r.text());
        assert!(!r.text().contains("secret"), "{path}");
    }
    let post = ask(&site, "POST /channels/stable HTTP/1.1\r\nHost: t\r\nContent-Length: 0\r\n\r\n");
    assert_eq!((post.status, post.header("allow")), (405, Some("GET, HEAD")));
    assert_eq!(ask(&site, "GET /channels/stable HTTP/2.0\r\nHost: t\r\n\r\n").status, 505);
    assert_eq!(ask(&site, "GET /channels/stable HTTP/1.1\r\n\r\n").status, 400);
    assert_eq!(ask(&site, "GET /channels/stable HTTP/1.1\r\nHost: t\r\nContent-Length: 5\r\n\r\nhello").status, 400);
}

#[test]
fn ranges() {
    let site = site(|_| {});
    let path = format!("/blobs/{DIGEST}");
    let all = blob();
    let part = |range: &str| get(&site, &path, &format!("Range: {range}\r\n"));
    let r = part("bytes=1000-");
    assert_eq!((r.status, r.header("content-range")), (206, Some("bytes 1000-99999/100000")));
    assert_eq!(r.body, all[1000..]);
    let r = part("bytes=10-19");
    assert_eq!((r.status, r.body.as_slice()), (206, &all[10..20]));
    let r = part("bytes=-5");
    assert_eq!((r.status, r.header("content-range"), r.body.as_slice()), (206, Some("bytes 99995-99999/100000"), &all[99995..]));
    let r = part("bytes=100000-");
    assert_eq!((r.status, r.header("content-range")), (416, Some("bytes */100000")));
    let r = part("bytes=0-1,5-6");
    assert_eq!((r.status, r.body.len()), (200, 100_000));
}

#[test]
fn head_conditional_and_keep_alive() {
    let site = site(|_| {});
    let mut c = connect(&site);
    // HEAD, then GET, then a conditional GET, pipelined on one connection.
    let path = "/channels/stable";
    let tag = get(&site, path, "").header("etag").unwrap().to_string();
    c.get_mut().write_all(format!("HEAD {path} HTTP/1.1\r\nHost: t\r\n\r\nGET {path} HTTP/1.1\r\nHost: t\r\n\r\nGET {path} HTTP/1.1\r\nHost: t\r\nIf-None-Match: {tag}\r\n\r\n").as_bytes()).unwrap();
    let head = read_response(&mut c, true);
    assert_eq!((head.status, head.header("content-length")), (200, Some("25")));
    let body = read_response(&mut c, false);
    assert_eq!((body.status, body.text().as_str()), (200, "{\"version\":6}\ned25519 00\n"));
    let not = read_response(&mut c, false);
    assert_eq!((not.status, not.header("etag")), (304, Some(tag.as_str())));
    // HTTP/1.0 without keep-alive: the server closes after its response.
    let mut old = connect(&site);
    old.get_mut().write_all(format!("GET {path} HTTP/1.0\r\n\r\n").as_bytes()).unwrap();
    assert_eq!(read_response(&mut old, false).status, 200);
    let mut rest = Vec::new();
    assert_eq!(old.read_to_end(&mut rest).unwrap(), 0);
}

#[test]
fn connections_at_once() {
    let site = site(|c| { c.per_client = 2; c.idle_timeout = Duration::from_secs(5); });
    let held = [connect(&site), connect(&site)];
    std::thread::sleep(Duration::from_millis(200));
    let mut third = connect(&site);
    let r = read_response(&mut third, false);
    assert_eq!(r.status, 503);
    assert!(r.text().contains("this address holds its limit of 2 connections"), "{}", r.text());
    drop(held);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(get(&site, "/channels/stable", "").status, 200);
}

#[test]
fn requests_a_minute_head_size_and_deadlines() {
    let rated = site(|c| { c.per_minute = 3; c.head_timeout = Duration::from_secs(1); c.idle_timeout = Duration::from_secs(3); });
    let mut c = connect(&rated);
    for _ in 0..3 {
        c.get_mut().write_all(b"GET /channels/stable HTTP/1.1\r\nHost: t\r\n\r\n").unwrap();
        assert_eq!(read_response(&mut c, false).status, 200);
    }
    c.get_mut().write_all(b"GET /channels/stable HTTP/1.1\r\nHost: t\r\n\r\n").unwrap();
    let r = read_response(&mut c, false);
    assert_eq!(r.status, 429);
    assert!(r.text().contains("more than 3 requests a minute"), "{}", r.text());
    assert!(r.header("retry-after").is_some());

    let timed = site(|c| { c.head_timeout = Duration::from_secs(1); c.idle_timeout = Duration::from_secs(3); });
    let big = ask(&timed, &format!("GET /channels/stable HTTP/1.1\r\nHost: t\r\nX-Pad: {}\r\n", "a".repeat(9000)));
    assert_eq!(big.status, 431);
    // A head sent byte by slow byte runs out of its deadline.
    let mut slow = connect(&timed);
    slow.get_mut().write_all(b"GET /channels/stable HTTP/1.1\r\n").unwrap();
    std::thread::sleep(Duration::from_millis(1300));
    slow.get_mut().write_all(b"Host: t\r\n").unwrap();
    let r = read_response(&mut slow, false);
    assert_eq!(r.status, 408);
    assert!(r.text().contains("took longer than 1 s"), "{}", r.text());

    let quiet = site(|c| { c.idle_timeout = Duration::from_secs(1); });
    let mut idle = connect(&quiet);
    idle.get_mut().write_all(b"GET /channels/stable HTTP/1.1\r\n").unwrap();
    let r = read_response(&mut idle, false);
    assert_eq!(r.status, 408);
    assert!(r.text().contains("no byte of the request head for 1 s"), "{}", r.text());
}

#[test]
fn a_channel_replaced_while_it_is_read() {
    let site = site(|_| {});
    let old = vec![b'a'; 4 << 20];
    fs::write(site.root.join("channels/big"), &old).unwrap();
    let mut c = connect(&site);
    c.get_mut().write_all(b"GET /channels/big HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n").unwrap();
    let mut line = String::new();
    loop { line.clear(); c.read_line(&mut line).unwrap(); if line == "\r\n" { break; } }
    let mut first = vec![0; 65536];
    c.read_exact(&mut first).unwrap();
    // As release.py publishes: a new file renamed over the channel.
    fs::write(site.root.join("channels/big.new"), b"bbbb").unwrap();
    fs::rename(site.root.join("channels/big.new"), site.root.join("channels/big")).unwrap();
    let mut rest = Vec::new();
    c.read_to_end(&mut rest).unwrap();
    first.extend_from_slice(&rest);
    assert_eq!(first, old, "the reader saw the old channel whole");
    assert_eq!(get(&site, "/channels/big", "").text(), "bbbb");
}

#[test]
fn the_log() {
    let site = site(|_| {});
    get(&site, "/channels/stable", "");
    get(&site, "/channels/stable?token=secret", "Authorization: Bearer hidden\r\n");
    get(&site, "/channels/missing", "");
    std::thread::sleep(Duration::from_millis(200));
    let day = httpd::date::day(SystemTime::now());
    let text = fs::read_to_string(site.logs.join(format!("httpd-{day}.log"))).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    assert!(lines[0].contains(" 127.0.0.1 GET /channels/stable 200 25 "), "{}", lines[0]);
    assert!(lines[1].contains(" GET /channels/stable 400 ") && lines[1].contains("(a query string is not served)"), "{}", lines[1]);
    assert!(lines[2].contains(" GET /channels/missing 404 ") && lines[2].contains("(no such file)"), "{}", lines[2]);
    assert!(!text.contains("secret") && !text.contains("hidden"), "{text}");
}
