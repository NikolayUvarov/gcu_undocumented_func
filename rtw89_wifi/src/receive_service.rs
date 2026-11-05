//! wlanhw.wit 2.0 served receive-only on the startup session, before any channel is calibrated (213-KRN-0123's gate).
//! Nothing is sent: `send` is `failed` until a country is applied to a calibrated channel, which this stage cannot do.

use crate::{board::Board, btc_session::{DeviceIo, Session, listen::{self, Cause}}, country::{Limits, Refusal}, download, frames,
    fwcmd::Queue, phy::Phy, phycap::Capabilities, post_mac, rx, rxdesc, station::Filter};
use mind::{idl::{codec::List, wire, wlanhw}, ipc::Endpoint, sys, time};

const RECEIVE: usize = 9;
const POLL_MS: u32 = 2;
const POLLS: usize = 8;
const WAIT_MS: u64 = 1000;
const WAITERS: usize = 4;
const FRAMES: usize = 4;
const FRAME_BYTES: usize = frames::HEADER_BYTES + frames::FRAME_MAX;

/// What the driver logs while it serves; the native entry prints them in the agreed words.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    /// `[RTW89] SERVING WLANHW: RECEIVE ONLY UNTIL A COUNTRY IS APPLIED`, on this channel.
    Serving { channel: u8 },
    /// A well-formed `set-country` taken but not applied, as no channel is calibrated yet.
    CountryTaken { code: [u8; 2], channels: usize, listen_only: usize },
}

#[derive(Debug)]
pub enum Stop { Setup(listen::Failure), Radio(Cause), Ipc(sys::Error), Clock }

struct Waiter { call: wire::Call, deadline: u64 }

fn address(value: u64) -> [u8; 6] { core::array::from_fn(|i| (value >> (40 - 8 * i)) as u8) }
fn packed(value: [u8; 6]) -> u64 { value.into_iter().fold(0, |word, byte| (word << 8) | u64::from(byte)) }

pub(crate) struct ReceiveOnly {
    mac: [u8; 6], channel: u8, filter: Filter, limits: Option<Limits>, settings: u64,
    frames: [[u8; FRAME_BYTES]; FRAMES], lengths: [usize; FRAMES], head: usize, count: usize, dropped: u64,
    waiters: [Option<Waiter>; WAITERS], last_ms: u64, failure: Option<Stop>,
}

impl ReceiveOnly {
    pub(crate) fn new(mac: [u8; 6], channel: u8) -> Self {
        Self { mac, channel, filter: Filter::new(mac, true, [0; 6], [0; 6]), limits: None, settings: 0,
            frames: [[0; FRAME_BYTES]; FRAMES], lengths: [0; FRAMES], head: 0, count: 0, dropped: 0,
            waiters: [const { None }; WAITERS], last_ms: time::monotonic_ns() / 1_000_000, failure: None }
    }
    #[cfg(test)]
    pub(crate) fn limits(&self) -> Option<&Limits> { self.limits.as_ref() }
    #[cfg(test)]
    pub(crate) fn dropped(&self) -> u64 { self.dropped }

