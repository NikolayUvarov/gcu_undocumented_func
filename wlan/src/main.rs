#![no_std]
#![no_main]
// The 802.11 station (550-NET-0018): drives `dot11` over a radio's idl/wlanhw.wit client (slot 2) and serves its own
// endpoint, idl/net.wit to the network stack (a client without BADGE_WIFI) and idl/wifi.wit to clients with BADGE_WIFI.
// It holds the radio and nothing else that can act (MC-2.3); a passphrase becomes a PMK at once and is not kept. A
// network to remember goes to `keystore` (slot 3, BADGE_KEY_STATION, 550-NET-0024), which keeps its PMK and answers
// each handshake's keys: the station holds no PMK of a remembered network (MC-11.9). The country (wifi.wit 1.1,
// 550-NET-0032) needs the regulatory catalogue, not in this build yet (stage A): its calls answer `unsupported`, no
// country is applied, and the station keeps to the rules that hold before one is known (probes on 1 to 11 alone).
use dot11::scan::{Bss, Security};
use dot11::station::{Out, Reason, Secret, State, Station, ETHERNET_MAX, FRAME_MAX};
use mind::abi::BootInfo;
use mind::idl::codec::List;
use mind::idl::{keystore, net, wifi, wire, wlanhw};
use mind::ipc::Endpoint;
use mind::network::{BADGE_COUNTERS, BADGE_WIFI, BADGE_WIFI_CHANGE};

extern crate alloc;
use alloc::vec::Vec;

const RADIO: Endpoint = Endpoint(2);
const KEYSTORE: Endpoint = Endpoint(3);
const RECEIVED_CAP: usize = 9;
const POLL_MS: u32 = 10;
// Frames taken from the radio per poll, received Ethernet frames kept for the stack, parked `wait` calls.
const BURST: usize = 64;
const QUEUE: usize = 32;
const WAITERS: usize = 4;
const ATTEMPTS: u8 = 4;
const RADIO_HEADER: usize = 8;
// The radio is asked with a deadline, and again every RADIO_MS until it answers; meanwhile the station serves its
// clients as a card without a radio (550-NET-0030), so the stack does not wait on it.
const RADIO_MS: u32 = 2000;

fn mac(value: u64) -> [u8; 6] { let b = value.to_be_bytes(); [b[2], b[3], b[4], b[5], b[6], b[7]] }
fn mac64(m: &[u8; 6]) -> u64 { m.iter().fold(0, |v, &b| v << 8 | b as u64) }

// Overwrites a copy of key material.
fn wipe(bytes: &mut [u8]) { for byte in bytes.iter_mut() { unsafe { core::ptr::write_volatile(byte, 0) }; } }

// A remembered network's keys, one handshake at a time, from `keystore` (its SSID).
struct Kept([u8; 32], usize);

impl wpa::handshake::Pairwise for Kept {
    fn ptk(&mut self, ap: &[u8; 6], own: &[u8; 6], anonce: &[u8; 32], snonce: &[u8; 32]) -> Option<wpa::keys::Ptk> {
        let mut keys = match keystore::wifi_keys(KEYSTORE, &self.0[..self.1], mac64(ap), mac64(own), anonce, snonce) { Ok(Ok(keys)) => keys, _ => return None };
        let k = keys.as_slice();
        let ptk = (k.len() == 48).then(|| wpa::keys::Ptk { kck: k[..16].try_into().unwrap(), kek: k[16..32].try_into().unwrap(), tk: k[32..].try_into().unwrap() });
        unsafe { core::ptr::write_volatile(&mut keys, List::default()) };
        ptk
    }
}

// The names `keystore` keeps networks for (none without its client).
fn remembered() -> List<keystore::WifiNetwork, 16> { match keystore::wifi_networks(KEYSTORE) { Ok(Ok(list)) => list, _ => List::default() } }

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
}

