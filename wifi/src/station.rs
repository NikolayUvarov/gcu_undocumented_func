//! What `wifi` asks of the 802.11 station (550-APP-0033): the networks a scan found, joining one with a passphrase,
//! the link's state and forgetting a network. The calls follow `idl/wifi.wit` 1.0 as the network track agreed it
//! (550-NET-0018): an SSID is the bytes the access point sends, the signal minus dBm, the address the network stack's.
//! `Demo` serves the same interface without a radio. No system calls: tests/wifi_host.rs.
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// How a network is protected; the station joins `Open` and `Wpa2` (wifi.wit 1.0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security { Open, Wep, Wpa2, Wpa3, Enterprise, Other }

impl Security {
    pub fn name(self) -> &'static str {
        match self {
            Security::Open => "open", Security::Wep => "WEP", Security::Wpa2 => "WPA2", Security::Wpa3 => "WPA3",
            Security::Enterprise => "enterprise", Security::Other => "other",
        }
    }
}

/// One access point a scan heard: its network's name as sent (empty: hidden), BSSID, channel, signal as minus dBm (0
/// unknown), protection, and whether the station keeps a key for the name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network { pub ssid: Vec<u8>, pub bssid: u64, pub channel: u8, pub weakness: u8, pub security: Security, pub remembered: bool }

/// Why joining failed or the link went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason { WrongPassphrase, NoAnswer, Refused, NotFound, Unsupported, Lost }

impl Reason {
    pub fn text(self) -> &'static str {
        match self {
            Reason::WrongPassphrase => "the passphrase is wrong (the 4-way handshake failed)",
            Reason::NoAnswer => "the access point does not answer",
            Reason::Refused => "the access point refused to let it in",
            Reason::NotFound => "no network of that name was heard",
            Reason::Unsupported => "the station does not join that network's protection",
            Reason::Lost => "the link was lost",
        }
    }
}

/// The station's link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    Off,
    Scanning,
    Joining { ssid: Vec<u8> },
    /// `rate`: the data rate in 500 kb/s units.
    Joined { ssid: Vec<u8>, bssid: u64, channel: u8, weakness: u8, rate: u8 },
    /// Lost: the station is joining again by itself.
    Lost { ssid: Vec<u8> },
    Failed { ssid: Vec<u8>, reason: Reason },
}

/// Why the station did not do it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal { NoStation(String), Busy, Invalid, Denied, Unsupported, NotFound, Lost }

impl Refusal {
    pub fn text(&self) -> String {
        String::from(match self {
            Refusal::NoStation(why) => return why.clone(),
            Refusal::Busy => "the station is scanning: try again",
            Refusal::Invalid => "the station does not take that passphrase",
            Refusal::Denied => "wifi's client of the station only reads: joining, leaving and forgetting need the change badge",
            Refusal::Unsupported => "the station does not join that network's protection",
            Refusal::NotFound => "the last scan did not hear that network: r scans again",
            Refusal::Lost => "the station does not answer",
        })
    }
}

/// The station's configuration interface (idl/wifi.wit 1.0), as `wifi` uses it.
pub trait Station {
    /// The access points heard: the scan ends before it answers.
    fn scan(&mut self) -> Result<Vec<Network>, Refusal>;
    /// Starts joining `ssid` with `passphrase` (empty for an open or a remembered network); `remember`: the station
    /// keeps the key through `keystore`, never in a file (MC-11.9). `status` follows it.
    fn join(&mut self, ssid: &[u8], passphrase: &str, remember: bool) -> Result<(), Refusal>;
    fn status(&mut self) -> Result<Link, Refusal>;
    fn leave(&mut self) -> Result<(), Refusal>;
    /// Forgets a remembered network; false when none had that name.
    fn forget(&mut self, ssid: &[u8]) -> Result<bool, Refusal>;
}