    fn take(&mut self, channel: rx::Channel, packet: rxdesc::Packet, buffer: &[u8]) {
        if channel != rx::Channel::Rxq || packet.kind != rxdesc::TYPE_WIFI { return; }
        let Ok(frame) = frames::from_packet(packet, buffer) else { return };
        if !self.filter.accepts(frame.mpdu()) { return; }
        if self.count == FRAMES { self.dropped = self.dropped.saturating_add(1); return; }
        let at = (self.head + self.count) % FRAMES;
        if let Ok(length) = frame.write(self.channel, self.filter.all_beacons(), &mut self.frames[at]) {
            self.lengths[at] = length; self.count += 1;
        }
    }
    fn progress<B: DeviceIo>(&mut self, session: &mut Session<'_, '_, '_, B>, now: u64) -> Result<(), Stop> {
        for _ in 0..POLLS {
            let (received, _) = session.poll_frames(|c, p, b| self.take(c, p, b)).map_err(Stop::Radio)?;
            if received == [false, false] { break; }
        }
        let waiting = self.count as u32;
        for waiter in &mut self.waiters {
            if waiter.as_ref().is_some_and(|w| waiting != 0 || now >= w.deadline) {
                let _ = wlanhw::reply_wait(waiter.take().unwrap().call, waiting);
            }
        }
        Ok(())
    }
    fn fail(&mut self, stop: Stop) {
        if self.failure.is_none() { self.failure = Some(stop); }
        for waiter in &mut self.waiters { if let Some(waiter) = waiter.take() { let _ = wlanhw::reply_wait(waiter.call, 0); } }
    }
    fn info(&self) -> wlanhw::Info {
        // Receive-only on the one channel it is tuned to; its own channels are 1 to 13 at 20 MHz (wlanhw.wit 2.0).
        let mut channels = List::default(); channels.push(self.channel);
        let mut supported = List::default(); for ch in 1..=13 { supported.push(ch); }
        wlanhw::Info { mac: packed(self.mac), bands: mind::network::BAND_2GHZ | mind::network::BAND_2GHZ_OFDM,
            channels, supported, frame_max: frames::FRAME_MAX as u16, channel: self.channel }
    }

    /// One receive and one bounded RX step; continuous traffic on either side cannot starve the other.
    pub(crate) fn step<B: DeviceIo>(&mut self, session: &mut Session<'_, '_, '_, B>, events: &mut impl FnMut(Event)) -> bool {
        if self.failure.is_some() { return false; }
        let received = Endpoint::SERVICE.recv_timeout(RECEIVE, POLL_MS);
        let now = time::monotonic_ns() / 1_000_000;
        if now < self.last_ms { self.fail(Stop::Clock) } else { self.last_ms = now; if let Err(e) = self.progress(session, now) { self.fail(e) } }
        match received {
            Ok(request) => self.dispatch(request, now, events),
            Err(sys::Error::Other(code)) if code == mind::abi::ERR_TIMEOUT => {}
            Err(error) => self.fail(Stop::Ipc(error)),
        }
        self.failure.is_none()
    }
    fn dispatch(&mut self, received: mind::ipc::Received, now: u64, events: &mut impl FnMut(Event)) {
        let mut scratch = [0; wlanhw::REQUEST_MAX];
        let (request, call) = match wlanhw::decode(&received, RECEIVE, &mut scratch) {
            Ok(decoded) => decoded,
            Err(reason) => { if received.is_call { let _ = wire::reject(reason); } return }
        };
        if self.failure.is_some() { absent(request, call); return }
        match request {
            wlanhw::Request::Info => { let _ = wlanhw::reply_info(call, Ok(&self.info())); }
            wlanhw::Request::SetChannel { channel } => {
                let _ = wlanhw::reply_set_channel(call, if channel == self.channel { Ok(()) } else { Err(wlanhw::Error::Invalid) });
            }
            wlanhw::Request::SetFilter { filter } => {
                self.filter = Filter::new(self.mac, filter.all_beacons, address(filter.bssid), address(filter.own));
                self.head = 0; self.count = 0;
                let _ = wlanhw::reply_set_filter(call, Ok(()));
            }
            // No calibrated channel, so no transmission of any kind.
            wlanhw::Request::Send { .. } => { let _ = wlanhw::reply_send(call, Err(wlanhw::Error::Failed)); }
            wlanhw::Request::Receive => {
                if self.count == 0 { let _ = wlanhw::reply_receive(call, Err(wlanhw::Error::Empty)); return }
                let at = self.head;
                let _ = wlanhw::reply_receive(call, Ok(&self.frames[at][..self.lengths[at]]));
                self.head = (at + 1) % FRAMES; self.count -= 1;
            }
            wlanhw::Request::Wait => {
                let count = self.count as u32;
                if count != 0 { let _ = wlanhw::reply_wait(call, count); return }
                let Some(deadline) = now.checked_add(WAIT_MS) else { let _ = wlanhw::reply_wait(call, 0); self.fail(Stop::Clock); return };
                let mut call = call;
                match self.waiters.iter_mut().find(|entry| entry.is_none()) {
                    Some(entry) if call.defer().is_ok() => *entry = Some(Waiter { call, deadline }),
                    _ => { let _ = wlanhw::reply_wait(call, 0); }
                }
            }
            wlanhw::Request::Attach { ring } => { let _ = mind::ipc::drop_cap(ring); let _ = wlanhw::reply_attach(call, Err(wlanhw::Error::Invalid)); }
            wlanhw::Request::Kick => { let _ = wlanhw::reply_kick(call, Err(wlanhw::Error::Invalid)); }
            wlanhw::Request::SetCountry { code, limits, .. } => {
                self.settings = self.settings.saturating_add(1);
                let pairs = limits.as_slice().iter().map(|l| (l.channel, l.eirp, l.listen));
                let reply = match Limits::accept(code.as_str(), self.settings, pairs) {
                    Ok(taken) => {
                        let listen_only = taken.channels().filter(|&ch| taken.applied(ch).is_some_and(|a| a.listen)).count();
                        events(Event::CountryTaken { code: taken.code(), channels: taken.channels().count(), listen_only });
                        self.limits = Some(taken);
                        // Taken, not applied: 1.2's `failed` keeps the station from sending until a calibrated channel exists.
                        Err(wlanhw::Error::Failed)
                    }
                    Err(Refusal::Invalid) => Err(wlanhw::Error::Invalid),
                    Err(Refusal::Failed) => Err(wlanhw::Error::Failed),
                };
                let info = self.info();
                let _ = wlanhw::reply_set_country(call, reply.map(|()| &info));
            }
        }
    }
}

