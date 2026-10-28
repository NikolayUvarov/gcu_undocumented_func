//! effector485 (800-LNK-0011): a small Effector over MIND Link, for local use. `effector485 daemon` holds the cable as
//! its one master and serves Effector's operations on 127.0.0.1 (docs/effector.md, section 3): each with an id, an
//! idempotency key and a reason; queued and carried out in order; read for status and output; cancelled by id. The
//! other commands are its client. A lease holds the machine for one session (800-LNK-0021). Several machines may share
//! the cable, each at its address (800-LNK-0026). docs/link/effector485.md says how to use it.
mod bus;
mod feed;
mod http;
mod json;
mod ops;
mod traffic;
mod worker;
mod ws;
#[cfg(unix)]
#[path = "../../port_unix.rs"]
mod port;
#[cfg(windows)]
#[path = "../../port_windows.rs"]
mod port;

use json::{obj, s, Value};
use ops::{now, utc, State, Submitted, FINAL, KINDS, LEASE_MINUTES_MAX};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const AT: &str = "127.0.0.1:8485";

const USAGE: &str = "usage: effector485 daemon <port> [--baud <rate>] [--payload <bytes>] [--timeout <ms>] [--tries <n>]
                          [--addresses <n>[,<n>...]] [--listen <host:port>] [--state <dir>] [--key-file <file>]
       effector485 [--at <host:port>] [--key-file <file>] [--agent <machine>] <command>

The daemon holds the cable (<port> as for mindlink: COM22, /dev/ttyUSB0, tcp:HOST:PORT) and serves operations on
--listen (default 127.0.0.1:8485). Its state, the queue and the results, is kept in --state (default: .effector485 in
the home directory). With --key-file, every call must carry that file's text in the X-API-Key header.
A browser on this machine shows the operations live and starts the usual ones at http://<listen>/.

--addresses: the machines sharing one RS-485 cable, each at the address in its link.conf (address=<n>, 1 to 247), and
0 for one machine without an address. They are the API's link-<n> (link for 0), each with its own queue and lease.
Without it, one machine without an address: link. With several, every submit and reserve names one with --agent
(or EFFECTOR485_AGENT): the API's agent_id.

commands: submit <kind> [<field>=<value>...] --key <idempotency key> --reason <reason> [--wait]
              kinds and their fields:
                hello
                put file=<path here> [name=<there>] [force=true]
                get name=<there> file=<path here>
                list [name=<directory>]
                stat name=<there>
                mkdir name=<directory>
                move from=<there> to=<there> [replace=true]
                delete name=<file>
                rmdir name=<directory> [recursive=true]
                run name=<program there> [args=<arguments>] [timeout_seconds=<n>]
                install file=<path here> [name=<file name>]    into apps/, replacing any
                uninstall name=<file name>                     from apps/
                log [from=<cursor>] [max_bytes=<n>]
                reboot                                         restarts the other end (MIND Core: dev in link.conf)
          status <id>                    the operation as JSON
          wait <id>                      its output as it comes, until it ends; the exit code of a program
          output <id>                    its output so far
          list [--active]                the operations, newest last
          cancel <id> --key <idempotency key> --reason <reason>
          endpoints                      the cable's other end: online, its name, the lease
          reserve --minutes <1-60> --reason <reason> [--owner <track>]
                                         holds the machine for this session: prints SESSION:PIN alone on its first
                                         line. Others' operations that change it wait; reads pass
          renew [SESSION:PIN] [--minutes <1-60>]   the lease's time again, from now
          release [SESSION:PIN]
          reservations                   the lease held: session, owner, reason, expiry, seconds left

submit and cancel carry the lease of --reservation SESSION:PIN, or of EFFECTOR485_RESERVATION; renew and release take
it from there when it is not given.";

fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("effector485: {msg}");
    std::process::exit(1)
}

fn home_state() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).unwrap_or_else(|| ".".into());
    PathBuf::from(home).join(".effector485")
}

