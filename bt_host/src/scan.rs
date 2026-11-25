//! The listening rounds (700-BLT-0002, 0003), run by the stack: an active LE scan of 10 s, then a general inquiry of
//! 10.24 s, at start and every five minutes while no link is up; each device heard is logged once a round. They run
//! only when the disk asks (`data/bt-listen.txt`): a scan takes the radio a Wi-Fi may share.
use crate::ctl::Ctl;
use crate::hci;

pub const SCAN_MS: u64 = 10_000;
pub const ROUND_MS: u64 = 300_000;
const HEARD: usize = 64;
/// The general inquiry access code, 0x9E8B33, and 8 × 1.28 s.
const INQUIRY: [u8; 5] = [0x33, 0x8B, 0x9E, 8, 0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase { Idle, Le, Inquiry }

pub struct Scan { pub on: bool, phase: Phase, at: u64, next: u64, seen: [([u8; 6], bool); HEARD], count: usize, le: u32, bredr: u32, le_events: u32, inquiry_events: u32 }

impl Default for Scan { fn default() -> Self { Self::new() } }

impl Scan {
    pub const fn new() -> Self { Self { on: false, phase: Phase::Idle, at: 0, next: 0, seen: [([0; 6], false); HEARD], count: 0, le: 0, bredr: 0, le_events: 0, inquiry_events: 0 } }

    /// Whether a round holds the radio now.
    pub fn active(&self) -> bool { self.phase != Phase::Idle }

    /// Starts, steps or ends a round; `busy` (a link or a page) holds the next one back.
    pub fn tick(&mut self, ctl: &mut Ctl, busy: bool) {
        let now = ctl.now;
        match self.phase {
            Phase::Idle if self.on && !busy && now >= self.next => {
                (self.count, self.le, self.bredr, self.le_events, self.inquiry_events) = (0, 0, 0, 0, 0);
                // Active, every 60 ms for 30 ms, the public address, every advertiser.
                ctl.command(hci::LE_SET_SCAN_PARAMETERS, &[1, 0x60, 0, 0x30, 0, 0, 0]);
                ctl.command(hci::LE_SET_SCAN_ENABLE, &[1, 1]);
                (self.phase, self.at) = (Phase::Le, now);
            }
            Phase::Le if now >= self.at + SCAN_MS => {
                ctl.command(hci::LE_SET_SCAN_ENABLE, &[0, 0]);
                ctl.command(hci::INQUIRY, &INQUIRY);
                (self.phase, self.at) = (Phase::Inquiry, now);
            }
            Phase::Inquiry if now >= self.at + SCAN_MS + 2_000 => { self.end(ctl, None); }
            _ => {}
        }
    }

    fn end(&mut self, ctl: &mut Ctl, status: Option<u8>) {
        ctl.say(format_args!("ROUND: {} LE AND {} BR/EDR DEVICES HEARD; {} EVENTS IN THE LE SCAN, {} IN THE INQUIRY", self.le, self.bredr, self.le_events, self.inquiry_events));
        match status {
            Some(s) => ctl.say(format_args!("INQUIRY COMPLETE AFTER {} MS, STATUS {:02X}", ctl.now - self.at, s)),
            None => ctl.say(format_args!("NO INQUIRY COMPLETE IN {} MS", SCAN_MS + 2_000)),
        }
        (self.phase, self.next) = (Phase::Idle, ctl.now + ROUND_MS);
    }

    /// An event while a round runs: the devices in it noted.
    pub fn event(&mut self, ctl: &mut Ctl, code: u8, p: &[u8]) {
        match (self.phase, code) {
            (Phase::Le, hci::EV_LE_META) if p.first() == Some(&hci::LE_ADVERTISING_REPORT) => { self.le_events += 1; hci::le_reports(&p[1..], |h| self.note(ctl, h)); }
            (Phase::Inquiry, hci::EV_EXTENDED_INQUIRY_RESULT) => { self.inquiry_events += 1; if let Some(h) = hci::extended_inquiry(p) { self.note(ctl, h) } }
            (Phase::Inquiry, hci::EV_INQUIRY_RESULT_RSSI) => { self.inquiry_events += 1; hci::inquiry_rssi(p, |h| self.note(ctl, h)); }
            (Phase::Inquiry, hci::EV_INQUIRY_COMPLETE) => { self.inquiry_events += 1; self.end(ctl, Some(p.first().copied().unwrap_or(0xFF))); }
            _ => {}
        }
    }

    fn note(&mut self, ctl: &mut Ctl, h: hci::Heard) {
        if self.seen[..self.count].iter().any(|&(a, le)| a == h.addr && le == h.le) { return; }
        if self.count < HEARD { self.seen[self.count] = (h.addr, h.le); self.count += 1; }
        if h.le { self.le += 1 } else { self.bredr += 1 }
        let a = h.addr;
        let mut name = [b' '; 34];
        let shown = h.name.map_or(0, |n| { let n = &n[..n.len().min(32)]; name[0] = b'"'; for (i, &c) in n.iter().enumerate() { name[1 + i] = if (0x20..0x7F).contains(&c) { c } else { b'?' }; } name[1 + n.len()] = b'"'; n.len() + 2 });
        let name = core::str::from_utf8(&name[..shown]).unwrap_or("");
        if h.le {
            ctl.say(format_args!("LE {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} {} RSSI {} APPEARANCE {:04X} {}", a[0], a[1], a[2], a[3], a[4], a[5], if h.random { "RANDOM" } else { "PUBLIC" }, h.rssi, h.kind, name));
        } else {
            ctl.say(format_args!("BR/EDR {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} RSSI {} CLASS {:06X} {} {}", a[0], a[1], a[2], a[3], a[4], a[5], h.rssi, h.kind, hci::major_class(h.kind), name));
        }
    }
}
