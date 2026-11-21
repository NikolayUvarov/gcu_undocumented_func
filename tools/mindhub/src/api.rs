//! `hub-api 2` over HTTP (docs/hub/README.md, 10): the devices' routes, the agents', the admin's and the board, each
//! signed over a challenge the hub gave, and every answer carrying the next challenge. The pages are `pages`'.

use crate::event::{Decision, Event, Owner};
use crate::http::{Request, Response};
use crate::hub::{self, refuse, Actor, DeviceState, Extra, Hub, Item, Refusal};
use crate::journal::{self, Journal};
use crate::util::{self, hex, sha256};
use improve_proto::form::{Message, MessageKind};
use improve_proto::json::{self, Value, Writer};
use improve_proto::state::State;
use improve_proto::trust::Access;
use improve_proto::{sign, wire, API};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

pub const CHALLENGE_HEADER: &str = "Hub-Challenge";
/// Challenges given and not used yet, at most; the oldest go first.
pub const CHALLENGES_MAX: usize = 100_000;
/// An attachment's bytes, at most, whatever the band (the largest band's).
pub const ATTACHMENT_MAX: usize = 8 * 1024 * 1024;

/// Challenges given, each good for one request and 10 minutes (4.1). One network holds at most PER_NETWORK of them,
/// so that a flood from it pushes out its own and no one else's.
#[derive(Default)]
pub struct Challenges { live: HashMap<[u8; 32], u64>, order: VecDeque<[u8; 32]>, per: HashMap<String, VecDeque<[u8; 32]>> }

pub const PER_NETWORK: usize = 32;

impl Challenges {
    pub fn issue(&mut self, now: u64, network: &str) -> [u8; 32] {
        let c = util::random::<32>();
        let live = &mut self.live;
        let mine = self.per.entry(network.into()).or_default();
        mine.retain(|old| live.contains_key(old));
        while mine.len() >= PER_NETWORK {
            if let Some(old) = mine.pop_front() { live.remove(&old); }
        }
        mine.push_back(c);
        live.insert(c, now);
        self.order.push_back(c);
        while self.live.len() > CHALLENGES_MAX || self.order.len() > 2 * CHALLENGES_MAX {
            match self.order.pop_front() { Some(old) => { self.live.remove(&old); } None => break }
        }
        if self.per.len() > CHALLENGES_MAX {
            let live = &self.live;
            self.per.retain(|_, q| { q.retain(|c| live.contains_key(c)); !q.is_empty() });
        }
        c
    }
    /// Whether the challenge was given within its life and not used; it is used by this.
    pub fn take(&mut self, c: &[u8; 32], now: u64) -> bool {
        match self.live.remove(c) { Some(at) => at + sign::CHALLENGE_LIFE_MS > now, None => false }
    }
}

/// Everything a request reaches, under one lock.
pub struct Shared {
    pub hub: Hub,
    pub journal: Journal,
    pub challenges: Challenges,
    pub attachments: PathBuf,
}

/// Where a request came from and when.
pub struct Context { pub now: u64, pub network: String }

/// Who signed a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signer { Device([u8; 32]), Agent([u8; 32]), Admin(String), Unregistered([u8; 32]) }

impl Shared {
    pub fn commit(&mut self, events: Vec<Event>) -> Result<(), Refusal> {
        let erasing = events.iter().any(|e| matches!(e, Event::Deleted { .. } | Event::Erased { .. }));
        journal::commit(&mut self.hub, &mut self.journal, events).map_err(|e| refuse(500, "hub", format!("the hub refused its own change: {e}")))?;
        if erasing { self.erase().map_err(|e| refuse(500, "journal", format!("the deleted texts cannot leave the journal: {e}")))?; }
        Ok(())
    }

    /// The deleted texts out of the journal and the attachments no one keeps out of the store (`journal::erase`).
    pub fn erase(&mut self) -> std::io::Result<()> { journal::erase(&self.hub, &mut self.journal, &self.attachments, util::now_ms()) }

    /// The signer of a request, by its `Hub-Signature` over a challenge given and not used (4.1, 10.3).
    pub fn signer(&mut self, req: &Request, now: u64) -> Result<(Signer, [u8; 32]), Refusal> {
        let header = req.header(sign::HEADER).ok_or_else(|| refuse(401, "unsigned", "sign the request with the challenge this answer carries"))?;
        let signed = sign::parse_header(header).map_err(|_| refuse(401, "signature", "Hub-Signature is not ed25519 KEY CHALLENGE SIGNATURE in hex"))?;
        if !self.challenges.take(&signed.challenge, now) {
            return Err(refuse(401, "challenge", "the challenge is not one the hub gave, or it was used or is over 10 minutes old: sign with the one this answer carries"));
        }
        sign::verify(&signed, &req.method, &req.target, &req.body).map_err(|_| refuse(401, "signature", "the signature does not verify for this method, path and body"))?;
        let key = signed.public_key;
        let who = if let Some(name) = self.hub.keys.admins.get(&key) { Signer::Admin(name.clone()) }
            else if self.hub.keys.agents.contains_key(&key) { Signer::Agent(key) }
            else if self.hub.device_by_key(&key).is_some() { Signer::Device(sign::id(&key)) }
            else { Signer::Unregistered(key) };
        Ok((who, signed.challenge))
    }
}

