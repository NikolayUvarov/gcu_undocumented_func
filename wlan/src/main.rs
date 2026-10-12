#![no_std]
#![no_main]
// The 802.11 station (550-NET-0018): drives `dot11` over a radio's idl/wlanhw.wit client (slot 2) and serves its own
// endpoint, idl/net.wit to the network stack (a client without BADGE_WIFI) and idl/wifi.wit to clients with BADGE_WIFI.
// It holds the radio and nothing else that can act (MC-2.3); a passphrase becomes a PMK at once and is not kept.
use dot11::scan::{Bss, Security};
use dot11::station::{Out, Reason, State, Station, ETHERNET_MAX, FRAME_MAX};
use mind::abi::BootInfo;
use mind::idl::codec::List;
use mind::idl::{net, wifi, wire, wlanhw};
use mind::ipc::Endpoint;
use mind::network::{BADGE_COUNTERS, BADGE_WIFI, BADGE_WIFI_CHANGE};

extern crate alloc;
use alloc::vec::Vec;

const RADIO: Endpoint = Endpoint(2);
const RECEIVED_CAP: usize = 9;
const POLL_MS: u32 = 10;
// Frames taken from the radio per poll, received Ethernet frames kept for the stack, parked `wait` calls.
const BURST: usize = 64;
const QUEUE: usize = 32;
const WAITERS: usize = 4;
const ATTEMPTS: u8 = 4;
const RADIO_HEADER: usize = 8;

fn mac(value: u64) -> [u8; 6] { let b = value.to_be_bytes(); [b[2], b[3], b[4], b[5], b[6], b[7]] }
fn mac64(m: &[u8; 6]) -> u64 { m.iter().fold(0, |v, &b| v << 8 | b as u64) }

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

struct Wlan {
    station: Station,
    own: [u8; 6],
    tx: [u8; FRAME_MAX],
    rx: [u8; ETHERNET_MAX],
    radio_frame: [u8; FRAME_MAX + RADIO_HEADER],
    queue: Queue,
    counters: net::Counters,
    waiters: [Option<wire::Call>; WAITERS],
    scans: [Option<wire::Call>; WAITERS],
}

impl Wlan {
    // What the station asked of the radio: tune, filter, send; a received Ethernet frame for the stack.
    fn apply(&mut self, out: Out) {
        if let Some(channel) = out.channel { let _ = wlanhw::set_channel(RADIO, channel); }
        if let Some(f) = out.filter {
            let _ = wlanhw::set_filter(RADIO, &wlanhw::Filter { all_beacons: f.all_beacons, bssid: mac64(&f.bssid), own: 0 });
        }
        if let Some((len, rate)) = out.send {
            let unicast_data = self.tx[0] & 0x0C == 0x08 && self.tx[4] & 1 == 0;
            match wlanhw::send(RADIO, &self.tx[..len], rate, ATTEMPTS) {
                Ok(Ok(sent)) => { if unicast_data { self.station.sent(sent.acked); } }
                _ => if unicast_data { self.station.sent(false) },
            }
        }
        if let Some(len) = out.deliver {
            self.counters.received += 1;
            if !self.queue.push(&self.rx[..len]) { self.counters.dropped += 1; }
        }
    }

