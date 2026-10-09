// A self-signed X.509 v3 certificate (RFC 5280) for an Ed25519 key (RFC 8410), without extensions: the TLS client
// certificate of the device. DER is built by hand: the structure is fixed and small.
use alloc::vec::Vec;
use core::fmt::Write;
use ed25519_dalek::{Signer, SigningKey};
use mind::util::FixedBuf;

const ED25519: [u8; 7] = [0x30, 0x05, 0x06, 0x03, 0x2B, 0x65, 0x70]; // AlgorithmIdentifier { id-Ed25519 }

fn tlv(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 4);
    out.push(tag);
    match body.len() {
        n if n < 0x80 => out.push(n as u8),
        n if n < 0x100 => out.extend_from_slice(&[0x81, n as u8]),
        n => out.extend_from_slice(&[0x82, (n >> 8) as u8, n as u8]),
    }
    out.extend_from_slice(body);
    out
}

fn sequence(parts: &[&[u8]]) -> Vec<u8> { tlv(0x30, &parts.concat()) }

// Name ::= SEQUENCE { SET { SEQUENCE { id-at-commonName, UTF8String } } }
fn name(common: &str) -> Vec<u8> {
    let attribute = sequence(&[&[0x06, 0x03, 0x55, 0x04, 0x03], &tlv(0x0C, common.as_bytes())]);
    sequence(&[&tlv(0x31, &attribute)])
}

// UTCTime YYMMDDHHMMSSZ of a Unix time in 2000..2049.
fn utc_time(unix: u64) -> Vec<u8> {
    let (year, month, day) = mind::rtc::civil_from_days((unix / 86400).saturating_sub(10957) as u32);
    let s = unix % 86400;
    let digits = [year % 100, month, day, (s / 3600) as u32, (s / 60 % 60) as u32, (s % 60) as u32];
    let mut text = Vec::with_capacity(13);
    for d in digits { text.push(b'0' + (d / 10) as u8); text.push(b'0' + (d % 10) as u8); }
    text.push(b'Z');
    tlv(0x17, &text)
}

/// The common name of the device certificate: "MIND " and the first four bytes of the public key in hex.
pub fn common_name(key: &SigningKey) -> FixedBuf<32> {
    let mut out = FixedBuf::new();
    let _ = out.write_str("MIND ");
    for byte in &key.verifying_key().as_bytes()[..4] { let _ = write!(out, "{:02X}", byte); }
    out
}

/// The public key as an OpenSSH `authorized_keys` line starts (351-NET-0005): `ssh-ed25519 <base64 of the key blob>`.
pub fn openssh(key: &SigningKey) -> FixedBuf<96> {
    let mut blob = [0u8; 51];
    blob[..4].copy_from_slice(&11u32.to_be_bytes());
    blob[4..15].copy_from_slice(b"ssh-ed25519");
    blob[15..19].copy_from_slice(&32u32.to_be_bytes());
    blob[19..].copy_from_slice(key.verifying_key().as_bytes());
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = FixedBuf::new();
    let _ = out.write_str("ssh-ed25519 ");
    for chunk in blob.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (k, &b)| n | (b as u32) << (16 - 8 * k));
        for k in 0..4 {
            let c = if k <= chunk.len() { ALPHABET[(n >> (18 - 6 * k) & 63) as usize] } else { b'=' };
            let _ = out.write_char(c as char);
        }
    }
    out
}

/// The certificate, valid from `not_before` (Unix time; clamped to 2000..2049) with no end (99991231235959Z).
pub fn build(key: &SigningKey, serial: [u8; 16], not_before: u64) -> Vec<u8> {
    let mut serial = serial; serial[0] = serial[0] & 0x7F | 0x40; // positive, no leading zero
    let not_before = not_before.clamp(946_684_800, 2_524_607_999);
    let common = common_name(key);
    let subject = name(core::str::from_utf8(common.as_bytes()).unwrap_or("MIND"));
    let validity = sequence(&[&utc_time(not_before), &tlv(0x18, b"99991231235959Z")]);
    let mut bits = [0u8; 33]; bits[1..].copy_from_slice(key.verifying_key().as_bytes());
    let public = sequence(&[&ED25519, &tlv(0x03, &bits)]);
    let tbs = sequence(&[&[0xA0, 0x03, 0x02, 0x01, 0x02], &tlv(0x02, &serial), &ED25519, &subject, &validity, &subject, &public]);
    let mut signature = [0u8; 65]; signature[1..].copy_from_slice(&key.sign(&tbs).to_bytes());
    sequence(&[&tbs, &ED25519, &tlv(0x03, &signature)])
}