/// The bound of a route's body.
pub fn body_max(method: &str, path: &str) -> usize {
    if method == "PUT" && path.starts_with("/hub/v1/requests/") { ATTACHMENT_MAX } else { json::BODY_MAX }
}

pub fn refusal(r: &Refusal) -> Response {
    let mut w = Writer::new();
    w.begin_object().field_str("api", API).field_str("error", &r.limit).field_str("detail", &r.detail);
    let secs = r.retry_after_ms.map(|ms| ms.div_ceil(1000));
    if let Some(s) = secs { w.field_int("retry_after_s", s as i64); }
    if let Some(e) = r.earlier { w.field_int("earlier", e as i64); }
    if let Some(b) = r.work_bits { w.field_int("work_bits", b as i64); }
    w.end_object();
    let resp = Response::json(r.status, w.finish());
    match secs { Some(s) => resp.with("Retry-After", s.to_string()), None => resp }
}

fn ok(status: u16, w: Writer) -> Result<Response, Refusal> { Ok(Response::json(status, w.finish())) }

fn begin() -> Writer { let mut w = Writer::new(); w.begin_object().field_str("api", API); w }

/// A JSON body of the API's version, and its object's keys exactly these.
struct Body { value: Value }

impl Body {
    fn read(req: &Request, required: &[&str], optional: &[&str]) -> Result<Body, Refusal> {
        let text = std::str::from_utf8(&req.body).map_err(|_| refuse(400, "body", "the body is not UTF-8"))?;
        let value = json::parse(text).map_err(|e| refuse(400, "body", format!("the body is not JSON of the API's form: {e:?}")))?;
        if value.get("api").and_then(Value::as_str) != Some(API) { return Err(refuse(400, "api", format!("the body is not of {API}"))); }
        for k in value.keys().ok_or_else(|| refuse(400, "body", "the body is not an object"))? {
            if k != "api" && !required.contains(&k) && !optional.contains(&k) { return Err(refuse(400, "body", format!("an unknown key {k:?}"))); }
        }
        for k in required { if value.get(k).is_none() { return Err(refuse(400, "body", format!("no {k:?}"))); } }
        Ok(Body { value })
    }
    fn str(&self, k: &str) -> Result<&str, Refusal> { self.value.get(k).and_then(Value::as_str).ok_or_else(|| refuse(400, "body", format!("{k:?} is not a string"))) }
    fn opt_str(&self, k: &str) -> Result<Option<&str>, Refusal> { self.value.get(k).map(|_| self.str(k)).transpose() }
    fn int(&self, k: &str) -> Result<i64, Refusal> { self.value.get(k).and_then(Value::as_int).ok_or_else(|| refuse(400, "body", format!("{k:?} is not an integer"))) }
    fn opt_bool(&self, k: &str) -> Result<bool, Refusal> {
        self.value.get(k).map(|v| v.as_bool().ok_or_else(|| refuse(400, "body", format!("{k:?} is not true or false")))).transpose().map(|b| b.unwrap_or(false))
    }
}

fn wire_refusal(e: wire::Error) -> Refusal { refuse(400, "body", format!("the body is not the API's form: {e:?}")) }

fn number(text: &str) -> Option<u64> { (!text.is_empty() && text.len() <= 19 && text.bytes().all(|b| b.is_ascii_digit())).then(|| text.parse().ok()).flatten() }

fn query(req: &Request, key: &str) -> Option<String> { util::fields(&req.query).and_then(|f| util::field(&f, key).map(String::from)) }

// ---- views ----

pub fn write_summary(w: &mut Writer, r: &Item) {
    w.field_int("id", r.id as i64).field_str("title", &r.form.title).field_str("kind", r.form.kind.name()).field_str("state", r.state.name())
        .field_int("filed_ms", r.filed as i64).field_int("since_ms", r.since as i64).field_str("visibility", r.form.visibility.name())
        .field_bool("security", r.form.security).field_int("posts", r.thread.len() as i64)
        .field_bool("open_question", r.thread.iter().any(|p| p.message.kind == MessageKind::Question && !p.answered));
    if let Some(t) = &r.track { w.field_str("track", t); }
    if let Some(t) = &r.task { w.field_str("task", t); }
    if let Some(t) = &r.release { w.field_str("release", t); }
    if let Some(d) = r.duplicate_of { w.field_int("duplicate_of", d as i64); }
    w.field_bool("named", r.form.named);
    if !r.flags.is_empty() {
        w.key("flagged").begin_array();
        for f in &r.flags { w.string(f); }
        w.end_array().field_bool("cleared", r.cleared);
    }
    if r.deleted { w.field_bool("deleted", true); }
}

