// The radio as a service: idl/wlanhw.wit 1.2 on the service endpoint, which init lends to the station `wlan` alone
// (MC-2.3), as bcm_wifi serves it (550-DRV-0027). Received frames are polled from the ring every few milliseconds;
// `wait` answers when one is queued. No shared ring: frames go by `receive`.
use rtw88_wifi::radio::{Fail, Radio, FRAME_MAX, RADIO_HEADER};
use mind::idl::codec::List;
use mind::idl::{wire, wlanhw};
use mind::ipc::Endpoint;
use mind::network::{BAND_2GHZ, BAND_2GHZ_OFDM};
use rtw88_wifi::io::Regs;

const RECEIVED_CAP: usize = 9;
const POLL_MS: u32 = 2;
const WAITERS: usize = 4;
const CHANNELS: core::ops::RangeInclusive<u8> = 1..=13;
const PROBE_REQUEST: u8 = 0x40;
/// How often the log says what moved.
const TALLY_MS: u64 = 10_000;

// A country's channels and those to listen on only, as bit masks; None until first set (wlanhw 1.1's ways).
#[derive(Clone, Copy)]
struct Country { channels: u16, listen: u16 }

impl Country {
    fn allows(c: Option<Country>, ch: u8) -> bool { c.map_or(CHANNELS.contains(&ch), |c| c.channels >> ch & 1 != 0) }
    fn may_send(c: Option<Country>, ch: u8, frame: &[u8]) -> bool {
        let Some(c) = c else { return true };
        c.channels >> ch & 1 != 0 && !(c.listen >> ch & 1 != 0 && frame.first() == Some(&PROBE_REQUEST))
    }
    fn list(c: Option<Country>) -> List<u8, 64> {
        let mut l = List::default();
        for ch in CHANNELS.filter(|&ch| Country::allows(c, ch)) { l.push(ch); }
        l
    }
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

fn error(e: Fail) -> wlanhw::Error {
    match e { Fail::NoDevice => wlanhw::Error::NoDevice, Fail::Busy => wlanhw::Error::Busy, Fail::Empty => wlanhw::Error::Empty, Fail::Invalid => wlanhw::Error::Invalid, Fail::Failed => wlanhw::Error::Failed }
}

fn mac64(m: &[u8; 6]) -> u64 { m.iter().fold(0, |v, &b| v << 8 | b as u64) }
fn mac(v: u64) -> [u8; 6] { core::array::from_fn(|i| (v >> (40 - 8 * i)) as u8) }

fn info<R: Regs>(radio: &Radio<R>, country: Option<Country>) -> wlanhw::Info {
    wlanhw::Info { mac: mac64(&radio.mac), bands: BAND_2GHZ | BAND_2GHZ_OFDM, channels: Country::list(country), frame_max: FRAME_MAX as u16, channel: radio.channel() }
}

pub fn run<R: Regs>(radio: &mut Radio<R>) -> ! {
    let m = radio.mac;
    mind::println!("[RTW88] SERVING idl/wlanhw.wit {}.{}.{}: MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}, CHANNELS 1-13, 802.11B/G",
                   wlanhw::VERSION.0, wlanhw::VERSION.1, wlanhw::VERSION.2, m[0], m[1], m[2], m[3], m[4], m[5]);
    let mut scratch = [0u8; wlanhw::REQUEST_MAX];
    let mut out = [0u8; RADIO_HEADER + FRAME_MAX];
    let mut waiters: [Option<wire::Call>; WAITERS] = [const { None }; WAITERS];
    let mut country: Option<Country> = None;
    let mut tally_at = mind::time::uptime_ms() as u64 + TALLY_MS;
    let mut last = radio.counts;
    loop {
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, POLL_MS);
        radio.poll();
        let now = mind::time::uptime_ms() as u64;
        if now >= tally_at {
            let k = radio.counts;
            if k.frames != last.frames || k.sent != last.sent {
                mind::println!("[RTW88] CH {}: {} BUFFERS ({} KEPT, {} BAD FCS, {} FIRMWARE, {} DROPPED); SENT {} ({} ACKED, {} WITHOUT A REPORT)",
                    radio.channel(), k.frames - last.frames, k.kept - last.kept, k.crc - last.crc, k.c2h - last.c2h, k.dropped - last.dropped,
                    k.sent - last.sent, k.acked - last.acked, k.no_report - last.no_report);
            }
            (last, tally_at) = (k, now + TALLY_MS);
        }
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
                radio.set_filter(filter.all_beacons, mac(filter.bssid), mac(filter.own));
                wlanhw::reply_set_filter(call, Ok(()))
            }
            wlanhw::Request::Send { frame, .. } if !Country::may_send(country, radio.channel(), frame) => wlanhw::reply_send(call, Err(wlanhw::Error::Invalid)),
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
            wlanhw::Request::Attach { ring } => { let _ = mind::ipc::drop_cap(ring); wlanhw::reply_attach(call, Err(wlanhw::Error::Invalid)) }
            wlanhw::Request::Kick => wlanhw::reply_kick(call, Err(wlanhw::Error::Invalid)),
            wlanhw::Request::SetCountry { code, limits, .. } => match Country::from(code.as_str(), limits.as_slice()) {
                None => wlanhw::reply_set_country(call, Err(wlanhw::Error::Invalid)),
                Some(c) => {
                    country = Some(c);
                    mind::println!("[RTW88] COUNTRY {}: {} CHANNELS, {} TO LISTEN ON ONLY; TX POWER UNDER THE WORLD-WIDE LIMITS, EIRP NOT APPLIED",
                                   code.as_str(), limits.as_slice().len(), limits.as_slice().iter().filter(|l| l.listen).count());
                    if !Country::allows(country, radio.channel()) {
                        if let Some(&first) = Country::list(country).as_slice().first() { let _ = radio.set_channel(first); }
                    }
                    wlanhw::reply_set_country(call, Ok(&info(radio, country)))
                }
            },
        };
    }
}

/// No radio to serve: every request answered no-device, so the station is not left waiting.
pub fn absent() -> ! {
    mind::println!("[RTW88] THE RADIO DID NOT COME UP: idl/wlanhw.wit ANSWERED NO-DEVICE");
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
