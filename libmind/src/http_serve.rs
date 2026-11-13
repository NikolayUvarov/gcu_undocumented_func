//! The server's side of HTTP/1.x (110-NET-0070), with the rules of `tools/httpd` (stage 1): a request head parsed
//! within its bounds (the parser service's `http-request`), one byte range of a file, the release layout's paths
//! (scripts/release.py), and an entity tag matched. No allocation: every field is held in place.

/// The largest head a server reads, in bytes, and the most header lines.
pub const HEAD_MAX: usize = 8192;
pub const HEADERS_MAX: usize = 64;
/// The longest target, `Range` and `If-None-Match` kept; a longer `Range` or `If-None-Match` is not kept (and is then
/// as if absent: the whole file, no 304).
pub const TARGET_MAX: usize = 1024;
pub const VALUE_MAX: usize = 128;

/// Printable ASCII (spaces included) of at most `N` bytes, kept in place.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Text<const N: usize> { bytes: [u8; N], len: u16 }

impl<const N: usize> Text<N> {
    pub const EMPTY: Self = Text { bytes: [0; N], len: 0 };
    /// `text` if it is printable ASCII of at most `N` bytes.
    pub fn new(text: &str) -> Option<Self> {
        if text.len() > N || !text.bytes().all(|b| b == b' ' || b.is_ascii_graphic()) { return None; }
        let mut kept = Self::EMPTY;
        kept.bytes[..text.len()].copy_from_slice(text.as_bytes());
        kept.len = text.len() as u16;
        Some(kept)
    }
    pub fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("") }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}

impl<const N: usize> core::fmt::Debug for Text<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { core::fmt::Debug::fmt(self.as_str(), f) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method { Get, Head }

/// A head a server serves: the method, the target as sent, the version, whether the connection stays open after its
/// response, the `Range` and `If-None-Match` values (empty: none), and whether the request has a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub target: Text<TARGET_MAX>,
    pub http11: bool,
    pub keep_alive: bool,
    pub range: Text<VALUE_MAX>,
    pub if_none_match: Text<VALUE_MAX>,
    pub body: bool,
}

/// Why a head is refused: the status and the reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bad { Malformed, Method, Version, NoHost, TooManyHeaders, TooLong }

impl Bad {
    pub fn status(self) -> u16 {
        match self { Bad::Method => 405, Bad::Version => 505, Bad::TooManyHeaders => 431, Bad::TooLong => 414, _ => 400 }
    }
    pub fn text(self) -> &'static str {
        match self {
            Bad::Malformed => "a malformed request head",
            Bad::Method => "only GET and HEAD are served",
            Bad::Version => "only HTTP/1.1 and HTTP/1.0 are served",
            Bad::NoHost => "an HTTP/1.1 request without Host",
            Bad::TooManyHeaders => "more header lines than the server reads",
            Bad::TooLong => "a target longer than the server reads",
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

// Whether a comma-separated header value holds `token`, in any case.
fn holds(value: &str, token: &str) -> bool { value.split(',').any(|t| t.trim().eq_ignore_ascii_case(token)) }

/// Parses a whole head (up to its blank line) as `tools/httpd` does: the first of each header kept.
pub fn parse_request(head: &[u8]) -> Result<Request, Bad> {
    let text = core::str::from_utf8(head).map_err(|_| Bad::Malformed)?;
    let mut lines = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l));
    let first = lines.next().ok_or(Bad::Malformed)?;
    let mut parts = first.split(' ');
    let (Some(method), Some(target), Some(version), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else { return Err(Bad::Malformed) };
    let http11 = match version { "HTTP/1.1" => true, "HTTP/1.0" => false, v if v.starts_with("HTTP/") => return Err(Bad::Version), _ => return Err(Bad::Malformed) };
    if !token(method) || target.is_empty() { return Err(Bad::Malformed); }
    let method = match method { "GET" => Method::Get, "HEAD" => Method::Head, _ => return Err(Bad::Method) };
    let target = Text::new(target).ok_or(if target.len() > TARGET_MAX { Bad::TooLong } else { Bad::Malformed })?;
    let (mut host, mut connection, mut range, mut tag, mut length, mut coded, mut count) = (false, None, None, None, None, false, 0);
    for line in lines {
        if line.is_empty() { break; }
        // A folded line (obs-fold) is refused, as RFC 9112 allows a server to.
        if line.starts_with(' ') || line.starts_with('\t') { return Err(Bad::Malformed); }
        let (name, value) = line.split_once(':').ok_or(Bad::Malformed)?;
        if !token(name) { return Err(Bad::Malformed); }
        if count == HEADERS_MAX { return Err(Bad::TooManyHeaders); }
        count += 1;
        let value = value.trim_matches(|c| c == ' ' || c == '\t');
        let is = |n: &str| name.eq_ignore_ascii_case(n);
        if is("host") { host = true; }
        else if is("connection") && connection.is_none() { connection = Some(value); }
        else if is("range") && range.is_none() { range = Some(value); }
        else if is("if-none-match") && tag.is_none() { tag = Some(value); }
        else if is("content-length") && length.is_none() { length = Some(value); }
        else if is("transfer-encoding") { coded = true; }
    }
    if http11 && !host { return Err(Bad::NoHost); }
    let connection = connection.unwrap_or("");
    let keep_alive = if http11 { !holds(connection, "close") } else { holds(connection, "keep-alive") };
    let kept = |value: Option<&str>| value.and_then(Text::new).unwrap_or(Text::EMPTY);
    let body = length.is_some_and(|v| v != "0") || coded;
    Ok(Request { method, target, http11, keep_alive, range: kept(range), if_none_match: kept(tag), body })
}

/// What a `Range` header asks of a file of `len` bytes (RFC 9110, section 14): one range per request, which the
/// updater uses to resume a download (351-NET-0001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    /// The whole file: no header, more than one range, or one the server ignores as RFC 9110 allows.
    Whole,
    /// Bytes `start..=end`.
    Part(u64, u64),
    /// None of the file (416).
    Unsatisfiable,
}

