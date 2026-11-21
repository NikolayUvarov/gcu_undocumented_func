//! The hub's state and its commands (docs/hub/README.md, 4 to 7), without sockets or files, so tests drive it with a
//! clock of their own. A command reads the state and either refuses, naming its limit, or gives the events it makes;
//! `apply` alone changes the state, and the journal's replay through it rebuilds the same state (MC-10.3).

use crate::event::{self, Author, Decision, Event, Owner};
use crate::util::{at_least, hex, sha256, DAY_MS, HOUR_MS};
use improve_proto::bucket::Bucket;
use improve_proto::form::{self, BoardEntry, Joining, Message, MessageKind, Registration, Request, Verdict, Visibility};
use improve_proto::work::BITS_MAX;
use improve_proto::state::{self, Role, State};
use improve_proto::trust::{self, Access, Limits, Trust, BANDS, PENDING};
use improve_proto::{repeat, sign, work};
use std::collections::{HashMap, VecDeque};

/// A browser's session by a user's code (4.4), and the admin pages' by an admin's.
pub const SESSION_MS: u64 = 30 * DAY_MS;
pub const ADMIN_SESSION_MS: u64 = 12 * HOUR_MS;
pub const CODE_MS: u64 = 10 * 60_000;
/// A user's codes not used yet, at once.
pub const CODES_AT_ONCE: usize = 3;
/// New keys a day from one network (5.4), and after one of them was rejected within the day.
pub const REGISTRATIONS_A_DAY: usize = 5;
pub const REGISTRATIONS_AFTER_REJECTION: usize = 1;
pub const HELD_A_DAY: usize = 3;
/// Codes typed wrong from one network in an hour before it waits.
pub const SIGN_IN_FAILURES: usize = 10;
/// New keys in an hour from all networks that raise the proof of work by a bit, and each doubling of them by one more
/// (5.4; 100003-APP-0148).
pub const WORK_RAISED_FROM: usize = 8;
pub const CHECK_DAYS: u64 = 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `intake approved-only` (4.3): requests from approved devices alone.
    pub approved_only: bool,
    /// New requests an hour from users below band 2 (5.4).
    pub global_cap: u32,
    /// The registration's proof of work, 0 when off.
    pub work_bits: u8,
    /// How often a device polls `devices/me`.
    pub poll_s: u32,
}

impl Default for Settings { fn default() -> Settings { Settings { approved_only: false, global_cap: 60, work_bits: 0, poll_s: 60 } } }

impl Settings {
    pub const NAMES: [&'static str; 4] = ["intake", "global-cap", "work-bits", "poll-s"];
    pub fn get(&self, name: &str) -> Option<String> {
        Some(match name {
            "intake" => if self.approved_only { "approved-only" } else { "all" }.into(),
            "global-cap" => self.global_cap.to_string(),
            "work-bits" => self.work_bits.to_string(),
            "poll-s" => self.poll_s.to_string(),
            _ => return None,
        })
    }
    /// The setting changed, or why the value is not one it takes.
    pub fn set(&mut self, name: &str, value: &str) -> Result<(), String> {
        let number = |lo: u32, hi: u32| value.parse::<u32>().ok().filter(|v| (lo..=hi).contains(v)).ok_or(format!("{name} takes {lo} to {hi}"));
        match name {
            "intake" => self.approved_only = match value { "all" => false, "approved-only" => true, _ => return Err("intake takes all or approved-only".into()) },
            "global-cap" => self.global_cap = number(1, 100_000)?,
            "work-bits" => self.work_bits = number(0, work::BITS_MAX as u32)? as u8,
            "poll-s" => self.poll_s = number(10, 3600)?,
            _ => return Err(format!("no setting {name:?}")),
        }
        Ok(())
    }
}

/// An agent's key in the configuration: its track and its role (7.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Agent { pub track: String, pub role: Role }

/// The keys the configuration names: admins by name, agents by track and role.
#[derive(Clone, Debug, Default)]
pub struct Keys { pub admins: HashMap<[u8; 32], String>, pub agents: HashMap<[u8; 32], Agent> }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceState { Pending, Approved, Rejected, Revoked }

impl DeviceState {
    pub fn name(self) -> &'static str {
        match self { DeviceState::Pending => "pending", DeviceState::Approved => "approved", DeviceState::Rejected => "rejected", DeviceState::Revoked => "revoked" }
    }
}

/// A line of a registration's conversation: the admin's question, welcome or reason, the device's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note { pub at: u64, pub from: Author, pub kind: &'static str, pub text: String }

