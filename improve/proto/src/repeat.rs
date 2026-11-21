//! A request's normalised text, for refusing the same request twice from one user within 30 days (docs/hub/README.md,
//! 5.4): letters in lower case, every run of white space one space, none at the ends; then SHA-256 of the title, a line
//! end and the text.

use alloc::string::String;
use sha2::{Digest, Sha256};

/// How long a repeat is refused, in days.
pub const DAYS: u64 = 30;

/// `text` lower-cased, its white space runs made one space, trimmed.
pub fn normalise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars() {
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if space { out.push(' '); space = false; }
        for lower in c.to_lowercase() { out.push(lower); }
    }
    out
}

/// The digest two requests share when one repeats the other.
pub fn digest(title: &str, text: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(normalise(title).as_bytes());
    hash.update(b"\n");
    hash.update(normalise(text).as_bytes());
    hash.finalize().into()
}
