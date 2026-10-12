//! The station (550-NET-0017): scanning, Open System authentication, association, the WPA2-PSK handshakes through
//! `wpa`, and data frames to and from Ethernet. It does no I/O: each call says what the radio is to do (`Out`) with the
//! frame to send in `tx`, and hands a received Ethernet frame back in `rx`.
use crate::frame::{self, Header, Mac, ASSOCIATION_RESPONSE, AUTHENTICATION, BEACON, DEAUTHENTICATION, DISASSOCIATION, ETHERTYPE_EAPOL,
                   FROM_DS, PROBE_RESPONSE, PROTECTED, TO_DS, TYPE_DATA, TYPE_MANAGEMENT};
use crate::scan::{self, Bss, List, Security};
use crate::Error;
use wpa::ccmp::{self, Replay};
use wpa::eapol::Gtk;
use wpa::handshake::{Supplicant, REPLY_MAX};

pub const FRAME_MAX: usize = 2346;
pub const ETHERNET_MAX: usize = 1514;
/// How long a channel is listened to in a scan: with a probe request where the station may send, without elsewhere.
pub const DWELL_ACTIVE_MS: u64 = 100;
pub const DWELL_PASSIVE_MS: u64 = 150;
/// Authentication and association: tries, each waited for this long.
pub const TRIES: u8 = 3;
pub const ANSWER_MS: u64 = 300;
/// The handshake must end this long after association.
pub const HANDSHAKE_MS: u64 = 4000;
/// A joined network not heard from for this long is lost.
pub const SILENCE_MS: u64 = 10_000;
/// A lost network is joined again after this long.
pub const REJOIN_MS: u64 = 3000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State { Off, Scanning, Joining, Joined, Failed }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason { None, WrongPassphrase, NoAnswer, Refused, NotFound, Unsupported, Lost }

/// What the radio is to do after a call, in this order: tune, filter, send `tx[..len]` at `rate` (500 kb/s units); and
/// whether `rx` holds a received Ethernet frame of that length.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Out { pub channel: Option<u8>, pub filter: Option<Filter>, pub send: Option<(usize, u8)>, pub deliver: Option<usize> }

/// The radio's receive filter: every beacon (a scan), and the frames of `bssid` (zero: none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Filter { pub all_beacons: bool, pub bssid: Mac }

/// The channels where the station may send a probe request before it knows where it is: 1 to 11, which every
/// regulatory domain allows. Elsewhere it only listens, and sends only to an access point it heard there.
pub fn may_probe(channel: u8) -> bool { (1..=11).contains(&channel) }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step { Authenticating, Associating, Handshake }

#[derive(Clone, Copy)]
struct Join { bss: Bss, pmk: Option<[u8; 32]>, step: Step, tries: u8, deadline: u64, answered: bool }

struct Keys { tk: [u8; 16], pn: u64, replay: Replay, groups: [Option<(Gtk, Replay)>; 4] }

pub struct Station {
    own: Mac,
    channels: [u8; 64],
    channel_count: usize,
    state: State,
    reason: Reason,
    list: List,
    scanning: Option<(usize, u64)>,
    join: Option<Join>,
    supplicant: Option<Supplicant>,
    keys: Option<Keys>,
    heard: u64,
    rejoin: Option<u64>,
    sequence: u16,
    rates: [u8; 12],
    rate_count: usize,
    rate: usize,
    acked: u8,
    failed: u8,
}

impl Station {
    /// A station with address `own` on a radio that can use `channels`.
    pub fn new(own: Mac, channels: &[u8]) -> Self {
        let mut list = [0u8; 64];
        let count = channels.len().min(64);
        list[..count].copy_from_slice(&channels[..count]);
        Station { own, channels: list, channel_count: count, state: State::Off, reason: Reason::None, list: List::default(), scanning: None,
                  join: None, supplicant: None, keys: None, heard: 0, rejoin: None, sequence: 0, rates: [0; 12], rate_count: 0, rate: 0,
                  acked: 0, failed: 0 }
    }

    pub fn state(&self) -> State { self.state }
    pub fn reason(&self) -> Reason { self.reason }
    pub fn networks(&self) -> &List { &self.list }
    /// The network joined or being joined.
    pub fn network(&self) -> Option<&Bss> { self.join.as_ref().map(|j| &j.bss) }

    fn next_sequence(&mut self) -> u16 { self.sequence = (self.sequence + 1) & 0x0FFF; self.sequence }

