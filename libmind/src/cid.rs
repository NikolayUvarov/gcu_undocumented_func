//! Content identifiers (issue 300-STO-0001; MC-4.2, 4.13): the CIDv1 of multiformats — the format's version, the
//! content's type and a self-describing hash — in a binary form (36 bytes) and a text form (`b` and base32). Only the
//! formats below are supported; any other version, type, algorithm or encoding is refused, never read as one of them.
use crate::sha256;

/// The identifier format's version (CIDv1).
pub const VERSION: u64 = 1;
/// Bytes of the binary form of every supported identifier: version, type, algorithm, digest length, digest.
pub const BYTES: usize = 4 + sha256::DIGEST;
/// Characters of the text form: the multibase prefix `b`, then lowercase base32 without padding.
pub const TEXT: usize = 1 + (BYTES * 8 + 4) / 5;
const BASE32: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// What the hashed bytes are (a multicodec code).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Codec {
    /// Opaque bytes: the exact byte sequence is identified, nothing is assumed about its meaning (`raw`, 0x55).
    Raw,
}

impl Codec {
    pub const fn code(self) -> u64 { match self { Codec::Raw => 0x55 } }
    pub const fn from_code(code: u64) -> Option<Self> { match code { 0x55 => Some(Codec::Raw), _ => None } }
}

/// The hash algorithm (a multihash code) and its digest length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Algorithm {
    /// SHA-256, 32 bytes (`sha2-256`, 0x12).
    Sha2_256,
}

impl Algorithm {
    pub const fn code(self) -> u64 { match self { Algorithm::Sha2_256 => 0x12 } }
    pub const fn from_code(code: u64) -> Option<Self> { match code { 0x12 => Some(Algorithm::Sha2_256), _ => None } }
    pub const fn digest_len(self) -> usize { match self { Algorithm::Sha2_256 => sha256::DIGEST } }
    pub fn digest(self, data: &[u8]) -> [u8; sha256::DIGEST] { match self { Algorithm::Sha2_256 => sha256::digest(data) } }
}

// Every code fits one varint byte, so the binary form is BYTES long.
const _: () = assert!(VERSION < 0x80 && Codec::Raw.code() < 0x80 && Algorithm::Sha2_256.code() < 0x80);

/// Why bytes or text are not a supported identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Ends inside a field.
    Truncated,
    /// Bytes after the digest.
    Trailing,
    /// A number not in its shortest varint form, or longer than 9 bytes: an identifier has one encoding.
    Varint,
    /// A version other than 1 (CIDv0 starts with the hash code 0x12).
    Version,
    /// A content type not supported.
    Codec,
    /// A hash algorithm not supported.
    Algorithm,
    /// A digest length other than the algorithm's.
    Length,
    /// Not the `b` prefix with lowercase base32 and no padding, or stray bits at the end.
    Text,
}

/// The identifier of one immutable representation: the type, the algorithm and the digest of its exact bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cid { codec: Codec, algorithm: Algorithm, digest: [u8; sha256::DIGEST] }

impl Cid {
    /// The identifier of `data` as content of type `codec`, with SHA-256.
    pub fn of(codec: Codec, data: &[u8]) -> Self {
        let algorithm = Algorithm::Sha2_256;
        Self { codec, algorithm, digest: algorithm.digest(data) }
    }
    pub fn raw(data: &[u8]) -> Self { Self::of(Codec::Raw, data) }
    pub fn codec(&self) -> Codec { self.codec }
    pub fn algorithm(&self) -> Algorithm { self.algorithm }
    pub fn digest(&self) -> &[u8; sha256::DIGEST] { &self.digest }

    /// Whether `data` is the representation this identifier names (its digest by the identifier's algorithm).
    pub fn matches(&self, data: &[u8]) -> bool { self.algorithm.digest(data) == self.digest }

    pub fn to_bytes(&self) -> [u8; BYTES] {
        let mut out = [0u8; BYTES];
        out[..4].copy_from_slice(&[VERSION as u8, self.codec.code() as u8, self.algorithm.code() as u8, self.digest.len() as u8]);
        out[4..].copy_from_slice(&self.digest);
        out
    }

    /// Reads an identifier at the start of `bytes`; returns it and the bytes it took.
    pub fn read(bytes: &[u8]) -> Result<(Self, usize), Error> {
        let (version, mut at) = varint(bytes)?;
        if version != VERSION { return Err(Error::Version); }
        let (codec, n) = varint(&bytes[at..])?;
        at += n;
        let codec = Codec::from_code(codec).ok_or(Error::Codec)?;
        let (algorithm, n) = varint(&bytes[at..])?;
        at += n;
        let algorithm = Algorithm::from_code(algorithm).ok_or(Error::Algorithm)?;
        let (len, n) = varint(&bytes[at..])?;
        at += n;
        if len != algorithm.digest_len() as u64 { return Err(Error::Length); }
        let digest = bytes.get(at..at + algorithm.digest_len()).ok_or(Error::Truncated)?;
        Ok((Self { codec, algorithm, digest: digest.try_into().unwrap() }, at + algorithm.digest_len()))
    }

    /// The identifier that is exactly `bytes`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let (cid, len) = Self::read(bytes)?;
        if len != bytes.len() { return Err(Error::Trailing); }
        Ok(cid)
    }

    pub fn to_text(&self) -> [u8; TEXT] {
        let mut out = [0u8; TEXT];
        out[0] = b'b';
        let (mut bits, mut count, mut at) = (0u32, 0, 1);
        for &byte in self.to_bytes().iter() {
            bits = bits << 8 | byte as u32;
            count += 8;
            while count >= 5 { count -= 5; out[at] = BASE32[(bits >> count) as usize & 31]; at += 1; }
        }
        if count > 0 { out[at] = BASE32[(bits << (5 - count)) as usize & 31]; }
        out
    }

    /// The identifier whose text form is exactly `text`.
    pub fn from_text(text: &[u8]) -> Result<Self, Error> {
        let Some((&b'b', digits)) = text.split_first() else { return Err(Error::Text) };
        let mut out = [0u8; BYTES];
        let (mut bits, mut count, mut len) = (0u32, 0, 0);
        for &c in digits {
            let value = BASE32.iter().position(|&d| d == c).ok_or(Error::Text)? as u32;
            bits = bits << 5 | value;
            count += 5;
            if count >= 8 {
                count -= 8;
                if len == BYTES { return Err(Error::Trailing); }
                out[len] = (bits >> count) as u8;
                len += 1;
            }
        }
        // Fewer than 5 bits are left over and they are zero, so no other text decodes to the same bytes.
        if count >= 5 || bits & ((1 << count) - 1) != 0 { return Err(Error::Text); }
        Self::from_bytes(&out[..len])
    }
}

impl core::fmt::Display for Cid {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_str(core::str::from_utf8(&self.to_text()).unwrap_or("?"))
    }
}

/// An unsigned varint (multiformats): 7 bits a byte, low first, at most 9 bytes, in its shortest form.
fn varint(bytes: &[u8]) -> Result<(u64, usize), Error> {
    let mut value = 0u64;
    for (i, &byte) in bytes.iter().take(9).enumerate() {
        value |= ((byte & 0x7f) as u64) << (7 * i);
        if byte & 0x80 == 0 {
            if byte == 0 && i > 0 { return Err(Error::Varint); }
            return Ok((value, i + 1));
        }
    }
    if bytes.len() >= 9 { Err(Error::Varint) } else { Err(Error::Truncated) }
}
