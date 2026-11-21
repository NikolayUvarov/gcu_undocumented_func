//! The conformance suite (docs/hub/README.md, 10.3): drives a hub over `hub-api 2` and its pages, as devices, a
//! browser, the agents and the admin do, and checks each answer. It needs the hub's admin key and three agent keys
//! (intake, a track's, the integrator's), and a network that registered no key there within a day, as it registers
//! three and checks that a rejection holds the fourth back. `mindhub conformance` runs it against any hub; the host
//! tests run it against `tools/mindhub`, and MIND Core's native hub (stage 7) is to pass it too.

use crate::client::{Client, Reply};
use crate::util::{self, hex, sha256};
use ed25519_dalek::SigningKey;
use improve_proto::form::{Attachment, AttachmentKind, DeviceKind, Joining, Kind, Message, MessageKind, Registration, Request, Verdict, Visibility};
use improve_proto::json::{Value, Writer};
use improve_proto::{sign, wire, API};

pub struct Target {
    pub url: String,
    pub pins: Vec<[u8; 32]>,
    pub admin: SigningKey,
    pub intake: SigningKey,
    pub track: SigningKey,
    /// The track `track` is the agent of.
    pub track_code: String,
    pub integrator: SigningKey,
}

type Check = Result<(), String>;

fn fail<T>(what: &str, r: &Reply) -> Result<T, String> { Err(format!("{what}: the hub answered {} {}", r.status, r.text())) }

fn expect(what: &str, r: &Reply, status: u16, error: Option<&str>) -> Result<Value, String> {
    if r.status != status || (error.is_some() && r.error().as_deref() != error) { return fail(what, r); }
    if r.header("Hub-Challenge").and_then(improve_proto::parse_hex32).is_none() { return Err(format!("{what}: the answer carries no next challenge")); }
    Ok(r.json())
}

fn int(v: &Value, path: &[&str]) -> Option<i64> { path.iter().try_fold(v, |v, k| v.get(k))?.as_int() }
fn text<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> { path.iter().try_fold(v, |v, k| v.get(k))?.as_str() }

fn api_body(fields: &[(&str, &str)]) -> Vec<u8> {
    let mut w = Writer::new();
    w.begin_object().field_str("api", API);
    for (k, v) in fields { w.field_str(k, v); }
    w.end_object();
    w.finish().into_bytes()
}

struct Suite<'a> { t: &'a Target, log: &'a mut dyn FnMut(&str), n: usize }

impl Suite<'_> {
    fn client(&self, key: SigningKey) -> Result<Client, String> { Client::new(&self.t.url, &self.t.pins, key) }
    fn device(&self) -> Result<Client, String> { self.client(SigningKey::from_bytes(&util::random::<32>())) }
    fn ok(&mut self, what: &str) { self.n += 1; (self.log)(&format!("ok {} - {what}", self.n)); }
}

fn io(e: std::io::Error) -> String { format!("the hub cannot be reached: {e}") }

fn register(c: &mut Client, joining: Joining) -> std::io::Result<Reply> {
    let reg = Registration { public_key: c.public_key(), name: "conformance".into(), kind: DeviceKind::Mind, platform: "x86_64".into(),
                             version: "202611200000".into(), joining, contact: None, work: None };
    c.call("POST", "/hub/v1/devices", wire::encode_registration(&reg).as_bytes())
}

fn new_user(c: &mut Client, name: &str) -> std::io::Result<Reply> { register(c, Joining::NewUser { display_name: name.into() }) }

fn request(title: &str, visibility: Visibility, attachments: Vec<Attachment>) -> Request {
    Request { client_id: hex(&util::random::<8>()), kind: Kind::Fault, title: title.into(), text: format!("Conformance: {title}"), visibility, security: false, named: false, attachments }
}

fn message(kind: MessageKind, text: &str) -> Message { Message { kind, text: text.into(), answers: None, verdict: None, release: None, attachments: Vec::new() } }

fn send_message(c: &mut Client, base: &str, id: i64, m: &Message) -> std::io::Result<Reply> {
    c.call("POST", &format!("{base}/requests/{id}/messages"), wire::encode_message(m).as_bytes())
}

