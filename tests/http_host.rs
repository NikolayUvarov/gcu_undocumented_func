//! Host tests of HTTP/1.1 downloads (libmind/src/http.rs, issues 351-NET-0001, 109-NET-0008): URLs, the request sent,
//! 200 and 206 bodies into a sink, a cut connection resumed with `Range`, a server that sends the whole file again, a
//! file already complete (416), the refusals, the head parser on its own, and `get`'s checks against a parser that lies.
//! For services (750-NET-0019): requests with bodies and headers on a kept connection, chunked and close-delimited
//! bodies, the bounds of chunk lines and trailers, and the headers a caller cannot set.
#[path = "../libmind/src/http.rs"]
mod http;

use http::{get, http_date, parse_head, redirect, Connection, Encoding, Error, Field, Got, Head, Local, Method, Parser, Range, Redirected, RetryAfter, Sink, Source, Transport, Url, LOCATION_MAX, REDIRECTS_MAX};

/// A server's response, handed out in pieces of `step` bytes, closed after `cut` bytes.
struct Scripted { response: Vec<u8>, step: usize, cut: usize, given: usize, sent: Vec<u8>, fail: bool }

impl Scripted {
    fn new(response: &[u8]) -> Self { Scripted { response: response.to_vec(), step: 7, cut: usize::MAX, given: 0, sent: Vec::new(), fail: false } }
}

impl Transport for Scripted {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> { self.sent.extend_from_slice(data); Ok(()) }
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        let end = self.response.len().min(self.cut);
        if self.given >= end { return if self.fail { Err(Error::Transport) } else { Ok(0) }; }
        let n = buffer.len().min(self.step).min(end - self.given);
        buffer[..n].copy_from_slice(&self.response[self.given..self.given + n]);
        self.given += n;
        Ok(n)
    }
}

/// A file in memory.
#[derive(Default)]
struct File { data: Vec<u8>, begun: Vec<(u64, u64)> }

impl Sink for File {
    fn begin(&mut self, start: u64, total: u64) -> Result<(), Error> {
        self.begun.push((start, total));
        self.data.truncate(start as usize);
        Ok(())
    }
    fn write(&mut self, offset: u64, data: &[u8]) -> Result<(), Error> {
        if offset as usize != self.data.len() { return Err(Error::Sink); }
        self.data.extend_from_slice(data);
        Ok(())
    }
}

fn body(len: usize) -> Vec<u8> { (0..len).map(|i| (i * 31 % 251) as u8).collect() }

fn ok(data: &[u8]) -> Vec<u8> {
    let mut r = format!("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n", data.len()).into_bytes();
    r.extend_from_slice(data);
    r
}

fn partial(data: &[u8], from: usize) -> Vec<u8> {
    let mut r = format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\n\r\n", from, data.len() - 1, data.len(), data.len() - from).into_bytes();
    r.extend_from_slice(&data[from..]);
    r
}

const URL: &str = "http://10.0.2.2:8080/releases/big.bin";

#[test]
fn urls() {
    assert_eq!(Url::parse("http://10.0.2.2:8080/a/b"), Ok(Url { https: false, authority: "10.0.2.2:8080", host: "10.0.2.2", port: 8080, path: "/a/b" }));
    assert_eq!(Url::parse("https://updates.example.org"), Ok(Url { https: true, authority: "updates.example.org", host: "updates.example.org", port: 443, path: "/" }));
    assert_eq!(Url::parse("http://h/x").map(|u| u.port), Ok(80));
    for bad in ["ftp://h/x", "http://", "http://:80/", "http://h:0/", "http://h:99999/", "http://h:x/", "http://h/a b", "h/x"] {
        assert_eq!(Url::parse(bad), Err(Error::Url), "{bad}");
    }
}

