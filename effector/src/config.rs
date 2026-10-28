//! The agent's configuration (`data/effector/effector.conf`), written by the owner, parsed whole or refused whole
//! (750-EFF-0006). The token is kept apart from it and never printed; the server cannot change any of it but the pins'
//! rotation, which contract 1.0 does over the verified connection.

pub const FILE_MAX: usize = 2048;
pub const URL_MAX: usize = 200;
pub const NAME_MAX: usize = 64;
pub const TOKEN_MAX: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind { TooLarge, NotText, Version, UnknownKey, Malformed, Duplicate, Missing }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error { pub line: u16, pub kind: ErrorKind }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinKind { Leaf, Spki }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pin { pub kind: PinKind, pub digest: [u8; 32] }

#[derive(Clone, Copy, Debug)]
struct Text<const N: usize> { bytes: [u8; N], len: usize }
impl<const N: usize> Text<N> {
    fn new(s: &str) -> Option<Self> {
        if s.len() > N { return None; }
        let mut t = Self { bytes: [0; N], len: s.len() };
        t.bytes[..s.len()].copy_from_slice(s.as_bytes());
        Some(t)
    }
    fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("") }
}

/// The server's origin and base path, from `server https://HOST[:PORT]/PATH`.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    url: Text<URL_MAX>,
    host: (usize, usize),
    port: u16,
    path: (usize, usize),
    pins: [Option<Pin>; 2],
    name: Option<Text<NAME_MAX>>,
    address: Option<u32>,
    hostname: Text<NAME_MAX>,
    no_token: bool,
}

fn hex32(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    if b.len() != 64 { return None; }
    let mut out = [0u8; 32];
    for (i, pair) in b.chunks(2).enumerate() {
        let d = |c: u8| match c { b'0'..=b'9' => Some(c - b'0'), b'a'..=b'f' => Some(c - b'a' + 10), b'A'..=b'F' => Some(c - b'A' + 10), _ => None };
        out[i] = d(pair[0])? << 4 | d(pair[1])?;
    }
    Some(out)
}

pub fn ipv4(text: &str) -> Option<u32> {
    let mut address = [0u8; 4];
    let mut parts = text.split('.');
    for byte in address.iter_mut() {
        let p = parts.next()?;
        if p.is_empty() || p.len() > 3 || !p.bytes().all(|c| c.is_ascii_digit()) || (p.len() > 1 && p.starts_with('0')) { return None; }
        *byte = p.parse().ok()?;
    }
    parts.next().is_none().then_some(u32::from_be_bytes(address))
}

fn host_ok(h: &str) -> bool {
    !h.is_empty() && h.len() <= 253 && h.split('.').all(|l| !l.is_empty() && l.len() <= 63 && l.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') && !l.starts_with('-') && !l.ends_with('-'))
}

/// Checks `https://HOST[:PORT][/PATH]` and returns the host's and the path's spans and the port.
fn server(url: &str) -> Option<((usize, usize), u16, (usize, usize))> {
    let rest = url.strip_prefix("https://")?;
    let base = 8;
    if rest.bytes().any(|c| !(0x21..0x7f).contains(&c) || matches!(c, b'?' | b'#' | b'@' | b'\\' | b'%')) { return None; }
    let (authority, path) = match rest.find('/') { Some(i) => (&rest[..i], &rest[i..]), None => (rest, "") };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => { if p.is_empty() || p.len() > 5 || !p.bytes().all(|c| c.is_ascii_digit()) { return None; } (h, p.parse::<u32>().ok().filter(|&n| (1..=65535).contains(&n))? as u16) }
        None => (authority, 443),
    };
    if !host_ok(host) { return None; }
    let path = path.strip_suffix('/').unwrap_or(path);
    if path.split('/').skip(1).any(|p| p.is_empty() || p == "." || p == "..") { return None; }
    let path_start = base + authority.len();
    Some(((base, base + host.len()), port, (path_start, path_start + path.len())))
}

