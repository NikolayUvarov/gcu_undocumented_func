// The radio as a service (550-DRV-0027): idl/wlanhw.wit 1.2 served on the service endpoint, which init lends to the
// station `wlan` alone (MC-2.3). Received frames are polled from the ring every few milliseconds; `wait` answers when
// one is queued. No shared ring yet: frames go by `receive`, as the station takes them.
use crate::radio::{Counts, Error, Filter, Radio, Sent, FRAME_MAX, OFDM_RATES, RADIO_HEADER};
use mind::idl::codec::List;
use mind::idl::{wire, wlanhw};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use mind::ipc::Endpoint;
use mind::network::{BAND_2GHZ, BAND_2GHZ_OFDM};

// The slot a request's capability lands in; the receive timeout, which is also the polling period; deferred waits.
const RECEIVED_CAP: usize = 9;
const POLL_MS: u32 = 2;
const WAITERS: usize = 4;
// 2.4 GHz with OFDM there too (idl/wlanhw.wit 1.1, 550-NET-0028): OFDM is received up to 54 Mbit/s with core 2's front
// end off (550-DRV-0029), so the station offers 802.11g's rates.
// 802.11g is offered to the station only when asked (data/wifi-ofdm.txt, 550-DRV-0037): OFDM from the MacBook Pro's
// access point did not come through (550-DRV-0036), so the station keeps to 802.11b's rates and the access point to CCK.
// With 802.11g, frames with a bad FCS reach bcm_wifi too, to be counted by rate and length (550-DRV-0029).
static OFDM: AtomicBool = AtomicBool::new(false);
pub fn offer_ofdm(on: bool) { OFDM.store(on, Ordering::Relaxed); }
fn bands() -> u8 { BAND_2GHZ | if OFDM.load(Ordering::Relaxed) { BAND_2GHZ_OFDM } else { 0 } }
// The radio's own channels.
const CHANNELS: core::ops::RangeInclusive<u8> = 1..=13;
const PROBE_REQUEST: u8 = 0x40;

// A country's limits (wlanhw.wit 1.2, 550-NET-0035): the channels the station may use and those it may only listen on,
// as bit masks. None until first set, when 1.1's ways hold, as the station of stage A sets none (so does wlansim). EIRP
// is not applied: the transmit gain is fixed and not calibrated in dBm (550-DRV-0028).
#[derive(Clone, Copy)]
struct Country { channels: u16, listen: u16 }

impl Country {
    fn allows(c: Option<Country>, channel: u8) -> bool { c.map_or(CHANNELS.contains(&channel), |c| c.channels >> channel & 1 != 0) }

    // Whether `frame` may go out on `channel`: no probe request where the station may only listen.
    fn may_send(c: Option<Country>, channel: u8, frame: &[u8]) -> bool {
        let Some(c) = c else { return true };
        c.channels >> channel & 1 != 0 && !(c.listen >> channel & 1 != 0 && frame.first() == Some(&PROBE_REQUEST))
    }

    fn list(c: Option<Country>) -> List<u8, 64> {
        let mut channels = List::default();
        for ch in CHANNELS.filter(|&ch| Country::allows(c, ch)) { channels.push(ch); }
        channels
    }

    // The limits checked: a two-letter capital code, each channel once and the radio's own.
    fn from(code: &str, limits: &[wlanhw::Limit]) -> Option<Country> {
        if code.len() != 2 || !code.bytes().all(|b| b.is_ascii_uppercase()) { return None; }
        let mut c = Country { channels: 0, listen: 0 };
        for l in limits {
            if !CHANNELS.contains(&l.channel) || c.channels >> l.channel & 1 != 0 { return None; }
            c.channels |= 1 << l.channel;
            if l.listen { c.listen |= 1 << l.channel; }
        }
        Some(c)
    }
}

/// The radio's traffic over a period (550-DRV-0036), logged when unicast data went out in it, so a ping's loss can be
/// placed: the data frames sent to the access point, acknowledged or not and after how many tries, by rate; and what
/// came in meanwhile, by modulation and outcome.
struct Tally { start: u64, sent: u32, acked: u32, retried: u32, tries: u32, rates: [(u8, u16, u16); 8], seen: Counts }
const TALLY_MS: u64 = 10_000;