impl Wlan {
    // The radio answered: the station gets its address, channels and rates.
    fn radio_up(&mut self, info: &wlanhw::Info) {
        let own = mac(info.mac);
        let channels = info.channels.as_slice();
        // Without OFDM on 2.4 GHz (a 1.0 radio says nothing of it) the station keeps to 802.11b's rates there (550-NET-0028).
        let ofdm = info.bands & mind::network::BAND_2GHZ_OFDM != 0;
        mind::println!("[WLAN] RADIO {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}, {} CHANNELS, 2.4 GHZ {}", own[0], own[1], own[2], own[3], own[4], own[5],
                       channels.len(), if ofdm { "802.11G" } else { "802.11B" });
        self.station = Station::new(own, channels, ofdm);
        self.own = own;
        self.radio = true;
    }

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
            let mut kept = Kept([0; 32], 0);
            if let Some(bss) = self.station.network() { kept.1 = bss.ssid().len(); kept.0[..kept.1].copy_from_slice(bss.ssid()); }
            let _ = self.station.receive(&frame[..frame_len], channel, weakness, now, &random, &mut kept, &mut out, &mut self.tx, &mut self.rx);
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
        let kept = remembered();
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
            // No catalogue yet (stage A): no country to list, show or apply.
            wifi::Request::Countries { .. } => wifi::reply_countries(call, Err(wifi::Error::Unsupported)),
            wifi::Request::Channels { .. } => wifi::reply_channels(call, Err(wifi::Error::Unsupported)),
            wifi::Request::Country => wifi::reply_country(call, &wifi::CountryStatus::default()),
            _ if !may_change => match request {
                wifi::Request::Forget { .. } => wifi::reply_forget(call, Err(wifi::Error::Denied)),
                wifi::Request::Leave => wifi::reply_leave(call, Err(wifi::Error::Denied)),
                wifi::Request::SetCountry { .. } => wifi::reply_set_country(call, Err(wifi::Error::Denied)),
                _ => wifi::reply_join(call, Err(wifi::Error::Denied)),
            },
            wifi::Request::SetCountry { .. } => wifi::reply_set_country(call, Err(wifi::Error::Unsupported)),
            wifi::Request::Join { ssid, passphrase, remember } => wifi::reply_join(call, self.join(ssid.as_slice(), passphrase.as_str(), remember, now)),
            wifi::Request::Leave => {
                let mut out = Out::default();
                self.station.leave(&mut out, &mut self.tx);
                self.apply(out);
                mind::println!("[WLAN] LEFT THE NETWORK");
                wifi::reply_leave(call, Ok(()))
            }
            wifi::Request::Forget { ssid } => {
                let forgotten = match keystore::wifi_forget(KEYSTORE, ssid.as_slice()) { Ok(Ok(found)) => Ok(found), _ => Err(wifi::Error::Unsupported) };
                if forgotten == Ok(true) { mind::println!("[WLAN] FORGOT {}", Name(ssid.as_slice())); }
                wifi::reply_forget(call, forgotten)
            }
        };
    }

    // A WPA2 network's key: a remembered one stays in keystore; a passphrase to remember goes there, and its PMK is
    // dropped here once kept; one not to remember stays here as a PMK for this join and its rejoins.
    fn secret(ssid: &[u8], passphrase: &str, remember: bool) -> Result<Secret, wifi::Error> {
        if passphrase.is_empty() {
            let kept = remembered().as_slice().iter().any(|k| k.ssid.as_slice() == ssid);
            return if kept && !remember { Ok(Secret::Kept) } else { Err(wifi::Error::Invalid) };
        }
        let bytes = passphrase.as_bytes();
        let mut pmk = wpa::keys::psk_hex(bytes).or_else(|| wpa::keys::pmk(bytes, ssid)).ok_or(wifi::Error::Invalid)?;
        if !remember { return Ok(Secret::Pmk(pmk)); }
        let kept = keystore::wifi_remember(KEYSTORE, ssid, &pmk);
        wipe(&mut pmk);
        match kept {
            Ok(Ok(())) => { mind::println!("[WLAN] {} REMEMBERED BY KEYSTORE", Name(ssid)); Ok(Secret::Kept) }
            other => { mind::println!("[WLAN] {} NOT REMEMBERED: {:?}", Name(ssid), other.map_err(|_| ())); Err(wifi::Error::Unsupported) }
        }
    }

    fn join(&mut self, ssid: &[u8], passphrase: &str, remember: bool, now: u64) -> Result<(), wifi::Error> {
        if !self.radio { return Err(wifi::Error::NoDevice); }
        let bss = self.station.networks().best(ssid).copied();
        let secret = match bss.map(|b| b.security) {
            Some(Security::Wpa2) => {
                // Without RDRAND there is no SNonce to trust: fail closed (issue 103).
                if !mind::random::available() { mind::println!("[WLAN] NO {}: WPA2 REFUSED", mind::random::SOURCE); return Err(wifi::Error::Unsupported); }
                Some(Self::secret(ssid, passphrase, remember)?)
            }
            // An open network has no key to remember.
            _ => None,
        };
        let mut out = Out::default();
        let joined = self.station.join(ssid, secret, now, &mut out, &mut self.tx);
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
    mind::about!("wlan — the 802.11 station (a service): joins Wi-Fi networks through a radio's wlanhw.wit client and is the network stack's card for them.");
    // Without a radio's client (started by hand, not by init with a radio) there is nothing to drive.
    if mind::dev::cap_info(RADIO.0).0 != mind::abi::CAP_KIND_ENDPOINT { mind::println!("[WLAN] NO RADIO: NOTHING TO DRIVE"); mind::process::exit_with(1) }
    let mut wlan = Wlan { station: Station::new([0; 6], &[], false), radio: false, own: [0; 6], tx: [0; FRAME_MAX], rx: [0; ETHERNET_MAX],
                          radio_frame: [0; FRAME_MAX + RADIO_HEADER],
                          queue: Queue { frames: alloc::vec![0; QUEUE * ETHERNET_MAX], lens: [0; QUEUE], head: 0, count: 0 }, counters: net::Counters::default(),
                          waiters: [const { None }; WAITERS], scans: [const { None }; WAITERS] };
    let mut scratch = [0u8; net::REQUEST_MAX];
    let mut last = State::Off;
    let (mut asked, mut waited) = (0u64, false);
    loop {
        let now = mind::time::uptime_ms() as u64;
        if !wlan.radio && (asked == 0 || now - asked >= RADIO_MS as u64) {
            asked = now.max(1);
            match wire::with_timeout(RADIO_MS, || wlanhw::info(RADIO)) {
                Ok(Ok(info)) => wlan.radio_up(&info),
                Err(mind::sys::Error::Other(mind::abi::ERR_TIMEOUT)) => if !core::mem::replace(&mut waited, true) {
                    mind::println!("[WLAN] THE RADIO HAS NOT ANSWERED IN {} S: SERVING WITHOUT IT, ASKING AGAIN", RADIO_MS / 1000);
                },
                // A radio that answers it has no device, or a client that fails: nothing to drive.
                _ => { mind::println!("[WLAN] NO RADIO: NOTHING TO DRIVE"); mind::process::exit_with(1) }
            }
        }
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, POLL_MS);
        // The radio's calls are bounded too: one that stops answering does not hold the station's clients.
        if wlan.radio { wire::with_timeout(RADIO_MS, || wlan.pump()); }
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
