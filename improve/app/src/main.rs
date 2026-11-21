#![no_std]
#![no_main]
// improve (100003-APP-0151; docs/hub/README.md, 2 and 9): MIND Hub's client on MIND Core, a console program with the
// design's commands. Its authority is what the design gives it: a flow grant to the hub (a host it does not name is
// asked of the user, socket.wit 2.8), TLS to the hub's pinned key with no device certificate, its own Ed25519 key as
// its own secret in keystore (SLOT_SECRETS), its own directory for the hub's address and pins (SLOT_OWN). It holds a
// long-term key and the network, so the hub's answers are parsed by `parse` (MC-11.11), and their tree is checked with
// improve/proto's forms. Each call is signed over a challenge the hub gave in the body of `GET challenge`.
extern crate alloc;

mod words;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use ed25519_dalek::SigningKey;
use improve_proto::form::{DeviceKind, Joining, Kind, Message, MessageKind, Registration, Request, Verdict, Visibility};
use improve_proto::json::{self, Flat, FlatKind, Value};
use improve_proto::{sign, wire, work};
use mind::abi::{BootInfo, SLOT_NETWORK, SLOT_PARSE, SLOT_SECRETS, SLOT_TLS};
use mind::http::{self, Connection, Method, Transport, Url};
use mind::idl::codec::List;
use mind::idl::{keystore, parse, socket, tls};
use mind::ipc::Endpoint;
use words::Config;

mind::request!(REQUEST_CONSOLE | REQUEST_NETWORK | REQUEST_PARSE | REQUEST_TLS | REQUEST_SECRETS | REQUEST_OWN);

const STACK: Endpoint = Endpoint(SLOT_NETWORK);
const TLS: Endpoint = Endpoint(SLOT_TLS);
const PARSER: Endpoint = Endpoint(SLOT_PARSE);
const SECRETS: Endpoint = Endpoint(SLOT_SECRETS);
const KEY_SECRET: &str = "hub-key";
const CONFIG: &str = "hub.conf";
const IDLE_MS: usize = 15_000;
const CONNECT_MS: u32 = 10_000;
const ANSWER_MS: usize = 90_000;
/// The longest answer `parse.json` reads (`bytes<16384>`); a longer one is named as unread (requests-NET.md).
const PARSED_MAX: usize = 16384;

const USAGE: &str = "usage: improve COMMAND
  setup NAME ADDRESS|dns PORT PIN [PIN]   the hub, known by its TLS key's SPKI SHA-256; this device's key
  register \"DISPLAY NAME\" [--name DEVICE] | register --pair CODE [--name DEVICE]
  status | answer \"TEXT\"
  new fault|improvement|capability \"TITLE\" \"TEXT\" [--private] [--security] [--named] [--yes]
  list | show N | reply N \"TEXT\" [--answer Q] | check N works|partly|does-not-work RELEASE \"TEXT\"
  version RELEASE                         the release this machine runs, told to the hub
  devices | pair | revoke DEVICE | delete N";

fn now() -> usize { mind::time::uptime_ms() }

// A TLS session of the flow grant; a wait for a byte ends at IDLE_MS.
struct Link(u32);

impl Transport for Link {
    fn send(&mut self, data: &[u8]) -> Result<(), http::Error> {
        let (mut sent, until) = (0, now() + IDLE_MS);
        while sent < data.len() {
            let end = data.len().min(sent + 4096);
            let n = match tls::send(TLS, self.0, &data[sent..end]) { Ok(Ok(n)) => n as usize, Ok(Err(tls::Error::Again)) => 0, _ => return Err(http::Error::Transport) };
            if n == 0 { if now() >= until { return Err(http::Error::Transport); } mind::time::sleep(5); }
            sent += n;
        }
        Ok(())
    }
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, http::Error> {
        let (until, length) = (now() + IDLE_MS, buffer.len().min(4096));
        loop {
            match tls::receive(TLS, self.0, length as u32, &mut buffer[..length]) {
                Ok(Ok(n)) if n > 0 => return Ok(n as usize),
                Ok(Ok(_)) | Ok(Err(tls::Error::Again)) => {}
                Ok(Err(tls::Error::Closed)) => return Ok(0),
                _ => return Err(http::Error::Transport),
            }
            if now() >= until { return Err(http::Error::Transport); }
            mind::time::sleep(5);
        }
    }
}

