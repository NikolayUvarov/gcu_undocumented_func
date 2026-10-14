// The radio as a service (550-DRV-0027): idl/wlanhw.wit 1.0 served on the service endpoint, which init lends to the
// station `wlan` alone (MC-2.3). Received frames are polled from the ring every few milliseconds; `wait` answers when
// one is queued. No shared ring yet: frames go by `receive`, as the station takes them.
use crate::radio::{Error, Filter, Radio, FRAME_MAX, RADIO_HEADER};
use mind::idl::codec::List;
use mind::idl::{wire, wlanhw};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use mind::ipc::Endpoint;

// The slot a request's capability lands in; the receive timeout, which is also the polling period; deferred waits.
const RECEIVED_CAP: usize = 9;
const POLL_MS: u32 = 2;
const WAITERS: usize = 4;

fn error(e: Error) -> wlanhw::Error {
    match e { Error::NoDevice => wlanhw::Error::NoDevice, Error::Empty => wlanhw::Error::Empty, Error::Invalid => wlanhw::Error::Invalid, Error::Failed => wlanhw::Error::Failed }
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
            wlanhw::Request::Info => {
                let mut channels = List::default();
                for c in 1..=13 { channels.push(c); }
                wlanhw::reply_info(call, Ok(&wlanhw::Info { mac: mac64(&radio.mac), bands: 1, channels, frame_max: FRAME_MAX as u16, channel: radio.channel() }))
            }
            wlanhw::Request::SetChannel { channel } => wlanhw::reply_set_channel(call, radio.set_channel(channel).map_err(error)),
            wlanhw::Request::SetFilter { filter } => {
                let f = Filter { all_beacons: filter.all_beacons, bssid: mac(filter.bssid), own: mac(filter.own) };
                wlanhw::reply_set_filter(call, radio.set_filter(f).map_err(error))
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
        };
    }
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
            wlanhw::reply_info(call, Ok(&wlanhw::Info { mac: HOLD_MAC.load(Ordering::Relaxed), bands: 1, channels, frame_max: FRAME_MAX as u16, channel: 0 }))
        }
        wlanhw::Request::SetChannel { .. } => wlanhw::reply_set_channel(call, Err(busy)),
        wlanhw::Request::SetFilter { .. } => wlanhw::reply_set_filter(call, Err(busy)),
        wlanhw::Request::Send { .. } => wlanhw::reply_send(call, Err(busy)),
        wlanhw::Request::Receive => wlanhw::reply_receive(call, Err(wlanhw::Error::Empty)),
        wlanhw::Request::Wait => wlanhw::reply_wait(call, 0),
        wlanhw::Request::Attach { ring } => { let _ = mind::ipc::drop_cap(ring); wlanhw::reply_attach(call, Err(wlanhw::Error::Invalid)) }
        wlanhw::Request::Kick => wlanhw::reply_kick(call, Err(wlanhw::Error::Invalid)),
    };
}