fn write_attachments(w: &mut Writer, list: &[improve_proto::form::Attachment], received: &[bool]) {
    w.key("attachments").begin_array();
    for (a, got) in list.iter().zip(received) {
        w.begin_object().field_str("kind", a.kind.name()).field_str("name", &a.name).field_int("size", a.size as i64)
            .field_str("sha256", &hex(&a.sha256)).field_bool("received", *got).end_object();
    }
    w.end_array();
}

pub fn write_request(w: &mut Writer, r: &Item) {
    write_summary(w, r);
    w.field_str("text", &r.form.text);
    write_attachments(w, &r.form.attachments, &r.received);
    w.key("thread").begin_array();
    for p in &r.thread {
        w.begin_object().field_int("n", p.n as i64).field_int("at_ms", p.at as i64).field_str("author", &p.author.text())
            .field_str("kind", p.message.kind.name()).field_str("text", &p.message.text);
        if let Some(a) = p.message.answers { w.field_int("answers", a as i64); }
        if let Some(v) = p.message.verdict { w.field_str("verdict", v.name()); }
        if let Some(r) = &p.message.release { w.field_str("release", r); }
        if p.message.kind == MessageKind::Question { w.field_bool("answered", p.answered); }
        if !p.message.attachments.is_empty() { write_attachments(w, &p.message.attachments, &p.received); }
        w.end_object();
    }
    w.end_array();
    w.key("changes").begin_array();
    for c in &r.changes {
        w.begin_object().field_int("at_ms", c.at as i64).field_str("from", c.from.name()).field_str("to", c.to.name()).field_str("role", c.role.name())
            .field_str("author", &c.author.text()).field_str("reason", &c.reason).end_object();
    }
    w.end_array();
}

fn write_device(w: &mut Writer, hub: &Hub, d: &hub::Device, now: u64, admin: bool) {
    w.field_str("device", &hex(&d.id)).field_str("state", d.state.name()).field_str("name", &d.name).field_str("kind", d.kind.name())
        .field_str("platform", &d.platform).field_str("version", &d.version).field_int("registered_ms", d.at as i64).field_bool("paired", d.paired);
    if admin {
        w.field_str("network", &d.network);
        if let Some(c) = &d.contact { w.field_str("contact", c); }
    }
    let u = hub.user(d.user).expect("a device's user");
    w.key("user").begin_object().field_int("id", u.id as i64).field_str("display_name", &u.display_name).field_int("trust", u.trust.value() as i64);
    match u.trust.band() { Some(b) => { w.field_int("band", b as i64); } None => { w.key("band").null(); } }
    w.end_object();
    let access = match improve_proto::trust::access(u.trust, d.state == DeviceState::Approved) {
        Access::Banned => "banned", Access::ReadOnly => "read-only", Access::Pending(_) => "pending", Access::Open(_) => "open",
    };
    w.field_str("access", access);
    w.key("notes").begin_array();
    for n in &d.notes {
        w.begin_object().field_int("at_ms", n.at as i64).field_str("from", &n.from.text()).field_str("kind", n.kind).field_str("text", &n.text).end_object();
    }
    w.end_array();
    w.key("trust_history").begin_array();
    for c in &u.history {
        w.begin_object().field_int("at_ms", c.at as i64).field_int("from", c.from as i64).field_int("to", c.to as i64)
            .field_str("reason", &c.reason).field_str("author", &c.author.text()).end_object();
    }
    w.end_array();
    let (requests, messages) = hub.quotas(&d.id, now);
    w.key("quotas").begin_object().field_int("requests", requests as i64).field_int("messages", messages as i64).end_object();
}

// ---- routing ----

/// One request's answer; the caller adds the next challenge.
pub fn handle(s: &mut Shared, req: &Request, cx: &Context) -> Response {
    let result = if let Some(rest) = req.path.strip_prefix("/hub/v1/") { device_route(s, req, cx, rest) }
        else if let Some(rest) = req.path.strip_prefix("/hub/agent/v1/") { agent_route(s, req, cx, rest) }
        else if let Some(rest) = req.path.strip_prefix("/hub/admin/v1/") { admin_route(s, req, cx, rest) }
        else { return crate::pages::handle(s, req, cx) };
    result.unwrap_or_else(|r| refusal(&r))
}

fn segments(rest: &str) -> Vec<&str> { rest.split('/').collect() }

