#![no_std]
#![no_main]
// The 802.11 station (550-NET-0018): drives `dot11` over a radio's idl/wlanhw.wit client (slot 2) and serves its own
// endpoint, idl/net.wit to the network stack (a client without BADGE_WIFI) and idl/wifi.wit to clients with BADGE_WIFI.
// It holds the radio and nothing else that can act (MC-2.3); a passphrase becomes a PMK at once and is not kept. A
// network to remember goes to `keystore` (slot 3, BADGE_KEY_STATION, 550-NET-0024), which keeps its PMK and answers
// each handshake's keys: the station holds no PMK of a remembered network (MC-11.9). The country (wifi.wit 1.2,
// 550-NET-0035 stage B) comes from the regulatory catalogue (`regdb`): the one the user chose and `keystore` keeps;
// with none, the first scan sends nothing and the station runs under the country the access points announce, or `00`
// (the maintainer's decision of 2026-11-04: it must work, not go quiet). The radio is given the country's limits.
// At start and after a network is lost it joins a remembered one heard by itself, trying again and logging each try,
// until `wifi leave` (550-NET-0041): a boot service, so every release through the link brings it. A network a client
// joined that `keystore` does not keep is held here, its PMK too, and joined again the same way, after a loss or a
// radio's restart, until `wifi leave` (801-NET-0084). A key joins a WPA2 access point alone, never an open one.
use dot11::catalogue::{self, Source};
use dot11::scan::{Bss, Security};
use dot11::station::{Allowed, Out, Reason, Secret, State, Station, ETHERNET_MAX, FRAME_MAX};
use mind::abi::BootInfo;
use mind::deadline::Deadline;
use mind::idl::codec::List;
use mind::idl::{keystore, net, wifi, wire, wlanhw};
use mind::ipc::Endpoint;
use mind::network::{BADGE_COUNTERS, BADGE_WIFI, BADGE_WIFI_CHANGE};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
mod auto;
mod service_turn;

extern crate alloc;
use alloc::vec::Vec;

const RADIO: Endpoint = Endpoint(2);
const KEYSTORE: Endpoint = Endpoint(3);
const RECEIVED_CAP: usize = 9;
const POLL_MS: u32 = 10;
// Received Ethernet frames kept for the stack and parked `wait` calls.
const QUEUE: usize = 32;
const WAITERS: usize = 4;
// Tries of a frame the station sends: the most a radio takes, to outlast bursts of interference (550-NET-0043; 7 lost
// about 20% of the MacBook Pro's frames in a burst, 550-NET-0042).
const ATTEMPTS: u8 = 15;
const RADIO_HEADER: usize = 8;
// The radio is asked with a deadline, and again every RADIO_MS until it answers; meanwhile the station serves its
// clients as a card without a radio (550-NET-0030), so the stack does not wait on it.
const RADIO_MS: u32 = 2000;
// How often, while not joined, the station asks a radio it tuned whether it still has a channel; also before each tune (550-NET-0061).
const RESTART_CHECK_MS: u64 = 10_000;
// A radio that answered none of the station's calls for this long is asked again as a new one, and the station starts
// anew over it (801-NET-0083); and the longest the automatic join may go without a try while it is on and a network is
// remembered: then the station starts anew and the log says why.
const RADIO_SILENT_MS: u64 = 10_000;
const STALL_MS: u64 = auto::LATER_MS + 70_000;

// The radio's last answer, and whether a call found its driver ended (801-NET-0083): each radio call goes through `radio_call`.
static ANSWERED: AtomicU64 = AtomicU64::new(0);
static DRIVER_ENDED: AtomicBool = AtomicBool::new(false);
// The PID that answered the station's last radio call, and the one before it when another answered (801-NET-0085):
// `init` starts a failed driver again behind the same endpoint, so another PID is a fresh driver, whatever it reports.
static RADIO_PID: AtomicU64 = AtomicU64::new(0);
static FRESH: AtomicU64 = AtomicU64::new(0);
fn radio_call<T>(deadline: Deadline, call: impl FnOnce() -> mind::sys::Result<T>) -> mind::sys::Result<T> {
    let result = deadline.call(call);
    match &result {
        Ok(_) => {
            ANSWERED.store(mind::time::uptime_ms() as u64, Ordering::Relaxed);
            let pid = mind::idl::wire::replied_by();
            let was = RADIO_PID.swap(pid, Ordering::Relaxed);
            if was != 0 && pid != 0 && pid != was { FRESH.store(was, Ordering::Relaxed); }
        }
        Err(mind::sys::Error::Peer) => DRIVER_ENDED.store(true, Ordering::Relaxed),
        Err(_) => {}
    }
    result
}

// Why the station set the radio up again (801-NET-0083): no channel set though it was tuned (550-NET-0061), a channel
// other than the one it took, a call that found its driver ended, another PID answering (801-NET-0085), or the
// automatic join stalled.
#[derive(Clone, Copy)]
enum Again { NoChannel, Moved(u8, u8), Ended, Fresh(u64, u64), Stalled }

fn mac(value: u64) -> [u8; 6] { let b = value.to_be_bytes(); [b[2], b[3], b[4], b[5], b[6], b[7]] }
fn mac64(m: &[u8; 6]) -> u64 { m.iter().fold(0, |v, &b| v << 8 | b as u64) }

// Overwrites a copy of key material.
fn wipe(bytes: &mut [u8]) { for byte in bytes.iter_mut() { unsafe { core::ptr::write_volatile(byte, 0) }; } }

// A remembered network's keys, one handshake at a time, from `keystore` (its SSID).
struct Kept([u8; 32], usize, Deadline);

impl wpa::handshake::Pairwise for Kept {
    fn ptk(&mut self, ap: &[u8; 6], own: &[u8; 6], anonce: &[u8; 32], snonce: &[u8; 32]) -> Option<wpa::keys::Ptk> {
        let mut keys = match self.2.call(|| keystore::wifi_keys(KEYSTORE, &self.0[..self.1], mac64(ap), mac64(own), anonce, snonce)) { Ok(Ok(keys)) => keys, _ => return None };
        let k = keys.as_slice();
        let ptk = (k.len() == 48).then(|| wpa::keys::Ptk { kck: k[..16].try_into().unwrap(), kek: k[16..32].try_into().unwrap(), tk: k[32..].try_into().unwrap() });
        unsafe { core::ptr::write_volatile(&mut keys, List::default()) };
        ptk
    }
}

// A network a client joined that `keystore` does not keep (801-NET-0084): its name and its PMK (none: an open one),
// wiped when dropped.
struct Held { ssid: [u8; 32], len: usize, pmk: Option<[u8; 32]> }

impl Held {
    // None for a kept network: keystore holds its key.
    fn new(ssid: &[u8], secret: Option<Secret>) -> Option<Self> {
        let pmk = match secret { Some(Secret::Kept) => return None, Some(Secret::Pmk(pmk)) => Some(pmk), None => None };
        let len = ssid.len().min(32);
        let mut held = Held { ssid: [0; 32], len, pmk };
        held.ssid[..len].copy_from_slice(&ssid[..len]);
        Some(held)
    }
    fn ssid(&self) -> &[u8] { &self.ssid[..self.len] }
    fn secret(&self) -> Option<Secret> { self.pmk.map(Secret::Pmk) }
}

