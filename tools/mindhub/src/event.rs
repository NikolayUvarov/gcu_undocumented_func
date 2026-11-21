//! The journal's events (docs/hub/README.md, 4 to 7): everything that changes the hub's state is one of them, written
//! before it is applied, so the state is the journal's replay (MC-10.3). One JSON object a line, its keys exactly these.

use improve_proto::form::{DeviceKind, Message, Request};
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
    Attached { at: u64, id: u64, n: u32 },
    Posted { at: u64, id: u64, author: Author, message: Message },
    Moved { at: u64, id: u64, to: State, role: Role, author: Author, reason: String,
            track: Option<String>, task: Option<String>, duplicate_of: Option<u64>, release: Option<String> },
    Trust { at: u64, user: u64, to: i16, reason: String, author: Author },
    Setting { at: u64, name: String, value: String, admin: String },
    Version { at: u64, device: [u8; 32], version: String },
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
            | Event::Setting { at, .. } | Event::Version { at, .. } => *at,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Event::Registered { .. } => "registered", Event::Decided { .. } => "decided", Event::Answered { .. } => "answered",
            Event::Revoked { .. } => "revoked", Event::Code { .. } => "code", Event::Session { .. } => "session",
            Event::SignedOut { .. } => "signed-out", Event::Filed { .. } => "filed", Event::Attached { .. } => "attached",
            Event::Posted { .. } => "posted", Event::Moved { .. } => "moved", Event::Trust { .. } => "trust",
            Event::Setting { .. } => "setting", Event::Version { .. } => "version",
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
            Event::Attached { id, n, .. } => { w.field_int("id", *id as i64).field_int("n", *n as i64); }
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
            "attached" => (&["id", "n"], &[]),
            "posted" => (&["id", "author", "message"], &[]),
            "moved" => (&["id", "to", "role", "author", "reason"], &["track", "task", "duplicate_of", "release"]),
            "trust" => (&["user", "to", "reason", "author"], &[]),
            "setting" => (&["name", "value", "admin"], &[]),
            "version" => (&["device", "version"], &[]),
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
            "attached" => Event::Attached { at, id: f.unsigned("id")?, n: u32::try_from(f.unsigned("n")?).map_err(|_| "n")? },
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
