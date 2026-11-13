//! The server's rules in libmind (libmind/src/http_serve.rs, 110-NET-0070): every case of tools/httpd's head, range and
//! layout tests (stage 1), so that the MIND Core service and the Linux program serve alike; and what the parser service's
//! record keeps of a head (its bounds, the first of each header, a body).
#[allow(dead_code)]
#[path = "../libmind/src/http_serve.rs"]
mod http_serve;
use http_serve::*;

fn get(head: &[u8]) -> Request { parse_request(head).unwrap() }

#[test]
fn heads_as_tools_httpd_reads_them() {
    let r = get(b"GET /channels/stable HTTP/1.1\r\nHost: x\r\nRange: bytes=0-9\r\n\r\n");
    assert_eq!((r.method, r.target.as_str(), r.http11), (Method::Get, "/channels/stable", true));
    assert_eq!(r.range.as_str(), "bytes=0-9");
    assert!(r.keep_alive && !r.body);
    assert!(!get(b"GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").keep_alive);
    assert!(!get(b"HEAD / HTTP/1.0\r\n\r\n").keep_alive);
    assert!(get(b"HEAD / HTTP/1.0\nConnection: Keep-Alive\n\n").keep_alive);
    assert_eq!(parse_request(b"POST / HTTP/1.1\r\nHost: x\r\n\r\n"), Err(Bad::Method));
    assert_eq!(parse_request(b"GET / HTTP/2.0\r\nHost: x\r\n\r\n"), Err(Bad::Version));
    assert_eq!(parse_request(b"GET / HTTP/1.1\r\n\r\n"), Err(Bad::NoHost));
    assert_eq!(parse_request(b"GET /  HTTP/1.1\r\nHost: x\r\n\r\n"), Err(Bad::Malformed));
    assert_eq!(parse_request(b"GET / HTTP/1.1\r\nHost: x\r\n folded\r\n\r\n"), Err(Bad::Malformed));
    assert_eq!(parse_request(b"GET / HTTP/1.1\r\nHo st: x\r\n\r\n"), Err(Bad::Malformed));
    let many: String = (0..=HEADERS_MAX).map(|i| format!("X-{i}: 1\r\n")).collect();
    assert_eq!(parse_request(format!("GET / HTTP/1.1\r\nHost: x\r\n{many}\r\n").as_bytes()), Err(Bad::TooManyHeaders));
    assert_eq!(end(b"GET / HTTP/1.1\r\nHost: x\r\n\r\nrest"), Some(27));
    assert_eq!(end(b"GET / HTTP/1.1\r\nHost: x\r\n"), None);
    let statuses: Vec<u16> = [Bad::Malformed, Bad::Method, Bad::Version, Bad::NoHost, Bad::TooManyHeaders, Bad::TooLong].iter().map(|b| b.status()).collect();
    assert_eq!(statuses, [400, 405, 505, 400, 431, 414]);
}

#[test]
fn what_the_record_keeps() {
    // The first of each header, its value trimmed; spaces kept inside it.
    let r = get(b"GET /blobs/x HTTP/1.1\r\nhost: a\r\nIf-None-Match:  \"t1\", W/\"t2\" \r\nIf-None-Match: \"t3\"\r\nRANGE: bytes=5-\r\nRange: bytes=1-2\r\n\r\n");
    assert_eq!((r.if_none_match.as_str(), r.range.as_str()), ("\"t1\", W/\"t2\"", "bytes=5-"));
    // A value longer than kept is as if absent; a target longer than kept is refused with 414.
    let long = format!("GET / HTTP/1.1\r\nHost: a\r\nRange: bytes={}-\r\n\r\n", "1".repeat(VALUE_MAX));
    assert!(get(long.as_bytes()).range.is_empty());
    let target = format!("GET /{} HTTP/1.1\r\nHost: a\r\n\r\n", "a".repeat(TARGET_MAX));
    assert_eq!(parse_request(target.as_bytes()), Err(Bad::TooLong));
    assert_eq!(get(format!("GET /{} HTTP/1.1\r\nHost: a\r\n\r\n", "a".repeat(TARGET_MAX - 1)).as_bytes()).target.as_str().len(), TARGET_MAX);
    // A body: a length other than 0 (the first one), or any transfer coding.
    for (head, body) in [("Content-Length: 0", false), ("Content-Length: 5", true), ("Content-Length: 0\r\nContent-Length: 5", false), ("Transfer-Encoding: chunked", true)] {
        assert_eq!(get(format!("GET / HTTP/1.1\r\nHost: a\r\n{head}\r\n\r\n").as_bytes()).body, body, "{head}");
    }
    // Not text, or a byte outside printable ASCII in the target.
    assert_eq!(parse_request(b"GET /\xff HTTP/1.1\r\nHost: a\r\n\r\n"), Err(Bad::Malformed));
    assert_eq!(parse_request(b"GET /\x01 HTTP/1.1\r\nHost: a\r\n\r\n"), Err(Bad::Malformed));
    assert_eq!(parse_request(b""), Err(Bad::Malformed));
}