#[derive(Clone, Debug)]
pub struct Device {
    pub id: [u8; 32],
    pub key: [u8; 32],
    pub name: String,
    pub kind: form::DeviceKind,
    pub platform: String,
    pub version: String,
    pub network: String,
    pub contact: Option<String>,
    pub user: u64,
    pub state: DeviceState,
    pub at: u64,
    /// Approved by a pairing code rather than by the admin.
    pub paired: bool,
    pub notes: Vec<Note>,
    /// A pending device's messages to its registration (5.2).
    messages: Bucket,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustChange { pub at: u64, pub from: i16, pub to: i16, pub reason: String, pub author: Author }

#[derive(Clone, Debug)]
pub struct User {
    pub id: u64,
    /// The user deleted their account (design 9): their name, contact and devices' answers are gone.
    pub erased: bool,
    pub display_name: String,
    pub trust: Trust,
    pub history: Vec<TrustChange>,
    pub devices: Vec<[u8; 32]>,
    requests: Bucket,
    messages: Bucket,
    client_ids: HashMap<String, u64>,
    repeats: HashMap<[u8; 32], (u64, u64)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Post { pub n: u32, pub at: u64, pub author: Author, pub message: Message, pub answered: bool, pub received: Vec<bool> }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change { pub at: u64, pub from: State, pub to: State, pub role: Role, pub author: Author, pub reason: String }

/// A request and its thread.
#[derive(Clone, Debug)]
pub struct Item {
    pub id: u64,
    pub user: u64,
    pub device: [u8; 32],
    pub form: Request,
    pub state: State,
    pub filed: u64,
    pub since: u64,
    pub received: Vec<bool>,
    pub thread: Vec<Post>,
    pub changes: Vec<Change>,
    pub track: Option<String>,
    pub task: Option<String>,
    pub duplicate_of: Option<u64>,
    pub release: Option<String>,
    /// The state a question was asked in, where the user's answer takes it back.
    pub before_question: Option<State>,
    /// The trust's raise for an acceptance was given.
    pub credited: bool,
    /// Deleted by its user before it was accepted: its texts are gone, and it is shown to no one.
    pub deleted: bool,
    /// Why it asks for a credential or authority (`flag`), and whether the admin cleared it for the agents.
    pub flags: Vec<&'static str>,
    pub cleared: bool,
}

impl Item {
    pub fn open_question(&self, n: u32) -> bool {
        self.thread.iter().any(|p| p.n == n && p.message.kind == MessageKind::Question && !p.answered)
    }
    fn open_questions(&self) -> usize { self.thread.iter().filter(|p| p.message.kind == MessageKind::Question && !p.answered).count() }
    /// Proposal `n` waits for its user's answer: accepted, corrected or declined (hub-api 3, 100003-APP-0155).
    pub fn open_proposal(&self, n: u32) -> bool {
        self.thread.iter().any(|p| p.n == n && p.message.kind == MessageKind::Proposal && !p.answered)
    }
    /// A question or a proposal waits for the user.
    pub fn waits_for_user(&self) -> (bool, bool) {
        let open = |kind| self.thread.iter().any(|p| p.message.kind == kind && !p.answered);
        (open(MessageKind::Question), open(MessageKind::Proposal))
    }
    /// The user's answer to post `n`: its consent, if it was a proposal's.
    pub fn consent_to(&self, n: u32) -> Option<form::Consent> {
        self.thread.iter().find(|p| p.message.kind == MessageKind::Answer && p.message.answers == Some(n)).and_then(|p| p.message.consent)
    }
    /// Flagged and not cleared: it waits for the admin, as a held request does.
    pub fn waits_for_admin(&self) -> bool { !self.flags.is_empty() && !self.cleared && !self.deleted }
    /// Its user may delete it: it was never accepted into a track's plan.
    pub fn deletable(&self) -> bool { self.track.is_none() && !self.deleted }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code { pub owner: Owner, pub until: u64, pub used: bool }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session { pub owner: Owner, pub until: u64 }

#[derive(Clone, Debug, Default)]
struct Network { registrations: VecDeque<u64>, held: VecDeque<u64>, rejections: VecDeque<u64> }

/// A refusal: its HTTP status, the limit it met, in words, and when it ends (10.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal { pub status: u16, pub limit: String, pub detail: String, pub retry_after_ms: Option<u64>, pub earlier: Option<u64>, pub work_bits: Option<u8> }

pub fn refuse(status: u16, limit: &str, detail: impl Into<String>) -> Refusal {
    Refusal { status, limit: limit.into(), detail: detail.into(), retry_after_ms: None, earlier: None, work_bits: None }
}

fn wait(limit: &str, detail: impl Into<String>, ms: u64) -> Refusal { Refusal { retry_after_ms: Some(ms.max(1)), ..refuse(429, limit, detail) } }

fn form_refusal(e: form::Error) -> Refusal {
    match e {
        form::Error::TooLong(f, max) => refuse(413, f, format!("{f} is longer than {max}")),
        form::Error::Count(f, n) => refuse(413, f, format!("{f}: at most {n} at this trust")),
        form::Error::Size(f, n) => refuse(413, f, format!("{f}: at most {n} bytes each at this trust")),
        form::Error::Links(n) => refuse(413, "links", format!("at most {n} links below band 2")),
        form::Error::Empty(f) => refuse(400, f, format!("{f} is empty")),
        form::Error::Control(f) => refuse(400, f, format!("{f} holds a control character")),
        form::Error::Value(f) => refuse(400, f, format!("{f} is not one of the values it takes")),
    }
}

/// Who posts or moves a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Actor { Device([u8; 32]), Agent([u8; 32]), Admin(String) }

/// The extra a move may carry: the task an acceptance names, the request a duplicate points to, why a rejection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extra { pub track: Option<String>, pub task: Option<String>, pub duplicate_of: Option<u64>, pub not_a_request: bool, pub spam: bool }

/// The hub's whole state.
#[derive(Clone, Debug)]
pub struct Hub {
    pub keys: Keys,
    pub settings: Settings,
    devices: HashMap<[u8; 32], Device>,
    users: Vec<User>,
    requests: Vec<Item>,
    networks: HashMap<String, Network>,
    /// Filings by users below band 2 in the last hour.
    global: VecDeque<u64>,
    /// New keys from every network in the last hour, which raise the proof of work.
    recent: VecDeque<u64>,
    codes: HashMap<[u8; 32], Code>,
    sessions: HashMap<[u8; 32], Session>,
    /// Codes typed wrong, by network, in the last hour; not journaled, as a refusal changes no state.
    failures: HashMap<String, VecDeque<u64>>,
    /// Events applied.
    pub events: u64,
}

fn prune(times: &mut VecDeque<u64>, now: u64, window: u64) { while times.front().is_some_and(|t| t + window <= now) { times.pop_front(); } }
fn within(times: &VecDeque<u64>, now: u64, window: u64) -> Vec<u64> { times.iter().copied().filter(|t| t + window > now).collect() }

pub fn agent_author(key: &[u8; 32], track: &str) -> Author { Author::Agent { track: track.into(), id: hex(&sign::id(key)[..8]) } }

fn message_text(text: &str) -> Result<(), Refusal> {
    Message { kind: MessageKind::Comment, text: text.into(), answers: None, consent: None, verdict: None, release: None, attachments: Vec::new() }.check().map_err(form_refusal)
}

fn reason_line(reason: &str) -> Result<(), Refusal> {
    if reason.trim().is_empty() { return Err(refuse(400, "reason", "reason is empty")); }
    if reason.chars().count() > form::REASON_MAX { return Err(refuse(413, "reason", format!("reason is longer than {}", form::REASON_MAX))); }
    if reason.chars().any(char::is_control) { return Err(refuse(400, "reason", "reason holds a control character")); }
    Ok(())
}

impl Hub {
    pub fn new(keys: Keys, settings: Settings) -> Hub {
        Hub { keys, settings, devices: HashMap::new(), users: Vec::new(), requests: Vec::new(), networks: HashMap::new(), global: VecDeque::new(), recent: VecDeque::new(),
              codes: HashMap::new(), sessions: HashMap::new(), failures: HashMap::new(), events: 0 }
    }

    // ---- reading ----

    pub fn device(&self, id: &[u8; 32]) -> Option<&Device> { self.devices.get(id) }
    pub fn device_by_key(&self, key: &[u8; 32]) -> Option<&Device> { self.devices.get(&sign::id(key)) }
    pub fn devices(&self) -> impl Iterator<Item = &Device> { self.devices.values() }
    pub fn user(&self, id: u64) -> Option<&User> { id.checked_sub(1).and_then(|i| self.users.get(i as usize)) }
    pub fn users(&self) -> &[User] { &self.users }
    pub fn request(&self, id: u64) -> Option<&Item> { id.checked_sub(1).and_then(|i| self.requests.get(i as usize)) }
    pub fn requests(&self) -> &[Item] { &self.requests }
    pub fn code(&self, code: &str) -> Option<&Code> { self.codes.get(&sha256(code.as_bytes())) }

    /// A session by its token, while it lasts and its device is not revoked.
    pub fn session(&self, token: &[u8; 32], now: u64) -> Option<&Session> {
        let s = self.sessions.get(&sha256(token))?;
        if s.until <= now { return None; }
        if let Owner::User { device, .. } = &s.owner {
            if self.devices.get(device).map(|d| d.state) != Some(DeviceState::Approved) { return None; }
        }
        Some(s)
    }

    /// What a device may do now, or why it may do nothing.
    pub fn access(&self, device: &[u8; 32]) -> Result<(Access, &Device, &User), Refusal> {
        let d = self.devices.get(device).ok_or_else(|| refuse(403, "unregistered", "this key is not registered: POST /hub/v1/devices first"))?;
        match d.state {
            DeviceState::Rejected => {
                let reason = d.notes.iter().rev().find(|n| n.kind == "reason").map(|n| n.text.as_str()).unwrap_or("");
                return Err(refuse(403, "rejected", format!("this key was rejected by the admin: {reason}")));
            }
            DeviceState::Revoked => return Err(refuse(403, "revoked", "this key was revoked")),
            _ => {}
        }
        let u = self.user(d.user).expect("a device's user");
        match trust::access(u.trust, d.state == DeviceState::Approved) {
            Access::Banned => Err(refuse(403, "ban", "the user is banned (trust -1023)")),
            a => Ok((a, d, u)),
        }
    }