fn device_route(s: &mut Shared, req: &Request, cx: &Context, rest: &str) -> Result<Response, Refusal> {
    let now = cx.now;
    let seg = segments(rest);
    let m = req.method.as_str();
    match (m, seg.as_slice()) {
        ("GET", ["challenge"]) => {
            // The proof of work a registration needs now, so that a device solves it before it asks (5.4); and a
            // challenge in the body too, for a client that reads no header of the hub's own (MIND Core's `mind::http`).
            let challenge = s.challenges.issue(now, &cx.network);
            let mut w = begin();
            w.field_int("work_bits", s.hub.work_bits(now) as i64).field_str("challenge", &hex(&challenge)).end_object();
            return ok(200, w);
        }
        ("GET", ["board"]) => return Ok(Response::json(200, wire::encode_board(&s.hub.board()))),
        _ => {}
    }
    let (signer, challenge) = s.signer(req, now)?;
    if let ("POST", ["devices"]) = (m, seg.as_slice()) {
        let key = match &signer {
            Signer::Unregistered(k) => *k,
            Signer::Device(id) => s.hub.device(id).unwrap().key,
            _ => return Err(refuse(400, "public_key", "an admin's or an agent's key is not a device's")),
        };
        let text = std::str::from_utf8(&req.body).map_err(|_| refuse(400, "body", "the body is not UTF-8"))?;
        let reg = wire::read(text, wire::decode_registration).map_err(wire_refusal)?;
        let (events, id) = s.hub.register(now, &reg, &key, &challenge, &cx.network)?;
        let created = !events.is_empty();
        s.commit(events)?;
        let d = s.hub.device(&id).unwrap();
        let mut w = begin();
        w.field_str("device", &hex(&id)).field_str("state", d.state.name()).field_int("user", d.user as i64).field_int("poll_s", s.hub.settings.poll_s as i64).end_object();
        return ok(if created { 201 } else { 200 }, w);
    }
    let device = match signer {
        Signer::Device(id) => id,
        Signer::Unregistered(_) => return Err(refuse(403, "unregistered", "this key is not registered: POST /hub/v1/devices first")),
        _ => return Err(refuse(403, "key", "the devices' routes take a device's key")),
    };
    // A banned user's, a rejected or revoked key's every call is refused, naming why.
    let user = s.hub.access(&device)?.1.user;
    match (m, seg.as_slice()) {
        ("GET", ["devices"]) => {
            // The user's devices, for `improve devices` and `improve revoke` (4.4).
            let mut w = begin();
            w.key("devices").begin_array();
            for id in &s.hub.user(user).expect("a device's user").devices {
                let d = s.hub.device(id).expect("a user's device");
                w.begin_object().field_str("device", &hex(&d.id)).field_str("name", &d.name).field_str("kind", d.kind.name()).field_str("platform", &d.platform)
                    .field_str("version", &d.version).field_str("state", d.state.name()).field_int("registered_ms", d.at as i64).field_bool("paired", d.paired)
                    .field_bool("this", d.id == device).end_object();
            }
            w.end_array().end_object();
            ok(200, w)
        }
        ("GET", ["devices", "me"]) => {
            let mut w = begin();
            write_device(&mut w, &s.hub, s.hub.device(&device).unwrap(), now, false);
            w.field_int("poll_s", s.hub.settings.poll_s as i64).end_object();
            ok(200, w)
        }
        ("POST", ["devices", "me", "messages"]) => {
            let b = Body::read(req, &["text"], &[])?;
            let events = s.hub.answer_registration(now, &device, &util::lines(b.str("text")?))?;
            s.commit(events)?;
            let mut w = begin(); w.end_object(); ok(201, w)
        }
        ("POST", ["devices", "me", "version"]) => {
            let b = Body::read(req, &["version"], &[])?;
            let events = s.hub.report_version(now, &device, b.str("version")?)?;
            s.commit(events)?;
            let mut w = begin(); w.end_object(); ok(200, w)
        }
        ("POST", ["devices", "pair"]) => {
            let code = util::code();
            let events = s.hub.make_code(now, Owner::User { user, device }, &code)?;
            s.commit(events)?;
            let mut w = begin();
            w.field_str("code", &code).field_int("until_ms", (now + hub::CODE_MS) as i64).end_object();
            ok(201, w)
        }
        ("DELETE", ["devices", id]) => {
            let target = improve_proto::parse_hex32(id).ok_or_else(|| refuse(404, "device", "no such device"))?;
            let events = s.hub.revoke(now, &Actor::Device(device), &target)?;
            s.commit(events)?;
            let mut w = begin(); w.end_object(); ok(200, w)
        }
        ("POST", ["requests"]) => {
            let text = std::str::from_utf8(&req.body).map_err(|_| refuse(400, "body", "the body is not UTF-8"))?;
            let form = wire::read(text, wire::decode_request).map_err(wire_refusal)?;
            let (events, id, again) = s.hub.file(now, &device, &form)?;
            s.commit(events)?;
            let r = s.hub.request(id).unwrap();
            let mut w = begin();
            w.field_int("id", id as i64).field_str("state", r.state.name()).field_bool("filed_before", again).end_object();
            ok(if again { 200 } else { 201 }, w)
        }
        ("PUT", ["requests", id, "attachments", n]) => put_attachment(s, now, &device, id, None, n, &req.body),
        ("PUT", ["requests", id, "messages", p, "attachments", n]) => {
            let p = number(p).and_then(|p| u32::try_from(p).ok()).ok_or_else(|| refuse(404, "message", "no such message"))?;
            put_attachment(s, now, &device, id, Some(p), n, &req.body)
        }
        ("GET", ["requests", id, "attachments", n]) => { let r = own(s, user, id)?; attachment(s, &r, None, n) }
        ("GET", ["requests", id, "messages", p, "attachments", n]) => { let r = own(s, user, id)?; attachment(s, &r, Some(p), n) }
        ("DELETE", ["requests", id]) => {
            let id = number(id).ok_or_else(|| refuse(404, "request", "no such request"))?;
            let events = s.hub.delete_request(now, &device, id)?;
            s.commit(events)?;
            let mut w = begin(); w.field_bool("deleted", true).end_object(); ok(200, w)
        }
        ("GET", ["me", "export"]) => Ok(Response::json(200, export(&s.hub, user, now))),
        ("DELETE", ["me"]) => {
            let events = s.hub.erase(now, &device)?;
            s.commit(events)?;
            let mut w = begin(); w.field_bool("erased", true).end_object(); ok(200, w)
        }
        ("GET", ["requests"]) => {
            let since = query(req, "since").map(|v| number(&v).ok_or_else(|| refuse(400, "since", "since is a time in ms"))).transpose()?.unwrap_or(0);
            let mut w = begin();
            w.key("requests").begin_array();
            for r in s.hub.requests().iter().filter(|r| r.user == user && r.since >= since && !r.deleted) { w.begin_object(); write_summary(&mut w, r); w.end_object(); }
            w.end_array().end_object();
            ok(200, w)
        }
        ("GET", ["requests", id]) => {
            let r = own(s, user, id)?;
            let mut w = begin();
            w.key("request").begin_object();
            write_request(&mut w, &r);
            w.end_object().end_object();
            ok(200, w)
        }
        ("POST", ["requests", id, "messages"]) => {
            let id = number(id).ok_or_else(|| refuse(404, "request", "no such request"))?;
            let text = std::str::from_utf8(&req.body).map_err(|_| refuse(400, "body", "the body is not UTF-8"))?;
            let message = wire::read(text, wire::decode_message).map_err(wire_refusal)?;
            post(s, now, Actor::Device(device), id, message)
        }
        _ => Err(refuse(404, "route", "no such route of hub-api 2")),
    }
}