fn read_key(file: &Option<String>) -> Option<String> {
    file.as_ref().map(|f| std::fs::read_to_string(f).unwrap_or_else(|e| die(format!("{f}: {e}"))).trim().to_string()).filter(|k| !k.is_empty())
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let take = |args: &mut Vec<String>, flag: &str| -> Option<String> {
        let i = args.iter().position(|a| a == flag)?;
        if i + 1 >= args.len() { die(format!("{flag} takes a value")); }
        let v = args.remove(i + 1);
        args.remove(i);
        Some(v)
    };
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") { println!("{USAGE}"); return; }
    let key_file = take(&mut args, "--key-file").or_else(|| std::env::var("EFFECTOR485_KEY_FILE").ok());
    if args[0] == "daemon" {
        let num = |args: &mut Vec<String>, flag: &str| take(args, flag).map(|v| v.parse::<u64>().unwrap_or_else(|_| die(format!("{flag} takes a number"))));
        let o = worker::Options {
            baud: num(&mut args, "--baud").unwrap_or(115_200) as u32,
            timeout: num(&mut args, "--timeout").map(|t| t as u32),
            tries: num(&mut args, "--tries").unwrap_or(8).max(1) as u32,
            payload: num(&mut args, "--payload").unwrap_or(linkproto::frame::MAX_PAYLOAD as u64) as usize,
        };
        let addresses = take(&mut args, "--addresses").map_or(vec![0], |list| addresses(&list).unwrap_or_else(|e| die(e)));
        let listen = take(&mut args, "--listen").unwrap_or_else(|| AT.to_string());
        let dir = take(&mut args, "--state").map(PathBuf::from).unwrap_or_else(home_state);
        let [_, spec] = &args[..] else { die(USAGE) };
        return daemon(spec.clone(), o, &addresses, &listen, &dir, read_key(&key_file));
    }
    let at = take(&mut args, "--at").or_else(|| std::env::var("EFFECTOR485_AT").ok()).unwrap_or_else(|| AT.to_string());
    let key = read_key(&key_file);
    client(&at, key.as_deref(), args);
}

/// `--addresses`: distinct addresses, 0 to ADDRESS_MAX, comma-separated.
fn addresses(list: &str) -> Result<Vec<u8>, String> {
    let mut v = Vec::new();
    for word in list.split(',').map(str::trim) {
        let a = word.parse::<u8>().ok().filter(|&a| a <= linkproto::frame::ADDRESS_MAX)
            .ok_or_else(|| format!("--addresses: {word:?} is not 0 to {}", linkproto::frame::ADDRESS_MAX))?;
        if v.contains(&a) { return Err(format!("--addresses: {a} twice")); }
        v.push(a);
    }
    Ok(v)
}

fn daemon(spec: String, o: worker::Options, addresses: &[u8], listen: &str, dir: &std::path::Path, key: Option<String>) {
    let state = State::load(dir, &spec, addresses).unwrap_or_else(|e| die(e));
    let listener = TcpListener::bind(listen).unwrap_or_else(|e| die(format!("{listen}: {e}")));
    let bound = listener.local_addr().map(|a| a.to_string()).unwrap_or_else(|_| listen.to_string());
    if !bound.starts_with("127.") && !bound.starts_with("[::1]") && key.is_none() { die(format!("{bound} is reachable from other machines: give --key-file")); }
    // The first line says where it listens (the tests read it).
    println!("effector485 listening on {bound}, the cable on {spec}, the state in {}", dir.display());
    if addresses.len() > 1 || addresses[0] != 0 { eprintln!("effector485: {spec}: the machines {}", state.agents()); }
    traffic::start(if spec.contains(':') { 0 } else { o.baud });
    let changes = state.changes.clone();
    let state = Arc::new(Mutex::new(state));
    let bus = bus::Bus::new(&spec, o.baud);
    for index in 0..addresses.len() {
        let (state, bus) = (state.clone(), bus.clone());
        std::thread::spawn(move || worker::work(state, index, bus, o));
    }
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let (state, key, changes) = (state.clone(), key.clone(), changes.clone());
        let bound = bound.clone();
        std::thread::spawn(move || {
            let request = match http::read_request(&mut stream) { Ok(r) => r, Err(e) => return http::respond(&mut stream, 400, "text/plain", &e) };
            if let Err(why) = same_site(&request, &bound) { return http::respond(&mut stream, 403, "application/json", &error(why).text()); }
            // The page itself is static and holds nothing: it asks the API, which the key guards.
            if request.method == "GET" && request.path == "/" { return http::respond(&mut stream, 200, "text/html; charset=utf-8", PAGE); }
            // The page's live feed: a browser sends no X-API-Key on a WebSocket, so the feed asks for the key itself.
            if request.method == "GET" && request.path == "/api/v1/stream" {
                let Some(websocket) = &request.websocket else { return http::respond(&mut stream, 400, "application/json", &error("a WebSocket is expected here").text()) };
                let Some(place) = feed::Place::take() else { return http::respond(&mut stream, 503, "application/json", &error("too many pages are open").text()) };
                if ws::handshake(&mut stream, websocket).is_ok() { feed::serve(stream, state, changes, key, place); }
                return;
            }
            let (status, body) = if key.is_some() && request.key != key { (401, error("the X-API-Key header is missing or wrong")) } else { route(&state, &request) };
            http::respond(&mut stream, status, "application/json", &body.text());
        });
    }
}