    /// The limits a device's calls take: its band's once approved, PENDING's before; None for a negative trust.
    pub fn limits(&self, device: &[u8; 32]) -> Option<Limits> {
        match self.access(device).ok()?.0 { Access::Open(l) | Access::Pending(l) => Some(l), _ => None }
    }

    /// The requests and messages a device's user may still send now; a pending device's messages go to its registration.
    pub fn quotas(&self, device: &[u8; 32], now: u64) -> (u32, u32) {
        let (Some(l), Some(d)) = (self.limits(device), self.devices.get(device)) else { return (0, 0) };
        let u = self.user(d.user).expect("a device's user");
        let requests = u.requests.resized(l.burst, l.requests_per_day, now).tokens(now);
        if d.state == DeviceState::Pending {
            let held = self.requests.iter().any(|r| r.device == *device && r.state == State::Held);
            return (if held { 0 } else { requests }, d.messages.resized(l.messages_per_day, l.messages_per_day, now).tokens(now));
        }
        (requests, u.messages.resized(l.messages_per_day, l.messages_per_day, now).tokens(now))
    }

    /// Whether an agent reads this request: never a held one; its own track's, intake what is in intake, the integrator
    /// what a track took, and anyone a public one that is not about security (7.1).
    pub fn agent_reads(&self, agent: &Agent, r: &Item) -> bool {
        if r.state == State::Held || r.deleted || r.waits_for_admin() { return false; }
        let public = r.form.visibility == Visibility::Public && !r.form.security;
        let own = match agent.role {
            Role::Intake => r.track.is_none() || r.track.as_deref() == Some(agent.track.as_str()),
            Role::Integrator => r.track.is_some(),
            _ => r.track.as_deref() == Some(agent.track.as_str()),
        };
        own || public
    }

    /// An agent's queue: intake's new requests, a track's open ones, the integrator's in progress.
    pub fn queue(&self, agent: &Agent) -> Vec<&Item> {
        self.requests.iter().filter(|r| self.agent_reads(agent, r)).filter(|r| match agent.role {
            Role::Intake => r.state == State::New,
            Role::Integrator => r.state == State::InProgress,
            _ => r.track.as_deref() == Some(agent.track.as_str()) && !r.state.closed() && r.state != State::Held,
        }).collect()
    }

    /// The board: what became a task, public and not about security, with no user named (6.4).
    pub fn board(&self) -> Vec<BoardEntry> {
        self.requests.iter().filter(|r| r.state.on_board() && r.form.visibility == Visibility::Public && !r.form.security && !r.deleted)
            .filter_map(|r| {
                // The user's name with their leave alone, and none once they deleted their account.
                let author = self.user(r.user).filter(|u| r.form.named && !u.erased).map(|u| u.display_name.clone());
                Some(BoardEntry { id: r.id, title: r.form.title.clone(), kind: r.form.kind, track: r.track.clone()?, task: r.task.clone()?,
                                  state: r.state, since_ms: r.since, release: r.release.clone(), author })
            })
            .collect()
    }

    /// The proof of work a new key needs now: off while the admin's floor is 0; above it, a bit for WORK_RAISED_FROM new
    /// keys in the last hour and one more for each doubling, up to BITS_MAX (5.4).
    pub fn work_bits(&self, now: u64) -> u8 {
        let floor = self.settings.work_bits;
        if floor == 0 { return 0; }
        let n = within(&self.recent, now, HOUR_MS).len();
        let raised = if n < WORK_RAISED_FROM { 0 } else { (n / WORK_RAISED_FROM).ilog2() + 1 };
        (floor as u32 + raised).min(BITS_MAX as u32) as u8
    }

    // ---- commands ----

    /// `POST devices` (4.2), signed by `signer` over `challenge`.
    pub fn register(&self, now: u64, reg: &Registration, signer: &[u8; 32], challenge: &[u8; 32], network: &str) -> Result<(Vec<Event>, [u8; 32]), Refusal> {
        if reg.public_key != *signer { return Err(refuse(400, "public_key", "public_key is not the key that signed the request")); }
        reg.check().map_err(form_refusal)?;
        let id = sign::id(signer);
        if let Some(d) = self.devices.get(&id) {
            return match d.state {
                DeviceState::Rejected => Err(refuse(403, "rejected", "this key was rejected by the admin")),
                DeviceState::Revoked => Err(refuse(403, "revoked", "this key was revoked")),
                _ => Ok((Vec::new(), id)),
            };
        }
        if self.keys.admins.contains_key(signer) || self.keys.agents.contains_key(signer) {
            return Err(refuse(400, "public_key", "an admin's or an agent's key is not a device's"));
        }
        let bits = self.work_bits(now);
        if bits > 0 && !reg.work.is_some_and(|n| work::check(challenge, n, bits)) {
            return Err(Refusal { work_bits: Some(bits), ..refuse(401, "work", format!("a proof of work of {bits} bits over the challenge is asked")) });
        }
        let (user, display_name, code) = match &reg.joining {
            Joining::NewUser { display_name } => {
                let net = self.networks.get(network);
                let regs = net.map(|n| within(&n.registrations, now, DAY_MS)).unwrap_or_default();
                let rejected = net.is_some_and(|n| !within(&n.rejections, now, DAY_MS).is_empty());
                let allowed = if rejected { REGISTRATIONS_AFTER_REJECTION } else { REGISTRATIONS_A_DAY };
                if regs.len() >= allowed {
                    let ends = regs[regs.len() - allowed] + DAY_MS - now;
                    let why = if rejected { "new keys a day from this network after a rejection" } else { "new keys a day from this network" };
                    return Err(wait("registrations", format!("{allowed} {why}"), ends));
                }
                (self.users.len() as u64 + 1, Some(display_name.clone()), None)
            }
            Joining::Pair { code } => {
                let hash = sha256(code.as_bytes());
                let Some(Code { owner: Owner::User { user, .. }, until, used: false }) = self.codes.get(&hash) else {
                    return Err(refuse(403, "pair_code", "the pairing code is not one the hub gave, or it was used"));
                };
                if *until <= now { return Err(refuse(403, "pair_code", "the pairing code expired: codes last 10 minutes")); }
                if self.user(*user).is_some_and(|u| u.trust.value() == trust::MIN) { return Err(refuse(403, "ban", "the user is banned (trust -1023)")); }
                (*user, None, Some(hash))
            }
        };
        Ok((vec![Event::Registered { at: now, key: *signer, name: reg.name.clone(), kind: reg.kind, platform: reg.platform.clone(), version: reg.version.clone(),
                                     network: network.into(), user, display_name, contact: reg.contact.clone(), code }], id))
    }

    /// The admin's decision on a pending device, with its message (4.2); held requests follow it (4.3).
    pub fn decide(&self, now: u64, admin: &str, device: &[u8; 32], decision: Decision, text: &str) -> Result<Vec<Event>, Refusal> {
        let d = self.devices.get(device).ok_or_else(|| refuse(404, "device", "no such device"))?;
        if d.state != DeviceState::Pending { return Err(refuse(409, "state", format!("the device is {}, not pending", d.state.name()))); }
        message_text(text)?;
        let mut out = vec![Event::Decided { at: now, device: *device, admin: admin.into(), decision, text: text.into() }];
        let (to, reason) = match decision {
            Decision::Approve => (State::New, "the device was approved"),
            Decision::Reject => (State::Rejected, "the device was rejected"),
            Decision::Ask => return Ok(out),
        };
        for r in self.requests.iter().filter(|r| r.device == *device && r.state == State::Held) {
            out.push(Event::Moved { at: now, id: r.id, to, role: Role::Hub, author: Author::Hub, reason: reason.into(), track: None, task: None, duplicate_of: None, release: None });
        }
        Ok(out)
    }

