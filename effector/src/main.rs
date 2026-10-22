#![no_std]
#![no_main]
// effector CONFIG [--for SECONDS]: the Effector agent's session (750-EFF-0006, first slice), started from the shell.
// CONFIG is the owner's `effector.conf`; its directory, the only one the shell lends (REQUEST_FILE), also holds
// `token`, `agent_id` and `journal`. As a boot service in a mode, `init` starts it with `--service` (750-KRN-0119,
// 0143): it reads `data/effector/effector.conf` and the owner's token through SLOT_FILE and keeps the token,
// `agent_id` and `journal` in its private `system/effector` (SLOT_VFS, `mind::effector`). It heartbeats, polls and
// answers every command `denied` after journaling the answer, over one HTTPS connection to the pinned server. Heads
// and JSON bodies are read by the parser service (MC-11.11); the token is never printed. One line per event.
extern crate alloc;

use alloc::{boxed::Box, string::String, vec::Vec};
use effector::session::{self, Accepted, Outcome, Session, Time};
use effector::{config, journal, wire};
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_FILE, SLOT_NETWORK, SLOT_PARSE, SLOT_TLS, SLOT_VFS};
use mind::effector::{OWNER_DIR, SESSION_DIR, SLOT_GATEWAY};
use mind::idl::codec::Text;
use mind::idl::effector as gateway;
use mind::fs::{self, File, MODE_CREATE, MODE_WRITE};
use mind::http::{self, Connection, Method, Transport, Url};
use mind::idl::{parse, tls};
use mind::ipc::Endpoint;

mind::request!(REQUEST_CONSOLE | REQUEST_NETWORK | REQUEST_PARSE | REQUEST_TLS | REQUEST_FILE);

const TLS: Endpoint = Endpoint(SLOT_TLS);
const PARSER: Endpoint = Endpoint(SLOT_PARSE);
const GATEWAY: Endpoint = Endpoint(SLOT_GATEWAY);
const ACTION_MS: u64 = 120_000; // a gateway request's deadline, from when it is handed over
const BODY_MAX: usize = 16384;
const REQUEST_MS: usize = 3000;
const CONNECT_MS: u32 = 10_000;
const RECONNECT_MAX_MS: usize = 30_000;

fn now() -> usize { mind::time::uptime_ms() }
fn time() -> Time { Time { monotonic_ms: now() as u64, unix_seconds: mind::rtc::unix_time() } }
fn jitter() -> u32 { mind::random::u64().unwrap_or(0) as u32 }

fn fail(what: core::fmt::Arguments) -> ! {
    mind::println!("EFFECTOR: {}", what);
    mind::process::exit_with(1)
}

// A TLS session of the flow grant; a receive waits up to `idle_ms` for a byte, so every request is bounded.
struct Link { session: u32, idle_ms: usize }
impl Transport for Link {
    fn send(&mut self, data: &[u8]) -> Result<(), http::Error> {
        let (mut sent, deadline) = (0, now() + REQUEST_MS);
        while sent < data.len() {
            let end = data.len().min(sent + 4096);
            let n = match tls::send(TLS, self.session, &data[sent..end]) {
                Ok(Ok(n)) => n as usize, Ok(Err(tls::Error::Again)) => 0, _ => return Err(http::Error::Transport),
            };
            if n == 0 { if now() >= deadline { return Err(http::Error::Transport); } mind::time::sleep(2); }
            sent += n;
        }
        Ok(())
    }
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, http::Error> {
        let deadline = now() + self.idle_ms;
        let length = buffer.len().min(4096);
        loop {
            match tls::receive(TLS, self.session, length as u32, &mut buffer[..length]) {
                Ok(Ok(n)) if n > 0 => return Ok(n as usize),
                Ok(Ok(_)) | Ok(Err(tls::Error::Again)) => {}
                Ok(Err(tls::Error::Closed)) => return Ok(0),
                _ => return Err(http::Error::Transport),
            }
            if now() >= deadline { return Err(http::Error::Transport); }
            mind::time::sleep(2);
        }
    }
}
impl Drop for Link { fn drop(&mut self) { let _ = tls::close(TLS, self.session); } }