impl Drop for Link { fn drop(&mut self) { let _ = tls::close(TLS, self.0); } }

/// The hub, its key, and the connection while there is one.
struct Hub { config: Config, key: SigningKey, connection: Option<Connection<Link>> }

/// An answer: its status and its body's tree (Null when the body is not JSON).
struct Answer { status: u16, body: Value }

fn read_config() -> Result<Config, String> {
    if !mind::fs::own::given() { return Err("no directory of its own: run improve by its name".into()); }
    let mut buffer = [0u8; 2048];
    let n = mind::fs::own::read(CONFIG, &mut buffer).map_err(|_| String::from("no hub yet: improve setup NAME ADDRESS|dns PORT PIN"))?;
    words::config(core::str::from_utf8(&buffer[..n]).map_err(|_| String::from("hub.conf is not UTF-8"))?)
}

fn read_key() -> Result<SigningKey, String> {
    let mut seed = [0u8; 32];
    match keystore::secret_get(SECRETS, KEY_SECRET, &mut seed) {
        Ok(Ok(32)) => Ok(SigningKey::from_bytes(&seed)),
        Ok(Ok(_)) => Err("its key in keystore is not 32 bytes".into()),
        Ok(Err(keystore::Error::Missing)) => Err("no key yet: improve setup first".into()),
        Ok(Err(e)) => Err(format!("keystore refused its key: {e:?}")),
        Err(_) => Err("no client of keystore for its secrets (run improve by its name)".into()),
    }
}

impl Hub {
    fn open() -> Result<Hub, String> { Ok(Hub { config: read_config()?, key: read_key()?, connection: None }) }

    // The hub's address: as configured, or the stack's DNS answer.
    fn address(&self) -> Result<u32, String> {
        if let Some(a) = self.config.address { return Ok(a); }
        match socket::resolve(STACK, &self.config.name, 0, 0, 5000) {
            Ok(Ok(a)) => Ok(a),
            Ok(Err(e)) => Err(format!("{}: not resolved ({e:?})", self.config.name)),
            Err(_) => Err("no network grant".into()),
        }
    }

    // A TLS session to the hub's pinned key; Denied when the grant does not name it.
    fn session(&self, address: u32) -> Result<u32, tls::Error> {
        let session = match tls::attach(TLS, SLOT_NETWORK) { Ok(Ok(s)) => s, Ok(Err(e)) => return Err(e), Err(_) => return Err(tls::Error::NoNetwork) };
        let mut pins = List::<tls::Pin, 2>::default();
        for p in &self.config.pins {
            let mut digest = List::default();
            for b in p { digest.push(*b); }
            pins.push(tls::Pin { kind: tls::PinKind::Spki, digest });
        }
        match tls::connect_pins(TLS, session, &self.config.name, address, self.config.port, pins.as_slice(), CONNECT_MS) {
            Ok(Ok(_)) => Ok(session),
            other => { let _ = tls::close(TLS, session); Err(other.ok().and_then(|r| r.err()).unwrap_or(tls::Error::NoNetwork)) }
        }
    }

    // A host the grant does not name is asked of the user once (socket.wit 2.8): the answer's address.
    fn ask(&self) -> Result<u32, String> {
        let (name, port) = (&self.config.name, self.config.port);
        let id = match socket::ask(STACK, name, port, "MIND Hub, the hub in improve's hub.conf") {
            Ok(Ok(id)) => id,
            other => return Err(format!("{name}:{port} is not in its grant, and could not be asked ({:?})", other.map_err(|_| ()))),
        };
        mind::println!("improve: {name}:{port} is not in its grant: you are asked");
        let asked = now();
        loop {
            match socket::asked(STACK, id) {
                Ok(Ok(r)) if r.state == socket::QuestionState::Allowed => return Ok(self.config.address.unwrap_or(r.address)),
                Ok(Ok(r)) if r.state == socket::QuestionState::Refused => return Err(format!("{name}:{port}: refused by the user")),
                Ok(Ok(_)) if now() - asked < ANSWER_MS => { mind::time::sleep(250); }
                _ => return Err(format!("{name}:{port}: no answer to its question")),
            }
        }
    }