fn post(s: &mut Shared, now: u64, by: Actor, id: u64, message: Message) -> Result<Response, Refusal> {
    let events = s.hub.post(now, &by, id, &message)?;
    s.commit(events)?;
    let r = s.hub.request(id).unwrap();
    let n = r.thread.iter().rev().find(|p| p.message.kind != MessageKind::Status).map(|p| p.n).unwrap_or(0);
    let mut w = begin();
    w.field_int("n", n as i64).field_str("state", r.state.name()).end_object();
    ok(201, w)
}

/// An attachment's bytes under their SHA-256, written whole before the event that names them.
pub fn store(s: &Shared, bytes: &[u8]) -> Result<(), Refusal> {
    let fail = |e: std::io::Error| refuse(500, "store", format!("the attachment cannot be stored: {e}"));
    std::fs::create_dir_all(&s.attachments).map_err(fail)?;
    let path = s.attachments.join(hex(&sha256(bytes)));
    if path.exists() { return Ok(()); }
    let tmp = s.attachments.join(format!(".part-{}", hex(&util::random::<8>())));
    let mut f = std::fs::File::create(&tmp).map_err(fail)?;
    std::io::Write::write_all(&mut f, bytes).and_then(|_| f.sync_all()).map_err(fail)?;
    std::fs::rename(&tmp, &path).map_err(fail)
}

/// An attachment's bytes: the request's own `n`th, or the `n`th of its message `post`.
pub fn attachment(s: &Shared, r: &Item, post: Option<&str>, n: &str) -> Result<Response, Refusal> {
    let none = || refuse(404, "attachment", "no such attachment received");
    let n = number(n).ok_or_else(none)? as usize;
    let (list, received, name) = match post {
        None => (&r.form.attachments, &r.received, format!("request-{}-{}.bin", r.id, n)),
        Some(p) => {
            let p = number(p).and_then(|p| r.thread.iter().find(|x| x.n as u64 == p)).ok_or_else(none)?;
            (&p.message.attachments, &p.received, format!("request-{}-message-{}-{}.bin", r.id, p.n, n))
        }
    };
    let a = list.get(n).filter(|_| received[n]).ok_or_else(none)?;
    let bytes = std::fs::read(s.attachments.join(hex(&a.sha256))).map_err(|e| refuse(500, "store", format!("the attachment cannot be read: {e}")))?;
    // Data, never opened by the browser: a download under a name of its own.
    Ok(Response { status: 200, headers: vec![("Content-Type".into(), "application/octet-stream".into()),
                                             ("Content-Disposition".into(), format!("attachment; filename=\"{name}\""))], body: bytes })
}

