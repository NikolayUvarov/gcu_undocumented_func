//! wlanhw.wit 2.0 served receive-only on the startup session, before any channel is calibrated (213-KRN-0123's gate).
//! Every channel is tuned passively, so the station's first scan hears them all. Nothing is sent until a country is
//! applied: the first `send` on a channel it allows calibrates that channel and hands the endpoint to the data service.

use crate::{args::Platform, board::Board, btc_session::{DeviceIo, Inbox, Session, connection, data_mode, listen::{self, Cause}},
    channel_power::Constraints, channel_rfk, country::{Limits, Refusal}, download::{self, DefaultTables}, frames, initial_phy,
    native_dma::Queues, phy::{Phy, channel::{self, Policy, Rule, calibration_policy::{PlatformInput, Resolved}}},
    phycap::Capabilities, post_mac, rx, rxdesc, service, station::{self, Filter}};
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
    /// A well-formed `set-country` taken; `applied` unless the platform's power policy is unknown (213-KRN-0122).
    CountryTaken { code: [u8; 2], channels: usize, listen_only: usize, applied: bool },
    /// `[RTW89] DATAPATH ADMITTED ON CHANNEL n`: calibrated under the station's country, and served for data.
    Admitted { channel: u8 },
}

#[derive(Debug)]
pub enum Stop { Setup(listen::Failure), Radio(Cause), Ipc(sys::Error), Clock, Retune(Retune), Admission(Admission), Data(Data) }

// The details below are read by Debug alone: the native entry prints the stop.
/// A passive tune to `channel` that failed; `failure` is None when no policy exists for it.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Retune { pub channel: u8, failure: Option<channel::Failure> }
/// The admission of `channel` that failed, at the stage `failure` names.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Admission { pub channel: u8, failure: connection::Failure }
/// The data service's end: its failure, if any, and the data owner's shutdown.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Data { failure: Option<service::Fatal>, shutdown: data_mode::Shutdown }

struct Waiter { call: wire::Call, deadline: u64 }

/// What a request asks of the radio beyond its reply: a passive tune, answered after it, or the admission of a channel.
pub(crate) enum Ask { Tune(u8, wire::Call), Admit(u8) }

fn address(value: u64) -> [u8; 6] { core::array::from_fn(|i| (value >> (40 - 8 * i)) as u8) }
fn packed(value: [u8; 6]) -> u64 { value.into_iter().fold(0, |word, byte| (word << 8) | u64::from(byte)) }

pub(crate) struct ReceiveOnly {
    mac: [u8; 6], channel: u8, filter: Filter, limits: Option<Limits>, settings: u64, admits: bool, ask: Option<Ask>,
    frames: [[u8; FRAME_BYTES]; FRAMES], lengths: [usize; FRAMES], head: usize, count: usize, dropped: u64,
    waiters: [Option<Waiter>; WAITERS], last_ms: u64, failure: Option<Stop>,
}

