//! Badges of network stack clients (idl/socket.wit 2.0, issue 102): who may do what is part of the capability.

/// The operator (the shell): every destination, no term or volume.
pub const BADGE_OPERATOR: u16 = 0xFFFE;
/// The policy broker's control client: registers and drops grants, opens no flows.
pub const BADGE_POLICY: u16 = 0xFFFF;
/// Flow grants of the broker use badges from 1 up to this one.
pub const BADGE_GRANT_LAST: u16 = 0xFFFD;

/// A client of the network policy broker that may only launch (idl/netpolicy.wit 1.2, 800-NET-0038): it prepares, takes
/// and binds grants and reads the policy, but may not change the policy or revoke a grant. The shell's has no badge.
pub const BADGE_LAUNCH: u16 = 1;

/// Whether a client of the policy broker with `badge` may change the policy and revoke grants: only one with no badge,
/// whose holder asks its user first (the shell, 108). A badge this version does not know changes nothing either.
pub fn may_change_policy(badge: u16) -> bool { badge == 0 }

/// A network card driver's client that only reads the card (idl/net.wit 1.3, 000-NET-0015): `info`, `counters`,
/// `offloads`.
pub const BADGE_COUNTERS: u16 = 1;

/// The 802.11 station's configuration client (idl/wifi.wit, 550-NET-0018): it reads; with BADGE_WIFI_CHANGE it may
/// also join, leave and forget. A station client without BADGE_WIFI is the network stack's card (idl/net.wit).
pub const BADGE_WIFI: u16 = 0x0100;
pub const BADGE_WIFI_CHANGE: u16 = 0x0300;
/// A radio's bands (idl/wlanhw.wit 1.1 `info`): 2.4 GHz, 5 GHz, and OFDM on 2.4 GHz (802.11g; without it 802.11b there).
pub const BAND_2GHZ: u8 = 1;
pub const BAND_5GHZ: u8 = 2;
pub const BAND_2GHZ_OFDM: u8 = 4;

/// A client of the TLS service that may not offer the device certificate (idl/tls.wit 1.2, 351-NET-0022): the one a
/// launcher lends to a program, so that it cannot sign in as the device.
pub const BADGE_TLS_NO_DEVICE: u16 = 1;

/// The key service's signer (idl/keystore.wit, issue 103): the TLS service's client may ask for signatures.
pub const BADGE_KEY_SIGNER: u16 = 1;
/// The key service's Wi-Fi station (idl/keystore.wit 1.1, 550-NET-0024): `wlan`'s client may keep, list and forget
/// networks and ask for each handshake's keys.
pub const BADGE_KEY_STATION: u16 = 2;
/// The key service's SSH service (idl/keystore.wit 1.2, 351-NET-0027): `ssh`'s client may ask for `ssh-login`
/// signatures, and nothing else of the device key.
pub const BADGE_KEY_SSH: u16 = 3;
/// The key service's loader (idl/keystore.wit 1.4, 000-KRN-0182): the loader's client may ask for a client for each
/// program's secrets, which it lends in SLOT_SECRETS; it reads no secret itself.
pub const BADGE_KEY_LOADER: u16 = 4;

/// Whether `address` (IPv4, its first byte in bits 24-31) is a public unicast address, which a browsing grant's rule
/// reaches (idl/socket.wit 2.7, a rule's address 0, 000-NET-0049). Not public: this network (0/8), the private ranges
/// (10/8, 172.16/12, 192.168/16), shared space (100.64/10), loopback (127/8), link-local (169.254/16), the IETF's
/// assignments (192.0.0/24), documentation (192.0.2/24, 198.51.100/24, 203.0.113/24), benchmarking (198.18/15),
/// multicast (224/4) and the reserved block with the broadcast address (240/4).
pub const fn public(address: u32) -> bool {
    const NOT_PUBLIC: [(u32, u32); 14] = [
        (0x0000_0000, 8), (0x0A00_0000, 8), (0x6440_0000, 10), (0x7F00_0000, 8), (0xA9FE_0000, 16), (0xAC10_0000, 12),
        (0xC000_0000, 24), (0xC000_0200, 24), (0xC0A8_0000, 16), (0xC612_0000, 15), (0xC633_6400, 24), (0xCB00_7100, 24),
        (0xE000_0000, 4), (0xF000_0000, 4),
    ];
    let mut i = 0;
    while i < NOT_PUBLIC.len() {
        let (network, bits) = NOT_PUBLIC[i];
        if address >> (32 - bits) == network >> (32 - bits) { return false; }
        i += 1;
    }
    true
}

/// The ports a browsing grant may use at most.
pub const BROWSE_PORTS: usize = 4;

/// A browsing grant's line of the network policy (000-NET-0049): `browse <program> public tcp <port>[,<port>...]
/// [<seconds> [<bytes>]]`, at most BROWSE_PORTS ports, none 0. Its program, ports, term and volume, or None.
pub fn browse_line(line: &str) -> Option<(&str, [u16; BROWSE_PORTS], usize, Option<u32>, Option<u64>)> {
    let mut words = line.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("browse") { return None; }
    let program = words.next().filter(|p| (1..=16).contains(&p.len()))?;
    if !words.next()?.eq_ignore_ascii_case("public") || !words.next()?.eq_ignore_ascii_case("tcp") { return None; }
    let (mut ports, mut count) = ([0u16; BROWSE_PORTS], 0);
    for port in words.next()?.split(',') {
        let port = port.parse::<u16>().ok().filter(|&p| p != 0)?;
        if count == BROWSE_PORTS || ports[..count].contains(&port) { return None; }
        ports[count] = port;
        count += 1;
    }
    let seconds = match words.next() { Some(s) => Some(s.parse().ok()?), None => None };
    let bytes = match words.next() { Some(b) => Some(b.parse().ok()?), None => None };
    if words.next().is_some() { return None; }
    Some((program, ports, count, seconds, bytes))
}