/// Whether `security` is one the station joins and `passphrase` one it takes: WPA2-PSK 8 to 63 printable ASCII
/// characters or 64 hexadecimal digits (IEEE 802.11, the PSK mapping); an open network none.
pub fn check(security: Security, passphrase: &str) -> Result<(), &'static str> {
    let len = passphrase.len();
    match security {
        Security::Open => Ok(()),
        Security::Wep => Err("WEP is broken and is not joined"),
        Security::Wpa3 => Err("the station does not join WPA3 (SAE) networks yet"),
        Security::Enterprise => Err("an enterprise network needs 802.1X (EAP), which the station does not do"),
        Security::Other => Err("the station does not join this protection (TKIP, required frame protection or another key management)"),
        Security::Wpa2 if len == 64 && passphrase.bytes().all(|b| b.is_ascii_hexdigit()) => Ok(()),
        Security::Wpa2 if !passphrase.bytes().all(|b| (0x20..=0x7E).contains(&b)) => Err("a WPA2 passphrase is printable ASCII (Latin letters, digits, signs)"),
        Security::Wpa2 if !(8..=63).contains(&len) => Err("a WPA2 passphrase has 8 to 63 characters (or 64 hexadecimal digits)"),
        Security::Wpa2 => Ok(()),
    }
}

/// An SSID as text: UTF-8 as it is, every other byte and every control character as `\xNN`, so a name stays exact.
pub fn shown(ssid: &[u8]) -> String {
    let mut out = String::new();
    let mut rest = ssid;
    while !rest.is_empty() {
        let (good, bad) = match core::str::from_utf8(rest) {
            Ok(text) => (text, 0),
            Err(e) => (core::str::from_utf8(&rest[..e.valid_up_to()]).unwrap_or(""), e.error_len().unwrap_or(rest.len() - e.valid_up_to())),
        };
        for c in good.chars() {
            if c.is_control() { out.push_str(&format!("\\x{:02X}", c as u32)); } else { out.push(c); }
        }
        for b in &rest[good.len()..good.len() + bad] { out.push_str(&format!("\\x{:02X}", b)); }
        rest = &rest[good.len() + bad..];
    }
    out
}

/// A BSSID (low 48 bits, first byte in bits 40-47) as 02:4D:49:4E:44:01.
pub fn mac(bssid: u64) -> String {
    let b = bssid.to_be_bytes();
    format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", b[2], b[3], b[4], b[5], b[6], b[7])
}

/// A data rate in 500 kb/s units as Mb/s: 54, 5.5.
pub fn rate(units: u8) -> String { if units % 2 == 0 { format!("{}", units / 2) } else { format!("{}.5", units / 2) } }

/// The station's stand-in for `wifi --demo` and the QEMU test (QEMU has no Wi-Fi), serving wifi.wit 1.0 with fixed
/// access points: joining answers at the second `status` call; MIND-Lab's and Home's passphrase is `DEMO_PASSPHRASE`.
/// `remembers` false answers `remember` as the station does until it keeps keys (unsupported).
pub struct Demo { remembered: Vec<Vec<u8>>, link: Link, steps: u8, pending: Option<(bool, bool)>, pub remembers: bool, pub scans: usize }

/// The stand-in's passphrase for its protected networks.
pub const DEMO_PASSPHRASE: &str = "correct horse";

// What the stand-in hears: name, BSSID, channel, minus dBm, protection.
const HEARD: [(&[u8], u64, u8, u8, Security); 10] = [
    (b"MIND-Lab", 0x024D_494E_4401, 6, 48, Security::Wpa2),
    (b"MIND-Lab", 0x024D_494E_4402, 11, 63, Security::Wpa2),
    (b"Home", 0x0248_4F4D_4503, 1, 60, Security::Wpa2),
    (b"Cafe guests", 0x0243_4146_4504, 1, 71, Security::Open),
    (b"", 0x0248_4944_4505, 6, 77, Security::Wpa2),
    (b"Office", 0x024F_4646_4906, 11, 80, Security::Enterprise),
    (b"Caf\xE9 TKIP", 0x0254_4B49_5007, 9, 84, Security::Other),
    (b"Old router", 0x024F_4C44_5208, 3, 88, Security::Wep),
    (b"Neighbour", 0x024E_4549_4709, 36, 90, Security::Wpa3),
    (b"Far away", 0x0246_4152_410A, 13, 92, Security::Wpa2),
];

