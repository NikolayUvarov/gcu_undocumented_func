//! Receive-only listen on one 2.4 GHz channel after the initial PHY; no calibration, station role or packet TX.

use super::{DeviceIo, PacketHandler, Session};
use crate::{board::Board, btc_context, btc_session, download::{self, DefaultTables}, frames, fwcmd::Queue, initial_phy,
    io::{self, Fault, FaultKind, RegisterIo}, phy::{Phy, channel::{self, Policy}}, phycap::Capabilities, post_mac, report_tail, rx, rxdesc};

pub const HEARD_MAX: usize = 32;
const POLL_US: u64 = 20_000;
const IDLE_US: u32 = 200;
const EVENTS_US: u32 = 10_000;
const RX_FILTER: usize = 0xce20;
// FIF_BCN_PRBRESP_PROMISC as mac80211 sets it for a scan (mac80211.c:304–378): beacon, broadcast and A1 checks off.
pub const SCAN_OPEN: u32 = 1 << 7 | 1 << 2 | 1 << 1;

/// One transmitter heard in beacons or probe responses; raw SSID bytes, DS channel zero when absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Heard { pub bssid: [u8; 6], pub ds_channel: u8, ssid: [u8; 32], ssid_len: u8, pub frames: u32 }
impl Heard { pub fn ssid(&self) -> &[u8] { &self.ssid[..usize::from(self.ssid_len)] } }

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Tally {
    pub frames: u32, pub beacons: u32, pub probe_responses: u32, pub other_frames: u32,
    pub rejected: u32, pub unlisted: u32, pub other_packets: u32, pub btc_events: u32,
}

#[derive(Debug)]
pub struct Listened {
    pub initial: initial_phy::Observations, pub channel: u8, pub elapsed_us: u64,
    pub tally: Tally, pub heard: usize, pub counts: btc_session::Counts,
}

