//! effector485 (800-LNK-0011): a small Effector over MIND Link, for local use. `effector485 daemon` holds the cable as
//! its one master and serves Effector's operations on 127.0.0.1 (docs/effector.md, section 3): each with an id, an
//! idempotency key and a reason; queued and carried out in order; read for status and output; cancelled by id. The
//! other commands are its client. docs/link/effector485.md says how to use it.
mod http;
mod json;
mod ops;
mod worker;
#[cfg(unix)]
#[path = "../../port_unix.rs"]
mod port;
#[cfg(windows)]
#[path = "../../port_windows.rs"]
mod port;

use json::{obj, s, Value};
use ops::{State, Submitted, FINAL, KINDS};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const AT: &str = "127.0.0.1:8485";

const USAGE: &str = "usage: effector485 daemon <port> [--baud <rate>] [--payload <bytes>] [--timeout <ms>] [--tries <n>]
                          [--listen <host:port>] [--state <dir>] [--key-file <file>]
       effector485 [--at <host:port>] [--key-file <file>] <command>

The daemon holds the cable (<port> as for mindlink: COM22, /dev/ttyUSB0, tcp:HOST:PORT) and serves operations on
--listen (default 127.0.0.1:8485). Its state, the queue and the results, is kept in --state (default: .effector485 in
the home directory). With --key-file, every call must carry that file's text in the X-API-Key header.

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
          endpoints                      the cable's other end: online, its name";

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
        let listen = take(&mut args, "--listen").unwrap_or_else(|| AT.to_string());
        let dir = take(&mut args, "--state").map(PathBuf::from).unwrap_or_else(home_state);
        let [_, spec] = &args[..] else { die(USAGE) };
        return daemon(spec.clone(), o, &listen, &dir, read_key(&key_file));
    }
    let at = take(&mut args, "--at").or_else(|| std::env::var("EFFECTOR485_AT").ok()).unwrap_or_else(|| AT.to_string());
    let key = read_key(&key_file);
    client(&at, key.as_deref(), args);
}

fn daemon(spec: String, o: worker::Options, listen: &str, dir: &std::path::Path, key: Option<String>) {
    let state = State::load(dir, &spec).unwrap_or_else(|e| die(e));
    let listener = TcpListener::bind(listen).unwrap_or_else(|e| die(format!("{listen}: {e}")));
    let bound = listener.local_addr().map(|a| a.to_string()).unwrap_or_else(|_| listen.to_string());
    if !bound.starts_with("127.") && !bound.starts_with("[::1]") && key.is_none() { die(format!("{bound} is reachable from other machines: give --key-file")); }
    // The first line says where it listens (the tests read it).
    println!("effector485 listening on {bound}, the cable on {spec}, the state in {}", dir.display());
    let state = Arc::new(Mutex::new(state));
    let worker_state = state.clone();
    std::thread::spawn(move || worker::work(worker_state, spec, o));
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let (state, key) = (state.clone(), key.clone());
        std::thread::spawn(move || {
            let request = match http::read_request(&mut stream) { Ok(r) => r, Err(e) => return http::respond(&mut stream, 400, "text/plain", &e) };
            let (status, body) = if key.is_some() && request.key != key { (401, error("the X-API-Key header is missing or wrong")) } else { route(&state, &request) };
            http::respond(&mut stream, status, "application/json", &body.text());
        });
    }
}

fn error(text: &str) -> Value { obj(vec![("error", s(text))]) }

