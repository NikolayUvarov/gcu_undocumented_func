//! The key as `recovery` shows it and takes it back (308-APP-0122), and its file for a machine without a screen:
//! `recovery new|open --key-file <path>`. The file's first line names its form, the second holds the key as its owner
//! writes it down (`mind::blockstore::recovery_text`: 8 groups of 4, Crockford's base32 with a check).
use crate::blockstore::{recovery_parse, recovery_text, Typo};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The file's first line: its form, refused with this name when another is there (MC-12.4).
pub const HEADER: &str = "MIND CORE RECOVERY KEY 1";

/// The words after `recovery` without a screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command { New(String), Open(String) }

pub const USAGE: &str = "usage: recovery | recovery new --key-file <file> | recovery open --key-file <file>";

/// No words: the screen; `new` or `open` with `--key-file`: without it.
pub fn command(args: &str) -> Result<Option<Command>, &'static str> {
    match args.split_whitespace().collect::<Vec<_>>().as_slice() {
        [] => Ok(None),
        ["new", "--key-file", path] => Ok(Some(Command::New(String::from(*path)))),
        ["open", "--key-file", path] => Ok(Some(Command::Open(String::from(*path)))),
        _ => Err(USAGE),
    }
}

/// Why a typed key is refused before the store is asked.
pub fn typo(t: Typo) -> &'static str {
    match t {
        Typo::Character => "a character that no recovery key has (they are 0-9 and A-Z but I, L, O and U)",
        Typo::Length => "not 32 characters, the dashes and spaces aside",
        Typo::Check => "its last group does not match the rest: a character is mistyped",
    }
}

/// A typed key in the form `recovery_text` writes, once its check holds: case, dashes and spaces aside, I and L read
/// as 1 and O as 0.
pub fn canonical(typed: &str) -> Result<String, Typo> {
    let key = recovery_parse(typed.as_bytes())?;
    Ok(recovery_text(&key).iter().map(|&b| b as char).collect())
}

/// The file's text for `key`.
pub fn render(key: &str) -> String { format!("{}\n{}\n", HEADER, key) }

/// The key a file holds, in its written form, or why it holds none.
pub fn parse(text: &str) -> Result<String, String> {
    let mut lines = text.lines().map(str::trim);
    match lines.next() {
        Some(HEADER) => {}
        Some(other) if other.starts_with("MIND CORE RECOVERY KEY ") => return Err(format!("the file is of another form ({}): this recovery reads {}", other, HEADER)),
        _ => return Err(format!("not a recovery key file: its first line is not {}", HEADER)),
    }
    let key = lines.next().filter(|l| !l.is_empty()).ok_or_else(|| String::from("no key on the file's second line"))?;
    canonical(key).map_err(|t| format!("the file's key: {}", typo(t)))
}

/// A secret's bytes overwritten before it is dropped.
pub fn wipe(secret: &mut String) { wipe_bytes(&mut core::mem::take(secret).into_bytes()); }

/// The same for bytes.
pub fn wipe_bytes(bytes: &mut [u8]) {
    bytes.fill(0);
    core::hint::black_box(&bytes); // the zeros are written, though nothing reads them
}