    /// A pending device's answer in its registration's conversation, 5 a day.
    pub fn answer_registration(&self, now: u64, device: &[u8; 32], text: &str) -> Result<Vec<Event>, Refusal> {
        let (_, d, _) = self.access(device)?;
        if d.state != DeviceState::Pending { return Err(refuse(409, "state", "the registration is decided; messages go to requests now")); }
        message_text(text)?;
        if let Err(w) = d.messages.resized(PENDING.messages_per_day, PENDING.messages_per_day, now).take(now) {
            return Err(wait("messages", format!("{} messages a day while pending", PENDING.messages_per_day), w.unwrap_or(DAY_MS)));
        }
        Ok(vec![Event::Answered { at: now, device: *device, text: text.into() }])
    }

    /// A device revoked by another of its user's or by the admin (4.4); its held requests are dropped.
    pub fn revoke(&self, now: u64, by: &Actor, device: &[u8; 32]) -> Result<Vec<Event>, Refusal> {
        let target = self.devices.get(device).ok_or_else(|| refuse(404, "device", "no such device"))?;
        let author = match by {
            Actor::Device(mine) => {
                let (_, d, _) = self.access(mine)?;
                if d.state != DeviceState::Approved { return Err(refuse(403, "pending", "a pending device revokes nothing")); }
                if target.user != d.user { return Err(refuse(404, "device", "no such device of this user")); }
                Author::User
            }
            Actor::Admin(name) => Author::Admin(name.clone()),
            Actor::Agent(_) => return Err(refuse(403, "role", "an agent revokes no device")),
        };
        if matches!(target.state, DeviceState::Rejected | DeviceState::Revoked) { return Err(refuse(409, "state", format!("the device is {} already", target.state.name()))); }
        let mut out = vec![Event::Revoked { at: now, device: *device, author }];
        for r in self.requests.iter().filter(|r| r.device == *device && r.state == State::Held) {
            out.push(Event::Moved { at: now, id: r.id, to: State::Rejected, role: Role::Hub, author: Author::Hub, reason: "the device was revoked".into(), track: None, task: None, duplicate_of: None, release: None });
        }
        Ok(out)
    }

    /// A one-time code of 8 characters for 10 minutes: a user's, to pair a device or open a browser's session, or an
    /// admin's, to open the admin pages. The code itself is returned; the journal keeps its SHA-256.
    pub fn make_code(&self, now: u64, owner: Owner, code: &str) -> Result<Vec<Event>, Refusal> {
        if let Owner::User { device, .. } = &owner {
            let (_, d, _) = self.access(device)?;
            if d.state != DeviceState::Approved { return Err(refuse(403, "pending", "a device pairs others once it is approved")); }
        }
        let live: Vec<u64> = self.codes.values().filter(|c| same_owner(&c.owner, &owner) && !c.used && c.until > now).map(|c| c.until).collect();
        if live.len() >= CODES_AT_ONCE {
            return Err(wait("codes", format!("{CODES_AT_ONCE} codes not used yet at once"), live.iter().min().unwrap() - now));
        }
        let owner = match owner { Owner::User { device, .. } => Owner::User { user: self.devices[&device].user, device }, o => o };
        Ok(vec![Event::Code { at: now, code: sha256(code.as_bytes()), owner, until: now + CODE_MS }])
    }

    /// A browser's session opened by a code: the user's pages, or with `admin` the admin pages.
    pub fn open_session(&self, now: u64, code: &str, admin: bool, token: &[u8; 32], network: &str) -> Result<Vec<Event>, Refusal> {
        if let Some(f) = self.failures.get(network) {
            let recent = within(f, now, HOUR_MS);
            if recent.len() >= SIGN_IN_FAILURES { return Err(wait("sign-in", format!("{SIGN_IN_FAILURES} wrong codes an hour from this network"), recent[0] + HOUR_MS - now)); }
        }
        let hash = sha256(code.trim().to_ascii_uppercase().as_bytes());
        let c = self.codes.get(&hash).filter(|c| !c.used && c.until > now && matches!(c.owner, Owner::Admin(_)) == admin);
        let Some(c) = c else { return Err(refuse(403, "code", "the code is not one the hub gave for this, or it was used or expired")) };
        if let Owner::User { device, .. } = &c.owner { self.access(device)?; }
        let until = now + if admin { ADMIN_SESSION_MS } else { SESSION_MS };
        Ok(vec![Event::Session { at: now, token: sha256(token), code: hash, until }])
    }

    /// A wrong code typed from a network, counted for its hour.
    pub fn note_failure(&mut self, network: &str, now: u64) {
        let f = self.failures.entry(network.into()).or_default();
        prune(f, now, HOUR_MS);
        f.push_back(now);
    }

    pub fn sign_out(&self, now: u64, token: &[u8; 32]) -> Vec<Event> {
        if self.sessions.contains_key(&sha256(token)) { vec![Event::SignedOut { at: now, token: sha256(token) }] } else { Vec::new() }
    }

    /// `POST requests` (4.3, 5.2, 5.4): the request's ID and whether it was filed before by its client ID.
    pub fn file(&self, now: u64, device: &[u8; 32], form: &Request) -> Result<(Vec<Event>, u64, bool), Refusal> {
        let (access, d, u) = self.access(device)?;
        let limits = match access {
            Access::Open(l) | Access::Pending(l) => l,
            _ => return Err(refuse(403, "trust", format!("a negative trust ({}) takes no new requests; the user's own are still read", u.trust.value()))),
        };
        let pending = d.state == DeviceState::Pending;
        if pending && self.settings.approved_only { return Err(refuse(403, "intake", "the hub takes requests from approved devices alone (intake approved-only)")); }
        form.check(&limits).map_err(form_refusal)?;
        if let Some(&id) = u.client_ids.get(&form.client_id) { return Ok((Vec::new(), id, true)); }
        let digest = repeat::digest(&form.title, &form.text);
        if let Some(&(earlier, at)) = u.repeats.get(&digest) {
            if at + repeat::DAYS * DAY_MS > now {
                return Err(Refusal { earlier: Some(earlier), ..refuse(409, "repeat", format!("the same request was sent in the last {} days as {earlier}", repeat::DAYS)) });
            }
        }
        if pending {
            if self.requests.iter().any(|r| r.device == *device && r.state == State::Held) {
                return Err(refuse(403, "held", "a pending device has one held request at a time, until the admin decides"));
            }
            let held = self.networks.get(&d.network).map(|n| within(&n.held, now, DAY_MS)).unwrap_or_default();
            if held.len() >= HELD_A_DAY { return Err(wait("held", format!("{HELD_A_DAY} held requests a day from one network"), held[held.len() - HELD_A_DAY] + DAY_MS - now)); }
        }
        if u.trust.band().is_some_and(|b| b < 2) {
            let recent = within(&self.global, now, HOUR_MS);
            let cap = self.settings.global_cap as usize;
            if recent.len() >= cap { return Err(wait("intake", format!("{cap} new requests an hour from users below band 2"), recent[recent.len() - cap] + HOUR_MS - now)); }
        }
        if let Err(w) = u.requests.resized(limits.burst, limits.requests_per_day, now).take(now) {
            return Err(wait("requests", format!("{} requests a day, a burst of {}, at this trust", limits.requests_per_day, limits.burst), w.unwrap_or(DAY_MS)));
        }
        let id = self.requests.len() as u64 + 1;
        Ok((vec![Event::Filed { at: now, id, user: u.id, device: *device, form: form.clone(), held: pending }], id, false))
    }

