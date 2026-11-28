//! The devices the stack may connect to, and the keys it keeps for them: both read from text files on the boot volume
//! for the testing stage (700-BLT-0006). Only a device named in the list is paged, accepted or paired: the scans hear
//! the neighbours' devices too.
//!
//! `data/bluetooth/devices.txt` (or `data/inbox/bt-devices.txt`), a line a device: `AA:BB:CC:DD:EE:FF audio`
//! (headphones that take the machine's output while their stream runs; `audio tone` for a test tone instead), `... hid`,
//! or `... le-hid` (`le-hid random` for a random address). `data/bluetooth/keys.txt`, a line a key: `AA:BB:CC:DD:EE:FF <link key, 32 hex digits> <type, 2>` for
//! BR/EDR, `le AA:BB:CC:DD:EE:FF <LTK, 32> <EDIV, 4> <Rand, 16> <authenticated, 0 or 1>` for LE.
use crate::buf::{unhex, Addr, Hex};

pub const DEVICES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Audio, Hid, LeHid }

impl Kind {
    pub fn name(self) -> &'static str { match self { Kind::Audio => "AUDIO", Kind::Hid => "HID", Kind::LeHid => "LE HID" } }
}

/// An LE link's long term key as the peripheral distributed it (or as LE Secure Connections made it, EDIV and Rand 0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ltk { pub key: [u8; 16], pub ediv: u16, pub rand: [u8; 8], pub authenticated: bool }

#[derive(Clone, Copy, Debug)]
pub struct Device {
    pub addr: Addr, pub kind: Kind, pub random: bool, pub tone: bool,
    /// An audio device's stream at 44.1 kHz rather than 48 (`audio 44100`), as phones stream SBC.
    pub hz44: bool,
    /// The absolute volume (of 127) given an audio device at first (`volume n`), else AVRCP's start value.
    pub volume: Option<u8>,
    pub key: Option<([u8; 16], u8)>, pub ltk: Option<Ltk>,
    /// When the stack may page it next, and how often it failed in a row.
    pub next_try: u64, pub failures: u32,
    /// Once its failures are no longer said one by one: the tries since the last line that summed them up, and when.
    pub quiet_tries: u32, pub summed_at: u64,
}

pub struct Devices { pub list: [Option<Device>; DEVICES] }

impl Default for Devices { fn default() -> Self { Self::new() } }

fn words(line: &str) -> impl Iterator<Item = &str> { line.split(|c: char| c == ' ' || c == '\t').filter(|w| !w.is_empty()) }

impl Devices {
    pub const fn new() -> Self { Self { list: [None; DEVICES] } }

    pub fn is_empty(&self) -> bool { self.list.iter().all(Option::is_none) }
    pub fn get(&self, addr: Addr) -> Option<&Device> { self.list.iter().flatten().find(|d| d.addr == addr) }
    pub fn get_mut(&mut self, addr: Addr) -> Option<&mut Device> { self.list.iter_mut().flatten().find(|d| d.addr == addr) }

    /// The device list's text; `bad` gets each line not understood, with its number.
    pub fn load(&mut self, text: &str, mut bad: impl FnMut(usize, &str)) {
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() { continue; }
            let mut w = words(line);
            let addr = w.next().and_then(Addr::parse);
            let kind = match w.next() { Some("audio") => Some(Kind::Audio), Some("hid") => Some(Kind::Hid), Some("le-hid") => Some(Kind::LeHid), _ => None };
            let (mut random, mut tone, mut hz44, mut volume, mut known) = (false, false, false, None, true);
            let mut w = w.peekable();
            while let Some(word) = w.next() {
                match word {
                    "volume" if kind == Some(Kind::Audio) => match w.next().and_then(|v| v.parse::<u8>().ok()).filter(|&v| v <= 127) { Some(v) => volume = Some(v), None => known = false },
                    "public" => {} "random" => random = true,
                    "tone" if kind == Some(Kind::Audio) => tone = true,
                    "44100" if kind == Some(Kind::Audio) => hz44 = true,
                    "48000" if kind == Some(Kind::Audio) => hz44 = false,
                    _ => known = false,
                }
            }
            let (Some(addr), Some(kind), true) = (addr, kind, known) else { bad(n + 1, line); continue };
            if self.get(addr).is_some() { continue; }
            match self.list.iter_mut().find(|d| d.is_none()) {
                Some(slot) => *slot = Some(Device { addr, kind, random, tone, hz44, volume, key: None, ltk: None, next_try: 0, failures: 0, quiet_tries: 0, summed_at: 0 }),
                None => bad(n + 1, line),
            }
        }
    }

    /// The keys file's text: each key goes to its device in the list; keys of devices not listed are left out.
    pub fn load_keys(&mut self, text: &str) -> usize {
        let mut taken = 0;
        for line in text.lines() {
            let mut w = words(line);
            let first = w.next().unwrap_or("");
            if first == "le" {
                let Some(d) = w.next().and_then(Addr::parse).and_then(|a| self.get_mut(a)) else { continue };
                let (mut ltk, mut ediv, mut rand) = (Ltk::default(), [0u8; 2], [0u8; 8]);
                let ok = w.next().is_some_and(|t| unhex(t, &mut ltk.key)) && w.next().is_some_and(|t| unhex(t, &mut ediv)) && w.next().is_some_and(|t| unhex(t, &mut rand));
                let auth = w.next();
                if !ok || !matches!(auth, Some("0") | Some("1")) { continue; }
                (ltk.ediv, ltk.rand, ltk.authenticated) = (u16::from_be_bytes(ediv), rand, auth == Some("1"));
                d.ltk = Some(ltk);
                taken += 1;
            } else {
                let Some(d) = Addr::parse(first).and_then(|a| self.get_mut(a)) else { continue };
                let (mut key, mut kind) = ([0u8; 16], [0u8; 1]);
                if w.next().is_some_and(|t| unhex(t, &mut key)) && w.next().is_some_and(|t| unhex(t, &mut kind)) { d.key = Some((key, kind[0])); taken += 1; }
            }
        }
        taken
    }

    /// The keys file's text for every key held, into `out`; its length.
    pub fn keys_text(&self, out: &mut [u8]) -> usize {
        let mut line = crate::buf::Line::default();
        let mut at = 0;
        for d in self.list.iter().flatten() {
            line.len = 0;
            if let Some((key, kind)) = d.key { let _ = core::fmt::write(&mut line, format_args!("{} {} {:02X}\n", d.addr, Hex(&key), kind)); }
            if let Some(l) = d.ltk { let _ = core::fmt::write(&mut line, format_args!("le {} {} {:04X} {} {}\n", d.addr, Hex(&l.key), l.ediv, Hex(&l.rand), l.authenticated as u8)); }
            let n = line.len.min(out.len() - at);
            out[at..at + n].copy_from_slice(&line.bytes[..n]);
            at += n;
        }
        at
    }
}
