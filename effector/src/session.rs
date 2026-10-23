//! Typed session scheduling and journal-before-ACK, without network parsing or local action authority (750-EFF-0006).
use crate::sha256::Sha256;

pub const RECORDS_MAX: usize = 64;
pub const TEXT_MAX: usize = 512;
pub const HEARTBEAT_MS: u64 = 1500;
pub const POLL_MS: u64 = 10_000;
pub const ACK_RETRY_MS: u64 = 5000;
pub const BACKOFF_MAX_MS: u64 = 60_000;
pub const REQUEST_MS: u64 = 1500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Identifier, Text, WrongAgent, Clock, ClockRange, Full, Busy, Conflict, Journal, Stale, Sequence }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Id { bytes: [u8; 64], len: u8 }
impl Id {
    /// An agent ID: 1–64 of `[A-Za-z0-9_-]` (contract 1.0).
    pub fn new(text: &str) -> Result<Self, Error> { Self::checked(text, false) }
    /// A command ID: also `.`, which the server's IDs carry (`20261021064900.000000001_000042`).
    pub fn command(text: &str) -> Result<Self, Error> { Self::checked(text, true) }
    fn checked(text: &str, dot: bool) -> Result<Self, Error> {
        let ok = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || (dot && c == b'.');
        if text.is_empty() || text.len() > 64 || !text.bytes().all(ok) || text.starts_with('.') { return Err(Error::Identifier); }
        let mut id = Self { bytes: [0; 64], len: text.len() as u8 };
        id.bytes[..text.len()].copy_from_slice(text.as_bytes());
        Ok(id)
    }
    pub fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Time { pub monotonic_ms: u64, pub unix_seconds: Option<u64> }

/// Only fields already decoded and schema-checked by the parser adapter enter the core.
#[derive(Clone, Copy, Debug)]
pub struct Command<'a> {
    pub id: &'a str, pub agent_id: &'a str, pub action: &'a str,
    pub program: &'a str, pub payload: &'a str, pub expires_unix: Option<u64>,
}
impl Command<'_> {
    fn identity(&self, agent: Id) -> Result<(Id, [u8; 32]), Error> {
        let id = Id::command(self.id)?;
        if Id::new(self.agent_id)? != agent { return Err(Error::WrongAgent); }
        if self.action.is_empty() || [self.action, self.program, self.payload].iter().any(|s| s.len() > TEXT_MAX) { return Err(Error::Text); }
        let mut hash = Sha256::new();
        hash.update(b"MIND effector denied command 1\0");
        for field in [self.id, self.agent_id, self.action, self.program, self.payload] {
            hash.update(&(field.len() as u16).to_le_bytes()); hash.update(field.as_bytes());
        }
        hash.update(&[self.expires_unix.is_some() as u8]);
        hash.update(&self.expires_unix.unwrap_or(0).to_le_bytes());
        Ok((id, hash.finish()))
    }
}