    /// An attachment's bytes, checked against its entry: the request's own `n`th, or with `post` the `n`th of the
    /// user's message there. Storing them is the caller's, before the event is written.
    pub fn attach(&self, now: u64, device: &[u8; 32], id: u64, post: Option<u32>, n: u32, bytes: &[u8]) -> Result<Vec<Event>, Refusal> {
        let (_, d, _) = self.access(device)?;
        let r = self.request(id).filter(|r| r.user == d.user && !r.deleted).ok_or_else(|| refuse(404, "request", "no such request of this user"))?;
        let (entries, received) = match post {
            None => (&r.form.attachments, &r.received),
            Some(p) => {
                let p = r.thread.iter().find(|x| x.n == p && x.author == Author::User).ok_or_else(|| refuse(404, "message", "no such message of this user"))?;
                (&p.message.attachments, &p.received)
            }
        };
        let entry = entries.get(n as usize).ok_or_else(|| refuse(404, "attachment", "no such attachment there"))?;
        if received[n as usize] { return Ok(Vec::new()); }
        if r.state.closed() && post.is_none() { return Err(refuse(409, "state", "the request is closed")); }
        if bytes.len() as u64 != entry.size { return Err(refuse(400, "size", format!("the attachment is {} bytes, its entry says {}", bytes.len(), entry.size))); }
        if sha256(bytes) != entry.sha256 { return Err(refuse(400, "sha256", "the bytes do not match the entry's SHA-256")); }
        Ok(vec![Event::Attached { at: now, id, post, n }])
    }

    /// A request deleted by its user, before it was accepted (design 9).
    pub fn delete_request(&self, now: u64, device: &[u8; 32], id: u64) -> Result<Vec<Event>, Refusal> {
        let (_, d, _) = self.access(device)?;
        let r = self.request(id).filter(|r| r.user == d.user && !r.deleted).ok_or_else(|| refuse(404, "request", "no such request of this user"))?;
        if !r.deletable() { return Err(refuse(409, "accepted", format!("request {id} was accepted as {}: its task stays; ask the admin", r.task.as_deref().unwrap_or("a task")))); }
        Ok(vec![Event::Deleted { at: now, id, author: Author::User }])
    }

    /// A user's account deleted from one of their devices (design 9): every request not accepted deleted, every device
    /// revoked, their names, contact and answers erased. Accepted requests stay, with no name.
    pub fn erase(&self, now: u64, device: &[u8; 32]) -> Result<Vec<Event>, Refusal> {
        let (_, d, u) = self.access(device)?;
        if d.state != DeviceState::Approved { return Err(refuse(403, "pending", "a pending device deletes nothing: the admin's rejection does")); }
        let mut out: Vec<Event> = self.requests.iter().filter(|r| r.user == u.id && r.deletable())
            .map(|r| Event::Deleted { at: now, id: r.id, author: Author::User }).collect();
        for id in &u.devices {
            if matches!(self.devices[id].state, DeviceState::Pending | DeviceState::Approved) { out.push(Event::Revoked { at: now, device: *id, author: Author::User }); }
        }
        out.push(Event::Erased { at: now, user: u.id, author: Author::User });
        Ok(out)
    }

    /// The admin clears a request's flag, with the reason: the agents read it from then on.
    pub fn clear_flag(&self, now: u64, admin: &str, id: u64, reason: &str) -> Result<Vec<Event>, Refusal> {
        reason_line(reason)?;
        let r = self.request(id).filter(|r| !r.deleted).ok_or_else(|| refuse(404, "request", "no such request"))?;
        if !r.waits_for_admin() { return Err(refuse(409, "flag", "the request is not flagged, or its flag was cleared")); }
        Ok(vec![Event::Cleared { at: now, id, admin: admin.into(), reason: reason.into() }])
    }

    /// A message in a request's thread (6.2), and the moves it makes: a question to needs-info, the last answer back,
    /// a verdict to verified or reopened (6.3).
    pub fn post(&self, now: u64, by: &Actor, id: u64, message: &Message) -> Result<Vec<Event>, Refusal> {
        message.check().map_err(form_refusal)?;
        let r = self.request(id).filter(|r| !r.deleted).ok_or_else(|| refuse(404, "request", "no such request"))?;
        let (role, author) = match by {
            Actor::Device(device) => {
                let (access, d, u) = self.access(device)?;
                let limits = match access {
                    Access::Open(l) => l,
                    Access::Pending(_) => return Err(refuse(403, "pending", "a pending device's messages go to its registration")),
                    _ => return Err(refuse(403, "trust", format!("a negative trust ({}) is read-only", u.trust.value()))),
                };
                if r.user != d.user { return Err(refuse(404, "request", "no such request of this user")); }
                if !form::may_post(Role::User, message.kind) { return Err(refuse(403, "kind", format!("a user posts no {}", message.kind.name()))); }
                form::attachments_within(&message.attachments, &limits).map_err(form_refusal)?;
                let answer = message.kind == MessageKind::Answer;
                if answer {
                    // A question's answer has no consent; a proposal's accepts, corrects or declines it (hub-api 3).
                    let n = message.answers.unwrap_or(0);
                    match (r.open_question(n), r.open_proposal(n), message.consent) {
                        (true, _, None) | (false, true, Some(_)) => {}
                        (true, _, Some(_)) => return Err(refuse(400, "consent", "an answer to a question takes no consent: accept, correct and decline answer a proposal")),
                        (false, true, None) => return Err(refuse(400, "consent", "an answer to a proposal accepts, corrects or declines it")),
                        (false, false, _) => return Err(refuse(400, "answers", "answers names no open question or proposal of the thread")),
                    }
                }
                if !answer {
                    if let Err(w) = u.messages.resized(limits.messages_per_day, limits.messages_per_day, now).take(now) {
                        return Err(wait("messages", format!("{} messages a day at this trust", limits.messages_per_day), w.unwrap_or(DAY_MS)));
                    }
                }
                if message.kind == MessageKind::Verdict {
                    let fits = match message.verdict { Some(Verdict::Works) => r.state == State::AwaitingCheck, _ => matches!(r.state, State::AwaitingCheck | State::ClosedUnchecked) };
                    if !fits { return Err(refuse(409, "state", format!("a verdict of {} is not taken in {}", message.verdict.map(Verdict::name).unwrap_or(""), r.state.name()))); }
                }
                (Role::User, Author::User)
            }
            Actor::Agent(key) => {
                let agent = self.keys.agents.get(key).ok_or_else(|| refuse(403, "key", "not an agent's key"))?;
                if !self.agent_reads(agent, r) { return Err(refuse(404, "request", "no such request for this agent")); }
                if !form::may_post(agent.role, message.kind) { return Err(refuse(403, "kind", format!("an agent posts no {}", message.kind.name()))); }
                (agent.role, agent_author(key, &agent.track))
            }
            Actor::Admin(name) => {
                if !form::may_post(Role::Admin, message.kind) { return Err(refuse(403, "kind", format!("the admin posts no {}", message.kind.name()))); }
                (Role::Admin, Author::Admin(name.clone()))
            }
        };
        if r.state.closed() && message.kind != MessageKind::Verdict && role != Role::Admin { return Err(refuse(409, "state", format!("the request is {}", r.state.name()))); }
        let mut out = vec![Event::Posted { at: now, id, author: author.clone(), message: message.clone() }];
        let mut moved = |to: State, role: Role, author: Author, reason: String| out.push(Event::Moved { at: now, id, to, role, author, reason, track: None, task: None, duplicate_of: None, release: None });
        match message.kind {
            MessageKind::Question if r.state != State::NeedsInfo => {
                if !state::allowed(r.state, State::NeedsInfo, role) || (role == Role::Track && !self.own_track(by, r)) {
                    return Err(refuse(409, "state", format!("no question from {} while {}", role.name(), r.state.name())));
                }
                moved(State::NeedsInfo, role, author.clone(), "a question for the user".into());
            }
            MessageKind::Answer if r.state == State::NeedsInfo && r.open_question(message.answers.unwrap_or(0)) && r.open_questions() == 1 => {
                let back = r.before_question.unwrap_or(State::New);
                moved(back, Role::User, Author::User, "the user answered".into());
            }
            MessageKind::Verdict => {
                let release = message.release.clone().unwrap_or_default();
                match message.verdict {
                    Some(Verdict::Works) => {
                        moved(State::Verified, Role::User, Author::User, format!("the user's verdict: works on {release}"));
                        out.extend(self.raise(now, r.user, trust::Event::Verified, id));
                    }
                    Some(v) => moved(State::Reopened, Role::User, Author::User, format!("the user's verdict: {} on {release}", v.name())),
                    None => {}
                }
            }
            _ => {}
        }
        Ok(out)
    }