#[test]
fn ranges_as_tools_httpd_reads_them() {
    assert_eq!(asked(None, 100), Asked::Whole);
    assert_eq!(asked(Some("bytes=10-"), 100), Asked::Part(10, 99));
    assert_eq!(asked(Some("bytes=10-19"), 100), Asked::Part(10, 19));
    assert_eq!(asked(Some("bytes=90-200"), 100), Asked::Part(90, 99));
    assert_eq!(asked(Some("bytes=-30"), 100), Asked::Part(70, 99));
    assert_eq!(asked(Some("bytes=-300"), 100), Asked::Part(0, 99));
    assert_eq!(asked(Some("bytes=100-"), 100), Asked::Unsatisfiable);
    assert_eq!(asked(Some("bytes=150-160"), 100), Asked::Unsatisfiable);
    assert_eq!(asked(Some("bytes=-0"), 100), Asked::Unsatisfiable);
    assert_eq!(asked(Some("bytes=0-"), 0), Asked::Unsatisfiable);
    for ignored in ["bytes=0-1,5-6", "bytes=20-10", "bytes=a-", "bytes=-", "items=0-1", "bytes=1", "bytes=99999999999999999999-"] {
        assert_eq!(asked(Some(ignored), 100), Asked::Whole, "{ignored}");
    }
}

#[test]
fn tags() {
    assert!(matches_tag(Some("\"a\", W/\"b\""), "\"b\"") && matches_tag(Some("*"), "\"x\""));
    assert!(!matches_tag(Some("\"a\""), "\"b\"") && !matches_tag(None, "\"b\""));
}

const BLOB: &str = "/blobs/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn the_layout_and_each_refusal_as_tools_httpd_has_them() {
    assert_eq!(resolve(BLOB).unwrap().1, Kind::Blob);
    assert_eq!(resolve("/releases/6/x86_64/MANIFEST").unwrap(), ("releases/6/x86_64/MANIFEST", Kind::Manifest));
    assert_eq!(resolve("/releases/12/aarch64/MANIFEST.SIG").unwrap().1, Kind::Manifest);
    assert_eq!(resolve("/channels/stable").unwrap().1, Kind::Channel);
    assert_eq!(resolve("/keys/current").unwrap().1, Kind::Channel);
    assert_eq!(resolve("/keys/other").err(), Some(Refusal::NotInLayout));
    let upper = BLOB.to_uppercase().replace("/BLOBS/", "/blobs/");
    let refused = [
        ("/channels/stable?x=1", Refusal::Query), ("channels/stable", Refusal::NotAbsolute), ("/channels/st%61ble", Refusal::Byte),
        ("/channels/a b", Refusal::Byte), ("/channels\\stable", Refusal::Byte), ("/channels//stable", Refusal::EmptyName),
        ("/channels/stable/", Refusal::EmptyName), ("/", Refusal::EmptyName), ("/channels/.hidden", Refusal::DotName),
        ("/blobs/../channels/stable", Refusal::DotName), ("/blobs", Refusal::NotInLayout), ("/blobs/ABCDEF", Refusal::NotInLayout),
        (upper.as_str(), Refusal::NotInLayout), ("/releases/6/riscv64/MANIFEST", Refusal::NotInLayout),
        ("/releases/v6/x86_64/MANIFEST", Refusal::NotInLayout), ("/releases/6/x86_64/kernel.elf", Refusal::NotInLayout),
        ("/channels/Stable", Refusal::NotInLayout), ("/index.html", Refusal::NotInLayout), ("/channels/-x", Refusal::NotInLayout),
        // More names than any path of the layout has.
        ("/a/b/c/d/e/f", Refusal::NotInLayout),
    ];
    for (target, why) in refused { assert_eq!(resolve(target), Err(why), "{target}"); }
}