    // The lowest basic rate of the band, for management frames.
    fn basic_rate(channel: u8) -> u8 { if channel <= 14 { 2 } else { 12 } }

    // A unicast frame's duration: SIFS and an acknowledgement at the lowest rate of its modulation.
    fn duration(rate: u8) -> u16 { if rate <= 22 { 314 } else { 44 } }

    /// Starts a scan of every channel (not while joining or joined: the list then follows the network's channel).
    pub fn scan(&mut self, now: u64, out: &mut Out, tx: &mut [u8]) -> Result<(), Error> {
        if matches!(self.state, State::Joined | State::Joining | State::Scanning) { return Err(Error::Busy); }
        if self.channel_count == 0 { return Err(Error::NoDevice); }
        self.list.clear();
        self.state = State::Scanning;
        out.filter = Some(Filter { all_beacons: true, bssid: [0; 6] });
        self.scan_channel(0, now, out, tx)
    }

    fn scan_channel(&mut self, index: usize, now: u64, out: &mut Out, tx: &mut [u8]) -> Result<(), Error> {
        let channel = self.channels[index];
        out.channel = Some(channel);
        let active = may_probe(channel);
        if active {
            let sequence = self.next_sequence();
            let len = frame::probe_request(tx, &self.own, &[], channel, sequence)?;
            out.send = Some((len, Self::basic_rate(channel)));
        }
        self.scanning = Some((index, now + if active { DWELL_ACTIVE_MS } else { DWELL_PASSIVE_MS }));
        Ok(())
    }

    /// Starts joining the strongest access point of `ssid` from the last scan; `pmk` for a WPA2 network.
    pub fn join(&mut self, ssid: &[u8], pmk: Option<[u8; 32]>, now: u64, out: &mut Out, tx: &mut [u8]) -> Result<(), Error> {
        if self.state == State::Scanning { return Err(Error::Busy); }
        let named = self.list.iter().any(|b| b.ssid() == ssid);
        let Some(bss) = self.list.best(ssid).copied() else {
            let (reason, error) = if named { (Reason::Unsupported, Error::Unsupported) } else { (Reason::NotFound, Error::NotFound) };
            self.fail(reason);
            return Err(error);
        };
        if bss.security == Security::Wpa2 && pmk.is_none() { return Err(Error::Invalid); }
        if matches!(self.state, State::Joined | State::Joining) { self.leave(out, tx); }
        self.start(bss, pmk, now, out, tx)
    }

    fn start(&mut self, bss: Bss, pmk: Option<[u8; 32]>, now: u64, out: &mut Out, tx: &mut [u8]) -> Result<(), Error> {
        self.supplicant = None;
        self.keys = None;
        self.rejoin = None;
        self.state = State::Joining;
        out.channel = Some(bss.channel);
        out.filter = Some(Filter { all_beacons: false, bssid: bss.bssid });
        self.join = Some(Join { bss, pmk: pmk.filter(|_| bss.security == Security::Wpa2), step: Step::Authenticating, tries: 0,
                                deadline: 0, answered: false });
        self.retry(now, out, tx)
    }

    // Sends the frame of the current step again, or gives up after TRIES.
    fn retry(&mut self, now: u64, out: &mut Out, tx: &mut [u8]) -> Result<(), Error> {
        let Some(join) = self.join else { return Ok(()) };
        if join.step == Step::Handshake { return Ok(()); }
        if join.tries >= TRIES { self.fail(Reason::NoAnswer); return Ok(()); }
        let sequence = self.next_sequence();
        let (bss, rate) = (join.bss, Self::basic_rate(join.bss.channel));
        let len = if join.step == Step::Authenticating {
            frame::authentication(tx, &self.own, &bss.bssid, sequence, Self::duration(rate))?
        } else {
            let rsne = scan::own_rsne();
            let capability = frame::CAPABILITY_ESS
                | (bss.capability & (frame::CAPABILITY_PRIVACY | frame::CAPABILITY_SHORT_PREAMBLE | frame::CAPABILITY_SHORT_SLOT));
            frame::association_request(tx, &self.own, &bss.bssid, capability, bss.ssid(), bss.channel,
                                       (bss.security == Security::Wpa2).then_some(&rsne[..]), sequence, Self::duration(rate))?
        };
        if let Some(j) = self.join.as_mut() { j.tries += 1; j.deadline = now + ANSWER_MS; }
        out.send = Some((len, rate));
        Ok(())
    }

