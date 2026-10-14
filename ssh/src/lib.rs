//! An SSH 2 client that reads files over SFTP (351-NET-0026, for 351-NET-0004): curve25519-sha256 with OpenSSH's strict
//! key exchange, an ssh-ed25519 host key that must match a pin, chacha20-poly1305@openssh.com, public-key login with an
//! ssh-ed25519 key whose signatures come from the caller (the key service, MC-11.9), one session channel and the SFTP
//! subsystem (version 3) to open, size, read and close files. It does no I/O of its own: the caller's `Io` moves the
//! bytes. No libmind, so `cargo test` runs it on the host against OpenSSH's server. Without its `client` feature it is
//! `login` and `wire` alone: what a login signs, for the key service's check (351-NET-0027).
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(feature = "client")]
pub mod cipher;
pub mod login;
#[cfg(feature = "client")]
pub mod session;
#[cfg(feature = "client")]
pub mod transport;
pub mod wire;

#[cfg(feature = "client")]
pub use session::{connect, Handle, Session};

/// Why a session failed or a request was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The `Io` failed (its own reason is the caller's).
    Io,
    /// The server closed the connection.
    Closed,
    /// A message out of place, malformed or past a bound.
    Protocol,
    /// No algorithm both sides offer, or not SSH 2.
    Algorithms,
    /// The server's host key is not the pinned one, or not ssh-ed25519.
    HostKey,
    /// The host key's signature of the exchange does not verify.
    Signature,
    /// A packet's tag does not match.
    Mac,
    /// The server refused the login, the channel or the subsystem.
    Refused,
    /// The server disconnected.
    Disconnected,
    /// The server started a second key exchange, which this client does not do.
    Rekey,
    /// An SFTP status other than OK, with its code (2: no such file, 3: permission denied).
    Status(u32),
    /// The signer gave no signature.
    NoKey,
    /// No random bytes.
    Random,
}

/// The connection's bytes, moved by the caller: `send` takes them all or fails; `receive` gives at least one byte, or
/// 0 once the connection is closed. A read a signal interrupts is the caller's to retry.
pub trait Io {
    fn send(&mut self, data: &[u8]) -> Result<(), Error>;
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, Error>;
}

/// The login key: its ssh-ed25519 public key, and signatures of the data a public-key login signs (RFC 4252 section 7).
pub trait Keys {
    fn public(&self) -> [u8; 32];
    fn sign(&mut self, data: &[u8]) -> Option<[u8; 64]>;
}
