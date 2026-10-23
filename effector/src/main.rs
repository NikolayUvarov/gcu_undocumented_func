#![no_std]
#![no_main]
// effector CONFIG [--for SECONDS]: the Effector agent's session (750-EFF-0006, first slice), started from the shell.
// CONFIG is the owner's `effector.conf`; its directory, the only one the shell lends (REQUEST_FILE), also holds
// `token`, `agent_id` and `journal`. As a boot service in a mode, `init` starts it with `--service` (750-KRN-0119,
// 0143): it reads `data/effector/effector.conf` and the owner's token through SLOT_FILE and keeps the token,
// `agent_id` and `journal` in its private `system/effector` (SLOT_VFS, `mind::effector`). It heartbeats, polls, hands
// `collect_file`, `deliver_file`, `collect_logs`, `screenshot` and the service actions to `effector_gw` (whose owner's
// policy decides each, 750-EFF-0007), answers any other command `denied`, journals each answer before its ACK, and
// sends the gateway's counters as telemetry every 30 s (750-EFF-0014), over one HTTPS connection to the pinned server.
// Heads and JSON bodies are read by the parser service (MC-11.11); the token is never printed. One line per event.
extern crate alloc;

use alloc::{boxed::Box, string::String, vec::Vec};
use effector::session::{self, Accepted, Artifact, Outcome, Session, Time};
use effector::{config, journal, telemetry, wire};
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_FILE, SLOT_NETWORK, SLOT_PARSE, SLOT_TLS, SLOT_VFS};
use mind::effector::{OWNER_DIR, SESSION_DIR, SLOT_GATEWAY};
use mind::idl::codec::Text;
use mind::idl::effector as gateway;
use mind::fs::{self, File, MODE_CREATE, MODE_WRITE};
use mind::http::{self, Connection, Method, Transport, Url};
use mind::idl::{parse, socket, tls};
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
const PIECE: usize = 32768; // the gateway's transfers move this much a call
const RAW: usize = 3 * 4096; // an artifact is encoded this much at a time (a multiple of 3: base64 without padding)
const INLINE_MAX: u64 = 8 << 20; // an artifact's bytes, so its ACK as base64 stays below the server's 12 MiB
const JOURNAL_MAX: usize = 8 * session::RECORDS_MAX * journal::LINE_MAX; // compacted at each start, so far above its need

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

/// An answer's ACK with its artifact (`system/effector/artifact`, after its header line) as base64: `wire::artifact_ack`'s
/// text, the data encoded a piece at a time, then `wire::ARTIFACT_SUFFIX`.
struct Inline { file: File, at: usize, end: usize, raw: Vec<u8>, text: Vec<u8>, sent: usize, done: bool }
impl http::Source for Inline {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, http::Error> {
        while self.sent == self.text.len() {
            if self.done { return Ok(0); }
            self.text.clear();
            self.sent = 0;
            if self.at < self.end {
                let want = (self.end - self.at).min(RAW);
                let n = self.file.read_at(self.at, &mut self.raw[..want]).map_err(|_| http::Error::Transport)?;
                if n != want { return Err(http::Error::Transport); }
                self.text.resize(4 * RAW.div_ceil(3), 0);
                let written = wire::base64(&self.raw[..n], &mut self.text);
                self.text.truncate(written);
                self.at += n;
            } else {
                self.text.extend_from_slice(wire::ARTIFACT_SUFFIX);
                self.done = true;
            }
        }
        let n = buffer.len().min(self.text.len() - self.sent);
        buffer[..n].copy_from_slice(&self.text[self.sent..self.sent + n]);
        self.sent += n;
        Ok(n)
    }
}

