#![no_std]
#![no_main]
// A stand-in radio for QEMU (550-NET-0018), which has no Wi-Fi device: serves idl/wlanhw.wit as a radio within reach of
// one access point, `MIND-TEST` on channel 6, WPA2-PSK with the passphrase "mind test passphrase". The access point is
// simulated here (its handshake from wpa::authenticator) with a small network behind it: it answers DHCP (10.0.4.15/24,
// gateway and DNS 10.0.4.1), ARP for the gateway and ICMP echo to it. Like `bcm_wifi` today, the radio has no OFDM on
// 2.4 GHz (550-NET-0028): it refuses a frame at an OFDM rate, and the access point logs the rates the station offers.
// It takes a country's limits (wlanhw.wit 1.2, 550-NET-0035): then it tunes only to their channels and refuses a probe
// request where the station may only listen. Until limits are first set it keeps 1.1's ways, as the station of stage A
// sets none. A test service: in no image but the tests'.
use dot11::frame::{self, Mac, BROADCAST, ETHERTYPE_EAPOL, FROM_DS, PROTECTED, SNAP, TO_DS, TYPE_DATA, TYPE_MANAGEMENT};
use mind::abi::BootInfo;
use mind::idl::codec::List;
use mind::idl::{wire, wlanhw};
use mind::ipc::Endpoint;
use wpa::authenticator::{Authenticator, FRAME_MAX as EAPOL_MAX};
use wpa::ccmp::{self, Replay};
use wpa::eapol::Gtk;

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

const RECEIVED_CAP: usize = 9;
const POLL_MS: u32 = 10;
const OWN: Mac = [0x02, 0x00, 0x00, 0x57, 0x4C, 0x01];
const BSSID: Mac = [0x02, 0x00, 0x00, 0x57, 0x4C, 0xA0];
const SSID: &[u8] = b"MIND-TEST";
const PASSPHRASE: &[u8] = b"mind test passphrase";
const CHANNEL: u8 = 6;
const WEAKNESS: u8 = 42;
const GATEWAY: [u8; 4] = [10, 0, 4, 1]; // not 10.0.3.0/24, the second card's network in the net suite
const LEASE: [u8; 4] = [10, 0, 4, 15];
const RADIO_HEADER: usize = 8;
const QUEUE: usize = 16;
const BEACON_MS: u64 = 100;
const FRAME_MAX: usize = 2346;
// The rates the radio sends at, in 500 kb/s units: DSSS and CCK alone.
const RATES: [u8; 4] = [2, 4, 11, 22];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Expect { Nothing, Message2, Message4 }

struct Sim {
    channel: u8,
    filter: wlanhw::Filter,
    queue: Vec<Vec<u8>>,
    waiters: Vec<wire::Call>,
    beacon_at: u64,
    sequence: u16,
    associated: bool,
    authenticator: Option<Authenticator>,
    expect: Expect,
    tries: u8,
    tk: Option<[u8; 16]>,
    replay: Replay,
    pn: u64,
    gtk: Gtk,
    group_pn: u64,
    // Whether a frame at a rate the radio lacks was refused (logged once).
    refused: bool,
    // The country's limits (1.2): each channel and whether the station may only listen there; None before any.
    limits: Option<Vec<(u8, bool)>>,
}

fn mac64(m: &Mac) -> u64 { m.iter().fold(0, |v, &b| v << 8 | b as u64) }

impl Sim {
    // The channels it tunes to: those of the country's limits once set, else 1 to 13.
    fn channels(&self) -> List<u8, 64> {
        let mut channels = List::default();
        match &self.limits { Some(limits) => for &(c, _) in limits { channels.push(c); }, None => for c in 1..=13 { channels.push(c); } }
        channels
    }

    fn info(&self) -> wlanhw::Info {
        wlanhw::Info { mac: mac64(&OWN), bands: mind::network::BAND_2GHZ, channels: self.channels(), frame_max: FRAME_MAX as u16, channel: self.channel }
    }

