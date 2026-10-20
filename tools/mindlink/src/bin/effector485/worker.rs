//! The cable's one master: queued operations carried out one at a time, and between them the programs they started
//! followed: their output read, their end seen, a cancel or a time limit turned into STOP.
use crate::json::{s, Value};
use crate::ops::{now, State, RUN_TIMEOUT};
use crate::port::Port;
use linkproto::app::{Client, EXIT_UNKNOWN, RUN_RUNNING, RUN_STOPPED};
use linkproto::endpoint::code;
use linkproto::frame::{MAX_PAYLOAD, MAX_WIRE};
use linkproto::Error;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub type Shared = Arc<Mutex<State>>;

pub struct Options { pub baud: u32, pub timeout: Option<u32>, pub tries: u32, pub payload: usize }

pub fn explain(e: Error) -> String {
    match e {
        Error::Timeout => "no answer from the other end (its port, rate or wiring, or it is down)".into(),
        Error::Line => "the line failed (closed, or the converter was pulled out)".into(),
        Error::TooBig => "too big for one frame".into(),
        Error::Protocol => "the other end answered out of protocol".into(),
        Error::Refused(c) => format!("refused: {} (code {c})", match c {
            code::NOT_FOUND => "no such file, directory or run",
            code::DENIED => "not allowed (a bad name, or on MIND Core a program without \"run\" in link.conf)",
            code::IO => "the other end's storage failed",
            code::MISMATCH => "SHA-256 does not match",
            code::BAD_REQUEST => "the other end did not take the request",
            code::BUSY => "the other end is busy",
            code::UNSUPPORTED => "the other end does not serve this",
            code::NOT_EMPTY => "the directory is not empty",
            code::EXISTS => "something of that name is there already",
            _ => "refused",
        }),
    }
}

/// The line is gone or silent: the daemon says hello again before the next request.
fn line_lost(e: Error) -> bool { matches!(e, Error::Timeout | Error::Line) }

fn connect(spec: &str, o: &Options) -> Result<(Client<Port>, String), String> {
    let mut port = Port::open(spec, o.baud)?;
    port.turnaround = Duration::ZERO;
    let serial = !spec.contains(':');
    if let Some(l) = port.latency_ms().filter(|&l| serial && l > 2) {
        eprintln!("effector485: {}: the FTDI latency timer is {l} ms, which slows every answer (mindlink hello says how to lower it)", port.name);
    }
    let wait = o.timeout.unwrap_or(if serial { 200 + (4 * MAX_WIRE as u64 * 10 * 1000 / o.baud as u64) as u32 } else { 2000 });
    let mut c = Client::new(port);
    c.master.timeout_ms = wait;
    c.master.tries = o.tries;
    c.max_payload = o.payload;
    let mut theirs = [0u8; 64];
    let (hello, n) = c.hello("effector485", &mut theirs).map_err(explain)?;
    Ok((c, format!("{}, protocol {}", String::from_utf8_lossy(&theirs[..n]), hello.version)))
}

/// Runs `f` on operation `id`, if it is still kept.
fn with<T>(state: &Shared, id: &str, f: impl FnOnce(&mut crate::ops::Op) -> T) -> Option<T> {
    let mut st = state.lock().unwrap();
    let i = st.find(id)?;
    let r = f(&mut st.ops[i]);
    st.touch();
    Some(r)
}

pub fn work(state: Shared, spec: String, o: Options) {
    let mut link: Option<Client<Port>> = None;
    loop {
        if link.is_none() {
            match connect(&spec, &o) {
                Ok((c, name)) => {
                    eprintln!("effector485: {spec}: {name}");
                    let mut st = state.lock().unwrap();
                    (st.link.online, st.link.name, st.link.last_seen, st.link.error) = (true, name, now(), String::new());
                    link = Some(c);
                }
                Err(e) => {
                    let mut st = state.lock().unwrap();
                    if st.link.error != e { eprintln!("effector485: {spec}: {e}"); }
                    (st.link.online, st.link.error) = (false, e);
                    drop(st);
                    std::thread::sleep(Duration::from_secs(3));
                    continue;
                }
            }
        }
        let c = link.as_mut().unwrap();
        let mut lost = None;
        let runs: Vec<String> = state.lock().unwrap().ops.iter().filter(|o| o.status == "running" && o.run.is_some()).map(|o| o.id.clone()).collect();
        for id in &runs {
            if let Err(e) = follow(c, &state, id) { if line_lost(e) { lost = Some(e); break; } }
        }
        if lost.is_none() {
            let next = {
                let mut st = state.lock().unwrap();
                let next = st.ops.iter_mut().find(|o| o.status == "queued").map(|op| {
                    (op.status, op.started) = ("running".into(), Some(now()));
                    (op.id.clone(), op.kind.clone(), op.params.clone())
                });
                st.touch();
                st.save();
                next
            };
            match next {
                Some((id, kind, params)) => {
                    eprintln!("effector485: {id}: {kind}");
                    if let Err(e) = execute(c, &state, &id, &kind, &params) {
                        with(&state, &id, |op| if !op.ended() { op.fail(&explain(e)) });
                        if line_lost(e) || c.master.line.failed() { lost = Some(e); }
                    }
                }
                None => std::thread::sleep(Duration::from_millis(if runs.is_empty() { 200 } else { 100 })),
            }
        }
        let mut st = state.lock().unwrap();
        if let Some(e) = lost {
            eprintln!("effector485: {spec}: {}", explain(e));
            (st.link.online, st.link.error) = (false, explain(e));
            link = None;
        } else {
            st.link.last_seen = now();
        }
        st.save();
    }
}

