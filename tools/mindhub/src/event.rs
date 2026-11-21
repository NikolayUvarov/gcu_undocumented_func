//! The journal's events (docs/hub/README.md, 4 to 7): everything that changes the hub's state is one of them, written
//! before it is applied, so the state is the journal's replay (MC-10.3). One JSON object a line, its keys exactly these.

use improve_proto::form::{Attachment, DeviceKind, Message, Request};
use improve_proto::json::{self, Value, Writer};
use improve_proto::state::{Role, State};
use improve_proto::wire;

/// Who said or did it, as the thread and the journal name them (6.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Author {
    Hub,
    /// The request's own user.
    User,
    /// An agent, by its track and the first 8 bytes of its key's ID in hex.
    Agent { track: String, id: String },
    /// An admin, by their name in the configuration.
    Admin(String),
}

impl Author {
    pub fn text(&self) -> String {
        match self {
            Author::Hub => "hub".into(),
            Author::User => "user".into(),
            Author::Agent { track, id } => format!("agent {track} {id}"),
            Author::Admin(name) => format!("admin {name}"),
        }
    }
    pub fn from_text(text: &str) -> Option<Author> {
        let parts: Vec<&str> = text.split(' ').collect();
        match parts.as_slice() {
            ["hub"] => Some(Author::Hub),
            ["user"] => Some(Author::User),
            ["agent", track, id] => Some(Author::Agent { track: (*track).into(), id: (*id).into() }),
            ["admin", name] => Some(Author::Admin((*name).into())),
            _ => None,
        }
    }
}

/// The admin's decision on a pending device (4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision { Approve, Ask, Reject }

impl Decision {
    pub fn name(self) -> &'static str { match self { Decision::Approve => "approve", Decision::Ask => "ask", Decision::Reject => "reject" } }
    pub fn from_name(w: &str) -> Option<Decision> { [Decision::Approve, Decision::Ask, Decision::Reject].into_iter().find(|d| d.name() == w) }
}

/// Whose a one-time code is: a user's, made on one of their devices, or an admin's, for the admin pages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Owner { User { user: u64, device: [u8; 32] }, Admin(String) }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A device registered: a new user with its display name, or another device of a user by a code.
    Registered { at: u64, key: [u8; 32], name: String, kind: DeviceKind, platform: String, version: String, network: String, user: u64,
                 display_name: Option<String>, contact: Option<String>, code: Option<[u8; 32]> },
    Decided { at: u64, device: [u8; 32], admin: String, decision: Decision, text: String },
    /// A pending device's answer in its registration's conversation.
    Answered { at: u64, device: [u8; 32], text: String },
    Revoked { at: u64, device: [u8; 32], author: Author },
    /// A one-time code made, by its SHA-256.
    Code { at: u64, code: [u8; 32], owner: Owner, until: u64 },
    /// A browser's session opened by a code, by the SHA-256 of its token.
    Session { at: u64, token: [u8; 32], code: [u8; 32], until: u64 },
    SignedOut { at: u64, token: [u8; 32] },
    Filed { at: u64, id: u64, user: u64, device: [u8; 32], form: Request, held: bool },
    /// An attachment's bytes received: the request's own `n`th, or the `n`th of its message `post` (100003-APP-0148).
    Attached { at: u64, id: u64, post: Option<u32>, n: u32 },
    Posted { at: u64, id: u64, author: Author, message: Message },
    Moved { at: u64, id: u64, to: State, role: Role, author: Author, reason: String,
            track: Option<String>, task: Option<String>, duplicate_of: Option<u64>, release: Option<String> },
    Trust { at: u64, user: u64, to: i16, reason: String, author: Author },
    Setting { at: u64, name: String, value: String, admin: String },
    Version { at: u64, device: [u8; 32], version: String },
    /// A request deleted by its user before it was accepted (design 9): its texts and its thread's leave the journal.
    Deleted { at: u64, id: u64, author: Author },
    /// A user's account deleted: their names and contact, and their devices' answers, leave the journal.
    Erased { at: u64, user: u64, author: Author },
    /// The admin cleared a request's flag (6.2): the agents read it from then on.
    Cleared { at: u64, id: u64, admin: String, reason: String },
}

/// What a deleted text reads as, in the state and in the journal.
pub const DELETED: &str = "[deleted]";
pub const ERASED_NAME: &str = "deleted user";
pub const ERASED_DEVICE: &str = "deleted";
pub const ERASED_NETWORK: &str = "erased";

/// The attachments' entries with their names gone: a name is the user's words; the kind, size and SHA-256 stay.
pub fn redact_attachments(list: &[Attachment]) -> Vec<Attachment> {
    list.iter().enumerate().map(|(i, a)| Attachment { name: format!("deleted-{}", i + 1), ..a.clone() }).collect()
}