    // Whether the country's limits let `f` go out on the channel now: a probe request only where the station may send.
    fn may_send(&self, f: &[u8]) -> bool {
        let Some(limits) = &self.limits else { return true };
        let probe = f.first().is_some_and(|&b| b == frame::PROBE_REQUEST << 4);
        limits.iter().any(|&(c, listen)| c == self.channel && !(listen && probe))
    }

    // A frame for the station, led by the radio header of wlanhw.wit, if the filter lets it through.
    fn hear(&mut self, f: &[u8]) {
        let Ok(h) = frame::header(f) else { return };
        let beacon = h.kind == TYPE_MANAGEMENT && (h.subtype == frame::BEACON || h.subtype == frame::PROBE_RESPONSE);
        let ours = self.filter.bssid == mac64(&BSSID);
        if !(ours || (beacon && self.filter.all_beacons)) || self.queue.len() >= QUEUE { return; }
        let mut framed = Vec::with_capacity(RADIO_HEADER + f.len());
        framed.extend_from_slice(&(f.len() as u16).to_le_bytes());
        framed.extend_from_slice(&[WEAKNESS, self.channel, 2, self.filter.all_beacons as u8, 0, 0]);
        framed.extend_from_slice(f);
        self.queue.push(framed);
    }

    fn header(&mut self, subtype: u8, to: &Mac) -> Vec<u8> {
        self.sequence = (self.sequence + 1) & 0x0FFF;
        let mut f = Vec::with_capacity(256);
        f.extend_from_slice(&[subtype << 4, 0, 0, 0]);
        f.extend_from_slice(to);
        f.extend_from_slice(&BSSID);
        f.extend_from_slice(&BSSID);
        f.extend_from_slice(&(self.sequence << 4).to_le_bytes());
        f
    }

    fn announcement(&mut self, subtype: u8, to: &Mac) -> Vec<u8> {
        let mut f = self.header(subtype, to);
        f.extend_from_slice(&[0; 8]);
        f.extend_from_slice(&100u16.to_le_bytes());
        f.extend_from_slice(&(frame::CAPABILITY_ESS | frame::CAPABILITY_PRIVACY | frame::CAPABILITY_SHORT_SLOT).to_le_bytes());
        f.extend_from_slice(&[0, SSID.len() as u8]);
        f.extend_from_slice(SSID);
        f.extend_from_slice(&[1, 8, 0x82, 0x84, 0x8B, 0x96, 0x0C, 0x12, 0x18, 0x24, 3, 1, CHANNEL]);
        f.extend_from_slice(&dot11::scan::own_rsne());
        f.extend_from_slice(&[50, 4, 0x30, 0x48, 0x60, 0x6C]);
        f
    }

    // A data frame from the distribution system to `to`, from `source`, under the TK or (to a group) the GTK.
    fn data(&mut self, to: &Mac, source: &Mac, ethertype: u16, payload: &[u8]) {
        self.sequence = (self.sequence + 1) & 0x0FFF;
        let mut f = Vec::with_capacity(payload.len() + 64);
        f.extend_from_slice(&[TYPE_DATA << 2, FROM_DS, 0, 0]);
        f.extend_from_slice(to);
        f.extend_from_slice(&BSSID);
        f.extend_from_slice(source);
        f.extend_from_slice(&(self.sequence << 4).to_le_bytes());
        f.extend_from_slice(&SNAP);
        f.extend_from_slice(&ethertype.to_be_bytes());
        f.extend_from_slice(payload);
        let Some(tk) = self.tk else { self.hear(&f); return };
        let mut out = alloc::vec![0u8; f.len() + 16];
        let sealed = if frame::group(to) {
            self.group_pn += 1;
            ccmp::encrypt(&self.gtk.key, self.gtk.id, self.group_pn, &f, &mut out)
        } else {
            self.pn += 1;
            ccmp::encrypt(&tk, 0, self.pn, &f, &mut out)
        };
        if let Ok(len) = sealed { out.truncate(len); self.hear(&out); }
    }