/// Why an action the agent handed to its gateway failed: the gateway's errors (`idl/effector.wit`) and the agent's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure { Denied, NoPolicy, NotFound, Bounds, Digest, Busy, Expired, Audit, Io, Malformed, Unsupported, Upload }
impl Failure {
    pub const ALL: [Failure; 12] = [Self::Denied, Self::NoPolicy, Self::NotFound, Self::Bounds, Self::Digest, Self::Busy, Self::Expired,
                                    Self::Audit, Self::Io, Self::Malformed, Self::Unsupported, Self::Upload];
    pub fn name(self) -> &'static str {
        match self {
            Self::Denied => "denied", Self::NoPolicy => "no-policy", Self::NotFound => "not-found", Self::Bounds => "bounds",
            Self::Digest => "digest", Self::Busy => "busy", Self::Expired => "expired", Self::Audit => "audit", Self::Io => "io",
            Self::Malformed => "malformed", Self::Unsupported => "unsupported", Self::Upload => "upload",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> { Self::ALL.iter().copied().find(|f| f.name() == name) }
}

macro_rules! failed { ($name:literal) => { concat!(r#"{"status":"failed","exit_code":1,"stdout":"","stderr":"","truncated":false,"run_as":"effector","error":""#, $name, r#""}"#) } }

/// What an answer carries back besides its status (750-EFF-0007): the screen as a PNG, or a log's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Artifact { Screen, Log }
impl Artifact {
    pub fn name(self) -> &'static str { match self { Self::Screen => "screen", Self::Log => "log" } }
    pub fn from_name(name: &str) -> Option<Self> { [Self::Screen, Self::Log].into_iter().find(|a| a.name() == name) }
}

/// A command's answer: denied (no gateway action for it), expired, or what the gateway did. `Captured`: completed with
/// an artifact the agent keeps until the answer is acknowledged; its result is built from that file, not `result_json`.
/// `Reported`: completed with a result the agent measured (`performance_measure`), kept the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Final { Denied, Expired, Completed, Captured(Artifact), Reported, Failed(Failure) }
impl Final {
    pub fn status(self) -> &'static str {
        match self { Self::Denied => "denied", Self::Expired => "expired", Self::Completed | Self::Captured(_) | Self::Reported => "completed", Self::Failed(_) => "failed" }
    }
    pub fn reason(self) -> &'static str {
        match self { Self::Denied => "action_not_allowed", Self::Expired => "command_expired", Self::Completed | Self::Captured(_) | Self::Reported => "", Self::Failed(f) => f.name() }
    }
    /// The result as a JSON text; a `Captured` answer whose artifact is gone is answered as the `io` failure it is.
    pub fn result_json(self) -> &'static str {
        match self {
            Self::Denied => r#"{"status":"denied","exit_code":1,"stdout":"","stderr":"","truncated":false,"run_as":"effector","error":"action_not_allowed"}"#,
            Self::Expired => r#"{"status":"expired","exit_code":1,"stdout":"","stderr":"","truncated":false,"run_as":"effector","error":"command_expired"}"#,
            Self::Completed => r#"{"status":"completed","exit_code":0,"stdout":"","stderr":"","truncated":false,"run_as":"effector"}"#,
            Self::Captured(_) | Self::Reported => failed!("io"),
            Self::Failed(f) => match f {
                Failure::Denied => failed!("denied"), Failure::NoPolicy => failed!("no-policy"), Failure::NotFound => failed!("not-found"),
                Failure::Bounds => failed!("bounds"), Failure::Digest => failed!("digest"), Failure::Busy => failed!("busy"),
                Failure::Expired => failed!("expired"), Failure::Audit => failed!("audit"), Failure::Io => failed!("io"),
                Failure::Malformed => failed!("malformed"), Failure::Unsupported => failed!("unsupported"), Failure::Upload => failed!("upload"),
            },
        }
    }
}

/// The trusted journal adapter must persist this complete record atomically and durably before confirming Store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record { pub agent: Id, pub id: Id, pub identity: [u8; 32], pub result: Final, pub expires_unix: Option<u64> }

/// A record to make durable. `rewrite`: an acknowledged record was dropped to make room, so the journal is written
/// again whole (`Session::journal`) rather than appended to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Store { pub sequence: u64, pub slot: usize, pub record: Record, pub rewrite: bool }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accepted { Store(Store), Replay, InProgress }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attempt { pub sequence: u64, pub started_ms: u64, pub deadline_ms: u64 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ack { pub attempt: Attempt, pub record: Record }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome { Accepted, Retry, Permanent }

/// Classify only after validating a successful response's JSON status; redirects never trigger a retry to that URL.
pub fn outcome(status: u16, valid_ok: bool) -> Outcome {
    if status == 200 && valid_ok { Outcome::Accepted }
    else if status == 429 || (500..600).contains(&status) { Outcome::Retry }
    else { Outcome::Permanent }
}

#[derive(Clone, Copy)]
struct Lane { due: u64, active: Option<Attempt>, sequence: u64, failures: u8, stopped: bool }
impl Lane {
    const fn new(now: u64) -> Self { Self { due: now, active: None, sequence: 0, failures: 0, stopped: false } }
    fn begin(&mut self, now: u64) -> Result<Option<Attempt>, Error> {
        if self.stopped || self.active.is_some() || now < self.due { return Ok(None); }
        self.sequence = self.sequence.checked_add(1).ok_or(Error::Sequence)?;
        let attempt = Attempt { sequence: self.sequence, started_ms: now, deadline_ms: now.saturating_add(REQUEST_MS) };
        self.active = Some(attempt); Ok(Some(attempt))
    }
    fn finish(&mut self, attempt: Attempt, result: Outcome, now: u64, period: u64, floor: u64, jitter: u32) -> Result<(), Error> {
        if self.active != Some(attempt) { return Err(Error::Stale); }
        self.active = None;
        match result {
            Outcome::Accepted => { self.failures = 0; self.due = now.max(attempt.started_ms.saturating_add(period)); }
            Outcome::Retry => {
                let delay = floor.saturating_mul(1 << self.failures.min(6)).saturating_add((jitter % 251) as u64).min(BACKOFF_MAX_MS);
                self.failures = self.failures.saturating_add(1); self.due = now.saturating_add(delay);
            }
            Outcome::Permanent => self.stopped = true,
        }
        Ok(())
    }
    fn reset(&mut self, now: u64) { self.due = now; self.active = None; self.failures = 0; self.stopped = false; }
    fn overdue(&self, now: u64) -> Option<Attempt> { self.active.filter(|a| now >= a.deadline_ms) }
}

#[derive(Clone, Copy)]
struct Saved { record: Record, durable: bool, done: bool, sequence: u64 }
#[derive(Clone, Copy)]
struct Pending { slot: usize }

pub struct Session {
    agent: Id, records: [Option<Saved>; RECORDS_MAX], pending: Option<Pending>, saving: Option<Store>,
    store_sequence: u64, journal_failed: bool, registered: bool, stopping: bool,
    wake_pending: bool, shutdown_attempt: Option<Attempt>, shutdown_done: bool,
    heartbeat: Lane, poll: Lane, ack: Lane,
}
impl Session {
    /// Restore only validated complete records from the trusted bounded journal; never infer completion from an ID alone.
    pub fn new(agent_id: &str, recovered: &[Record], now_ms: u64) -> Result<Self, Error> { Self::restore(agent_id, recovered, &[], now_ms) }