pub fn redact_request(form: &Request) -> Request {
    Request { title: DELETED.into(), text: DELETED.into(), attachments: redact_attachments(&form.attachments), ..form.clone() }
}

pub fn redact_message(m: &Message) -> Message {
    Message { text: DELETED.into(), attachments: redact_attachments(&m.attachments), ..m.clone() }
}

/// What the journal keeps of the deleted and the erased (100003-APP-0148).
pub struct Redaction<'a> {
    pub requests: &'a std::collections::HashSet<u64>,
    pub users: &'a std::collections::HashSet<u64>,
    pub devices: &'a std::collections::HashSet<[u8; 32]>,
    /// A registration's network stays a day, for the anti-flood count; an older one is erased.
    pub now: u64,
}

impl Event {
    /// The event as the journal keeps it after the deletions and erasures named.
    pub fn redacted(&self, r: &Redaction) -> Event {
        match self {
            Event::Filed { id, form, .. } if r.requests.contains(id) => {
                let Event::Filed { at, id, user, device, held, .. } = self.clone() else { unreachable!() };
                Event::Filed { at, id, user, device, held, form: redact_request(form) }
            }
            Event::Posted { at, id, author, message } if r.requests.contains(id) => Event::Posted { at: *at, id: *id, author: author.clone(), message: redact_message(message) },
            Event::Registered { at, key, kind, platform, version, network, user, display_name, code, .. } if r.users.contains(user) => Event::Registered {
                at: *at, key: *key, name: ERASED_DEVICE.into(), kind: *kind, platform: platform.clone(), version: version.clone(),
                network: if *at + crate::util::DAY_MS <= r.now { ERASED_NETWORK.into() } else { network.clone() },
                user: *user, display_name: display_name.as_ref().map(|_| ERASED_NAME.into()), contact: None, code: *code,
            },
            Event::Answered { at, device, .. } if r.devices.contains(device) => Event::Answered { at: *at, device: *device, text: DELETED.into() },
            e => e.clone(),
        }
    }
}

fn role_name(name: &str) -> Option<Role> {
    [Role::Hub, Role::User, Role::Intake, Role::Track, Role::Integrator, Role::Admin].into_iter().find(|r| r.name() == name)
}

impl Event {
    pub fn at(&self) -> u64 {
        match self {
            Event::Registered { at, .. } | Event::Decided { at, .. } | Event::Answered { at, .. } | Event::Revoked { at, .. }
            | Event::Code { at, .. } | Event::Session { at, .. } | Event::SignedOut { at, .. } | Event::Filed { at, .. }
            | Event::Attached { at, .. } | Event::Posted { at, .. } | Event::Moved { at, .. } | Event::Trust { at, .. }
            | Event::Setting { at, .. } | Event::Version { at, .. } | Event::Deleted { at, .. } | Event::Erased { at, .. } | Event::Cleared { at, .. } => *at,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Event::Registered { .. } => "registered", Event::Decided { .. } => "decided", Event::Answered { .. } => "answered",
            Event::Revoked { .. } => "revoked", Event::Code { .. } => "code", Event::Session { .. } => "session",
            Event::SignedOut { .. } => "signed-out", Event::Filed { .. } => "filed", Event::Attached { .. } => "attached",
            Event::Posted { .. } => "posted", Event::Moved { .. } => "moved", Event::Trust { .. } => "trust",
            Event::Setting { .. } => "setting", Event::Version { .. } => "version", Event::Deleted { .. } => "deleted",
            Event::Erased { .. } => "erased", Event::Cleared { .. } => "cleared",
        }
    }