#[derive(Debug)]
pub enum Cause {
    Capabilities, Policy, Session(btc_session::Error), Initial(initial_phy::Cause),
    Channel { stage: &'static str, cause: Fault, scheduler_restore: Option<Fault>, radio_cleanup: Option<Fault>, radio_stop: Option<Fault> },
    Filter(btc_session::Error), Listen(btc_session::Error), Events(btc_context::Error),
}

#[derive(Debug)]
pub struct Failure { pub cause: Cause, pub coexistence: Option<btc_context::RfkFailure>, pub tally: Tally }

fn channel_cause(error: channel::Failure) -> Cause {
    use channel::Stage;
    let stage = match error.stage {
        Stage::Guard => "guard", Stage::Pause => "pause", Stage::Prepare => "prepare", Stage::Mac => "mac", Stage::Baseband => "baseband",
        Stage::Radio => "radio", Stage::Power => "power", Stage::Finish => "finish", Stage::Restore => "restore", Stage::Verify => "verify",
    };
    Cause::Channel { stage, cause: error.cause, scheduler_restore: error.scheduler_restore, radio_cleanup: error.radio_cleanup, radio_stop: error.radio_stop }
}

// Fixed beacon/probe-response fields precede the elements (802.11-2020 9.3.3.3); a truncated tail ends the walk.
fn parse(mpdu: &[u8]) -> Option<Heard> {
    let bssid = mpdu.get(16..22)?.try_into().ok()?;
    let header = if mpdu[1] & 0x80 != 0 { 28 } else { 24 };
    let mut heard = Heard { bssid, ds_channel: 0, ssid: [0; 32], ssid_len: 0, frames: 1 };
    let mut elements = mpdu.get(header + 12..)?;
    while let [id, length, rest @ ..] = elements {
        let Some(body) = rest.get(..usize::from(*length)) else { break };
        match (*id, body.len()) {
            (0, 0..=32) => { heard.ssid[..body.len()].copy_from_slice(body); heard.ssid_len = body.len() as u8; }
            (3, 1) => heard.ds_channel = body[0],
            _ => (),
        }
        elements = &rest[body.len()..];
    }
    Some(heard)
}

struct Listener<F> { table: [Option<Heard>; HEARD_MAX], tally: Tally, first: F }
impl<F: FnMut(&Heard)> Listener<F> {
    fn take(&mut self, channel: rx::Channel, packet: rxdesc::Packet, buffer: &[u8]) {
        let tally = &mut self.tally;
        if channel != rx::Channel::Rxq || packet.kind != rxdesc::TYPE_WIFI { tally.other_packets = tally.other_packets.saturating_add(1); return; }
        let Ok(frame) = frames::from_packet(packet, buffer) else { tally.rejected = tally.rejected.saturating_add(1); return };
        tally.frames = tally.frames.saturating_add(1);
        let mpdu = frame.mpdu();
        match mpdu[0] {
            0x80 => tally.beacons = tally.beacons.saturating_add(1),
            0x50 => tally.probe_responses = tally.probe_responses.saturating_add(1),
            _ => { tally.other_frames = tally.other_frames.saturating_add(1); return; }
        }
        let Some(found) = parse(mpdu) else { tally.rejected = tally.rejected.saturating_add(1); return };
        if let Some(known) = self.table.iter_mut().flatten().find(|heard| heard.bssid == found.bssid) {
            known.frames = known.frames.saturating_add(1);
        } else if let Some(slot) = self.table.iter_mut().find(|heard| heard.is_none()) {
            *slot = Some(found); (self.first)(&found);
        } else { tally.unlisted = tally.unlisted.saturating_add(1); }
    }
}

struct Each<F>(F);
impl<F: FnMut(rx::Channel, rxdesc::Packet, &[u8])> PacketHandler for Each<F> {
    fn packet(&mut self, _: u64, channel: rx::Channel, packet: rxdesc::Packet, buffer: &[u8], _: bool) -> Result<bool, btc_session::Error> {
        (self.0)(channel, packet, buffer); Ok(true)
    }
}

impl<B: DeviceIo> Session<'_, '_, '_, B> {
    /// At most one packet per hardware channel to `each`, after the startup permit's re-check; then the BTC events
    /// retained meanwhile are observed. Returns which channels gave a packet and how many events there were.
    pub(crate) fn poll_frames(&mut self, each: impl FnMut(rx::Channel, rxdesc::Packet, &[u8])) -> Result<([bool; 2], usize), Cause> {
        let now = self.clock().map_err(Cause::Listen)?;
        let received = self.poll_before_with(now + POLL_US, &mut Each(each)).map_err(Cause::Listen)?;
        let pending = self.pending_events();
        if pending != 0 { self.observe_events(EVENTS_US).map_err(Cause::Events)?; }
        Ok((received, pending))
    }
    /// A short wait between empty polls, inside the session's register window.
    pub(crate) fn idle(&mut self) -> Result<(), Cause> {
        let now = self.clock().map_err(Cause::Listen)?;
        self.registers(now + POLL_US, |window| window.delay_us(IDLE_US)).map_err(Cause::Listen)
    }
}

/// The startup session for one receive-only listen or service, its default tables and its passive policy.
/// A frequency outside channels 1–13 is refused here, before any command.
pub(crate) fn open<'a, 'c, 'r, B: DeviceIo>(bus: &'a mut B, queue: &'a mut Queue<'c>, rings: &'a mut rx::Rings<'r>,
    receiving: &'a post_mac::Receiving, firmware: download::Ready, board: &Board, caps: &Capabilities, scratch: &'a mut [u8],
    center_mhz: u16) -> Result<(Session<'a, 'c, 'r, B>, DefaultTables, Policy), Failure> {
    let fail = |cause| Failure { cause, coexistence: None, tally: Tally::default() };
    if !caps.belongs_to(&firmware) { return Err(fail(Cause::Capabilities)); }
    let policy = Policy::passive(board, center_mhz).map_err(|_| fail(Cause::Policy))?;
    let tables = firmware.default_tables();
    let session = Session::new(bus, queue, rings, receiving, firmware, scratch).map_err(|e| fail(Cause::Session(e)))?;
    Ok((session, tables, policy))
}

