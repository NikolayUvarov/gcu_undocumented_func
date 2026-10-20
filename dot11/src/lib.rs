//! The 802.11 station (550-NET-0017, IEEE 802.11-2020): its frames, what it hears of the networks around, and the state
//! machine that scans, joins a network (open or WPA2-PSK through `wpa`) and carries Ethernet frames. No I/O: the
//! station service `wlan` drives it with the radio's frames and a clock.
#![no_std]
#![forbid(unsafe_code)]

pub mod country;
pub mod frame;
pub mod scan;
pub mod station;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Too short, a length that does not fit, a field out of range.
    Malformed,
    /// Not a frame or step the station expects now.
    Unexpected,
    /// The output buffer is too small.
    Space,
    /// A scan or a join is under way.
    Busy,
    /// No radio channels, or not joined.
    NoDevice,
    /// No network of that name in the last scan.
    NotFound,
    /// Its networks use a protection the station does not join (WEP, WPA3, enterprise, TKIP), or a channel where the
    /// country applied leaves it nothing to send (550-NET-0032).
    Unsupported,
    /// A WPA2 network without a key, or a frame the station will not send.
    Invalid,
    /// A protected frame whose MIC does not match.
    Integrity,
    /// A packet number or replay counter already seen.
    Replay,
    /// The access point's RSN element in the handshake is not the advertised one: the station left.
    Downgrade,
    /// The key service gave no keys for the handshake (550-NET-0024): the station left.
    NoKey,
}