impl Drop for Held {
    fn drop(&mut self) { if let Some(pmk) = self.pmk.as_mut() { wipe(pmk); } }
}

// The names `keystore` keeps networks for, or why it gave none (none without its client).
fn remembered(deadline: Deadline) -> Result<List<keystore::WifiNetwork, 16>, &'static str> {
    match deadline.call(|| keystore::wifi_networks(KEYSTORE)) {
        Ok(Ok(list)) => Ok(list),
        Ok(Err(_)) => Err("REFUSED"),
        Err(mind::sys::Error::Other(mind::abi::ERR_TIMEOUT)) => Err("DID NOT ANSWER"),
        Err(_) => Err("NOT REACHED"),
    }
}

// Received Ethernet frames for the stack, oldest first (on the heap: 48 KiB would not fit the 64 KiB stack).
struct Queue { frames: Vec<u8>, lens: [usize; QUEUE], head: usize, count: usize }

impl Queue {
    fn push(&mut self, frame: &[u8]) -> bool {
        if self.count == QUEUE { return false; }
        let at = (self.head + self.count) % QUEUE;
        self.frames[at * ETHERNET_MAX..at * ETHERNET_MAX + frame.len()].copy_from_slice(frame);
        self.lens[at] = frame.len();
        self.count += 1;
        true
    }
    fn pop(&mut self, out: &mut [u8; ETHERNET_MAX]) -> Option<usize> {
        if self.count == 0 { return None; }
        let len = self.lens[self.head];
        out[..len].copy_from_slice(&self.frames[self.head * ETHERNET_MAX..self.head * ETHERNET_MAX + len]);
        self.head = (self.head + 1) % QUEUE;
        self.count -= 1;
        Some(len)
    }
}

// An automatic try under way: its scan, then its join of a remembered network (its name), and its number.
#[derive(Clone, Copy)]
enum Try { Scanning(u32), Joining([u8; 32], usize, u32) }

struct Wlan {
    station: Station,
    deadline: Deadline,
    // Whether the radio has answered `info`: until then the station has no channels and answers no-device.
    radio: bool,
    own: [u8; 6],
    tx: [u8; FRAME_MAX],
    rx: [u8; ETHERNET_MAX],
    radio_frame: [u8; FRAME_MAX + RADIO_HEADER],
    queue: Queue,
    counters: net::Counters,
    waiters: [Option<wire::Call>; WAITERS],
    scans: [Option<wire::Call>; WAITERS],
    auto: auto::Auto,
    trying: Option<Try>,
    // The network last joined, tried first after it is lost.
    previous: ([u8; 32], usize),
    // The radio's channels, as its `info` gave them.
    channels: ([u8; catalogue::CHANNELS], usize),
    // The country (550-NET-0035): the user's choice and whether `keystore` keeps it, the one the station runs under and
    // why, whether the radio confirmed or refused its limits, and whether the first scan is still listening for one.
    chosen: Option<[u8; 2]>,
    kept: bool,
    running: Option<([u8; 2], Source)>,
    confirmed: bool,
    refused: bool,
    first: bool,
    // A client that only reads (BADGE_WIFI alone), minted once from the station's endpoint for `reader` (550-NET-0058).
    reader: Option<usize>,
    // Whether the radio took a channel since it answered, the last restart check, and a restart found before a tune (550-NET-0061).
    tuned: bool,
    checked: u64,
    restarted: Option<wlanhw::Info>,
    // The channel the radio last took, a reason to set it up again that needs no word from its `info`, whether it ever
    // answered, when the automatic join last began a try or the station joined, whether nothing is remembered or held, and
    // whether keystore's last answer failed (801-NET-0083).
    tuned_to: u8,
    again: Option<Again>,
    had_radio: bool,
    try_began: u64,
    nothing_kept: bool,
    keystore_failed: bool,
    // The network a client joined this boot that keystore does not keep, and a client's join under way, held once it
    // joins (801-NET-0084).
    held: Option<Held>,
    offered: Option<Option<Held>>,
}

impl Wlan {
    // The radio answered: its address, rates and own channels, over which each country is planned (550-NET-0059).
    fn radio_up(&mut self, info: &wlanhw::Info) {
        FRESH.store(0, Ordering::Relaxed);
        self.station_up(info);
        self.country_start();
    }

    // A station anew over the radio's own channels.
    fn station_up(&mut self, info: &wlanhw::Info) {
        let own = mac(info.mac);
        let channels = info.supported.as_slice();
        // Without OFDM on 2.4 GHz (a 1.0 radio says nothing of it) the station keeps to 802.11b's rates there (550-NET-0028).
        let ofdm = info.bands & mind::network::BAND_2GHZ_OFDM != 0;
        mind::println!("[WLAN] RADIO {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}, {} CHANNELS OF ITS OWN, {} UNDER ITS LIMITS NOW, 2.4 GHZ {}", own[0], own[1], own[2], own[3], own[4], own[5],
                       channels.len(), info.channels.as_slice().len(), if ofdm { "802.11G" } else { "802.11B" });
        self.station = Station::new(own, channels, ofdm);
        self.own = own;
        self.radio = true;
        let n = channels.len().min(catalogue::CHANNELS);
        self.channels.0[..n].copy_from_slice(&channels[..n]);
        self.channels.1 = n;
        self.tuned = false;
        self.tuned_to = 0;
        self.had_radio = true;
    }

