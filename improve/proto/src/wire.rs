//! The forms in JSON, `hub-api 1` (docs/hub/README.md, 10.1). Each object carries exactly its keys: a missing key, an
//! unknown one or a value of another type is refused, naming it. Decoding builds the form; its bounds are `check`'s.

use crate::form::*;
use crate::json::{self, Value, Writer};
use crate::state::{Role, State};
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Json(json::Error),
    /// Not an object of the API's version.
    Api,
    Missing(&'static str),
    Unknown(String),
    Type(&'static str),
    Value(&'static str),
}

impl From<json::Error> for Error { fn from(e: json::Error) -> Error { Error::Json(e) } }

/// An object's fields, refusing any key not in `required` or `optional`.
struct Fields<'a> { value: &'a Value }

impl<'a> Fields<'a> {
    fn of(value: &'a Value, required: &[&'static str], optional: &[&'static str]) -> Result<Fields<'a>, Error> {
        let keys = value.keys().ok_or(Error::Type("object"))?;
        for k in keys {
            if !required.contains(&k) && !optional.contains(&k) { return Err(Error::Unknown(String::from(k))); }
        }
        for k in required { if value.get(k).is_none() { return Err(Error::Missing(k)); } }
        Ok(Fields { value })
    }
    fn str(&self, k: &'static str) -> Result<&'a str, Error> { self.value.get(k).ok_or(Error::Missing(k))?.as_str().ok_or(Error::Type(k)) }
    fn opt_str(&self, k: &'static str) -> Result<Option<&'a str>, Error> { self.value.get(k).map(|v| v.as_str().ok_or(Error::Type(k))).transpose() }
    fn bool(&self, k: &'static str) -> Result<bool, Error> { self.value.get(k).ok_or(Error::Missing(k))?.as_bool().ok_or(Error::Type(k)) }
    fn unsigned(&self, k: &'static str) -> Result<u64, Error> {
        let v = self.value.get(k).ok_or(Error::Missing(k))?.as_int().ok_or(Error::Type(k))?;
        u64::try_from(v).map_err(|_| Error::Value(k))
    }
    fn opt_unsigned(&self, k: &'static str) -> Result<Option<u64>, Error> {
        self.value.get(k).map(|_| self.unsigned(k)).transpose()
    }
    fn hex32(&self, k: &'static str) -> Result<[u8; 32], Error> { crate::parse_hex32(self.str(k)?).ok_or(Error::Value(k)) }
    fn word<T>(&self, k: &'static str, from: fn(&str) -> Option<T>) -> Result<T, Error> { from(self.str(k)?).ok_or(Error::Value(k)) }
}

fn api(value: &Value) -> Result<(), Error> {
    if value.get("api").and_then(Value::as_str) != Some(crate::API) { return Err(Error::Api); }
    Ok(())
}

fn begin(w: &mut Writer) { w.begin_object().field_str("api", crate::API); }

pub fn encode_registration(r: &Registration) -> String {
    let mut w = Writer::new();
    begin(&mut w);
    w.field_str("public_key", &crate::hex32(&r.public_key)).field_str("name", &r.name).field_str("kind", r.kind.name())
        .field_str("platform", &r.platform).field_str("version", &r.version);
    match &r.joining {
        Joining::NewUser { display_name } => { w.field_str("display_name", display_name); }
        Joining::Pair { code } => { w.field_str("pair_code", code); }
    }
    if let Some(c) = &r.contact { w.field_str("contact", c); }
    if let Some(n) = r.work { w.field_int("work", n as i64); }
    w.end_object();
    w.finish()
}

pub fn decode_registration(value: &Value) -> Result<Registration, Error> {
    api(value)?;
    let f = Fields::of(value, &["api", "public_key", "name", "kind", "platform", "version"], &["display_name", "pair_code", "contact", "work"])?;
    let joining = match (f.opt_str("display_name")?, f.opt_str("pair_code")?) {
        (Some(name), None) => Joining::NewUser { display_name: name.into() },
        (None, Some(code)) => Joining::Pair { code: code.into() },
        (None, None) => return Err(Error::Missing("display_name")),
        (Some(_), Some(_)) => return Err(Error::Value("pair_code")),
    };
    Ok(Registration {
        public_key: f.hex32("public_key")?,
        name: f.str("name")?.into(),
        kind: f.word("kind", DeviceKind::from_name)?,
        platform: f.str("platform")?.into(),
        version: f.str("version")?.into(),
        joining,
        contact: f.opt_str("contact")?.map(String::from),
        work: f.opt_unsigned("work")?,
    })
}

pub fn encode_request(r: &Request) -> String {
    let mut w = Writer::new();
    begin(&mut w);
    w.field_str("client_id", &r.client_id).field_str("kind", r.kind.name()).field_str("title", &r.title).field_str("text", &r.text)
        .field_str("visibility", r.visibility.name()).field_bool("security", r.security);
    w.key("attachments").begin_array();
    for a in &r.attachments {
        w.begin_object().field_str("kind", a.kind.name()).field_str("name", &a.name).field_int("size", a.size as i64)
            .field_str("sha256", &crate::hex32(&a.sha256)).end_object();
    }
    w.end_array().end_object();
    w.finish()
}

pub fn decode_request(value: &Value) -> Result<Request, Error> {
    api(value)?;
    let f = Fields::of(value, &["api", "client_id", "kind", "title", "text", "visibility", "security", "attachments"], &[])?;
    let list = value.get("attachments").and_then(Value::as_array).ok_or(Error::Type("attachments"))?;
    let mut attachments = Vec::with_capacity(list.len().min(8));
    for item in list {
        let a = Fields::of(item, &["kind", "name", "size", "sha256"], &[])?;
        attachments.push(Attachment { kind: a.word("kind", AttachmentKind::from_name)?, name: a.str("name")?.into(), size: a.unsigned("size")?, sha256: a.hex32("sha256")? });
    }
    Ok(Request {
        client_id: f.str("client_id")?.into(),
        kind: f.word("kind", Kind::from_name)?,
        title: f.str("title")?.into(),
        text: f.str("text")?.into(),
        visibility: f.word("visibility", Visibility::from_name)?,
        security: f.bool("security")?,
        attachments,
    })
}

pub fn encode_message(m: &Message) -> String {
    let mut w = Writer::new();
    begin(&mut w);
    w.field_str("kind", m.kind.name()).field_str("text", &m.text);
    if let Some(q) = m.answers { w.field_int("answers", q as i64); }
    if let Some(v) = m.verdict { w.field_str("verdict", v.name()); }
    if let Some(r) = &m.release { w.field_str("release", r); }
    w.end_object();
    w.finish()
}

pub fn decode_message(value: &Value) -> Result<Message, Error> {
    api(value)?;
    let f = Fields::of(value, &["api", "kind", "text"], &["answers", "verdict", "release"])?;
    let answers = f.opt_unsigned("answers")?.map(|n| u32::try_from(n).map_err(|_| Error::Value("answers"))).transpose()?;
    let verdict = f.opt_str("verdict")?.map(|w| Verdict::from_name(w).ok_or(Error::Value("verdict"))).transpose()?;
    Ok(Message { kind: f.word("kind", MessageKind::from_name)?, text: f.str("text")?.into(), answers, verdict, release: f.opt_str("release")?.map(String::from) })
}

fn role_from_name(name: &str) -> Option<Role> {
    [Role::Hub, Role::User, Role::Intake, Role::Track, Role::Integrator, Role::Admin].into_iter().find(|r| r.name() == name)
}

pub fn encode_change(c: &Change) -> String {
    let mut w = Writer::new();
    w.begin_object().field_str("from", c.from.name()).field_str("to", c.to.name()).field_str("role", c.role.name())
        .field_int("at_ms", c.at_ms as i64).field_str("reason", &c.reason).end_object();
    w.finish()
}

pub fn decode_change(value: &Value) -> Result<Change, Error> {
    let f = Fields::of(value, &["from", "to", "role", "at_ms", "reason"], &[])?;
    Ok(Change {
        from: f.word("from", State::from_name)?,
        to: f.word("to", State::from_name)?,
        role: f.word("role", role_from_name)?,
        at_ms: f.unsigned("at_ms")?,
        reason: f.str("reason")?.into(),
    })
}

pub fn encode_board(entries: &[BoardEntry]) -> String {
    let mut w = Writer::new();
    begin(&mut w);
    w.key("board").begin_array();
    for e in entries {
        w.begin_object().field_int("id", e.id as i64).field_str("title", &e.title).field_str("kind", e.kind.name())
            .field_str("track", &e.track).field_str("task", &e.task).field_str("state", e.state.name()).field_int("since_ms", e.since_ms as i64);
        if let Some(r) = &e.release { w.field_str("release", r); }
        if let Some(a) = &e.author { w.field_str("author", a); }
        w.end_object();
    }
    w.end_array().end_object();
    w.finish()
}

pub fn decode_board(value: &Value) -> Result<Vec<BoardEntry>, Error> {
    api(value)?;
    Fields::of(value, &["api", "board"], &[])?;
    let list = value.get("board").and_then(Value::as_array).ok_or(Error::Type("board"))?;
    let mut out = Vec::with_capacity(list.len().min(1024));
    for item in list {
        let f = Fields::of(item, &["id", "title", "kind", "track", "task", "state", "since_ms"], &["release", "author"])?;
        out.push(BoardEntry {
            id: f.unsigned("id")?,
            title: f.str("title")?.into(),
            kind: f.word("kind", Kind::from_name)?,
            track: f.str("track")?.into(),
            task: f.str("task")?.into(),
            state: f.word("state", State::from_name)?,
            since_ms: f.unsigned("since_ms")?,
            release: f.opt_str("release")?.map(String::from),
            author: f.opt_str("author")?.map(String::from),
        });
    }
    Ok(out)
}

/// Parses `text` and decodes it with `decode`: the host's path; MIND Core gets the tree from `parse`.
pub fn read<T>(text: &str, decode: fn(&Value) -> Result<T, Error>) -> Result<T, Error> { decode(&json::parse(text)?) }
