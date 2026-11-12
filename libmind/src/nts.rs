//! NTS (RFC 8915) for `timed` (111-NET-0075): the NTS-KE request and the response's records, and the extension fields
//! of an NTS-protected NTP request and answer. No I/O and no cipher: the caller seals and opens with
//! AEAD_AES_SIV_CMAC_256 under the keys the TLS exporter gave it.
extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// NTS-KE's TCP port.
pub const KE_PORT: u16 = 4460;
/// The application protocol NTS-KE's TLS agrees on.
pub const ALPN: &str = "ntske/1";
/// The TLS exporter's label for the keys.
pub const EXPORTER_LABEL: &str = "EXPORTER-network-time-security";
/// The only AEAD offered: AEAD_AES_SIV_CMAC_256 (IANA AEAD 15).
pub const AEAD_AES_SIV_CMAC_256: u16 = 15;
/// The only next protocol: NTPv4 (IANA NTS next protocol 0).
pub const NTPV4: u16 = 0;
/// A key's length, either direction.
pub const KEY: usize = 32;
/// The cookies kept for one server (RFC 8915, section 5.7: placeholders so that eight stay unused).
pub const COOKIES: usize = 8;
/// The longest cookie taken.
pub const COOKIE_MAX: usize = 512;
/// The longest NTS-KE response read.
pub const RESPONSE_MAX: usize = 16 * 1024;
/// The unique identifier's length.
pub const UID: usize = 32;
/// The nonce the client sends: 16 bytes, so that no additional padding is needed (RFC 8915, section 5.6).
pub const NONCE: usize = 16;
/// AES-SIV's tag, which leads its ciphertext.
pub const TAG: usize = 16;

const CRITICAL: u16 = 0x8000;
const END: u16 = 0;
const NEXT_PROTOCOL: u16 = 1;
const ERROR: u16 = 2;
const WARNING: u16 = 3;
const AEAD: u16 = 4;
const NEW_COOKIE: u16 = 5;
const SERVER: u16 = 6;
const PORT: u16 = 7;

/// Extension field types (RFC 8915, section 5).
pub const FIELD_UID: u16 = 0x0104;
pub const FIELD_COOKIE: u16 = 0x0204;
pub const FIELD_PLACEHOLDER: u16 = 0x0304;
pub const FIELD_AUTHENTICATOR: u16 = 0x0404;

/// The exporter's context of one direction: NTPv4, AEAD_AES_SIV_CMAC_256, 0 client to server, 1 server to client.
pub fn context(server_to_client: bool) -> [u8; 5] {
    let [p0, p1] = NTPV4.to_be_bytes();
    let [a0, a1] = AEAD_AES_SIV_CMAC_256.to_be_bytes();
    [p0, p1, a0, a1, server_to_client as u8]
}

fn record(out: &mut Vec<u8>, kind: u16, body: &[u8]) {
    out.extend_from_slice(&kind.to_be_bytes());
    out.extend_from_slice(&(body.len() as u16).to_be_bytes());
    out.extend_from_slice(body);
}

/// The NTS-KE request: NTPv4, AEAD_AES_SIV_CMAC_256, end of message, every record critical.
pub fn ke_request() -> Vec<u8> {
    let mut out = Vec::new();
    record(&mut out, CRITICAL | NEXT_PROTOCOL, &NTPV4.to_be_bytes());
    record(&mut out, CRITICAL | AEAD, &AEAD_AES_SIV_CMAC_256.to_be_bytes());
    record(&mut out, CRITICAL | END, &[]);
    out
}

/// What an NTS-KE response gave: the cookies (at most COOKIES), and the NTP server and port it named, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ke { pub cookies: Vec<Vec<u8>>, pub server: Option<String>, pub port: Option<u16> }

/// Why an NTS-KE response was not taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeRefused {
    /// No end of message yet: more bytes may come.
    Incomplete,
    /// Longer than RESPONSE_MAX before its end.
    TooLong,
    /// The server's error record, with its code.
    Error(u16),
    /// A warning record, with its code: no warning is defined, so the session ends (RFC 8915, section 4.1.4).
    Warning(u16),
    /// No next protocol record, more than one, or one that is not NTPv4 alone.
    Protocol,
    /// No AEAD record, more than one, or one that is not AEAD_AES_SIV_CMAC_256 alone.
    Aead,
    /// A critical record of a type not known.
    Critical(u16),
    /// No cookie, an empty one or one longer than COOKIE_MAX.
    Cookie,
    /// A server or port record that is not well formed.
    Malformed,
}

impl KeRefused {
    pub fn text(&self) -> &'static str {
        match self {
            KeRefused::Incomplete => "NO END OF MESSAGE",
            KeRefused::TooLong => "LONGER THAN 16 KIB",
            KeRefused::Error(_) => "THE SERVER'S ERROR",
            KeRefused::Warning(_) => "THE SERVER'S WARNING",
            KeRefused::Protocol => "NOT NTPV4 ALONE AS THE NEXT PROTOCOL",
            KeRefused::Aead => "NOT AEAD_AES_SIV_CMAC_256 ALONE",
            KeRefused::Critical(_) => "A CRITICAL RECORD NOT KNOWN",
            KeRefused::Cookie => "NO COOKIE, OR ONE EMPTY OR LONGER THAN 512 BYTES",
            KeRefused::Malformed => "A SERVER OR PORT RECORD NOT WELL FORMED",
        }
    }
}