    fn connect(&mut self) -> Result<&mut Connection<Link>, String> {
        if self.connection.is_none() {
            let address = self.address()?;
            let session = match self.session(address) {
                Ok(s) => s,
                Err(tls::Error::Denied) => {
                    let allowed = self.ask()?;
                    self.session(allowed).map_err(|e| self.why(e))?
                }
                Err(e) => return Err(self.why(e)),
            };
            self.connection = Some(Connection::new(Link(session)));
        }
        Ok(self.connection.as_mut().unwrap())
    }

    fn why(&self, e: tls::Error) -> String {
        let at = format!("{}:{}", self.config.name, self.config.port);
        match e {
            tls::Error::Certificate => format!("{at}: its key is not pinned (improve setup with the hub's pin)"),
            tls::Error::Timeout => format!("{at}: no answer"),
            tls::Error::Denied => format!("{at}: denied by the network policy"),
            other => format!("{at}: TLS {other:?}"),
        }
    }

    /// One request and its answer's body, the connection dropped on any failure so that the next one starts anew.
    fn exchange(&mut self, method: Method, path: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<(u16, Vec<u8>), String> {
        let text = format!("https://{}:{}{}", self.config.name, self.config.port, path);
        let url = Url::parse(&text).map_err(|_| format!("{path}: not a path"))?;
        let result = (|| {
            let connection = self.connect()?;
            let mut source: &[u8] = body;
            let mut parser = mind::parse::Service(PARSER);
            let with_body = matches!(method, Method::Post | Method::Put).then(|| (body.len() as u64, &mut source as &mut dyn http::Source));
            let mut response = connection.request(method, &url, headers, with_body, &mut parser, false).map_err(|e| format!("{path}: {e:?}"))?;
            let status = response.status;
            let mut out = Vec::new();
            let mut chunk = [0u8; 2048];
            loop {
                match response.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) if out.len() + n <= json::BODY_MAX => out.extend_from_slice(&chunk[..n]),
                    Ok(_) => return Err(format!("{path}: an answer over {} bytes", json::BODY_MAX)),
                    Err(e) => return Err(format!("{path}: {e:?}")),
                }
            }
            Ok((status, out))
        })();
        if result.is_err() { self.connection = None; }
        result
    }

    /// The answer's body as a tree: parsed by `parse`, built and checked by improve/proto (MC-11.11).
    fn tree(&self, body: &[u8]) -> Result<Value, String> {
        if body.len() > PARSED_MAX { return Err(format!("the hub's answer is {} bytes, past the {PARSED_MAX} the parse service reads (NET's change)", body.len())); }
        let mut owned: Vec<(FlatKind, Option<u16>, String, String)> = Vec::new();
        let mut start = 0u32;
        loop {
            let page = match parse::json(PARSER, body, start) {
                Ok(Ok(page)) => page,
                Ok(Err(_)) => return Err("the parse service refused the hub's answer (a text over 512 bytes waits for NET's change)".into()),
                Err(_) => return Err("no parse service".into()),
            };
            for v in page.page.as_slice() {
                let kind = match v.kind {
                    parse::JsonKind::Object => FlatKind::Object, parse::JsonKind::Array => FlatKind::Array, parse::JsonKind::String => FlatKind::String,
                    parse::JsonKind::Number => FlatKind::Number, parse::JsonKind::True => FlatKind::True, parse::JsonKind::False => FlatKind::False,
                    parse::JsonKind::Null => FlatKind::Null,
                };
                owned.push((kind, (v.parent != u16::MAX).then_some(v.parent), String::from(v.key.as_str()), String::from(v.text.as_str())));
            }
            let got = page.page.as_slice().len() as u32;
            start += got;
            if start >= page.values || got == 0 { break; }
        }
        let flat: Vec<Flat> = owned.iter().map(|(kind, parent, key, text)| Flat { kind: *kind, parent: *parent, key, text }).collect();
        json::tree(&flat).map_err(|e| format!("the hub's answer is not of the API's form: {e:?}"))
    }

    /// A fresh challenge, and the proof of work a registration needs now.
    fn challenge(&mut self) -> Result<([u8; 32], u8), String> {
        let (status, body) = self.exchange(Method::Get, "/hub/v1/challenge", &[], &[])?;
        let v = self.tree(&body)?;
        if status != 200 { return Err(format!("the hub answered {status} to a challenge")); }
        let c = v.get("challenge").and_then(Value::as_str).and_then(improve_proto::parse_hex32).ok_or("the hub gave no challenge in its body")?;
        Ok((c, v.get("work_bits").and_then(Value::as_int).unwrap_or(0).clamp(0, work::BITS_MAX as i64) as u8))
    }

    /// A call signed over a fresh challenge; a challenge gone stale is taken again once.
    fn call(&mut self, method: Method, path: &str, body: &[u8]) -> Result<Answer, String> {
        for attempt in 0..2 {
            let (challenge, _) = self.challenge()?;
            let header = sign::header(&self.key, method.name(), path, body, &challenge).map_err(|e| format!("{e:?}"))?;
            let (status, bytes) = self.exchange(method, path, &[("Content-Type", "application/json"), (sign::HEADER, &header)], body)?;
            let body = self.tree(&bytes).unwrap_or(Value::Null);
            if status == 401 && error(&body) == Some("challenge") && attempt == 0 { continue; }
            return Ok(Answer { status, body });
        }
        unreachable!()
    }

    /// A call whose answer must be one of `ok`, or the refusal in words.
    fn expect(&mut self, method: Method, path: &str, body: &[u8], ok: &[u16]) -> Result<Value, String> {
        let a = self.call(method, path, body)?;
        if ok.contains(&a.status) { Ok(a.body) } else { Err(refusal(&a)) }
    }
}