fn own(s: &Shared, user: u64, id: &str) -> Result<Item, Refusal> {
    number(id).and_then(|id| s.hub.request(id)).filter(|r| r.user == user && !r.deleted).cloned().ok_or_else(|| refuse(404, "request", "no such request of this user"))
}

fn put_attachment(s: &mut Shared, now: u64, device: &[u8; 32], id: &str, post: Option<u32>, n: &str, body: &[u8]) -> Result<Response, Refusal> {
    let id = number(id).ok_or_else(|| refuse(404, "request", "no such request"))?;
    let n = number(n).and_then(|n| u32::try_from(n).ok()).ok_or_else(|| refuse(404, "attachment", "no such attachment"))?;
    let events = s.hub.attach(now, device, id, post, n, body)?;
    if !events.is_empty() { store(s, body)?; }
    s.commit(events)?;
    let mut w = begin(); w.end_object(); ok(200, w)
}

/// Everything the hub keeps of a user, as JSON: their name, trust and its history, their devices with their contact
/// and registration's conversation, and their requests with their threads (design 9). Attachments by their entries;
/// their bytes come from their routes.
pub fn export(hub: &Hub, user: u64, now: u64) -> String {
    let u = hub.user(user).expect("a user");
    let mut w = begin();
    w.field_int("exported_ms", now as i64).key("user").begin_object().field_int("id", u.id as i64).field_str("display_name", &u.display_name)
        .field_int("trust", u.trust.value() as i64).key("trust_history").begin_array();
    for c in &u.history {
        w.begin_object().field_int("at_ms", c.at as i64).field_int("from", c.from as i64).field_int("to", c.to as i64)
            .field_str("reason", &c.reason).field_str("author", &c.author.text()).end_object();
    }
    w.end_array().end_object().key("devices").begin_array();
    for id in &u.devices {
        let d = hub.device(id).expect("a user's device");
        w.begin_object().field_str("device", &hex(&d.id)).field_str("public_key", &hex(&d.key)).field_str("state", d.state.name()).field_str("name", &d.name)
            .field_str("kind", d.kind.name()).field_str("platform", &d.platform).field_str("version", &d.version).field_int("registered_ms", d.at as i64);
        if let Some(c) = &d.contact { w.field_str("contact", c); }
        w.key("notes").begin_array();
        for n in &d.notes { w.begin_object().field_int("at_ms", n.at as i64).field_str("from", &n.from.text()).field_str("kind", n.kind).field_str("text", &n.text).end_object(); }
        w.end_array().end_object();
    }
    w.end_array().key("requests").begin_array();
    for r in hub.requests().iter().filter(|r| r.user == user && !r.deleted) { w.begin_object(); write_request(&mut w, r); w.end_object(); }
    w.end_array().end_object();
    w.finish()
}

fn state_body(req: &Request) -> Result<(State, String, Extra), Refusal> {
    let b = Body::read(req, &["to", "reason"], &["track", "task", "duplicate_of", "not_a_request", "spam"])?;
    let to = State::from_name(b.str("to")?).ok_or_else(|| refuse(400, "to", "to names no state"))?;
    let duplicate_of = b.value.get("duplicate_of").map(|_| b.int("duplicate_of").ok().and_then(|v| u64::try_from(v).ok()).ok_or_else(|| refuse(400, "duplicate_of", "a request's number"))).transpose()?;
    let extra = Extra { track: b.opt_str("track")?.map(String::from), task: b.opt_str("task")?.map(String::from), duplicate_of,
                        not_a_request: b.opt_bool("not_a_request")?, spam: b.opt_bool("spam")? };
    Ok((to, b.str("reason")?.to_string(), extra))
}