/// The server's response, read up to its end of message; what follows the end is not looked at.
pub fn ke_response(data: &[u8]) -> Result<Ke, KeRefused> {
    let (mut at, mut ke) = (0, Ke { cookies: Vec::new(), server: None, port: None });
    let (mut protocol, mut aead) = (None::<bool>, None::<bool>);
    loop {
        if at >= RESPONSE_MAX { return Err(KeRefused::TooLong); }
        let Some(head) = data.get(at..at + 4) else { return Err(if data.len() >= RESPONSE_MAX { KeRefused::TooLong } else { KeRefused::Incomplete }) };
        let (kind, length) = (u16::from_be_bytes([head[0], head[1]]), u16::from_be_bytes([head[2], head[3]]) as usize);
        let Some(body) = data.get(at + 4..at + 4 + length) else { return Err(if at + 4 + length > RESPONSE_MAX { KeRefused::TooLong } else { KeRefused::Incomplete }) };
        at += 4 + length;
        let pairs = |body: &[u8]| (body.len() % 2 == 0).then(|| body.chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect::<Vec<_>>());
        match kind & !CRITICAL {
            END => break,
            NEXT_PROTOCOL => {
                let alone = pairs(body).is_some_and(|ids| ids == [NTPV4]);
                if protocol.is_some() { return Err(KeRefused::Protocol); }
                protocol = Some(alone);
            }
            ERROR => return Err(KeRefused::Error(pairs(body).and_then(|c| c.first().copied()).unwrap_or(u16::MAX))),
            WARNING => return Err(KeRefused::Warning(pairs(body).and_then(|c| c.first().copied()).unwrap_or(u16::MAX))),
            AEAD => {
                let alone = pairs(body).is_some_and(|ids| ids == [AEAD_AES_SIV_CMAC_256]);
                if aead.is_some() { return Err(KeRefused::Aead); }
                aead = Some(alone);
            }
            NEW_COOKIE => {
                if body.is_empty() || body.len() > COOKIE_MAX { return Err(KeRefused::Cookie); }
                if ke.cookies.len() < COOKIES { ke.cookies.push(body.to_vec()); }
            }
            SERVER => {
                let name = core::str::from_utf8(body).ok().filter(|n| !n.is_empty() && n.len() <= 253 && n.bytes().all(|b| b.is_ascii_graphic()));
                ke.server = Some(String::from(name.ok_or(KeRefused::Malformed)?));
            }
            PORT => {
                if body.len() != 2 { return Err(KeRefused::Malformed); }
                ke.port = Some(u16::from_be_bytes([body[0], body[1]]));
            }
            other if kind & CRITICAL != 0 => return Err(KeRefused::Critical(other)),
            _ => {}
        }
    }
    if protocol != Some(true) { return Err(KeRefused::Protocol); }
    if aead != Some(true) { return Err(KeRefused::Aead); }
    if ke.cookies.is_empty() { return Err(KeRefused::Cookie); }
    Ok(ke)
}

// An extension field: its value zero-padded to a multiple of 4, and to 12 bytes at least (RFC 7822's 16 in all).
fn field(out: &mut Vec<u8>, kind: u16, value: &[u8]) {
    let padded = ((value.len() + 3) & !3).max(12);
    out.extend_from_slice(&kind.to_be_bytes());
    out.extend_from_slice(&((4 + padded) as u16).to_be_bytes());
    out.extend_from_slice(value);
    out.resize(out.len() + padded - value.len(), 0);
}

/// The placeholders that bring the unused cookies back to COOKIES once the answer's come, with `left` still unused
/// after this request's (at most 7).
pub fn placeholders(left: usize) -> usize { (COOKIES - 1).saturating_sub(left).min(COOKIES - 1) }

/// An NTS-protected request: `header` (SNTP's 48 bytes), the unique identifier, the cookie, `placeholders` cookie
/// placeholders of its length, and the authenticator, whose tag `seal(associated data, nonce)` makes over everything
/// before it with the client-to-server key (an empty plaintext: the tag alone).
pub fn request(header: &[u8], uid: &[u8; UID], cookie: &[u8], placeholders: usize, nonce: &[u8; NONCE], seal: impl FnOnce(&[u8], &[u8]) -> [u8; TAG]) -> Vec<u8> {
    let mut out = Vec::with_capacity(header.len() + 40 + (cookie.len() + 8) * (placeholders + 1) + 40);
    out.extend_from_slice(header);
    field(&mut out, FIELD_UID, uid);
    field(&mut out, FIELD_COOKIE, cookie);
    let zeros = alloc::vec![0u8; cookie.len()];
    for _ in 0..placeholders.min(COOKIES - 1) { field(&mut out, FIELD_PLACEHOLDER, &zeros); }
    let tag = seal(&out, nonce);
    let mut value = Vec::with_capacity(4 + NONCE + TAG);
    value.extend_from_slice(&(NONCE as u16).to_be_bytes());
    value.extend_from_slice(&(TAG as u16).to_be_bytes());
    value.extend_from_slice(nonce);
    value.extend_from_slice(&tag);
    field(&mut out, FIELD_AUTHENTICATOR, &value);
    out
}

