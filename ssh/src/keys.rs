//! The owner's list of login keys for the SSH server (112-NET-0072): OpenSSH's `authorized_keys` lines with what each
//! key may do in front, `console`, `files` or `console,files`, then `ssh-ed25519`, the key blob in base64 and a comment.
//! A line that names no right, another key type, a damaged key or a word it does not know is left out, with why. No
//! OpenSSH option is taken (no `command=`, `from=`, `restrict`): a right is the whole of what a key gets.
use crate::server::Rights;
use crate::wire::Reader;
use alloc::string::String;
use alloc::vec::Vec;

/// Lines taken from one file.
pub const KEYS_MAX: usize = 64;
/// The longest line read; a longer one is left out.
pub const LINE_MAX: usize = 1024;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// A key the owner listed, with its rights and the line's comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed { pub key: [u8; 32], pub rights: Rights, pub comment: String }

/// Base64 (RFC 4648) with padding; None on any other byte or a bad length.
pub fn unbase64(text: &[u8]) -> Option<Vec<u8>> {
    if text.len() % 4 != 0 { return None; }
    let body = text.strip_suffix(b"==").or_else(|| text.strip_suffix(b"=")).unwrap_or(text);
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut n, mut bits) = (0u32, 0);
    for &c in body {
        n = n << 6 | ALPHABET.iter().position(|&b| b == c)? as u32;
        bits += 6;
        if bits >= 8 { bits -= 8; out.push((n >> bits) as u8); }
    }
    // The bits past the last byte must be zero, as a writer of the canonical form leaves them.
    (n & ((1 << bits) - 1) == 0).then_some(out)
}

/// Base64 without padding, as OpenSSH prints a fingerprint.
pub fn base64_unpadded(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (k, &b)| n | (b as u32) << (16 - 8 * k));
        for k in 0..=chunk.len() { out.push(ALPHABET[(n >> (18 - 6 * k) & 63) as usize] as char); }
    }
    out
}

/// OpenSSH's fingerprint of a key blob's SHA-256: `SHA256:` and its base64 without padding.
pub fn fingerprint(sha256: &[u8; 32]) -> String {
    let mut out = String::from("SHA256:");
    out.push_str(&base64_unpadded(sha256));
    out
}

fn rights(word: &str) -> Option<Rights> {
    let mut rights = Rights { console: false, files: false };
    for part in word.split(',') {
        match part { "console" => rights.console = true, "files" => rights.files = true, _ => return None }
    }
    Some(rights)
}

/// One line: None for a blank line or a comment; else the key, or why it is left out.
pub fn line(text: &str) -> Option<Result<Listed, &'static str>> {
    let text = text.trim();
    if text.is_empty() || text.starts_with('#') { return None; }
    if text.len() > LINE_MAX { return Some(Err("a line longer than 1024 bytes")); }
    let mut words = text.split_ascii_whitespace();
    let first = words.next()?;
    if first.starts_with("ssh-") { return Some(Err("no rights before the key: console, files or console,files")); }
    let Some(rights) = rights(first) else { return Some(Err("a word other than console, files or console,files before the key")) };
    match words.next() {
        Some("ssh-ed25519") => {}
        Some(_) => return Some(Err("a key other than ssh-ed25519")),
        None => return Some(Err("no key")),
    }
    let Some(blob) = words.next().and_then(|b| unbase64(b.as_bytes())) else { return Some(Err("a key not in base64")) };
    let mut r = Reader::new(&blob);
    let key = (r.string().ok() == Some(&b"ssh-ed25519"[..])).then(|| r.string().ok().and_then(|k| <[u8; 32]>::try_from(k).ok())).flatten();
    let Some(key) = key.filter(|_| r.rest().is_empty()) else { return Some(Err("a key blob that is not an ssh-ed25519 key")) };
    let comment = words.collect::<Vec<_>>().join(" ");
    Some(Ok(Listed { key, rights, comment }))
}

/// The file's keys, and the lines left out (their numbers from 1, and why). At most KEYS_MAX keys; the rest are left out.
pub fn parse(text: &str) -> (Vec<Listed>, Vec<(usize, &'static str)>) {
    let (mut keys, mut left) = (Vec::new(), Vec::new());
    for (number, text) in text.lines().enumerate() {
        match line(text) {
            None => {}
            Some(Ok(_)) if keys.len() >= KEYS_MAX => left.push((number + 1, "more than 64 keys")),
            Some(Ok(listed)) => keys.push(listed),
            Some(Err(why)) => left.push((number + 1, why)),
        }
    }
    (keys, left)
}

/// The directory below the files' root that holds this list (`data/ssh/` on a MIND machine).
pub const DIR: &str = "ssh";

/// Whether a path below the files' root (`server::normal`'s form) is the keys' directory or below it, as a FAT volume
/// matches names: in any case, with trailing dots and spaces dropped. The SFTP subsystem refuses such a path, so a key
/// with the files' right cannot list another key (MC-3.4).
pub fn under_dir(path: &str) -> bool {
    let first = path.trim_start_matches('/').split('/').next().unwrap_or("");
    first.trim_end_matches(['.', ' ']).eq_ignore_ascii_case(DIR)
}
