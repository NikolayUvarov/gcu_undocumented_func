// The radio as a service (550-DRV-0027): idl/wlanhw.wit 1.2 served on the service endpoint, which init lends to the
// station `wlan` alone (MC-2.3). Received frames are polled from the ring every few milliseconds; `wait` answers when
// one is queued. No shared ring yet: frames go by `receive`, as the station takes them.
use crate::radio::{Error, Filter, Radio, FRAME_MAX, RADIO_HEADER};
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
const BANDS: u8 = BAND_2GHZ | BAND_2GHZ_OFDM;
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
    loop {
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, POLL_MS);
        radio.poll();
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
                let sent = radio.send(frame, rate, attempts).map(|s| wlanhw::Sent { acked: s.acked, attempts: s.attempts });
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
    wlanhw::Info { mac: mac64(&radio.mac), bands: BANDS, channels: Country::list(country), frame_max: FRAME_MAX as u16, channel: radio.channel() }
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
            wlanhw::reply_info(call, Ok(&wlanhw::Info { mac: HOLD_MAC.load(Ordering::Relaxed), bands: BANDS, channels, frame_max: FRAME_MAX as u16, channel: 0 }))
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