fn agent_route(s: &mut Shared, req: &Request, cx: &Context, rest: &str) -> Result<Response, Refusal> {
    let now = cx.now;
    let (signer, _) = s.signer(req, now)?;
    let Signer::Agent(key) = signer else { return Err(refuse(403, "key", "the agents' routes take an agent's key")) };
    let agent = s.hub.keys.agents[&key].clone();
    let seg = segments(rest);
    let readable = |s: &Shared, id: &str| number(id).and_then(|id| s.hub.request(id)).filter(|r| s.hub.agent_reads(&agent, r)).cloned()
        .ok_or_else(|| refuse(404, "request", "no such request for this agent"));
    match (req.method.as_str(), seg.as_slice()) {
        ("GET", ["queue"]) => {
            let mut w = begin();
            w.field_str("track", &agent.track).field_str("role", agent.role.name()).key("requests").begin_array();
            for r in s.hub.queue(&agent) { w.begin_object(); write_summary(&mut w, r); w.end_object(); }
            w.end_array().end_object();
            ok(200, w)
        }
        ("GET", ["requests", id]) => {
            let r = readable(s, id)?;
            let mut w = begin();
            w.key("request").begin_object();
            write_request(&mut w, &r);
            w.end_object().end_object();
            ok(200, w)
        }
        ("GET", ["requests", id, "attachments", n]) => { let r = readable(s, id)?; attachment(s, &r, None, n) }
        ("GET", ["requests", id, "messages", p, "attachments", n]) => { let r = readable(s, id)?; attachment(s, &r, Some(p), n) }
        ("POST", ["requests", id, "messages"]) => {
            let id = number(id).ok_or_else(|| refuse(404, "request", "no such request"))?;
            let text = std::str::from_utf8(&req.body).map_err(|_| refuse(400, "body", "the body is not UTF-8"))?;
            post(s, now, Actor::Agent(key), id, wire::read(text, wire::decode_message).map_err(wire_refusal)?)
        }
        ("POST", ["requests", id, "state"]) => {
            let id = number(id).ok_or_else(|| refuse(404, "request", "no such request"))?;
            let (to, reason, extra) = state_body(req)?;
            let events = s.hub.move_request(now, &Actor::Agent(key), id, to, &reason, &extra)?;
            s.commit(events)?;
            let mut w = begin(); w.field_str("state", s.hub.request(id).unwrap().state.name()).end_object(); ok(200, w)
        }
        _ => Err(refuse(404, "route", "no such route of the agents' API")),
    }
}

fn admin_route(s: &mut Shared, req: &Request, cx: &Context, rest: &str) -> Result<Response, Refusal> {
    let now = cx.now;
    let (signer, _) = s.signer(req, now)?;
    let Signer::Admin(admin) = signer else { return Err(refuse(403, "key", "the admin's routes take an admin's key")) };
    admin_call(s, req, now, &admin, rest)
}