impl ReceiveOnly {
    /// `admits`: the platform's power policy is known (213-KRN-0122), so a country can be applied.
    pub(crate) fn new(mac: [u8; 6], channel: u8, admits: bool) -> Self {
        Self { mac, channel, filter: Filter::new(mac, true, [0; 6], [0; 6]), limits: None, settings: 0, admits, ask: None,
            frames: [[0; FRAME_BYTES]; FRAMES], lengths: [0; FRAMES], head: 0, count: 0, dropped: 0,
            waiters: [const { None }; WAITERS], last_ms: time::monotonic_ns() / 1_000_000, failure: None }
    }
    pub(crate) fn limits(&self) -> Option<&Limits> { self.limits.as_ref() }
    #[cfg(test)]
    pub(crate) fn dropped(&self) -> u64 { self.dropped }
    pub(crate) fn take_ask(&mut self) -> Option<Ask> { self.ask.take() }
    /// The passive tune `Ask::Tune` asked for is done: frames heard before it are of another channel.
    pub(crate) fn tuned(&mut self, channel: u8) { self.channel = channel; self.head = 0; self.count = 0; }
    /// Before the data service takes the endpoint: every waiter answered, no frame kept.
    pub(crate) fn hand_over(&mut self) {
        for waiter in &mut self.waiters { if let Some(waiter) = waiter.take() { let _ = wlanhw::reply_wait(waiter.call, 0); } }
        self.head = 0; self.count = 0;
    }
    // A country is applied, and it lets this radio send on `channel`.
    fn sends_on(&self, channel: u8) -> bool {
        self.admits && self.limits.as_ref().and_then(|l| l.applied(channel)).is_some_and(|a| !a.listen)
    }

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
        // Its own channels are 1 to 13 at 20 MHz (wlanhw.wit 2.0), each tuned passively; an applied country leaves its own.
        let mut channels = List::default();
        match &self.limits { Some(limits) if self.admits => for ch in limits.channels() { channels.push(ch); },
            _ => for ch in 1..=13 { channels.push(ch); } }
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
                if channel == self.channel { let _ = wlanhw::reply_set_channel(call, Ok(())); return }
                let left = self.limits.as_ref().filter(|_| self.admits).is_none_or(|l| l.applied(channel).is_some());
                if !(1..=13).contains(&channel) || !left || self.ask.is_some() {
                    let _ = wlanhw::reply_set_channel(call, Err(wlanhw::Error::Invalid)); return
                }
                // Answered after the passive tune, which the serving loop runs.
                let mut call = call;
                if call.defer().is_err() { let _ = wlanhw::reply_set_channel(call, Err(wlanhw::Error::Busy)); return }
                self.ask = Some(Ask::Tune(channel, call));
            }
            wlanhw::Request::SetFilter { filter } => {
                self.filter = Filter::new(self.mac, filter.all_beacons, address(filter.bssid), address(filter.own));
                self.head = 0; self.count = 0;
                let _ = wlanhw::reply_set_filter(call, Ok(()));
            }
            // No calibrated channel, so nothing is sent. Under a country that lets it send here, the first `send` admits this
            // channel: `busy` now, and the station's next try goes out through the data service.
            wlanhw::Request::Send { .. } => {
                let admit = self.sends_on(self.channel) && self.ask.is_none();
                let _ = wlanhw::reply_send(call, Err(if admit { wlanhw::Error::Busy } else { wlanhw::Error::Failed }));
                if admit { self.ask = Some(Ask::Admit(self.channel)); }
            }
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
                        events(Event::CountryTaken { code: taken.code(), channels: taken.channels().count(), listen_only,
                            applied: self.admits });
                        self.limits = Some(taken);
                        // Applied: each channel is calibrated when the station first sends there. With the platform's power
                        // policy unknown, 1.2's `failed` keeps the station listening.
                        if self.admits { Ok(()) } else { Err(wlanhw::Error::Failed) }
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

// 213-KRN-0122's word as the calibration policy takes it.
fn platform_input(platform: Platform<'_>) -> PlatformInput {
    match platform {
        Platform::Absent => PlatformInput::CheckedAbsent,
        Platform::Limits { per_ntx, per_path } => PlatformInput::Limits(Constraints { per_ntx, per_path }),
        Platform::Unknown(_) => PlatformInput::Unknown,
    }
}

/// Admission up to the data mode, on the live PHY of `prepare_receive`, as connection::run's order: the station's country
/// on `channel`, tuned to initiate radiation, granted, calibrated (IQK, TSSI, DPK) and its station prepared.
pub(crate) fn calibrate<B: DeviceIo>(phy: &mut Phy<'_, Session<'_, '_, '_, B>>, board: &Board, caps: &Capabilities,
    tables: &DefaultTables, prepared: &initial_phy::Prepared, limits: &Limits, channel: u8, platform: PlatformInput)
    -> Result<(channel::Tuned, channel_rfk::Completed, station::Prepared), connection::Failure> {
    use connection::Failure as F;
    let applied = limits.applied(channel).filter(|a| !a.listen)
        .ok_or(F::Admission(channel::calibration_policy::Error::Passive))?;
    let rule = Rule::station(limits.code(), limits.version(), channel, applied.eirp_half_dbm, false);
    let resolved = Resolved::resolve(board, rule, platform).map_err(F::Admission)?;
    let mut tuned = channel::run(phy, board, &prepared.prefix, &prepared.paths, tables, resolved.policy()).map_err(F::Channel)?;
    let [iqk, tssi, dpk] = resolved.grant(phy, board, tables, &mut tuned, &prepared.owner).map_err(F::Admission)?;
    let calibrated = channel_rfk::run(phy, board, &prepared.prefix, &tuned, &prepared.owner,
        channel_rfk::Authorities { iqk, tssi, dpk }).map_err(F::Calibration)?;
    let mut station = station::prepare(phy, board, &tuned, &calibrated, &prepared.owner, caps).map_err(F::Station)?;
    let filter = station.filter(true, [0; 6], [0; 6]);
    station.configure_filter(phy, board, &tuned, &prepared.owner, filter).map_err(F::Station)?;
    Ok((tuned, calibrated, station))
}

