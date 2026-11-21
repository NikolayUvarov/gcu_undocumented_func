//! HTTP/1.1 as the hub takes it: GET, POST, PUT and DELETE, a body by Content-Length alone and within the route's
//! bound, keep-alive. A head out of this form is refused with its status rather than read leniently.

use std::io::{self, Read, Write};

pub const HEAD_MAX: usize = 16 * 1024;
pub const HEADERS_MAX: usize = 64;
pub const TARGET_MAX: usize = 2048;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    /// The path and the query as sent, which a signature covers.
    pub target: String,
    pub path: String,
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// The client asked to close after this answer, or speaks HTTP/1.0.
    pub close: bool,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn cookie(&self, name: &str) -> Option<&str> {
        self.headers.iter().filter(|(k, _)| k.eq_ignore_ascii_case("cookie"))
            .flat_map(|(_, v)| v.split(';')).map(str::trim).find_map(|c| c.strip_prefix(name)?.strip_prefix('='))
    }
}

#[derive(Debug)]
pub enum ReadError {
    /// The connection ended between requests.
    Closed,
    /// A request out of form, with the status that names it.
    Bad(u16, &'static str),
    Io(io::Error),
}

impl From<io::Error> for ReadError { fn from(e: io::Error) -> ReadError { ReadError::Io(e) } }

/// A connection and the bytes read past the last request.
pub struct Conn<S: Read + Write> { pub stream: S, buf: Vec<u8> }

impl<S: Read + Write> Conn<S> {
    pub fn new(stream: S) -> Conn<S> { Conn { stream, buf: Vec::new() } }

    fn fill(&mut self) -> Result<usize, ReadError> {
        let mut chunk = [0u8; 8192];
        let n = self.stream.read(&mut chunk)?;
        self.buf.extend_from_slice(&chunk[..n]);
        Ok(n)
    }