    // A radio that restarted (550-NET-0061): `init` starts a failed driver again behind the same endpoint, and the fresh
    // one has no channel and no country, so it sends nothing. Its `info` says so: no channel set, though the station tuned
    // it; or a channel other than the one it took (a fresh driver that tuned itself, 801-NET-0083). Checked joined or
    // not. A call that found the driver ended, or a stalled automatic join, sets it up again without that word. The
    // station starts anew, the radio gets the country the station ran under, and the automatic join joins again.
    fn radio_restarted(&mut self, now: u64) {
        let asked = self.restarted.is_some();
        let (info, why) = match self.restarted.take() {
            Some(info) => (info, Again::NoChannel),
            None => {
                if !self.radio || (!self.tuned && self.again.is_none()) { return; }
                if self.again.is_none() && now.saturating_sub(self.checked) < RESTART_CHECK_MS { return; }
                self.checked = now;
                let Ok(Ok(info)) = radio_call(self.deadline, || wlanhw::info(RADIO)) else { return };
                let why = match self.again.take() {
                    _ if self.tuned && info.channel == 0 => Again::NoChannel,
                    _ if self.tuned && self.tuned_to != 0 && info.channel != self.tuned_to => Again::Moved(info.channel, self.tuned_to),
                    Some(why) => why,
                    None => return,
                };
                (info, why)
            }
        };
        let running = self.running;
        let code = running.map(|(code, _)| code);
        let country = code.as_ref().and_then(|c| core::str::from_utf8(c).ok()).unwrap_or("CHOSEN");
        let again = " AGAIN";
        match why {
            Again::NoChannel => mind::println!("[WLAN] THE RADIO RESTARTED (IT HAS NO CHANNEL SET{}): ITS COUNTRY {}{}", if asked { ", ASKED BEFORE A TUNE" } else { "" }, country, again),
            Again::Moved(now_on, was) => mind::println!("[WLAN] THE RADIO RESTARTED (ON CHANNEL {}, NOT THE {} IT TOOK): ITS COUNTRY {}{}", now_on, was, country, again),
            Again::Ended => mind::println!("[WLAN] THE RADIO RESTARTED (A CALL FOUND ITS DRIVER ENDED): ITS COUNTRY {}{}", country, again),
            Again::Fresh(was, now_pid) => mind::println!("[WLAN] THE RADIO RESTARTED (ANSWERED BY PID {}, NOT {}): ITS COUNTRY {}{}", now_pid, was, country, again),
            Again::Stalled => mind::println!("[WLAN] THE AUTOMATIC JOIN BEGAN NO TRY FOR {} S: THE STATION STARTS ANEW, ITS COUNTRY {}{}", STALL_MS / 1000, country, again),
        }
        self.station_up(&info);
        self.first = false;
        match running { Some((code, source)) => self.run_under(code, source), None => self.country_start() }
        // The station's calls to the fresh radio came from the new station: one restart for it.
        FRESH.store(0, Ordering::Relaxed);
        // A try under way was the old station's: the next one begins at once.
        if let Some(Try::Scanning(number) | Try::Joining(_, _, number)) = self.trying.take() {
            mind::println!("[WLAN] AUTO-JOIN: TRY {} ENDED BY THE RADIO'S RESTART, AGAIN AT ONCE", number);
        }
        self.auto.next = now;
        self.try_began = now;
        // A client's scan the old station took is scanned again by the new one.
        if self.scans.iter().any(Option::is_some) && !self.first {
            let mut out = Out::default();
            let _ = self.station.scan(now, &mut out, &mut self.tx);
            self.apply(out);
        }
    }

    // 801-NET-0083: each turn, before the station's work. A call that found the radio's driver ended sets the radio up
    // again; a radio that answered nothing for RADIO_SILENT_MS is asked again as a new one (the main loop's ask, which
    // starts the station anew); an automatic join that began no try for STALL_MS while a network is remembered or held
    // starts the station anew. Each says so in the log: the station never waits without a word.
    fn watch(&mut self, now: u64) -> bool {
        let ended = DRIVER_ENDED.swap(false, Ordering::Relaxed);
        if !self.radio { return false; }
        if ended && self.again.is_none() { self.again = Some(Again::Ended); }
        let fresh = FRESH.load(Ordering::Relaxed);
        if fresh != 0 && self.again.is_none() { self.again = Some(Again::Fresh(fresh, RADIO_PID.load(Ordering::Relaxed))); }
        let answered = ANSWERED.load(Ordering::Relaxed);
        if now.saturating_sub(answered) >= RADIO_SILENT_MS {
            mind::println!("[WLAN] THE RADIO HAS NOT ANSWERED FOR {} S: ASKED AGAIN AS A NEW RADIO, THE STATION STARTS ANEW", now.saturating_sub(answered) / 1000);
            self.radio = false;
            self.restarted = None;
            self.again = None;
            if let Some(Try::Scanning(number) | Try::Joining(_, _, number)) = self.trying.take() {
                mind::println!("[WLAN] AUTO-JOIN: TRY {} ENDED: THE RADIO DID NOT ANSWER", number);
            }
            self.auto.next = now;
            self.try_began = now;
            return true;
        }
        let joined = matches!(self.station.state(), State::Joined);
        if self.auto.on && !self.nothing_kept && !joined && now.saturating_sub(self.try_began) >= STALL_MS && self.again.is_none() {
            self.again = Some(Again::Stalled);
        }
        false
    }

    // Before a tune of a radio the station tuned: whether it restarted, as a tune would set a channel and hide it.
    fn restarted_before_tune(&mut self) -> bool {
        if self.restarted.is_some() { return true; }
        if !self.tuned { return false; }
        let Ok(Ok(info)) = radio_call(self.deadline, || wlanhw::info(RADIO)) else { return false };
        if info.channel != 0 { return false; }
        self.restarted = Some(info);
        true
    }

    fn radio_channels(&self) -> &[u8] { &self.channels.0[..self.channels.1] }

    // At the radio's first answer: the country `keystore` keeps, or the first scan, which only listens.
    fn country_start(&mut self) {
        self.chosen = match self.deadline.call(|| keystore::wifi_country(KEYSTORE)) {
            Ok(Ok(code)) => code.as_str().as_bytes().try_into().ok(),
            _ => None,
        };
        self.kept = self.chosen.is_some();
        if let Some(code) = self.chosen.filter(|&c| regdb::select(c).is_ok()) { self.run_under(code, Source::Chosen); return; }
        let plan = catalogue::listening(self.radio_channels());
        let mut out = Out::default();
        self.station.restrict(plan.allowed(), &mut out, &mut self.tx);
        self.apply(out);
        let mut out = Out::default();
        let started = self.station.scan(mind::time::uptime_ms() as u64, &mut out, &mut self.tx);
        self.apply(out);
        self.first = started.is_ok();
        mind::println!("[WLAN] NO COUNTRY CHOSEN: THE FIRST SCAN SENDS NOTHING AND LISTENS FOR THE COUNTRY THE ACCESS POINTS ANNOUNCE");
        if !self.first { let (code, source) = catalogue::decide(None, None); self.run_under(code, source); }
    }

    // A scan ended: the first decides the country; a later one, while the station is not joined, may move a country
    // the user did not choose to another one the access points announce.
    fn scanned(&mut self) {
        let announced = self.station.networks().announced();
        if core::mem::replace(&mut self.first, false) {
            let (code, source) = catalogue::decide(None, announced);
            self.run_under_first(code, source);
        } else if let Some((code, source)) = self.running.and_then(|r| catalogue::moves(r, announced)) {
            if !matches!(self.station.state(), State::Joining | State::Joined) { self.run_under(code, source); }
        }
    }

    // The first country, after the scan that only listened: the networks it heard stay (`relax`), so a network listed
    // then is joined with no new scan.
    fn run_under_first(&mut self, code: [u8; 2], source: Source) { self.run_country(code, source, true) }

    // Runs the station under country `code`: its channels restricted, the radio given the limits, and the log says why.
    fn run_under(&mut self, code: [u8; 2], source: Source) { self.run_country(code, source, false) }