    fn own_track(&self, by: &Actor, r: &Item) -> bool {
        match by { Actor::Agent(key) => self.keys.agents.get(key).is_some_and(|a| r.track.as_deref() == Some(a.track.as_str())), _ => true }
    }

    /// The trust's own change for an event, when it changes anything (5.3).
    fn raise(&self, now: u64, user: u64, event: trust::Event, id: u64) -> Option<Event> {
        let u = self.user(user)?;
        let to = u.trust.after(event);
        (to != u.trust).then(|| Event::Trust { at: now, user, to: to.value(), reason: format!("request {id}: {}", event.name()), author: Author::Hub })
    }

    /// A state's move by an agent or the admin (6.1, 7.2), checked by the state machine.
    pub fn move_request(&self, now: u64, by: &Actor, id: u64, to: State, reason: &str, extra: &Extra) -> Result<Vec<Event>, Refusal> {
        reason_line(reason)?;
        let r = self.request(id).filter(|r| !r.deleted).ok_or_else(|| refuse(404, "request", "no such request"))?;
        let (role, author) = match by {
            Actor::Agent(key) => {
                let agent = self.keys.agents.get(key).ok_or_else(|| refuse(403, "key", "not an agent's key"))?;
                if !self.agent_reads(agent, r) { return Err(refuse(404, "request", "no such request for this agent")); }
                if agent.role == Role::Track && r.track.as_deref() != Some(agent.track.as_str()) { return Err(refuse(403, "track", "the request is another track's")); }
                if extra.spam { return Err(refuse(403, "spam", "only the admin marks spam")); }
                (agent.role, agent_author(key, &agent.track))
            }
            Actor::Admin(name) => (Role::Admin, Author::Admin(name.clone())),
            Actor::Device(_) => return Err(refuse(403, "role", "a user moves a request by answering or by a verdict")),
        };
        if !state::allowed(r.state, to, role) { return Err(refuse(409, "state", format!("{} does not move a request from {} to {}", role.name(), r.state.name(), to.name()))); }
        let (mut track, mut task, mut duplicate_of) = (None, None, None);
        match to {
            State::Accepted => {
                let t = extra.track.as_deref().unwrap_or("");
                if t.len() != 3 || !t.bytes().all(|b| b.is_ascii_uppercase()) { return Err(refuse(400, "track", "an acceptance names the track, three capital letters")); }
                let k = extra.task.as_deref().unwrap_or("");
                if k.is_empty() || k.len() > form::TASK_MAX || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                    return Err(refuse(400, "task", format!("an acceptance names the task, NNN-TRK-MMMM, at most {} characters", form::TASK_MAX)));
                }
                track = Some(t.to_string());
                task = Some(k.to_string());
            }
            State::Duplicate => {
                let other = extra.duplicate_of.filter(|&o| o != id && self.request(o).is_some()).ok_or_else(|| refuse(400, "duplicate_of", "a duplicate names another request"))?;
                duplicate_of = Some(other);
            }
            _ => {}
        }
        let mut out = vec![Event::Moved { at: now, id, to, role, author, reason: reason.into(), track, task, duplicate_of, release: None }];
        if to == State::Accepted && !r.credited { out.extend(self.raise(now, r.user, trust::Event::Accepted, id)); }
        if to == State::Rejected {
            if extra.spam { out.extend(self.raise(now, r.user, trust::Event::Spam, id)); }
            else if extra.not_a_request { out.extend(self.raise(now, r.user, trust::Event::NotARequest, id)); }
        }
        Ok(out)
    }

    /// The admin's word that a release carries landed requests (stage 3 takes it from the release itself, 8.4), and
    /// those whose user runs it already to awaiting-check.
    pub fn delivered(&self, now: u64, admin: &str, release: &str, ids: &[u64]) -> Result<Vec<Event>, Refusal> {
        if release.is_empty() || release.len() > form::SHORT_MAX || !release.bytes().all(|b| b.is_ascii_graphic()) { return Err(refuse(400, "release", "a release's name, at most 32 characters")); }
        let mut out = Vec::new();
        for &id in ids {
            let r = self.request(id).filter(|r| !r.deleted).ok_or_else(|| refuse(404, "request", format!("no request {id}")))?;
            if r.state != State::Landed { return Err(refuse(409, "state", format!("request {id} is {}, not landed", r.state.name()))); }
            out.push(Event::Moved { at: now, id, to: State::Delivered, role: Role::Hub, author: Author::Admin(admin.into()), reason: format!("release {release} carries it"),
                                    track: None, task: None, duplicate_of: None, release: Some(release.into()) });
            let runs = self.user(r.user).into_iter().flat_map(|u| &u.devices).filter_map(|d| self.devices.get(d))
                .find(|d| d.state == DeviceState::Approved && at_least(&d.version, release));
            if let Some(d) = runs {
                out.push(Event::Moved { at: now, id, to: State::AwaitingCheck, role: Role::Hub, author: Author::Hub, reason: format!("the user's device runs {}", d.version),
                                        track: None, task: None, duplicate_of: None, release: None });
            }
        }
        Ok(out)
    }

    /// A device's version, and the delivered requests it now runs (6.3).
    pub fn report_version(&self, now: u64, device: &[u8; 32], version: &str) -> Result<Vec<Event>, Refusal> {
        let (_, d, u) = self.access(device)?;
        if version.is_empty() || version.chars().count() > form::SHORT_MAX || version.chars().any(char::is_control) { return Err(refuse(400, "version", "a version, at most 32 characters")); }
        if d.version == version { return Ok(Vec::new()); }
        let mut out = vec![Event::Version { at: now, device: *device, version: version.into() }];
        if d.state == DeviceState::Approved {
            for r in self.requests.iter().filter(|r| r.user == u.id && r.state == State::Delivered) {
                if r.release.as_deref().is_some_and(|rel| at_least(version, rel)) {
                    out.push(Event::Moved { at: now, id: r.id, to: State::AwaitingCheck, role: Role::Hub, author: Author::Hub, reason: format!("the user's device runs {version}"),
                                            track: None, task: None, duplicate_of: None, release: None });
                }
            }
        }
        Ok(out)
    }