/// A started program's new output, and its end; a cancel or the time limit stops it.
fn follow(c: &mut Client<Port>, state: &Shared, id: &str) -> Result<(), Error> {
    let Some((run, cursor, cancel, late)) = with(state, id, |op| (op.run.unwrap(), op.cursor, op.cancel.is_some(), op.deadline.is_some_and(|d| now() >= d))) else { return Ok(()) };
    if cancel || late {
        let why = if cancel { "cancel" } else { "timeout" };
        match c.stop(run) {
            Ok(()) => { with(state, id, |op| { op.result.set("stopped_by", s(why)); op.cancel = None; op.deadline = None; }); }
            Err(Error::Refused(code::UNSUPPORTED)) => {
                let note = "the other end cannot stop programs yet (MIND Core: 800-KRN-0132, point 3): it may still run";
                with(state, id, |op| {
                    if late { op.result.set("error", s(note)); op.finish("timeout"); } else { op.result.set("cancel_error", s(note)); op.cancel = None; }
                });
                if late { return Ok(()); }
            }
            Err(e) => return Err(e),
        }
    }
    let mut cursor = cursor;
    let mut buf = [0u8; MAX_PAYLOAD];
    for _ in 0..16 {
        let o = match c.output(run, cursor, &mut buf) {
            Ok(o) => o,
            Err(Error::Refused(code::NOT_FOUND)) => {
                with(state, id, |op| { op.fail("the other end no longer knows this run (it restarted, or forgot the run)"); op.status = "lost".into(); });
                return Ok(());
            }
            Err(e) => return Err(e),
        };
        with(state, id, |op| {
            if o.from > cursor { op.print(format!("\n[{} bytes of output were no longer held]\n", o.from - cursor).as_bytes()); }
            op.print(&buf[..o.len]);
            op.cursor = o.from + o.len as u64;
            if o.len == 0 && o.state != RUN_RUNNING {
                op.result.set("exit_code", if o.exit == EXIT_UNKNOWN { Value::Null } else { Value::Num(o.exit as i32 as f64) });
                let stopped_by = op.result.str("stopped_by").map(str::to_string);
                op.finish(match (o.state, stopped_by.as_deref()) { (RUN_STOPPED, Some("timeout")) => "timeout", (RUN_STOPPED, _) => "cancelled", _ => "completed" });
            }
        });
        cursor = o.from + o.len as u64;
        if o.len == 0 { break; }
    }
    Ok(())
}

fn cancelled(state: &Shared, id: &str) -> bool { with(state, id, |op| op.cancel.is_some()).unwrap_or(true) }

/// A put; skipped when the other end holds the same bytes, unless forced. Inside, this machine's own failure.
fn put(c: &mut Client<Port>, state: &Shared, id: &str, file: &Path, name: &str, force: bool) -> Result<Result<(), String>, Error> {
    let mut f = match File::open(file) { Ok(f) => f, Err(e) => return Ok(Err(format!("{}: {e}", file.display()))) };
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut sha = linkproto::sha256::Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop { let n = f.read(&mut buf).unwrap_or(0); if n == 0 { break; } sha.update(&buf[..n]); }
    let digest = sha.finish();
    if !force {
        let wait = c.master.timeout_ms;
        c.master.timeout_ms = wait.max(15_000);
        let theirs = c.stat(name);
        c.master.timeout_ms = wait;
        if theirs == Ok((size, digest)) {
            with(state, id, |op| { op.result.set("sent", Value::Bool(false)); op.print(format!("{name}: the other end holds these {size} bytes already\n").as_bytes()); });
            return Ok(Ok(()));
        }
    }
    let mut aborted = false;
    let r = c.put(name, size, |offset, out| {
        if cancelled(state, id) { aborted = true; return 0; }
        f.seek(SeekFrom::Start(offset)).ok();
        f.read(out).unwrap_or(0)
    });
    if aborted { with(state, id, |op| op.finish("cancelled")); return Ok(Ok(())); }
    let (sent, resumed) = r?;
    with(state, id, |op| {
        op.result.set("sent", Value::Bool(true));
        op.result.set("resumed_from", Value::Num(resumed as f64));
        if sent != digest { op.print(b"the file changed while it was sent\n"); }
    });
    Ok(Ok(()))
}