fn state_of(c: &mut Client, id: i64) -> Result<String, String> {
    let r = c.call("GET", &format!("/hub/v1/requests/{id}"), b"").map_err(io)?;
    Ok(text(&expect("a request read back", &r, 200, None)?, &["request", "state"]).unwrap_or("").to_string())
}

fn move_to(c: &mut Client, base: &str, id: i64, to: &str, extra: &[(&str, &str)]) -> std::io::Result<Reply> {
    let mut fields = vec![("to", to), ("reason", "conformance")];
    fields.extend_from_slice(extra);
    c.call("POST", &format!("{base}/requests/{id}/state"), &api_body(&fields))
}

/// The cookie a sign-in set, as `name=value`.
fn cookie(r: &Reply) -> Option<String> { r.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("set-cookie")).map(|(_, v)| v.split(';').next().unwrap_or("").to_string()) }

/// A form's token, read from a page as a browser would.
fn token(page: &str) -> Option<String> {
    let at = page.find("name=t value=\"")? + "name=t value=\"".len();
    Some(page[at..].split('"').next()?.to_string())
}

fn form(fields: &[(&str, &str)]) -> Vec<u8> {
    fields.iter().map(|(k, v)| format!("{k}={}", v.bytes().map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>()))
        .collect::<Vec<_>>().join("&").into_bytes()
}

