//! The machine's name (000-APP-0123, for 750-EFF-0018 and 000-KRN-0180): one RFC 1123 label in `data/machine/name`, a
//! directory of its own so that a reader's read-only scope reaches nothing else; `mindcore` when none is set. `hostname`
//! says it, `hostname <name>` sets it. No system calls: tests/shell_host.rs.

use alloc::format;
use alloc::string::String;

pub const DIR: &str = "data/machine";
pub const FILE: &str = "data/machine/name";
/// The name when none is set, or the file holds no label.
pub const DEFAULT: &str = "mindcore";
/// A label's longest (RFC 1123, DHCP's option 12 and DNS).
pub const MAX: usize = 63;
/// The rule, as the refusals say it.
pub const RULE: &str = "A NAME IS 1 TO 63 OF a-z 0-9 -, NEITHER FIRST NOR LAST A HYPHEN";

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

/// The file's text for `name`, a label already.
pub fn text(name: &str) -> String { format!("{}\n", name) }

/// `hostname`'s line: the name, and where it came from.
pub fn shown(name: &str, source: Source) -> String {
    match source {
        Source::File => format!("THE MACHINE'S NAME: {} ({})", name, FILE),
        Source::Missing => format!("THE MACHINE'S NAME: {} (NONE SET: NO {}; HOSTNAME <NAME> SETS ONE)", name, FILE),
        Source::Invalid(why) => format!("THE MACHINE'S NAME: {} ({} HOLDS NO NAME: {}; HOSTNAME <NAME> SETS ONE)", name, FILE, why),
        Source::Unread(why) => format!("THE MACHINE'S NAME: {} ({} NOT READ: {})", name, FILE, why),
    }
}

/// The system log's line for a change.
pub fn changed(old: &str, new: &str) -> String { format!("[SHELL] THE MACHINE'S NAME: {} -> {}", old, new) }