/// What `prepare_receive` leaves on the caller's live PHY: the initial PHY's state and the receive-only channel.
pub(crate) struct Receiving { pub channel: u8, pub(crate) prepared: initial_phy::Prepared }

/// Initial PHY, one receive-only channel, the report tail and the scan filter. The PHY stays the caller's, so that a
/// later calibration continues this attempt; the scheduler and data DMA stay stopped.
pub(crate) fn prepare_receive<B: DeviceIo>(phy: &mut Phy<'_, Session<'_, '_, '_, B>>, board: &Board, caps: &Capabilities,
    tables: &DefaultTables, policy: &Policy) -> Result<Receiving, Failure> {
    let fail = |cause| Failure { cause, coexistence: None, tally: Tally::default() };
    let mut prepared = initial_phy::prepare(phy, board, caps).map_err(|cause| Failure {
        cause: Cause::Initial(cause), coexistence: btc_context::last_rfk_failure(phy.bus_between_stages()), tally: Tally::default() })?;
    let tuned = channel::run(phy, board, &prepared.prefix, &prepared.paths, tables, policy).map_err(|error| Failure {
        cause: channel_cause(error), coexistence: btc_context::last_rfk_failure(phy.bus_between_stages()), tally: Tally::default() })?;
    let session = phy.bus_between_stages();
    let reports = report_tail::configure(&mut *session.bus, &mut *session.queue, &mut *session.rings, session.receiving, None)
        .map_err(|error| fail(Cause::Initial(initial_phy::reports_cause(error))))?;
    prepared.observations.encoded_rts = reports.encoded_rts;
    let now = session.clock().map_err(|e| fail(Cause::Filter(e)))?;
    session.registers(now + POLL_US, |window| {
        io::modify32(window, RX_FILTER, SCAN_OPEN, 0)?;
        let value = io::read32(window, RX_FILTER)?;
        if value & SCAN_OPEN != 0 { return Err(Fault::new(FaultKind::PowerState, RX_FILTER, value)); }
        Ok(())
    }).map_err(|e| fail(Cause::Filter(e)))?;
    Ok(Receiving { channel: tuned.channel, prepared })
}

/// Consume this successful firmware startup once: initial PHY, one receive-only channel and a bounded listen.
/// Scheduler and data DMA stay stopped and every poll re-checks it; `first` sees each BSSID once.
/// The caller still stops RX/WCPU, verifies PCI quiescence and retains DMA after either result.
pub fn listen<B: DeviceIo>(bus: &mut B, queue: &mut Queue<'_>, rings: &mut rx::Rings<'_>, receiving: &post_mac::Receiving,
    firmware: download::Ready, board: &Board, caps: &Capabilities, scratch: &mut [u8], center_mhz: u16, listen_us: u64,
    first: impl FnMut(&Heard)) -> Result<Listened, Failure> {
    let (mut session, tables, policy) = open(bus, queue, rings, receiving, firmware, board, caps, scratch, center_mhz)?;
    let mut phy = Phy::new(&mut session, board.identity);
    let Receiving { channel, prepared, .. } = prepare_receive(&mut phy, board, caps, &tables, &policy)?;
    let initial = prepared.observations;
    let session = phy.bus_between_stages();
    let mut listener = Listener { table: [None; HEARD_MAX], tally: Tally::default(), first };
    let result = (|| {
        let begin = session.clock().map_err(Cause::Listen)?;
        let end = begin.saturating_add(listen_us);
        loop {
            let now = session.clock().map_err(Cause::Listen)?;
            if now >= end { return Ok(now - begin); }
            let (received, events) = session.poll_frames(|c, p, b| listener.take(c, p, b))?;
            listener.tally.btc_events = listener.tally.btc_events.saturating_add(events as u32);
            if received == [false, false] { session.idle()?; }
        }
    })();
    match result {
        Ok(elapsed_us) => Ok(Listened { initial, channel, elapsed_us, tally: listener.tally,
            heard: listener.table.iter().flatten().count(), counts: session.counts() }),
        Err(cause) => Err(Failure { cause, coexistence: btc_context::last_rfk_failure(session), tally: listener.tally }),
    }
}