    /// The event as one line of the journal, without its line end.
    pub fn encode(&self) -> String {
        let mut w = Writer::new();
        w.begin_object().field_str("event", self.name()).field_int("at_ms", self.at() as i64);
        let hex = |b: &[u8; 32]| improve_proto::hex32(b);
        match self {
            Event::Registered { key, name, kind, platform, version, network, user, display_name, contact, code, .. } => {
                w.field_str("key", &hex(key)).field_str("name", name).field_str("kind", kind.name()).field_str("platform", platform)
                    .field_str("version", version).field_str("network", network).field_int("user", *user as i64);
                if let Some(d) = display_name { w.field_str("display_name", d); }
                if let Some(c) = contact { w.field_str("contact", c); }
                if let Some(c) = code { w.field_str("code", &hex(c)); }
            }
            Event::Decided { device, admin, decision, text, .. } => {
                w.field_str("device", &hex(device)).field_str("admin", admin).field_str("decision", decision.name()).field_str("text", text);
            }
            Event::Answered { device, text, .. } => { w.field_str("device", &hex(device)).field_str("text", text); }
            Event::Revoked { device, author, .. } => { w.field_str("device", &hex(device)).field_str("author", &author.text()); }
            Event::Code { code, owner, until, .. } => {
                w.field_str("code", &hex(code));
                match owner {
                    Owner::User { user, device } => { w.field_int("user", *user as i64).field_str("device", &hex(device)); }
                    Owner::Admin(name) => { w.field_str("admin", name); }
                }
                w.field_int("until_ms", *until as i64);
            }
            Event::Session { token, code, until, .. } => {
                w.field_str("token", &hex(token)).field_str("code", &hex(code)).field_int("until_ms", *until as i64);
            }
            Event::SignedOut { token, .. } => { w.field_str("token", &hex(token)); }
            Event::Filed { id, user, device, form, held, .. } => {
                w.field_int("id", *id as i64).field_int("user", *user as i64).field_str("device", &hex(device))
                    .field_str("form", &wire::encode_request(form)).field_bool("held", *held);
            }
            Event::Attached { id, post, n, .. } => {
                w.field_int("id", *id as i64);
                if let Some(p) = post { w.field_int("post", *p as i64); }
                w.field_int("n", *n as i64);
            }
            Event::Posted { id, author, message, .. } => {
                w.field_int("id", *id as i64).field_str("author", &author.text()).field_str("message", &wire::encode_message(message));
            }
            Event::Moved { id, to, role, author, reason, track, task, duplicate_of, release, .. } => {
                w.field_int("id", *id as i64).field_str("to", to.name()).field_str("role", role.name()).field_str("author", &author.text())
                    .field_str("reason", reason);
                if let Some(t) = track { w.field_str("track", t); }
                if let Some(t) = task { w.field_str("task", t); }
                if let Some(d) = duplicate_of { w.field_int("duplicate_of", *d as i64); }
                if let Some(r) = release { w.field_str("release", r); }
            }
            Event::Trust { user, to, reason, author, .. } => {
                w.field_int("user", *user as i64).field_int("to", *to as i64).field_str("reason", reason).field_str("author", &author.text());
            }
            Event::Setting { name, value, admin, .. } => { w.field_str("name", name).field_str("value", value).field_str("admin", admin); }
            Event::Version { device, version, .. } => { w.field_str("device", &hex(device)).field_str("version", version); }
            Event::Deleted { id, author, .. } => { w.field_int("id", *id as i64).field_str("author", &author.text()); }
            Event::Erased { user, author, .. } => { w.field_int("user", *user as i64).field_str("author", &author.text()); }
            Event::Cleared { id, admin, reason, .. } => { w.field_int("id", *id as i64).field_str("admin", admin).field_str("reason", reason); }
        }
        w.end_object();
        w.finish()
    }