    fn fail(&mut self, reason: Reason) {
        self.state = State::Failed;
        self.reason = reason;
        self.supplicant = None;
        self.keys = None;
        self.scanning = None;
        self.rejoin = None;
    }

    /// Leaves the network (deauthentication, reason 3) and forgets its keys.
    pub fn leave(&mut self, out: &mut Out, tx: &mut [u8]) {
        if let (Some(join), State::Joined | State::Joining) = (self.join, self.state) {
            let sequence = self.next_sequence();
            if let Ok(len) = frame::deauthentication(tx, &self.own, &join.bss.bssid, 3, sequence) {
                out.send = Some((len, Self::basic_rate(join.bss.channel)));
            }
        }
        self.join = None;
        self.fail(Reason::None);
        self.state = State::Off;
    }

    /// Timers: the next channel of a scan, an unanswered step, a silent network, a rejoin.
    pub fn tick(&mut self, now: u64, out: &mut Out, tx: &mut [u8]) -> Result<(), Error> {
        if let Some((index, until)) = self.scanning {
            if now < until { return Ok(()); }
            if index + 1 < self.channel_count { return self.scan_channel(index + 1, now, out, tx); }
            self.scanning = None;
            self.state = if self.reason == Reason::None { State::Off } else { State::Failed };
            out.filter = Some(Filter { all_beacons: false, bssid: [0; 6] });
            return Ok(());
        }
        if let Some(at) = self.rejoin {
            if now < at { return Ok(()); }
            let Some(join) = self.join else { self.rejoin = None; return Ok(()) };
            let reason = self.reason;
            self.start(join.bss, join.pmk, now, out, tx)?;
            self.reason = reason;
            return Ok(());
        }
        let Some(join) = self.join else { return Ok(()) };
        match self.state {
            State::Joining if now >= join.deadline && join.step == Step::Handshake => {
                let reason = if join.answered { Reason::WrongPassphrase } else { Reason::NoAnswer };
                self.leave(out, tx);
                self.fail(reason);
                Ok(())
            }
            State::Joining if now >= join.deadline => self.retry(now, out, tx),
            State::Joined if now >= self.heard + SILENCE_MS => { self.lost(now); Ok(()) }
            _ => Ok(()),
        }
    }

    // The network went away: its keys are dropped, and it is joined again after a while.
    fn lost(&mut self, now: u64) {
        self.state = State::Joining;
        self.reason = Reason::Lost;
        self.supplicant = None;
        self.keys = None;
        self.rejoin = Some(now + REJOIN_MS);
    }

    /// Takes a received frame (FCS already checked), heard on `channel` with `weakness` (minus dBm). `random` is fresh
    /// for a new SNonce. A data frame for the system comes back in `rx` (`out.deliver`).
    pub fn receive(&mut self, frame: &[u8], channel: u8, weakness: u8, now: u64, random: &[u8; 32], out: &mut Out, tx: &mut [u8],
                   rx: &mut [u8]) -> Result<(), Error> {
        let h = frame::header(frame)?;
        if h.kind == TYPE_MANAGEMENT && (h.subtype == BEACON || h.subtype == PROBE_RESPONSE) {
            if let Some(bss) = scan::describe(frame, &h, channel, weakness, now) {
                if self.join.is_some_and(|j| j.bss.bssid == bss.bssid) { self.heard = now; }
                self.list.insert(bss);
            }
            return Ok(());
        }
        let Some(join) = self.join else { return Err(Error::Unexpected) };
        if h.a2 != join.bss.bssid || (h.a1 != self.own && !(h.kind == TYPE_DATA && frame::group(&h.a1))) { return Err(Error::Unexpected); }
        self.heard = now;
        if h.kind == TYPE_MANAGEMENT { self.management(frame, &h, join, now, out, tx) } else { self.data(frame, &h, random, out, tx, rx) }
    }

