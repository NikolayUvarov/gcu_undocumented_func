//! Small things the hub's parts share: the clock, random bytes, hex, a source's network, HTML and form escapes.

use std::io::Read;
use std::net::IpAddr;
use std::time::{SystemTime, UNIX_EPOCH};

pub const HOUR_MS: u64 = 3_600_000;
pub use improve_proto::DAY_MS;

/// The hub's clock, in milliseconds since 1970; a clock before that reads as 0.
pub fn now_ms() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }

/// Bytes from the kernel's random source; the hub stops rather than run without one.
pub fn random<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut out)).expect("/dev/urandom");
    out
}

pub fn hex(bytes: &[u8]) -> String { improve_proto::hex(bytes) }
pub fn sha256(bytes: &[u8]) -> [u8; 32] { use sha2::Digest; sha2::Sha256::digest(bytes).into() }

/// The network a source counts in for anti-flood (docs/hub/README.md, 5.4): an IPv4 address, an IPv6 /64.
pub fn network(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(a) => a.to_string(),
        IpAddr::V6(a) => match a.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => { let s = a.segments(); format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3]) }
        },
    }
}

/// A pairing code or a sign-in code: PAIR_LEN characters of PAIR_ALPHABET, each from one random byte without bias.
pub fn code() -> String {
    let alphabet = improve_proto::form::PAIR_ALPHABET.as_bytes();
    let mut out = String::new();
    while out.len() < improve_proto::form::PAIR_LEN {
        let [b] = random::<1>();
        // 31 letters: bytes from 248 up would favour the first ones, so they are drawn again.
        if (b as usize) < 248 { out.push(alphabet[b as usize % alphabet.len()] as char); }
    }
    out
}

/// Text for HTML, element content or a quoted attribute.
pub fn html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// One `application/x-www-form-urlencoded` or query component decoded; None when it is not UTF-8 or a `%` is broken.
pub fn unescape(text: &str) -> Option<String> {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => { out.push(b' '); i += 1; }
            b'%' => {
                let h = b.get(i + 1..i + 3)?;
                let v = u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()?;
                out.push(v);
                i += 3;
            }
            c => { out.push(c); i += 1; }
        }
    }
    String::from_utf8(out).ok()
}

/// A form's or a query's fields, each key once; None when one is broken or comes twice.
pub fn fields(text: &str) -> Option<Vec<(String, String)>> {
    let mut out: Vec<(String, String)> = Vec::new();
    for part in text.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = part.split_once('=').unwrap_or((part, ""));
        let (k, v) = (unescape(k)?, unescape(v)?);
        if out.iter().any(|(o, _)| *o == k) { return None; }
        out.push((k, v));
    }
    Some(out)
}

pub fn field<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> { fields.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str()) }

/// A form's text with its line ends as the API keeps them: CR LF and lone CR become LF.
pub fn lines(text: &str) -> String { text.replace("\r\n", "\n").replace('\r', "\n") }

/// A release's or a version's order, as `improve` reads it too (improve/proto).
pub use improve_proto::runs_release as at_least;
