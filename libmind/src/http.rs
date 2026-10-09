//! HTTP/1.1 downloads (351-NET-0001, docs/network/downloads.md): one GET, its body streamed into a sink, resumed with
//! `Range` after a cut. The transport is the caller's (a TCP socket of its flow grant, later a TLS session), so the
//! network policy that applies to the caller applies to its downloads. Only what a download needs is read: the status,
//! `Content-Length`, `Content-Range` and `Transfer-Encoding`; a chunked body, a redirect or another status is refused.
//! The head is parsed by a `Parser`: the parser service `parse` in the system (109-NET-0008, so the program that holds
//! the network and the file does not parse it), or `Local` in the host tests. `get` checks the typed head against what
//! it asked for whoever parsed it (MC-11.5).

/// The longest response head read.
pub const HEAD_MAX: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The transport failed before the body began.
    Transport,
    /// A malformed or too long URL.
    Url,
    /// A malformed response head, or one longer than `HEAD_MAX`.
    Head,
    /// A status other than 200, 206, or 416 for a file already complete.
    Status(u16),
    /// A chunked body: a download needs its length.
    Chunked,
    /// A response without the length of its body.
    Length,
    /// A `Content-Range` other than the one asked for.
    Range,
    /// The sink refused the data.
    Sink,
    /// The parser could not be asked (109-NET-0008).
    Parser,
}

/// What a response head says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head { pub status: u16, pub length: Option<u64>, pub range: Option<Range>, pub chunked: bool }

/// `Content-Range`: bytes `start` to `end` (inclusive) of `total`, or `*/total` (nothing satisfied the request).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range { Bytes { start: u64, end: u64, total: u64 }, Unsatisfied { total: u64 } }

/// Who turns the bytes of a response head into a `Head`.
pub trait Parser {
    /// The head: the status line and the header lines, without the blank line that ends them.
    fn head(&mut self, head: &[u8]) -> Result<Head, Error>;
}

/// The parser in this process (the host tests; the system uses the parser service).
pub struct Local;
impl Parser for Local { fn head(&mut self, head: &[u8]) -> Result<Head, Error> { parse_head(head) } }

/// Moves bytes to and from the server.
pub trait Transport {
    /// Sends all of `data`.
    fn send(&mut self, data: &[u8]) -> Result<(), Error>;
    /// Receives at least one byte into `buffer`; 0 once the peer has closed.
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, Error>;
}

/// Where the body goes.
pub trait Sink {
    /// The body starts at byte `start` of `total`: 0 when the server sent the whole file again.
    fn begin(&mut self, start: u64, total: u64) -> Result<(), Error>;
    /// `data` belongs at byte `offset`.
    fn write(&mut self, offset: u64, data: &[u8]) -> Result<(), Error>;
}

/// An `http://` or `https://` URL: the authority as the `Host` header names it, the host, port and path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Url<'a> { pub https: bool, pub authority: &'a str, pub host: &'a str, pub port: u16, pub path: &'a str }

impl<'a> Url<'a> {
    pub fn parse(url: &'a str) -> Result<Self, Error> {
        let (https, rest) = if let Some(rest) = url.strip_prefix("http://") { (false, rest) } else if let Some(rest) = url.strip_prefix("https://") { (true, rest) } else { return Err(Error::Url) };
        let (authority, path) = match rest.find('/') { Some(i) => (&rest[..i], &rest[i..]), None => (rest, "/") };
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) => (host, port.parse::<u16>().ok().filter(|&p| p != 0).ok_or(Error::Url)?),
            None => (authority, if https { 443 } else { 80 }),
        };
        let printable = |s: &str| s.bytes().all(|b| b.is_ascii_graphic());
        if host.is_empty() || host.len() > 253 || !printable(authority) || !printable(path) || path.len() > 1024 { return Err(Error::Url); }
        Ok(Url { https, authority, host, port, path })
    }
}

/// How far a GET got: its body covered bytes `start` to `end` of `total`. A connection cut before the end is not an
/// error: the caller resumes from `end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Got { pub start: u64, pub end: u64, pub total: u64 }

impl Got {
    pub fn complete(&self) -> bool { self.end == self.total }
}

// Formats into a fixed buffer.
struct Text<'b> { buffer: &'b mut [u8], len: usize }
impl core::fmt::Write for Text<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        if end > self.buffer.len() { return Err(core::fmt::Error); }
        self.buffer[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

fn number(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) { return None; }
    text.parse().ok()
}

