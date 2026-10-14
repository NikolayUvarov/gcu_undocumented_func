//! What a public-key login signs (RFC 4252 section 7), built by the client and checked by the key service before it
//! signs (351-NET-0027): so the device key signs SSH logins with itself and nothing else. No crypto: the key service
//! takes this module alone (the crate without its `client` feature).
use crate::wire::{Reader, Writer};
use alloc::vec::Vec;

/// SSH_MSG_USERAUTH_REQUEST.
pub const USERAUTH_REQUEST: u8 = 50;
/// The longest user name taken.
pub const USER_MAX: usize = 64;
/// The session identifier's length: the SHA-256 exchange hash of curve25519-sha256, the one exchange the client does.
pub const SESSION_ID: usize = 32;

/// An ssh-ed25519 public key blob: the name and the 32-byte key.
pub fn key_blob(public: &[u8; 32]) -> Vec<u8> {
    let mut w = Writer::default();
    w.string(b"ssh-ed25519").string(public);
    w.data
}

/// The data a login as `user` with ssh-ed25519 key `public` signs, in a session with identifier `session_id`.
pub fn signed_data(session_id: &[u8], user: &[u8], public: &[u8; 32]) -> Vec<u8> {
    let mut w = Writer::default();
    w.string(session_id).u8(USERAUTH_REQUEST).string(user).string(b"ssh-connection").string(b"publickey").bool(true)
        .string(b"ssh-ed25519").string(&key_blob(public));
    w.data
}

/// A user name the key service signs a login for: 1 to USER_MAX printable ASCII characters, no space.
pub fn user_fits(user: &[u8]) -> bool { (1..=USER_MAX).contains(&user.len()) && user.iter().all(|&b| (0x21..=0x7E).contains(&b)) }

/// Whether `data` is exactly what a login with key `public` signs: a 32-byte session identifier, the request, a user
/// `user_fits` takes, `ssh-connection`, `publickey` with a signature, `ssh-ed25519` and that key, and nothing after.
pub fn fits(data: &[u8], public: &[u8; 32]) -> bool {
    let mut r = Reader::new(data);
    let mut read = || -> Result<bool, crate::Error> {
        let session = r.string()?.len() == SESSION_ID;
        let request = r.u8()? == USERAUTH_REQUEST;
        let user = user_fits(r.string()?);
        let service = r.string()? == b"ssh-connection";
        let method = r.string()? == b"publickey";
        let signed = r.u8()? == 1;
        let algorithm = r.string()? == b"ssh-ed25519";
        let key = r.string()? == key_blob(public).as_slice();
        Ok(session && request && user && service && method && signed && algorithm && key)
    };
    read().unwrap_or(false) && r.rest().is_empty()
}
