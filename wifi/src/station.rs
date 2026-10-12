//! What `wifi` asks of the 802.11 station (550-APP-0033): the networks a scan found, joining one with a passphrase,
//! the link's state and forgetting a network. The calls follow the interface the tools track proposed to the network
//! track for `idl/wifi.wit` (requests-NET.md); `Demo` stands in for the station until the network track's `wlan`
//! serves it. No system calls: tests/wifi_host.rs.
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// How a network is protected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security { Open, Wep, Wpa2, Wpa3, Enterprise }

impl Security {
    pub fn name(self) -> &'static str {
        match self { Security::Open => "open", Security::Wep => "WEP", Security::Wpa2 => "WPA2", Security::Wpa3 => "WPA3", Security::Enterprise => "enterprise" }
    }
}

/// One access point a scan heard: its network's name (empty: hidden), address, channel, signal in dBm, protection,
/// and whether the station keeps a passphrase for the name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network { pub ssid: String, pub bssid: [u8; 6], pub channel: u8, pub signal: i8, pub security: Security, pub remembered: bool }

/// Why joining failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason { WrongPassphrase, NoAnswer, Refused, NoAddress, Lost }

impl Reason {
    pub fn text(self) -> &'static str {
        match self {
            Reason::WrongPassphrase => "the passphrase is wrong (the 4-way handshake failed)",
            Reason::NoAnswer => "the access point does not answer",
            Reason::Refused => "the access point refused to associate",
            Reason::NoAddress => "joined, but DHCP gave no address",
            Reason::Lost => "the link was lost",
        }
    }
}

/// The station's link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    Off,
    Joining { ssid: String },
    /// `address`: IPv4 as a.b.c.d in a u32, 0 while DHCP has not answered.
    Joined { ssid: String, address: u32, signal: i8 },
    Failed { ssid: String, reason: Reason },
}

/// Why the station did not do it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal { NoStation(String), Busy, Invalid, Denied, Unsupported, Lost }

impl Refusal {
    pub fn text(&self) -> String {
        match self {
            Refusal::NoStation(why) => why.clone(),
            Refusal::Busy => String::from("the station is busy: try again"),
            Refusal::Invalid => String::from("the station does not take that"),
            Refusal::Denied => String::from("wifi's client may not change the station's settings"),
            Refusal::Unsupported => String::from("the station does not support that network's protection"),
            Refusal::Lost => String::from("the station does not answer"),
        }
    }
}

/// The station's configuration interface, as `wifi` uses it.
pub trait Station {
    /// The access points heard on the last scan, a new scan started.
    fn scan(&mut self) -> Result<Vec<Network>, Refusal>;
    /// Starts joining `ssid` (with `passphrase`, none for an open or a remembered network); `remember`: the station
    /// keeps the passphrase through `keystore`, never in a file (MC-11.9). `status` follows it.
    fn join(&mut self, ssid: &str, passphrase: Option<&str>, remember: bool) -> Result<(), Refusal>;
    fn status(&mut self) -> Result<Link, Refusal>;
    fn leave(&mut self) -> Result<(), Refusal>;
    /// Forgets a remembered network; false when none had that name.
    fn forget(&mut self, ssid: &str) -> Result<bool, Refusal>;
}

/// Whether `passphrase` is one `security` takes: WPA2-PSK 8 to 63 printable ASCII characters or 64 hexadecimal
/// digits (IEEE 802.11, the PSK mapping); WPA3-SAE 1 to 128 printable characters; an open network none. WEP and
/// enterprise networks are not joined.
pub fn check(security: Security, passphrase: &str) -> Result<(), &'static str> {
    let printable = passphrase.bytes().all(|b| (0x20..=0x7E).contains(&b));
    let len = passphrase.len();
    match security {
        Security::Open => Ok(()),
        Security::Wep => Err("WEP is broken and is not joined"),
        Security::Enterprise => Err("an enterprise network needs 802.1X (EAP), which the station does not do"),
        Security::Wpa2 if len == 64 && passphrase.bytes().all(|b| b.is_ascii_hexdigit()) => Ok(()),
        Security::Wpa2 if !printable => Err("a WPA2 passphrase is printable ASCII (Latin letters, digits, signs)"),
        Security::Wpa2 if !(8..=63).contains(&len) => Err("a WPA2 passphrase has 8 to 63 characters (or 64 hexadecimal digits)"),
        Security::Wpa3 if !printable => Err("a WPA3 password is printable ASCII (Latin letters, digits, signs)"),
        Security::Wpa3 if !(1..=128).contains(&len) => Err("a WPA3 password has 1 to 128 characters"),
        _ => Ok(()),
    }
}

