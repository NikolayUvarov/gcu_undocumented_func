//! The hub's release tree under /updates/ (100003-APP-0159): a tree laid out as scripts/release.py publishes it, its
//! public channels served with httpd's name checks and ranges, the restricted ones refused, and a rate per network.

use mindhub::server::{self, Options};
use mindhub::updates::{Updates, RESTRICTED};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DIGEST: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
const LARGE: &str = "0000000000000000000000000000000000000000000000000000000000000001";

fn public(n: u8) -> String { improve_proto::hex(ed25519_dalek::SigningKey::from_bytes(&[n; 32]).verifying_key().as_bytes()) }

// A release tree as release.py lays it out: two channels, their timestamps, a recovery order, a manifest, a blob (and a
// large one), a key.
fn tree(dir: &Path, large: bool) -> Vec<u8> {
    let put = |rel: &str, bytes: &[u8]| { let p = dir.join(rel); std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, bytes).unwrap(); };
    put("channels/stable", b"{\"channel\":\"stable\",\"version\":202611250001}\n");
    put("channels/test", b"{\"channel\":\"test\",\"version\":202611250002}\n");
    put("timestamps/stable", b"stable 202611250001\n");
    put("timestamps/test", b"test 202611250002\n");
    put("recovery/test", b"test recovery\n");
    put("releases/202611250001/x86_64/MANIFEST", b"manifest of 202611250001\n");
    put("keys/current", b"key 1\n");
    let blob: Vec<u8> = (0..100_000u32).map(|i| (i * 7 % 251) as u8).collect();
    put(&format!("blobs/{DIGEST}"), &blob);
    if large { put(&format!("blobs/{LARGE}"), &vec![0x5a; 32 * 1024 * 1024]); }
    std::os::unix::fs::symlink("/etc/hostname", dir.join("blobs").join("f".repeat(64))).unwrap();
    blob
}

// A hub on loopback over `releases` (none when None), at `per_minute`; its URL's address.
fn hub(name: &str, releases: Option<&Path>, per_minute: u32) -> String {
    let dir = std::env::temp_dir().join(format!("mindhub-updates-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("hub.conf"), format!("admin maintainer {}\n", public(1))).unwrap();
    let shared = server::open(&dir).unwrap();
    let updates = releases.map(|r| Arc::new(Updates::new(r.to_path_buf(), vec![], per_minute).unwrap()));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || server::serve(listener, Arc::new(Mutex::new(shared)), Options { updates, ..Options::new() }));
    addr
}

struct Answer { status: u16, head: String, body: Vec<u8> }

impl Answer {
    fn header(&self, name: &str) -> Option<String> {
        self.head.lines().find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim().to_string()))
    }
    fn text(&self) -> String { String::from_utf8_lossy(&self.body).into_owned() }
}

// One request on its own connection, closed after it; the whole answer read.
fn ask(addr: &str, method: &str, target: &str, headers: &[&str]) -> Answer {
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let extra: String = headers.iter().map(|h| format!("{h}\r\n")).collect();
    write!(s, "{method} {target} HTTP/1.1\r\nHost: hub\r\n{extra}Connection: close\r\n\r\n").unwrap();
    let mut all = Vec::new();
    s.read_to_end(&mut all).unwrap();
    let end = all.windows(4).position(|w| w == b"\r\n\r\n").expect("a head");
    let head = String::from_utf8(all[..end].to_vec()).unwrap();
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    Answer { status, head, body: all[end + 4..].to_vec() }
}