    /// One line of the journal read back; the error names what is wrong with it.
    pub fn decode(line: &str) -> Result<Event, String> {
        let value = json::parse(line).map_err(|e| format!("not JSON of the journal's form: {e:?}"))?;
        let name = value.get("event").and_then(Value::as_str).ok_or("no event")?;
        let (required, optional): (&[&str], &[&str]) = match name {
            "registered" => (&["key", "name", "kind", "platform", "version", "network", "user"], &["display_name", "contact", "code"]),
            "decided" => (&["device", "admin", "decision", "text"], &[]),
            "answered" => (&["device", "text"], &[]),
            "revoked" => (&["device", "author"], &[]),
            "code" => (&["code", "until_ms"], &["user", "device", "admin"]),
            "session" => (&["token", "code", "until_ms"], &[]),
            "signed-out" => (&["token"], &[]),
            "filed" => (&["id", "user", "device", "form", "held"], &[]),
            "attached" => (&["id", "n"], &["post"]),
            "posted" => (&["id", "author", "message"], &[]),
            "moved" => (&["id", "to", "role", "author", "reason"], &["track", "task", "duplicate_of", "release"]),
            "trust" => (&["user", "to", "reason", "author"], &[]),
            "setting" => (&["name", "value", "admin"], &[]),
            "version" => (&["device", "version"], &[]),
            "deleted" => (&["id", "author"], &[]),
            "erased" => (&["user", "author"], &[]),
            "cleared" => (&["id", "admin", "reason"], &[]),
            other => return Err(format!("an unknown event {other:?}")),
        };
        let f = Fields { value: &value };
        for k in value.keys().ok_or("not an object")? {
            if k != "event" && k != "at_ms" && !required.contains(&k) && !optional.contains(&k) { return Err(format!("an unknown key {k:?}")); }
        }
        for k in required { if value.get(k).is_none() { return Err(format!("no {k:?}")); } }
        let at = f.unsigned("at_ms")?;
        Ok(match name {
            "registered" => Event::Registered {
                at, key: f.hex("key")?, name: f.str("name")?, kind: DeviceKind::from_name(&f.str("kind")?).ok_or("kind")?,
                platform: f.str("platform")?, version: f.str("version")?, network: f.str("network")?, user: f.unsigned("user")?,
                display_name: f.opt_str("display_name")?, contact: f.opt_str("contact")?, code: f.opt("code").map(|_| f.hex("code")).transpose()?,
            },
            "decided" => Event::Decided { at, device: f.hex("device")?, admin: f.str("admin")?, decision: Decision::from_name(&f.str("decision")?).ok_or("decision")?, text: f.str("text")? },
            "answered" => Event::Answered { at, device: f.hex("device")?, text: f.str("text")? },
            "revoked" => Event::Revoked { at, device: f.hex("device")?, author: f.author("author")? },
            "code" => Event::Code {
                at, code: f.hex("code")?, until: f.unsigned("until_ms")?,
                owner: match (f.opt("user"), f.opt("device"), f.opt_str("admin")?) {
                    (Some(_), Some(_), None) => Owner::User { user: f.unsigned("user")?, device: f.hex("device")? },
                    (None, None, Some(admin)) => Owner::Admin(admin),
                    _ => return Err("a code's owner".into()),
                },
            },
            "session" => Event::Session { at, token: f.hex("token")?, code: f.hex("code")?, until: f.unsigned("until_ms")? },
            "signed-out" => Event::SignedOut { at, token: f.hex("token")? },
            "filed" => Event::Filed {
                at, id: f.unsigned("id")?, user: f.unsigned("user")?, device: f.hex("device")?,
                form: wire::read(&f.str("form")?, wire::decode_request).map_err(|e| format!("form: {e:?}"))?,
                held: value.get("held").and_then(Value::as_bool).ok_or("held")?,
            },
            "attached" => Event::Attached {
                at, id: f.unsigned("id")?, n: u32::try_from(f.unsigned("n")?).map_err(|_| "n")?,
                post: f.opt("post").map(|_| f.unsigned("post").and_then(|p| u32::try_from(p).map_err(|_| "post".to_string()))).transpose()?,
            },
            "posted" => Event::Posted {
                at, id: f.unsigned("id")?, author: f.author("author")?,
                message: wire::read(&f.str("message")?, wire::decode_message).map_err(|e| format!("message: {e:?}"))?,
            },
            "moved" => Event::Moved {
                at, id: f.unsigned("id")?, to: State::from_name(&f.str("to")?).ok_or("to")?, role: role_name(&f.str("role")?).ok_or("role")?,
                author: f.author("author")?, reason: f.str("reason")?, track: f.opt_str("track")?, task: f.opt_str("task")?,
                duplicate_of: f.opt("duplicate_of").map(|_| f.unsigned("duplicate_of")).transpose()?, release: f.opt_str("release")?,
            },
            "trust" => Event::Trust {
                at, user: f.unsigned("user")?, reason: f.str("reason")?, author: f.author("author")?,
                to: value.get("to").and_then(Value::as_int).and_then(|v| i16::try_from(v).ok()).ok_or("to")?,
            },
            "setting" => Event::Setting { at, name: f.str("name")?, value: f.str("value")?, admin: f.str("admin")? },
            "version" => Event::Version { at, device: f.hex("device")?, version: f.str("version")? },
            "deleted" => Event::Deleted { at, id: f.unsigned("id")?, author: f.author("author")? },
            "erased" => Event::Erased { at, user: f.unsigned("user")?, author: f.author("author")? },
            "cleared" => Event::Cleared { at, id: f.unsigned("id")?, admin: f.str("admin")?, reason: f.str("reason")? },
            _ => unreachable!(),
        })
    }
}

struct Fields<'a> { value: &'a Value }

impl Fields<'_> {
    fn opt(&self, k: &str) -> Option<&Value> { self.value.get(k) }
    fn str(&self, k: &str) -> Result<String, String> { self.value.get(k).and_then(Value::as_str).map(String::from).ok_or_else(|| format!("{k:?} is not a string")) }
    fn opt_str(&self, k: &str) -> Result<Option<String>, String> { self.value.get(k).map(|_| self.str(k)).transpose() }
    fn unsigned(&self, k: &str) -> Result<u64, String> {
        self.value.get(k).and_then(Value::as_int).and_then(|v| u64::try_from(v).ok()).ok_or_else(|| format!("{k:?} is not a count"))
    }
    fn hex(&self, k: &str) -> Result<[u8; 32], String> { improve_proto::parse_hex32(&self.str(k)?).ok_or_else(|| format!("{k:?} is not 32 bytes in hex")) }
    fn author(&self, k: &str) -> Result<Author, String> { Author::from_text(&self.str(k)?).ok_or_else(|| format!("{k:?} names no author")) }
}