    fn management(&mut self, frame: &[u8], h: &Header, join: Join, now: u64, out: &mut Out, tx: &mut [u8]) -> Result<(), Error> {
        let body = &frame[h.len..];
        let field = |at: usize| body.get(at..at + 2).map(|f| u16::from_le_bytes([f[0], f[1]])).ok_or(Error::Malformed);
        match h.subtype {
            AUTHENTICATION if self.state == State::Joining && join.step == Step::Authenticating => {
                if field(0)? != 0 || field(2)? != 2 { return Err(Error::Unexpected); }
                if field(4)? != 0 { self.fail(Reason::Refused); return Ok(()); }
                if let Some(j) = self.join.as_mut() { j.step = Step::Associating; j.tries = 0; }
                self.retry(now, out, tx)
            }
            ASSOCIATION_RESPONSE if self.state == State::Joining && join.step == Step::Associating => {
                if field(2)? != 0 { self.fail(Reason::Refused); return Ok(()); }
                self.take_rates(&body[6..]);
                match join.pmk {
                    None => { self.state = State::Joined; self.reason = Reason::None; }
                    Some(pmk) => {
                        let rsne = scan::own_rsne();
                        let theirs = join.bss.rsne().ok_or(Error::Malformed)?;
                        self.supplicant = Some(Supplicant::new(pmk, self.own, join.bss.bssid, &rsne, theirs).map_err(|_| Error::Malformed)?);
                        if let Some(j) = self.join.as_mut() { j.step = Step::Handshake; j.deadline = now + HANDSHAKE_MS; }
                    }
                }
                Ok(())
            }
            DEAUTHENTICATION | DISASSOCIATION => {
                match self.state {
                    State::Joining if join.step == Step::Handshake && join.answered => self.fail(Reason::WrongPassphrase),
                    State::Joining if self.rejoin.is_none() => self.fail(Reason::Refused),
                    State::Joined => self.lost(now),
                    _ => {}
                }
                Ok(())
            }
            _ => Err(Error::Unexpected),
        }
    }

    // The data rates both sides support, fastest first, from the association response's rate elements.
    fn take_rates(&mut self, elements: &[u8]) {
        let channel = self.join.map_or(1, |j| j.bss.channel);
        let mut theirs = [0u8; 16];
        let mut n = 0;
        for (id, body) in frame::Elements::new(elements) {
            if id == frame::RATES || id == frame::EXTENDED_RATES {
                for &rate in body { if n < theirs.len() { theirs[n] = rate & 0x7F; n += 1; } }
            }
        }
        let (ours, extended) = frame::rates(channel);
        self.rate_count = 0;
        for rate in ours.iter().chain(extended.iter()).map(|r| r & 0x7F) {
            if theirs[..n].contains(&rate) && self.rate_count < self.rates.len() { self.rates[self.rate_count] = rate; self.rate_count += 1; }
        }
        self.rates[..self.rate_count].sort_unstable_by(|a, b| b.cmp(a));
        if self.rate_count == 0 { self.rates[0] = Self::basic_rate(channel); self.rate_count = 1; }
        // Start at 24 Mb/s or the fastest below it; climb on acknowledgements.
        self.rate = self.rates[..self.rate_count].iter().position(|&r| r <= 48).unwrap_or(self.rate_count - 1);
        self.acked = 0;
        self.failed = 0;
    }

    /// The rate for data frames now, in 500 kb/s units.
    pub fn data_rate(&self) -> u8 { if self.rate_count == 0 { 2 } else { self.rates[self.rate] } }

    /// What became of the last data frame sent: two failures in a row step the rate down, ten successes up.
    pub fn sent(&mut self, acked: bool) {
        if acked {
            self.failed = 0;
            self.acked += 1;
            if self.acked >= 10 && self.rate > 0 { self.rate -= 1; self.acked = 0; }
        } else {
            self.acked = 0;
            self.failed += 1;
            if self.failed >= 2 && self.rate + 1 < self.rate_count { self.rate += 1; self.failed = 0; }
        }
    }

    fn data(&mut self, frame: &[u8], h: &Header, random: &[u8; 32], out: &mut Out, tx: &mut [u8], rx: &mut [u8]) -> Result<(), Error> {
        // Only frames from the distribution system, with a body (not null frames), and not the station's own group frames
        // that the access point sends back.
        if h.flags & (TO_DS | FROM_DS) != FROM_DS || h.subtype & 4 != 0 || h.a3 == self.own { return Ok(()); }
        let protected = h.flags & PROTECTED != 0;
        let mut plain = [0u8; FRAME_MAX];
        let (len, ethertype) = if protected {
            let keys = self.keys.as_mut().ok_or(Error::Unexpected)?;
            let tid = ccmp::tid(frame).map_err(|_| Error::Malformed)?;
            let (key, replay) = if frame::group(&h.a1) {
                let (id, _) = ccmp::peek(frame).map_err(|_| Error::Malformed)?;
                let (gtk, replay) = keys.groups[id as usize].as_mut().ok_or(Error::Unexpected)?;
                (gtk.key, replay)
            } else {
                (keys.tk, &mut keys.replay)
            };
            let (n, pn) = ccmp::decrypt(&key, frame, &mut plain).map_err(|_| Error::Integrity)?;
            replay.accept(tid, pn).map_err(|_| Error::Replay)?;
            let inner = frame::header(&plain[..n])?;
            frame::to_ethernet(&plain[..n], &inner, rx)?
        } else {
            frame::to_ethernet(frame, h, rx)?
        };
        if ethertype == ETHERTYPE_EAPOL {
            let mut key = [0u8; ETHERNET_MAX];
            key[..len - 14].copy_from_slice(&rx[14..len]);
            return self.eapol(&key[..len - 14], random, out, tx);
        }
        // A protected network passes nothing in the clear, and nothing before its keys.
        let open = self.join.is_some_and(|j| j.pmk.is_none());
        if self.state != State::Joined || (!open && !protected) { return Ok(()); }
        out.deliver = Some(len);
        Ok(())
    }