    fn run_country(&mut self, code: [u8; 2], source: Source, first: bool) {
        let Some(country) = catalogue::entry(code) else { return };
        let plan = catalogue::plan(country, self.radio_channels());
        let mut limits = List::<wlanhw::Limit, 64>::default();
        for &(channel, eirp, listen) in plan.limits() { limits.push(wlanhw::Limit { channel, eirp, listen }); }
        let dfs = match country.dfs_region() { regdb::DfsRegion::Unspecified => 0, regdb::DfsRegion::Fcc => 1, regdb::DfsRegion::Etsi => 2, regdb::DfsRegion::Jp => 3 };
        let text = core::str::from_utf8(&code).unwrap_or("00");
        let answer = radio_call(self.deadline, || wlanhw::set_country(RADIO, text, dfs, limits.as_slice()));
        (self.confirmed, self.refused) = match answer { Ok(Ok(_)) => (true, false), Ok(Err(wlanhw::Error::Failed)) => (false, true), _ => (false, false) };
        // A radio that refuses the limits sends nothing: the station only listens, and does not fall back.
        let allowed = if self.refused { catalogue::listening(self.radio_channels()) } else { plan };
        if first { self.station.relax(allowed.allowed()); } else {
            let mut out = Out::default();
            self.station.restrict(allowed.allowed(), &mut out, &mut self.tx);
            self.apply(out);
        }
        self.running = Some((code, source));
        let (send, listen) = plan.counts();
        let why = match source {
            Source::Chosen => alloc::format!("CHOSEN BY THE USER{}", if self.kept { ", KEPT BY KEYSTORE" } else { "" }),
            Source::Detected { announced, heard } => alloc::format!("NONE CHOSEN; ANNOUNCED BY {} OF THE {} ACCESS POINTS THAT NAMED A COUNTRY", announced, heard),
            Source::World => alloc::format!("NONE CHOSEN, NONE ANNOUNCED: THE RULES EVERY COUNTRY ALLOWS"),
        };
        let radio = if self.refused { "THE RADIO REFUSED ITS LIMITS: LISTENING ONLY" } else if self.confirmed { "THE RADIO APPLIED ITS LIMITS" } else { "THE RADIO DID NOT CONFIRM: THE STATION KEEPS TO THE CHANNELS" };
        mind::println!("[WLAN] COUNTRY {} ({}): {} CHANNELS TO SEND ON, {} TO LISTEN ON; {}", text, why, send, listen, radio);
    }

    fn country_status(&self) -> wifi::CountryStatus {
        let text = |code: Option<[u8; 2]>| code.and_then(|c| core::str::from_utf8(&c).ok().and_then(wire_text)).unwrap_or_default();
        let state = match (self.running, self.refused) { (None, _) => wifi::CountryState::None, (Some(_), true) => wifi::CountryState::Failed, _ => wifi::CountryState::Applied };
        wifi::CountryStatus { asked: text(self.chosen), applied: text(self.running.map(|r| r.0)), state, radio: self.confirmed, kept: self.kept,
                              catalogue: mind::idl::codec::Text::new(&regdb::SOURCE_SHA256[..16]).unwrap_or_default() }
    }

    fn country_origin(&self) -> wifi::CountryOrigin {
        match self.running.map(|r| r.1) {
            None => wifi::CountryOrigin::default(),
            Some(Source::Chosen) => wifi::CountryOrigin { origin: wifi::Origin::Chosen, announced: 0, heard: 0 },
            Some(Source::Detected { announced, heard }) => wifi::CountryOrigin { origin: wifi::Origin::Detected, announced, heard },
            Some(Source::World) => wifi::CountryOrigin { origin: wifi::Origin::World, announced: 0, heard: 0 },
        }
    }

    // A page of the catalogue, `00` left out, each country with its channels to send and listen on.
    fn countries(&self, from: u16) -> wifi::Countries {
        let all = regdb::countries().iter().filter(|c| !c.is_world());
        let total = all.clone().count().min(u16::MAX as usize) as u16;
        let mut page = List::default();
        for country in all.skip(from as usize).take(32) {
            let (send, listen) = catalogue::plan(country, self.radio_channels()).counts();
            let code = country.alpha2();
            page.push(wifi::Country { code: core::str::from_utf8(&code).ok().and_then(wire_text).unwrap_or_default(), send, listen });
        }
        wifi::Countries { total, page }
    }

    // The radio's channels under `code` (`00` too).
    fn channels_under(&self, code: &str) -> Result<List<wifi::Channel, 64>, wifi::Error> {
        let code = code_of(code, true)?;
        let country = catalogue::entry(code).ok_or(wifi::Error::NotFound)?;
        if !self.radio { return Err(wifi::Error::NoDevice); }
        let plan = catalogue::plan(country, self.radio_channels());
        let mut list = List::default();
        for &(number, allowed) in plan.allowed() {
            let eirp = plan.limits().iter().find(|l| l.0 == number).map_or(0, |l| l.1);
            let standing = match allowed { Allowed::Send => wifi::Standing::Send, Allowed::Listen => wifi::Standing::Listen, Allowed::Nothing => wifi::Standing::Barred };
            list.push(wifi::Channel { number, standing, eirp });
        }
        Ok(list)
    }

    // The user's country: the station leaves the network and drops the scan (`restrict`), runs under it, and `keystore`
    // keeps it.
    fn set_country(&mut self, code: &str) -> Result<wifi::CountryStatus, wifi::Error> {
        let code = code_of(code, false)?;
        regdb::select(code).map_err(|_| wifi::Error::NotFound)?;
        if !self.radio { return Err(wifi::Error::NoDevice); }
        self.first = false;
        self.chosen = Some(code);
        let text = core::str::from_utf8(&code).unwrap_or("");
        self.kept = matches!(self.deadline.call(|| keystore::wifi_keep_country(KEYSTORE, text)), Ok(Ok(())));
        self.run_under(code, Source::Chosen);
        Ok(self.country_status())
    }

    // What the station asked of the radio: tune, filter, send; a received Ethernet frame for the stack.
    fn apply(&mut self, out: Out) {
        // A radio found restarted gets nothing of the old station's: the next turn starts a new one (550-NET-0061), also
        // once another PID answered (801-NET-0085), so the old station sends nothing to a fresh driver it never tuned.
        let restarted = self.restarted.is_some() || (out.channel.is_some() && self.restarted_before_tune());
        if restarted || FRESH.load(Ordering::Relaxed) != 0 { return; }
        if let Some(channel) = out.channel {
            match radio_call(self.deadline, || wlanhw::set_channel(RADIO, channel)) {
                Ok(Ok(())) => { self.tuned = true; self.tuned_to = channel; }
                _ => {}
            }
        }
        if let Some(f) = out.filter {
            let _ = radio_call(self.deadline, || wlanhw::set_filter(RADIO, &wlanhw::Filter { all_beacons: f.all_beacons, bssid: mac64(&f.bssid), own: 0 }));
        }
        if let Some((len, rate)) = out.send {
            let unicast_data = self.tx[0] & 0x0C == 0x08 && self.tx[4] & 1 == 0;
            match radio_call(self.deadline, || wlanhw::send(RADIO, &self.tx[..len], rate, ATTEMPTS)) {
                Ok(Ok(sent)) => { if unicast_data { self.station.sent(sent.acked); } }
                _ => if unicast_data { self.station.sent(false) },
            }
        }
        if let Some(len) = out.deliver {
            self.counters.received += 1;
            if !self.queue.push(&self.rx[..len]) { self.counters.dropped += 1; }
        }
    }