    // Takes the radio's frames (a burst at most), runs the station's timers, answers parked calls.
    fn pump(&mut self) {
        let now = mind::time::uptime_ms() as u64;
        for _ in 0..BURST {
            let len = match wlanhw::receive(RADIO, &mut self.radio_frame) { Ok(Ok(len)) => len.min(self.radio_frame.len()), _ => break };
            if len < RADIO_HEADER { continue; }
            let header = &self.radio_frame[..RADIO_HEADER];
            let frame_len = u16::from_le_bytes([header[0], header[1]]) as usize;
            let (weakness, channel) = (header[2], header[3]);
            if RADIO_HEADER + frame_len > len { continue; }
            let mut frame = [0u8; FRAME_MAX];
            frame[..frame_len].copy_from_slice(&self.radio_frame[RADIO_HEADER..RADIO_HEADER + frame_len]);
            // A fresh SNonce is drawn only for data frames, where message 1 may be; without RDRAND they are dropped.
            let mut random = [0u8; 32];
            if frame_len > 0 && frame[0] & 0x0C == 0x08 && !mind::random::fill(&mut random) { continue; }
            let mut out = Out::default();
            let _ = self.station.receive(&frame[..frame_len], channel, weakness, now, &random, &mut out, &mut self.tx, &mut self.rx);
            self.apply(out);
        }
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
        let mut list = List::default();
        for bss in self.station.networks().iter() { list.push(network(bss)); }
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
            _ if !may_change => match request {
                wifi::Request::Forget { .. } => wifi::reply_forget(call, Err(wifi::Error::Denied)),
                wifi::Request::Leave => wifi::reply_leave(call, Err(wifi::Error::Denied)),
                _ => wifi::reply_join(call, Err(wifi::Error::Denied)),
            },
            wifi::Request::Join { ssid, passphrase, remember } => wifi::reply_join(call, self.join(ssid.as_slice(), passphrase.as_str(), remember, now)),
            wifi::Request::Leave => {
                let mut out = Out::default();
                self.station.leave(&mut out, &mut self.tx);
                self.apply(out);
                mind::println!("[WLAN] LEFT THE NETWORK");
                wifi::reply_leave(call, Ok(()))
            }
            // Nothing is remembered until keystore keeps keys for the station (550-NET-0018).
            wifi::Request::Forget { .. } => wifi::reply_forget(call, Ok(false)),
        };
    }

    fn join(&mut self, ssid: &[u8], passphrase: &str, remember: bool, now: u64) -> Result<(), wifi::Error> {
        if remember { return Err(wifi::Error::Unsupported); }
        let bss = self.station.networks().best(ssid).copied();
        let pmk = match bss.map(|b| b.security) {
            Some(Security::Wpa2) => {
                // Without RDRAND there is no SNonce to trust: fail closed (issue 103).
                if !mind::random::available() { mind::println!("[WLAN] NO {}: WPA2 REFUSED", mind::random::SOURCE); return Err(wifi::Error::Unsupported); }
                let bytes = passphrase.as_bytes();
                Some(wpa::keys::psk_hex(bytes).or_else(|| wpa::keys::pmk(bytes, ssid)).ok_or(wifi::Error::Invalid)?)
            }
            _ => None,
        };
        let mut out = Out::default();
        let joined = self.station.join(ssid, pmk, now, &mut out, &mut self.tx);
        self.apply(out);
        match joined {
            Ok(()) => { mind::println!("[WLAN] JOINING {} ON CHANNEL {}", Name(ssid), bss.map_or(0, |b| b.channel)); Ok(()) }
            Err(dot11::Error::NotFound) => Err(wifi::Error::NotFound),
            Err(dot11::Error::Unsupported) => Err(wifi::Error::Unsupported),
            Err(dot11::Error::Busy) => Err(wifi::Error::Busy),
            Err(_) => Err(wifi::Error::Invalid),
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
    let info = match wlanhw::info(RADIO) { Ok(Ok(info)) => info, _ => { mind::println!("[WLAN] NO RADIO"); wlanhw::Info::default() } };
    let own = mac(info.mac);
    let channels = info.channels.as_slice();
    mind::println!("[WLAN] RADIO {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}, {} CHANNELS", own[0], own[1], own[2], own[3], own[4], own[5], channels.len());
    let mut wlan = Wlan { station: Station::new(own, channels), own, tx: [0; FRAME_MAX], rx: [0; ETHERNET_MAX], radio_frame: [0; FRAME_MAX + RADIO_HEADER],
                          queue: Queue { frames: alloc::vec![0; QUEUE * ETHERNET_MAX], lens: [0; QUEUE], head: 0, count: 0 }, counters: net::Counters::default(),
                          waiters: [const { None }; WAITERS], scans: [const { None }; WAITERS] };
    let mut scratch = [0u8; net::REQUEST_MAX];
    let mut last = State::Off;
    loop {
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, POLL_MS);
        wlan.pump();
        let state = wlan.station.state();
        if state != last {
            mind::println!("[WLAN] {:?}{}", state, if state == State::Failed || wlan.station.reason() == Reason::Lost { match wlan.station.reason() {
                Reason::WrongPassphrase => ": WRONG PASSPHRASE", Reason::NoAnswer => ": NO ANSWER", Reason::Refused => ": REFUSED",
                Reason::NotFound => ": NOT FOUND", Reason::Unsupported => ": UNSUPPORTED", Reason::Lost => ": LOST", Reason::None => "" } } else { "" });
            last = state;
        }
        let Ok(request) = received else { continue };
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
    }
}
