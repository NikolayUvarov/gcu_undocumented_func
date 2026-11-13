//! The paths the server answers: scripts/release.py's layout, each name checked against its pattern (110).

use std::fs::{self, File};
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// What a path names; a blob never changes, a channel and the key file are fetched fresh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Blob, Manifest, Channel }

/// Why a target is not served: the status and the reason the response and the log give.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NotAbsolute,
    Query,
    Byte,
    EmptyName,
    DotName,
    NotInLayout,
    Link,
    NotAFile,
    Missing,
}

impl Refusal {
    pub fn status(self) -> u16 {
        match self { Refusal::NotAbsolute | Refusal::Query | Refusal::Byte => 400, _ => 404 }
    }
    pub fn text(self) -> &'static str {
        match self {
            Refusal::NotAbsolute => "the target is not an absolute path",
            Refusal::Query => "a query string is not served",
            Refusal::Byte => "a byte outside the names' alphabet (A-Z a-z 0-9 . _ - /)",
            Refusal::EmptyName => "an empty name in the path",
            Refusal::DotName => "a name starting with a dot",
            Refusal::NotInLayout => "not a file of the release layout",
            Refusal::Link => "a symbolic link",
            Refusal::NotAFile => "not a regular file",
            Refusal::Missing => "no such file",
        }
    }
}

const ARCHES: [&str; 2] = ["x86_64", "aarch64"];
const MANIFESTS: [&str; 2] = ["MANIFEST", "MANIFEST.SIG"];

fn digits(s: &str) -> bool { !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit()) }
fn sha256_hex(s: &str) -> bool { s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) }
fn channel_name(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty() && b.len() <= 64 && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-'))
}

/// The relative path and kind a request target names, or why it is refused; nothing is opened here.
pub fn resolve(target: &str) -> Result<(String, Kind), Refusal> {
    if target.contains('?') { return Err(Refusal::Query); }
    if !target.starts_with('/') { return Err(Refusal::NotAbsolute); }
    if !target.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/')) { return Err(Refusal::Byte); }
    let names: Vec<&str> = target[1..].split('/').collect();
    if names.iter().any(|n| n.is_empty()) { return Err(Refusal::EmptyName); }
    if names.iter().any(|n| n.starts_with('.')) { return Err(Refusal::DotName); }
    let kind = match names.as_slice() {
        ["blobs", digest] if sha256_hex(digest) => Kind::Blob,
        ["releases", version, arch, file] if digits(version) && ARCHES.contains(arch) && MANIFESTS.contains(file) => Kind::Manifest,
        ["channels", name] if channel_name(name) => Kind::Channel,
        ["keys", "current"] => Kind::Channel, // the key file (351-UPD-0026), fetched fresh as a channel is
        ["keys", version] if digits(version) => Kind::Manifest, // its history (351-UPD-0027): a version never changes
        _ => return Err(Refusal::NotInLayout),
    };
    Ok((names.join("/"), kind))
}

/// Opens `relative` under `root` (canonical): no link on the way, a regular file, the one checked (device and inode).
pub fn open(root: &Path, relative: &str) -> Result<(File, fs::Metadata), Refusal> {
    let mut at = root.to_path_buf();
    for name in relative.split('/') {
        at.push(name);
        match fs::symlink_metadata(&at) {
            Ok(m) if m.file_type().is_symlink() => return Err(Refusal::Link),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(Refusal::Missing),
            Err(_) => return Err(Refusal::Missing),
        }
    }
    let checked = fs::symlink_metadata(&at).map_err(|_| Refusal::Missing)?;
    if !checked.file_type().is_file() { return Err(Refusal::NotAFile); }
    let file = File::open(&at).map_err(|_| Refusal::Missing)?;
    let opened = file.metadata().map_err(|_| Refusal::Missing)?;
    // A link put in its place between the check and the open is not followed out of the root.
    if opened.dev() != checked.dev() || opened.ino() != checked.ino() || !opened.is_file() { return Err(Refusal::Link); }
    Ok((file, opened))
}

#[cfg(test)]
mod tests {
    use super::*;
    const BLOB: &str = "/blobs/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn the_layout_and_each_refusal() {
        assert_eq!(resolve(BLOB).unwrap().1, Kind::Blob);
        assert_eq!(resolve("/releases/6/x86_64/MANIFEST").unwrap(), ("releases/6/x86_64/MANIFEST".into(), Kind::Manifest));
        assert_eq!(resolve("/releases/12/aarch64/MANIFEST.SIG").unwrap().1, Kind::Manifest);
        assert_eq!(resolve("/channels/stable").unwrap().1, Kind::Channel);
        assert_eq!(resolve("/keys/current").unwrap(), ("keys/current".to_string(), Kind::Channel));
        assert_eq!(resolve("/keys/12").unwrap(), ("keys/12".to_string(), Kind::Manifest));
        assert_eq!(resolve("/keys/other"), Err(Refusal::NotInLayout));
        let refused = [
            ("/channels/stable?x=1", Refusal::Query), ("channels/stable", Refusal::NotAbsolute), ("/channels/st%61ble", Refusal::Byte),
            ("/channels/a b", Refusal::Byte), ("/channels\\stable", Refusal::Byte), ("/channels//stable", Refusal::EmptyName),
            ("/channels/stable/", Refusal::EmptyName), ("/", Refusal::EmptyName), ("/channels/.hidden", Refusal::DotName),
            ("/blobs/../channels/stable", Refusal::DotName), ("/blobs", Refusal::NotInLayout), ("/blobs/ABCDEF", Refusal::NotInLayout),
            (&BLOB.to_uppercase().replace("/BLOBS/", "/blobs/"), Refusal::NotInLayout), ("/releases/6/riscv64/MANIFEST", Refusal::NotInLayout),
            ("/releases/v6/x86_64/MANIFEST", Refusal::NotInLayout), ("/releases/6/x86_64/kernel.elf", Refusal::NotInLayout),
            ("/channels/Stable", Refusal::NotInLayout), ("/index.html", Refusal::NotInLayout), ("/channels/-x", Refusal::NotInLayout),
        ];
        for (target, why) in refused { assert_eq!(resolve(target), Err(why), "{target}"); }
    }
}