/// What the artifact file's first line says: `<command> <size> <sha256> <truncated 0|1> <name>`.
struct Kept { command: String, size: usize, sha256: String, truncated: bool, name: String, start: usize }

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
    telemetry_at: usize,
    last: Option<telemetry::Counters>,
    telemetry_said: Option<u16>,
    executed: u64,
    executed_total: u64,
    cpu_model: String,
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
        self.send(method, route, query, "application/json", length.map(|n| (n, &mut source as &mut dyn http::Source)), None)
    }

    /// One request, its body (if any) read from `body` up to its announced length; the answer's body goes to `sink`
    /// (false stops it) or, without one, into `self.body`. Every request names the agent (`X-Effector-Agent-ID`), by
    /// which the server counts its traffic.
    fn send(&mut self, method: Method, route: &str, query: &str, content_type: &str, body: Option<(u64, &mut dyn http::Source)>,
            mut sink: Option<&mut dyn FnMut(&[u8]) -> bool>) -> Option<u16> {
        let mut text = String::new();
        text.push_str("https://"); text.push_str(self.config.host());
        if self.config.port() != 443 { let _ = core::fmt::write(&mut text, format_args!(":{}", self.config.port())); }
        text.push_str(self.config.base_path()); text.push_str(route); text.push_str(query);
        let mut authorization = String::from("Bearer ");
        authorization.push_str(&self.token);
        let send_token = !self.token.is_empty();
        let agent_id = self.agent_id.clone();
        let mut buffer = core::mem::take(&mut self.body);
        buffer.clear();
        let result = (|| {
            let url = Url::parse(&text).ok()?;
            let connection = self.connect()?;
            let mut parser = mind::parse::Service(PARSER);
            // `token none`: the server asks no agent token, so none is sent.
            let all = [("Content-Type", content_type), ("X-Effector-Agent-ID", agent_id.as_str()), ("Authorization", authorization.as_str())];
            let headers = if send_token { &all[..] } else { &all[..2] };
            let sent = connection.request(method, &url, headers, body, &mut parser, false);
            let mut response = match sent { Ok(r) => r, Err(e) => { mind::println!("EFFECTOR: {} {}: {:?}", match method { Method::Get => "GET", Method::Post => "POST", Method::Put => "PUT" }, route, e); return None; } };
            let status = response.status;
            let mut chunk = [0u8; 2048];
            loop {
                match response.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) if sink.is_some() && status == 200 => if !(sink.as_mut().unwrap())(&chunk[..n]) { return None; },
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
        if let session::Final::Captured(kind) = ack.record.result {
            if let Some(outcome) = self.ack_artifact(ack, kind) { return outcome; }
            mind::println!("EFFECTOR: THE ARTIFACT OF {} IS GONE: ANSWERED io", ack.record.id.as_str());
        }
        let mut buf = [0u8; 1024];
        let Ok(body) = wire::ack(&mut buf, ack.record.id.as_str(), ack.record.result.result_json()) else { return Outcome::Permanent };
        let body: Vec<u8> = body.to_vec();
        let Some(status) = self.request(Method::Post, "/api/commands/ack", "", Some(&body)) else { return Outcome::Retry };
        let ok = status == 200 && self.answer(wire::ok_answer).is_ok();
        mind::println!("EFFECTOR: ACK {} {} {}", ack.record.id.as_str(), ack.record.result.status(), if ok { "ACCEPTED" } else { "NOT ACCEPTED" });
        session::outcome(status, ok)
    }

    /// The ACK of a result with its artifact, streamed from the private file it was kept in; `None` if that file does
    /// not hold this command's artifact.
    #[inline(never)]
    fn ack_artifact(&mut self, ack: &session::Ack, kind: Artifact) -> Option<Outcome> {
        let file = File::open(&self.path("artifact")).ok()?;
        let kept = kept(&file)?;
        if kept.command != ack.record.id.as_str() || file.size() != kept.start + kept.size { return None; }
        // A PNG's width and height, from its header chunk.
        let mut head = [0u8; 24];
        let picture = (kind == Artifact::Screen && file.read_at(kept.start, &mut head).ok() == Some(24))
            .then(|| (u32::from_be_bytes([head[16], head[17], head[18], head[19]]), u32::from_be_bytes([head[20], head[21], head[22], head[23]])));
        let content_type = if kind == Artifact::Screen { "image/png" } else { "text/plain; charset=utf-8" };
        let a = wire::Artifact { name: &kept.name, content_type, size: kept.size as u64, sha256: &kept.sha256, truncated: kept.truncated, picture };
        let mut buf = alloc::vec![0u8; 2048];
        let prefix = wire::artifact_ack(&mut buf, ack.record.id.as_str(), &a).ok()?.to_vec();
        let length = prefix.len() as u64 + wire::base64_len(kept.size as u64) + wire::ARTIFACT_SUFFIX.len() as u64;
        let mut source = Inline { file, at: kept.start, end: kept.start + kept.size, raw: alloc::vec![0; RAW], text: prefix, sent: 0, done: false };
        let Some(status) = self.send(Method::Post, "/api/commands/ack", "", "application/json", Some((length, &mut source)), None) else { return Some(Outcome::Retry) };
        let ok = status == 200 && self.answer(wire::ok_answer).is_ok();
        mind::println!("EFFECTOR: ACK {} completed WITH {} ({} BYTES) {}", ack.record.id.as_str(), kept.name, kept.size, if ok { "ACCEPTED" } else { "NOT ACCEPTED" });
        Some(session::outcome(status, ok))
    }

    /// An answer that needs no more ACK: its `done` line, and its artifact removed.
    #[inline(never)]
    fn finished(&self, record: &session::Record) {
        let mut line = [0u8; journal::LINE_MAX];
        let n = journal::done_line(record, &mut line);
        let path = self.path("journal");
        let written = File::open_mode(&path, MODE_WRITE).and_then(|mut f| { let at = f.size(); f.write_at(at, &line[..n])?; f.flush() });
        if written.is_err() { mind::println!("EFFECTOR: JOURNAL: THE DONE LINE OF {} WAS NOT WRITTEN", record.id.as_str()); }
        if matches!(record.result, session::Final::Captured(_)) { let _ = fs::remove(&self.path("artifact")); }
    }

    /// The journal written again whole (a record was dropped to make room): to `journal.new`, then put in its place.
    #[inline(never)]
    fn rewrite_journal(&self, session: &Session) -> bool {
        let (path, new) = (self.path("journal"), self.path("journal.new"));
        let Ok(mut file) = File::open_mode(&new, MODE_WRITE | MODE_CREATE | fs::MODE_TRUNCATE) else { return false };
        let mut at = 0;
        let written = journal::write_all(session.journal(), &mut |b| { let ok = file.write_at(at, b).map(|w| w == b.len()).unwrap_or(false); at += b.len(); ok });
        if !written || file.flush().is_err() { return false; }
        drop(file);
        let _ = fs::remove(&path);
        fs::rename(&new, &path).is_ok()
    }

    /// One more command answered: the session's count and the total kept across restarts (`executed`).
    fn count_executed(&mut self) {
        self.executed += 1;
        self.executed_total += 1;
        let text = alloc::format!("{}\n", self.executed_total);
        let _ = File::open_mode(&self.path("executed"), MODE_WRITE | MODE_CREATE | fs::MODE_TRUNCATE).and_then(|mut f| { f.write_at(0, text.as_bytes())?; f.flush() });
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
                        // An action the gateway serves runs once, for a new command in time whose answer the journal
                        // has room for; any other is denied.
                        let gateway_ok = mind::dev::cap_info(SLOT_GATEWAY).0 == CAP_KIND_ENDPOINT;
                        let done = if gateway_ok && session.runnable(&command, time()) { self.act(id, action, program, payload, initiator, reason) } else { None };
                        let received = match done { Some(done) => session.receive_with(command, time(), done), None => session.receive(command, time()) };
                        match received {
                            Ok(Accepted::Store(store)) => {
                                let durable = if store.rewrite { self.rewrite_journal(session) } else { self.append_journal(&store.record) };
                                if durable { self.count_executed(); }
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
            let status = self.send(Method::Post, "/api/commands/file", &query, "application/octet-stream", Some((opened.size, &mut body)), None);
            match (status, body.failed) {
                (_, Some(f)) => Final::Failed(f),
                (Some(201), None) => Final::Completed,
                _ => Final::Failed(Failure::Upload),
            }
        };
        let _ = gateway::close(GATEWAY, opened.handle);
        result
    }

    /// The action of a command the gateway serves, or `None` for one it does not (answered `denied`).
    #[inline(never)]
    fn act(&mut self, id: &str, action: &str, program: &str, payload: &str, initiator: &str, reason: &str) -> Option<session::Final> {
        use session::{Failure, Final};
        let service = |a| {
            // The server names a program; on MIND Core a program the owner's policy lets be controlled is a boot service.
            let r = gateway::service_control(GATEWAY, &origin(id, initiator, reason), program, a);
            match r { Ok(Ok(_)) => Final::Completed, Ok(Err(e)) => Final::Failed(failure(e)), Err(_) => Final::Failed(Failure::Unsupported) }
        };
        Some(match action {
            "collect_file" => self.collect_file(id, payload, initiator, reason),
            "deliver_file" => self.deliver_file(id, payload, initiator, reason),
            "collect_logs" => self.collect_logs(id, payload, initiator, reason),
            "screenshot" => {
                let opened = gateway::open_screen(GATEWAY, &origin(id, initiator, reason));
                self.capture(id, opened, Artifact::Screen, "screenshot.png")
            }
            "start_service" => service(gateway::ServiceAction::Start),
            "stop_service" => service(gateway::ServiceAction::Stop),
            "restart_service" => service(gateway::ServiceAction::Restart),
            _ => return None,
        })
    }

    /// `collect_logs`: `ring` (the system log) and `boot` (the last boot's log) through `open-log`, any other path
    /// through `open-file`; the text goes back inline, as the other agents' logs do.
    #[inline(never)]
    fn collect_logs(&mut self, id: &str, payload: &str, initiator: &str, reason: &str) -> session::Final {
        let path = if payload.trim_start().starts_with('{') {
            match self.parsed(payload.as_bytes(), |v| wire::log_path(v).map(String::from)) { Ok(p) => p, Err(_) => return session::Final::Failed(session::Failure::Malformed) }
        } else { String::from(payload.trim()) };
        let o = origin(id, initiator, reason);
        let (opened, name) = match path.as_str() {
            "ring" | "boot" => (gateway::open_log(GATEWAY, &o, &path), alloc::format!("{}.log", path)),
            p => (gateway::open_file(GATEWAY, &o, p), String::from(p.rsplit(['/', ':']).next().unwrap_or("collected.log"))),
        };
        self.capture(id, opened, Artifact::Log, &name)
    }

    /// An opened transfer kept as the answer's artifact, then closed.
    fn capture(&mut self, id: &str, opened: mind::sys::Result<core::result::Result<gateway::Transfer, gateway::Error>>, kind: Artifact, name: &str) -> session::Final {
        use session::{Failure, Final};
        let opened = match opened {
            Ok(Ok(t)) => t,
            Ok(Err(e)) => { mind::println!("EFFECTOR: {}: THE GATEWAY ANSWERED {:?}", id, e); return Final::Failed(failure(e)) }
            Err(e) => { mind::println!("EFFECTOR: {}: NO ANSWER FROM THE GATEWAY: {:?}", id, e); return Final::Failed(Failure::Unsupported) }
        };
        let kept = self.keep(id, &opened, name);
        if let Err(f) = kept { mind::println!("EFFECTOR: {}: {} BYTES NOT KEPT: {}", id, opened.size, f.name()); }
        let _ = gateway::close(GATEWAY, opened.handle);
        match kept { Ok(()) => Final::Captured(kind), Err(f) => Final::Failed(f) }
    }

    /// Copies an open transfer into the private `artifact` file, after a header line naming the command, checking the
    /// SHA-256 the gateway announced: it stays there until its answer is acknowledged.
    #[inline(never)]
    fn keep(&mut self, id: &str, t: &gateway::Transfer, name: &str) -> Result<(), session::Failure> {
        use session::Failure;
        if t.size > INLINE_MAX { return Err(Failure::Bounds); }
        let name: String = name.chars().filter(|c| c.is_ascii_graphic() && *c != '"' && *c != '\\').take(64).collect();
        let header = alloc::format!("{} {} {} {} {}\n", id, t.size, t.sha256.as_str(), t.truncated as u8, if name.is_empty() { "artifact" } else { &name });
        let mut file = File::open_mode(&self.path("artifact"), MODE_WRITE | MODE_CREATE | fs::MODE_TRUNCATE).map_err(|_| Failure::Io)?;
        file.write_at(0, header.as_bytes()).map_err(|_| Failure::Io)?;
        let mut piece = alloc::vec![0u8; PIECE];
        let mut hash = effector::sha256::Sha256::new();
        let mut offset = 0u64;
        while offset < t.size {
            let n = match gateway::read(GATEWAY, t.handle, offset, &mut piece) {
                Ok(Ok(0)) | Err(_) => return Err(Failure::Io),
                Ok(Ok(n)) => n,
                Ok(Err(e)) => return Err(failure(e)),
            };
            hash.update(&piece[..n]);
            if file.write_at(header.len() + offset as usize, &piece[..n]).ok() != Some(n) { return Err(Failure::Io); }
            offset += n as u64;
        }
        let digest = hash.finish();
        let hex: String = digest.iter().map(|b| alloc::format!("{:02x}", b)).collect();
        if hex != t.sha256.as_str() { return Err(Failure::Digest); }
        file.flush().map_err(|_| Failure::Io)
    }

    /// `deliver_file`: the bytes downloaded from `/api/commands/file` straight into the gateway, which checks their size
    /// and SHA-256 before the file takes its name under the policy's delivery root (750-KRN-0120).
    #[inline(never)]
    fn deliver_file(&mut self, id: &str, payload: &str, initiator: &str, reason: &str) -> session::Final {
        use session::{Failure, Final};
        let Ok((path, size, sha256, overwrite)) = self.parsed(payload.as_bytes(), |v| wire::deliver(v).map(|d| (String::from(d.path), d.size, String::from(d.sha256), d.overwrite))) else {
            return Final::Failed(Failure::Malformed);
        };
        let handle = match gateway::deliver_begin(GATEWAY, &origin(id, initiator, reason), &path, size, &sha256, overwrite) {
            Ok(Ok(h)) => h,
            Ok(Err(e)) => return Final::Failed(failure(e)),
            Err(_) => return Final::Failed(Failure::Unsupported),
        };
        let query = alloc::format!("?agent_id={}&command_id={}", self.agent_id, id);
        let (mut piece, mut offset, mut failed) = (Vec::with_capacity(PIECE), 0u64, None);
        let status = {
            let mut sink = |data: &[u8]| -> bool {
                for b in data.chunks(PIECE) {
                    if offset + (piece.len() + b.len()) as u64 > size { failed = Some(Failure::Digest); return false; }
                    let room = PIECE - piece.len();
                    piece.extend_from_slice(&b[..b.len().min(room)]);
                    if piece.len() == PIECE {
                        match gateway::write(GATEWAY, handle, offset, &piece) { Ok(Ok(())) => {}, Ok(Err(e)) => { failed = Some(failure(e)); return false; } Err(_) => { failed = Some(Failure::Io); return false; } }
                        offset += PIECE as u64;
                        piece.clear();
                        piece.extend_from_slice(&b[b.len().min(room)..]);
                    }
                }
                true
            };
            self.send(Method::Get, "/api/commands/file", &query, "application/json", None, Some(&mut sink))
        };
        let result = match (status, failed) {
            (_, Some(f)) => Err(f),
            (Some(200), None) if !piece.is_empty() => match gateway::write(GATEWAY, handle, offset, &piece) { Ok(Ok(())) => Ok(()), Ok(Err(e)) => Err(failure(e)), Err(_) => Err(Failure::Io) },
            (Some(200), None) => Ok(()),
            _ => Err(Failure::Upload),
        };
        match result.and_then(|()| match gateway::deliver_commit(GATEWAY, handle) { Ok(Ok(())) => Ok(()), Ok(Err(e)) => Err(failure(e)), Err(_) => Err(Failure::Io) }) {
            Ok(()) => Final::Completed,
            Err(f) => { let _ = gateway::close(GATEWAY, handle); Final::Failed(f) }
        }
    }

    /// One telemetry sample (750-EFF-0014): the gateway's counters and the network cards, rates against the last one.
    #[inline(never)]
    fn telemetry(&mut self) {
        if mind::dev::cap_info(SLOT_GATEWAY).0 != CAP_KIND_ENDPOINT { return; }
        let s = match gateway::sample(GATEWAY) { Ok(s) => s, Err(e) => return self.no_telemetry(alloc::format!("NO SAMPLE FROM THE GATEWAY: {:?}", e)) };
        // Without the system monitor there is no processor count, which the server requires.
        if s.cpus == 0 { return self.no_telemetry(String::from("THE GATEWAY COUNTS NO PROCESSORS")); }
        let Some(unix) = mind::rtc::unix_time() else { return self.no_telemetry(String::from("NO CLOCK")) };
        let state = |st: gateway::ServiceState| match st {
            gateway::ServiceState::Running => "running", gateway::ServiceState::Stopped => "stopped",
            gateway::ServiceState::Quarantined => "quarantined", gateway::ServiceState::Absent => "not_found",
        };
        let counters = telemetry::Counters {
            cpus: s.cpus, busy_ns: s.busy_ns, idle_ns: s.idle_ns, uptime_ms: s.uptime_ms, memory_total: s.memory_total, memory_free: s.memory_free,
            services: s.services.as_slice().iter().map(|l| telemetry::Load { name: String::from(l.name.as_str()), state: state(l.state), pid: l.pid, run_ns: l.run_ns, started_ns: l.started_ns, memory: l.memory }).collect(),
            volumes: s.volumes.as_slice().iter().map(|v| telemetry::Space { name: String::from(v.name.as_str()), bytes: v.bytes, free: v.free }).collect(),
        };
        let cards = cards();
        let mut stamp = [0u8; 20];
        let sampled_at = String::from(wire::rfc3339_utc(unix, &mut stamp));
        let sample_id = alloc::format!("s{}-{}", unix, s.uptime_ms);
        let id = telemetry::Identity { sample_id: &sample_id, sampled_at: &sampled_at, cpu_model: &self.cpu_model, arch: wire::arch(), executed: self.executed, executed_total: self.executed_total };
        let mut buf = alloc::vec![0u8; telemetry::BODY_MAX];
        let Ok(body) = telemetry::sample(&mut buf, &counters, self.last.as_ref(), &cards, &id) else { mind::println!("EFFECTOR: TELEMETRY OVER {} BYTES", telemetry::BODY_MAX); return };
        let body = body.to_vec();
        self.last = Some(counters);
        // One line when what the server answers changes, not one every 30 s; a lost connection says so itself.
        let Some(status) = self.request(Method::Post, "/api/telemetry", "", Some(&body)) else { return };
        if self.telemetry_said != Some(status) {
            if status == 204 { mind::println!("EFFECTOR: TELEMETRY SENT ({} BYTES), EVERY {} S", body.len(), telemetry::PERIOD_MS / 1000); }
            else { mind::println!("EFFECTOR: TELEMETRY ANSWERED {}", status); }
            self.telemetry_said = Some(status);
        }
    }

    /// Why no telemetry is sent, once until it is sent again.
    fn no_telemetry(&mut self, why: String) {
        if self.telemetry_said != Some(0) { mind::println!("EFFECTOR: TELEMETRY NOT SENT: {}", why); self.telemetry_said = Some(0); }
    }

    fn path(&self, name: &str) -> String { let mut p = self.dir.clone(); p.push('/'); p.push_str(name); p }
}

/// The network cards as the session's grant sees them (`socket.wit` 2.5: a grant may ask `interfaces` and `cards`).
fn cards() -> Vec<telemetry::Card> {
    let network = Endpoint(SLOT_NETWORK);
    let Ok(Ok(cards)) = socket::cards(network) else { return Vec::new() };
    let interfaces = socket::interfaces(network).ok().and_then(|r| r.ok());
    cards.as_slice().iter().filter(|c| c.present).map(|c| {
        let i = interfaces.as_ref().and_then(|l| l.as_slice().iter().find(|i| i.card == c.card).copied());
        telemetry::Card { card: c.card, wireless: c.kind == socket::CardKind::Wireless, up: c.up, mac: i.map_or(0, |i| i.mac), address: i.map_or(0, |i| i.address) }
    }).collect()
}

/// The processor's name as it gives it (x86's CPUID brand string); empty where user code cannot read it.
fn cpu_model() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        #[allow(unused_unsafe)]
        let leaf = |n: u32| unsafe { core::arch::x86_64::__cpuid(n) };
        if leaf(0x8000_0000).eax >= 0x8000_0004 {
            let mut bytes = Vec::with_capacity(48);
            for n in 0x8000_0002..=0x8000_0004 { let r = leaf(n); for v in [r.eax, r.ebx, r.ecx, r.edx] { bytes.extend_from_slice(&v.to_le_bytes()); } }
            let text: String = bytes.iter().take_while(|&&b| b != 0).filter(|b| b.is_ascii_graphic() || **b == b' ').map(|&b| b as char).collect();
            return String::from(text.split_whitespace().collect::<Vec<_>>().join(" ").as_str());
        }
    }
    String::new()
}

