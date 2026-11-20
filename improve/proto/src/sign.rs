//! What a device's or an agent's signature covers (docs/hub/README.md, 4.1). Every request that changes something is
//! signed over its method, its path, its body's SHA-256 and a challenge the hub gave in its last answer, good for one
//! request: so a replay is refused and no clock is trusted. A device's ID is the SHA-256 of its public key.
//!
//! The header: `Hub-Signature: ed25519 <public key> <challenge> <signature>`, each in lower-case hex.

use alloc::string::String;
use alloc::vec::Vec;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

/// The bytes before the rest, so that this signature never passes as one of another kind.
pub const PREFIX: &[u8] = b"MIND Hub request 1\0";
/// A challenge's bytes, and how long the hub keeps one open.
pub const CHALLENGE_LEN: usize = 32;
pub const CHALLENGE_LIFE_MS: u64 = 600_000;
/// The longest path a request may name.
pub const PATH_MAX: usize = 256;
/// The header's name.
pub const HEADER: &str = "Hub-Signature";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not `ed25519` and three hex fields of the right lengths.
    Header,
    /// A method other than the API's, or a path out of its form.
    Request,
    /// Not a valid public key.
    Key,
    /// The signature does not verify.
    Signature,
}

/// The methods the API uses.
const METHODS: [&str; 4] = ["GET", "POST", "PUT", "DELETE"];

/// A device's or an agent's ID: the SHA-256 of its public key.
pub fn id(public_key: &[u8; 32]) -> [u8; 32] { Sha256::digest(public_key).into() }

/// The signed bytes: PREFIX, the method, the path, the body's SHA-256 in hex and the challenge in hex, a line end after
/// each but the last.
pub fn signed_bytes(method: &str, path: &str, body: &[u8], challenge: &[u8; 32]) -> Result<Vec<u8>, Error> {
    if !METHODS.contains(&method) || path.is_empty() || path.len() > PATH_MAX || !path.starts_with('/')
        || path.bytes().any(|b| !(0x21..0x7f).contains(&b)) {
        return Err(Error::Request);
    }
    let mut out = Vec::with_capacity(PREFIX.len() + method.len() + path.len() + 64 * 2 + 3);
    out.extend_from_slice(PREFIX);
    out.extend_from_slice(method.as_bytes());
    out.push(b'\n');
    out.extend_from_slice(path.as_bytes());
    out.push(b'\n');
    out.extend_from_slice(crate::hex32(&Sha256::digest(body).into()).as_bytes());
    out.push(b'\n');
    out.extend_from_slice(crate::hex32(challenge).as_bytes());
    Ok(out)
}

/// The header's value for a request signed with `key`.
pub fn header(key: &SigningKey, method: &str, path: &str, body: &[u8], challenge: &[u8; 32]) -> Result<String, Error> {
    let bytes = signed_bytes(method, path, body, challenge)?;
    let signature = key.sign(&bytes);
    let mut out = String::from("ed25519 ");
    out.push_str(&crate::hex32(key.verifying_key().as_bytes()));
    out.push(' ');
    out.push_str(&crate::hex32(challenge));
    out.push(' ');
    out.push_str(&crate::hex(&signature.to_bytes()));
    Ok(out)
}

/// A header's fields: the public key, the challenge and the signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signed { pub public_key: [u8; 32], pub challenge: [u8; 32], pub signature: [u8; 64] }

/// The header's value parsed, in its one encoding.
pub fn parse_header(value: &str) -> Result<Signed, Error> {
    let mut fields = value.split(' ');
    let (Some("ed25519"), Some(key), Some(challenge), Some(signature), None) = (fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
    else { return Err(Error::Header) };
    let mut out = Signed { public_key: [0; 32], challenge: [0; 32], signature: [0; 64] };
    if !crate::parse_hex(key, &mut out.public_key) || !crate::parse_hex(challenge, &mut out.challenge) || !crate::parse_hex(signature, &mut out.signature) {
        return Err(Error::Header);
    }
    Ok(out)
}

/// Whether `signed` signs this request. Whether its challenge is one the hub gave and has not seen used is the hub's
/// to check, before or after this.
pub fn verify(signed: &Signed, method: &str, path: &str, body: &[u8]) -> Result<(), Error> {
    let key = VerifyingKey::from_bytes(&signed.public_key).map_err(|_| Error::Key)?;
    let bytes = signed_bytes(method, path, body, &signed.challenge)?;
    // Strict: a weak (small-order) key or a non-canonical signature is refused, so a signature fits one key alone.
    key.verify_strict(&bytes, &Signature::from_bytes(&signed.signature)).map_err(|_| Error::Signature)
}