/// Runs the suite, saying each check as it passes; the first that fails ends it, naming what the hub answered.
pub fn run(t: &Target, log: &mut dyn FnMut(&str)) -> Check {
    let mut s = Suite { t, log, n: 0 };
    let mut admin = s.client(t.admin.clone())?;
    let mut intake = s.client(t.intake.clone())?;
    let mut track = s.client(t.track.clone())?;
    let mut lander = s.client(t.integrator.clone())?;

    // The challenge, the board, unsigned and stale calls.
    let mut anon = s.device()?;
    let r = anon.plain("GET", "/hub/v1/challenge", &[], b"").map_err(io)?;
    let v = expect("a challenge", &r, 200, None)?;
    if int(&v, &["work_bits"]).is_none() { return Err("the challenge's answer does not say the proof of work it asks".into()); }
    let r = anon.plain("GET", "/hub/v1/board", &[], b"").map_err(io)?;
    wire::read(&r.text(), wire::decode_board).map_err(|e| format!("the board's JSON: {e:?}"))?;
    s.ok("a challenge in every answer, and the board's JSON");
    let r = anon.plain("POST", "/hub/v1/requests", &[], b"{}").map_err(io)?;
    expect("an unsigned call", &r, 401, Some("unsigned"))?;
    let used = anon.challenge().ok_or("no challenge given")?;
    let r = anon.call("GET", "/hub/v1/devices/me", b"").map_err(io)?;
    expect("an unregistered key", &r, 403, Some("unregistered"))?;
    let header = sign::header(anon.key(), "GET", "/hub/v1/devices/me", b"", &used).map_err(|e| format!("{e:?}"))?;
    let r = anon.plain("GET", "/hub/v1/devices/me", &[(sign::HEADER, header)], b"").map_err(io)?;
    expect("a used challenge", &r, 401, Some("challenge"))?;
    let fresh = anon.challenge().ok_or("no challenge given")?;
    let other = SigningKey::from_bytes(&util::random::<32>());
    let forged = sign::header(&other, "GET", "/hub/v1/devices/other", b"", &fresh).map_err(|e| format!("{e:?}"))?;
    let r = anon.plain("GET", "/hub/v1/devices/me", &[(sign::HEADER, forged)], b"").map_err(io)?;
    expect("a signature over another path", &r, 401, Some("signature"))?;
    s.ok("unsigned, used challenges and signatures over another call are refused");

    // A registers: pending, one held request, the admin asks, A answers, the admin approves.
    let mut a = s.device()?;
    let a_id = hex(&sign::id(&a.public_key()));
    let v = expect("a registration", &new_user(&mut a, "Conformance A").map_err(io)?, 201, None)?;
    if text(&v, &["state"]) != Some("pending") { return Err("a new device is not pending".into()); }
    expect("the same registration again", &new_user(&mut a, "Conformance A").map_err(io)?, 200, None)?;
    let v = expect("a pending device's state", &a.call("GET", "/hub/v1/devices/me", b"").map_err(io)?, 200, None)?;
    if text(&v, &["access"]) != Some("pending") { return Err("devices/me does not say pending".into()); }
    let a_user = int(&v, &["user", "id"]).ok_or("devices/me names no user")?;
    let held = request("A held request", Visibility::Private, Vec::new());
    let v = expect("a pending device's request", &a.call("POST", "/hub/v1/requests", wire::encode_request(&held).as_bytes()).map_err(io)?, 201, None)?;
    let held_id = int(&v, &["id"]).ok_or("no id")?;
    if text(&v, &["state"]) != Some("held") { return Err("a pending device's request is not held".into()); }
    let r = a.call("POST", "/hub/v1/requests", wire::encode_request(&request("A second", Visibility::Private, Vec::new())).as_bytes()).map_err(io)?;
    expect("a second held request", &r, 403, Some("held"))?;
    s.ok("a pending device files one held request");
    let v = expect("the admin's devices", &admin.call("GET", "/hub/admin/v1/devices", b"").map_err(io)?, 200, None)?;
    let listed = v.get("devices").and_then(Value::as_array).is_some_and(|l| l.iter().any(|d| text(d, &["device"]) == Some(&a_id) && text(d, &["state"]) == Some("pending")
        && d.get("held").and_then(Value::as_array).is_some_and(|h| h.iter().any(|r| int(r, &["id"]) == Some(held_id)))));
    if !listed { return Err("the admin does not see the pending device with its held request".into()); }
    let decide = |c: &mut Client, id: &str, d: &str, text: &str| c.call("POST", &format!("/hub/admin/v1/devices/{id}/decision"), &api_body(&[("decision", d), ("text", text)]));
    expect("the admin's question", &decide(&mut admin, &a_id, "ask", "Which machine is it?").map_err(io)?, 200, None)?;
    let v = expect("the question read", &a.call("GET", "/hub/v1/devices/me", b"").map_err(io)?, 200, None)?;
    let asked = v.get("notes").and_then(Value::as_array).and_then(|n| n.last()).is_some_and(|n| text(n, &["kind"]) == Some("question") && text(n, &["text"]) == Some("Which machine is it?"));
    if !asked { return Err("the device does not see the admin's question".into()); }
    expect("the device's answer", &a.call("POST", "/hub/v1/devices/me/messages", &api_body(&[("text", "A test machine")])).map_err(io)?, 201, None)?;
    expect("the approval", &decide(&mut admin, &a_id, "approve", "Welcome").map_err(io)?, 200, None)?;
    if state_of(&mut a, held_id)? != "new" { return Err("the held request did not go to intake on approval".into()); }
    s.ok("the admin asks, the device answers, the approval releases its held request");
    let r = a.call("POST", "/hub/v1/requests", wire::encode_request(&request("A again today", Visibility::Public, Vec::new())).as_bytes()).map_err(io)?;
    expect("band 0's second request in a day", &r, 429, Some("requests"))?;
    if r.header("Retry-After").and_then(|v| v.parse::<u64>().ok()).is_none_or(|v| v == 0) { return Err("a 429 without Retry-After".into()); }
    s.ok("band 0 takes one request a day, and the refusal says when");

    // B: a public request with an attachment, its repeat, its bounds.
    let mut b = s.device()?;
    let b_id = hex(&sign::id(&b.public_key()));
    expect("B's registration", &new_user(&mut b, "Conformance B").map_err(io)?, 201, None)?;
    expect("B's approval", &decide(&mut admin, &b_id, "approve", "Welcome").map_err(io)?, 200, None)?;
    let log = b"[    0.000000] conformance boot log\n".repeat(20);
    let entry = Attachment { kind: AttachmentKind::Log, name: "boot.log".into(), size: log.len() as u64, sha256: sha256(&log) };
    let mut long = request("B title", Visibility::Public, Vec::new());
    long.title = "x".repeat(121);
    expect("a title over its bound", &b.call("POST", "/hub/v1/requests", wire::encode_request(&long).as_bytes()).map_err(io)?, 413, Some("title"))?;
    let big = vec![b' '; improve_proto::json::BODY_MAX + 1];
    let r = b.call("POST", "/hub/v1/requests", &big).map_err(io)?;
    if r.status != 413 { return fail("a body over 64 KiB", &r); }
    let breq = Request { named: true, ..request("B: the camera does not switch", Visibility::Public, vec![entry.clone()]) };
    let v = expect("B's request", &b.call("POST", "/hub/v1/requests", wire::encode_request(&breq).as_bytes()).map_err(io)?, 201, None)?;
    let bid = int(&v, &["id"]).ok_or("no id")?;
    let v = expect("the same request again", &b.call("POST", "/hub/v1/requests", wire::encode_request(&breq).as_bytes()).map_err(io)?, 200, None)?;
    if int(&v, &["id"]) != Some(bid) { return Err("a client ID filed twice".into()); }
    let repeat = Request { client_id: hex(&util::random::<8>()), ..breq.clone() };
    let r = b.call("POST", "/hub/v1/requests", wire::encode_request(&repeat).as_bytes()).map_err(io)?;
    let v = expect("a repeat", &r, 409, Some("repeat"))?;
    if int(&v, &["earlier"]) != Some(bid) { return Err("a repeat does not name the earlier request".into()); }
    s.ok("bounds, a client ID filed once, a repeat refused with the earlier number");
    let put = |c: &mut Client, bytes: &[u8]| c.call("PUT", &format!("/hub/v1/requests/{bid}/attachments/0"), bytes);
    let mut wrong = log.clone();
    wrong[0] ^= 1;
    expect("an attachment's wrong bytes", &put(&mut b, &wrong).map_err(io)?, 400, Some("sha256"))?;
    expect("an attachment", &put(&mut b, &log).map_err(io)?, 200, None)?;
    s.ok("an attachment's bytes checked against its SHA-256");

    // A deletes its request, which no track took: gone from its list and from intake's queue (design 9).
    let a_second_look = held_id;
    expect("A deleting its request", &a.call("DELETE", &format!("/hub/v1/requests/{a_second_look}"), b"").map_err(io)?, 200, None)?;
    expect("a deleted request read", &a.call("GET", &format!("/hub/v1/requests/{held_id}"), b"").map_err(io)?, 404, Some("request"))?;
    s.ok("a request its user deleted before a track took it is gone");

    // R is rejected, and after a rejection this network waits for its next new key.
    let mut rj = s.device()?;
    let r_id = hex(&sign::id(&rj.public_key()));
    expect("R's registration", &new_user(&mut rj, "Conformance R").map_err(io)?, 201, None)?;
    expect("R's rejection", &decide(&mut admin, &r_id, "reject", "Not about MIND Core").map_err(io)?, 200, None)?;
    expect("a rejected key's call", &rj.call("GET", "/hub/v1/devices/me", b"").map_err(io)?, 403, Some("rejected"))?;
    expect("a rejected key registering again", &new_user(&mut rj, "Conformance R").map_err(io)?, 403, Some("rejected"))?;
    let mut q = s.device()?;
    expect("a new key after a rejection", &new_user(&mut q, "Conformance Q").map_err(io)?, 429, Some("registrations"))?;
    s.ok("a rejected key is refused, and its network waits");

    // The agents: intake asks, B answers, intake accepts, the track works, the integrator lands, B checks.
    let v = expect("intake's queue", &intake.call("GET", "/hub/agent/v1/queue", b"").map_err(io)?, 200, None)?;
    let queued: Vec<i64> = v.get("requests").and_then(Value::as_array).map(|l| l.iter().filter_map(|r| int(r, &["id"])).collect()).unwrap_or_default();
    if !queued.contains(&bid) || queued.contains(&held_id) { return Err("intake's queue lacks the new request, or holds the deleted one".into()); }
    expect("a deleted request for an agent", &track.call("GET", &format!("/hub/agent/v1/requests/{held_id}"), b"").map_err(io)?, 404, Some("request"))?;
    expect("intake's question", &send_message(&mut intake, "/hub/agent/v1", bid, &message(MessageKind::Question, "Which cameras?")).map_err(io)?, 201, None)?;
    if state_of(&mut b, bid)? != "needs-info" { return Err("a question did not move the request to needs-info".into()); }
    let v = expect("B's thread", &b.call("GET", &format!("/hub/v1/requests/{bid}"), b"").map_err(io)?, 200, None)?;
    let qn = v.get("request").and_then(|r| r.get("thread")).and_then(Value::as_array)
        .and_then(|t| t.iter().find(|p| text(p, &["kind"]) == Some("question")).and_then(|p| int(p, &["n"]))).ok_or("the question is not in the thread")?;
    let lsusb = b"Bus 001 Device 002: ID 046d:0825 Webcam C270\nBus 001 Device 003: ID 0c45:6366 Microdia\n".to_vec();
    let lsusb_entry = Attachment { kind: AttachmentKind::Text, name: "lsusb.txt".into(), size: lsusb.len() as u64, sha256: sha256(&lsusb) };
    let answer = Message { answers: Some(qn as u32), attachments: vec![lsusb_entry], ..message(MessageKind::Answer, "Two USB cameras") };
    let v = expect("B's answer", &send_message(&mut b, "/hub/v1", bid, &answer).map_err(io)?, 201, None)?;
    let an = int(&v, &["n"]).ok_or("the answer's number")?;
    expect("the answer's attachment", &b.call("PUT", &format!("/hub/v1/requests/{bid}/messages/{an}/attachments/0"), &lsusb).map_err(io)?, 200, None)?;
    if state_of(&mut b, bid)? != "new" { return Err("the answer did not take the request back".into()); }
    s.ok("a question and its answer, both ways");
    let task = format!("000-{}-9999", t.track_code);
    expect("intake's acceptance", &move_to(&mut intake, "/hub/agent/v1", bid, "accepted", &[("track", &t.track_code), ("task", &task)]).map_err(io)?, 200, None)?;
    let v = expect("B's trust", &b.call("GET", "/hub/v1/devices/me", b"").map_err(io)?, 200, None)?;
    if int(&v, &["user", "trust"]) != Some(32) { return Err("an acceptance did not raise the trust by 32".into()); }
    let board = wire::read(&anon.plain("GET", "/hub/v1/board", &[], b"").map_err(io)?.text(), wire::decode_board).map_err(|e| format!("{e:?}"))?;
    if !board.iter().any(|e| e.id == bid as u64 && e.task == task && e.author.as_deref() == Some("Conformance B")) { return Err("the board does not show the accepted request with the name its user allowed".into()); }
    if board.iter().any(|e| e.id == held_id as u64) { return Err("the board shows a request that is not a task".into()); }
    s.ok("an acceptance names the task, raises the trust, and the board shows it alone, with the name its user allowed");
    expect("deleting an accepted request", &b.call("DELETE", &format!("/hub/v1/requests/{bid}"), b"").map_err(io)?, 409, Some("accepted"))?;
    expect("the track's move", &move_to(&mut track, "/hub/agent/v1", bid, "in-progress", &[]).map_err(io)?, 200, None)?;
    let result = Message { release: Some("202611210000".into()), ..message(MessageKind::Result, "Press n; the title names the camera") };
    expect("the track's result", &send_message(&mut track, "/hub/agent/v1", bid, &result).map_err(io)?, 201, None)?;
    expect("the track landing it itself", &move_to(&mut track, "/hub/agent/v1", bid, "landed", &[]).map_err(io)?, 409, Some("state"))?;
    expect("the integrator's landing", &move_to(&mut lander, "/hub/agent/v1", bid, "landed", &[]).map_err(io)?, 200, None)?;
    let r = admin.call("POST", "/hub/admin/v1/delivered", format!(r#"{{"api":"{API}","release":"202611210000","requests":[]}}"#).as_bytes()).map_err(io)?;
    expect("an empty delivery", &r, 200, None)?;
    let body = format!(r#"{{"api":"{API}","release":"202611210000","requests":[{bid}]}}"#);
    expect("the delivery", &admin.call("POST", "/hub/admin/v1/delivered", body.as_bytes()).map_err(io)?, 200, None)?;
    if state_of(&mut b, bid)? != "delivered" { return Err("the request is not delivered".into()); }
    expect("B's version", &b.call("POST", "/hub/v1/devices/me/version", &api_body(&[("version", "202611210000")])).map_err(io)?, 200, None)?;
    if state_of(&mut b, bid)? != "awaiting-check" { return Err("a device that runs the release does not await the check".into()); }
    let works = Message { verdict: Some(Verdict::Works), release: Some("202611210000".into()), ..message(MessageKind::Verdict, "It switches") };
    expect("B's verdict", &send_message(&mut b, "/hub/v1", bid, &works).map_err(io)?, 201, None)?;
    if state_of(&mut b, bid)? != "verified" { return Err("a works verdict did not verify the request".into()); }
    let r = track.call("GET", &format!("/hub/agent/v1/requests/{bid}/attachments/0"), b"").map_err(io)?;
    if r.status != 200 || r.body != log { return fail("the attachment for its track", &r); }
    let r = track.call("GET", &format!("/hub/agent/v1/requests/{bid}/messages/{an}/attachments/0"), b"").map_err(io)?;
    if r.status != 200 || r.body != lsusb { return fail("the answer's attachment for its track", &r); }
    s.ok("in progress, landed, delivered, awaiting the check, verified; the track reads the request's and the answer's attachments");
    let v = expect("B's export", &b.call("GET", "/hub/v1/me/export", b"").map_err(io)?, 200, None)?;
    let exported = v.get("requests").and_then(Value::as_array).is_some_and(|l| l.iter().any(|r| int(r, &["id"]) == Some(bid)));
    if !exported || text(&v, &["user", "display_name"]) != Some("Conformance B") { return Err("the export lacks the user or their request".into()); }
    s.ok("a user's export holds their name and their requests");

    // Pairing: a device joins B by a code; a browser signs in by another; revoking ends both.
    let pair = |c: &mut Client| -> Result<String, String> {
        let v = expect("a pairing code", &c.call("POST", "/hub/v1/devices/pair", b"").map_err(io)?, 201, None)?;
        text(&v, &["code"]).map(String::from).ok_or_else(|| "no code".into())
    };
    let code = pair(&mut b)?;
    let mut c = s.device()?;
    let c_id = hex(&sign::id(&c.public_key()));
    let v = expect("a paired registration", &register(&mut c, Joining::Pair { code: code.clone() }).map_err(io)?, 201, None)?;
    if text(&v, &["state"]) != Some("approved") { return Err("a device paired by a code is not approved".into()); }
    let v = expect("B's devices", &b.call("GET", "/hub/v1/devices", b"").map_err(io)?, 200, None)?;
    let listed: Vec<(String, bool)> = v.get("devices").and_then(Value::as_array).map(|l| l.iter()
        .filter_map(|d| Some((text(d, &["device"])?.to_string(), d.get("this")?.as_bool()?))).collect()).unwrap_or_default();
    if listed != [(b_id.clone(), true), (c_id.clone(), false)] { return Err(format!("B's devices are not B's and C's: {listed:?}")); }
    let mut c2 = s.device()?;
    expect("a code used twice", &register(&mut c2, Joining::Pair { code }).map_err(io)?, 403, Some("pair_code"))?;
    let browser_code = pair(&mut c)?;
    let r = anon.plain("POST", "/hub/sign-in", &[("Content-Type", "application/x-www-form-urlencoded".into())], &form(&[("code", &browser_code)])).map_err(io)?;
    let session = cookie(&r).filter(|_| r.status == 303).ok_or_else(|| format!("the sign-in answered {}", r.status))?;
    let page = anon.plain("GET", "/hub/", &[("Cookie", session.clone())], b"").map_err(io)?;
    if page.status != 200 || !page.text().contains("Conformance B") { return fail("the user's page", &page); }
    let t_value = token(&page.text()).ok_or("the page's forms carry no token")?;
    let r = anon.plain("GET", "/hub/export", &[("Cookie", session.clone())], b"").map_err(io)?;
    if r.status != 200 || !r.text().contains("Conformance B") { return fail("the export from a browser", &r); }
    let comment = form(&[("t", "0"), ("kind", "comment"), ("text", "from a browser")]);
    let r = anon.plain("POST", &format!("/hub/requests/{bid}/messages"), &[("Cookie", session.clone())], &comment).map_err(io)?;
    if r.status != 403 { return fail("a form without its session's token", &r); }
    let file = form(&[("t", &t_value), ("client_id", &hex(&util::random::<8>())), ("kind", "fault"), ("title", "From a browser"), ("text", "typed on a phone"), ("visibility", "public")]);
    let r = anon.plain("POST", "/hub/requests", &[("Cookie", session.clone())], &file).map_err(io)?;
    if r.status != 429 { return fail("a browser's request beyond the user's limit", &r); }
    s.ok("a device paired by a code, a browser signed in by one, its forms checked");
    expect("revoking C from B", &b.call("DELETE", &format!("/hub/v1/devices/{c_id}"), b"").map_err(io)?, 200, None)?;
    expect("a revoked key's call", &c.call("GET", "/hub/v1/devices/me", b"").map_err(io)?, 403, Some("revoked"))?;
    let page = anon.plain("GET", "/hub/", &[("Cookie", session)], b"").map_err(io)?;
    if page.text().contains("Conformance B") { return Err("a revoked device's browser session still opens the user's page".into()); }
    s.ok("revoking a device ends its key and its browser's session");

    // The admin pages, by a code from the admin key.
    let v = expect("the admin's code", &admin.call("POST", "/hub/admin/v1/sign-in-code", b"").map_err(io)?, 201, None)?;
    let code = text(&v, &["code"]).ok_or("no code")?.to_string();
    let r = anon.plain("POST", "/hub/admin/sign-in", &[], &form(&[("code", &code)])).map_err(io)?;
    let admin_session = cookie(&r).filter(|_| r.status == 303).ok_or_else(|| format!("the admin's sign-in answered {}", r.status))?;
    for page in ["/hub/admin/", "/hub/admin/users", "/hub/admin/requests", &format!("/hub/admin/requests/{bid}"), "/hub/admin/settings", "/hub/admin/journal"] {
        let r = anon.plain("GET", page, &[("Cookie", admin_session.clone())], b"").map_err(io)?;
        if r.status != 200 || !r.text().contains("Sign out") { return fail(page, &r); }
    }
    let v = expect("the journal", &admin.call("GET", "/hub/admin/v1/journal", b"").map_err(io)?, 200, None)?;
    if v.get("lines").and_then(Value::as_array).is_none_or(|l| l.is_empty()) { return Err("the journal is empty".into()); }
    s.ok("the admin pages open by a code from the admin key, and the journal is read");

    // A negative trust reads, files nothing; -1023 is a ban.
    let set = |c: &mut Client, user: i64, trust: i64| c.call("POST", &format!("/hub/admin/v1/users/{user}/trust"),
        format!(r#"{{"api":"{API}","trust":{trust},"reason":"conformance"}}"#).as_bytes());
    expect("a negative trust set", &set(&mut admin, a_user, -5).map_err(io)?, 200, None)?;
    let r = a.call("POST", "/hub/v1/requests", wire::encode_request(&request("A read-only", Visibility::Public, Vec::new())).as_bytes()).map_err(io)?;
    expect("a request at a negative trust", &r, 403, Some("trust"))?;
    expect("reading at a negative trust", &a.call("GET", "/hub/v1/requests", b"").map_err(io)?, 200, None)?;
    expect("a ban set", &set(&mut admin, a_user, -1023).map_err(io)?, 200, None)?;
    expect("a banned user's call", &a.call("GET", "/hub/v1/devices/me", b"").map_err(io)?, 403, Some("ban"))?;
    s.ok("a negative trust is read-only, and -1023 a ban");

    let r = anon.plain("GET", "/hub/board", &[], b"").map_err(io)?;
    if r.status != 200 || !r.text().contains("B: the camera does not switch") { return fail("the board's page", &r); }
    s.ok("the board's page");

    // B deletes their account: its devices revoked, the task it asked for stays on the board with no name.
    expect("B's account deleted", &b.call("DELETE", "/hub/v1/me", b"").map_err(io)?, 200, None)?;
    expect("a deleted account's device", &b.call("GET", "/hub/v1/devices/me", b"").map_err(io)?, 403, Some("revoked"))?;
    let board = wire::read(&anon.plain("GET", "/hub/v1/board", &[], b"").map_err(io)?.text(), wire::decode_board).map_err(|e| format!("{e:?}"))?;
    if !board.iter().any(|e| e.id == bid as u64 && e.author.is_none()) { return Err("a deleted account's task is not on the board without a name".into()); }
    s.ok("a deleted account: its devices revoked, its task on the board with no name");
    Ok(())
}