fn error(v: &Value) -> Option<&str> { v.get("error").and_then(Value::as_str) }

/// A refusal in words: the hub's detail, the limit it met, and when it ends.
fn refusal(a: &Answer) -> String {
    let Some(limit) = error(&a.body) else { return format!("the hub answered {}", a.status) };
    let mut out = format!("refused ({} {limit}): {}", a.status, s(&a.body, "detail"));
    if let Some(sec) = a.body.get("retry_after_s").and_then(Value::as_int) { out += &format!("; again in {}", words::duration(sec as u64)); }
    if let Some(n) = a.body.get("earlier").and_then(Value::as_int) { out += &format!("; see request {n}"); }
    out
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn n(v: &Value, k: &str) -> i64 { v.get(k).and_then(Value::as_int).unwrap_or(0) }
fn list<'a>(v: &'a Value, k: &str) -> &'a [Value] { v.get(k).and_then(Value::as_array).unwrap_or(&[]) }
fn flag(v: &Value, k: &str) -> bool { v.get(k).and_then(Value::as_bool) == Some(true) }

fn body(fields: &[(&str, &str)]) -> Vec<u8> {
    let mut w = json::Writer::new();
    w.begin_object().field_str("api", improve_proto::API);
    for (k, v) in fields { w.field_str(k, v); }
    w.end_object();
    w.finish().into_bytes()
}

fn hex(bytes: &[u8]) -> String { improve_proto::hex(bytes) }

fn number(text: &str) -> Result<u64, String> { text.parse().map_err(|_| format!("{text:?} is not a request's number")) }

/// The person's yes before anything leaves the machine (design 9): `--yes` for a script.
fn confirm(yes: bool) -> bool {
    if yes { return true; }
    mind::print!("send it? [y/N] ");
    let answer = mind::input::wait_key(60_000).and_then(|k| k.char());
    mind::println!("{}", answer.map(String::from).unwrap_or_default());
    matches!(answer, Some('y' | 'Y'))
}