impl Config {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > FILE_MAX { return Err(Error { line: 0, kind: ErrorKind::TooLarge }); }
        let text = core::str::from_utf8(bytes).map_err(|_| Error { line: 0, kind: ErrorKind::NotText })?;
        let (mut version, mut url, mut pins, mut name, mut address, mut hostname, mut no_token) = (false, None, [None; 2], None, None, None, None);
        let mut parts = None;
        for (i, raw) in text.split('\n').enumerate() {
            let line_no = (i + 1).min(u16::MAX as usize) as u16;
            let err = |kind| Error { line: line_no, kind };
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if line.bytes().any(|c| c < 0x20 && c != b'\t' || c == 0x7f) { return Err(err(ErrorKind::NotText)); }
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let mut words = line.split_ascii_whitespace();
            let (key, value) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
            if words.next().is_some() || value.is_empty() { return Err(err(if version { ErrorKind::Malformed } else { ErrorKind::Version })); }
            if !version {
                if key != "effector-config" || value != "1" { return Err(err(ErrorKind::Version)); }
                version = true;
                continue;
            }
            match key {
                "server" => {
                    if url.is_some() { return Err(err(ErrorKind::Duplicate)); }
                    if value.len() > URL_MAX { return Err(err(ErrorKind::Malformed)); }
                    parts = Some(server(value).ok_or(err(ErrorKind::Malformed))?);
                    url = Text::<URL_MAX>::new(value);
                }
                "pin" => {
                    let (kind, hex) = value.split_once(':').ok_or(err(ErrorKind::Malformed))?;
                    let kind = match kind { "leaf" => PinKind::Leaf, "spki" => PinKind::Spki, _ => return Err(err(ErrorKind::Malformed)) };
                    let pin = Pin { kind, digest: hex32(hex).ok_or(err(ErrorKind::Malformed))? };
                    if pins.contains(&Some(pin)) { return Err(err(ErrorKind::Duplicate)); }
                    match pins.iter_mut().find(|p| p.is_none()) { Some(slot) => *slot = Some(pin), None => return Err(err(ErrorKind::Malformed)) }
                }
                "name" => {
                    if name.is_some() { return Err(err(ErrorKind::Duplicate)); }
                    if !host_ok(value) || value.len() > NAME_MAX { return Err(err(ErrorKind::Malformed)); }
                    name = Text::<NAME_MAX>::new(value);
                }
                "address" => {
                    if address.is_some() { return Err(err(ErrorKind::Duplicate)); }
                    address = Some(ipv4(value).ok_or(err(ErrorKind::Malformed))?);
                }
                "hostname" => {
                    if hostname.is_some() { return Err(err(ErrorKind::Duplicate)); }
                    if value.len() > NAME_MAX || !value.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') { return Err(err(ErrorKind::Malformed)); }
                    hostname = Text::<NAME_MAX>::new(value);
                }
                // The server asks no agent token (Effector's `security.mode=disabled`): said here, never implied by a missing file.
                "token" => {
                    if no_token.is_some() { return Err(err(ErrorKind::Duplicate)); }
                    if value != "none" { return Err(err(ErrorKind::Malformed)); }
                    no_token = Some(true);
                }
                _ => return Err(err(ErrorKind::UnknownKey)),
            }
        }
        if !version { return Err(Error { line: 0, kind: ErrorKind::Version }); }
        let (url, (host, port, path)) = match (url, parts) { (Some(u), Some(p)) => (u, p), _ => return Err(Error { line: 0, kind: ErrorKind::Missing }) };
        if pins[0].is_none() { return Err(Error { line: 0, kind: ErrorKind::Missing }); }
        let config = Config { url, host, port, path, pins, name, address, hostname: hostname.unwrap_or(Text::new("mindcore").unwrap()), no_token: no_token.unwrap_or(false) };
        // A host name needs an address to reach it: `address`, or the host is itself one.
        if config.address().is_none() { return Err(Error { line: 0, kind: ErrorKind::Missing }); }
        Ok(config)
    }

    pub fn url(&self) -> &str { self.url.as_str() }
    /// `token none`: no token file is read and no `Authorization` header is sent.
    pub fn no_token(&self) -> bool { self.no_token }
    pub fn host(&self) -> &str { &self.url.as_str()[self.host.0..self.host.1] }
    pub fn port(&self) -> u16 { self.port }
    /// The base path without a final `/` (`/effector`, or empty); routes are appended to it.
    pub fn base_path(&self) -> &str { &self.url.as_str()[self.path.0..self.path.1] }
    pub fn pins(&self) -> impl Iterator<Item = &Pin> { self.pins.iter().flatten() }
    /// The name the certificate is checked for: `name`, or the URL's host.
    pub fn tls_name(&self) -> &str { self.name.as_ref().map_or(self.host(), |n| n.as_str()) }
    /// The IPv4 address connected to: `address`, or the URL's host when it is one.
    pub fn address(&self) -> Option<u32> { self.address.or_else(|| ipv4(self.host())) }
    pub fn hostname(&self) -> &str { self.hostname.as_str() }
}

