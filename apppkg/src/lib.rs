//! Signed application packages (750-UPD-0019, contract draft 0.1): one application for one architecture. The signed
//! bytes are MANIFEST's exactly; MANIFEST.SIG is an Ed25519 signature over `PREFIX` and them, so an application key's
//! signature never passes as a boot manifest's or a channel's. `parse` demands the one encoding the draft states, within
//! its bounds: anything else is refused, not repaired. A manifest describes; it grants nothing (MC-3.11).
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

/// The first line of a manifest of format 1.
pub const FORMAT_LINE: &str = "MIND-APP-MANIFEST 1";
/// What the signature covers before MANIFEST's bytes.
pub const PREFIX: &[u8] = b"MIND application package 1\0";
pub const MANIFEST_MAX: usize = 64 * 1024;
pub const FILES_MAX: usize = 256;
pub const NAME_MAX: usize = 32;
pub const PATH_MAX: usize = 128;
// The header lines before the first `file` line.
const HEADERS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arch { X86_64, Aarch64 }

impl Arch {
    pub fn name(self) -> &'static str { match self { Arch::X86_64 => "x86_64", Arch::Aarch64 => "aarch64" } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Larger than MANIFEST_MAX, or more than FILES_MAX files.
    TooLarge,
    /// Not the one encoding: this line (from 1) is unknown, out of order, out of range or malformed.
    Malformed(usize),
    /// No key the owner trusts for this name has the identity the manifest names.
    Untrusted,
    /// The signature does not verify under that key.
    Signature,
    /// Built for another architecture than the machine's.
    Arch,
    /// It needs a newer release than the one running.
    System,
    /// Lower than the version installed, or the same version with other bytes.
    Older,
}

/// One file of the package: its path under the application's directory, size, SHA-256, the requests its program
/// makes (as the boot manifest's) and the memory it asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct File<'a> { pub path: &'a str, pub size: u64, pub sha256: [u8; 32], pub requests: u32, pub memory: u64 }

impl File<'_> {
    /// Whether `data` is this file: its size and SHA-256.
    pub fn matches(&self, data: &[u8]) -> bool { data.len() as u64 == self.size && Sha256::digest(data)[..] == self.sha256[..] }
}

/// A parsed manifest. Its files come from `files`, in path order.
#[derive(Clone, Copy, Debug)]
pub struct Manifest<'a> { pub key: [u8; 8], pub name: &'a str, pub version: u64, pub arch: Arch, pub system: u64, files: &'a str, count: usize }

impl<'a> Manifest<'a> {
    pub fn files(&self) -> impl Iterator<Item = File<'a>> + 'a { self.files.lines().map(|line| file_line(line).expect("checked by parse")) }
    pub fn count(&self) -> usize { self.count }
}

/// A key the owner's policy trusts (`application-key <public> <name or *>`): for one application's name, or any.
#[derive(Clone, Copy, Debug)]
pub struct Trusted<'a> { pub key: [u8; 32], pub name: &'a str }

/// The identity a manifest names its key by: the first 8 bytes of the key's SHA-256 (16 hex digits).
pub fn key_id(public: &[u8; 32]) -> [u8; 8] { Sha256::digest(public)[..8].try_into().expect("8 of 32 bytes") }

/// The application installed already, if any: its version and the SHA-256 of its MANIFEST.
#[derive(Clone, Copy, Debug)]
pub struct Installed { pub version: u64, pub manifest: [u8; 32] }

/// Checks a package for a machine of `arch` running release `system`: MANIFEST's encoding, the signature in
/// MANIFEST.SIG under a key `trusted` holds for its name, the architecture, the release, and the version against the
/// one installed (lower only if `downgrade`, the owner's `application-downgrade <name>`). The files are the caller's to
/// check against `files`, each with `File::matches`.
pub fn verify<'a>(manifest: &'a [u8], signature: &[u8; 64], trusted: &[Trusted], arch: Arch, system: u64, installed: Option<Installed>,
                  downgrade: bool) -> Result<Manifest<'a>, Error> {
    let parsed = parse(manifest)?;
    let key = trusted.iter().find(|t| (t.name == "*" || t.name == parsed.name) && key_id(&t.key) == parsed.key).ok_or(Error::Untrusted)?;
    let key = VerifyingKey::from_bytes(&key.key).map_err(|_| Error::Untrusted)?;
    let mut signed = alloc::vec::Vec::with_capacity(PREFIX.len() + manifest.len());
    signed.extend_from_slice(PREFIX);
    signed.extend_from_slice(manifest);
    key.verify_strict(&signed, &Signature::from_bytes(signature)).map_err(|_| Error::Signature)?;
    if parsed.arch != arch { return Err(Error::Arch); }
    if parsed.system > system { return Err(Error::System); }
    if let Some(installed) = installed {
        let same = Sha256::digest(manifest)[..] == installed.manifest[..];
        if (parsed.version < installed.version && !downgrade) || (parsed.version == installed.version && !same) { return Err(Error::Older); }
    }
    Ok(parsed)
}