fn run(args: &[String]) -> Result<(), String> {
    let word = |i: usize, what: &str| args.get(i).map(String::as_str).filter(|w| !w.starts_with("--")).ok_or_else(|| format!("{what} is missing\n{USAGE}"));
    let has = |f: &str| args.iter().any(|a| a == f);
    let value = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).map(String::as_str);
    match args.first().map(String::as_str) {
        Some("setup") => setup(args),
        Some("register") => {
            let mut hub = Hub::open()?;
            let joining = match value("--pair") {
                Some(code) => Joining::Pair { code: code.trim().to_ascii_uppercase() },
                None => Joining::NewUser { display_name: String::from(word(1, "your display name")?) },
            };
            let mut reg = Registration { public_key: hub.key.verifying_key().to_bytes(), name: String::from(value("--name").unwrap_or("MIND machine")), kind: DeviceKind::Mind,
                                         platform: String::from(if cfg!(target_arch = "aarch64") { "mindcore-arm64" } else { "mindcore-x64" }),
                                         version: String::from("unknown"), joining, contact: None, work: None };
            reg.check().map_err(|e| format!("the registration is out of its bounds: {e:?}"))?;
            // The proof of work over the challenge the registration is signed with.
            let (challenge, bits) = hub.challenge()?;
            reg.work = if bits > 0 { mind::println!("improve: the hub asks a proof of work of {bits} bits"); work::solve(&challenge, bits) } else { None };
            let encoded = wire::encode_registration(&reg);
            let header = sign::header(&hub.key, "POST", "/hub/v1/devices", encoded.as_bytes(), &challenge).map_err(|e| format!("{e:?}"))?;
            let (status, bytes) = hub.exchange(Method::Post, "/hub/v1/devices", &[("Content-Type", "application/json"), (sign::HEADER, &header)], encoded.as_bytes())?;
            let a = Answer { status, body: hub.tree(&bytes).unwrap_or(Value::Null) };
            if !matches!(status, 200 | 201) { return Err(refusal(&a)); }
            let state = s(&a.body, "state");
            mind::println!("registered {}: {state}{}", &s(&a.body, "device")[..16.min(s(&a.body, "device").len())],
                           if state == "pending" { " (the admin decides; improve status shows their words)" } else { "" });
            Ok(())
        }
        Some("status") => {
            let mut hub = Hub::open()?;
            let v = hub.expect(Method::Get, "/hub/v1/devices/me", b"", &[200])?;
            let u = v.get("user").unwrap_or(&Value::Null);
            let q = v.get("quotas").unwrap_or(&Value::Null);
            let band = u.get("band").and_then(Value::as_int).map(|b| format!("band {b}")).unwrap_or_else(|| String::from("read-only"));
            mind::println!("device {} ({}): {}", s(&v, "name"), &s(&v, "device")[..16.min(s(&v, "device").len())], s(&v, "state"));
            mind::println!("user {} #{}, trust {} ({band}), access {}", s(u, "display_name"), n(u, "id"), n(u, "trust"), s(&v, "access"));
            mind::println!("left now: {} new requests, {} messages", n(q, "requests"), n(q, "messages"));
            for note in list(&v, "notes") { mind::println!("  {}, {}: {}", s(note, "from"), s(note, "kind"), s(note, "text")); }
            Ok(())
        }
        Some("answer") => {
            let mut hub = Hub::open()?;
            hub.expect(Method::Post, "/hub/v1/devices/me/messages", &body(&[("text", word(1, "the answer")?)]), &[201])?;
            mind::println!("sent to the admin");
            Ok(())
        }
        Some("new") => {
            let kind = Kind::from_name(word(1, "the kind")?).ok_or("a kind is fault, improvement or capability")?;
            let security = has("--security");
            let form = Request {
                client_id: { let mut b = [0u8; 12]; mind::random::fill(&mut b); hex(&b) },
                kind, title: String::from(word(2, "the title")?), text: String::from(word(3, "the text")?),
                visibility: if has("--private") || security { Visibility::Private } else { Visibility::Public },
                security, named: has("--named"), attachments: Vec::new(),
            };
            mind::println!("a {} request, {}{}{}: {}\n{}", kind.name(), form.visibility.name(), if security { ", about security" } else { "" },
                           if form.named { ", your name on the board" } else { "" }, form.title, form.text);
            if !confirm(has("--yes")) { mind::println!("not sent"); return Ok(()); }
            let mut hub = Hub::open()?;
            let v = hub.expect(Method::Post, "/hub/v1/requests", wire::encode_request(&form).as_bytes(), &[200, 201])?;
            mind::println!("request {}: {}", n(&v, "id"), s(&v, "state"));
            Ok(())
        }
        Some("list") => {
            let mut hub = Hub::open()?;
            let v = hub.expect(Method::Get, "/hub/v1/requests", b"", &[200])?;
            let all = list(&v, "requests");
            if all.is_empty() { mind::println!("no requests"); }
            for r in all {
                let mark = if flag(r, "open_question") { "  [a question for you]" } else if !list(r, "flagged").is_empty() && !flag(r, "cleared") { "  [waits for the admin]" } else { "" };
                mind::println!("{:>5}  {:<16} {}{mark}", n(r, "id"), s(r, "state"), s(r, "title"));
            }
            Ok(())
        }
        Some("show") => {
            let id = number(word(1, "the request's number")?)?;
            let mut hub = Hub::open()?;
            let v = hub.expect(Method::Get, &format!("/hub/v1/requests/{id}"), b"", &[200])?;
            let r = v.get("request").unwrap_or(&Value::Null);
            let task = match (r.get("track").and_then(Value::as_str), r.get("task").and_then(Value::as_str)) { (Some(t), Some(k)) => format!(", {t} {k}"), _ => String::new() };
            mind::println!("#{} {}\n{} {}, {}{task}\n\n{}\n", n(r, "id"), s(r, "title"), s(r, "kind"), s(r, "visibility"), s(r, "state"), s(r, "text"));
            for p in list(r, "thread") {
                let extra = match s(p, "kind") {
                    "answer" => format!(" to #{}", n(p, "answers")),
                    "verdict" => format!(": {} on {}", s(p, "verdict"), s(p, "release")),
                    "question" if p.get("answered").and_then(Value::as_bool) == Some(false) => String::from(" (waits for your answer: improve reply N \"TEXT\" --answer #)"),
                    _ => String::new(),
                };
                mind::println!("#{} {}{extra}, {}\n  {}", n(p, "n"), s(p, "kind"), s(p, "author"), s(p, "text"));
            }
            Ok(())
        }
        Some("reply") => {
            let id = number(word(1, "the request's number")?)?;
            let text = String::from(word(2, "the text")?);
            let message = match value("--answer") {
                Some(q) => Message { kind: MessageKind::Answer, text, answers: Some(q.trim_start_matches('#').parse().map_err(|_| "--answer takes the question's number")?), verdict: None, release: None, attachments: Vec::new() },
                None => Message { kind: MessageKind::Comment, text, answers: None, verdict: None, release: None, attachments: Vec::new() },
            };
            post(id, message)
        }
        Some("check") => {
            let id = number(word(1, "the request's number")?)?;
            let verdict = Verdict::from_name(word(2, "the verdict")?).ok_or("a verdict is works, partly or does-not-work")?;
            post(id, Message { kind: MessageKind::Verdict, text: String::from(word(4, "what you saw")?), answers: None, verdict: Some(verdict),
                               release: Some(String::from(word(3, "the release you checked on")?)), attachments: Vec::new() })
        }
        Some("version") => {
            let mut hub = Hub::open()?;
            hub.expect(Method::Post, "/hub/v1/devices/me/version", &body(&[("version", word(1, "the release")?)]), &[200])?;
            mind::println!("the hub knows this machine runs {}", word(1, "the release")?);
            Ok(())
        }
        Some("devices") => {
            let mut hub = Hub::open()?;
            let v = hub.expect(Method::Get, "/hub/v1/devices", b"", &[200])?;
            for d in list(&v, "devices") {
                mind::println!("{}  {:<9} {} {} {}{}", &s(d, "device")[..16.min(s(d, "device").len())], s(d, "state"), s(d, "kind"), s(d, "name"), s(d, "platform"),
                               if flag(d, "this") { "  (this one)" } else { "" });
            }
            Ok(())
        }
        Some("pair") => {
            let mut hub = Hub::open()?;
            let v = hub.expect(Method::Post, "/hub/v1/devices/pair", b"", &[201])?;
            mind::println!("pairing code {}: another device registers with --pair, or a browser signs in at /hub/ (10 minutes, once)", s(&v, "code"));
            Ok(())
        }
        Some("revoke") => {
            let prefix = word(1, "the device")?.to_ascii_lowercase();
            let mut hub = Hub::open()?;
            let v = hub.expect(Method::Get, "/hub/v1/devices", b"", &[200])?;
            let found: Vec<String> = list(&v, "devices").iter().map(|d| String::from(s(d, "device"))).filter(|id| id.starts_with(&prefix)).collect();
            let [id] = found.as_slice() else { return Err(format!("{} devices start with {prefix:?}: name one (improve devices)", found.len())) };
            hub.expect(Method::Delete, &format!("/hub/v1/devices/{id}"), b"", &[200])?;
            mind::println!("revoked {}", &id[..16]);
            Ok(())
        }
        Some("delete") => {
            let id = number(word(1, "the request's number")?)?;
            let mut hub = Hub::open()?;
            hub.expect(Method::Delete, &format!("/hub/v1/requests/{id}"), b"", &[200])?;
            mind::println!("request {id} deleted, with its thread");
            Ok(())
        }
        _ => Err(String::from(USAGE)),
    }
}