    fn eapol(&mut self, frame: &[u8]) { let payload = frame.to_vec(); self.data(&OWN, &BSSID, ETHERTYPE_EAPOL, &payload); }

    fn message_1(&mut self) {
        let mut m = [0u8; EAPOL_MAX];
        let Some(len) = self.authenticator.as_mut().and_then(|a| a.message_1(&mut m).ok()) else { return };
        self.expect = Expect::Message2;
        self.tries += 1;
        self.eapol(&m[..len]);
    }

    // What the access point does with a frame the station sent on its channel; whether it was acknowledged.
    fn station_sent(&mut self, f: &[u8]) -> bool {
        let Ok(h) = frame::header(f) else { return false };
        if h.kind == TYPE_MANAGEMENT {
            match h.subtype {
                frame::PROBE_REQUEST => { let r = self.announcement(frame::PROBE_RESPONSE, &h.a2); self.hear(&r); }
                frame::AUTHENTICATION if h.a1 == BSSID => {
                    let mut r = self.header(frame::AUTHENTICATION, &h.a2);
                    r.extend_from_slice(&[0, 0, 2, 0, 0, 0]);
                    self.hear(&r);
                }
                frame::ASSOCIATION_REQUEST if h.a1 == BSSID => {
                    mind::println!("[WLANSIM] OFFERED RATES{}", offered(f.get(h.len + 4..).unwrap_or(&[])));
                    let mut r = self.header(frame::ASSOCIATION_RESPONSE, &h.a2);
                    r.extend_from_slice(&[0x01, 0x04, 0, 0, 0x01, 0xC0, 1, 8, 0x82, 0x84, 0x8B, 0x96, 0x0C, 0x12, 0x18, 0x24, 50, 4, 0x30, 0x48, 0x60, 0x6C]);
                    self.hear(&r);
                    let anonce = random32();
                    self.authenticator = Authenticator::new(wpa::keys::pmk(PASSPHRASE, SSID).unwrap_or([0; 32]), BSSID, OWN, &dot11::scan::own_rsne(), anonce).ok();
                    self.associated = true;
                    self.tk = None;
                    self.tries = 0;
                    self.replay = Replay::default();
                    mind::println!("[WLANSIM] ASSOCIATED");
                    self.message_1();
                }
                frame::DEAUTHENTICATION | frame::DISASSOCIATION => { self.associated = false; self.tk = None; self.expect = Expect::Nothing; }
                _ => {}
            }
            return h.a1 == BSSID;
        }
        if h.flags & (TO_DS | FROM_DS) != TO_DS || h.a1 != BSSID || !self.associated { return false; }
        let mut plain = [0u8; FRAME_MAX];
        let body = if h.flags & PROTECTED != 0 {
            let Some(tk) = self.tk else { return true };
            let Ok((len, pn)) = ccmp::decrypt(&tk, f, &mut plain) else { return true };
            if self.replay.accept(h.tid.unwrap_or(0) as usize, pn).is_err() { return true; }
            &plain[24..len]
        } else {
            &f[24..]
        };
        if body.len() < 8 || body[..6] != SNAP { return true; }
        let ethertype = u16::from_be_bytes([body[6], body[7]]);
        let payload = body[8..].to_vec();
        if ethertype == ETHERTYPE_EAPOL { self.handshake(&payload); } else if self.tk.is_some() { self.network(&h.a2, ethertype, &payload); }
        true
    }

