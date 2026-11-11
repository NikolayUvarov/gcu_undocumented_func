//! A request's head, parsed within its bound: the method, the target, the version and the headers (110).

/// The largest head the server reads, in bytes, and the most header lines.
pub const HEAD_MAX: usize = 8192;
pub const HEADERS_MAX: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method { Get, Head }

#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub target: String,
    pub http11: bool,
    headers: Vec<(String, String)>,
}

impl Request {
    /// The value of a header (its name in any case), the first one.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    /// Whether the connection stays open after this request's response.
    pub fn keep_alive(&self) -> bool {
        let tokens = self.header("connection").unwrap_or("").to_ascii_lowercase();
        let has = |t: &str| tokens.split(',').any(|x| x.trim() == t);
        if self.http11 { !has("close") } else { has("keep-alive") }
    }
}

/// Why a head is refused: the status and the reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bad { Malformed, Method, Version, NoHost, TooManyHeaders }

impl Bad {
    pub fn status(self) -> u16 {
        match self { Bad::Method => 405, Bad::Version => 505, Bad::TooManyHeaders => 431, _ => 400 }
    }
    pub fn text(self) -> &'static str {
        match self {
            Bad::Malformed => "a malformed request head",
            Bad::Method => "only GET and HEAD are served",
            Bad::Version => "only HTTP/1.1 and HTTP/1.0 are served",
            Bad::NoHost => "an HTTP/1.1 request without Host",
            Bad::TooManyHeaders => "more header lines than the server reads",
        }
    }
}

/// Where the head ends in `buf` (the byte after its blank line), if it has ended.
pub fn end(buf: &[u8]) -> Option<usize> {
    (0..buf.len()).find_map(|i| {
        if buf[i..].starts_with(b"\r\n\r\n") { Some(i + 4) } else if buf[i..].starts_with(b"\n\n") { Some(i + 2) } else { None }
    })
}

fn token(s: &str) -> bool { !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)) }

/// Parses a whole head (up to its blank line).
pub fn parse(head: &[u8]) -> Result<Request, Bad> {
    let text = std::str::from_utf8(head).map_err(|_| Bad::Malformed)?;
    let mut lines = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l));
    let first = lines.next().ok_or(Bad::Malformed)?;
    let mut parts = first.split(' ');
    let (Some(method), Some(target), Some(version), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else { return Err(Bad::Malformed) };
    let http11 = match version { "HTTP/1.1" => true, "HTTP/1.0" => false, v if v.starts_with("HTTP/") => return Err(Bad::Version), _ => return Err(Bad::Malformed) };
    if !token(method) || target.is_empty() { return Err(Bad::Malformed); }
    let method = match method { "GET" => Method::Get, "HEAD" => Method::Head, _ => return Err(Bad::Method) };
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() { break; }
        // A folded line (obs-fold) is refused, as RFC 9112 allows a server to.
        if line.starts_with(' ') || line.starts_with('\t') { return Err(Bad::Malformed); }
        let (name, value) = line.split_once(':').ok_or(Bad::Malformed)?;
        if !token(name) { return Err(Bad::Malformed); }
        if headers.len() == HEADERS_MAX { return Err(Bad::TooManyHeaders); }
        headers.push((name.to_string(), value.trim_matches(|c| c == ' ' || c == '\t').to_string()));
    }
    let request = Request { method, target: target.to_string(), http11, headers };
    if http11 && request.header("host").is_none() { return Err(Bad::NoHost); }
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heads() {
        let r = parse(b"GET /channels/stable HTTP/1.1\r\nHost: x\r\nRange: bytes=0-9\r\n\r\n").unwrap();
        assert_eq!((r.method, r.target.as_str(), r.http11), (Method::Get, "/channels/stable", true));
        assert_eq!(r.header("RANGE"), Some("bytes=0-9"));
        assert!(r.keep_alive());
        assert!(!parse(b"GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").unwrap().keep_alive());
        assert!(!parse(b"HEAD / HTTP/1.0\r\n\r\n").unwrap().keep_alive());
        assert!(parse(b"HEAD / HTTP/1.0\nConnection: Keep-Alive\n\n").unwrap().keep_alive());
        assert_eq!(parse(b"POST / HTTP/1.1\r\nHost: x\r\n\r\n"), Err(Bad::Method));
        assert_eq!(parse(b"GET / HTTP/2.0\r\nHost: x\r\n\r\n"), Err(Bad::Version));
        assert_eq!(parse(b"GET / HTTP/1.1\r\n\r\n"), Err(Bad::NoHost));
        assert_eq!(parse(b"GET /  HTTP/1.1\r\nHost: x\r\n\r\n"), Err(Bad::Malformed));
        assert_eq!(parse(b"GET / HTTP/1.1\r\nHost: x\r\n folded\r\n\r\n"), Err(Bad::Malformed));
        assert_eq!(parse(b"GET / HTTP/1.1\r\nHo st: x\r\n\r\n"), Err(Bad::Malformed));
        let many: String = (0..=HEADERS_MAX).map(|i| format!("X-{i}: 1\r\n")).collect();
        assert_eq!(parse(format!("GET / HTTP/1.1\r\nHost: x\r\n{many}\r\n").as_bytes()), Err(Bad::TooManyHeaders));
        assert_eq!(end(b"GET / HTTP/1.1\r\nHost: x\r\n\r\nrest"), Some(27));
        assert_eq!(end(b"GET / HTTP/1.1\r\nHost: x\r\n"), None);
    }
}
