//! MIND Hub's protocol, `hub-api 1` (100003-APP-0145; docs/hub/README.md, sections 4 to 6 and 10). One crate for every
//! side, so that MIND Core's `improve`, the native hub and `tools/mindhub` agree byte for byte:
//! - `form`: a registration, a request, an attachment's entry, a message, a state change, a board entry, and their bounds;
//! - `state`: which role moves a request from which state to which;
//! - `trust`: the bands of trust from −1023 to +1023, what each allows, and the hub's own changes within −256 … +512;
//! - `bucket`: a token bucket on a clock the caller gives;
//! - `repeat`: a request's normalised text, for refusing a repeat;
//! - `sign`: what a signature covers, over a challenge the hub gave, so that no clock is trusted;
//! - `work`: the registration's proof of work;
//! - `json`: a strict reader and a writer, and `wire` the forms in it.
//!
//! On MIND Core a program does not parse a network's JSON itself (MC-11.11): `parse` gives it the tree, and it builds the
//! forms through the same constructors and checks.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod bucket;
pub mod form;
pub mod json;
pub mod repeat;
pub mod sign;
pub mod state;
pub mod trust;
pub mod wire;
pub mod work;

/// The API's name and version, sent with every request and answer.
pub const API: &str = "hub-api 1";

/// A day on the hub's clock, in milliseconds.
pub const DAY_MS: u64 = 86_400_000;

/// 32 bytes as 64 lower-case hex digits.
pub fn hex32(bytes: &[u8; 32]) -> alloc::string::String { hex(bytes) }

/// Bytes as lower-case hex digits.
pub fn hex(bytes: &[u8]) -> alloc::string::String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = alloc::string::String::with_capacity(bytes.len() * 2);
    for b in bytes { out.push(DIGITS[(b >> 4) as usize] as char); out.push(DIGITS[(b & 15) as usize] as char); }
    out
}

/// 64 lower-case hex digits as 32 bytes; None for anything else, upper case included (one encoding only).
pub fn parse_hex32(text: &str) -> Option<[u8; 32]> {
    let mut out = [0u8; 32];
    parse_hex(text, &mut out).then_some(out)
}

/// Exactly `out.len() * 2` lower-case hex digits into `out`.
pub fn parse_hex(text: &str, out: &mut [u8]) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != out.len() * 2 { return false; }
    let digit = |c: u8| match c { b'0'..=b'9' => Some(c - b'0'), b'a'..=b'f' => Some(c - b'a' + 10), _ => None };
    for (i, pair) in bytes.chunks_exact(2).enumerate() {
        let (Some(h), Some(l)) = (digit(pair[0]), digit(pair[1])) else { return false };
        out[i] = h << 4 | l;
    }
    true
}