fn error(text: &str) -> Value { obj(vec![("error", s(text))]) }
fn not_held(text: &str) -> Value { obj(vec![("error", s(text)), ("code", s("reservation_not_held"))]) }

/// A lease's `duration_minutes`: a whole number from 1 to LEASE_MINUTES_MAX, or none.
fn minutes(v: &Value) -> Result<Option<u64>, String> {
    match v.get("duration_minutes") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Num(n)) if n.fract() == 0.0 && (1.0..=LEASE_MINUTES_MAX as f64).contains(n) => Ok(Some(*n as u64)),
        _ => Err(format!("\"duration_minutes\" is a whole number from 1 to {LEASE_MINUTES_MAX}")),
    }
}

/// The lease a call names: by `reservation_session_id` and `reservation_pin` in `body`, or by the headers.
fn credentials(r: &http::Request, body: &Value) -> Option<(String, String)> {
    let session = body.str("reservation_session_id").map(str::to_string).or_else(|| r.session.clone());
    let pin = body.str("reservation_pin").map(str::to_string).or_else(|| r.pin.clone());
    if session.is_none() && pin.is_none() { return None; }
    Some((session.unwrap_or_default(), pin.unwrap_or_default()))
}

/// The browser page: the daemon's state, its operations and their output, and buttons for the usual ones.
const PAGE: &str = include_str!("ui.html");

/// A request from this daemon's own page or from a program, not from another site in the same browser: on a loopback
/// address the Host must be a loopback name with this port (no DNS rebinding), and an Origin, which browsers send, must be
/// this daemon's own.
fn same_site(r: &http::Request, bound: &str) -> Result<(), &'static str> {
    let port = bound.rsplit(':').next().unwrap_or("");
    let loopback = bound.starts_with("127.") || bound.starts_with("[::1]");
    let local = |host: &str| [format!("127.0.0.1:{port}"), format!("localhost:{port}"), format!("[::1]:{port}")].iter().any(|h| h == host);
    if loopback {
        if let Some(host) = &r.host { if !local(host) { return Err("this daemon answers only to its own loopback address"); } }
    }
    if let Some(origin) = &r.origin {
        let own = origin.strip_prefix("http://").map(str::to_ascii_lowercase);
        let same = match (&own, &r.host) { (Some(o), Some(h)) => o == h && (!loopback || local(o)), _ => false };
        if !same { return Err("a request from another site is refused"); }
    }
    Ok(())
}