    fn handshake(&mut self, payload: &[u8]) {
        let Some(a) = self.authenticator.as_mut() else { return };
        let mut m = [0u8; EAPOL_MAX];
        match self.expect {
            Expect::Message2 => match a.message_2(payload) {
                Ok(()) => {
                    let Ok(len) = a.message_3(&self.gtk, [0; 8], &mut m) else { return };
                    self.expect = Expect::Message4;
                    let reply = m[..len].to_vec();
                    self.eapol(&reply);
                }
                // A wrong passphrase: message 1 again twice, then a deauthentication, reason 15 (handshake timeout).
                Err(_) => {
                    mind::println!("[WLANSIM] MESSAGE 2 MIC FAILED");
                    if self.tries < 3 { self.message_1(); return; }
                    let mut r = self.header(frame::DEAUTHENTICATION, &OWN);
                    r.extend_from_slice(&15u16.to_le_bytes());
                    self.hear(&r);
                    self.associated = false;
                    self.expect = Expect::Nothing;
                }
            },
            Expect::Message4 => if a.confirmation(payload).is_ok() {
                self.tk = a.ptk().map(|p| p.tk);
                self.expect = Expect::Nothing;
                mind::println!("[WLANSIM] HANDSHAKE DONE");
            },
            Expect::Nothing => {}
        }
    }

    // The network behind the access point: DHCP, ARP for the gateway, ICMP echo to it.
    fn network(&mut self, source: &Mac, ethertype: u16, p: &[u8]) {
        if ethertype == 0x0806 && p.len() >= 28 && p[6..8] == [0, 1] && p[24..28] == GATEWAY {
            let mut r = Vec::with_capacity(28);
            r.extend_from_slice(&[0, 1, 0x08, 0x00, 6, 4, 0, 2]);
            r.extend_from_slice(&BSSID);
            r.extend_from_slice(&GATEWAY);
            r.extend_from_slice(&p[8..18]);
            self.data(source, &BSSID, 0x0806, &r);
            return;
        }
        if ethertype != 0x0800 || p.len() < 20 || p[0] >> 4 != 4 { return; }
        let ihl = (p[0] & 0x0F) as usize * 4;
        let total = (u16::from_be_bytes([p[2], p[3]]) as usize).min(p.len());
        if total < ihl + 8 { return; }
        let (protocol, src, dst) = (p[9], [p[12], p[13], p[14], p[15]], [p[16], p[17], p[18], p[19]]);
        let body = &p[ihl..total];
        if protocol == 17 && u16::from_be_bytes([body[2], body[3]]) == 67 { self.dhcp(source, &body[8..]); }
        if protocol == 1 && dst == GATEWAY && body[0] == 8 {
            let mut reply = body.to_vec();
            reply[0] = 0;
            reply[2..4].copy_from_slice(&[0, 0]);
            let sum = checksum(&reply);
            reply[2..4].copy_from_slice(&sum.to_be_bytes());
            let packet = ipv4(1, GATEWAY, src, &reply);
            mind::println!("[WLANSIM] ECHO TO {}.{}.{}.{}", src[0], src[1], src[2], src[3]);
            self.data(source, &BSSID, 0x0800, &packet);
        }
    }

    fn dhcp(&mut self, source: &Mac, d: &[u8]) {
        if d.len() < 240 || d[0] != 1 || d[236..240] != [99, 130, 83, 99] { return; }
        let mut kind = 0;
        let mut at = 240;
        while at + 1 < d.len() && d[at] != 255 {
            if d[at] == 0 { at += 1; continue; }
            let len = d[at + 1] as usize;
            if d[at] == 53 && len == 1 && at + 2 < d.len() { kind = d[at + 2]; }
            at += 2 + len;
        }
        let answer = match kind { 1 => 2, 3 => 5, _ => return };
        let mut r = alloc::vec![0u8; 240];
        r[0] = 2; r[1] = 1; r[2] = 6;
        r[4..8].copy_from_slice(&d[4..8]);
        r[10..12].copy_from_slice(&d[10..12]);
        r[16..20].copy_from_slice(&LEASE);
        r[20..24].copy_from_slice(&GATEWAY);
        r[28..44].copy_from_slice(&d[28..44]);
        r[236..240].copy_from_slice(&[99, 130, 83, 99]);
        r.extend_from_slice(&[53, 1, answer, 54, 4]);
        r.extend_from_slice(&GATEWAY);
        r.extend_from_slice(&[51, 4, 0, 0, 0x0E, 0x10, 1, 4, 255, 255, 255, 0, 3, 4]);
        r.extend_from_slice(&GATEWAY);
        r.extend_from_slice(&[6, 4]);
        r.extend_from_slice(&GATEWAY);
        r.push(255);
        let mut udp = Vec::with_capacity(8 + r.len());
        udp.extend_from_slice(&67u16.to_be_bytes());
        udp.extend_from_slice(&68u16.to_be_bytes());
        udp.extend_from_slice(&((8 + r.len()) as u16).to_be_bytes());
        udp.extend_from_slice(&[0, 0]); // no UDP checksum (allowed over IPv4)
        udp.extend_from_slice(&r);
        let packet = ipv4(17, GATEWAY, [255; 4], &udp);
        mind::println!("[WLANSIM] DHCP {} {}.{}.{}.{}", if answer == 2 { "OFFER" } else { "ACK" }, LEASE[0], LEASE[1], LEASE[2], LEASE[3]);
        let _ = source;
        self.data(&BROADCAST, &BSSID, 0x0800, &packet);
    }