/// Parses MANIFEST's bytes, demanding format 1's one encoding.
pub fn parse(bytes: &[u8]) -> Result<Manifest<'_>, Error> {
    if bytes.len() > MANIFEST_MAX { return Err(Error::TooLarge); }
    let text = core::str::from_utf8(bytes).map_err(|_| Error::Malformed(1))?;
    if !text.ends_with('\n') || text.bytes().any(|b| b != b'\n' && !(0x20..0x7F).contains(&b)) { return Err(Error::Malformed(text.lines().count().max(1))); }
    let mut lines = text.split_terminator('\n');
    let mut header = |n: usize, word: &str| -> Result<&str, Error> {
        let line = lines.next().ok_or(Error::Malformed(n))?;
        line.strip_prefix(word).and_then(|rest| rest.strip_prefix(' ')).ok_or(Error::Malformed(n))
    };
    if header(1, "MIND-APP-MANIFEST")? != "1" { return Err(Error::Malformed(1)); }
    let key = hex::<8>(header(2, "key")?).ok_or(Error::Malformed(2))?;
    let name = header(3, "name")?;
    if !name_ok(name) { return Err(Error::Malformed(3)); }
    let version = decimal(header(4, "version")?).filter(|&v| v > 0).ok_or(Error::Malformed(4))?;
    let arch = match header(5, "arch")? { "x86_64" => Arch::X86_64, "aarch64" => Arch::Aarch64, _ => return Err(Error::Malformed(5)) };
    let system = decimal(header(6, "system")?).ok_or(Error::Malformed(6))?;
    let start = text.split_terminator('\n').take(HEADERS).map(|l| l.len() + 1).sum::<usize>();
    let files = &text[start..];
    let mut count = 0;
    let mut previous: Option<&str> = None;
    for (i, line) in files.split_terminator('\n').enumerate() {
        let n = HEADERS + 1 + i;
        let file = file_line(line).ok_or(Error::Malformed(n))?;
        if previous.is_some_and(|p| p >= file.path) { return Err(Error::Malformed(n)); }
        previous = Some(file.path);
        count += 1;
        if count > FILES_MAX { return Err(Error::TooLarge); }
    }
    if count == 0 { return Err(Error::Malformed(HEADERS + 1)); }
    Ok(Manifest { key, name, version, arch, system, files, count })
}

// 1 to NAME_MAX of a-z, 0-9 and '-', not starting with '-'.
fn name_ok(name: &str) -> bool {
    (1..=NAME_MAX).contains(&name.len()) && !name.starts_with('-') && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

// A relative path of a-z, 0-9, '.', '_', '-' and '/', with no empty, `.` or `..` part (so never MANIFEST itself).
fn path_ok(path: &str) -> bool {
    (1..=PATH_MAX).contains(&path.len())
        && path.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-' | b'/'))
        && path.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

// `file <path> <size> <sha256> <requests> <memory>`.
fn file_line(line: &str) -> Option<File<'_>> {
    let mut words = line.split(' ');
    if words.next()? != "file" { return None; }
    let path = words.next().filter(|p| path_ok(p))?;
    let size = decimal(words.next()?)?;
    let sha256 = hex::<32>(words.next()?)?;
    let requests = u32::from_be_bytes(hex::<4>(words.next()?)?);
    let memory = decimal(words.next()?)?;
    words.next().is_none().then_some(File { path, size, sha256, requests, memory })
}

// A decimal of one encoding: "0", or no leading zero; it must fit u64.
fn decimal(text: &str) -> Option<u64> {
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|b| b.is_ascii_digit()) || (text.len() > 1 && text.starts_with('0')) { return None; }
    text.parse().ok()
}

// Exactly 2N lower-case hex digits.
fn hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let digits = text.as_bytes();
    if digits.len() != 2 * N { return None; }
    let value = |d: u8| match d { b'0'..=b'9' => Some(d - b'0'), b'a'..=b'f' => Some(d - b'a' + 10), _ => None };
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() { *byte = value(digits[2 * i])? << 4 | value(digits[2 * i + 1])?; }
    Some(out)
}