fn post(id: u64, message: Message) -> Result<(), String> {
    message.check().map_err(|e| format!("the message is out of its bounds: {e:?}"))?;
    let mut hub = Hub::open()?;
    let v = hub.expect(Method::Post, &format!("/hub/v1/requests/{id}/messages"), wire::encode_message(&message).as_bytes(), &[201])?;
    mind::println!("message #{} on request {id}; the request is {}", n(&v, "n"), s(&v, "state"));
    Ok(())
}

/// The hub's address and pins into the program's own directory, and its key, made once, into keystore.
fn setup(args: &[String]) -> Result<(), String> {
    let [_, name, address, port, pins @ ..] = args else { return Err(String::from(USAGE)) };
    let (name, address, port) = words::hub_line(name, address, port).map_err(String::from)?;
    let pins: Vec<[u8; 32]> = pins.iter().map(|p| words::hex32(p).ok_or("a pin is the hub key's SPKI SHA-256, 64 lower-case hex")).collect::<Result<_, _>>()?;
    if pins.is_empty() || pins.len() > 2 { return Err("one or two pins: the hub's key, as it printed it at its start".into()); }
    let config = Config { name, address, port, pins };
    if !mind::fs::own::given() { return Err("no directory of its own: run improve by its name".into()); }
    mind::fs::own::write(CONFIG, words::config_text(&config).as_bytes()).map_err(|e| format!("hub.conf cannot be written: {e:?}"))?;
    let key = match read_key() {
        Ok(key) => key,
        Err(_) => {
            let mut seed = [0u8; 32];
            if !mind::random::fill(&mut seed) { return Err("no random bytes for a key".into()); }
            match keystore::secret_put(SECRETS, KEY_SECRET, &seed) {
                Ok(Ok(())) => SigningKey::from_bytes(&seed),
                other => return Err(format!("keystore refused the key: {:?}", other.map_err(|_| ()))),
            }
        }
    };
    mind::println!("hub {}:{}\nthis device's key {}", config.name, config.port, hex(key.verifying_key().as_bytes()));
    Ok(())
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("improve — MIND Hub's client: requests to the agents who build MIND Core, their questions and your check.\nUsage: improve setup | register | status | answer | new | list | show N | reply N | check N | version | devices | pair | revoke | delete N\n(improve with no command names each one's words)");
    let args = match words::split(mind::process::args_str()) {
        Ok(a) => a,
        Err(why) => { mind::println!("improve: {why}"); mind::process::exit_with(1) }
    };
    if let Err(why) = run(&args) {
        mind::println!("improve: {why}");
        mind::process::exit_with(1);
    }
}