/// The admin's routes once the admin is known: by a signature, or by the admin pages' session.
pub fn admin_call(s: &mut Shared, req: &Request, now: u64, admin: &str, rest: &str) -> Result<Response, Refusal> {
    let seg = segments(rest);
    let device_id = |id: &str| improve_proto::parse_hex32(id).ok_or_else(|| refuse(404, "device", "no such device"));
    match (req.method.as_str(), seg.as_slice()) {
        ("GET", ["devices"]) => {
            let mut list: Vec<&hub::Device> = s.hub.devices().collect();
            list.sort_by_key(|d| (d.state != DeviceState::Pending, d.at));
            let mut w = begin();
            w.key("devices").begin_array();
            for d in list {
                w.begin_object();
                write_device(&mut w, &s.hub, d, now, true);
                w.key("held").begin_array();
                for r in s.hub.requests().iter().filter(|r| r.device == d.id && r.state == State::Held) { w.begin_object(); write_summary(&mut w, r); w.end_object(); }
                w.end_array().end_object();
            }
            w.end_array().end_object();
            ok(200, w)
        }
        ("POST", ["devices", id, "decision"]) => {
            let b = Body::read(req, &["decision", "text"], &[])?;
            let decision = Decision::from_name(b.str("decision")?).ok_or_else(|| refuse(400, "decision", "approve, ask or reject"))?;
            let events = s.hub.decide(now, admin, &device_id(id)?, decision, &util::lines(b.str("text")?))?;
            s.commit(events)?;
            let mut w = begin(); w.field_str("state", s.hub.device(&device_id(id)?).unwrap().state.name()).end_object(); ok(200, w)
        }
        ("DELETE", ["devices", id]) => {
            let events = s.hub.revoke(now, &Actor::Admin(admin.into()), &device_id(id)?)?;
            s.commit(events)?;
            let mut w = begin(); w.end_object(); ok(200, w)
        }
        ("GET", ["users"]) => {
            let mut w = begin();
            w.key("users").begin_array();
            for u in s.hub.users() {
                w.begin_object().field_int("id", u.id as i64).field_str("display_name", &u.display_name).field_int("trust", u.trust.value() as i64);
                w.key("devices").begin_array();
                for d in &u.devices { w.string(&hex(d)); }
                w.end_array().key("history").begin_array();
                for c in &u.history {
                    w.begin_object().field_int("at_ms", c.at as i64).field_int("from", c.from as i64).field_int("to", c.to as i64)
                        .field_str("reason", &c.reason).field_str("author", &c.author.text()).end_object();
                }
                w.end_array().end_object();
            }
            w.end_array().end_object();
            ok(200, w)
        }
        ("POST", ["users", id, "trust"]) => {
            let b = Body::read(req, &["trust", "reason"], &[])?;
            let to = i16::try_from(b.int("trust")?).map_err(|_| refuse(400, "trust", "a trust is from -1023 to 1023"))?;
            let id = number(id).ok_or_else(|| refuse(404, "user", "no such user"))?;
            let events = s.hub.set_trust(now, admin, id, to, b.str("reason")?)?;
            s.commit(events)?;
            let mut w = begin(); w.field_int("trust", to as i64).end_object(); ok(200, w)
        }
        ("GET", ["requests"]) => {
            let state = query(req, "state").map(|v| State::from_name(&v).ok_or_else(|| refuse(400, "state", "state names no state"))).transpose()?;
            let mut w = begin();
            w.key("requests").begin_array();
            for r in s.hub.requests().iter().filter(|r| state.is_none_or(|st| r.state == st)) {
                w.begin_object();
                write_summary(&mut w, r);
                w.field_int("user", r.user as i64).field_str("device", &hex(&r.device)).end_object();
            }
            w.end_array().end_object();
            ok(200, w)
        }
        ("GET", ["requests", id]) => {
            let r = number(id).and_then(|id| s.hub.request(id)).ok_or_else(|| refuse(404, "request", "no such request"))?;
            let mut w = begin();
            w.key("request").begin_object();
            write_request(&mut w, r);
            w.field_int("user", r.user as i64).field_str("device", &hex(&r.device)).end_object().end_object();
            ok(200, w)
        }
        ("GET", ["requests", id, "attachments", n]) => {
            let r = number(id).and_then(|id| s.hub.request(id)).cloned().ok_or_else(|| refuse(404, "request", "no such request"))?;
            attachment(s, &r, None, n)
        }
        ("GET", ["requests", id, "messages", p, "attachments", n]) => {
            let r = number(id).and_then(|id| s.hub.request(id)).cloned().ok_or_else(|| refuse(404, "request", "no such request"))?;
            attachment(s, &r, Some(p), n)
        }
        ("POST", ["requests", id, "clear"]) => {
            let id = number(id).ok_or_else(|| refuse(404, "request", "no such request"))?;
            let b = Body::read(req, &["reason"], &[])?;
            let events = s.hub.clear_flag(now, admin, id, b.str("reason")?)?;
            s.commit(events)?;
            let mut w = begin(); w.end_object(); ok(200, w)
        }
        ("POST", ["requests", id, "state"]) => {
            let id = number(id).ok_or_else(|| refuse(404, "request", "no such request"))?;
            let (to, reason, extra) = state_body(req)?;
            let events = s.hub.move_request(now, &Actor::Admin(admin.into()), id, to, &reason, &extra)?;
            s.commit(events)?;
            let mut w = begin(); w.field_str("state", s.hub.request(id).unwrap().state.name()).end_object(); ok(200, w)
        }
        ("POST", ["requests", id, "messages"]) => {
            let id = number(id).ok_or_else(|| refuse(404, "request", "no such request"))?;
            let text = std::str::from_utf8(&req.body).map_err(|_| refuse(400, "body", "the body is not UTF-8"))?;
            post(s, now, Actor::Admin(admin.into()), id, wire::read(text, wire::decode_message).map_err(wire_refusal)?)
        }
        ("POST", ["delivered"]) => {
            let b = Body::read(req, &["release", "requests"], &[])?;
            let ids = b.value.get("requests").and_then(Value::as_array).ok_or_else(|| refuse(400, "requests", "a list of request numbers"))?
                .iter().map(|v| v.as_int().and_then(|i| u64::try_from(i).ok()).ok_or_else(|| refuse(400, "requests", "a list of request numbers"))).collect::<Result<Vec<_>, _>>()?;
            let events = s.hub.delivered(now, admin, b.str("release")?, &ids)?;
            s.commit(events)?;
            let mut w = begin(); w.end_object(); ok(200, w)
        }
        ("GET", ["settings"]) => {
            let mut w = begin();
            w.key("settings").begin_object();
            for name in hub::Settings::NAMES { w.field_str(name, &s.hub.settings.get(name).unwrap()); }
            w.end_object().end_object();
            ok(200, w)
        }
        ("POST", ["settings"]) => {
            let b = Body::read(req, &["name", "value"], &[])?;
            let events = s.hub.set_setting(now, admin, b.str("name")?, b.str("value")?)?;
            s.commit(events)?;
            let mut w = begin(); w.end_object(); ok(200, w)
        }
        ("GET", ["journal"]) => {
            let from = query(req, "from").map(|v| number(&v).ok_or_else(|| refuse(400, "from", "from is a line's number"))).transpose()?.unwrap_or(1);
            let lines = s.journal.lines(from, 1000).map_err(|e| refuse(500, "journal", format!("the journal cannot be read: {e}")))?;
            let mut w = begin();
            w.key("lines").begin_array();
            for (n, l) in lines { w.begin_object().field_int("n", n as i64).field_str("event", &l).end_object(); }
            w.end_array().end_object();
            ok(200, w)
        }
        ("POST", ["sign-in-code"]) => {
            let code = util::code();
            let events = s.hub.make_code(now, Owner::Admin(admin.into()), &code)?;
            s.commit(events)?;
            let mut w = begin();
            w.field_str("code", &code).field_int("until_ms", (now + hub::CODE_MS) as i64).end_object();
            ok(201, w)
        }
        _ => Err(refuse(404, "route", "no such route of the admin's API")),
    }
}