    fn tick(&mut self, now: u64) {
        if now >= self.beacon_at {
            self.beacon_at = now + BEACON_MS;
            if self.channel == CHANNEL { let b = self.announcement(frame::BEACON, &BROADCAST); self.hear(&b); }
        }
    }
}

fn checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    for pair in data.chunks(2) { sum += u16::from_be_bytes([pair[0], *pair.get(1).unwrap_or(&0)]) as u32; }
    while sum > 0xFFFF { sum = (sum & 0xFFFF) + (sum >> 16); }
    !(sum as u16)
}

fn ipv4(protocol: u8, source: [u8; 4], destination: [u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut p = Vec::with_capacity(20 + payload.len());
    p.extend_from_slice(&[0x45, 0]);
    p.extend_from_slice(&((20 + payload.len()) as u16).to_be_bytes());
    p.extend_from_slice(&[0, 0, 0, 0, 64, protocol, 0, 0]);
    p.extend_from_slice(&source);
    p.extend_from_slice(&destination);
    let sum = checksum(&p);
    p[10..12].copy_from_slice(&sum.to_be_bytes());
    p.extend_from_slice(payload);
    p
}

// The rates of an association request's elements, in Mb/s: " 1 2 5.5 11".
fn offered(elements: &[u8]) -> String {
    let mut text = String::new();
    for (id, body) in frame::Elements::new(elements) {
        if id != frame::RATES && id != frame::EXTENDED_RATES { continue; }
        for &r in body {
            let r = r & 0x7F;
            let _ = if r % 2 == 1 { write!(text, " {}.5", r / 2) } else { write!(text, " {}", r / 2) };
        }
    }
    text
}

// An ANonce: RDRAND where there is one; the uptime otherwise (a test access point, not a security boundary).
fn random32() -> [u8; 32] {
    let mut r = [0u8; 32];
    if !mind::random::fill(&mut r) { r[..8].copy_from_slice(&(mind::time::uptime_ms() as u64).to_le_bytes()); }
    r
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut sim = Sim { channel: 0, filter: wlanhw::Filter::default(), queue: Vec::new(), waiters: Vec::new(), beacon_at: 0, sequence: 0,
                        associated: false, authenticator: None, expect: Expect::Nothing, tries: 0, tk: None, replay: Replay::default(), pn: 0,
                        gtk: Gtk { id: 1, key: random32()[..16].try_into().unwrap_or([7; 16]) }, group_pn: 0, refused: false, limits: None };
    mind::println!("[WLANSIM] READY: {} ON CHANNEL {}", core::str::from_utf8(SSID).unwrap_or("?"), CHANNEL);
    let mut scratch = [0u8; wlanhw::REQUEST_MAX];
    let mut out = [0u8; FRAME_MAX + RADIO_HEADER];
    loop {
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, POLL_MS);
        sim.tick(mind::time::uptime_ms() as u64);
        if !sim.queue.is_empty() { for call in sim.waiters.drain(..) { let _ = wlanhw::reply_wait(call, sim.queue.len() as u32); } }
        let Ok(request) = received else { continue };
        let (request, call) = match wlanhw::decode(&request, RECEIVED_CAP, &mut scratch) {
            Ok(decoded) => decoded,
            Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; }
        };
        let _ = match request {
            wlanhw::Request::Info => wlanhw::reply_info(call, Ok(&sim.info())),
            wlanhw::Request::SetChannel { channel } if sim.channels().as_slice().contains(&channel) => { sim.channel = channel; wlanhw::reply_set_channel(call, Ok(())) }
            wlanhw::Request::SetChannel { .. } => wlanhw::reply_set_channel(call, Err(wlanhw::Error::Invalid)),
            wlanhw::Request::SetFilter { filter } => { sim.filter = filter; wlanhw::reply_set_filter(call, Ok(())) }
            wlanhw::Request::Send { rate, .. } if !RATES.contains(&rate) => {
                if !sim.refused { mind::println!("[WLANSIM] REFUSED A FRAME AT RATE {} (500 KB/S UNITS)", rate); sim.refused = true; }
                wlanhw::reply_send(call, Err(wlanhw::Error::Invalid))
            }
            wlanhw::Request::Send { frame: f, .. } if !sim.may_send(f) => {
                mind::println!("[WLANSIM] REFUSED A FRAME ON CHANNEL {}: THE COUNTRY'S LIMITS DO NOT LET IT GO OUT", sim.channel);
                wlanhw::reply_send(call, Err(wlanhw::Error::Invalid))
            }
            wlanhw::Request::Send { frame: f, attempts, .. } => {
                let acked = sim.channel == CHANNEL && sim.station_sent(f);
                wlanhw::reply_send(call, Ok(&wlanhw::Sent { acked, attempts: if acked { 1 } else { attempts.max(1) } }))
            }
            wlanhw::Request::Receive => {
                if sim.queue.is_empty() { wlanhw::reply_receive(call, Err(wlanhw::Error::Empty)) } else {
                    let f = sim.queue.remove(0);
                    out[..f.len()].copy_from_slice(&f);
                    wlanhw::reply_receive(call, Ok(&out[..f.len()]))
                }
            }
            wlanhw::Request::Wait => {
                if !sim.queue.is_empty() { wlanhw::reply_wait(call, sim.queue.len() as u32) } else {
                    let mut call = call;
                    if call.defer().is_ok() && sim.waiters.len() < 4 { sim.waiters.push(call); Ok(()) } else { wlanhw::reply_wait(call, 0) }
                }
            }
            // No shared ring: frames go by `receive`.
            wlanhw::Request::Attach { ring } => { let _ = mind::ipc::drop_cap(ring); wlanhw::reply_attach(call, Err(wlanhw::Error::Invalid)) }
            wlanhw::Request::Kick => wlanhw::reply_kick(call, Err(wlanhw::Error::Invalid)),
            // 1.2: the country's limits on 1 to 13, each channel once; it answers the channels they leave.
            wlanhw::Request::SetCountry { code, limits, .. } => {
                let list = limits.as_slice();
                let twice = list.iter().enumerate().any(|(i, l)| list[..i].iter().any(|m| m.channel == l.channel));
                let named = code.as_str().len() == 2 && code.as_str().bytes().all(|b| b.is_ascii_uppercase());
                if twice || !named || list.iter().any(|l| !(1..=13).contains(&l.channel)) { wlanhw::reply_set_country(call, Err(wlanhw::Error::Invalid)) } else {
                    sim.limits = Some(list.iter().map(|l| (l.channel, l.listen)).collect());
                    mind::println!("[WLANSIM] COUNTRY {}: {} CHANNELS, {} TO LISTEN ON ONLY", code.as_str(), list.len(), list.iter().filter(|l| l.listen).count());
                    wlanhw::reply_set_country(call, Ok(&sim.info()))
                }
            }
        };
    }
}