#[test]
fn the_request() {
    let data = body(10);
    let mut t = Scripted::new(&ok(&data));
    get(&mut t, &Url::parse(URL).unwrap(), 0, &mut File::default(), &mut Local).unwrap();
    let sent = String::from_utf8(t.sent).unwrap();
    assert!(sent.starts_with("GET /releases/big.bin HTTP/1.1\r\nHost: 10.0.2.2:8080\r\n"), "{sent}");
    assert!(sent.contains("Connection: close\r\n") && sent.ends_with("\r\n\r\n") && !sent.contains("Range"), "{sent}");
    let mut t = Scripted::new(&partial(&data, 4));
    get(&mut t, &Url::parse(URL).unwrap(), 4, &mut File { data: data[..4].to_vec(), ..Default::default() }, &mut Local).unwrap();
    assert!(String::from_utf8(t.sent).unwrap().contains("\r\nRange: bytes=4-\r\n"));
}

#[test]
fn a_whole_body_in_small_pieces() {
    let data = body(100_000);
    for step in [1, 7, 4096, 9000] {
        let mut t = Scripted::new(&ok(&data));
        t.step = step;
        let mut file = File::default();
        assert_eq!(get(&mut t, &Url::parse(URL).unwrap(), 0, &mut file, &mut Local), Ok(Got { start: 0, end: 100_000, total: 100_000 }));
        assert!(file.data == data, "step {step}");
    }
    // An empty file.
    let mut file = File::default();
    let got = get(&mut Scripted::new(&ok(b"")), &Url::parse(URL).unwrap(), 0, &mut file, &mut Local).unwrap();
    assert!(got.complete() && file.data.is_empty());
}

#[test]
fn a_cut_is_resumed_with_range() {
    let data = body(50_000);
    let url = Url::parse(URL).unwrap();
    let mut file = File::default();
    // The connection is cut, or fails, after 20 000 bytes of the response: the GET ends there, no error.
    for fail in [false, true] {
        file = File::default();
        let mut t = Scripted::new(&ok(&data));
        t.cut = 20_000;
        t.fail = fail;
        let got = get(&mut t, &url, 0, &mut file, &mut Local).unwrap();
        assert!(!got.complete() && got.end == file.data.len() as u64 && got.end > 0);
    }
    let from = file.data.len();
    let got = get(&mut Scripted::new(&partial(&data, from)), &url, from as u64, &mut file, &mut Local).unwrap();
    assert_eq!(got, Got { start: from as u64, end: 50_000, total: 50_000 });
    assert!(file.data == data);
    assert_eq!(file.begun, [(0, 50_000), (from as u64, 50_000)]);
}

#[test]
fn a_server_without_ranges_sends_it_all_again() {
    let data = body(3000);
    let mut file = File { data: data[..1000].to_vec(), ..Default::default() };
    let got = get(&mut Scripted::new(&ok(&data)), &Url::parse(URL).unwrap(), 1000, &mut file, &mut Local).unwrap();
    assert_eq!(got, Got { start: 0, end: 3000, total: 3000 });
    assert_eq!(file.begun, [(0, 3000)]);
    assert!(file.data == data);
}

#[test]
fn a_complete_file() {
    let response = b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */3000\r\nContent-Length: 0\r\n\r\n";
    let mut file = File::default();
    assert_eq!(get(&mut Scripted::new(response), &Url::parse(URL).unwrap(), 3000, &mut file, &mut Local), Ok(Got { start: 3000, end: 3000, total: 3000 }));
    assert!(file.begun.is_empty());
    // Past the end of a shorter file: refused, not taken as complete.
    assert_eq!(get(&mut Scripted::new(response), &Url::parse(URL).unwrap(), 4000, &mut file, &mut Local), Err(Error::Status(416)));
}

