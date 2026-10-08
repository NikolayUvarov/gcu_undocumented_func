//! Host tests of HTTP/1.1 downloads (libmind/src/http.rs, issue 351-NET-0001): URLs, the request sent, 200 and 206
//! bodies into a sink, a cut connection resumed with `Range`, a server that sends the whole file again, a file already
//! complete (416), and the refusals.
#[path = "../libmind/src/http.rs"]
mod http;

use http::{get, Error, Got, Sink, Transport, Url};

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
    get(&mut t, &Url::parse(URL).unwrap(), 0, &mut File::default()).unwrap();
    let sent = String::from_utf8(t.sent).unwrap();
    assert!(sent.starts_with("GET /releases/big.bin HTTP/1.1\r\nHost: 10.0.2.2:8080\r\n"), "{sent}");
    assert!(sent.contains("Connection: close\r\n") && sent.ends_with("\r\n\r\n") && !sent.contains("Range"), "{sent}");
    let mut t = Scripted::new(&partial(&data, 4));
    get(&mut t, &Url::parse(URL).unwrap(), 4, &mut File { data: data[..4].to_vec(), ..Default::default() }).unwrap();
    assert!(String::from_utf8(t.sent).unwrap().contains("\r\nRange: bytes=4-\r\n"));
}

#[test]
fn a_whole_body_in_small_pieces() {
    let data = body(100_000);
    for step in [1, 7, 4096, 9000] {
        let mut t = Scripted::new(&ok(&data));
        t.step = step;
        let mut file = File::default();
        assert_eq!(get(&mut t, &Url::parse(URL).unwrap(), 0, &mut file), Ok(Got { start: 0, end: 100_000, total: 100_000 }));
        assert!(file.data == data, "step {step}");
    }
    // An empty file.
    let mut file = File::default();
    let got = get(&mut Scripted::new(&ok(b"")), &Url::parse(URL).unwrap(), 0, &mut file).unwrap();
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
        let got = get(&mut t, &url, 0, &mut file).unwrap();
        assert!(!got.complete() && got.end == file.data.len() as u64 && got.end > 0);
    }
    let from = file.data.len();
    let got = get(&mut Scripted::new(&partial(&data, from)), &url, from as u64, &mut file).unwrap();
    assert_eq!(got, Got { start: from as u64, end: 50_000, total: 50_000 });
    assert!(file.data == data);
    assert_eq!(file.begun, [(0, 50_000), (from as u64, 50_000)]);
}

#[test]
fn a_server_without_ranges_sends_it_all_again() {
    let data = body(3000);
    let mut file = File { data: data[..1000].to_vec(), ..Default::default() };
    let got = get(&mut Scripted::new(&ok(&data)), &Url::parse(URL).unwrap(), 1000, &mut file).unwrap();
    assert_eq!(got, Got { start: 0, end: 3000, total: 3000 });
    assert_eq!(file.begun, [(0, 3000)]);
    assert!(file.data == data);
}

#[test]
fn a_complete_file() {
    let response = b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */3000\r\nContent-Length: 0\r\n\r\n";
    let mut file = File::default();
    assert_eq!(get(&mut Scripted::new(response), &Url::parse(URL).unwrap(), 3000, &mut file), Ok(Got { start: 3000, end: 3000, total: 3000 }));
    assert!(file.begun.is_empty());
    // Past the end of a shorter file: refused, not taken as complete.
    assert_eq!(get(&mut Scripted::new(response), &Url::parse(URL).unwrap(), 4000, &mut file), Err(Error::Status(416)));
}

#[test]
fn refusals() {
    let url = Url::parse(URL).unwrap();
    let refused = |response: &[u8], offset: u64| get(&mut Scripted::new(response), &url, offset, &mut File::default()).unwrap_err();
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
    assert_eq!(get(&mut Scripted::new(&ok(&body(10))), &Url::parse(URL).unwrap(), 0, &mut Full), Err(Error::Sink));
}