impl Tally {
    fn new(radio: &Radio) -> Self { Tally { start: mind::time::uptime_ms() as u64, sent: 0, acked: 0, retried: 0, tries: 0, rates: [(0, 0, 0); 8], seen: radio.counts } }

    fn sent(&mut self, frame: &[u8], rate: u8, s: &Sent) {
        // Unicast data only: a ping and its kin, not management frames or broadcasts.
        if frame.len() < 10 || frame[0] & 0x0C != 0x08 || frame[4] & 1 != 0 { return; }
        self.sent += 1;
        self.tries += s.attempts as u32;
        if s.acked { self.acked += 1; if s.attempts > 1 { self.retried += 1; } }
        if let Some(r) = self.rates.iter_mut().find(|r| r.0 == rate || r.0 == 0) {
            r.0 = rate;
            r.1 += 1;
            if s.acked { r.2 += 1; }
        }
    }

    fn tick(&mut self, radio: &Radio) {
        if (mind::time::uptime_ms() as u64) < self.start + TALLY_MS { return; }
        if self.sent > 0 {
            let (k, b) = (radio.counts, self.seen);
            let f = |kind: usize, outcome: usize| k.frames[kind][outcome].wrapping_sub(b.frames[kind][outcome]);
            let mut rates = [0u8; 96];
            let mut w = Text { buf: &mut rates, len: 0 };
            for r in self.rates.iter().filter(|r| r.0 != 0) {
                let _ = core::fmt::Write::write_fmt(&mut w, format_args!(" {}{}:{}/{}", r.0 / 2, if r.0 % 2 == 1 { ".5" } else { "" }, r.1, r.2));
            }
            let rates = core::str::from_utf8(&w.buf[..w.len]).unwrap_or("");
            // Two lines: a log record keeps 200 bytes.
            let seconds = (mind::time::uptime_ms() as u64 - self.start) / 1000;
            mind::println!("[BCM] {} S SENT: {} DATA, {} ACKED ({} RETRIED), {} LOST, {} TRIES; MBIT/S SENT/ACKED{}",
                           seconds, self.sent, self.acked, self.retried, self.sent - self.acked, self.tries, rates);
            mind::println!("[BCM] {} S RECEIVED (OK/FCS/PLCP): CCK {}/{}/{} OFDM {}/{}/{} HT {}/{}/{}; QUEUED {}, FILTERED {}, BAD {}; ACKS TO US CCK {} OFDM {}; DMA ERRORS {}",
                           seconds, f(0, 0), f(0, 1), f(0, 2), f(1, 0), f(1, 1), f(1, 2), f(2, 0), f(2, 1), f(2, 2),
                           k.queued.wrapping_sub(b.queued), k.filtered.wrapping_sub(b.filtered), k.bad.wrapping_sub(b.bad),
                           k.acks_cck.wrapping_sub(b.acks_cck), k.acks_ofdm.wrapping_sub(b.acks_ofdm), k.dma_errors.wrapping_sub(b.dma_errors));
            if OFDM.load(Ordering::Relaxed) && f(1, 0) + f(1, 1) > 0 { ofdm_lines(seconds, &k, &b); }
        }
        *self = Tally::new(radio);
    }
}