    /// As `new`, with `done[i]` telling that record `i`'s answer needs no more ACK (the journal's `done` line).
    pub fn restore(agent_id: &str, recovered: &[Record], done: &[bool], now_ms: u64) -> Result<Self, Error> {
        if recovered.len() > RECORDS_MAX { return Err(Error::Full); }
        let mut session = Self {
            agent: Id::new(agent_id)?, records: [None; RECORDS_MAX], pending: None, saving: None,
            store_sequence: 0, journal_failed: false, registered: false, stopping: false,
            wake_pending: false, shutdown_attempt: None, shutdown_done: false,
            heartbeat: Lane::new(now_ms), poll: Lane::new(now_ms), ack: Lane::new(now_ms),
        };
        for (slot, &record) in recovered.iter().enumerate() {
            if record.agent != session.agent { return Err(Error::WrongAgent); }
            if session.records[..slot].iter().flatten().any(|old| old.record.id == record.id) { return Err(Error::Journal); }
            session.records[slot] = Some(Saved { record, durable: true, done: done.get(slot).copied().unwrap_or(false), sequence: slot as u64 + 1 });
        }
        session.store_sequence = recovered.len() as u64;
        Ok(session)
    }
    pub fn agent_id(&self) -> &str { self.agent.as_str() }
    pub fn busy(&self) -> bool { self.pending.is_some() || self.journal_failed || self.records.iter().flatten().any(|r| r.durable && !r.done) }
    pub fn stopping(&self) -> bool { self.stopping }
    pub fn journal_failed(&self) -> bool { self.journal_failed }
    pub fn record_count(&self) -> usize { self.records.iter().flatten().filter(|r| r.durable).count() }
    pub fn record(&self, slot: usize) -> Option<Record> { self.records.get(slot).and_then(|s| s.filter(|r| r.durable).map(|r| r.record)) }
    /// What the journal holds, oldest first: each record (with the one being saved) and whether it needs no more ACK.
    /// Records are copied one at a time (the program stack is 64 KiB).
    pub fn journal(&self) -> impl Iterator<Item = (Record, bool)> + '_ {
        let mut order = [(u64::MAX, 0u8); RECORDS_MAX];
        let mut n = 0;
        for (slot, saved) in self.records.iter().enumerate() { if let Some(s) = saved { order[n] = (s.sequence, slot as u8); n += 1; } }
        order[..n].sort_unstable();
        (0..n).map(move |i| { let s = self.records[order[i].1 as usize].unwrap(); (s.record, s.done) })
    }