    // Complete a started frame and its actions under the existing aggregate radio deadline.
    fn pump_frame(&mut self) -> bool {
        let len = match radio_call(self.deadline, || wlanhw::receive(RADIO, &mut self.radio_frame)) { Ok(Ok(len)) => len.min(self.radio_frame.len()), _ => return false };
        if len < RADIO_HEADER { return true; }
        let header = &self.radio_frame[..RADIO_HEADER];
        let frame_len = u16::from_le_bytes([header[0], header[1]]) as usize;
        let (weakness, channel) = (header[2], header[3]);
        if RADIO_HEADER + frame_len > len { return true; }
        let mut frame = [0u8; FRAME_MAX];
        frame[..frame_len].copy_from_slice(&self.radio_frame[RADIO_HEADER..RADIO_HEADER + frame_len]);
        // A fresh SNonce is drawn only for data frames, where message 1 may be; without RDRAND they are dropped.
        let mut random = [0u8; 32];
        if frame_len > 0 && frame[0] & 0x0C == 0x08 && !mind::random::fill(&mut random) { return true; }
        let mut out = Out::default();
        let mut kept = Kept([0; 32], 0, self.deadline);
        if let Some(bss) = self.station.network() { kept.1 = bss.ssid().len(); kept.0[..kept.1].copy_from_slice(bss.ssid()); }
        let now = mind::time::uptime_ms() as u64;
        let _ = self.station.receive(&frame[..frame_len], channel, weakness, now, &random, &mut kept, &mut out, &mut self.tx, &mut self.rx);
        self.apply(out);
        true
    }

    // Yield between radio frames; the time slice never shortens a started frame's radio/key-store calls.
    fn pump(&mut self) {
        service_turn::receive_batch(self, || mind::time::monotonic_ns() / 1_000_000, |wlan| wlan.pump_frame());
        // Do not advance the station and produce another action when no aggregate radio time remains.
        if self.deadline.remaining(mind::time::monotonic_ns() / 1_000_000).is_none() { return; }
        let now = mind::time::uptime_ms() as u64;
        let mut out = Out::default();
        let _ = self.station.tick(now, &mut out, &mut self.tx);
        self.apply(out);
        if self.queue.count > 0 {
            for waiter in self.waiters.iter_mut() { if let Some(call) = waiter.take() { let _ = net::reply_wait(call, self.queue.count as u32); } }
        }
        if self.station.state() != State::Scanning {
            for scan in 0..WAITERS { if let Some(call) = self.scans[scan].take() { let _ = wifi::reply_scan(call, Ok(self.networks().as_slice())); } }
        }
    }

    fn networks(&self) -> List<wifi::Network, 32> {
        let kept = remembered(self.deadline).unwrap_or_default();
        let mut list = List::default();
        for bss in self.station.networks().iter() {
            let mut n = network(bss);
            n.remembered = kept.as_slice().iter().any(|k| k.ssid.as_slice() == bss.ssid());
            list.push(n);
        }
        list
    }

    fn net(&mut self, request: net::Request, call: wire::Call, badge: u16) {
        let reader = badge & BADGE_COUNTERS != 0;
        let joined = self.station.state() == State::Joined;
        let info = net::Info { mac: mac64(&self.own), mtu: 1500, link: joined };
        let _ = match request {
            // Until joined the card is absent, so the stack starts DHCP once the link is up.
            net::Request::Info => net::reply_info(call, if joined { Ok(&info) } else { Err(net::Error::NoDevice) }),
            net::Request::Counters => net::reply_counters(call, &self.counters),
            net::Request::Offloads => net::reply_offloads(call, 0),
            _ if reader => match request {
                net::Request::Send { .. } => net::reply_send(call, Err(net::Error::Denied)),
                net::Request::Receive => net::reply_receive(call, Err(net::Error::Denied)),
                net::Request::Attach { ring } => { let _ = mind::ipc::drop_cap(ring); net::reply_attach(call, Err(net::Error::Denied)) }
                net::Request::SendPartial { .. } => net::reply_send_partial(call, Err(net::Error::Denied)),
                net::Request::Kick => net::reply_kick(call, Err(net::Error::Denied)),
                _ => wire::reject(wire::Reject::Invalid),
            },
            net::Request::Send { frame } => {
                let mut out = Out::default();
                let sent = self.station.send(frame, &mut out, &mut self.tx);
                self.apply(out);
                if sent.is_ok() { self.counters.sent += 1; }
                net::reply_send(call, sent.map_err(|e| if e == dot11::Error::NoDevice { net::Error::NoDevice } else { net::Error::Invalid }))
            }
            net::Request::Receive => {
                let mut frame = [0u8; ETHERNET_MAX];
                match self.queue.pop(&mut frame) { Some(len) => net::reply_receive(call, Ok(&frame[..len])), None => net::reply_receive(call, Err(net::Error::Empty)) }
            }
            net::Request::Wait => {
                if self.queue.count > 0 { net::reply_wait(call, self.queue.count as u32) } else {
                    let mut call = call;
                    match (self.waiters.iter().position(Option::is_none), call.defer()) {
                        (Some(index), Ok(())) => { self.waiters[index] = Some(call); Ok(()) }
                        _ => net::reply_wait(call, 0),
                    }
                }
            }
            // No checksum offload and no shared ring: frames go by call (net.wit 1.0).
            net::Request::SendPartial { .. } => net::reply_send_partial(call, Err(net::Error::Invalid)),
            net::Request::Attach { ring } => { let _ = mind::ipc::drop_cap(ring); net::reply_attach(call, Err(net::Error::Invalid)) }
            net::Request::Kick => net::reply_kick(call, Err(net::Error::Invalid)),
        };
    }