/// OFDM received over a period, good and with a bad FCS, by rate and by length, and the mean powers of each by core: a
/// frame received badly is told from one not heard at all (550-DRV-0029). Two lines, as a log record keeps 200 bytes.
fn ofdm_lines(seconds: u64, k: &Counts, b: &Counts) {
    let mut buf = [0u8; 160];
    let mut w = Text { buf: &mut buf, len: 0 };
    for (i, rate) in OFDM_RATES.iter().enumerate() {
        let n = |o: usize| k.ofdm_rates[i][o].wrapping_sub(b.ofdm_rates[i][o]);
        let _ = core::fmt::Write::write_fmt(&mut w, format_args!(" {}:{}/{}", rate / 2, n(0), n(1)));
    }
    let rates = core::str::from_utf8(&w.buf[..w.len]).unwrap_or("");
    mind::println!("[BCM] {} S OFDM BY MBIT/S (OK/BAD FCS):{}", seconds, rates);
    let l = |i: usize, o: usize| k.ofdm_lengths[i][o].wrapping_sub(b.ofdm_lengths[i][o]);
    let (good, bad) = (k.frames[1][0].wrapping_sub(b.frames[1][0]), k.frames[1][1].wrapping_sub(b.frames[1][1]));
    let mean = |sum: i32, base: i32, n: u32| if n == 0 { 0 } else { sum.wrapping_sub(base) / n as i32 };
    let g: [i32; 3] = core::array::from_fn(|c| mean(k.ofdm_power[c], b.ofdm_power[c], good));
    let x: [i32; 3] = core::array::from_fn(|c| mean(k.bad_fcs_power[c], b.bad_fcs_power[c], bad));
    mind::println!("[BCM] {} S OFDM BY BYTES (OK/BAD FCS): <=64 {}/{}, 65-256 {}/{}, MORE {}/{}; MEAN DBM BY CORE OK {}/{}/{}, BAD FCS {}/{}/{}; CLIPPED {}/{}",
                   seconds, l(0, 0), l(0, 1), l(1, 0), l(1, 1), l(2, 0), l(2, 1), g[0], g[1], g[2], x[0], x[1], x[2],
                   k.ofdm_clipped.wrapping_sub(b.ofdm_clipped), k.bad_fcs_clipped.wrapping_sub(b.bad_fcs_clipped));
}

/// A line built without a heap.
struct Text<'a> { buf: &'a mut [u8], len: usize }
impl core::fmt::Write for Text<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let n = s.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

fn error(e: Error) -> wlanhw::Error {
    match e { Error::NoDevice => wlanhw::Error::NoDevice, Error::Busy => wlanhw::Error::Busy, Error::Empty => wlanhw::Error::Empty, Error::Invalid => wlanhw::Error::Invalid, Error::Failed => wlanhw::Error::Failed }
}

// An address as the interface carries it: the low 48 bits, the first byte in bits 40-47.
fn mac64(m: &[u8; 6]) -> u64 { m.iter().fold(0, |v, &b| v << 8 | b as u64) }
fn mac(v: u64) -> [u8; 6] { core::array::from_fn(|i| (v >> (40 - 8 * i)) as u8) }