/// A file the gateway opened, read in its pieces as an upload's body.
struct Pieces { handle: u32, offset: u64, size: u64, piece: Vec<u8>, at: usize, end: usize, failed: Option<session::Failure> }
impl http::Source for Pieces {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, http::Error> {
        if self.at == self.end {
            if self.offset >= self.size { return Ok(0); }
            match gateway::read(GATEWAY, self.handle, self.offset, &mut self.piece) {
                Ok(Ok(0)) | Err(_) => { self.failed = Some(session::Failure::Io); return Err(http::Error::Transport); }
                Ok(Ok(n)) => { self.at = 0; self.end = n; self.offset += n as u64; }
                Ok(Err(e)) => { self.failed = Some(failure(e)); return Err(http::Error::Transport); }
            }
        }
        let n = buffer.len().min(self.end - self.at);
        buffer[..n].copy_from_slice(&self.piece[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

fn failure(e: gateway::Error) -> session::Failure {
    use session::Failure as F;
    match e {
        gateway::Error::Denied => F::Denied, gateway::Error::NoPolicy => F::NoPolicy, gateway::Error::NotFound => F::NotFound,
        gateway::Error::Bounds => F::Bounds, gateway::Error::Digest => F::Digest, gateway::Error::Busy => F::Busy,
        gateway::Error::Expired => F::Expired, gateway::Error::Audit => F::Audit, gateway::Error::Malformed => F::Malformed,
        gateway::Error::Unsupported => F::Unsupported, _ => F::Io,
    }
}

/// The command's origin as the gateway audits it; a field cut to fit says so.
fn origin(id: &str, initiator: &str, reason: &str) -> gateway::Origin {
    fn cut(s: &str, n: usize) -> &str { let mut e = s.len().min(n); while !s.is_char_boundary(e) { e -= 1; } &s[..e] }
    gateway::Origin {
        command: Text::new(cut(id, 64)).unwrap_or_default(), initiator: Text::new(cut(initiator, 128)).unwrap_or_default(),
        reason: Text::new(cut(reason, 512)).unwrap_or_default(), truncated: initiator.len() > 128 || reason.len() > 512,
        deadline_ms: now() as u64 + ACTION_MS,
    }
}

struct Agent {
    config: config::Config,
    token: String,
    agent_id: String,
    dir: String,
    connection: Option<Box<Connection<Link>>>,
    reconnect_at: usize,
    reconnect_ms: usize,
    body: Vec<u8>,
    activation_noted: bool,
}

impl Agent {
    #[inline(never)]
    fn connect(&mut self) -> Option<&mut Connection<Link>> {
        if self.connection.is_some() { return self.connection.as_deref_mut(); }
        if now() < self.reconnect_at { return None; }
        let mut pins = mind::idl::codec::List::<tls::Pin, 2>::default();
        for p in self.config.pins() {
            let mut digest = mind::idl::codec::List::default();
            for b in p.digest { digest.push(b); }
            pins.push(tls::Pin { kind: match p.kind { config::PinKind::Leaf => tls::PinKind::Leaf, config::PinKind::Spki => tls::PinKind::Spki }, digest });
        }
        let address = self.config.address().unwrap_or(0);
        let attempt = tls::attach(TLS, SLOT_NETWORK).ok().and_then(|r| r.ok()).ok_or("attach").and_then(|session| {
            match tls::connect_pins(TLS, session, self.config.tls_name(), address, self.config.port(), pins.as_slice(), CONNECT_MS) {
                Ok(Ok(_)) => Ok(session),
                other => { let _ = tls::close(TLS, session); mind::println!("EFFECTOR: TLS CONNECT REFUSED: {:?}", other.map_err(|_| ())); Err("connect") }
            }
        });
        match attempt {
            Ok(session) => {
                mind::println!("EFFECTOR: CONNECTED TO {} (PINNED)", self.config.host());
                self.reconnect_ms = 1000;
                self.connection = Some(Box::new(Connection::new(Link { session, idle_ms: REQUEST_MS })));
                self.connection.as_deref_mut()
            }
            Err(_) => {
                self.reconnect_at = now() + self.reconnect_ms;
                self.reconnect_ms = (self.reconnect_ms * 2).min(RECONNECT_MAX_MS);
                None
            }
        }
    }

    /// One request; the body read into `self.body`. A transport or framing failure drops the connection.
    #[inline(never)]
    fn request(&mut self, method: Method, route: &str, query: &str, body: Option<&[u8]>) -> Option<u16> {
        let mut source = body.unwrap_or(&[]);
        let length = body.map(|b| b.len() as u64);
        self.send(method, route, query, "application/json", length.map(|n| (n, &mut source as &mut dyn http::Source)))
    }

    /// One request, its body (if any) read from `body` up to its announced length; the answer's body is in `self.body`.
    fn send(&mut self, method: Method, route: &str, query: &str, content_type: &str, body: Option<(u64, &mut dyn http::Source)>) -> Option<u16> {
        let mut text = String::new();
        text.push_str("https://"); text.push_str(self.config.host());
        if self.config.port() != 443 { let _ = core::fmt::write(&mut text, format_args!(":{}", self.config.port())); }
        text.push_str(self.config.base_path()); text.push_str(route); text.push_str(query);
        let mut authorization = String::from("Bearer ");
        authorization.push_str(&self.token);
        let send_token = !self.token.is_empty();
        let mut buffer = core::mem::take(&mut self.body);
        buffer.clear();
        let result = (|| {
            let url = Url::parse(&text).ok()?;
            let connection = self.connect()?;
            let mut parser = mind::parse::Service(PARSER);
            // `token none`: the server asks no agent token, so none is sent.
            let all = [("Content-Type", content_type), ("Authorization", authorization.as_str())];
            let headers = if send_token { &all[..] } else { &all[..1] };
            let sent = connection.request(method, &url, headers, body, &mut parser, false);
            let mut response = match sent { Ok(r) => r, Err(e) => { mind::println!("EFFECTOR: {} {}: {:?}", match method { Method::Get => "GET", Method::Post => "POST", Method::Put => "PUT" }, route, e); return None; } };
            let status = response.status;
            let mut chunk = [0u8; 2048];
            loop {
                match response.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) if buffer.len() + n <= BODY_MAX => buffer.extend_from_slice(&chunk[..n]),
                    Ok(_) => { mind::println!("EFFECTOR: {} ANSWER OVER {} BYTES", route, BODY_MAX); return None; }
                    Err(e) => { mind::println!("EFFECTOR: {} BODY: {:?}", route, e); return None; }
                }
            }
            Some(status)
        })();
        self.body = buffer;
        if result.is_none() { self.connection = None; }
        if self.connection.as_ref().is_some_and(|c| !c.open()) { self.connection = None; }
        result
    }

    /// The body as `parse.json` reads it, checked by `check`.
    #[inline(never)]
    fn answer<T>(&self, check: impl FnOnce(&[wire::Value<'_>]) -> Result<T, wire::Error>) -> Result<T, wire::Error> {
        self.parsed(&self.body, check)
    }

    /// The values `parse.json` reads in `text` (an answer's body, or a command's payload), checked by `check`.
    #[inline(never)]
    fn parsed<T>(&self, text: &[u8], check: impl FnOnce(&[wire::Value<'_>]) -> Result<T, wire::Error>) -> Result<T, wire::Error> {
        let mut owned: Vec<(wire::Kind, Option<u16>, String, String)> = Vec::new();
        let mut start = 0u32;
        loop {
            let (total, got) = self.page(text, start, &mut owned)?;
            start += got;
            if start >= total || got == 0 { break; }
        }
        let values: Vec<wire::Value<'_>> = owned.iter().map(|(k, p, key, text)| wire::Value { kind: *k, parent: *p, key, text }).collect();
        check(&values)
    }

    // One page of `parse.json`'s answer into `owned`: the page lives in this frame only (the stack is 64 KiB).
    #[inline(never)]
    fn page(&self, text: &[u8], start: u32, owned: &mut Vec<(wire::Kind, Option<u16>, String, String)>) -> Result<(u32, u32), wire::Error> {
        let result = parse::json(PARSER, text, start);
        let Ok(Ok(page)) = &result else { return Err(wire::Error::Shape) };
        if page.values as usize > wire::VALUES_MAX { return Err(wire::Error::Bounds); }
        for v in page.page.as_slice() {
            let kind = match v.kind {
                parse::JsonKind::Object => wire::Kind::Object, parse::JsonKind::Array => wire::Kind::Array, parse::JsonKind::String => wire::Kind::String,
                parse::JsonKind::Number => wire::Kind::Number, parse::JsonKind::True => wire::Kind::True, parse::JsonKind::False => wire::Kind::False,
                parse::JsonKind::Null => wire::Kind::Null,
            };
            owned.push((kind, (v.parent != u16::MAX).then_some(v.parent), String::from(v.key.as_str()), String::from(v.text.as_str())));
        }
        Ok((page.values, page.page.as_slice().len() as u32))
    }

    #[inline(never)]
    fn heartbeat(&mut self, stopping: bool) -> Outcome {
        let mut buf = [0u8; 1024];
        let mut stamp = [0u8; 20];
        let agent_time = mind::rtc::unix_time().map(|t| wire::rfc3339_utc(t, &mut stamp));
        let h = wire::Heartbeat { agent_id: &self.agent_id, hostname: self.config.hostname(), arch: wire::arch(), agent_version: env!("CARGO_PKG_VERSION"), stopping, agent_time };
        let Ok(body) = wire::heartbeat(&mut buf, &h) else { return Outcome::Permanent };
        let body: Vec<u8> = body.to_vec();
        let Some(status) = self.request(Method::Post, "/api/heartbeat", "", Some(&body)) else { return Outcome::Retry };
        let ok = status == 200 && self.answer(wire::ok_answer).is_ok();
        if !ok { mind::println!("EFFECTOR: HEARTBEAT ANSWERED {}", status); }
        session::outcome(status, ok)
    }

    #[inline(never)]
    fn ack(&mut self, ack: &session::Ack) -> Outcome {
        let mut buf = [0u8; 1024];
        let Ok(body) = wire::ack(&mut buf, ack.record.id.as_str(), ack.record.result.result_json()) else { return Outcome::Permanent };
        let body: Vec<u8> = body.to_vec();
        let Some(status) = self.request(Method::Post, "/api/commands/ack", "", Some(&body)) else { return Outcome::Retry };
        let ok = status == 200 && self.answer(wire::ok_answer).is_ok();
        mind::println!("EFFECTOR: ACK {} {} {}", ack.record.id.as_str(), ack.record.result.status(), if ok { "ACCEPTED" } else { "NOT ACCEPTED" });
        session::outcome(status, ok)
    }

    #[inline(never)]
    fn append_journal(&self, record: &session::Record) -> bool {
        let path = self.path("journal");
        let Ok(mut file) = File::open_mode(&path, MODE_WRITE | MODE_CREATE) else { return false };
        if file.size() == 0 && file.write_at(0, journal::HEADER.as_bytes()).is_err() { return false; }
        let mut line = [0u8; journal::LINE_MAX];
        let n = journal::line(record, &mut line);
        let at = file.size();
        file.write_at(at, &line[..n]).map(|w| w == n).unwrap_or(false) && file.flush().is_ok()
    }


    /// One poll: each command is journaled with its answer before the answer is sent (MC-6.6).
    #[inline(never)]
    fn poll(&mut self, session: &mut Session) -> (Outcome, bool) {
        let mut took_command = false;
        // While an answer waits for its ACK the agent takes no new command: the server delivers each command once, so
        // `at_capacity=1` asks it to hold them (contract 1.0) rather than hand over one that would be refused.
        let query = alloc::format!("?agent_id={}{}", self.agent_id, if session.busy() { "&at_capacity=1" } else { "" });
        let outcome = match self.request(Method::Get, "/api/commands/poll", &query, None) {
            None => Outcome::Retry,
            Some(200) => match self.answer(|v| wire::commands(v).map(|c| (c.count, c.activation_required, c.list.map(|d| d.map(|d| (String::from(d.command.id), String::from(d.command.agent_id), String::from(d.command.action), String::from(d.command.program), String::from(d.command.payload), d.command.expires_unix, String::from(d.initiator), String::from(d.reason))))))) {
                Ok((count, activation, list)) => {
                    if activation && !self.activation_noted { mind::println!("EFFECTOR: THE SERVER WAITS FOR THE OWNER TO ACTIVATE THIS AGENT"); self.activation_noted = true; }
                    took_command = count > 0;
                    for (id, agent_id, action, program, payload, expires_unix, initiator, reason) in list.iter().take(count).flatten() {
                        let command = session::Command { id, agent_id, action, program, payload, expires_unix: *expires_unix };
                        // An action the gateway serves runs once, for a command not saved before; any other is denied.
                        let gateway_ok = mind::dev::cap_info(SLOT_GATEWAY).0 == CAP_KIND_ENDPOINT;
                        let run = action == "collect_file" && gateway_ok && !session.busy() && session.fresh(&command) == Ok(true);
                        let received = if run {
                            let done = self.collect_file(id, payload, initiator, reason);
                            session.receive_with(command, time(), done)
                        } else { session.receive(command, time()) };
                        match received {
                            Ok(Accepted::Store(store)) => {
                                let durable = self.append_journal(&store.record);
                                let result = store.record.result;
                                let why = if let session::Final::Failed(f) = result { f.name() } else { "" };
                                mind::println!("EFFECTOR: {} {} {}{}{}{}", result.status().to_ascii_uppercase(), id, action, if why.is_empty() { "" } else { ": " }, why, if durable { "" } else { " (JOURNAL WRITE FAILED)" });
                                let _ = session.stored(store, durable);
                            }
                            Ok(Accepted::Replay) => mind::println!("EFFECTOR: REPEATED {}: THE SAVED ANSWER AGAIN", id),
                            Ok(Accepted::InProgress) => {}
                            Err(e) => mind::println!("EFFECTOR: COMMAND {} REFUSED: {:?}", id, e),
                        }
                    }
                    Outcome::Accepted
                }
                Err(e) => { mind::println!("EFFECTOR: POLL ANSWER REFUSED: {:?}", e); Outcome::Retry }
            },
            Some(status) => { mind::println!("EFFECTOR: POLL ANSWERED {}", status); session::outcome(status, false) }
        };
        (outcome, took_command)
    }

    /// `collect_file` through the gateway (750-EFF-0007): opened there (the owner's policy decides, the gateway audits),
    /// then its bytes uploaded with their SHA-256 as the server's `/api/commands/file` takes them.
    #[inline(never)]
    fn collect_file(&mut self, id: &str, payload: &str, initiator: &str, reason: &str) -> session::Final {
        use session::{Failure, Final};
        let Ok((path, max)) = self.parsed(payload.as_bytes(), |v| wire::collect(v).map(|c| (String::from(c.path), c.max_bytes))) else {
            return Final::Failed(Failure::Malformed);
        };
        let opened = match gateway::open_file(GATEWAY, &origin(id, initiator, reason), &path) {
            Ok(Ok(t)) => t,
            Ok(Err(e)) => return Final::Failed(failure(e)),
            Err(_) => return Final::Failed(Failure::Unsupported),
        };
        let result = if opened.truncated || max.is_some_and(|m| opened.size > m) { Final::Failed(Failure::Bounds) } else {
            let query = alloc::format!("?agent_id={}&command_id={}&sha256={}", self.agent_id, id, opened.sha256.as_str());
            let mut body = Pieces { handle: opened.handle, offset: 0, size: opened.size, piece: alloc::vec![0; 32768], at: 0, end: 0, failed: None };
            let status = self.send(Method::Post, "/api/commands/file", &query, "application/octet-stream", Some((opened.size, &mut body)));
            match (status, body.failed) {
                (_, Some(f)) => Final::Failed(f),
                (Some(201), None) => Final::Completed,
                _ => Final::Failed(Failure::Upload),
            }
        };
        let _ = gateway::close(GATEWAY, opened.handle);
        result
    }

    fn path(&self, name: &str) -> String { let mut p = self.dir.clone(); p.push('/'); p.push_str(name); p }
}

fn read_file(path: &str, max: usize) -> Option<Vec<u8>> {
    let file = File::open(path).ok()?;
    if file.size() > max { return None; }
    let mut out = alloc::vec![0u8; file.size()];
    let mut at = 0;
    while at < out.len() {
        let n = file.read_at(at, &mut out[at..]).ok()?;
        if n == 0 { return None; }
        at += n;
    }
    Some(out)
}

#[inline(never)]
fn recover(agent: &Agent) -> Vec<session::Record> {
    let Some(bytes) = read_file(&agent.path("journal"), 64 * journal::LINE_MAX + 64) else { return Vec::new() };
    let result = journal::read(&bytes, &agent.agent_id);
    match &result {
        Ok((records, n)) => records[..*n].iter().flatten().copied().collect(),
        Err(e) => fail(format_args!("JOURNAL REFUSED: {:?}", e)),
    }
}

#[inline(never)]
fn start() -> (Box<Agent>, Box<Session>, Option<usize>) {
    let mut words = mind::process::args_str().split_whitespace();
    // Started by init in a mode with `--service` (750-KRN-0119): the owner's directory and its own private one.
    let service_conf = alloc::format!("{}/effector.conf", OWNER_DIR);
    let (service, conf) = match words.next() {
        Some("--service") => (true, service_conf.as_str()),
        Some(c) => (false, c),
        None => fail(format_args!("USAGE: effector CONFIG [--for SECONDS]")),
    };
    if service && mind::dev::cap_info(SLOT_VFS).0 != CAP_KIND_ENDPOINT { fail(format_args!("NO PRIVATE DIRECTORY ({})", SESSION_DIR)); }
    let mut run_for = None;
    while let Some(w) = words.next() {
        match w {
            "--for" => run_for = words.next().and_then(|s| s.parse::<usize>().ok()).filter(|&s| s > 0),
            _ => fail(format_args!("UNKNOWN OPTION {}", w)),
        }
    }
    for (slot, what) in [(SLOT_NETWORK, "NETWORK GRANT"), (SLOT_TLS, "TLS CLIENT"), (SLOT_PARSE, "PARSER SERVICE"), (SLOT_FILE, "FILE CLIENT")] {
        if mind::dev::cap_info(slot).0 != CAP_KIND_ENDPOINT { fail(format_args!("NO {}", what)); }
    }
    let (volume, rest) = fs::split(conf);
    let dir_rest = rest.rfind('/').map_or("", |i| &rest[..i]);
    let mut dir = String::new();
    if !volume.is_empty() { dir.push_str(volume); dir.push(':'); }
    dir.push_str(dir_rest);
    fs::use_scope(Endpoint(SLOT_FILE), &dir);
    // The owner's token, read while the owner's directory is in reach; a service keeps it in its private directory.
    let owner_token = read_file(&alloc::format!("{}/token", dir), config::TOKEN_MAX + 2);

    let config = match read_file(conf, config::FILE_MAX).map(|b| config::Config::parse(&b)) {
        Some(Ok(c)) => c,
        Some(Err(e)) => fail(format_args!("CONFIGURATION REFUSED AT LINE {}: {:?}", e.line, e.kind)),
        None => fail(format_args!("CANNOT READ {}", conf)),
    };
    let dir = if service { fs::use_scope(Endpoint(SLOT_VFS), ""); String::from(SESSION_DIR) } else { dir };
    let mut agent = Box::new(Agent { config, token: String::new(), agent_id: String::new(), dir, connection: None, reconnect_at: 0, reconnect_ms: 1000, body: Vec::new(), activation_noted: false });
    let kept = if service && !agent.config.no_token() { read_file(&agent.path("token"), config::TOKEN_MAX + 2) } else { None };
    match kept.as_deref().and_then(config::token).or_else(|| owner_token.as_deref().and_then(config::token)) {
        _ if agent.config.no_token() => mind::println!("EFFECTOR: NO AGENT TOKEN (token none): THE SERVER ASKS NONE"),
        Some(t) => agent.token = String::from(t),
        None => fail(format_args!("NO VALID TOKEN FILE BESIDE THE CONFIGURATION")),
    }
    if service && !agent.config.no_token() && kept.as_deref().and_then(config::token).is_none() {
        let written = File::open_mode(&agent.path("token"), MODE_WRITE | MODE_CREATE | fs::MODE_TRUNCATE).and_then(|mut f| { f.write_at(0, agent.token.as_bytes())?; f.write_at(agent.token.len(), b"\n")?; f.flush() });
        if written.is_err() { fail(format_args!("CANNOT KEEP THE TOKEN IN {}", SESSION_DIR)); }
        mind::println!("EFFECTOR: THE TOKEN IS KEPT IN {}; THE OWNER MAY REMOVE {}/token", SESSION_DIR, OWNER_DIR);
    }
    let id_path = agent.path("agent_id");
    match read_file(&id_path, 80) {
        Some(bytes) => match config::agent_id(&bytes) { Some(id) => agent.agent_id = String::from(id), None => fail(format_args!("agent_id IS MALFORMED")) },
        None => {
            let Some(random) = mind::random::u64() else { fail(format_args!("NO RANDOM SOURCE FOR THE AGENT ID")) };
            let mut out = [0u8; 21];
            let id = config::new_agent_id(random, &mut out);
            let written = File::open_mode(&id_path, MODE_WRITE | MODE_CREATE).and_then(|mut f| { f.write_at(0, id.as_bytes())?; f.write_at(id.len(), b"\n")?; f.flush() });
            if written.is_err() { fail(format_args!("CANNOT WRITE {}", id_path)); }
            agent.agent_id = String::from(id);
            mind::println!("EFFECTOR: NEW AGENT ID {}", id);
        }
    }
    let records = recover(&agent);
    let count = records.len();
    let session = match Session::new(&agent.agent_id, &records, now() as u64) { Ok(s) => Box::new(s), Err(e) => fail(format_args!("SESSION: {:?}", e)) };
    mind::println!("EFFECTOR: AGENT {} TO {}{} ({} SAVED RESULTS)", agent.agent_id, agent.config.host(), agent.config.base_path(), count);

    (agent, session, run_for)
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("effector — the Effector agent's session (750-EFF-0006): heartbeat, poll and denied answers to a pinned Effector server.\nUsage: effector data/effector/effector.conf [--for SECONDS]");
    let (mut agent, mut session, run_for) = start();
    let started = now();
    let mut stopped_at = None;
    loop {
        let t = time();
        let ms = t.monotonic_ms;
        if let (Some(limit), None) = (run_for, stopped_at) {
            if now() >= started + limit * 1000 { session.stop(ms); stopped_at = Some(now()); mind::println!("EFFECTOR: STOPPING"); }
        }
        if session.shutdown_done() || stopped_at.is_some_and(|s| now() > s + 10_000) { break; }
        match session.heartbeat(ms) {
            Ok(Some(attempt)) => {
                let outcome = agent.heartbeat(session.stopping());
                let _ = session.heartbeat_done(attempt, outcome, now() as u64, jitter());
            }
            Ok(None) => {}
            Err(e) => fail(format_args!("HEARTBEAT SCHEDULE: {:?}", e)),
        }
        match session.next_ack(time()) {
            Ok(Some(ack)) => {
                let outcome = agent.ack(&ack);
                let _ = session.ack_done(ack, outcome, now() as u64, jitter());
                // The agent is free again: ask for the next command at once.
                if outcome == Outcome::Accepted && !session.busy() { session.wake(now() as u64); }
            }
            Ok(None) => {}
            Err(e) => mind::println!("EFFECTOR: ACK SCHEDULE: {:?}", e),
        }
        match session.poll(now() as u64) {
            Ok(Some(attempt)) => {
                let (outcome, took_command) = agent.poll(&mut session);
                let _ = session.poll_done(attempt, outcome, now() as u64, jitter());
                // The server hands one command a poll: while it has more, poll again at once (as after a wake-up).
                if took_command { session.wake(now() as u64); }
            }
            Ok(None) => {}
            Err(e) => mind::println!("EFFECTOR: POLL SCHEDULE: {:?}", e),
        }
        mind::time::sleep(20);
    }
    mind::println!("EFFECTOR: STOPPED{}", if session.shutdown_done() { "" } else { " WITHOUT A SHUTDOWN HEARTBEAT" });
}
