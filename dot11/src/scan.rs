//! What the station hears of the networks around: a BSS from each beacon or probe response, its protection read from
//! the RSN element (9.4.2.24), and a bounded list of the strongest.
use crate::frame::{self, Elements, Header, Mac, BEACON, PROBE_RESPONSE, TYPE_MANAGEMENT};

pub const RSNE_MAX: usize = 64;
pub const LIST: usize = 32;

/// How a network is protected, as far as the station can tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security {
    Open,
    /// The privacy bit without an RSN element: WEP or WPA (version 1), which the station does not join.
    Wep,
    /// RSN with PSK (AKM 2) and CCMP-128 as pairwise and group cipher: what the station joins.
    Wpa2,
    /// RSN with SAE (AKM 8) and no PSK.
    Wpa3,
    /// RSN with 802.1X (AKM 1) and no PSK.
    Enterprise,
    /// Any other RSN: TKIP, a required management frame protection, another AKM.
    Other,
}

/// A network an access point announced.
#[derive(Clone, Copy, Debug)]
pub struct Bss {
    pub bssid: Mac,
    pub ssid: [u8; 32],
    pub ssid_len: usize,
    pub channel: u8,
    /// The signal as minus dBm (60: -60 dBm; 0 unknown).
    pub weakness: u8,
    pub capability: u16,
    pub security: Security,
    pub rsne: [u8; RSNE_MAX],
    pub rsne_len: usize,
    /// When it was last heard (the station's clock, ms).
    pub seen: u64,
}

impl Bss {
    pub fn ssid(&self) -> &[u8] { &self.ssid[..self.ssid_len] }
    pub fn rsne(&self) -> Option<&[u8]> { (self.rsne_len > 0).then(|| &self.rsne[..self.rsne_len]) }
}

const OUI: [u8; 3] = [0x00, 0x0F, 0xAC];
const CCMP: u8 = 4;
const AKM_8021X: u8 = 1;
const AKM_PSK: u8 = 2;
const AKM_SAE: u8 = 8;
/// RSN capabilities: management frame protection required.
const MFPR: u16 = 1 << 6;

/// The protection an RSN element (header included) describes.
pub fn security(rsne: &[u8]) -> Security {
    let suites = |data: &[u8], at: usize| -> Option<(usize, usize)> {
        let count = u16::from_le_bytes([*data.get(at)?, *data.get(at + 1)?]) as usize;
        (data.len() >= at + 2 + 4 * count).then_some((at + 2, count))
    };
    let parse = || -> Option<Security> {
        let body = rsne.get(2..)?;
        if u16::from_le_bytes([*body.first()?, *body.get(1)?]) != 1 { return None; }
        let group = body.get(2..6)?;
        let (pairwise, count) = suites(body, 6)?;
        let ccmp = (0..count).any(|i| body[pairwise + 4 * i..pairwise + 4 * i + 4] == [OUI[0], OUI[1], OUI[2], CCMP]);
        let (akms, akm_count) = suites(body, pairwise + 4 * count)?;
        let has = |akm: u8| (0..akm_count).any(|i| body[akms + 4 * i..akms + 4 * i + 4] == [OUI[0], OUI[1], OUI[2], akm]);
        let at = akms + 4 * akm_count;
        let capabilities = body.get(at..at + 2).map_or(0, |c| u16::from_le_bytes([c[0], c[1]]));
        Some(if has(AKM_PSK) && ccmp && group == [OUI[0], OUI[1], OUI[2], CCMP] && capabilities & MFPR == 0 { Security::Wpa2 }
             else if has(AKM_SAE) { Security::Wpa3 }
             else if has(AKM_8021X) { Security::Enterprise }
             else { Security::Other })
    };
    parse().unwrap_or(Security::Other)
}

/// The RSN element the station sends for a WPA2-PSK network: version 1, the network's group cipher (CCMP), CCMP
/// pairwise, PSK, no capabilities. Returns it and its length.
pub fn own_rsne() -> [u8; 22] {
    [0x30, 20, 1, 0, 0x00, 0x0F, 0xAC, CCMP, 1, 0, 0x00, 0x0F, 0xAC, CCMP, 1, 0, 0x00, 0x0F, 0xAC, AKM_PSK, 0, 0]
}

/// The BSS a beacon or probe response describes. `channel` is the one the radio was on; the DS parameter element,
/// where present, names the access point's own.
pub fn describe(frame: &[u8], h: &Header, channel: u8, weakness: u8, now: u64) -> Option<Bss> {
    if h.kind != TYPE_MANAGEMENT || (h.subtype != BEACON && h.subtype != PROBE_RESPONSE) || frame::group(&h.a3) { return None; }
    let fixed = frame.get(h.len..h.len + 12)?;
    let capability = u16::from_le_bytes([fixed[10], fixed[11]]);
    if capability & frame::CAPABILITY_ESS == 0 { return None; }
    let elements = &frame[h.len + 12..];
    let ssid = Elements::find(elements, frame::SSID).filter(|s| s.len() <= 32)?;
    let mut bss = Bss { bssid: h.a3, ssid: [0; 32], ssid_len: ssid.len(), channel, weakness, capability, security: Security::Open,
                        rsne: [0; RSNE_MAX], rsne_len: 0, seen: now };
    bss.ssid[..ssid.len()].copy_from_slice(ssid);
    if let Some(&[own]) = Elements::find(elements, frame::DS_PARAMETER) { bss.channel = own; }
    match Elements::find_whole(elements, frame::RSN) {
        Some(rsne) if rsne.len() <= RSNE_MAX => {
            bss.rsne[..rsne.len()].copy_from_slice(rsne);
            bss.rsne_len = rsne.len();
            bss.security = security(rsne);
        }
        Some(_) => bss.security = Security::Other,
        None if capability & frame::CAPABILITY_PRIVACY != 0 => bss.security = Security::Wep,
        None => {}
    }
    // An all-zero SSID is a hidden network's: it is listed with an empty name.
    if ssid.iter().all(|&b| b == 0) { bss.ssid_len = 0; }
    Some(bss)
}

/// The networks heard, one entry per BSSID; when full, the weakest gives way to a stronger one.
pub struct List { entries: [Option<Bss>; LIST] }

impl Default for List { fn default() -> Self { List { entries: [None; LIST] } } }

impl List {
    pub fn clear(&mut self) { self.entries = [None; LIST]; }
    pub fn insert(&mut self, bss: Bss) {
        if let Some(same) = self.entries.iter_mut().flatten().find(|e| e.bssid == bss.bssid) { *same = bss; return; }
        if let Some(free) = self.entries.iter_mut().find(|e| e.is_none()) { *free = Some(bss); return; }
        // Larger weakness is a weaker signal (0, unknown, counts as weakest).
        let rank = |b: &Bss| if b.weakness == 0 { u8::MAX } else { b.weakness };
        if let Some(weakest) = self.entries.iter_mut().max_by_key(|e| e.as_ref().map_or(0, rank)) {
            if weakest.as_ref().is_some_and(|w| rank(w) > rank(&bss)) { *weakest = Some(bss); }
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &Bss> { self.entries.iter().flatten() }
    /// The strongest access point of the network named `ssid` that the station can join.
    pub fn best(&self, ssid: &[u8]) -> Option<&Bss> {
        let rank = |b: &&Bss| if b.weakness == 0 { u8::MAX } else { b.weakness };
        self.iter().filter(|b| b.ssid() == ssid && matches!(b.security, Security::Open | Security::Wpa2)).min_by_key(rank)
    }
}