    fn eapol(&mut self, key: &[u8], random: &[u8; 32], out: &mut Out, tx: &mut [u8]) -> Result<(), Error> {
        let supplicant = self.supplicant.as_mut().ok_or(Error::Unexpected)?;
        let mut reply = [0u8; REPLY_MAX];
        let step = match supplicant.receive(key, random, &mut reply) {
            Ok(step) => step,
            Err(wpa::Error::Downgrade) => { self.leave(out, tx); self.fail(Reason::Refused); return Err(Error::Downgrade); }
            Err(wpa::Error::Replay) => return Err(Error::Replay),
            Err(wpa::Error::Mic) => return Err(Error::Integrity),
            Err(_) => return Err(Error::Malformed),
        };
        if let Some(j) = self.join.as_mut() { j.answered = true; }
        let bssid = self.join.ok_or(Error::Unexpected)?.bss.bssid;
        // The answer goes out under the keys installed before this message: message 4 of the first handshake in the
        // clear, a group answer or a rekey's message 4 under the TK in use.
        self.send_frame(&bssid, ETHERTYPE_EAPOL, &reply[..step.reply], out, tx)?;
        if let Some(tk) = step.pairwise {
            let groups = self.keys.take().map_or([None; 4], |k| k.groups);
            self.keys = Some(Keys { tk, pn: 0, replay: Replay::default(), groups });
        }
        if let (Some((gtk, rsc)), Some(keys)) = (step.group, self.keys.as_mut()) {
            let start = u64::from_le_bytes([rsc[0], rsc[1], rsc[2], rsc[3], rsc[4], rsc[5], 0, 0]);
            keys.groups[gtk.id as usize] = Some((gtk, Replay::starting(start)));
        }
        if step.pairwise.is_some() && self.state == State::Joining {
            self.state = State::Joined;
            self.reason = Reason::None;
        }
        Ok(())
    }

    // A data frame to `destination` through the access point, protected once the keys are installed.
    fn send_frame(&mut self, destination: &Mac, ethertype: u16, payload: &[u8], out: &mut Out, tx: &mut [u8]) -> Result<(), Error> {
        let bssid = self.join.ok_or(Error::Unexpected)?.bss.bssid;
        let rate = self.data_rate();
        let duration = if frame::group(destination) { 0 } else { Self::duration(rate) };
        let sequence = self.next_sequence();
        let len = match self.keys.as_mut() {
            None => frame::data(tx, &self.own, &bssid, destination, ethertype, payload, sequence, duration)?,
            Some(keys) => {
                let mut plain = [0u8; FRAME_MAX];
                let len = frame::data(&mut plain, &self.own, &bssid, destination, ethertype, payload, sequence, duration)?;
                keys.pn += 1;
                ccmp::encrypt(&keys.tk, 0, keys.pn, &plain[..len], tx).map_err(|_| Error::Space)?
            }
        };
        out.send = Some((len, rate));
        Ok(())
    }

    /// Sends an Ethernet frame from the system (destination, source, type, payload) once joined.
    pub fn send(&mut self, ethernet: &[u8], out: &mut Out, tx: &mut [u8]) -> Result<(), Error> {
        if self.state != State::Joined { return Err(Error::NoDevice); }
        if ethernet.len() < 14 || ethernet.len() > ETHERNET_MAX || ethernet[6..12] != self.own { return Err(Error::Invalid); }
        let mut destination = [0u8; 6];
        destination.copy_from_slice(&ethernet[..6]);
        self.send_frame(&destination, u16::from_be_bytes([ethernet[12], ethernet[13]]), &ethernet[14..], out, tx)
    }
}