    pub fn heartbeat(&mut self, now_ms: u64) -> Result<Option<Attempt>, Error> {
        let attempt = self.heartbeat.begin(now_ms)?;
        if self.stopping && attempt.is_some() { self.shutdown_attempt = attempt; }
        Ok(attempt)
    }
    pub fn shutdown_done(&self) -> bool { self.shutdown_done }
    pub fn heartbeat_done(&mut self, attempt: Attempt, result: Outcome, now_ms: u64, jitter: u32) -> Result<(), Error> {
        self.heartbeat.finish(attempt, result, now_ms, HEARTBEAT_MS, 1000, jitter)?;
        if result == Outcome::Accepted { self.registered = true; }
        if result == Outcome::Permanent { self.registered = false; }
        if self.stopping && result == Outcome::Accepted {
            if self.shutdown_attempt == Some(attempt) { self.shutdown_done = true; self.heartbeat.stopped = true; }
            else { self.heartbeat.due = now_ms; }
        }
        Ok(())
    }
    pub fn poll(&mut self, now_ms: u64) -> Result<Option<Attempt>, Error> {
        if !self.registered || self.stopping || self.journal_failed { return Ok(None); }
        self.poll.begin(now_ms)
    }
    pub fn poll_done(&mut self, attempt: Attempt, result: Outcome, now_ms: u64, jitter: u32) -> Result<(), Error> {
        self.poll.finish(attempt, result, now_ms, POLL_MS, 1000, jitter)?;
        if self.wake_pending && result == Outcome::Accepted { self.wake_pending = false; self.poll.due = now_ms; }
        Ok(())
    }
    pub fn wake(&mut self, now_ms: u64) {
        if self.poll.active.is_some() { self.wake_pending = true; }
        else if !self.poll.stopped && self.poll.failures == 0 { self.poll.due = self.poll.due.min(now_ms); }
    }
    pub fn stop(&mut self, now_ms: u64) { self.stopping = true; self.heartbeat.due = now_ms; }
    pub fn heartbeat_overdue(&self, now_ms: u64) -> Option<Attempt> { self.heartbeat.overdue(now_ms) }
    pub fn poll_overdue(&self, now_ms: u64) -> Option<Attempt> { self.poll.overdue(now_ms) }
    pub fn ack_overdue(&self, now_ms: u64) -> Option<Attempt> { self.ack.overdue(now_ms) }

    /// Whether `command` is new (true) or the one already saved under its ID (false): an action runs only for a new one.
    pub fn fresh(&self, command: &Command<'_>) -> Result<bool, Error> {
        let (id, identity) = command.identity(self.agent)?;
        match self.records.iter().flatten().find(|s| s.record.id == id) {
            Some(saved) if saved.record.identity != identity => Err(Error::Conflict),
            Some(_) => Ok(false),
            None => Ok(true),
        }
    }

    /// Whether an action may run for `command` now: the agent free, the command new and not expired, and room for its
    /// answer in the journal. An action never runs when its answer could not be kept.
    pub fn runnable(&self, command: &Command<'_>, now: Time) -> bool {
        !self.busy() && !self.stopping && self.fresh(command) == Ok(true) && self.room(now).is_some()
            && deadline(command.expires_unix, now).is_ok_and(|d| d.map_or(true, |d| d > now.monotonic_ms))
    }

    pub fn registered(&self) -> bool { self.registered }

    /// The command whose answer waits for the server, the oldest first: the agent's one slot is occupied meanwhile.
    pub fn waiting(&self) -> Option<Id> {
        let pending = self.pending.and_then(|p| self.records[p.slot]).map(|s| (s.sequence, s.record.id));
        pending.or_else(|| self.records.iter().flatten().filter(|s| !s.done).min_by_key(|s| s.sequence).map(|s| (s.sequence, s.record.id))).map(|(_, id)| id)
    }

    /// A slot for a new record, and whether a record is dropped for it. A full journal makes room by dropping its
    /// oldest record that is acknowledged and past its command's expiry: a redelivery after that is answered
    /// `expired`, never run. Nothing else is ever dropped.
    fn room(&self, now: Time) -> Option<(usize, bool)> {
        if let Some(slot) = self.records.iter().position(Option::is_none) { return Some((slot, false)); }
        let past = |s: &Saved| s.record.expires_unix.is_some_and(|e| now.unix_seconds.is_some_and(|u| u >= e));
        let oldest = self.records.iter().enumerate().filter_map(|(i, s)| s.filter(|s| s.durable && s.done && past(s)).map(|s| (i, s.sequence))).min_by_key(|&(_, q)| q);
        oldest.map(|(slot, _)| (slot, true))
    }