/// The token file: one line of visible ASCII, at most 256 bytes; returned without its line end.
pub fn token(bytes: &[u8]) -> Option<&str> {
    let text = core::str::from_utf8(bytes).ok()?;
    let t = text.strip_suffix('\n').unwrap_or(text);
    let t = t.strip_suffix('\r').unwrap_or(t);
    if t.is_empty() || t.len() > TOKEN_MAX || !t.bytes().all(|c| (0x21..0x7f).contains(&c)) { return None; }
    Some(t)
}

/// A stored agent ID, checked as contract 1.0 asks: 1–64 of `[A-Za-z0-9_-]`.
pub fn agent_id(bytes: &[u8]) -> Option<&str> {
    let text = core::str::from_utf8(bytes).ok()?;
    let t = text.trim_end_matches(['\n', '\r']);
    if t.is_empty() || t.len() > 64 || !t.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-') { return None; }
    Some(t)
}

/// A new agent ID from 64 random bits: `mind_` and 16 hex digits.
pub fn new_agent_id(random: u64, out: &mut [u8; 21]) -> &str {
    out[..5].copy_from_slice(b"mind_");
    for i in 0..16 { out[5 + i] = b"0123456789abcdef"[(random >> (60 - 4 * i)) as usize & 15]; }
    core::str::from_utf8(&out[..]).unwrap()
}

/// The longest machine print and record (750-EFF-0017).
pub const PRINT_MAX: usize = 96;
pub const RECORD_MAX: usize = PRINT_MAX + NAME_MAX + 2;

/// What the machine record beside the kept agent ID says of it (750-EFF-0017).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Machine<'a> {
    /// No record, or an unreadable one: an agent from before the record; this machine is taken as the identity's own.
    Unrecorded,
    /// Recorded on this machine; `foreign` is the hostname the owner's configuration gave the machine the identity was
    /// copied from, empty if it was not copied.
    This { foreign: &'a str },
    /// Recorded on another machine: the identity, and the owner's files with it, were copied here.
    Other { print: &'a str },
}

/// The machine's print: the processor's name and the memory in GiB rounded to a power of two, so that neither the
/// kernel's reserve nor a build's moves it.
pub fn print<'a>(cpu_model: &str, memory_bytes: u64, out: &'a mut [u8; PRINT_MAX]) -> &'a str {
    let gib = (memory_bytes + (1 << 29)) >> 30;
    let pow = if gib == 0 { 0 } else { let lo = 1u64 << (63 - gib.leading_zeros()); if gib - lo >= lo * 2 - gib { lo * 2 } else { lo } };
    let model = cpu_model.get(..cpu_model.len().min(PRINT_MAX - 32)).unwrap_or("");
    let mut at = 0;
    for part in [model, if model.is_empty() { "" } else { ", " }] { out[at..at + part.len()].copy_from_slice(part.as_bytes()); at += part.len(); }
    let mut digits = [0u8; 20];
    let mut n = digits.len();
    let mut v = pow;
    loop { n -= 1; digits[n] = b'0' + (v % 10) as u8; v /= 10; if v == 0 { break; } }
    for part in [&digits[n..], b" GiB"] { out[at..at + part.len()].copy_from_slice(part); at += part.len(); }
    core::str::from_utf8(&out[..at]).unwrap_or("")
}

