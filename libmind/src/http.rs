//! HTTP/1.1 downloads (351-NET-0001, docs/network/downloads.md): one GET, its body streamed into a sink, resumed with
//! `Range` after a cut. The transport is the caller's (a TCP socket of its flow grant, later a TLS session), so the
//! network policy that applies to the caller applies to its downloads. Only what a download needs is read: the status,
//! `Content-Length`, `Content-Range` and `Transfer-Encoding`; a chunked body, a redirect or another status is refused.
//! The head is parsed by a `Parser`: the parser service `parse` in the system (109-NET-0008, so the program that holds
//! the network and the file does not parse it), or `Local` in the host tests. `get` checks the typed head against what
//! it asked for whoever parsed it (MC-11.5).
//!
//! For services (750-NET-0019): `Connection` keeps one transport for several requests (`GET`, `POST`, `PUT`), with the
//! caller's headers within a bound and a body streamed from a `Source`; a response's body is read in pieces, framed by
//! `Content-Length`, chunked, or (when the caller allows it) the end of the connection. The library frames only: the
//! head is still the parser's, chunk-size lines and trailers are bounded and not interpreted.

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
    /// A caller's header the library keeps (`Host`, `Content-Length`, `Connection`, …), or one that is not a header.
    Header,
    /// The body source failed, or ended before its length.
    Body,
    /// A chunk-size line, a chunk's end or the trailers are malformed or longer than their bounds.
    Framing,
    /// The connection is closed: a new one is needed.
    Closed,
}

/// What a response head says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head { pub status: u16, pub length: Option<u64>, pub range: Option<Range>, pub chunked: bool, pub close: bool }

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
    let version = words.next().filter(|v| *v == "HTTP/1.1" || *v == "HTTP/1.0").ok_or(Error::Head)?;
    let status = words.next().and_then(|c| (c.len() == 3).then(|| c.parse::<u16>().ok()).flatten()).ok_or(Error::Head)?;
    let (mut length, mut range, mut chunked) = (None, None, false);
    // HTTP/1.0 closes unless it says keep-alive; HTTP/1.1 keeps the connection unless it says close.
    let (mut close, mut keep) = (false, false);
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(Error::Head)?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") { length = Some(number(value).ok_or(Error::Head)?); }
        else if name.eq_ignore_ascii_case("content-range") { range = Some(content_range(value).ok_or(Error::Head)?); }
        else if name.eq_ignore_ascii_case("transfer-encoding") && !value.eq_ignore_ascii_case("identity") { chunked = true; }
        else if name.eq_ignore_ascii_case("connection") {
            for token in value.split(',').map(str::trim) {
                if token.eq_ignore_ascii_case("close") { close = true; } else if token.eq_ignore_ascii_case("keep-alive") { keep = true; }
            }
        }
    }
    Ok(Head { status, length, range, chunked, close: close || (version == "HTTP/1.0" && !keep) })
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
    let Head { status, length, range, chunked, .. } = parser.head(&head[..end - 4])?;
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

/// A request method (750-NET-0019).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method { Get, Post, Put }

impl Method {
    fn name(self) -> &'static str { match self { Method::Get => "GET", Method::Post => "POST", Method::Put => "PUT" } }
}

/// Bounds of a request and a response: the caller's headers, the request head, a chunk-size line (extensions
/// included), the trailers after the last chunk.
pub const HEADERS_MAX: usize = 16;
pub const REQUEST_HEAD_MAX: usize = 4096;
pub const CHUNK_LINE_MAX: usize = 128;
pub const TRAILERS_MAX: usize = 4096;

/// Headers the library writes or that only a connection's ends may use: a caller cannot set them.
const KEPT: [&str; 10] = ["host", "content-length", "connection", "transfer-encoding", "te", "trailer", "upgrade", "keep-alive", "proxy-connection", "expect"];

/// Where a request body comes from, in pieces.
pub trait Source {
    /// Fills the start of `buffer`; 0 at the end.
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error>;
}

impl Source for &[u8] {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        let n = buffer.len().min(self.len());
        buffer[..n].copy_from_slice(&self[..n]);
        *self = &self[n..];
        Ok(n)
    }
}

// A header name is a token; a value is visible ASCII, spaces and tabs (no CR or LF, so nothing can be injected).
fn header_ok(name: &str, value: &str) -> bool {
    let token = |b: u8| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b);
    !name.is_empty() && name.bytes().all(token) && !KEPT.iter().any(|k| name.eq_ignore_ascii_case(k))
        && value.bytes().all(|b| b == b'\t' || (0x20..0x7F).contains(&b))
}