#[test]
fn the_release_tree_is_served_under_updates() {
    let root: PathBuf = std::env::temp_dir().join(format!("mindhub-tree-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let blob = tree(&root, true);
    let addr = hub("served", Some(&root), 600);

    // A channel, a manifest, a blob and the key: their bytes, and how long each may be kept.
    let a = ask(&addr, "GET", "/updates/channels/stable", &[]);
    assert_eq!((a.status, a.text().as_str()), (200, "{\"channel\":\"stable\",\"version\":202611250001}\n"));
    assert_eq!(a.header("cache-control").as_deref(), Some("no-cache"));
    let m = ask(&addr, "GET", "/updates/releases/202611250001/x86_64/MANIFEST", &[]);
    assert_eq!((m.status, m.text().as_str(), m.header("cache-control").as_deref()), (200, "manifest of 202611250001\n", Some("public, max-age=31536000, immutable")));
    let b = ask(&addr, "GET", &format!("/updates/blobs/{DIGEST}"), &[]);
    assert_eq!((b.status, b.body.len(), b.header("etag")), (200, blob.len(), Some(format!("\"{DIGEST}\""))));
    assert!(b.body == blob);
    assert_eq!(ask(&addr, "GET", "/updates/keys/current", &[]).text(), "key 1\n");
    assert_eq!(ask(&addr, "GET", "/updates/timestamps/stable", &[]).text(), "stable 202611250001\n");

    // HEAD: the head alone. A range, one past the end, and a tag the client holds.
    let h = ask(&addr, "HEAD", &format!("/updates/blobs/{DIGEST}"), &[]);
    assert_eq!((h.status, h.header("content-length"), h.body.len()), (200, Some("100000".into()), 0));
    let r = ask(&addr, "GET", &format!("/updates/blobs/{DIGEST}"), &["Range: bytes=99990-"]);
    assert_eq!((r.status, r.header("content-range").as_deref(), &r.body[..]), (206, Some("bytes 99990-99999/100000"), &blob[99990..]));
    let past = ask(&addr, "GET", &format!("/updates/blobs/{DIGEST}"), &["Range: bytes=100000-"]);
    assert_eq!((past.status, past.header("content-range").as_deref()), (416, Some("bytes */100000")));
    let same = ask(&addr, "GET", &format!("/updates/blobs/{DIGEST}"), &[&format!("If-None-Match: \"{DIGEST}\"")]);
    assert_eq!((same.status, same.body.len()), (304, 0));

    // A restricted channel's files are not offered; a public one's are.
    for target in ["/updates/channels/test", "/updates/timestamps/test", "/updates/recovery/test"] {
        let t = ask(&addr, "GET", target, &[]);
        assert_eq!((t.status, t.text().trim()), (404, RESTRICTED), "{target}");
    }

    // What httpd refuses, refused alike.
    assert_eq!(ask(&addr, "GET", "/updates/hub.conf", &[]).status, 404, "outside the layout");
    assert_eq!(ask(&addr, "GET", "/updates/channels/.stable", &[]).status, 404, "a dot name");
    assert_eq!(ask(&addr, "GET", "/updates/channels/stable?x=1", &[]).status, 400, "a query");
    let link = ask(&addr, "GET", &format!("/updates/blobs/{}", "f".repeat(64)), &[]);
    assert_eq!((link.status, link.text().trim()), (404, "a symbolic link"));
    assert_eq!(ask(&addr, "GET", "/updates/blobs/0001", &[]).status, 404, "not a digest");
    let post = ask(&addr, "POST", "/updates/channels/stable", &["Content-Length: 0"]);
    assert_eq!((post.status, post.header("allow").as_deref()), (405, Some("GET, HEAD")));
    assert_eq!(ask(&addr, "HEAD", "/hub/v1/challenge", &[]).status, 405, "HEAD is the tree's alone");

    // The API answers while a large blob is being sent to a reader that does not read it.
    let mut slow = TcpStream::connect(&addr).unwrap();
    write!(slow, "GET /updates/blobs/{LARGE} HTTP/1.1\r\nHost: hub\r\n\r\n").unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let started = Instant::now();
    let c = ask(&addr, "GET", "/hub/v1/challenge", &[]);
    assert!(c.status == 200 && started.elapsed() < Duration::from_secs(2), "{} after {:?}", c.status, started.elapsed());
    drop(slow);
}

#[test]
fn the_release_tree_has_a_rate_and_needs_a_directory() {
    let root: PathBuf = std::env::temp_dir().join(format!("mindhub-tree-rate-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    tree(&root, false);
    let addr = hub("rate", Some(&root), 3);
    for _ in 0..3 { assert_eq!(ask(&addr, "GET", "/updates/channels/stable", &[]).status, 200); }
    let over = ask(&addr, "GET", "/updates/channels/stable", &[]);
    assert_eq!(over.status, 429);
    assert!(over.header("retry-after").is_some_and(|s| (1..=60).contains(&s.parse::<u64>().unwrap())));
    assert!(over.text().contains("more than 3 requests a minute"), "{}", over.text());
    assert_eq!(ask(&addr, "GET", "/hub/v1/challenge", &[]).status, 200, "the API has buckets of its own");
    // A hub given no tree says so.
    let none = hub("none", None, 600);
    let n = ask(&none, "GET", "/updates/channels/stable", &[]);
    assert_eq!((n.status, n.text().trim()), (404, "this hub serves no releases (mindhub serve --releases DIR)"));
    assert!(Updates::new(root.join("missing"), vec![], 600).is_err());
}