/// The record read against this machine's print: its first line the print, its second, if any, the foreign hostname.
pub fn machine<'a>(record: Option<&'a [u8]>, print: &str) -> Machine<'a> {
    let Some(text) = record.and_then(|r| core::str::from_utf8(r).ok()) else { return Machine::Unrecorded };
    let mut lines = text.split('\n').map(|l| l.trim_end_matches('\r'));
    let recorded = lines.next().unwrap_or("");
    if recorded.is_empty() || recorded.len() > PRINT_MAX { return Machine::Unrecorded; }
    if recorded != print { return Machine::Other { print: recorded }; }
    let foreign = lines.next().filter(|h| hostname_ok(h)).unwrap_or("");
    Machine::This { foreign }
}

/// The record for this machine, with the hostname written for the machine the identity came from.
pub fn record<'a>(print: &str, foreign: &str, out: &'a mut [u8; RECORD_MAX]) -> &'a [u8] {
    let foreign = if hostname_ok(foreign) { foreign } else { "" };
    let mut at = 0;
    for part in [print.get(..print.len().min(PRINT_MAX)).unwrap_or(""), "\n", foreign, if foreign.is_empty() { "" } else { "\n" }] {
        out[at..at + part.len()].copy_from_slice(part.as_bytes());
        at += part.len();
    }
    &out[..at]
}

/// A hostname of this machine's own while the configuration's was written for another: `mind-` and the processor's
/// name without its maker, marks and clock (`Intel(R) N100` gives `mind-n100`), else `mindcore`.
pub fn own_hostname<'a>(cpu_model: &str, out: &'a mut [u8; NAME_MAX]) -> &'a str {
    const DROPPED: [&str; 6] = ["intel", "amd", "cpu", "processor", "apple", "with"];
    out[..5].copy_from_slice(b"mind-");
    let mut at = 5;
    let model = cpu_model.split('@').next().unwrap_or("");
    for word in model.split_whitespace() {
        // The word in lower case without its marks, (R) and (TM), and without what a hostname cannot hold.
        let (bytes, mut clean, mut n, mut i) = (word.as_bytes(), [0u8; 48], 0, 0);
        while i < bytes.len() {
            if let Some(m) = [&b"(r)"[..], b"(tm)"].iter().find(|m| bytes[i..].len() >= m.len() && bytes[i..i + m.len()].eq_ignore_ascii_case(m)) { i += m.len(); continue; }
            let c = bytes[i].to_ascii_lowercase();
            if (c.is_ascii_alphanumeric() || c == b'-') && n < clean.len() { clean[n] = c; n += 1; }
            i += 1;
        }
        let w = &clean[..n];
        let text = core::str::from_utf8(w).unwrap_or("");
        if text.is_empty() || DROPPED.contains(&text) || at + 1 + w.len() > NAME_MAX { continue; }
        if at > 5 { out[at] = b'-'; at += 1; }
        out[at..at + w.len()].copy_from_slice(w);
        at += w.len();
    }
    if at == 5 { return "mindcore"; }
    core::str::from_utf8(&out[..at]).unwrap_or("mindcore")
}

fn hostname_ok(h: &str) -> bool { !h.is_empty() && h.len() <= NAME_MAX && h.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'.') }