fn route(state: &Arc<Mutex<State>>, r: &http::Request) -> (u16, Value) {
    let mut st = worker::lock(state);
    let t = now();
    st.expire(t);
    match (r.method.as_str(), r.path.as_str()) {
        ("GET", "/api/v1/api-key") => (200, obj(vec![("valid", Value::Bool(true))])),
        ("GET", "/api/v1/endpoints") => {
            // The cable's bytes are the port's, the same for every machine on it.
            let traffic = traffic::sample();
            let list = st.endpoints(t).into_iter().map(|mut v| {
                v.set("sent_bytes", Value::Num(traffic.sent as f64));
                v.set("received_bytes", Value::Num(traffic.received as f64));
                v
            }).collect();
            (200, Value::Arr(list))
        }
        ("POST", "/api/v1/operations") => {
            let request = match json::parse(&r.body) { Ok(v) => v, Err(e) => return (400, error(&e)) };
            // The lease the headers name; a key used already answers its operation, whatever became of that lease.
            let used = request.str("idempotency_key").is_some_and(|k| st.ops.iter().any(|o| o.key == k));
            let agent = match st.agent_of(&request) { Ok(a) => a, Err(e) if !used => return (400, error(&e)), Err(_) => String::new() };
            let session = match credentials(r, &Value::Null) {
                None => None,
                Some((session, pin)) => match st.holds(&agent, &session, &pin, t) {
                    Ok(()) => Some(session),
                    Err(e) if !used => return (409, not_held(&e)),
                    Err(_) => None,
                },
            };
            match st.submit(&request, session) {
                Ok(Submitted::New(i)) => { let v = st.view(i, t); st.save(); (201, v) }
                Ok(Submitted::Again(i)) => (200, st.view(i, t)),
                Err(e) => (400, error(&e)),
            }
        }
        ("GET", "/api/v1/operations") => {
            if let Some(id) = r.param("id") {
                return match st.find(id) { Some(i) => (200, st.view(i, t)), None => (404, error(&format!("no operation {id}"))) };
            }
            let active = r.param("status") == Some("active");
            let agent = r.param("agent_id");
            let list = (0..st.ops.len()).filter(|&i| (!active || !st.ops[i].ended()) && agent.is_none_or(|a| st.ops[i].agent == a)).map(|i| {
                let mut v = st.view(i, t);
                // A list is for finding operations: their output is read one by one.
                if let Some(Value::Obj(result)) = match &mut v { Value::Obj(f) => f.iter_mut().find(|(k, _)| k == "result").map(|(_, v)| v), _ => None } {
                    result.retain(|(k, _)| k != "stdout");
                }
                v
            }).collect();
            (200, Value::Arr(list))
        }
        ("GET", "/api/v1/operations/output") => {
            let Some(i) = r.param("id").and_then(|id| st.find(id)) else { return (404, error("no such operation")) };
            let op = &st.ops[i];
            let first = op.output_bytes - op.output.len() as u64;
            let from = r.param("from").and_then(|f| f.parse::<u64>().ok()).unwrap_or(0).clamp(first, op.output_bytes);
            let text = String::from_utf8_lossy(&op.output[(from - first) as usize..]).into_owned();
            (200, obj(vec![("id", s(&op.id)), ("status", s(&op.status)), ("from", Value::Num(from as f64)),
                           ("next", Value::Num(op.output_bytes as f64)), ("text", s(&text))]))
        }
        ("POST", "/api/v1/operations/cancel") => {
            let request = match json::parse(&r.body) { Ok(v) => v, Err(e) => return (400, error(&e)) };
            let (Some(id), Some(key), Some(reason)) = (request.str("operation_id"), request.str("idempotency_key"), request.str("reason")) else {
                return (400, error("a cancel needs \"operation_id\", its own \"idempotency_key\" and a \"reason\""));
            };
            match st.cancel(id, key, reason) { Ok(i) => { let v = st.view(i, t); st.save(); (200, v) } Err((code, e)) => (code, error(&e)) }
        }
        ("GET", "/api/v1/terminal-reservations") => {
            let agent = r.param("agent_id");
            let list = st.leases.iter().filter(|l| l.expires > t && agent.is_none_or(|a| l.agent == a)).map(|l| l.json(t)).collect();
            (200, obj(vec![("reservations", Value::Arr(list))]))
        }
        ("POST", "/api/v1/terminal-reservations") => {
            let request = match json::parse(&r.body) { Ok(v) => v, Err(e) => return (400, error(&e)) };
            let minutes = match minutes(&request) {
                Ok(Some(m)) => m,
                Ok(None) => return (400, error(&format!("a reservation needs \"duration_minutes\", 1 to {LEASE_MINUTES_MAX}"))),
                Err(e) => return (400, error(&e)),
            };
            let Some(reason) = request.str("reason").map(str::trim).filter(|r| !r.is_empty()) else {
                return (400, error("a reservation needs a \"reason\", which everyone sees"));
            };
            let agent = match st.agent_of(&request) { Ok(a) => a, Err(e) => return (400, error(&e)) };
            match st.reserve(&agent, minutes, reason, request.str("owner").unwrap_or("").trim(), t) {
                Ok(l) => (201, obj(vec![("reservation", l.json(t)), ("pin", s(&l.pin))])),
                Err(l) => (409, obj(vec![("error", l.waiting().get("message").cloned().unwrap_or(Value::Null)), ("code", s("terminal_reserved")), ("reservation", l.json(t))])),
            }
        }
        ("PATCH" | "DELETE", "/api/v1/terminal-reservations") => {
            let request = if r.body.trim().is_empty() { obj(vec![]) } else { match json::parse(&r.body) { Ok(v) => v, Err(e) => return (400, error(&e)) } };
            let Some((session, pin)) = credentials(r, &request) else {
                return (400, error("the reservation is named by \"reservation_session_id\" and \"reservation_pin\", or the X-Effector-Reservation-Session and -Pin headers"));
            };
            if r.method == "DELETE" {
                return match st.release(&session, &pin, t) { Ok(l) => (200, obj(vec![("status", s("released")), ("reservation", l.json(t))])), Err(e) => (409, not_held(&e)) };
            }
            let minutes = match minutes(&request) { Ok(m) => m, Err(e) => return (400, error(&e)) };
            match st.renew(&session, &pin, minutes, t) { Ok(l) => (200, obj(vec![("reservation", l.json(t))])), Err(e) => (409, not_held(&e)) }
        }
        _ => (404, error(&format!("no route {} {}", r.method, r.path))),
    }
}