pub fn run(radio: &mut Radio) -> ! {
    HOLDING.store(false, Ordering::Relaxed);
    mind::println!("[BCM] SERVING idl/wlanhw.wit {}.{}.{}: MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}, CHANNELS 1-13", wlanhw::VERSION.0, wlanhw::VERSION.1,
                   wlanhw::VERSION.2, radio.mac[0], radio.mac[1], radio.mac[2], radio.mac[3], radio.mac[4], radio.mac[5]);
    let mut scratch = [0u8; wlanhw::REQUEST_MAX];
    let mut out = [0u8; RADIO_HEADER + FRAME_MAX];
    let mut waiters: [Option<wire::Call>; WAITERS] = [const { None }; WAITERS];
    let mut country: Option<Country> = None;
    if OFDM.load(Ordering::Relaxed) { radio.keep(false, true); mind::println!("[BCM] FRAMES WITH A BAD FCS OR PLCP KEPT, TO COUNT OFDM BY RATE AND LENGTH"); }
    let mut tally = Tally::new(radio);
    loop {
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, POLL_MS);
        radio.poll();
        tally.tick(radio);
        if radio.waiting() != 0 {
            for call in waiters.iter_mut().filter_map(Option::take) { let _ = wlanhw::reply_wait(call, radio.waiting() as u32); }
        }
        let Ok(request) = received else { continue };
        let (request, call) = match wlanhw::decode(&request, RECEIVED_CAP, &mut scratch) {
            Ok(decoded) => decoded,
            Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; }
        };
        let _ = match request {
            wlanhw::Request::Info => wlanhw::reply_info(call, Ok(&info(radio, country))),
            wlanhw::Request::SetChannel { channel } if !Country::allows(country, channel) => wlanhw::reply_set_channel(call, Err(wlanhw::Error::Invalid)),
            wlanhw::Request::SetChannel { channel } => wlanhw::reply_set_channel(call, radio.set_channel(channel).map_err(error)),
            wlanhw::Request::SetFilter { filter } => {
                let f = Filter { all_beacons: filter.all_beacons, bssid: mac(filter.bssid), own: mac(filter.own) };
                wlanhw::reply_set_filter(call, radio.set_filter(f).map_err(error))
            }
            wlanhw::Request::Send { frame, .. } if !Country::may_send(country, radio.channel(), frame) => {
                mind::println!("[BCM] FRAME REFUSED ON CHANNEL {}: THE COUNTRY'S LIMITS DO NOT LET IT GO OUT", radio.channel());
                wlanhw::reply_send(call, Err(wlanhw::Error::Invalid))
            }
            wlanhw::Request::Send { frame, rate, attempts } => {
                let sent = radio.send(frame, rate, attempts);
                if let Ok(s) = &sent { tally.sent(frame, rate, s); }
                let sent = sent.map(|s| wlanhw::Sent { acked: s.acked, attempts: s.attempts });
                wlanhw::reply_send(call, sent.as_ref().map_err(|&e| error(e)))
            }
            wlanhw::Request::Receive => match radio.take(&mut out) {
                Ok(n) => wlanhw::reply_receive(call, Ok(&out[..n])),
                Err(e) => wlanhw::reply_receive(call, Err(error(e))),
            },
            wlanhw::Request::Wait => {
                if radio.waiting() != 0 { wlanhw::reply_wait(call, radio.waiting() as u32) } else {
                    let mut call = call;
                    match waiters.iter_mut().find(|w| w.is_none()) {
                        Some(slot) if call.defer().is_ok() => { *slot = Some(call); Ok(()) }
                        _ => wlanhw::reply_wait(call, 0),
                    }
                }
            }
            // No shared ring yet: frames go by `receive`.
            wlanhw::Request::Attach { ring } => { let _ = mind::ipc::drop_cap(ring); wlanhw::reply_attach(call, Err(wlanhw::Error::Invalid)) }
            wlanhw::Request::Kick => wlanhw::reply_kick(call, Err(wlanhw::Error::Invalid)),
            wlanhw::Request::SetCountry { code, limits, .. } => match Country::from(code.as_str(), limits.as_slice()) {
                None => wlanhw::reply_set_country(call, Err(wlanhw::Error::Invalid)),
                Some(c) => {
                    country = Some(c);
                    let list = limits.as_slice();
                    mind::println!("[BCM] COUNTRY {}: {} CHANNELS, {} TO LISTEN ON ONLY; EIRP NOT APPLIED (THE TRANSMIT GAIN IS FIXED, NOT CALIBRATED IN DBM)",
                                   code.as_str(), list.len(), list.iter().filter(|l| l.listen).count());
                    // Off a channel the country does not allow, to its first one.
                    if !Country::allows(country, radio.channel()) {
                        if let Some(&first) = Country::list(country).as_slice().first() { let _ = radio.set_channel(first); }
                    }
                    wlanhw::reply_set_country(call, Ok(&info(radio, country)))
                }
            },
        };
    }
}

fn info(radio: &Radio, country: Option<Country>) -> wlanhw::Info {
    wlanhw::Info { mac: mac64(&radio.mac), bands: bands(), channels: Country::list(country), frame_max: FRAME_MAX as u16, channel: radio.channel() }
}