/// One transport kept for several requests, and what the server sent past the last response.
pub struct Connection<T: Transport> { transport: T, carry: [u8; HEAD_MAX], start: usize, end: usize, open: bool, served: u32 }

/// How a response's body ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Framing { Length(u64), Chunk(u64), ChunkEnd, ChunkSize, Close, Done }

/// A response whose body is read in pieces; the connection serves the next request once it has been read to its end.
pub struct Response<'c, T: Transport> { pub status: u16, pub length: Option<u64>, connection: &'c mut Connection<T>, framing: Framing, close: bool }

impl<T: Transport> Connection<T> {
    pub fn new(transport: T) -> Self { Connection { transport, carry: [0; HEAD_MAX], start: 0, end: 0, open: true, served: 0 } }
    /// Whether it may carry another request: false after `Connection: close`, a close-delimited body or a failure.
    pub fn open(&self) -> bool { self.open }
    /// How many responses it has carried.
    pub fn served(&self) -> u32 { self.served }
    pub fn transport(&mut self) -> &mut T { &mut self.transport }

    // The next received bytes: what is carried first, else one receive. 0: the peer closed.
    fn fill(&mut self) -> Result<usize, Error> {
        if self.start < self.end { return Ok(self.end - self.start); }
        self.start = 0;
        self.end = self.transport.receive(&mut self.carry).map_err(|e| { self.open = false; e })?;
        Ok(self.end)
    }

    fn byte(&mut self) -> Result<u8, Error> {
        if self.fill()? == 0 { self.open = false; return Err(Error::Closed); }
        self.start += 1;
        Ok(self.carry[self.start - 1])
    }

    // A line ending in CRLF, at most `max` bytes before it, into `out`; returns its length.
    fn line(&mut self, out: &mut [u8], max: usize) -> Result<usize, Error> {
        let mut n = 0;
        loop {
            match self.byte()? {
                b'\r' if self.byte()? == b'\n' => return Ok(n),
                b if b != b'\r' && b != b'\n' && n < max && n < out.len() => { out[n] = b; n += 1; }
                _ => { self.open = false; return Err(Error::Framing); }
            }
        }
    }

    /// Sends a request for `url`'s path: the method, `Host`, the caller's `headers`, and a body of `length` bytes from
    /// `body` when there is one. Reads the response head (skipping 1xx answers) and has `parser` read it. A body that
    /// only the end of the connection ends is refused unless `close_delimited`.
    pub fn request<'c>(&'c mut self, method: Method, url: &Url, headers: &[(&str, &str)], body: Option<(u64, &mut dyn Source)>,
                       parser: &mut impl Parser, close_delimited: bool) -> Result<Response<'c, T>, Error> {
        if !self.open { return Err(Error::Closed); }
        if headers.len() > HEADERS_MAX || !headers.iter().all(|(n, v)| header_ok(n, v)) { return Err(Error::Header); }
        let mut head = [0u8; REQUEST_HEAD_MAX];
        let mut text = Text { buffer: &mut head, len: 0 };
        use core::fmt::Write;
        let mut written = write!(text, "{} {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: MIND-Core\r\n", method.name(), url.path, url.authority).is_ok();
        for (name, value) in headers { written &= write!(text, "{}: {}\r\n", name, value).is_ok(); }
        if let Some((length, _)) = body.as_ref() { written &= write!(text, "Content-Length: {}\r\n", length).is_ok(); }
        written &= text.write_str("\r\n").is_ok();
        if !written { return Err(Error::Header); }
        let len = text.len;
        let sent = self.transport.send(&head[..len]).and_then(|_| match body {
            None => Ok(()),
            Some((length, source)) => {
                let (mut left, mut buffer) = (length, [0u8; 4096]);
                while left > 0 {
                    let want = (left as usize).min(buffer.len());
                    let n = source.read(&mut buffer[..want])?.min(want);
                    if n == 0 { return Err(Error::Body); }
                    self.transport.send(&buffer[..n])?;
                    left -= n as u64;
                }
                Ok(())
            }
        });
        if let Err(error) = sent { self.open = false; return Err(error); }
        loop {
            let mut raw = [0u8; HEAD_MAX];
            let mut n = 0;
            // The head ends at its blank line; a line longer than what is left of HEAD_MAX is refused.
            loop {
                let len = self.line(&mut raw[n..], HEAD_MAX - n).map_err(|e| if e == Error::Framing { Error::Head } else { e })?;
                if len == 0 { break; }
                n += len;
                if n + 2 > HEAD_MAX { self.open = false; return Err(Error::Head); }
                raw[n..n + 2].copy_from_slice(b"\r\n");
                n += 2;
            }
            if n < 2 { self.open = false; return Err(Error::Head); }
            let parsed = parser.head(&raw[..n - 2]).map_err(|e| { self.open = false; e })?;
            if (100..200).contains(&parsed.status) { continue; } // 100 Continue and the like: the real answer follows
            let framing = if parsed.status == 204 || parsed.status == 304 { Framing::Done }
                else if parsed.chunked { Framing::ChunkSize }
                else if let Some(length) = parsed.length { if length == 0 { Framing::Done } else { Framing::Length(length) } }
                else if close_delimited { Framing::Close }
                else { self.open = false; return Err(Error::Length) };
            let close = parsed.close || framing == Framing::Close;
            let mut response = Response { status: parsed.status, length: parsed.length.filter(|_| !parsed.chunked), connection: self, framing, close };
            if framing == Framing::Done { response.end(); }
            return Ok(response);
        }
    }
}