/// Every request answered `no-device`: a radio that stopped still answers its station.
fn absent(request: wlanhw::Request<'_>, call: wire::Call) {
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

/// After a stop: answer `no-device` for as long as the process lives.
pub fn absent_forever() -> ! {
    loop {
        let Ok(received) = Endpoint::SERVICE.recv_timeout(RECEIVE, 60_000) else { continue };
        let mut scratch = [0; wlanhw::REQUEST_MAX];
        match wlanhw::decode(&received, RECEIVE, &mut scratch) {
            Ok((request, call)) => absent(request, call),
            Err(reason) => { if received.is_call { let _ = wire::reject(reason); } }
        }
    }
}

/// Consume this successful firmware startup: initial PHY, one receive-only channel, then wlanhw served until a stop.
/// Slot 9 must be empty; the endpoint is the station's alone. The caller stops RX/WCPU and retains DMA afterwards.
pub fn serve<B: DeviceIo>(bus: &mut B, queue: &mut Queue<'_>, rings: &mut rx::Rings<'_>, receiving: &post_mac::Receiving,
    firmware: download::Ready, board: &Board, caps: &Capabilities, scratch: &mut [u8], center_mhz: u16,
    mut events: impl FnMut(Event)) -> Stop {
    if mind::dev::cap_info(RECEIVE).0 != mind::abi::CAP_KIND_NONE { return Stop::Ipc(sys::Error::NoSlot); }
    let (mut session, tables, policy) = match listen::open(bus, queue, rings, receiving, firmware, board, caps, scratch, center_mhz) {
        Ok(opened) => opened, Err(failure) => return Stop::Setup(failure),
    };
    let mut phy = Phy::new(&mut session, board.identity);
    let channel = match listen::prepare_receive(&mut phy, board, caps, &tables, &policy) {
        Ok(ready) => ready.channel, Err(failure) => return Stop::Setup(failure),
    };
    let session = phy.bus_between_stages();
    let mut service = ReceiveOnly::new(board.identity.mac, channel);
    events(Event::Serving { channel });
    while service.step(session, &mut events) {}
    service.failure.take().unwrap_or(Stop::Clock)
}