/// The artifact file's header line.
fn kept(file: &File) -> Option<Kept> {
    let mut head = [0u8; 256];
    let n = file.read_at(0, &mut head).ok()?;
    let end = head[..n].iter().position(|&b| b == b'\n')?;
    let line = core::str::from_utf8(&head[..end]).ok()?;
    let mut w = line.split(' ');
    let (command, size, sha256, truncated, name) = (w.next()?, w.next()?.parse().ok()?, w.next()?, w.next()?, w.next()?);
    if w.next().is_some() || sha256.len() != 64 { return None; }
    Some(Kept { command: String::from(command), size, sha256: String::from(sha256), truncated: truncated == "1", name: String::from(name), start: end + 1 })
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
fn recover(agent: &Agent) -> (Vec<session::Record>, Vec<bool>) {
    // A rewrite cut between removing the journal and renaming its new form: the new form is whole (it was flushed).
    let (path, new) = (agent.path("journal"), agent.path("journal.new"));
    if File::open(&path).is_err() && File::open(&new).is_ok() { let _ = fs::rename(&new, &path); }
    let _ = fs::remove(&new);
    if File::open(&path).is_err() { return (Vec::new(), Vec::new()); }
    // Never read as empty: a journal too large to read refuses the start, so no saved answer is forgotten.
    let Some(bytes) = read_file(&path, JOURNAL_MAX) else { fail(format_args!("JOURNAL REFUSED: OVER {} BYTES OR UNREADABLE", JOURNAL_MAX)) };
    let result = journal::read(&bytes, &agent.agent_id);
    match &result {
        Ok((records, n, done)) => (records[..*n].iter().flatten().copied().collect(), done[..*n].to_vec()),
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
    // The owner's agent ID, so a machine written afresh stays the agent the server knows (kept privately at first start).
    let owner_id = read_file(&alloc::format!("{}/agent_id", dir), 80);

    let config = match read_file(conf, config::FILE_MAX).map(|b| config::Config::parse(&b)) {
        Some(Ok(c)) => c,
        Some(Err(e)) => fail(format_args!("CONFIGURATION REFUSED AT LINE {}: {:?}", e.line, e.kind)),
        None => fail(format_args!("CANNOT READ {}", conf)),
    };
    let dir = if service { fs::use_scope(Endpoint(SLOT_VFS), ""); String::from(SESSION_DIR) } else { dir };
    let mut agent = Box::new(Agent { config, token: String::new(), agent_id: String::new(), dir, connection: None, reconnect_at: 0, reconnect_ms: 1000, body: Vec::new(), activation_noted: false,
        telemetry_at: 0, last: None, telemetry_said: None, executed: 0, executed_total: 0, cpu_model: cpu_model() });
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
        None if service && owner_id.is_some() => {
            let Some(id) = owner_id.as_deref().and_then(config::agent_id) else { fail(format_args!("{}/agent_id IS MALFORMED", OWNER_DIR)) };
            let written = File::open_mode(&id_path, MODE_WRITE | MODE_CREATE).and_then(|mut f| { f.write_at(0, id.as_bytes())?; f.write_at(id.len(), b"\n")?; f.flush() });
            if written.is_err() { fail(format_args!("CANNOT WRITE {}", id_path)); }
            agent.agent_id = String::from(id);
            mind::println!("EFFECTOR: AGENT ID {} FROM {}/agent_id, KEPT IN {}", id, OWNER_DIR, SESSION_DIR);
        }
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
    let (records, done) = recover(&agent);
    let count = records.len();
    let session = match Session::restore(&agent.agent_id, &records, &done, now() as u64) { Ok(s) => Box::new(s), Err(e) => fail(format_args!("SESSION: {:?}", e)) };
    agent.executed_total = read_file(&agent.path("executed"), 24).and_then(|b| core::str::from_utf8(&b).ok().and_then(|t| t.trim().parse().ok())).unwrap_or(0);
    let waiting = done.iter().filter(|d| !**d).count();
    // Compacted at each start: each record once, with one done line (a redelivered answer adds another).
    if count > 0 && !agent.rewrite_journal(&session) { mind::println!("EFFECTOR: JOURNAL: NOT COMPACTED, KEPT AS IT WAS"); }
    mind::println!("EFFECTOR: AGENT {} TO {}{} ({} SAVED RESULTS, {} TO ACKNOWLEDGE)", agent.agent_id, agent.config.host(), agent.config.base_path(), count, waiting);

    (agent, session, run_for)
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("effector — the Effector agent's session (750-EFF-0006): heartbeat, poll, the gateway's actions and telemetry with a pinned Effector server.\nUsage: effector data/effector/effector.conf [--for SECONDS]");
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
                if outcome != Outcome::Retry { agent.finished(&ack.record); }
                // The agent is free again: ask for the next command at once.
                if outcome == Outcome::Accepted && !session.busy() { session.wake(now() as u64); }
            }
            Ok(None) => {}
            Err(e) => mind::println!("EFFECTOR: ACK SCHEDULE: {:?}", e),
        }
        // Telemetry every 30 s once the server knows the agent, the first 5 s after.
        if session.registered() && !session.stopping() {
            if agent.telemetry_at == 0 { agent.telemetry_at = now() + 5000; }
            if now() >= agent.telemetry_at { agent.telemetry(); agent.telemetry_at = now() + telemetry::PERIOD_MS as usize; }
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