fn call(at: &str, key: Option<&str>, method: &str, path: &str, body: Option<&Value>) -> Value { call_with(at, key, &[], method, path, body) }

fn call_with(at: &str, key: Option<&str>, headers: &[(&str, &str)], method: &str, path: &str, body: Option<&Value>) -> Value {
    let (status, text) = http::call(at, method, path, key, headers, body.map(|b| b.text()).as_deref()).unwrap_or_else(|e| die(e));
    let v = json::parse(&text).unwrap_or_else(|e| die(format!("the daemon's answer: {e}")));
    if status >= 400 { die(v.str("error").unwrap_or(&text)); }
    v
}

/// What a `field=value` gives: true, false and numbers as such; a path here made absolute, since the daemon's
/// directory is not the caller's.
fn field(name: &str, value: &str) -> Value {
    if name == "file" { return s(&std::path::absolute(value).unwrap_or_else(|e| die(format!("{value}: {e}"))).to_string_lossy()); }
    match value {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        v if !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) && matches!(name, "timeout_seconds" | "from" | "max_bytes") => Value::Num(v.parse().unwrap()),
        v => s(v),
    }
}

fn client(at: &str, key: Option<&str>, mut args: Vec<String>) {
    let take = |args: &mut Vec<String>, flag: &str| -> Option<String> {
        let i = args.iter().position(|a| a == flag)?;
        let v = args.get(i + 1).cloned().unwrap_or_else(|| die(format!("{flag} takes a value")));
        args.drain(i..i + 2);
        Some(v)
    };
    let wait_flag = args.iter().position(|a| a == "--wait").map(|i| { args.remove(i); }).is_some();
    let active = args.iter().position(|a| a == "--active").map(|i| { args.remove(i); }).is_some();
    let idem = take(&mut args, "--key");
    let reason = take(&mut args, "--reason");
    let minutes = take(&mut args, "--minutes").map(|m| m.parse::<u64>().unwrap_or_else(|_| die("--minutes takes a number")));
    let owner = take(&mut args, "--owner");
    // The machine, on a cable with several (800-LNK-0026).
    let agent = take(&mut args, "--agent").or_else(|| std::env::var("EFFECTOR485_AGENT").ok()).filter(|a| !a.trim().is_empty());
    // The lease this call holds, as `reserve` printed it.
    let pair = |l: &str| l.trim().split_once(':').map(|(a, b)| (a.to_string(), b.to_string()))
        .unwrap_or_else(|| die(format!("{l}: a reservation is SESSION:PIN, as `effector485 reserve` prints it")));
    let lease = take(&mut args, "--reservation").or_else(|| std::env::var("EFFECTOR485_RESERVATION").ok()).filter(|l| !l.trim().is_empty()).map(|l| pair(&l));
    let held: Vec<(&str, &str)> = lease.iter().flat_map(|(session, pin)| [("X-Effector-Reservation-Session", session.as_str()), ("X-Effector-Reservation-Pin", pin.as_str())]).collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["submit", kind, fields @ ..] => {
            if !KINDS.iter().any(|(k, _)| k == kind) { die(format!("no kind {kind}: {}", KINDS.map(|k| k.0).join(", "))); }
            let mut request = obj(vec![("kind", s(kind))]);
            if let Some(a) = &agent { request.set("agent_id", s(a)); }
            for f in fields {
                let (name, value) = f.split_once('=').unwrap_or_else(|| die(format!("{f}: a field is <name>=<value>")));
                request.set(name, field(name, value));
            }
            request.set("idempotency_key", s(&idem.unwrap_or_else(|| die("--key <idempotency key> is needed"))));
            request.set("reason", s(&reason.unwrap_or_else(|| die("--reason <reason> is needed"))));
            let op = call_with(at, key, &held, "POST", "/api/v1/operations", Some(&request));
            let id = op.str("id").unwrap_or("").to_string();
            if wait_flag { wait(at, key, &id) } else { println!("{}", op.text()) }
        }
        ["status", id] => println!("{}", call(at, key, "GET", &format!("/api/v1/operations?id={}", http::escape(id)), None).text()),
        ["wait", id] => wait(at, key, id),
        ["output", id] => print!("{}", call(at, key, "GET", &format!("/api/v1/operations/output?id={}", http::escape(id)), None).str("text").unwrap_or("")),
        ["list"] => {
            let mut path = format!("/api/v1/operations?status={}", if active { "active" } else { "all" });
            if let Some(a) = &agent { path += &format!("&agent_id={}", http::escape(a)); }
            let list = call(at, key, "GET", &path, None);
            let Value::Arr(ops) = list else { die("the daemon's answer is not a list") };
            for op in ops {
                println!("{}\t{}\t{}\t{}", op.str("id").unwrap_or(""), op.str("status").unwrap_or(""), op.str("kind").unwrap_or(""), op.str("reason").unwrap_or(""));
            }
        }
        ["cancel", id] => {
            let request = obj(vec![("operation_id", s(id)), ("idempotency_key", s(&idem.unwrap_or_else(|| die("--key <idempotency key> is needed")))),
                                   ("reason", s(&reason.unwrap_or_else(|| die("--reason <reason> is needed"))))]);
            println!("{}", call_with(at, key, &held, "POST", "/api/v1/operations/cancel", Some(&request)).text());
        }
        ["endpoints"] => println!("{}", call(at, key, "GET", "/api/v1/endpoints", None).text()),
        ["reserve"] => {
            let minutes = minutes.unwrap_or_else(|| die(format!("--minutes <1 to {LEASE_MINUTES_MAX}> is needed")));
            let mut request = obj(vec![("duration_minutes", Value::Num(minutes as f64)), ("reason", s(&reason.unwrap_or_else(|| die("--reason <reason> is needed"))))]);
            if let Some(o) = &owner { request.set("owner", s(o)); }
            if let Some(a) = &agent { request.set("agent_id", s(a)); }
            let v = call(at, key, "POST", "/api/v1/terminal-reservations", Some(&request));
            let l = v.get("reservation").cloned().unwrap_or(Value::Null);
            // Alone on the first line, for a script to keep.
            println!("{}:{}", l.str("session_id").unwrap_or(""), v.str("pin").unwrap_or(""));
            eprintln!("{}; renew it before then, release it when done", until(&l));
        }
        ["renew" | "release", rest @ ..] if rest.len() <= 1 => {
            let (session, pin) = rest.first().map(|r| pair(r)).or(lease.clone()).unwrap_or_else(|| die(format!("{} SESSION:PIN, or EFFECTOR485_RESERVATION", words[0])));
            let mut request = obj(vec![("reservation_session_id", s(&session)), ("reservation_pin", s(&pin))]);
            if words[0] == "release" {
                call(at, key, "DELETE", "/api/v1/terminal-reservations", Some(&request));
                return println!("{session}: released");
            }
            if let Some(m) = minutes { request.set("duration_minutes", Value::Num(m as f64)); }
            println!("{}", until(call(at, key, "PATCH", "/api/v1/terminal-reservations", Some(&request)).get("reservation").unwrap_or(&Value::Null)));
        }
        ["reservations"] => {
            let v = call(at, key, "GET", "/api/v1/terminal-reservations", None);
            let Some(Value::Arr(list)) = v.get("reservations") else { die("the daemon's answer has no list") };
            if list.is_empty() { eprintln!("the machine is not reserved"); }
            for l in list {
                println!("{}\t{}\t{}\tuntil {} UTC\t{} s left\t{}", l.str("session_id").unwrap_or(""), l.str("owner").unwrap_or(""), l.str("reason").unwrap_or(""),
                         utc(l.num("expires_at").unwrap_or(0.0) as u64), l.num("remaining_seconds").unwrap_or(0.0), l.str("agent_id").unwrap_or(""));
            }
        }
        _ => { eprintln!("{USAGE}"); std::process::exit(2); }
    }
}