    pub fn receive(&mut self, command: Command<'_>, now: Time) -> Result<Accepted, Error> { self.receive_with(command, now, Final::Denied) }

    /// As `receive`, with `done` as a new command's answer (what its action did); an expired one stays `expired`.
    pub fn receive_with(&mut self, command: Command<'_>, now: Time, done: Final) -> Result<Accepted, Error> {
        if self.stopping { return Err(Error::Busy); }
        if self.journal_failed { return Err(Error::Journal); }
        let (id, identity) = command.identity(self.agent)?;
        if let Some(slot) = self.records.iter().position(|s| s.is_some_and(|s| s.record.id == id)) {
            let saved = self.records[slot].unwrap();
            if saved.record.identity != identity { return Err(Error::Conflict); }
            if let Some(pending) = self.pending { return if pending.slot == slot { Ok(Accepted::InProgress) } else { Err(Error::Busy) }; }
            if !saved.durable { return Err(Error::Journal); }
            self.pending = Some(Pending { slot });
            self.ack.reset(now.monotonic_ms); Ok(Accepted::Replay)
        } else {
            if self.busy() { return Err(Error::Busy); }
            let (slot, rewrite) = self.room(now).ok_or(Error::Full)?;
            let deadline_ms = deadline(command.expires_unix, now)?;
            let expired = deadline_ms.is_some_and(|d| d <= now.monotonic_ms);
            let record = Record { agent: self.agent, id, identity, result: if expired { Final::Expired } else { done }, expires_unix: command.expires_unix };
            self.store_sequence = self.store_sequence.checked_add(1).ok_or(Error::Sequence)?;
            let store = Store { sequence: self.store_sequence, slot, record, rewrite };
            self.records[slot] = Some(Saved { record, durable: false, done: false, sequence: self.store_sequence });
            self.pending = Some(Pending { slot });
            self.saving = Some(store); self.ack.reset(now.monotonic_ms);
            Ok(Accepted::Store(store))
        }
    }
    pub fn stored(&mut self, store: Store, durable: bool) -> Result<(), Error> {
        if self.saving != Some(store) { return Err(Error::Stale); }
        self.saving = None;
        if !durable { self.journal_failed = true; return Err(Error::Journal); }
        self.records[store.slot].as_mut().ok_or(Error::Journal)?.durable = true;
        Ok(())
    }
    pub fn next_ack(&mut self, now: Time) -> Result<Option<Ack>, Error> {
        if !self.registered || self.journal_failed || self.saving.is_some() { return Ok(None); }
        for _ in 0..RECORDS_MAX {
            if self.pending.is_none() {
                let Some(slot) = self.records.iter().position(|s| s.is_some_and(|s| s.durable && !s.done)) else { return Ok(None); };
                self.pending = Some(Pending { slot });
                self.ack.reset(now.monotonic_ms);
            }
            // The command's term bounds its wait in the server's queue, not its report: a late answer is still sent until
            // the server takes it or refuses it for good (Effector ahelp, "Reservation lease, queue waiting and command runtime").
            let pending = self.pending.unwrap();
            let Some(attempt) = self.ack.begin(now.monotonic_ms)? else { return Ok(None); };
            return Ok(Some(Ack { attempt, record: self.records[pending.slot].unwrap().record }));
        }
        Ok(None)
    }
    pub fn ack_done(&mut self, ack: Ack, result: Outcome, now_ms: u64, jitter: u32) -> Result<(), Error> {
        let pending = self.pending.ok_or(Error::Stale)?;
        if self.records[pending.slot].unwrap().record != ack.record { return Err(Error::Stale); }
        self.ack.finish(ack.attempt, result, now_ms, 0, ACK_RETRY_MS, jitter)?;
        if result != Outcome::Retry {
            self.records[pending.slot].as_mut().unwrap().done = true;
            self.pending = None;
        }
        Ok(())
    }
}

fn deadline(expires: Option<u64>, now: Time) -> Result<Option<u64>, Error> {
    let Some(expires) = expires else { return Ok(None); };
    let wall = now.unix_seconds.ok_or(Error::Clock)?;
    let remaining_ms = expires.saturating_sub(wall).checked_mul(1000).ok_or(Error::ClockRange)?;
    Ok(Some(now.monotonic_ms.checked_add(remaining_ms).ok_or(Error::ClockRange)?))
}