// No radio to serve (it did not come up): every request answered no-device, `wait` with none, so the station is not
// left waiting.
pub fn absent() -> ! {
    mind::println!("[BCM] THE RADIO DID NOT COME UP: idl/wlanhw.wit ANSWERED NO-DEVICE");
    let mut scratch = [0u8; wlanhw::REQUEST_MAX];
    loop {
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, 1000) else { continue };
        let (request, call) = match wlanhw::decode(&request, RECEIVED_CAP, &mut scratch) {
            Ok(decoded) => decoded,
            Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; }
        };
        let no = wlanhw::Error::NoDevice;
        let _ = match request {
            wlanhw::Request::Info => wlanhw::reply_info(call, Err(no)),
            wlanhw::Request::SetChannel { .. } => wlanhw::reply_set_channel(call, Err(no)),
            wlanhw::Request::SetFilter { .. } => wlanhw::reply_set_filter(call, Err(no)),
            wlanhw::Request::Send { .. } => wlanhw::reply_send(call, Err(no)),
            wlanhw::Request::Receive => wlanhw::reply_receive(call, Err(no)),
            wlanhw::Request::Wait => wlanhw::reply_wait(call, 0),
            wlanhw::Request::Attach { ring } => { let _ = mind::ipc::drop_cap(ring); wlanhw::reply_attach(call, Err(no)) }
            wlanhw::Request::Kick => wlanhw::reply_kick(call, Err(no)),
            wlanhw::Request::SetCountry { .. } => wlanhw::reply_set_country(call, Err(no)),
        };
    }
}

// While the test runs before the station is served (550-DRV-0029): its requests are answered at once, so that it and
// the stack waiting on it are not held up. `info` is answered as it is, the rest as if the radio were busy.
static HOLDING: AtomicBool = AtomicBool::new(false);
static HOLD_MAC: AtomicU64 = AtomicU64::new(0);

pub fn hold(own: [u8; 6]) { HOLD_MAC.store(mac64(&own), Ordering::Relaxed); HOLDING.store(true, Ordering::Relaxed); }

// Waits `ms`, answering the station's requests meanwhile when they are held.
pub fn pause(ms: usize) {
    if !HOLDING.load(Ordering::Relaxed) { if ms != 0 { mind::time::sleep(ms); } return; }
    let end = mind::time::uptime_ms() + ms;
    let mut scratch = [0u8; wlanhw::REQUEST_MAX];
    loop {
        let wait = end.saturating_sub(mind::time::uptime_ms()).clamp(1, 50) as u32;
        if let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, wait) { busy(&request, &mut scratch); }
        if mind::time::uptime_ms() >= end { return; }
    }
}

fn busy(request: &mind::ipc::Received, scratch: &mut [u8; wlanhw::REQUEST_MAX]) {
    let (request, call) = match wlanhw::decode(request, RECEIVED_CAP, scratch) {
        Ok(decoded) => decoded,
        Err(reason) => { if request.is_call { let _ = wire::reject(reason); } return; }
    };
    let busy = wlanhw::Error::Busy;
    let _ = match request {
        wlanhw::Request::Info => {
            let mut channels = List::default();
            for c in 1..=13 { channels.push(c); }
            wlanhw::reply_info(call, Ok(&wlanhw::Info { mac: HOLD_MAC.load(Ordering::Relaxed), bands: bands(), channels, frame_max: FRAME_MAX as u16, channel: 0 }))
        }
        wlanhw::Request::SetChannel { .. } => wlanhw::reply_set_channel(call, Err(busy)),
        wlanhw::Request::SetFilter { .. } => wlanhw::reply_set_filter(call, Err(busy)),
        wlanhw::Request::Send { .. } => wlanhw::reply_send(call, Err(busy)),
        wlanhw::Request::Receive => wlanhw::reply_receive(call, Err(wlanhw::Error::Empty)),
        wlanhw::Request::Wait => wlanhw::reply_wait(call, 0),
        wlanhw::Request::Attach { ring } => { let _ = mind::ipc::drop_cap(ring); wlanhw::reply_attach(call, Err(wlanhw::Error::Invalid)) }
        wlanhw::Request::Kick => wlanhw::reply_kick(call, Err(wlanhw::Error::Invalid)),
        wlanhw::Request::SetCountry { .. } => wlanhw::reply_set_country(call, Err(busy)),
    };
}