fn execute(c: &mut Client<Port>, state: &Shared, id: &str, kind: &str, p: &Value) -> Result<(), Error> {
    let name = p.str("name").unwrap_or("");
    let file = p.str("file").map(PathBuf::from);
    let note = |text: String| with(state, id, |op| op.print(text.as_bytes()));
    let set = |key: &str, v: Value| with(state, id, |op| op.result.set(key, v));
    match kind {
        "hello" => { let mut theirs = [0u8; 64]; let (h, n) = c.hello("effector485", &mut theirs)?; note(format!("{}, protocol {}, payload up to {}\n", String::from_utf8_lossy(&theirs[..n]), h.version, h.max_payload)); }
        "put" | "install" => {
            let file = file.unwrap();
            let base = file.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
            let target = if kind == "install" {
                c.mkdir("apps")?;
                format!("apps/{}", if name.is_empty() { &base } else { name })
            } else if name.is_empty() { base } else { name.to_string() };
            if let Err(e) = put(c, state, id, &file, &target, kind == "install" || p.bool("force") == Some(true))? { with(state, id, |op| op.fail(&e)); return Ok(()); }
            set("name", s(&target));
            if kind == "install" { note(format!("installed as {target}: run it with {{\"kind\":\"run\",\"name\":\"{target}\"}}\n")); }
        }
        "get" => {
            let file = file.unwrap();
            let part = PathBuf::from(format!("{}.part", file.display()));
            let mut out = match File::create(&part) { Ok(f) => f, Err(e) => { with(state, id, |op| op.fail(&format!("{}: {e}", part.display()))); return Ok(()); } };
            let mut aborted = false;
            let r = c.get(name, |_, data| { if cancelled(state, id) { aborted = true; return false; } out.write_all(data).is_ok() });
            if aborted { std::fs::remove_file(&part).ok(); with(state, id, |op| op.finish("cancelled")); return Ok(()); }
            let (size, digest) = match r { Ok(v) => v, Err(e) => { std::fs::remove_file(&part).ok(); return Err(e); } };
            out.sync_all().ok();
            if let Err(e) = std::fs::rename(&part, &file) { with(state, id, |op| op.fail(&format!("{}: {e}", file.display()))); return Ok(()); }
            set("size", Value::Num(size as f64));
            set("sha256", s(&digest.iter().map(|b| format!("{b:02x}")).collect::<String>()));
        }
        "list" => {
            let mut buf = [0u8; MAX_PAYLOAD];
            let (mut cursor, mut old) = (0, false);
            loop {
                let (n, next) = match if old { c.list(cursor, &mut buf) } else { c.list_dir(name, cursor, &mut buf) } {
                    Err(Error::Refused(code::UNSUPPORTED)) if !old && name.is_empty() => { old = true; continue; }
                    r => r?,
                };
                note(String::from_utf8_lossy(&buf[..n]).into_owned());
                match next { Some(n) => cursor = n, None => break }
            }
        }
        "stat" => {
            let wait = c.master.timeout_ms;
            c.master.timeout_ms = wait.max(15_000);
            let r = c.stat(name);
            c.master.timeout_ms = wait;
            let (size, digest) = r?;
            set("size", Value::Num(size as f64));
            set("sha256", s(&digest.iter().map(|b| format!("{b:02x}")).collect::<String>()));
        }
        "mkdir" => c.mkdir(name)?,
        "move" => c.rename(p.str("from").unwrap(), p.str("to").unwrap(), p.bool("replace") == Some(true))?,
        "delete" => c.delete(name)?,
        "uninstall" => c.delete(&format!("apps/{name}"))?,
        "rmdir" => c.rmdir(name, p.bool("recursive") == Some(true))?,
        "run" => {
            let (run, pid) = c.run(name, p.str("args").unwrap_or(""))?;
            let limit = p.num("timeout_seconds").map_or(RUN_TIMEOUT, |t| t as u64);
            with(state, id, |op| {
                (op.run, op.cursor, op.deadline) = (Some(run), 0, Some(now() + limit));
                op.result.set("run", Value::Num(run as f64));
                op.result.set("pid", Value::Num(pid as f64));
            });
            // It stays running: `work` follows it between the other operations.
            return Ok(());
        }
        "log" => {
            let (mut cursor, max) = (p.num("from").unwrap_or(0.0) as u64, p.num("max_bytes").unwrap_or(65536.0) as u64);
            let (mut got, mut buf) = (0u64, [0u8; MAX_PAYLOAD]);
            while got < max {
                let (n, next) = c.log(cursor, &mut buf)?;
                cursor = next;
                if n == 0 { break; }
                note(String::from_utf8_lossy(&buf[..n]).into_owned());
                got += n as u64;
            }
            set("next", Value::Num(cursor as f64));
        }
        _ => { with(state, id, |op| op.fail("no such kind")); return Ok(()); }
    }
    with(state, id, |op| if !op.ended() { op.finish("completed") });
    Ok(())
}