impl Demo {
    pub fn new() -> Self { Self { remembered: Vec::from([Vec::from(&b"Home"[..])]), link: Link::Off, steps: 0, pending: None, remembers: true, scans: 0 } }
}

impl Default for Demo { fn default() -> Self { Self::new() } }

impl Station for Demo {
    fn scan(&mut self) -> Result<Vec<Network>, Refusal> {
        self.scans += 1;
        Ok(HEARD.iter().map(|&(ssid, bssid, channel, weakness, security)| Network {
            ssid: Vec::from(ssid), bssid, channel, weakness, security, remembered: self.remembered.iter().any(|r| r == ssid),
        }).collect())
    }
    fn join(&mut self, ssid: &[u8], passphrase: &str, remember: bool) -> Result<(), Refusal> {
        let Some(&(_, _, _, _, security)) = HEARD.iter().find(|h| h.0 == ssid && !ssid.is_empty()) else { return Err(Refusal::NotFound) };
        if !matches!(security, Security::Open | Security::Wpa2) { return Err(Refusal::Unsupported); }
        if remember && !self.remembers { return Err(Refusal::Unsupported); }
        let known = self.remembered.iter().any(|r| r == ssid);
        if security == Security::Wpa2 && (if passphrase.is_empty() { !known } else { check(security, passphrase).is_err() }) { return Err(Refusal::Invalid); }
        let right = security == Security::Open || if passphrase.is_empty() { known } else { passphrase == DEMO_PASSPHRASE };
        (self.link, self.steps, self.pending) = (Link::Joining { ssid: Vec::from(ssid) }, 0, Some((right, remember && !passphrase.is_empty())));
        Ok(())
    }
    fn status(&mut self) -> Result<Link, Refusal> {
        if let (Link::Joining { ssid }, Some((right, remember))) = (&self.link, self.pending) {
            self.steps += 1;
            if self.steps >= 2 {
                let ssid = ssid.clone();
                self.pending = None;
                self.link = if !right { Link::Failed { ssid, reason: Reason::WrongPassphrase } } else {
                    if remember && !self.remembered.contains(&ssid) { self.remembered.push(ssid.clone()); }
                    let &(_, bssid, channel, weakness, _) = HEARD.iter().filter(|h| h.0 == ssid.as_slice()).min_by_key(|h| h.3).unwrap_or(&HEARD[0]);
                    Link::Joined { ssid, bssid, channel, weakness, rate: if channel == 1 { 11 } else { 108 } } // 5.5 Mb/s, 54 Mb/s
                };
            }
        }
        Ok(self.link.clone())
    }
    fn leave(&mut self) -> Result<(), Refusal> { (self.link, self.pending) = (Link::Off, None); Ok(()) }
    fn forget(&mut self, ssid: &[u8]) -> Result<bool, Refusal> {
        let before = self.remembered.len();
        self.remembered.retain(|r| r != ssid);
        if matches!(&self.link, Link::Joined { ssid: s, .. } if s == ssid) { self.link = Link::Off; }
        Ok(self.remembered.len() != before)
    }
}

/// Without a station every call says why.
pub struct Absent(pub String);

impl Station for Absent {
    fn scan(&mut self) -> Result<Vec<Network>, Refusal> { Err(Refusal::NoStation(self.0.clone())) }
    fn join(&mut self, _: &[u8], _: &str, _: bool) -> Result<(), Refusal> { Err(Refusal::NoStation(self.0.clone())) }
    fn status(&mut self) -> Result<Link, Refusal> { Err(Refusal::NoStation(self.0.clone())) }
    fn leave(&mut self) -> Result<(), Refusal> { Err(Refusal::NoStation(self.0.clone())) }
    fn forget(&mut self, _: &[u8]) -> Result<bool, Refusal> { Err(Refusal::NoStation(self.0.clone())) }
}