// `bytes a-b/total`, or `bytes */total`.
fn content_range(value: &str) -> Option<Range> {
    let (range, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let total = number(total.trim())?;
    if range.trim() == "*" { return Some(Range::Unsatisfied { total }); }
    let (a, b) = range.trim().split_once('-')?;
    let (start, end) = (number(a)?, number(b)?);
    (start <= end && end < total).then_some(Range::Bytes { start, end, total })
}

/// Parses a response head: the status line and the header lines, without the blank line that ends them.
pub fn parse_head(head: &[u8]) -> Result<Head, Error> {
    let lines = core::str::from_utf8(head).map_err(|_| Error::Head)?;
    let mut lines = lines.split("\r\n");
    let status = lines.next().ok_or(Error::Head)?;
    let mut words = status.splitn(3, ' ');
    if !words.next().is_some_and(|v| v == "HTTP/1.1" || v == "HTTP/1.0") { return Err(Error::Head); }
    let status = words.next().and_then(|c| (c.len() == 3).then(|| c.parse::<u16>().ok()).flatten()).ok_or(Error::Head)?;
    let (mut length, mut range, mut chunked) = (None, None, false);
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(Error::Head)?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") { length = Some(number(value).ok_or(Error::Head)?); }
        else if name.eq_ignore_ascii_case("content-range") { range = Some(content_range(value).ok_or(Error::Head)?); }
        else if name.eq_ignore_ascii_case("transfer-encoding") && !value.eq_ignore_ascii_case("identity") { chunked = true; }
    }
    Ok(Head { status, length, range, chunked })
}

/// GET `url`'s path from byte `offset` on (`Range: bytes=offset-` when not 0), the head parsed by `parser`, the body into
/// `sink`.
pub fn get(transport: &mut impl Transport, url: &Url, offset: u64, sink: &mut impl Sink, parser: &mut impl Parser) -> Result<Got, Error> {
    let mut request = [0u8; 1536];
    let mut text = Text { buffer: &mut request, len: 0 };
    use core::fmt::Write;
    write!(text, "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: MIND-Core\r\nAccept-Encoding: identity\r\nConnection: close\r\n", url.path, url.authority).map_err(|_| Error::Url)?;
    if offset > 0 { write!(text, "Range: bytes={}-\r\n", offset).map_err(|_| Error::Url)?; }
    text.write_str("\r\n").map_err(|_| Error::Url)?;
    let len = text.len;
    transport.send(&request[..len])?;

    // The head, and whatever of the body came with it.
    let mut head = [0u8; HEAD_MAX];
    let mut filled = 0;
    let end = loop {
        if let Some(i) = head[..filled].windows(4).position(|w| w == b"\r\n\r\n") { break i + 4; }
        if filled == head.len() { return Err(Error::Head); }
        let n = transport.receive(&mut head[filled..])?;
        if n == 0 { return Err(Error::Head); }
        filled += n;
    };
    // Framing only: the head ends at its blank line. What it says is the parser's, checked here (MC-11.5).
    let Head { status, length, range, chunked } = parser.head(&head[..end - 4])?;
    let (start, stop, total) = match status {
        200 => { let total = length.ok_or(Error::Length)?; (0, total, total) }
        206 => match range {
            Some(Range::Bytes { start, end, total }) if start == offset && start <= end && end < total && length.is_none_or(|l| l == end - start + 1) => (start, end + 1, total),
            _ => return Err(Error::Range),
        },
        // Nothing past `offset`: the file is complete when that is its end.
        416 => match range {
            Some(Range::Unsatisfied { total }) if total == offset && offset > 0 => return Ok(Got { start: offset, end: offset, total }),
            _ => return Err(Error::Status(416)),
        },
        status => return Err(Error::Status(status)),
    };
    if chunked { return Err(Error::Chunked); }
    sink.begin(start, total)?;
    // The body: what came with the head, then the rest; anything past the length is ignored.
    let mut position = start;
    let mut write = |data: &[u8], position: &mut u64| -> Result<(), Error> {
        let take = (data.len() as u64).min(stop - *position) as usize;
        if take > 0 { sink.write(*position, &data[..take])?; *position += take as u64; }
        Ok(())
    };
    write(&head[end..filled], &mut position)?;
    let mut buffer = [0u8; 4096];
    while position < stop {
        // A cut or a failed transport after the body began ends the GET where it got to.
        match transport.receive(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => write(&buffer[..n], &mut position)?,
        }
    }
    Ok(Got { start, end: position, total })
}