/// The station's stand-in for `wifi --demo` and the QEMU test (QEMU has no Wi-Fi): fixed access points; joining
/// answers at the second `status` call; MIND-Lab's and Home-WPA3's passphrase is `DEMO_PASSPHRASE`.
pub struct Demo { remembered: Vec<String>, link: Link, steps: u8, pending: Option<(bool, bool)>, pub scans: usize }

/// The stand-in's passphrase for its protected networks.
pub const DEMO_PASSPHRASE: &str = "correct horse";

// What the stand-in hears: name, address, channel, signal, protection.
const HEARD: [(&str, [u8; 6], u8, i8, Security); 8] = [
    ("MIND-Lab", [0x02, 0x4D, 0x49, 0x4E, 0x44, 0x01], 6, -48, Security::Wpa2),
    ("MIND-Lab", [0x02, 0x4D, 0x49, 0x4E, 0x44, 0x02], 11, -63, Security::Wpa2),
    ("Home-WPA3", [0x02, 0x48, 0x4F, 0x4D, 0x45, 0x03], 1, -60, Security::Wpa3),
    ("Cafe guests", [0x02, 0x43, 0x41, 0x46, 0x45, 0x04], 1, -71, Security::Open),
    ("", [0x02, 0x48, 0x49, 0x44, 0x45, 0x05], 6, -77, Security::Wpa2),
    ("Office", [0x02, 0x4F, 0x46, 0x46, 0x49, 0x06], 11, -80, Security::Enterprise),
    ("Old router", [0x02, 0x4F, 0x4C, 0x44, 0x52, 0x07], 3, -88, Security::Wep),
    ("Far away", [0x02, 0x46, 0x41, 0x52, 0x41, 0x08], 13, -92, Security::Wpa2),
];

impl Demo {
    pub fn new() -> Self { Self { remembered: Vec::from([String::from("Home-WPA3")]), link: Link::Off, steps: 0, pending: None, scans: 0 } }
}

impl Default for Demo { fn default() -> Self { Self::new() } }

impl Station for Demo {
    fn scan(&mut self) -> Result<Vec<Network>, Refusal> {
        self.scans += 1;
        Ok(HEARD.iter().map(|&(ssid, bssid, channel, signal, security)| Network {
            ssid: ssid.to_string(), bssid, channel, signal, security, remembered: self.remembered.iter().any(|r| r == ssid),
        }).collect())
    }
    fn join(&mut self, ssid: &str, passphrase: Option<&str>, remember: bool) -> Result<(), Refusal> {
        let Some(&(_, _, _, _, security)) = HEARD.iter().find(|h| h.0 == ssid && !ssid.is_empty()) else { return Err(Refusal::Invalid) };
        if matches!(security, Security::Wep | Security::Enterprise) { return Err(Refusal::Unsupported); }
        let known = self.remembered.iter().any(|r| r == ssid);
        if security != Security::Open && passphrase.is_none() && !known { return Err(Refusal::Invalid); }
        let right = security == Security::Open || passphrase.map_or(known, |p| p == DEMO_PASSPHRASE);
        (self.link, self.steps, self.pending) = (Link::Joining { ssid: ssid.to_string() }, 0, Some((right, remember && passphrase.is_some())));
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
                    let signal = HEARD.iter().filter(|h| h.0 == ssid).map(|h| h.3).max().unwrap_or(-90);
                    Link::Joined { address: if ssid == "Cafe guests" { 0xC0A8_0417 } else { 0x0A00_002A }, ssid, signal } // 192.168.4.23, 10.0.0.42
                };
            }
        }
        Ok(self.link.clone())
    }
    fn leave(&mut self) -> Result<(), Refusal> { (self.link, self.pending) = (Link::Off, None); Ok(()) }
    fn forget(&mut self, ssid: &str) -> Result<bool, Refusal> {
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
    fn join(&mut self, _: &str, _: Option<&str>, _: bool) -> Result<(), Refusal> { Err(Refusal::NoStation(self.0.clone())) }
    fn status(&mut self) -> Result<Link, Refusal> { Err(Refusal::NoStation(self.0.clone())) }
    fn leave(&mut self) -> Result<(), Refusal> { Err(Refusal::NoStation(self.0.clone())) }
    fn forget(&mut self, _: &str) -> Result<bool, Refusal> { Err(Refusal::NoStation(self.0.clone())) }
}

/// An IPv4 address in a u32 as a.b.c.d.
pub fn dotted(address: u32) -> String {
    let b = address.to_be_bytes();
    alloc::format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
}