#[test]
fn refusals() {
    let url = Url::parse(URL).unwrap();
    let refused = |response: &[u8], offset: u64| get(&mut Scripted::new(response), &url, offset, &mut File::default(), &mut Local).unwrap_err();
    assert_eq!(refused(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n", 0), Error::Status(404));
    assert_eq!(refused(b"HTTP/1.1 301 Moved\r\nLocation: http://elsewhere/\r\n\r\n", 0), Error::Status(301));
    assert_eq!(refused(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 5\r\n\r\n", 0), Error::Chunked);
    assert_eq!(refused(b"HTTP/1.1 200 OK\r\n\r\nabc", 0), Error::Length);
    // A range other than the one asked for, or a length that disagrees with it.
    assert_eq!(refused(b"HTTP/1.1 206 Partial\r\nContent-Range: bytes 10-19/20\r\n\r\n", 5), Error::Range);
    assert_eq!(refused(b"HTTP/1.1 206 Partial\r\nContent-Range: bytes 5-19/20\r\nContent-Length: 3\r\n\r\n", 5), Error::Range);
    assert_eq!(refused(b"HTTP/1.1 206 Partial\r\nContent-Length: 15\r\n\r\n", 5), Error::Range);
    // Malformed heads, a head cut short, one too long.
    assert_eq!(refused(b"HTTP/2 200 OK\r\nContent-Length: 1\r\n\r\nx", 0), Error::Head);
    assert_eq!(refused(b"HTTP/1.1 2000 OK\r\n\r\n", 0), Error::Head);
    assert_eq!(refused(b"HTTP/1.1 200 OK\r\nContent-Length: -1\r\n\r\n", 0), Error::Head);
    assert_eq!(refused(b"HTTP/1.1 200 OK\r\nno colon\r\n\r\n", 0), Error::Head);
    assert_eq!(refused(b"HTTP/1.1 206 Partial\r\nContent-Range: bytes 9-5/20\r\n\r\n", 9), Error::Head);
    assert_eq!(refused(b"HTTP/1.1 200 OK\r\nContent-Le", 0), Error::Head);
    let mut long = b"HTTP/1.1 200 OK\r\n".to_vec();
    long.extend(std::iter::repeat_n(b'x', http::HEAD_MAX));
    assert_eq!(refused(&long, 0), Error::Head);
}

#[test]
fn the_sink_can_refuse() {
    struct Full;
    impl Sink for Full {
        fn begin(&mut self, _: u64, _: u64) -> Result<(), Error> { Ok(()) }
        fn write(&mut self, _: u64, _: &[u8]) -> Result<(), Error> { Err(Error::Sink) }
    }
    assert_eq!(get(&mut Scripted::new(&ok(&body(10))), &Url::parse(URL).unwrap(), 0, &mut Full, &mut Local), Err(Error::Sink));
}

#[test]
fn heads_parsed_on_their_own() {
    let head = b"HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 5-19/20\r\nContent-Length: 15\r\nServer: x";
    assert_eq!(parse_head(head), Ok(Head { status: 206, length: Some(15), range: Some(Range::Bytes { start: 5, end: 19, total: 20 }), chunked: false, close: false, ..Head::status(206) }));
    assert_eq!(parse_head(b"HTTP/1.0 416 No\r\nContent-Range: bytes */20"), Ok(Head { status: 416, length: None, range: Some(Range::Unsatisfied { total: 20 }), chunked: false, close: true, ..Head::status(416) }));
    assert_eq!(parse_head(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked").map(|h| h.chunked), Ok(true));
    assert_eq!(parse_head(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: identity").map(|h| h.chunked), Ok(false));
    for bad in [&b"HTTP/2 200 OK"[..], b"HTTP/1.1 20 OK", b"HTTP/1.1 200 OK\r\nno colon", b"HTTP/1.1 200 OK\r\nContent-Length: x", b"\xff\xfe", b""] {
        assert_eq!(parse_head(bad), Err(Error::Head), "{bad:?}");
    }
}

/// A parser that says what it is told to, whatever the bytes.
struct Lying(Head);
impl Parser for Lying { fn head(&mut self, _: &[u8]) -> Result<Head, Error> { Ok(self.0) } }

#[test]
fn get_checks_what_the_parser_says() {
    // The parser runs elsewhere (the parser service); a parser that lies must not make `get` write where it did not ask.
    let url = Url::parse(URL).unwrap();
    let data = body(100);
    let lie = |head: Head, offset: u64| get(&mut Scripted::new(&partial(&data, 10)), &url, offset, &mut File { data: data[..10].to_vec(), ..Default::default() }, &mut Lying(head));
    let bytes = |start, end, total, length| Head { status: 206, length, range: Some(Range::Bytes { start, end, total }), chunked: false, close: false, ..Head::status(206) };
    assert_eq!(lie(bytes(0, 99, 100, None), 10), Err(Error::Range)); // not where it asked
    assert_eq!(lie(bytes(10, 9, 100, None), 10), Err(Error::Range)); // an end before the start
    assert_eq!(lie(bytes(10, 100, 100, None), 10), Err(Error::Range)); // past the total
    assert_eq!(lie(bytes(10, 99, 100, Some(5)), 10), Err(Error::Range)); // a length that disagrees
    assert_eq!(lie(Head { status: 206, length: None, range: None, chunked: false, close: false, ..Head::status(206) }, 10), Err(Error::Range));
    assert_eq!(lie(Head { status: 416, length: None, range: Some(Range::Unsatisfied { total: 50 }), chunked: false, close: false, ..Head::status(416) }, 10), Err(Error::Status(416)));
    assert_eq!(lie(Head { status: 200, length: Some(100), range: None, chunked: true, close: false, ..Head::status(200) }, 0), Err(Error::Chunked));
    // What it says truthfully is taken.
    assert_eq!(lie(bytes(10, 99, 100, Some(90)), 10), Ok(Got { start: 10, end: 100, total: 100 }));
    // A parser that cannot be asked ends the GET with its error.
    struct Gone;
    impl Parser for Gone { fn head(&mut self, _: &[u8]) -> Result<Head, Error> { Err(Error::Parser) } }
    assert_eq!(get(&mut Scripted::new(&ok(&data)), &url, 0, &mut File::default(), &mut Gone), Err(Error::Parser));
}

// Reads a whole body in pieces of `step` bytes.
fn read_all<T: Transport>(response: &mut http::Response<'_, T>, step: usize) -> Result<Vec<u8>, Error> {
    let mut body = Vec::new();
    let mut buffer = vec![0u8; step];
    loop {
        let n = response.read(&mut buffer)?;
        if n == 0 { return Ok(body); }
        body.extend_from_slice(&buffer[..n]);
    }
}

const SERVICE: &str = "https://hesere.uz/effector/api/v1/heartbeat";

#[test]
fn requests_with_bodies_on_one_connection() {
    let url = Url::parse(SERVICE).unwrap();
    let mut answers = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{\"ok\":true}".to_vec();
    answers.extend_from_slice(b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 201 Created\r\nContent-Length: 0\r\n\r\n");
    answers.extend_from_slice(b"HTTP/1.1 204 No Content\r\n\r\n");
    let mut connection = Connection::new(Scripted::new(&answers));
    let body = br#"{"agent":"mind"}"#;
    let mut source: &[u8] = body;
    let headers = [("Authorization", "Bearer x"), ("Content-Type", "application/json")];
    let mut response = connection.request(Method::Post, &url, &headers, Some((body.len() as u64, &mut source)), &mut Local, false).unwrap();
    assert_eq!((response.status, response.length), (200, Some(11)));
    assert_eq!(read_all(&mut response, 3).unwrap(), br#"{"ok":true}"#);
    assert!(connection.open());
    let mut put: &[u8] = b"xyz";
    let response = connection.request(Method::Put, &url, &[], Some((3, &mut put)), &mut Local, false).unwrap();
    assert_eq!(response.status, 201); // the 100 Continue before it is skipped
    response.finish().unwrap();
    let response = connection.request(Method::Get, &url, &[], None, &mut Local, false).unwrap();
    assert_eq!(response.status, 204);
    assert_eq!(connection.served(), 3);
    let sent = String::from_utf8(connection.transport().sent.clone()).unwrap();
    assert_eq!(sent, "POST /effector/api/v1/heartbeat HTTP/1.1\r\nHost: hesere.uz\r\nUser-Agent: MIND-Core\r\nAuthorization: Bearer x\r\n\
                      Content-Type: application/json\r\nContent-Length: 16\r\n\r\n{\"agent\":\"mind\"}\
                      PUT /effector/api/v1/heartbeat HTTP/1.1\r\nHost: hesere.uz\r\nUser-Agent: MIND-Core\r\nContent-Length: 3\r\n\r\nxyz\
                      GET /effector/api/v1/heartbeat HTTP/1.1\r\nHost: hesere.uz\r\nUser-Agent: MIND-Core\r\n\r\n");
}

#[test]
fn a_hundred_requests_on_one_connection() {
    let url = Url::parse(SERVICE).unwrap();
    let mut answers = Vec::new();
    for i in 0..101 { answers.extend_from_slice(format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\n{i:03}\r\n0\r\n\r\n").as_bytes()); }
    let mut connection = Connection::new(Scripted::new(&answers));
    for i in 0..101 {
        let mut response = connection.request(Method::Get, &url, &[], None, &mut Local, false).unwrap();
        assert_eq!(read_all(&mut response, 2).unwrap(), format!("{i:03}").as_bytes());
    }
    assert_eq!(connection.served(), 101);
    assert!(connection.open());
}

#[test]
fn chunked_bodies_with_extensions_and_trailers() {
    let url = Url::parse(SERVICE).unwrap();
    let mut answers = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5;name=value\r\nhello\r\n6\r\n world\r\n0\r\nX-Checksum: 1\r\nX-Other: 2\r\n\r\n".to_vec();
    answers.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nnext");
    for step in [1, 4, 64] {
        let mut connection = Connection::new(Scripted { step, ..Scripted::new(&answers) });
        let mut response = connection.request(Method::Get, &url, &[], None, &mut Local, false).unwrap();
        assert_eq!(response.length, None);
        assert_eq!(read_all(&mut response, step).unwrap(), b"hello world");
        let mut response = connection.request(Method::Get, &url, &[], None, &mut Local, false).unwrap();
        assert_eq!(read_all(&mut response, 3).unwrap(), b"next");
    }
}

#[test]
fn chunk_framing_is_bounded() {
    let url = Url::parse(SERVICE).unwrap();
    let head = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n";
    let long_trailers = format!("0\r\n{}\r\n", "X-Long: 0123456789012345678901234567890123456789012345678901234567890123456789\r\n".repeat(60));
    for (bad, why) in [(format!("{}\r\n", "1".repeat(16)), "16 digits"), ("zz\r\n".to_string(), "not hexadecimal"), ("\r\n".to_string(), "no digits"),
                       (format!("5;{}\r\nhello\r\n", "e".repeat(200)), "a line past its bound"), ("5\r\nhelloXX0\r\n\r\n".to_string(), "no CRLF after the data"),
                       (long_trailers, "trailers past their bound"), ("5\nhello".to_string(), "a bare LF")] {
        let mut connection = Connection::new(Scripted::new(format!("{head}{bad}").as_bytes()));
        let mut response = connection.request(Method::Get, &url, &[], None, &mut Local, false).unwrap();
        assert_eq!(read_all(&mut response, 64), Err(Error::Framing), "{why}");
        assert!(!connection.open(), "{why}");
    }
}

#[test]
fn how_a_connection_ends() {
    let url = Url::parse(SERVICE).unwrap();
    // Connection: close, and HTTP/1.0 without keep-alive, end the connection after the body.
    for answer in [&b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 2\r\n\r\nok"[..], b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\nok"] {
        let mut connection = Connection::new(Scripted::new(answer));
        let mut response = connection.request(Method::Get, &url, &[], None, &mut Local, false).unwrap();
        assert_eq!(read_all(&mut response, 8).unwrap(), b"ok");
        assert!(!connection.open());
        assert!(matches!(connection.request(Method::Get, &url, &[], None, &mut Local, false), Err(Error::Closed)));
    }
    let mut connection = Connection::new(Scripted::new(b"HTTP/1.0 200 OK\r\nConnection: keep-alive\r\nContent-Length: 2\r\n\r\nok"));
    connection.request(Method::Get, &url, &[], None, &mut Local, false).unwrap().finish().unwrap();
    assert!(connection.open());
    // A body without a length: read to the end of the connection only when the caller allows it.
    let unframed = b"HTTP/1.1 200 OK\r\n\r\nuntil the end";
    let mut connection = Connection::new(Scripted::new(unframed));
    assert!(matches!(connection.request(Method::Get, &url, &[], None, &mut Local, false), Err(Error::Length)));
    let mut connection = Connection::new(Scripted::new(unframed));
    let mut response = connection.request(Method::Get, &url, &[], None, &mut Local, true).unwrap();
    assert_eq!(read_all(&mut response, 5).unwrap(), b"until the end");
    assert!(!connection.open());
    // Cut in the middle of a body with a length.
    let mut connection = Connection::new(Scripted { cut: 40, ..Scripted::new(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nshort") });
    let mut response = connection.request(Method::Get, &url, &[], None, &mut Local, false).unwrap();
    assert_eq!(read_all(&mut response, 8), Err(Error::Closed));
    assert!(!connection.open());
}

/// A body source that ends before its length.
struct Short(usize);
impl Source for Short { fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> { let n = self.0.min(buffer.len()); self.0 -= n; Ok(n) } }

#[test]
fn what_a_caller_cannot_send() {
    let url = Url::parse(SERVICE).unwrap();
    for headers in [&[("Host", "elsewhere")][..], &[("content-length", "5")], &[("Connection", "close")], &[("Transfer-Encoding", "chunked")],
                    &[("X-Bad", "a\r\nX-Injected: 1")], &[("Bad Name", "v")], &[("", "v")], &[("X-Bad", "\u{e9}")]] {
        let mut connection = Connection::new(Scripted::new(b""));
        assert!(matches!(connection.request(Method::Get, &url, headers, None, &mut Local, false), Err(Error::Header)), "{headers:?}");
        assert!(connection.transport().sent.is_empty());
    }
    let many: Vec<(String, String)> = (0..17).map(|i| (format!("X-{i}"), "v".to_string())).collect();
    let many: Vec<(&str, &str)> = many.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
    let mut connection = Connection::new(Scripted::new(b""));
    assert!(matches!(connection.request(Method::Get, &url, &many, None, &mut Local, false), Err(Error::Header)));
    let mut connection = Connection::new(Scripted::new(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"));
    let mut short = Short(10);
    assert!(matches!(connection.request(Method::Post, &url, &[], Some((20, &mut short)), &mut Local, false), Err(Error::Body)));
    assert!(!connection.open());
}

// 000-NET-0047: what a head says for API clients.
#[test]
fn heads_say_coding_type_location_and_retry() {
    let head = parse_head(b"HTTP/1.1 302 Found\r\nContent-Type: Text/Event-Stream; charset=utf-8\r\nContent-Encoding: GZIP\r\nLocation: /next?a=1\r\nRetry-After: 120").unwrap();
    assert_eq!((head.encoding, head.media.as_str(), head.location.as_str(), head.retry_after), (Encoding::Gzip, "text/event-stream", "/next?a=1", Some(RetryAfter::Seconds(120))));
    let coding = |value: &str| parse_head(format!("HTTP/1.1 200 OK\r\nContent-Encoding: {}", value).as_bytes()).unwrap().encoding;
    assert_eq!([coding("identity"), coding("x-gzip"), coding("deflate"), coding("br"), coding("gzip, br"), coding("identity, deflate")],
               [Encoding::Identity, Encoding::Gzip, Encoding::Deflate, Encoding::Other, Encoding::Other, Encoding::Deflate]);
    let plain = parse_head(b"HTTP/1.1 200 OK").unwrap();
    assert_eq!((plain.encoding, plain.media.is_empty(), plain.location.is_empty(), plain.retry_after), (Encoding::Identity, true, true, None));
    // A Location or media type that is not visible ASCII within its bound refuses the head; a bad Retry-After is ignored.
    assert_eq!(parse_head(format!("HTTP/1.1 301 M\r\nLocation: /{}", "a".repeat(LOCATION_MAX)).as_bytes()), Err(Error::Head));
    assert_eq!(parse_head(b"HTTP/1.1 200 OK\r\nContent-Type: nonsense"), Err(Error::Head));
    assert_eq!(parse_head(b"HTTP/1.1 503 Busy\r\nRetry-After: soon").unwrap().retry_after, None);
    assert_eq!(parse_head(b"HTTP/1.1 429 Slow\r\nRetry-After: Wed, 21 Oct 2015 07:28:00 GMT").unwrap().retry_after, Some(RetryAfter::Date(1_445_412_480)));
}

#[test]
fn http_dates_in_their_three_forms() {
    // RFC 9110's example: 784111777 seconds.
    for text in ["Sun, 06 Nov 1994 08:49:37 GMT", "Sunday, 06-Nov-94 08:49:37 GMT", "Sun Nov  6 08:49:37 1994"] { assert_eq!(http_date(text), Some(784_111_777), "{}", text); }
    assert_eq!(http_date("Thu, 01 Jan 1970 00:00:00 GMT"), Some(0));
    assert_eq!(http_date("Tue, 29 Feb 2028 23:59:59 GMT"), Some(1_835_481_599));
    assert_eq!(http_date("Monday, 01-Jan-30 00:00:00 GMT"), Some(1_893_456_000));
    for bad in ["", "Sun, 06 Nov 1994 08:49:37 UTC", "Sun, 6 Nov 1994 08:49:37 GMT", "Sun, 06 Foo 1994 08:49:37 GMT", "Sun, 06 Nov 1994 24:00:00 GMT",
                "Sun, 06 Nov 1994 08:49 GMT", "06 Nov 1994", "Sun, 06 Nov 1960 08:49:37 GMT", "Sun, 32 Nov 1994 08:49:37 GMT"] {
        assert_eq!(http_date(bad), None, "{:?}", bad);
    }
}

#[test]
fn redirects_follow_their_rules() {
    let from = Url::parse("https://api.test/v1/chat/send?x=1").unwrap();
    let mut out = [0u8; 2048];
    let follow = |status: u16, location: &str, method: Method, followed: u32| -> Result<Option<(String, Method, bool, bool)>, Error> {
        let mut out = [0u8; 2048];
        let head = Head { location: Field::new(location).unwrap(), ..Head::status(status) };
        let next = redirect(&from, method, &head, followed, &mut out)?;
        Ok(next.map(|r| (String::from_utf8(out[..r.length].to_vec()).unwrap(), r.method, r.keep_body, r.same_host)))
    };
    // Each status's method rule.
    assert_eq!(follow(301, "/a", Method::Post, 0), Ok(Some(("https://api.test/a".into(), Method::Get, false, true))));
    assert_eq!(follow(302, "/a", Method::Put, 0), Ok(Some(("https://api.test/a".into(), Method::Put, true, true))));
    assert_eq!(follow(303, "/a", Method::Put, 0), Ok(Some(("https://api.test/a".into(), Method::Get, false, true))));
    assert_eq!(follow(307, "/a", Method::Post, 0), Ok(Some(("https://api.test/a".into(), Method::Post, true, true))));
    assert_eq!(follow(308, "/a", Method::Post, 9), Ok(Some(("https://api.test/a".into(), Method::Post, true, true))));
    // References: absolute, scheme-relative, absolute path, relative to the path's directory.
    assert_eq!(follow(302, "https://cdn.test:8443/f", Method::Get, 0), Ok(Some(("https://cdn.test:8443/f".into(), Method::Get, true, false))));
    assert_eq!(follow(302, "//cdn.test/f", Method::Get, 0), Ok(Some(("https://cdn.test/f".into(), Method::Get, true, false))));
    assert_eq!(follow(302, "next?y=2", Method::Get, 0), Ok(Some(("https://api.test/v1/chat/next?y=2".into(), Method::Get, true, true))));
    // Refused: down to HTTP, no Location, the eleventh, and a Location that is no URL once resolved.
    assert_eq!(follow(302, "http://api.test/a", Method::Get, 0), Err(Error::Redirect));
    assert_eq!(redirect(&from, Method::Get, &Head::status(302), 0, &mut out), Err(Error::Redirect));
    assert_eq!(follow(302, "/a", Method::Get, REDIRECTS_MAX), Err(Error::Redirect));
    assert_eq!(follow(302, "ftp://x/y", Method::Get, 0), Err(Error::Redirect));
    assert_eq!(follow(302, "mailto:a@b", Method::Get, 0), Err(Error::Redirect));
    assert_eq!(follow(302, "/a#part", Method::Get, 0), Ok(Some(("https://api.test/a".into(), Method::Get, true, true))));
    // Not a redirect.
    assert_eq!(follow(200, "/a", Method::Get, 0), Ok(None));
    assert_eq!(follow(304, "/a", Method::Get, 0), Ok(None));
    // Up from HTTP is followed; the result must fit the caller's buffer.
    let plain = Url::parse("http://api.test/").unwrap();
    let head = Head { location: Field::new("https://api.test/").unwrap(), ..Head::status(301) };
    assert!(matches!(redirect(&plain, Method::Get, &head, 0, &mut out), Ok(Some(Redirected { same_host: false, .. }))));
    assert_eq!(redirect(&plain, Method::Get, &head, 0, &mut [0u8; 8]), Err(Error::Redirect));
}

#[test]
fn a_body_of_unknown_length_goes_chunked() {
    let url = Url::parse("http://example.test/upload").unwrap();
    // A source that gives its bytes in uneven pieces.
    struct Pieces { data: Vec<u8>, at: usize, step: usize }
    impl Source for Pieces {
        fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
            let n = buffer.len().min(self.step).min(self.data.len() - self.at);
            buffer[..n].copy_from_slice(&self.data[self.at..self.at + n]);
            self.at += n;
            self.step = self.step % 7000 + 3001;
            Ok(n)
        }
    }
    let data = body(10_000);
    let mut source = Pieces { data: data.clone(), at: 0, step: 5000 };
    let mut connection = Connection::new(Scripted::new(b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\n\r\n"));
    let response = connection.request_streamed(Method::Post, &url, &[("Content-Type", "application/json")], &mut source, &mut Local, false).unwrap();
    assert_eq!(response.status, 201);
    drop(response);
    let sent = connection.transport().sent.clone();
    let end = sent.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    let head = String::from_utf8(sent[..end].to_vec()).unwrap();
    assert!(head.contains("Transfer-Encoding: chunked\r\n") && !head.contains("Content-Length"), "{}", head);
    // Decode the chunks the client wrote.
    let (mut at, mut decoded) = (end, Vec::new());
    loop {
        let line_end = at + sent[at..].windows(2).position(|w| w == b"\r\n").unwrap();
        let size = usize::from_str_radix(std::str::from_utf8(&sent[at..line_end]).unwrap(), 16).unwrap();
        at = line_end + 2;
        if size == 0 { assert_eq!(&sent[at..], b"\r\n"); break; }
        assert!(size <= 4096);
        decoded.extend_from_slice(&sent[at..at + size]);
        assert_eq!(&sent[at + size..at + size + 2], b"\r\n");
        at += size + 2;
    }
    assert_eq!(decoded, data);
}