/// Consume this successful firmware startup: initial PHY, a receive-only channel tuned passively wherever the station
/// asks, then, once its country lets it send, one admitted channel served for data until a stop.
/// Slot 9 must be empty; the endpoint is the station's alone. `inbox` holds the data mode's packets. The caller stops
/// RX/WCPU and retains DMA afterwards.
pub fn serve<B: DeviceIo>(bus: &mut B, queues: &mut Queues<'_>, receiving: &post_mac::Receiving,
    firmware: download::Ready, board: &Board, caps: &Capabilities, scratch: &mut [u8], inbox: &mut Inbox, center_mhz: u16,
    platform: Platform<'_>, mut events: impl FnMut(Event)) -> Stop {
    if mind::dev::cap_info(RECEIVE).0 != mind::abi::CAP_KIND_NONE { return Stop::Ipc(sys::Error::NoSlot); }
    let platform = platform_input(platform);
    let Queues { command, receive, transmit, .. } = queues;
    let (mut session, tables, policy) = match listen::open(bus, command, receive, receiving, firmware, board, caps, scratch, center_mhz) {
        Ok(opened) => opened, Err(failure) => return Stop::Setup(failure),
    };
    let mut phy = Phy::new(&mut session, board.identity);
    let listen::Receiving { channel, prepared } = match listen::prepare_receive(&mut phy, board, caps, &tables, &policy) {
        Ok(ready) => ready, Err(failure) => return Stop::Setup(failure),
    };
    let mut service = ReceiveOnly::new(board.identity.mac, channel, platform != PlatformInput::Unknown);
    events(Event::Serving { channel });
    let admit = loop {
        if !service.step(phy.bus_between_stages(), &mut events) { return service.failure.take().unwrap_or(Stop::Clock) }
        match service.take_ask() {
            None => {}
            Some(Ask::Tune(channel, call)) => {
                let tuned = Policy::passive(board, 2407 + 5 * u16::from(channel)).map_err(|_| None)
                    .and_then(|policy| channel::run(&mut phy, board, &prepared.prefix, &prepared.paths, &tables, &policy).map_err(Some));
                match tuned {
                    Ok(_) => { service.tuned(channel); let _ = wlanhw::reply_set_channel(call, Ok(())); }
                    Err(failure) => {
                        let _ = wlanhw::reply_set_channel(call, Err(wlanhw::Error::Failed));
                        service.hand_over();
                        return Stop::Retune(Retune { channel, failure });
                    }
                }
            }
            Some(Ask::Admit(channel)) => break channel,
        }
    };
    service.hand_over();
    let failed = |failure| Stop::Admission(Admission { channel: admit, failure });
    let Some(limits) = service.limits() else { return Stop::Clock };
    let (tuned, calibrated, station) = match calibrate(&mut phy, board, caps, &tables, &prepared, limits, admit, platform) {
        Ok(done) => done, Err(failure) => return failed(failure),
    };
    if let Err(error) = phy.bus_between_stages().attach_inbox(inbox) { return Stop::Radio(Cause::Session(error)) }
    let activation = match data_mode::prepare(&mut phy, board, &tuned, &calibrated, &prepared.owner, station, transmit) {
        Ok(activation) => activation, Err(failure) => return failed(connection::Failure::Station(failure)),
    };
    drop(phy);
    let mut data = match connection::activate(session, transmit, activation) { Ok((data, _)) => data, Err(failure) => return failed(failure) };
    events(Event::Admitted { channel: admit });
    let stop = match service::Service::new(&mut data) {
        Ok(served) => { let stopped = served.run(); Stop::Data(Data { failure: stopped.failure, shutdown: stopped.shutdown }) }
        Err(error) => Stop::Ipc(error),
    };
    if matches!(stop, Stop::Ipc(_)) { data.shutdown(); }
    stop
}