    /// The admin sets a user's trust, with the reason (5.1).
    pub fn set_trust(&self, now: u64, admin: &str, user: u64, to: i16, reason: &str) -> Result<Vec<Event>, Refusal> {
        reason_line(reason)?;
        self.user(user).ok_or_else(|| refuse(404, "user", "no such user"))?;
        Trust::new(to).ok_or_else(|| refuse(400, "trust", "a trust is from -1023 to 1023"))?;
        Ok(vec![Event::Trust { at: now, user, to, reason: reason.into(), author: Author::Admin(admin.into()) }])
    }

    pub fn set_setting(&self, now: u64, admin: &str, name: &str, value: &str) -> Result<Vec<Event>, Refusal> {
        self.settings.clone().set(name, value).map_err(|why| refuse(400, "setting", why))?;
        Ok(vec![Event::Setting { at: now, name: name.into(), value: value.into(), admin: admin.into() }])
    }

    /// The hub's own moves on its clock: no verdict in 30 days after awaiting-check (6.3).
    pub fn sweep(&self, now: u64) -> Vec<Event> {
        self.requests.iter().filter(|r| r.state == State::AwaitingCheck && r.since + CHECK_DAYS * DAY_MS <= now)
            .map(|r| Event::Moved { at: now, id: r.id, to: State::ClosedUnchecked, role: Role::Hub, author: Author::Hub, reason: format!("no verdict in {CHECK_DAYS} days"),
                                    track: None, task: None, duplicate_of: None, release: None })
            .collect()
    }

    // ---- applying ----

    /// The state after one event; an event that does not fit the state is refused, naming why, and nothing changes.
    pub fn apply(&mut self, e: &Event) -> Result<(), String> {
        self.check(e)?;
        match e {
            Event::Registered { at, key, name, kind, platform, version, network, user, display_name, contact, code } => {
                if let Some(display_name) = display_name {
                    self.users.push(User { id: *user, erased: false, display_name: display_name.clone(), trust: Trust::new(0).unwrap(), history: Vec::new(), devices: Vec::new(),
                                           requests: Bucket::new(PENDING.burst, PENDING.requests_per_day, *at), messages: Bucket::new(BANDS[0].messages_per_day, BANDS[0].messages_per_day, *at),
                                           client_ids: HashMap::new(), repeats: HashMap::new() });
                }
                let id = sign::id(key);
                self.users[*user as usize - 1].devices.push(id);
                if let Some(c) = code { self.codes.get_mut(c).unwrap().used = true; } else {
                    prune(&mut self.recent, *at, HOUR_MS);
                    self.recent.push_back(*at);
                    let n = self.networks.entry(network.clone()).or_default();
                    prune(&mut n.registrations, *at, DAY_MS);
                    n.registrations.push_back(*at);
                }
                self.devices.insert(id, Device { id, key: *key, name: name.clone(), kind: *kind, platform: platform.clone(), version: version.clone(), network: network.clone(),
                                                 contact: contact.clone(), user: *user, state: if code.is_some() { DeviceState::Approved } else { DeviceState::Pending },
                                                 at: *at, paired: code.is_some(), notes: Vec::new(), messages: Bucket::new(PENDING.messages_per_day, PENDING.messages_per_day, *at) });
            }
            Event::Decided { at, device, admin, decision, text } => {
                let d = self.devices.get_mut(device).unwrap();
                let kind = match decision { Decision::Approve => "welcome", Decision::Ask => "question", Decision::Reject => "reason" };
                d.notes.push(Note { at: *at, from: Author::Admin(admin.clone()), kind, text: text.clone() });
                let user = d.user;
                match decision {
                    Decision::Approve => { d.state = DeviceState::Approved; self.rebucket(user, *at); }
                    Decision::Reject => {
                        d.state = DeviceState::Rejected;
                        let n = self.networks.entry(d.network.clone()).or_default();
                        prune(&mut n.rejections, *at, DAY_MS);
                        n.rejections.push_back(*at);
                    }
                    Decision::Ask => {}
                }
            }
            Event::Answered { at, device, text } => {
                let d = self.devices.get_mut(device).unwrap();
                d.messages = d.messages.resized(PENDING.messages_per_day, PENDING.messages_per_day, *at);
                let _ = d.messages.take(*at);
                d.notes.push(Note { at: *at, from: Author::User, kind: "answer", text: text.clone() });
            }
            Event::Revoked { at, device, author } => {
                let d = self.devices.get_mut(device).unwrap();
                d.state = DeviceState::Revoked;
                d.notes.push(Note { at: *at, from: author.clone(), kind: "revoked", text: "the device was revoked".into() });
                self.sessions.retain(|_, s| !matches!(&s.owner, Owner::User { device: d, .. } if d == device));
            }
            Event::Code { code, owner, until, .. } => { self.codes.insert(*code, Code { owner: owner.clone(), until: *until, used: false }); }
            Event::Session { token, code, until, .. } => {
                let c = self.codes.get_mut(code).unwrap();
                c.used = true;
                let owner = c.owner.clone();
                self.sessions.insert(*token, Session { owner, until: *until });
            }
            Event::SignedOut { token, .. } => { self.sessions.remove(token); }
            Event::Filed { at, id, user, device, form, held } => {
                let limits = if *held { PENDING } else { BANDS[self.users[*user as usize - 1].trust.band().unwrap_or(0) as usize] };
                let u = &mut self.users[*user as usize - 1];
                u.requests = u.requests.resized(limits.burst, limits.requests_per_day, *at);
                let _ = u.requests.take(*at);
                u.client_ids.insert(form.client_id.clone(), *id);
                u.repeats.insert(repeat::digest(&form.title, &form.text), (*id, *at));
                if u.trust.band().is_some_and(|b| b < 2) {
                    prune(&mut self.global, *at, HOUR_MS);
                    self.global.push_back(*at);
                }
                if *held {
                    let network = self.devices[device].network.clone();
                    let n = self.networks.entry(network).or_default();
                    prune(&mut n.held, *at, DAY_MS);
                    n.held.push_back(*at);
                }
                let state = state::first(!*held);
                let status = if *held { "held: the device waits for the admin's decision" } else { "new: in intake" };
                let mut item = Item { id: *id, user: *user, device: *device, form: form.clone(), state, filed: *at, since: *at, received: vec![false; form.attachments.len()],
                                      thread: Vec::new(), changes: Vec::new(), track: None, task: None, duplicate_of: None, release: None, before_question: None, credited: false,
                                      deleted: false, flags: crate::flag::reasons(&form.title, &form.text), cleared: false };
                item.thread.push(Post { n: 1, at: *at, author: Author::Hub, message: status_message(status.into()), answered: false, received: Vec::new() });
                self.requests.push(item);
            }
            Event::Attached { id, post, n, .. } => {
                let r = &mut self.requests[*id as usize - 1];
                match post {
                    None => r.received[*n as usize] = true,
                    Some(p) => r.thread.iter_mut().find(|x| x.n == *p).unwrap().received[*n as usize] = true,
                }
            }
            Event::Posted { at, id, author, message } => {
                let r = &mut self.requests[*id as usize - 1];
                if *author == Author::User && message.kind != MessageKind::Answer {
                    let u = &mut self.users[r.user as usize - 1];
                    let l = BANDS[u.trust.band().unwrap_or(0) as usize];
                    u.messages = u.messages.resized(l.messages_per_day, l.messages_per_day, *at);
                    let _ = u.messages.take(*at);
                }
                if message.kind == MessageKind::Answer {
                    if let Some(q) = r.thread.iter_mut().find(|p| Some(p.n) == message.answers) { q.answered = true; }
                }
                let n = r.thread.len() as u32 + 1;
                r.thread.push(Post { n, at: *at, author: author.clone(), message: message.clone(), answered: false, received: vec![false; message.attachments.len()] });
            }
            Event::Moved { at, id, to, role, author, reason, track, task, duplicate_of, release } => {
                let r = &mut self.requests[*id as usize - 1];
                let from = r.state;
                r.changes.push(Change { at: *at, from, to: *to, role: *role, author: author.clone(), reason: reason.clone() });
                let n = r.thread.len() as u32 + 1;
                r.thread.push(Post { n, at: *at, author: Author::Hub, message: status_message(format!("{} -> {}: {} ({})", from.name(), to.name(), reason, author.text())), answered: false, received: Vec::new() });
                if *to == State::NeedsInfo { r.before_question = Some(from); }
                if *to == State::Accepted { r.credited = true; }
                r.state = *to;
                r.since = *at;
                if track.is_some() { r.track = track.clone(); }
                if task.is_some() { r.task = task.clone(); }
                if duplicate_of.is_some() { r.duplicate_of = *duplicate_of; }
                if release.is_some() { r.release = release.clone(); }
            }
            Event::Trust { at, user, to, reason, author } => {
                let u = &mut self.users[*user as usize - 1];
                u.history.push(TrustChange { at: *at, from: u.trust.value(), to: *to, reason: reason.clone(), author: author.clone() });
                // The old band's credit up to now, the new band's from now on.
                self.rebucket(*user, *at);
                self.users[*user as usize - 1].trust = Trust::new(*to).unwrap();
                self.rebucket(*user, *at);
            }
            Event::Setting { name, value, .. } => { self.settings.set(name, value)?; }
            Event::Version { device, version, .. } => { self.devices.get_mut(device).unwrap().version = version.clone(); }
            Event::Deleted { id, .. } => {
                // The same as the journal keeps after its texts are erased (`Event::redacted`), so a replay gives this.
                let r = &mut self.requests[*id as usize - 1];
                r.deleted = true;
                r.form = event::redact_request(&r.form);
                for p in r.thread.iter_mut().filter(|p| p.message.kind != MessageKind::Status) { p.message = event::redact_message(&p.message); }
                r.flags.clear();
                let user = r.user;
                self.users[user as usize - 1].repeats.retain(|_, (request, _)| request != id);
            }
            Event::Erased { user, .. } => {
                let u = &mut self.users[*user as usize - 1];
                u.erased = true;
                u.display_name = event::ERASED_NAME.into();
                for id in u.devices.clone() {
                    let d = self.devices.get_mut(&id).unwrap();
                    d.name = event::ERASED_DEVICE.into();
                    d.contact = None;
                    d.network = event::ERASED_NETWORK.into();
                    for n in d.notes.iter_mut().filter(|n| n.kind == "answer") { n.text = event::DELETED.into(); }
                }
            }
            Event::Cleared { id, .. } => { self.requests[*id as usize - 1].cleared = true; }
        }
        self.events += 1;
        Ok(())
    }