    fn wifi(&mut self, request: wifi::Request, call: wire::Call, badge: u16) {
        let may_change = badge & BADGE_WIFI_CHANGE == BADGE_WIFI_CHANGE;
        let now = mind::time::uptime_ms() as u64;
        let _ = match request {
            wifi::Request::Scan => {
                let mut out = Out::default();
                let started = self.station.scan(now, &mut out, &mut self.tx);
                self.apply(out);
                match started {
                    Err(dot11::Error::NoDevice) => wifi::reply_scan(call, Err(wifi::Error::NoDevice)),
                    // Joining or joined: the list as it stands, without leaving the channel.
                    Err(_) if self.station.state() != State::Scanning => wifi::reply_scan(call, Ok(self.networks().as_slice())),
                    _ => {
                        let mut call = call;
                        match (self.scans.iter().position(Option::is_none), call.defer()) {
                            (Some(index), Ok(())) => { self.scans[index] = Some(call); Ok(()) }
                            _ => wifi::reply_scan(call, Err(wifi::Error::Busy)),
                        }
                    }
                }
            }
            wifi::Request::Status => wifi::reply_status(call, &self.status()),
            // The catalogue (550-NET-0035, stage B).
            wifi::Request::Countries { from } => wifi::reply_countries(call, Ok(&self.countries(from))),
            wifi::Request::Channels { code } => {
                let result = self.channels_under(code.as_str());
                wifi::reply_channels(call, result.as_ref().map(|l| l.as_slice()).map_err(|e| *e))
            }
            wifi::Request::Country => wifi::reply_country(call, &self.country_status()),
            wifi::Request::CountryOrigin => wifi::reply_country_origin(call, &self.country_origin()),
            wifi::Request::Reader => wifi::reply_reader(call, self.reader.ok_or(wifi::Error::Unsupported)),
            _ if !may_change => match request {
                wifi::Request::Forget { .. } => wifi::reply_forget(call, Err(wifi::Error::Denied)),
                wifi::Request::Leave => wifi::reply_leave(call, Err(wifi::Error::Denied)),
                wifi::Request::SetCountry { .. } => wifi::reply_set_country(call, Err(wifi::Error::Denied)),
                _ => wifi::reply_join(call, Err(wifi::Error::Denied)),
            },
            wifi::Request::SetCountry { code } => {
                let result = self.set_country(code.as_str());
                wifi::reply_set_country(call, result.as_ref().map_err(|e| *e))
            }
            wifi::Request::Join { ssid, passphrase, remember } => wifi::reply_join(call, self.join(ssid.as_slice(), passphrase.as_str(), remember, now)),
            wifi::Request::Leave => {
                let mut out = Out::default();
                self.station.leave(&mut out, &mut self.tx);
                self.apply(out);
                // The user's word: no automatic join until the station next joins a network, and nothing held.
                self.auto.left();
                self.trying = None;
                self.held = None;
                self.offered = None;
                mind::println!("[WLAN] LEFT THE NETWORK: NO AUTOMATIC JOIN UNTIL THE NEXT ONE");
                wifi::reply_leave(call, Ok(()))
            }
            wifi::Request::Forget { ssid } => {
                let forgotten = match self.deadline.call(|| keystore::wifi_forget(KEYSTORE, ssid.as_slice())) { Ok(Ok(found)) => Ok(found), _ => Err(wifi::Error::Unsupported) };
                // A network held since a client's join is forgotten too (801-NET-0084).
                let dropped = self.held.as_ref().is_some_and(|h| h.ssid() == ssid.as_slice());
                if dropped { self.held = None; }
                let forgotten = forgotten.map(|found| found || dropped);
                if forgotten == Ok(true) { mind::println!("[WLAN] FORGOT {}", Name(ssid.as_slice())); }
                wifi::reply_forget(call, forgotten)
            }
        };
    }

    // A WPA2 network's key: a remembered one stays in keystore; a passphrase to remember goes there, and its PMK is
    // dropped here once kept; one not to remember stays here as a PMK for this join and its rejoins.
    fn secret(&self, ssid: &[u8], passphrase: &str, remember: bool) -> Result<Secret, wifi::Error> {
        if passphrase.is_empty() {
            return if !remember && self.kept(ssid) { Ok(Secret::Kept) } else { Err(wifi::Error::Invalid) };
        }
        let bytes = passphrase.as_bytes();
        let mut pmk = wpa::keys::psk_hex(bytes).or_else(|| wpa::keys::pmk(bytes, ssid)).ok_or(wifi::Error::Invalid)?;
        if !remember { return Ok(Secret::Pmk(pmk)); }
        let kept = self.deadline.call(|| keystore::wifi_remember(KEYSTORE, ssid, &pmk));
        wipe(&mut pmk);
        match kept {
            Ok(Ok(())) => { mind::println!("[WLAN] {} REMEMBERED BY KEYSTORE", Name(ssid)); Ok(Secret::Kept) }
            other => { mind::println!("[WLAN] {} NOT REMEMBERED: {:?}", Name(ssid), other.map_err(|_| ())); Err(wifi::Error::Unsupported) }
        }
    }

    fn join(&mut self, ssid: &[u8], passphrase: &str, remember: bool, now: u64) -> Result<(), wifi::Error> {
        if !self.radio { return Err(wifi::Error::NoDevice); }
        // A client's join ends an automatic try (its scan is stopped, so the station takes the join), and the next try
        // waits a first wait, so the client's outcome is the station's for a while.
        if let Some(Try::Scanning(_)) = self.trying.take() {
            if self.station.state() == State::Scanning { let mut out = Out::default(); self.station.leave(&mut out, &mut self.tx); self.apply(out); }
        }
        self.auto.idle(now);
        let secret = self.join_kept(ssid, passphrase, remember, now)?;
        // One keystore does not keep is held once it joins (801-NET-0084).
        self.offered = Some(Held::new(ssid, secret));
        Ok(())
    }

    // A client's join: a passphrase, or a network keystore keeps, over WPA2 alone; neither, an open network alone
    // (801-NET-0084). The key it joins with comes back.
    fn join_kept(&mut self, ssid: &[u8], passphrase: &str, remember: bool, now: u64) -> Result<Option<Secret>, wifi::Error> {
        let kept = passphrase.is_empty() && !remember && self.kept(ssid);
        let secret = if !passphrase.is_empty() || remember || kept {
            if self.station.networks().best_of(ssid, Security::Wpa2).is_none() {
                let named = self.station.networks().iter().any(|b| b.ssid() == ssid);
                return Err(if named { wifi::Error::Unsupported } else { wifi::Error::NotFound });
            }
            // Without RDRAND there is no SNonce to trust: fail closed (issue 103).
            if !mind::random::available() { mind::println!("[WLAN] NO {}: WPA2 REFUSED", mind::random::SOURCE); return Err(wifi::Error::Unsupported); }
            Some(if kept { Secret::Kept } else { self.secret(ssid, passphrase, remember)? })
        } else {
            // An open network has no key to remember.
            None
        };
        self.join_with(ssid, secret, now)?;
        Ok(secret)
    }

    // The station joins `ssid` with `secret`, none for an open network.
    fn join_with(&mut self, ssid: &[u8], secret: Option<Secret>, now: u64) -> Result<(), wifi::Error> {
        if secret.is_some() && !mind::random::available() { mind::println!("[WLAN] NO {}: WPA2 REFUSED", mind::random::SOURCE); return Err(wifi::Error::Unsupported); }
        let mut out = Out::default();
        let joined = self.station.join(ssid, secret, now, &mut out, &mut self.tx);
        self.apply(out);
        match joined {
            Ok(()) => { mind::println!("[WLAN] JOINING {} ON CHANNEL {}", Name(ssid), self.station.network().map_or(0, |b| b.channel)); Ok(()) }
            Err(dot11::Error::NotFound) => Err(wifi::Error::NotFound),
            Err(dot11::Error::Unsupported) => Err(wifi::Error::Unsupported),
            Err(dot11::Error::Busy) => Err(wifi::Error::Busy),
            Err(_) => Err(wifi::Error::Invalid),
        }
    }

    // Whether keystore keeps `ssid` (no when it does not answer).
    fn kept(&self, ssid: &[u8]) -> bool { remembered(self.deadline).unwrap_or_default().as_slice().iter().any(|k| k.ssid.as_slice() == ssid) }

    // The networks keystore keeps; a failure is logged once until it answers again, and the automatic join waits its
    // first wait, as with none kept (801-NET-0083).
    fn remembered(&mut self) -> List<keystore::WifiNetwork, 16> {
        match remembered(self.deadline) {
            Ok(list) => { if core::mem::replace(&mut self.keystore_failed, false) { mind::println!("[WLAN] AUTO-JOIN: KEYSTORE ANSWERS AGAIN, {} NETWORKS KEPT", list.as_slice().len()); } list }
            Err(why) => {
                if !core::mem::replace(&mut self.keystore_failed, true) { mind::println!("[WLAN] AUTO-JOIN WAITS: KEYSTORE {} FOR THE REMEMBERED NETWORKS, ASKED AGAIN IN {} S", why, auto::FIRST_MS / 1000); }
                List::default()
            }
        }
    }