impl<T: Transport> Response<'_, T> {
    fn end(&mut self) {
        self.framing = Framing::Done;
        self.connection.served += 1;
        if self.close { self.connection.open = false; }
    }

    /// The next piece of the body into `out`; 0 at its end.
    pub fn read(&mut self, out: &mut [u8]) -> Result<usize, Error> {
        loop {
            match self.framing {
                Framing::Done => return Ok(0),
                Framing::ChunkSize => {
                    if let Some(size) = self.chunk_size()? { self.framing = Framing::Chunk(size); continue; }
                    self.trailers()?;
                    self.end();
                    return Ok(0);
                }
                Framing::ChunkEnd => {
                    let mut empty = [0u8; 1];
                    if self.connection.line(&mut empty, 0)? != 0 { self.connection.open = false; return Err(Error::Framing); }
                    self.framing = Framing::ChunkSize;
                }
                _ if out.is_empty() => return Ok(0),
                Framing::Close => {
                    let c = &mut *self.connection;
                    let available = c.fill()?;
                    if available == 0 { self.end(); return Ok(0); }
                    let n = available.min(out.len());
                    out[..n].copy_from_slice(&c.carry[c.start..c.start + n]);
                    c.start += n;
                    return Ok(n);
                }
                Framing::Length(left) | Framing::Chunk(left) => {
                    let c = &mut *self.connection;
                    let available = c.fill()?;
                    if available == 0 { c.open = false; return Err(Error::Closed); }
                    let n = available.min(out.len()).min(left.min(usize::MAX as u64) as usize);
                    out[..n].copy_from_slice(&c.carry[c.start..c.start + n]);
                    c.start += n;
                    let left = left - n as u64;
                    match self.framing {
                        Framing::Length(_) if left == 0 => self.end(),
                        Framing::Length(_) => self.framing = Framing::Length(left),
                        _ if left == 0 => self.framing = Framing::ChunkEnd,
                        _ => self.framing = Framing::Chunk(left),
                    }
                    return Ok(n);
                }
            }
        }
    }

    // A chunk-size line: hexadecimal digits, then extensions after ';', ignored. None: the last chunk.
    fn chunk_size(&mut self) -> Result<Option<u64>, Error> {
        let mut line = [0u8; CHUNK_LINE_MAX];
        let n = self.connection.line(&mut line, CHUNK_LINE_MAX)?;
        let digits = line[..n].split(|&b| b == b';').next().unwrap_or(&[]);
        let digits = core::str::from_utf8(digits).ok().map(|d| d.trim_end_matches([' ', '\t']));
        let size = digits.filter(|d| !d.is_empty() && d.len() <= 15).and_then(|d| u64::from_str_radix(d, 16).ok());
        let Some(size) = size else { self.connection.open = false; return Err(Error::Framing) };
        Ok((size > 0).then_some(size))
    }

    // The trailers after the last chunk: lines until a blank one, at most TRAILERS_MAX bytes, not interpreted.
    fn trailers(&mut self) -> Result<(), Error> {
        let mut total = 0;
        loop {
            let mut trailer = [0u8; 1024];
            let n = self.connection.line(&mut trailer, 1024)?;
            if n == 0 { return Ok(()); }
            total += n + 2;
            if total > TRAILERS_MAX { self.connection.open = false; return Err(Error::Framing); }
        }
    }

    /// Reads the rest of the body and drops it, so the connection can carry the next request.
    pub fn finish(mut self) -> Result<(), Error> {
        let mut buffer = [0u8; 1024];
        while self.read(&mut buffer)? > 0 {}
        Ok(())
    }
}