fn route(state: &Arc<Mutex<State>>, r: &http::Request) -> (u16, Value) {
    let mut st = state.lock().unwrap();
    match (r.method.as_str(), r.path.as_str()) {
        ("GET", "/api/v1/api-key") => (200, obj(vec![("valid", Value::Bool(true))])),
        ("GET", "/api/v1/endpoints") => {
            let l = &st.link;
            let link = obj(vec![
                ("agent_id", s("link")), ("port", s(&l.port)), ("online", Value::Bool(l.online)), ("name", s(&l.name)),
                ("last_seen", Value::Num(l.last_seen as f64)), ("error", s(&l.error)), ("parallel_limit", Value::Num(1.0)),
                ("queued", Value::Num(st.queued() as f64)), ("running", Value::Num(st.running() as f64)),
            ]);
            (200, Value::Arr(vec![link]))
        }
        ("POST", "/api/v1/operations") => {
            let request = match json::parse(&r.body) { Ok(v) => v, Err(e) => return (400, error(&e)) };
            match st.submit(&request) {
                Ok(Submitted::New(i)) => { st.save(); (201, st.ops[i].json(st.position(i), false)) }
                Ok(Submitted::Again(i)) => (200, st.ops[i].json(st.position(i), false)),
                Err(e) => (400, error(&e)),
            }
        }
        ("GET", "/api/v1/operations") => {
            if let Some(id) = r.param("id") {
                return match st.find(id) { Some(i) => (200, st.ops[i].json(st.position(i), false)), None => (404, error(&format!("no operation {id}"))) };
            }
            let active = r.param("status") == Some("active");
            let list = (0..st.ops.len()).filter(|&i| !active || !st.ops[i].ended()).map(|i| {
                let mut v = st.ops[i].json(st.position(i), false);
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
            match st.cancel(id, key, reason) { Ok(i) => { st.save(); (200, st.ops[i].json(st.position(i), false)) } Err((code, e)) => (code, error(&e)) }
        }
        _ => (404, error(&format!("no route {} {}", r.method, r.path))),
    }
}

fn call(at: &str, key: Option<&str>, method: &str, path: &str, body: Option<&Value>) -> Value {
    let (status, text) = http::call(at, method, path, key, body.map(|b| b.text()).as_deref()).unwrap_or_else(|e| die(e));
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
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["submit", kind, fields @ ..] => {
            if !KINDS.iter().any(|(k, _)| k == kind) { die(format!("no kind {kind}: {}", KINDS.map(|k| k.0).join(", "))); }
            let mut request = obj(vec![("kind", s(kind))]);
            for f in fields {
                let (name, value) = f.split_once('=').unwrap_or_else(|| die(format!("{f}: a field is <name>=<value>")));
                request.set(name, field(name, value));
            }
            request.set("idempotency_key", s(&idem.unwrap_or_else(|| die("--key <idempotency key> is needed"))));
            request.set("reason", s(&reason.unwrap_or_else(|| die("--reason <reason> is needed"))));
            let op = call(at, key, "POST", "/api/v1/operations", Some(&request));
            let id = op.str("id").unwrap_or("").to_string();
            if wait_flag { wait(at, key, &id) } else { println!("{}", op.text()) }
        }
        ["status", id] => println!("{}", call(at, key, "GET", &format!("/api/v1/operations?id={}", http::escape(id)), None).text()),
        ["wait", id] => wait(at, key, id),
        ["output", id] => print!("{}", call(at, key, "GET", &format!("/api/v1/operations/output?id={}", http::escape(id)), None).str("text").unwrap_or("")),
        ["list"] => {
            let list = call(at, key, "GET", if active { "/api/v1/operations?status=active" } else { "/api/v1/operations" }, None);
            let Value::Arr(ops) = list else { die("the daemon's answer is not a list") };
            for op in ops {
                println!("{}\t{}\t{}\t{}", op.str("id").unwrap_or(""), op.str("status").unwrap_or(""), op.str("kind").unwrap_or(""), op.str("reason").unwrap_or(""));
            }
        }
        ["cancel", id] => {
            let request = obj(vec![("operation_id", s(id)), ("idempotency_key", s(&idem.unwrap_or_else(|| die("--key <idempotency key> is needed")))),
                                   ("reason", s(&reason.unwrap_or_else(|| die("--reason <reason> is needed"))))]);
            println!("{}", call(at, key, "POST", "/api/v1/operations/cancel", Some(&request)).text());
        }
        ["endpoints"] => println!("{}", call(at, key, "GET", "/api/v1/endpoints", None).text()),
        _ => { eprintln!("{USAGE}"); std::process::exit(2); }
    }
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
