//! WPA2-PSK for the 802.11 station (550-NET-0016): the PMK of a passphrase, the PTK, EAPOL-Key frames, a supplicant's
//! 4-way and group key handshakes, and CCMP (IEEE 802.11-2020 12.5.3, 12.7). It holds no state beyond one association
//! and does no I/O: the station feeds it frames and sends what it answers. No allocation and no libmind, so its tests
//! run on the host (`cargo test`).
#![no_std]
#![forbid(unsafe_code)]

pub mod authenticator;
pub mod ccmp;
pub mod eapol;
pub mod handshake;
pub mod kept;
pub mod keys;

/// Why a frame was not taken. The station drops the frame; `Downgrade` also ends the association (12.7.6.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Too short, a length that does not fit, or a field the standard does not allow here.
    Malformed,
    /// Not the message the handshake waits for now.
    Unexpected,
    /// A replay counter or packet number already seen.
    Replay,
    /// The MIC does not match.
    Mic,
    /// The key data did not unwrap, or lacks what it must carry.
    KeyData,
    /// The access point's RSN element in message 3 is not the one it advertised.
    Downgrade,
    /// The output buffer is too small, or no room is left.
    Space,
    /// The key service gave no keys for the handshake (550-NET-0024).
    NoKey,
}