    /// A user's buckets brought to `at` and to the limits they now take: their band's once a device of theirs is
    /// approved, PENDING's before; a negative trust leaves them as they are until it is positive again.
    fn rebucket(&mut self, user: u64, at: u64) {
        let approved = self.users[user as usize - 1].devices.iter().any(|d| self.devices.get(d).is_some_and(|d| d.state == DeviceState::Approved));
        let u = &mut self.users[user as usize - 1];
        let Some(band) = u.trust.band() else { return };
        let l = if approved { BANDS[band as usize] } else { PENDING };
        u.requests = u.requests.resized(l.burst, l.requests_per_day, at);
        let m = BANDS[band as usize].messages_per_day;
        u.messages = u.messages.resized(m, m, at);
    }

    /// Whether an event fits the state: the ones it names exist, a new one is the next, its move is the machine's.
    pub fn check(&self, e: &Event) -> Result<(), String> {
        let device = |id: &[u8; 32]| self.devices.get(id).map(|_| ()).ok_or_else(|| format!("no device {}", hex(id)));
        let request = |id: u64| self.request(id).ok_or_else(|| format!("no request {id}"));
        match e {
            Event::Registered { key, user, display_name, code, .. } => {
                if self.devices.contains_key(&sign::id(key)) { return Err("the device is registered already".into()); }
                let next = self.users.len() as u64 + 1;
                match (display_name, code) {
                    (Some(_), None) if *user == next => {}
                    (None, Some(c)) if self.user(*user).is_some() && self.codes.get(c).is_some_and(|c| !c.used) => {}
                    _ => return Err("the device's user is neither the next nor one with an unused code".into()),
                }
            }
            Event::Decided { device: d, .. } | Event::Answered { device: d, .. } | Event::Revoked { device: d, .. } | Event::Version { device: d, .. } => device(d)?,
            Event::Code { owner, .. } => if let Owner::User { user, device: d } = owner { device(d)?; self.user(*user).ok_or("no such user")?; },
            Event::Session { code, .. } => if !self.codes.get(code).is_some_and(|c| !c.used) { return Err("no unused code for the session".into()); },
            Event::SignedOut { .. } => {}
            Event::Setting { name, value, .. } => self.settings.clone().set(name, value)?,
            Event::Filed { id, user, device: d, form, .. } => {
                if *id != self.requests.len() as u64 + 1 { return Err(format!("request {id} is not the next")); }
                device(d)?;
                self.user(*user).ok_or("no such user")?;
                if form.check(&BANDS[7]).is_err() { return Err("the request is out of every bound".into()); }
            }
            Event::Attached { id, post, n, .. } => {
                let r = request(*id)?;
                let count = match post { None => r.received.len(), Some(p) => r.thread.iter().find(|x| x.n == *p).map_or(0, |x| x.received.len()) };
                if *n as usize >= count { return Err(format!("request {id} has no attachment {n} there")); }
            }
            Event::Deleted { id, .. } => if !request(*id)?.deletable() { return Err(format!("request {id} was accepted, or deleted already")); },
            Event::Erased { user, .. } => if self.user(*user).ok_or("no such user")?.erased { return Err("the user is erased already".into()); },
            // A replay of a journal whose texts were erased reads no flag where the live hub read one: the clearing fits.
            Event::Cleared { id, .. } => if request(*id)?.deleted { return Err(format!("request {id} is deleted")); },
            Event::Posted { id, message, .. } => { request(*id)?; message.check().map_err(|e| format!("{e:?}"))?; }
            Event::Moved { id, to, role, .. } => {
                let r = request(*id)?;
                if !state::allowed(r.state, *to, *role) { return Err(format!("{} does not move request {id} from {} to {}", role.name(), r.state.name(), to.name())); }
            }
            Event::Trust { user, to, .. } => { self.user(*user).ok_or("no such user")?; Trust::new(*to).ok_or("a trust out of its range")?; }
        }
        Ok(())
    }
}

fn status_message(text: String) -> Message { Message { kind: MessageKind::Status, text, answers: None, consent: None, verdict: None, release: None, attachments: Vec::new() } }

fn same_owner(a: &Owner, b: &Owner) -> bool {
    match (a, b) {
        (Owner::User { device: x, .. }, Owner::User { device: y, .. }) => x == y,
        (Owner::Admin(x), Owner::Admin(y)) => x == y,
        _ => false,
    }
}
