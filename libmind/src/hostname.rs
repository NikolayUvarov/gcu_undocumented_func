//! The machine's name (000-KRN-0180, with 000-APP-0123's rule): one RFC 1123 label in `data/machine/name`, a directory
//! of its own so that a reader's read-only scope of it reaches nothing else; `mindcore` when none is set. No system
//! calls: tests/hostname_host.rs.

#[cfg(feature = "alloc")]
use alloc::string::String;

/// The directory, and the file in it.
pub const DIR: &str = "data/machine";
pub const FILE: &str = "data/machine/name";
/// The file's name within `DIR`, for a reader whose scope's root is that directory.
pub const NAME: &str = "name";
/// The name when none is set, or the file holds no label.
pub const DEFAULT: &str = "mindcore";
/// A label's longest (RFC 1123, DHCP's option 12 and DNS).
pub const MAX: usize = 63;

#[cfg(feature = "alloc")]
/// `text` as the machine's name: an RFC 1123 label, its capitals taken as small letters; else why not.
pub fn label(text: &str) -> Result<String, &'static str> {
    if text.is_empty() { return Err("NO NAME"); }
    if text.len() > MAX { return Err("LONGER THAN 63 CHARACTERS"); }
    if !text.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') { return Err("A CHARACTER OTHER THAN a-z 0-9 -"); }
    if text.starts_with('-') || text.ends_with('-') { return Err("A HYPHEN FIRST OR LAST"); }
    Ok(text.to_ascii_lowercase())
}

/// Where the name in use came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source { File, Missing, Invalid(&'static str), Unread(&'static str) }

#[cfg(feature = "alloc")]
/// The name in use from the file's bytes (None: no file): its one label, a line end after it taken off, in small
/// letters as the shell writes it; else `mindcore` and why.
pub fn read(content: Option<&[u8]>) -> (String, Source) {
    let Some(bytes) = content else { return (String::from(DEFAULT), Source::Missing) };
    let Ok(text) = core::str::from_utf8(bytes) else { return (String::from(DEFAULT), Source::Invalid("NOT UTF-8")) };
    let line = text.strip_suffix('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).unwrap_or(text);
    match label(line) {
        Ok(name) if name == line => (name, Source::File),
        Ok(_) => (String::from(DEFAULT), Source::Invalid("CAPITAL LETTERS")),
        Err(why) => (String::from(DEFAULT), Source::Invalid(why)),
    }
}
