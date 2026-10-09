// Signed boot volumes (350-UPD-0003, docs/update/README.md; MC-9.1, 9.2): MANIFEST's signature is checked against
// the public key built in (build.rs), then every image against the manifest's size and SHA-256 before it is used.
// The manifest describes images and what they ask for; it grants nothing (MC-3.11).
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

include!(concat!(env!("OUT_DIR"), "/boot_key.rs"));

pub struct Manifest<'a> { text: &'a [u8] }

// The `index`th space-separated field of a line.
fn field(line: &[u8], index: usize) -> Option<&[u8]> { line.split(|&b| b == b' ').nth(index) }

fn hex(text: &[u8]) -> Option<[u8; 32]> {
    if text.len() != 64 { return None; }
    let mut out = [0u8; 32];
    for (k, pair) in text.chunks(2).enumerate() {
        let digit = |c: u8| (c as char).to_digit(16);
        out[k] = (digit(pair[0])? * 16 + digit(pair[1])?) as u8;
    }
    Some(out)
}

impl<'a> Manifest<'a> {
    /// The manifest, if `signature` is the built-in key's over exactly `text` and the text is of format 1.
    pub fn verified(text: &'a [u8], signature: &[u8]) -> Result<Self, &'static str> {
        let key = VerifyingKey::from_bytes(&KEY).map_err(|_| "the built-in key is not a key")?;
        let signature: [u8; 64] = signature.try_into().map_err(|_| "a signature is 64 bytes")?;
        key.verify_strict(text, &Signature::from_bytes(&signature)).map_err(|_| "bad signature")?;
        if !text.starts_with(b"MIND-MANIFEST 1\n") { return Err("another manifest format"); }
        Ok(Manifest { text })
    }

    fn lines(&self) -> impl Iterator<Item = &'a [u8]> { self.text.split(|&b| b == b'\n').filter(|l| !l.is_empty()) }

    /// Whether `data` is the file `name` as the manifest lists it.
    pub fn check(&self, name: &str, data: &[u8]) -> Result<(), &'static str> {
        let line = self.lines().find(|l| field(l, 0) == Some(b"file") && field(l, 1) == Some(name.as_bytes())).ok_or("not in the manifest")?;
        let size = core::str::from_utf8(field(line, 2).unwrap_or(b"")).ok().and_then(|s| s.parse::<usize>().ok());
        let digest = field(line, 3).and_then(hex);
        if size != Some(data.len()) || digest.as_ref() != Some(Sha256::digest(data).as_ref()) { return Err("not as the manifest says"); }
        Ok(())
    }

    /// Whether the manifest lists a file `name` (one it does not list may be absent: a service not built).
    pub fn names(&self, name: &str) -> bool { self.lines().any(|l| field(l, 0) == Some(b"file") && field(l, 1) == Some(name.as_bytes())) }

    /// The signing key's identity as the manifest names it.
    pub fn key(&self) -> &'a str {
        self.lines().find(|l| field(l, 0) == Some(b"key")).and_then(|l| field(l, 1)).and_then(|k| core::str::from_utf8(k).ok()).unwrap_or("?")
    }

    /// The manifest's own SHA-256, for the launch record.
    pub fn digest(&self) -> [u8; 32] { Sha256::digest(self.text).into() }
}
