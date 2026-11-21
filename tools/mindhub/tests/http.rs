//! The hub's HTTP reading, its forms and small helpers: a head out of form is refused with its status, never read
//! leniently; the challenges of one network push out only its own; networks, codes and versions.

use mindhub::api::{Challenges, PER_NETWORK};
use mindhub::http::{Conn, ReadError};
use mindhub::util;
use std::io::{Cursor, Read, Write};
use std::net::IpAddr;

/// A connection over bytes given, writing into a buffer.
struct Pipe { input: Cursor<Vec<u8>>, output: Vec<u8> }
impl Read for Pipe { fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> { self.input.read(b) } }
impl Write for Pipe {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> { self.output.extend_from_slice(b); Ok(b.len()) }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

fn conn(text: &str) -> Conn<Pipe> { Conn::new(Pipe { input: Cursor::new(text.as_bytes().to_vec()), output: Vec::new() }) }

fn status(text: &str) -> u16 {
    match conn(text).read(|_, _| 1024) { Err(ReadError::Bad(s, _)) => s, Ok(_) => 0, Err(e) => panic!("{e:?}") }
}

#[test]
fn requests_are_read_in_their_one_form() {
    let mut c = conn("POST /hub/v1/requests?x=1 HTTP/1.1\r\nHost: h\r\nContent-Length: 5\r\nCookie: a=1; hub_session=ab\r\n\r\nhelloGET /hub/v1/board HTTP/1.1\r\nConnection: close\r\n\r\n");
    let r = c.read(|_, _| 1024).unwrap();
    assert_eq!((r.method.as_str(), r.path.as_str(), r.query.as_str(), r.target.as_str(), r.body.as_slice(), r.close), ("POST", "/hub/v1/requests", "x=1", "/hub/v1/requests?x=1", &b"hello"[..], false));
    assert_eq!(r.cookie("hub_session"), Some("ab"));
    assert_eq!(r.cookie("hub"), None);
    let r = c.read(|_, _| 1024).unwrap();
    assert_eq!((r.path.as_str(), r.close), ("/hub/v1/board", true));
    assert!(matches!(c.read(|_, _| 1024), Err(ReadError::Closed)));
}

#[test]
fn a_head_out_of_form_is_refused_with_its_status() {
    assert_eq!(status("GET /a HTTP/2\r\n\r\n"), 505);
    assert_eq!(status("PATCH /a HTTP/1.1\r\n\r\n"), 405);
    assert_eq!(status("GET a HTTP/1.1\r\n\r\n"), 400);
    assert_eq!(status("GET /a HTTP/1.1\r\nX: 1\r\n folded\r\n\r\n"), 400);
    assert_eq!(status("GET /a HTTP/1.1\r\nNo colon\r\n\r\n"), 400);
    assert_eq!(status("POST /a HTTP/1.1\r\n\r\n"), 411);
    assert_eq!(status("POST /a HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n"), 501);
    assert_eq!(status("POST /a HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\nx"), 400);
    assert_eq!(status("POST /a HTTP/1.1\r\nContent-Length: +1\r\n\r\nx"), 400);
    assert_eq!(status("POST /a HTTP/1.1\r\nContent-Length: 2000\r\n\r\n"), 413);
    assert_eq!(status("POST /a HTTP/1.1\r\nContent-Length: 10\r\n\r\nshort"), 400);
    assert_eq!(status(&format!("GET /{} HTTP/1.1\r\n\r\n", "a".repeat(3000))), 400);
    assert_eq!(status(&format!("GET /a HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(20000))), 431);
    let many: String = (0..70).map(|i| format!("X{i}: 1\r\n")).collect();
    assert_eq!(status(&format!("GET /a HTTP/1.1\r\n{many}\r\n")), 431);
}

#[test]
fn a_body_asked_to_continue_is_told_to() {
    let mut c = conn("PUT /a HTTP/1.1\r\nContent-Length: 2\r\nExpect: 100-continue\r\n\r\nok");
    let r = c.read(|_, _| 10).unwrap();
    assert_eq!(r.body, b"ok");
    assert_eq!(c.stream.output, b"HTTP/1.1 100 Continue\r\n\r\n");
}

#[test]
fn forms_and_queries_decode_strictly() {
    assert_eq!(util::fields("a=1&b=x+y%21&c="), Some(vec![("a".into(), "1".into()), ("b".into(), "x y!".into()), ("c".into(), "".into())]));
    assert_eq!(util::fields("a=1&a=2"), None);
    assert_eq!(util::fields("a=%zz"), None);
    assert_eq!(util::fields("a=%ff"), None);
    assert_eq!(util::html("<a href=\"x\">'&'</a>"), "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;");
    assert_eq!(util::lines("a\r\nb\rc\n"), "a\nb\nc\n");
}

#[test]
fn networks_codes_and_versions() {
    let ip = |s: &str| s.parse::<IpAddr>().unwrap();
    assert_eq!(util::network(ip("198.51.100.7")), "198.51.100.7");
    assert_eq!(util::network(ip("2001:db8:1:2:3:4:5:6")), util::network(ip("2001:db8:1:2:ffff::1")));
    assert_eq!(util::network(ip("2001:db8:1:2::1")), "2001:db8:1:2::/64");
    assert_eq!(util::network(ip("::ffff:192.0.2.1")), "192.0.2.1");
    for _ in 0..100 {
        let c = util::code();
        assert!(improve_proto::form::pair_code(&c).is_ok(), "{c}");
    }
    assert!(util::at_least("202611210000", "202611200000") && util::at_least("202611200000", "202611200000"));
    assert!(!util::at_least("202611190000", "202611200000"));
    assert!(!util::at_least("0.4.2", "202611200000") && util::at_least("0.4.2", "0.4.2"));
}

#[test]
fn a_flood_of_challenges_pushes_out_its_own_network_alone() {
    let mut c = Challenges::default();
    let mine = c.issue(0, "192.0.2.1");
    let first = c.issue(0, "198.51.100.7");
    for _ in 0..PER_NETWORK * 10 { c.issue(0, "198.51.100.7"); }
    assert!(c.take(&mine, 1), "another network's challenge stays");
    assert!(!c.take(&first, 1), "the flooding network's oldest went");
    assert!(!c.take(&mine, 1), "a challenge is used once");
    let late = c.issue(0, "192.0.2.1");
    assert!(!c.take(&late, improve_proto::sign::CHALLENGE_LIFE_MS), "a challenge lasts 10 minutes");
}