    // The automatic join (550-NET-0041): when the station is not joined, a scan, then the remembered network heard, or
    // the one a client joined this boot (801-NET-0084), the one last joined first, else the strongest; the held key
    // first, else the kept one. Each try ends in one line of the log.
    fn auto_join(&mut self, now: u64) {
        // The first scan decides the country before anything is joined.
        if self.first { return; }
        let state = self.station.state();
        if state == State::Joined {
            // A client's join that joined: its network is held from now on, unless keystore keeps it (801-NET-0084).
            if let Some(next) = self.offered.take() { self.held = next; }
            if let Some(Try::Joining(name, len, number)) = self.trying.take() {
                mind::println!("[WLAN] AUTO-JOIN {}: JOINED (TRY {})", Name(&name[..len]), number);
            }
            if let Some(bss) = self.station.network() { let s = bss.ssid(); self.previous.0[..s.len()].copy_from_slice(s); self.previous.1 = s.len(); }
            self.auto.joined();
            self.try_began = now;
            self.nothing_kept = false;
            return;
        }
        // A client's join that ended without joining: nothing of it is held.
        if state != State::Joining { self.offered = None; }
        match self.trying {
            Some(Try::Scanning(number)) if state != State::Scanning => {
                self.trying = None;
                let kept = self.remembered();
                let heard = self.station.networks().iter().filter(|b| matches!(b.security, Security::Wpa2 | Security::Open)).map(|b| (b.ssid(), b.weakness));
                let held = self.held.as_ref().map_or(&[][..], Held::ssid);
                let mut name = ([0u8; 32], 0);
                if let Some(ssid) = auto::choose(heard, |s| s == held || kept.as_slice().iter().any(|k| k.ssid.as_slice() == s), &self.previous.0[..self.previous.1]) {
                    name.0[..ssid.len()].copy_from_slice(ssid);
                    name.1 = ssid.len();
                }
                if name.1 == 0 {
                    let wait = self.auto.ended(now) / 1000;
                    let joined = if self.held.is_some() { " OR JOINED THIS BOOT" } else { "" };
                    mind::println!("[WLAN] AUTO-JOIN: NO REMEMBERED NETWORK{} HEARD, {} KEPT (TRY {}), AGAIN IN {} S", joined, kept.as_slice().len(), number, wait);
                    return;
                }
                // The held key of the network a client joined, else keystore's: a key joins a WPA2 access point alone.
                let secret = match self.held.as_ref().filter(|h| h.ssid() == &name.0[..name.1]) {
                    Some(held) => { mind::println!("[WLAN] AUTO-JOIN {}: THE NETWORK A CLIENT JOINED THIS BOOT, NOT REMEMBERED (TRY {})", Name(&name.0[..name.1]), number); held.secret() }
                    None => Some(Secret::Kept),
                };
                match self.join_with(&name.0[..name.1], secret, now) {
                    Ok(()) => self.trying = Some(Try::Joining(name.0, name.1, number)),
                    Err(error) => {
                        let wait = self.auto.ended(now) / 1000;
                        mind::println!("[WLAN] AUTO-JOIN {}: {:?} (TRY {}), AGAIN IN {} S", Name(&name.0[..name.1]), error, number, wait);
                    }
                }
            }
            Some(Try::Joining(name, len, number)) if state != State::Joining => {
                self.trying = None;
                let wait = self.auto.ended(now) / 1000;
                mind::println!("[WLAN] AUTO-JOIN {}: {} (TRY {}), AGAIN IN {} S", Name(&name[..len]), said(self.station.reason()), number, wait);
            }
            Some(_) => {}
            // A client's scan, or the station's own rejoin of a network just lost, goes first.
            None if !self.radio || !self.auto.due(now) || matches!(state, State::Scanning | State::Joining) => {}
            None => {
                let kept = self.remembered();
                let nothing = kept.is_empty() && self.held.is_none();
                let told = core::mem::replace(&mut self.nothing_kept, nothing && !self.keystore_failed);
                if nothing {
                    // Said once a loss (DRV, 2026-11-14): the Ryzen was silent three hours with nothing kept.
                    if self.nothing_kept && !told { mind::println!("[WLAN] AUTO-JOIN: NO REMEMBERED NETWORK; A JOIN NEEDS THE USER"); }
                    self.auto.idle(now);
                    return;
                }
                // A remembered network is WPA2's: without a random source its handshake is refused (issue 103), so no try.
                if !mind::random::available() && self.held.as_ref().is_none_or(|h| h.pmk.is_some()) {
                    mind::println!("[WLAN] AUTO-JOIN OFF: NO {}, WHICH A REMEMBERED NETWORK'S HANDSHAKE NEEDS", mind::random::SOURCE);
                    self.auto.left();
                    return;
                }
                let number = self.auto.begin(now);
                self.try_began = now;
                let mut out = Out::default();
                let started = self.station.scan(now, &mut out, &mut self.tx);
                self.apply(out);
                match started {
                    Ok(()) => self.trying = Some(Try::Scanning(number)),
                    Err(error) => {
                        let wait = self.auto.ended(now) / 1000;
                        mind::println!("[WLAN] AUTO-JOIN: NO SCAN, {:?} (TRY {}), AGAIN IN {} S", error, number, wait);
                    }
                }
            }
        }
    }

    fn status(&self) -> wifi::Status {
        let state = match self.station.state() {
            State::Off => wifi::State::Off, State::Scanning => wifi::State::Scanning, State::Joining => wifi::State::Joining,
            State::Joined => wifi::State::Joined, State::Failed => wifi::State::Failed,
        };
        let reason = match self.station.reason() {
            Reason::None => wifi::Reason::None, Reason::WrongPassphrase => wifi::Reason::WrongPassphrase, Reason::NoAnswer => wifi::Reason::NoAnswer,
            Reason::Refused => wifi::Reason::Refused, Reason::NotFound => wifi::Reason::NotFound, Reason::Unsupported => wifi::Reason::Unsupported,
            Reason::Lost => wifi::Reason::Lost,
        };
        let mut status = wifi::Status { state, reason, ..Default::default() };
        if let Some(bss) = self.station.network() {
            let n = network(bss);
            (status.ssid, status.bssid, status.channel, status.weakness) = (n.ssid, n.bssid, n.channel, n.weakness);
            if state == wifi::State::Joined { status.rate = self.station.data_rate(); }
        }
        status
    }
}

// A country's code as a client gave it: two capital letters, or `00` where `world` allows it.
fn code_of(code: &str, world: bool) -> Result<[u8; 2], wifi::Error> {
    let bytes: [u8; 2] = code.as_bytes().try_into().map_err(|_| wifi::Error::Invalid)?;
    if bytes == *b"00" { return if world { Ok(bytes) } else { Err(wifi::Error::Invalid) }; }
    if !bytes.iter().all(u8::is_ascii_uppercase) { return Err(wifi::Error::Invalid); }
    Ok(bytes)
}

