//! Host tests of decoding a response body by its content coding (libmind/src/http_coding.rs and inflate.rs's readers,
//! 000-NET-0047): identity, gzip with every optional header field, two gzip members, zlib and raw DEFLATE as
//! `deflate`, all arriving in small pieces and ending on a kept connection; a damaged gzip, an unknown coding, a bomb
//! stopped at the limit, and a sink that stops. The coded files come from tests/data/http/make.py.
extern crate alloc;
#[path = "../libmind/src/http.rs"]
#[allow(dead_code)]
mod http;
#[path = "../libmind/src/inflate.rs"]
#[allow(dead_code)]
mod inflate;
#[path = "../libmind/src/http_coding.rs"]
mod http_coding;

use http::{Connection, Error, Local, Method, Transport, Url};

fn file(name: &str) -> Vec<u8> { std::fs::read(format!("tests/data/http/{}", name)).unwrap() }

// The text make.py coded.
fn text() -> Vec<u8> {
    (0..2000).flat_map(|i: usize| {
        let mut line = format!("line {:05}: the quick brown fox jumps over the lazy dog ", i).into_bytes();
        line.extend(std::iter::repeat_n(b'a' + (i % 26) as u8, i % 40));
        line.push(b'\n');
        line
    }).collect()
}

/// Responses handed out in pieces of `step` bytes.
struct Scripted { response: Vec<u8>, step: usize, given: usize }
impl Transport for Scripted {
    fn send(&mut self, _: &[u8]) -> Result<(), Error> { Ok(()) }
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        let n = buffer.len().min(self.step).min(self.response.len() - self.given);
        buffer[..n].copy_from_slice(&self.response[self.given..self.given + n]);
        self.given += n;
        Ok(n)
    }
}

fn response(coding: &str, body: &[u8], chunked: bool) -> Vec<u8> {
    let mut r = if chunked { format!("HTTP/1.1 200 OK\r\nContent-Encoding: {}\r\nTransfer-Encoding: chunked\r\n\r\n", coding).into_bytes() }
        else { format!("HTTP/1.1 200 OK\r\nContent-Encoding: {}\r\nContent-Length: {}\r\n\r\n", coding, body.len()).into_bytes() };
    if chunked {
        for piece in body.chunks(1000) { r.extend_from_slice(format!("{:x}\r\n", piece.len()).as_bytes()); r.extend_from_slice(piece); r.extend_from_slice(b"\r\n"); }
        r.extend_from_slice(b"0\r\n\r\n");
    } else { r.extend_from_slice(body); }
    r
}

// Decodes one response, then checks the connection carries the next.
fn decoded(coding: &str, body: &[u8], step: usize, chunked: bool, limit: u64) -> Result<Vec<u8>, Error> {
    let mut script = response(coding, body, chunked);
    script.extend_from_slice(b"HTTP/1.1 204 No Content\r\n\r\n");
    let mut connection = Connection::new(Scripted { response: script, step, given: 0 });
    let url = Url::parse("http://api.test/x").unwrap();
    let mut out = Vec::new();
    let result = {
        let mut response = connection.request(Method::Get, &url, &[], None, &mut Local, false).unwrap();
        http_coding::decode(&mut response, limit, &mut |piece| { out.extend_from_slice(piece); true })
    };
    let given = result?;
    assert_eq!(given, out.len() as u64);
    assert_eq!(connection.request(Method::Get, &url, &[], None, &mut Local, false).unwrap().status, 204, "{} step {}", coding, step);
    Ok(out)
}

#[test]
fn every_coding_in_pieces() {
    let text = text();
    for step in [1, 7, 512, 100_000] {
        for chunked in [false, true] {
            if step == 1 && chunked { continue; }
            assert_eq!(decoded("identity", &text, step, chunked, 1 << 20).unwrap(), text);
            assert_eq!(decoded("gzip", &file("body.gz"), step, chunked, 1 << 20).unwrap(), text, "gzip step {}", step);
            assert_eq!(decoded("x-gzip", &file("two.gz"), step, chunked, 1 << 20).unwrap(), text, "two members step {}", step);
            assert_eq!(decoded("deflate", &file("body.zlib"), step, chunked, 1 << 20).unwrap(), text, "zlib step {}", step);
            assert_eq!(decoded("deflate", &file("body.deflate"), step, chunked, 1 << 20).unwrap(), text, "raw step {}", step);
        }
    }
    assert_eq!(decoded("gzip", b"", 7, false, 10).unwrap(), b"");
}

#[test]
fn refusals() {
    let damaged = file("damaged.gz");
    assert_eq!(http_coding_error(decoded("gzip", &damaged, 64, false, 1 << 20)), Error::Encoding);
    assert_eq!(http_coding_error(decoded("br", b"anything", 64, false, 1 << 20)), Error::Encoding);
    assert_eq!(http_coding_error(decoded("gzip", &file("body.zlib"), 64, false, 1 << 20)), Error::Encoding);
    assert_eq!(http_coding_error(decoded("deflate", b"\x78", 64, false, 1 << 20)), Error::Encoding);
    // A body cut short is a transport's end, not a decoded body.
    let gz = file("body.gz");
    let mut cut = response("gzip", &gz, false);
    cut.truncate(cut.len() - 100);
    let mut connection = Connection::new(Scripted { response: cut, step: 50, given: 0 });
    let mut response = connection.request(Method::Get, &Url::parse("http://api.test/").unwrap(), &[], None, &mut Local, false).unwrap();
    assert_eq!(http_coding::decode(&mut response, 1 << 20, &mut |_| true), Err(Error::Closed));
}

fn http_coding_error(result: Result<Vec<u8>, Error>) -> Error { result.unwrap_err() }

#[test]
fn a_bomb_stops_at_the_limit() {
    // 16 MiB of zeros in 16 KiB: refused at 1 MiB, having handed out no more than that.
    let bomb = file("bomb.gz");
    let mut connection = Connection::new(Scripted { response: response("gzip", &bomb, false), step: 4096, given: 0 });
    let mut response = connection.request(Method::Get, &Url::parse("http://api.test/").unwrap(), &[], None, &mut Local, false).unwrap();
    let mut seen = 0u64;
    assert_eq!(http_coding::decode(&mut response, 1 << 20, &mut |piece| { seen += piece.len() as u64; true }), Err(Error::TooLarge));
    assert!(seen <= 1 << 20, "{}", seen);
    // Within a limit that holds it, it decodes whole.
    assert_eq!(decoded("gzip", &bomb, 4096, false, 16 << 20).unwrap().len(), 16 << 20);
}

#[test]
fn a_sink_that_stops() {
    let gz = file("body.gz");
    let mut connection = Connection::new(Scripted { response: response("gzip", &gz, false), step: 100, given: 0 });
    let mut response = connection.request(Method::Get, &Url::parse("http://api.test/").unwrap(), &[], None, &mut Local, false).unwrap();
    let mut pieces = 0;
    let given = http_coding::decode(&mut response, 1 << 20, &mut |_| { pieces += 1; pieces < 3 }).unwrap();
    assert!(given > 0 && given < text().len() as u64);
}