/// A lease's end, as the command line says it.
fn until(l: &Value) -> String {
    let left = l.num("remaining_seconds").unwrap_or(0.0) as u64;
    format!("{}: held until {} UTC ({} min left)", l.str("session_id").unwrap_or(""), utc(l.num("expires_at").unwrap_or(0.0) as u64), left.div_ceil(60))
}

/// Shows an operation's output as it comes until it ends, then its end; exits with a program's exit code, or 1 when
/// the operation did not complete.
fn wait(at: &str, key: Option<&str>, id: &str) {
    let mut from = 0u64;
    loop {
        let out = call(at, key, "GET", &format!("/api/v1/operations/output?id={}&from={from}", http::escape(id)), None);
        let text = out.str("text").unwrap_or("");
        let start = out.num("from").unwrap_or(0.0) as u64;
        if start > from { eprintln!("[{} bytes of output were no longer held]", start - from); }
        print!("{text}");
        use std::io::Write;
        std::io::stdout().flush().ok();
        from = out.num("next").unwrap_or(0.0) as u64;
        if FINAL.contains(&out.str("status").unwrap_or("")) && text.is_empty() { break; }
        if text.is_empty() { std::thread::sleep(Duration::from_millis(500)); }
    }
    let op = call(at, key, "GET", &format!("/api/v1/operations?id={}", http::escape(id)), None);
    let status = op.str("status").unwrap_or("?");
    let result = op.get("result").cloned().unwrap_or(Value::Null);
    let exit = result.num("exit_code");
    eprintln!("{id}: {status}{}{}", exit.map_or(String::new(), |c| format!(", exit code {c}")),
              result.str("error").map_or(String::new(), |e| format!(": {e}")));
    match (status, exit) {
        ("completed", Some(c)) if c != 0.0 => std::process::exit((c as i32).clamp(1, 255)),
        ("completed", _) => {}
        _ => std::process::exit(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 800-LNK-0022: a cancel that makes one ended operation too many answers for that operation; the daemon then still
    // answers. Before, `save` pruned inside the route, the index moved, and the read past the end poisoned the lock.
    #[test]
    fn a_cancel_at_the_ended_limit() {
        let dir = std::env::temp_dir().join(format!("effector485-prune-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let mut st = State::load(&dir, "test", &[0]).unwrap();
        for n in 0..=500 {
            let r = json::parse(&format!(r#"{{"kind":"hello","idempotency_key":"k{n}","reason":"r"}}"#)).unwrap();
            let Ok(Submitted::New(i)) = st.submit(&r, None) else { panic!("submit {n}") };
            st.ops[i].finish("completed");
        }
        let r = json::parse(r#"{"kind":"mkdir","name":"x","idempotency_key":"last","reason":"r"}"#).unwrap();
        let Ok(Submitted::New(_)) = st.submit(&r, None) else { panic!("the queued one") };
        let id = st.ops.last().unwrap().id.clone();
        let state = Arc::new(Mutex::new(st));
        let ask = |method: &str, path: &str, body: &str| route(&state, &http::Request {
            method: method.into(), path: path.into(), query: Vec::new(), key: None, body: body.into(), origin: None, host: None,
            websocket: None, session: None, pin: None,
        });
        let (code, v) = ask("POST", "/api/v1/operations/cancel", &format!(r#"{{"operation_id":"{id}","idempotency_key":"c","reason":"r"}}"#));
        assert_eq!((code, v.str("id"), v.str("status")), (200, Some(id.as_str()), Some("cancelled")));
        // The worker prunes between operations, where no index is held.
        worker::lock(&state).prune();
        assert_eq!(worker::lock(&state).ops.iter().filter(|o| o.ended()).count(), 500);
        assert_eq!(ask("GET", "/api/v1/endpoints", "").0, 200);
        std::fs::remove_dir_all(&dir).ok();
    }

    // One request that panics while holding the state must not stop the daemon's other requests.
    #[test]
    fn a_poisoned_lock_is_taken_back() {
        let dir = std::env::temp_dir().join(format!("effector485-poison-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let state = Arc::new(Mutex::new(State::load(&dir, "test", &[0]).unwrap()));
        let s2 = state.clone();
        let _ = std::thread::spawn(move || { let _g = s2.lock().unwrap(); panic!("a failed request"); }).join();
        assert!(state.is_poisoned());
        assert_eq!(worker::lock(&state).ops.len(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }
}