fn wire_text<const N: usize>(text: &str) -> Option<mind::idl::codec::Text<N>> { mind::idl::codec::Text::new(text) }

// Why the station failed or lost its network, for the log.
fn said(reason: Reason) -> &'static str {
    match reason {
        Reason::WrongPassphrase => "WRONG PASSPHRASE", Reason::NoAnswer => "NO ANSWER", Reason::Refused => "REFUSED",
        Reason::NotFound => "NOT FOUND", Reason::Unsupported => "UNSUPPORTED", Reason::Lost => "LOST", Reason::None => "LEFT",
    }
}

fn network(bss: &Bss) -> wifi::Network {
    let mut ssid = List::default();
    for &b in bss.ssid() { ssid.push(b); }
    let security = match bss.security {
        Security::Open => wifi::Security::Open, Security::Wep => wifi::Security::Wep, Security::Wpa2 => wifi::Security::Wpa2,
        Security::Wpa3 => wifi::Security::Wpa3, Security::Enterprise => wifi::Security::Enterprise, Security::Other => wifi::Security::Other,
    };
    wifi::Network { ssid, bssid: mac64(&bss.bssid), channel: bss.channel, weakness: bss.weakness, security, remembered: false }
}

// An SSID for the log: printable ASCII as it is, any other byte as '?'.
struct Name<'a>(&'a [u8]);
impl core::fmt::Display for Name<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        for &b in self.0 { write!(f, "{}", if (0x20..0x7F).contains(&b) { b as char } else { '?' })?; }
        Ok(())
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("wlan — the 802.11 station (a service): joins Wi-Fi networks through a radio's wlanhw.wit client and is the network stack's card for them.");
    // Without a radio's client (started by hand, not by init with a radio) there is nothing to drive.
    if mind::dev::cap_info(RADIO.0).0 != mind::abi::CAP_KIND_ENDPOINT { mind::println!("[WLAN] NO RADIO: NOTHING TO DRIVE"); mind::process::exit_with(1) }
    let mut wlan = Wlan { station: Station::new([0; 6], &[], false), deadline: Deadline::after(0, 0), radio: false, own: [0; 6], tx: [0; FRAME_MAX], rx: [0; ETHERNET_MAX],
                          radio_frame: [0; FRAME_MAX + RADIO_HEADER],
                          queue: Queue { frames: alloc::vec![0; QUEUE * ETHERNET_MAX], lens: [0; QUEUE], head: 0, count: 0 }, counters: net::Counters::default(),
                          waiters: [const { None }; WAITERS], scans: [const { None }; WAITERS], auto: auto::Auto::new(), trying: None, previous: ([0; 32], 0),
                          channels: ([0; catalogue::CHANNELS], 0), chosen: None, kept: false, running: None, confirmed: false, refused: false, first: false,
                          reader: mind::ipc::mint_badged(Endpoint::SERVICE.0, mind::abi::CAP_WRITE | mind::abi::CAP_GRANT, BADGE_WIFI).ok(), tuned: false, checked: 0, restarted: None,
                          tuned_to: 0, again: None, had_radio: false, try_began: 0, nothing_kept: false, keystore_failed: false,
                          held: None, offered: None };
    if wlan.reader.is_none() { mind::println!("[WLAN] NO READING CLIENT: THE STATION'S ENDPOINT HAS NO RIGHT TO BADGE (wifi.wit's reader answers unsupported)"); }
    let mut scratch = [0u8; net::REQUEST_MAX];
    let mut last = State::Off;
    let (mut asked, mut waited) = (0u64, false);
    loop {
        let now = mind::time::uptime_ms() as u64;
        // Pump and client dispatch share the remaining time, including radio and key-store calls.
        wlan.deadline = Deadline::after(mind::time::monotonic_ns() / 1_000_000, RADIO_MS);
        if wlan.watch(now) { (asked, waited) = (0, false); }
        if !wlan.radio && (asked == 0 || now - asked >= RADIO_MS as u64) {
            asked = now.max(1);
            match radio_call(wlan.deadline, || wlanhw::info(RADIO)) {
                Ok(Ok(info)) => wlan.radio_up(&info),
                Err(mind::sys::Error::Other(mind::abi::ERR_TIMEOUT)) => if !core::mem::replace(&mut waited, true) {
                    mind::println!("[WLAN] THE RADIO HAS NOT ANSWERED IN {} S: SERVING WITHOUT IT, ASKING AGAIN", RADIO_MS / 1000);
                },
                // A radio that answered before, whose driver init starts again: asked again, as one that does not answer (801-NET-0083).
                Err(mind::sys::Error::Peer) if wlan.had_radio => if !core::mem::replace(&mut waited, true) {
                    mind::println!("[WLAN] THE RADIO'S DRIVER IS NOT THERE: SERVING WITHOUT IT, ASKING AGAIN");
                },
                // No device, another major version of wlanhw.wit, or a client that fails: nothing to drive.
                _ => { mind::println!("[WLAN] NO RADIO: NOTHING TO DRIVE (NO DEVICE, OR NOT A WLANHW {}.X RADIO)", wlanhw::VERSION.0); mind::process::exit_with(1) }
            }
        }
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, POLL_MS);
        // Dispatch accepted calls before bulk work; a long handler cannot renew the shared two-second deadline.
        service_turn::run(&mut wlan, received, |wlan, received| {
            let Ok(request) = received else { return };
            let badge = request.badge;
            if badge & BADGE_WIFI != 0 {
                match wifi::decode(&request, RECEIVED_CAP) {
                    Ok((request, call)) => wlan.wifi(request, call, badge),
                    Err(reason) => if request.is_call { let _ = wire::reject(reason); },
                }
            } else {
                match net::decode(&request, RECEIVED_CAP, &mut scratch) {
                    Ok((request, call)) => wlan.net(request, call, badge),
                    Err(reason) => if request.is_call { let _ = wire::reject(reason); },
                }
            }
        }, |wlan| wlan.radio && wlan.deadline.remaining(mind::time::monotonic_ns() / 1_000_000).is_some(), Wlan::pump);
        // Each with a deadline of its own: a turn whose radio calls spent theirs still checks the radio and the automatic
        // join, which a hung radio would otherwise starve for ever without a line (801-NET-0083).
        wlan.deadline = Deadline::after(mind::time::monotonic_ns() / 1_000_000, RADIO_MS);
        wlan.radio_restarted(mind::time::uptime_ms() as u64);
        wlan.deadline = Deadline::after(mind::time::monotonic_ns() / 1_000_000, RADIO_MS);
        wlan.auto_join(mind::time::uptime_ms() as u64);
        let state = wlan.station.state();
        if last == State::Scanning && state != State::Scanning { wlan.scanned(); }
        let state = wlan.station.state();
        if state != last {
            let reason = wlan.station.reason();
            let shown = (state == State::Failed && reason != Reason::None) || reason == Reason::Lost;
            mind::println!("[WLAN] {:?}{}{}", state, if shown { ": " } else { "" }, if shown { said(reason) } else { "" });
            last = state;
        }
    }
}