    /// The next request; `body_max` gives the bound of a route's body from its method and path.
    pub fn read(&mut self, body_max: impl Fn(&str, &str) -> usize) -> Result<Request, ReadError> {
        let end = loop {
            if let Some(i) = self.buf.windows(4).position(|w| w == b"\r\n\r\n") { break i; }
            if self.buf.len() > HEAD_MAX { return Err(ReadError::Bad(431, "the head is over 16 KiB")); }
            if self.fill()? == 0 {
                return Err(if self.buf.is_empty() { ReadError::Closed } else { ReadError::Bad(400, "the connection ended inside a head") });
            }
        };
        if end > HEAD_MAX { return Err(ReadError::Bad(431, "the head is over 16 KiB")); }
        let head = std::str::from_utf8(&self.buf[..end]).map_err(|_| ReadError::Bad(400, "the head is not UTF-8"))?.to_string();
        self.buf.drain(..end + 4);
        let mut lines = head.split("\r\n");
        let line = lines.next().unwrap_or("");
        let mut parts = line.split(' ');
        let (Some(method), Some(target), Some(version), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
            return Err(ReadError::Bad(400, "not a request line"));
        };
        let close_by_version = match version { "HTTP/1.1" => false, "HTTP/1.0" => true, _ => return Err(ReadError::Bad(505, "HTTP/1.1 only")) };
        if !matches!(method, "GET" | "POST" | "PUT" | "DELETE") { return Err(ReadError::Bad(405, "GET, POST, PUT or DELETE")); }
        if !target.starts_with('/') || target.len() > TARGET_MAX || target.bytes().any(|b| !(0x21..0x7f).contains(&b)) {
            return Err(ReadError::Bad(400, "the target is not a path of printable ASCII"));
        }
        let mut headers = Vec::new();
        for l in lines {
            if l.starts_with(' ') || l.starts_with('\t') { return Err(ReadError::Bad(400, "a folded header")); }
            let (k, v) = l.split_once(':').ok_or(ReadError::Bad(400, "a header without a colon"))?;
            if k.is_empty() || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)) { return Err(ReadError::Bad(400, "a header's name")); }
            headers.push((k.to_string(), v.trim().to_string()));
            if headers.len() > HEADERS_MAX { return Err(ReadError::Bad(431, "over 64 headers")); }
        }
        let find = |name: &str| headers.iter().filter(|(k, _): &&(String, String)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()).collect::<Vec<_>>();
        if !find("transfer-encoding").is_empty() { return Err(ReadError::Bad(501, "a body by Content-Length alone")); }
        let lengths = find("content-length");
        let length = match lengths.as_slice() {
            [] if matches!(method, "POST" | "PUT") => return Err(ReadError::Bad(411, "a body needs its Content-Length")),
            [] => 0,
            [one] if !one.is_empty() && one.len() <= 12 && one.bytes().all(|b| b.is_ascii_digit()) => one.parse::<usize>().unwrap(),
            _ => return Err(ReadError::Bad(400, "one Content-Length of digits")),
        };
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        if length > body_max(method, path) { return Err(ReadError::Bad(413, "the body is over this route's bound")); }
        let close = close_by_version || find("connection").iter().any(|v| v.eq_ignore_ascii_case("close"));
        if length > 0 && find("expect").iter().any(|v| v.eq_ignore_ascii_case("100-continue")) {
            self.stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
            self.stream.flush()?;
        }
        while self.buf.len() < length {
            if self.fill()? == 0 { return Err(ReadError::Bad(400, "the connection ended inside a body")); }
        }
        let body: Vec<u8> = self.buf.drain(..length).collect();
        Ok(Request { method: method.into(), target: target.into(), path: path.into(), query: query.into(), headers, body, close })
    }

    pub fn write(&mut self, response: &Response, close: bool) -> io::Result<()> {
        let mut head = format!("HTTP/1.1 {} {}\r\n", response.status, reason(response.status));
        for (k, v) in &response.headers { head.push_str(&format!("{k}: {v}\r\n")); }
        head.push_str(&format!("Content-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n", response.body.len()));
        head.push_str(if close { "Connection: close\r\n\r\n" } else { "Connection: keep-alive\r\n\r\n" });
        self.stream.write_all(head.as_bytes())?;
        self.stream.write_all(&response.body)?;
        self.stream.flush()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response { pub status: u16, pub headers: Vec<(String, String)>, pub body: Vec<u8> }

/// The pages run no script and load nothing from elsewhere; forms go to the hub alone.
pub const PAGE_POLICY: &str = "default-src 'none'; style-src 'unsafe-inline'; img-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'";

impl Response {
    pub fn json(status: u16, body: String) -> Response {
        Response { status, headers: vec![("Content-Type".into(), "application/json".into())], body: body.into_bytes() }
    }
    pub fn html(status: u16, body: String) -> Response {
        Response { status, headers: vec![("Content-Type".into(), "text/html; charset=utf-8".into()), ("Content-Security-Policy".into(), PAGE_POLICY.into())],
                   body: body.into_bytes() }
    }
    pub fn text(status: u16, body: &str) -> Response {
        Response { status, headers: vec![("Content-Type".into(), "text/plain; charset=utf-8".into())], body: body.as_bytes().to_vec() }
    }
    /// After a form: back to a page by GET.
    pub fn see(to: &str) -> Response { Response { status: 303, headers: vec![("Location".into(), to.into())], body: Vec::new() } }
    pub fn with(mut self, name: &str, value: String) -> Response { self.headers.push((name.into(), value)); self }
}

pub fn reason(status: u16) -> &'static str {
    match status {
        100 => "Continue", 200 => "OK", 201 => "Created", 303 => "See Other", 400 => "Bad Request", 401 => "Unauthorized", 403 => "Forbidden",
        404 => "Not Found", 405 => "Method Not Allowed", 409 => "Conflict", 411 => "Length Required", 413 => "Content Too Large",
        429 => "Too Many Requests", 431 => "Request Header Fields Too Large", 500 => "Internal Server Error", 501 => "Not Implemented",
        503 => "Service Unavailable", 505 => "HTTP Version Not Supported", _ => "Status",
    }
}