/// Why an NTS answer was not taken, past its 48-byte header's own checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Its extension fields do not parse (RFC 7822's lengths).
    Fields,
    /// No unique identifier, or not this request's: a replayed or stray answer.
    NotOurs,
    /// No authenticator, or not the last field.
    NoAuthenticator,
    /// The authenticator's tag does not hold under the server-to-client key: a forged or damaged answer.
    NotAuthentic,
    /// No new cookie in what it sealed.
    NoCookie,
    /// The server's NTS NAK (kiss code NTSN) for this request: it no longer takes the cookies.
    Nak,
}

impl Refused {
    pub fn text(&self) -> &'static str {
        match self {
            Refused::Fields => "ITS EXTENSION FIELDS DO NOT PARSE",
            Refused::NotOurs => "ITS UNIQUE IDENTIFIER IS NOT OUR REQUEST'S",
            Refused::NoAuthenticator => "NO AUTHENTICATOR AS ITS LAST FIELD",
            Refused::NotAuthentic => "NOT AUTHENTIC UNDER THE SERVER'S KEY",
            Refused::NoCookie => "NO NEW COOKIE",
            Refused::Nak => "THE SERVER'S NTS NAK: NEW KEYS NEEDED",
        }
    }
}

/// The extension fields after the header: each (type, value with its padding, offset of the field). None when a length
/// is not a multiple of 4, below 16 (outside encrypted fields), or runs past the end.
pub fn fields(data: &[u8], at: usize, minimum: usize) -> Option<Vec<(u16, &[u8], usize)>> {
    let mut out = Vec::new();
    let mut at = at;
    while at < data.len() {
        let head = data.get(at..at + 4)?;
        let (kind, length) = (u16::from_be_bytes([head[0], head[1]]), u16::from_be_bytes([head[2], head[3]]) as usize);
        if length % 4 != 0 || length < minimum.max(4) { return None; }
        out.push((kind, data.get(at + 4..at + length)?, at));
        at += length;
    }
    Some(out)
}

/// An NTS answer to the request with `uid`: its new cookies, once `open(associated data, nonce, ciphertext)` has
/// opened the authenticator with the server-to-client key (None: not authentic). The header (`answer[..48]`) is
/// SNTP's to check. Fields after the authenticator are not authenticated and are not looked at; a kiss-o'-death NTSN
/// with this request's identifier and no authenticator is `Nak`.
pub fn answer(packet: &[u8], uid: &[u8; UID], open: impl FnOnce(&[u8], &[u8], &[u8]) -> Option<Vec<u8>>) -> Result<Vec<Vec<u8>>, Refused> {
    if packet.len() < 48 { return Err(Refused::Fields); }
    let list = fields(packet, 48, 16).ok_or(Refused::Fields)?;
    let ours = |list: &[(u16, &[u8], usize)]| list.iter().any(|&(kind, value, _)| kind == FIELD_UID && value.get(..UID) == Some(&uid[..]) && value[UID..].iter().all(|&b| b == 0));
    let authenticator = list.iter().position(|&(kind, _, _)| kind == FIELD_AUTHENTICATOR);
    if packet[1] == 0 && &packet[12..16] == b"NTSN" && authenticator.is_none() {
        return if ours(&list) { Err(Refused::Nak) } else { Err(Refused::NotOurs) };
    }
    let Some(index) = authenticator else { return Err(Refused::NoAuthenticator) };
    // Only what the authenticator covers counts: the identifier must be before it.
    if !ours(&list[..index]) { return Err(Refused::NotOurs); }
    let (_, value, start) = list[index];
    if value.len() < 4 { return Err(Refused::Fields); }
    let (nonce_length, sealed_length) = (u16::from_be_bytes([value[0], value[1]]) as usize, u16::from_be_bytes([value[2], value[3]]) as usize);
    let nonce_end = 4 + nonce_length;
    let sealed_at = 4 + ((nonce_length + 3) & !3);
    let (Some(nonce), Some(sealed)) = (value.get(4..nonce_end), value.get(sealed_at..sealed_at + sealed_length)) else { return Err(Refused::Fields) };
    if nonce.is_empty() || sealed.len() < TAG { return Err(Refused::Fields); }
    let plain = open(&packet[..start], nonce, sealed).ok_or(Refused::NotAuthentic)?;
    let inner = fields(&plain, 0, 4).ok_or(Refused::Fields)?;
    let cookies: Vec<Vec<u8>> = inner.iter().filter(|&&(kind, value, _)| kind == FIELD_COOKIE && !value.is_empty() && value.len() <= COOKIE_MAX)
        .take(COOKIES).map(|&(_, value, _)| value.to_vec()).collect();
    if cookies.is_empty() { return Err(Refused::NoCookie); }
    Ok(cookies)
}