fn number(s: &str) -> Option<u64> { if s.is_empty() || s.len() > 19 || !s.bytes().all(|b| b.is_ascii_digit()) { None } else { s.parse().ok() } }

pub fn asked(header: Option<&str>, len: u64) -> Asked {
    let Some(value) = header else { return Asked::Whole };
    let Some(spec) = value.trim().strip_prefix("bytes=") else { return Asked::Whole };
    if spec.contains(',') { return Asked::Whole; }
    let Some((first, last)) = spec.trim().split_once('-') else { return Asked::Whole };
    match (first.is_empty(), number(first), number(last)) {
        // `-K`: the last K bytes.
        (true, _, Some(k)) => if k == 0 || len == 0 { Asked::Unsatisfiable } else { Asked::Part(len.saturating_sub(k), len - 1) },
        // `N-`: from N to the end.
        (false, Some(n), None) if last.is_empty() => if n < len { Asked::Part(n, len - 1) } else { Asked::Unsatisfiable },
        // `N-M`: N to M, M cut to the end.
        (false, Some(n), Some(m)) if n <= m => if n < len { Asked::Part(n, m.min(len - 1)) } else { Asked::Unsatisfiable },
        _ => Asked::Whole,
    }
}

/// Whether `If-None-Match` names `tag` (or is `*`), weak or strong.
pub fn matches_tag(header: Option<&str>, tag: &str) -> bool {
    header.is_some_and(|h| h.split(',').map(|t| t.trim().trim_start_matches("W/")).any(|t| t == "*" || t == tag))
}

/// What a path of the release layout names; a blob never changes, a channel, its timestamp and the key file are fetched fresh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Blob, Manifest, Channel }

/// Why a target is not served: the status and the reason the response and the log give.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal { NotAbsolute, Query, Byte, EmptyName, DotName, NotInLayout, NotAFile, Missing }

impl Refusal {
    pub fn status(self) -> u16 {
        match self { Refusal::NotAbsolute | Refusal::Query | Refusal::Byte => 400, _ => 404 }
    }
    pub fn text(self) -> &'static str {
        match self {
            Refusal::NotAbsolute => "the target is not an absolute path",
            Refusal::Query => "a query string is not served",
            Refusal::Byte => "a byte outside the names' alphabet (A-Z a-z 0-9 . _ - /)",
            Refusal::EmptyName => "an empty name in the path",
            Refusal::DotName => "a name starting with a dot",
            Refusal::NotInLayout => "not a file of the release layout",
            Refusal::NotAFile => "not a regular file",
            Refusal::Missing => "no such file",
        }
    }
}

const ARCHES: [&str; 2] = ["x86_64", "aarch64"];
const MANIFESTS: [&str; 2] = ["MANIFEST", "MANIFEST.SIG"];

fn digits(s: &str) -> bool { !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit()) }
fn sha256_hex(s: &str) -> bool { s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) }
fn channel_name(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty() && b.len() <= 64 && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-'))
}

/// The kind of file a request target names and its path below the layout's root (the target without its first `/`),
/// or why it is refused; nothing is opened here.
pub fn resolve(target: &str) -> Result<(&str, Kind), Refusal> {
    if target.contains('?') { return Err(Refusal::Query); }
    if !target.starts_with('/') { return Err(Refusal::NotAbsolute); }
    if !target.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/')) { return Err(Refusal::Byte); }
    let relative = &target[1..];
    if relative.split('/').any(|n| n.is_empty()) { return Err(Refusal::EmptyName); }
    if relative.split('/').any(|n| n.starts_with('.')) { return Err(Refusal::DotName); }
    let mut names = [""; 5];
    let mut count = 0;
    for name in relative.split('/') {
        if count == names.len() { return Err(Refusal::NotInLayout); }
        names[count] = name;
        count += 1;
    }
    let kind = match &names[..count] {
        ["blobs", digest] if sha256_hex(digest) => Kind::Blob,
        ["releases", version, arch, file] if digits(version) && ARCHES.contains(arch) && MANIFESTS.contains(file) => Kind::Manifest,
        ["channels", name] if channel_name(name) => Kind::Channel,
        ["keys", "current"] => Kind::Channel, // the key file (351-UPD-0026), fetched fresh as a channel is
        ["keys", version] if digits(version) => Kind::Manifest, // its history (351-UPD-0027): a version never changes
        ["timestamps", name] if channel_name(name) => Kind::Channel, // a channel's timestamp (351-UPD-0028), fetched fresh
        _ => return Err(Refusal::NotInLayout),
    };
    Ok((relative, kind))
}
