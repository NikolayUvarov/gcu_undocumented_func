//! The cable's one master, with a worker for each machine on it (800-LNK-0026): each carries out its machine's queued
//! operations one at a time, and between them follows the programs they started: their output read, their end seen,
//! a cancel or a time limit turned into STOP. While a lease is held, only its holder's operations and reads start; the
//! programs started before it are still followed. The workers take the line in turn, one exchange each (bus.rs).
use crate::bus::{Bus, BusLine};
use crate::json::{s, Value};
use crate::ops::{now, State, RUN_TIMEOUT};
use linkproto::app::{Client, EXIT_UNKNOWN, RUN_FAULTED, RUN_RUNNING, RUN_STOPPED};
use linkproto::endpoint::{channel, code, kind};
use linkproto::frame::{MAX_PAYLOAD, MAX_WIRE};
use linkproto::Error;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub type Shared = Arc<Mutex<State>>;

/// The state, even when a thread panicked holding it: one failed request must not stop the daemon (800-LNK-0022).
pub fn lock(state: &Shared) -> std::sync::MutexGuard<'_, State> { state.lock().unwrap_or_else(|e| e.into_inner()) }

#[derive(Clone, Copy)]
pub struct Options { pub baud: u32, pub timeout: Option<u32>, pub tries: u32, pub payload: usize }

pub fn explain(e: Error) -> String {
    match e {
        Error::Timeout => "no answer from the other end (its port, rate or wiring, or it is down)".into(),
        Error::Line => "the line failed (closed, or the converter was pulled out)".into(),
        Error::TooBig => "too big for one frame".into(),
        Error::Protocol => "the other end answered out of protocol".into(),
        Error::Refused(c) => format!("refused: {} (code {c})", match c {
            code::NOT_FOUND => "no such file, directory or run",
            code::DENIED => "not allowed (a bad name, or on MIND Core a program without \"run\" in link.conf, keys without \"dev\" and init's input capability)",
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

/// How long the line may stay quiet between operations: then a POLL keeps the other end's mark lit (800-LNK-0020) and
/// finds a lost line before the next request does. The other end does not log it, as it logs a hello.
const KEEP_WARM: Duration = Duration::from_secs(2);

/// The line is gone or silent: the daemon says hello again before the next request.
fn line_lost(e: Error) -> bool { matches!(e, Error::Timeout | Error::Line) }

/// A client for the machine at `address`, its wait for an answer from the line's rate.
fn client(bus: &Arc<Bus>, address: u8, o: &Options) -> Client<BusLine> {
    let wait = o.timeout.unwrap_or(if bus.serial() { 200 + (4 * MAX_WIRE as u64 * 10 * 1000 / o.baud as u64) as u32 } else { 2000 });
    let mut c = Client::new(bus.line());
    c.master.timeout_ms = wait;
    c.master.tries = o.tries;
    c.master.address = address;
    c.max_payload = o.payload;
    c
}

/// A hello to the machine, with `tries`: its name and protocol, or why it did not answer.
fn connect(c: &mut Client<BusLine>, bus: &Bus, tries: u32) -> Result<String, String> {
    let all = std::mem::replace(&mut c.master.tries, tries);
    let mut theirs = [0u8; 64];
    let r = c.hello("effector485", &mut theirs);
    c.master.tries = all;
    match r {
        Ok((hello, n)) => Ok(format!("{}, protocol {}", String::from_utf8_lossy(&theirs[..n]), hello.version)),
        Err(Error::Line) if !bus.error().is_empty() => Err(bus.error()),
        Err(e) => Err(explain(e)),
    }
}

/// Runs `f` on operation `id`, if it is still kept.
fn with<T>(state: &Shared, id: &str, f: impl FnOnce(&mut crate::ops::Op) -> T) -> Option<T> {
    let mut st = lock(&state);
    let i = st.find(id)?;
    let r = f(&mut st.ops[i]);
    st.touch();
    Some(r)
}

/// The worker of machine `index` in the state's links: on a cable of one, the only one.
pub fn work(state: Shared, index: usize, bus: Arc<Bus>, o: Options) {
    let (agent, address, shared) = { let st = lock(&state); (st.links[index].agent.clone(), st.links[index].address, st.links.len() > 1) };
    // The log names the machine when the cable has several.
    let spec = if shared { format!("{} {agent}", bus.spec()) } else { bus.spec().to_string() };
    let mut c = client(&bus, address, &o);
    let mut online = false;
    let mut spoke = Instant::now();
    loop {
        if !online {
            // On a shared cable a machine that is not there is asked once each time, so it takes little of the others' line.
            match connect(&mut c, &bus, if shared { 1 } else { o.tries }) {
                Ok(name) => {
                    eprintln!("effector485: {spec}: {name}");
                    let mut st = lock(&state);
                    let l = &mut st.links[index];
                    (l.online, l.name, l.last_seen, l.error) = (true, name, now(), String::new());
                    st.changed();
                    online = true;
                    spoke = Instant::now();
                }
                Err(e) => {
                    let mut st = lock(&state);
                    let l = &mut st.links[index];
                    let news = l.error != e || l.online;
                    if l.error != e { eprintln!("effector485: {spec}: {e}"); }
                    (l.online, l.error) = (false, e);
                    if news { st.changed(); }
                    st.expire(now());
                    drop(st);
                    std::thread::sleep(Duration::from_secs(3));
                    continue;
                }
            }
        }
        let c = &mut c;
        let mut lost = None;
        let runs: Vec<String> = lock(&state).ops.iter().filter(|o| o.agent == agent && o.status == "running" && o.run.is_some()).map(|o| o.id.clone()).collect();
        for id in &runs {
            spoke = Instant::now();
            if let Err(e) = follow(c, &state, id) { if line_lost(e) { lost = Some(e); break; } }
        }
        if lost.is_none() {
            let next = {
                let mut st = lock(&state);
                // The first queued that a lease lets through: its holder's and reads overtake the others', which wait.
                let at = now();
                let first = (0..st.ops.len()).find(|&i| st.ops[i].status == "queued" && st.ops[i].agent == agent && st.may_run(&st.ops[i], at));
                let next = first.map(|i| {
                    let op = &mut st.ops[i];
                    (op.status, op.started) = ("running".into(), Some(now()));
                    (op.id.clone(), op.kind.clone(), op.params.clone())
                });
                if next.is_some() { st.touch(); }
                st.save();
                next
            };
            match next {
                Some((id, kind, params)) => {
                    eprintln!("effector485: {id}: {}", crate::ops::command(&kind, &params));
                    if let Err(e) = execute(c, &state, &id, &kind, &params) {
                        with(&state, &id, |op| if !op.ended() { op.fail(&explain(e)) });
                        if line_lost(e) || c.master.line.failed() { lost = Some(e); }
                    }
                    spoke = Instant::now();
                }
                None if runs.is_empty() && spoke.elapsed() >= KEEP_WARM => {
                    let mut answer = [0u8; 16];
                    if let Err(e) = c.master.call(channel::CONTROL, kind::POLL, &[], &mut answer) {
                        if line_lost(e) || c.master.line.failed() { lost = Some(e); }
                    }
                    spoke = Instant::now();
                }
                None => std::thread::sleep(Duration::from_millis(if runs.is_empty() { 200 } else { 100 })),
            }
        }
        let mut st = lock(&state);
        if let Some(e) = lost {
            let why = if c.master.line.failed() && !bus.error().is_empty() { bus.error() } else { explain(e) };
            eprintln!("effector485: {spec}: {why}");
            (st.links[index].online, st.links[index].error) = (false, why);
            st.changed();
            online = false;
        } else {
            st.links[index].last_seen = now();
        }
        st.expire(now());
        st.prune();
        st.save();
    }
}

/// A started program's new output, and its end; a cancel or the time limit stops it.
fn follow(c: &mut Client<BusLine>, state: &Shared, id: &str) -> Result<(), Error> {
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
            if o.len == 0 && o.state == RUN_FAULTED {
                op.result.set("exit_code", Value::Null);
                op.fail(&format!("the program ended by a fault, vector {}", o.exit));
            } else if o.len == 0 && o.state != RUN_RUNNING {
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
fn put(c: &mut Client<BusLine>, state: &Shared, id: &str, file: &Path, name: &str, force: bool) -> Result<Result<(), String>, Error> {
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

fn execute(c: &mut Client<BusLine>, state: &Shared, id: &str, kind: &str, p: &Value) -> Result<(), Error> {
    let name = p.str("name").unwrap_or("");
    let file = p.str("file").map(PathBuf::from);
    let note = |text: String| with(state, id, |op| op.print(text.as_bytes()));
    let set = |key: &str, v: Value| with(state, id, |op| op.result.set(key, v));
    match kind {
        "hello" => { let mut theirs = [0u8; 64]; let (h, n) = c.hello("effector485", &mut theirs)?; note(format!("{}, protocol {}, payload up to {}\n", String::from_utf8_lossy(&theirs[..n]), h.version, h.max_payload)); }
        "put" | "install" => {
            let file = file.unwrap();
            let base = file.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
            let target = if kind == "install" { format!("apps/{}", if name.is_empty() { &base } else { name }) } else if name.is_empty() { base } else { name.to_string() };
            // A put into a directory makes it first; an end without directories (before 800-LNK-0010) has none to make.
            if let Some((parent, _)) = target.rsplit_once('/') {
                match c.mkdir(parent) { Ok(()) | Err(Error::Refused(code::UNSUPPORTED)) => {}, Err(e) => return Err(e) }
            }
            if let Err(e) = put(c, state, id, &file, &target, kind == "install" || p.bool("force") == Some(true))? { with(state, id, |op| op.fail(&e)); return Ok(()); }
            set("name", s(&target));
            if kind == "install" { note(format!("installed as {target}: run it with {{\"kind\":\"run\",\"name\":\"{target}\"}}\n")); }
        }
        // A service started again from a file put first (as replace/<its name> unless "name"), or from one the other end
        // holds ("name"), unverified; or from its boot image with "back" (800-LNK-0016).
        "replace" => {
            let unit = p.str("unit").unwrap_or("");
            // Back to its boot image: no file, and the operation ends below as every other does.
            if p.bool("back") == Some(true) {
                c.replace(unit, "")?;
                note(format!("{unit}: started again from its boot image\n"));
            } else {
                let target = match (&file, name.is_empty()) {
                    (_, false) => name.to_string(),
                    (Some(f), true) => format!("replace/{}", f.file_name().and_then(|n| n.to_str()).unwrap_or("service.elf")),
                    (None, true) => { with(state, id, |op| op.fail("a replace needs \"file\", \"name\" or \"back\"")); return Ok(()); }
                };
                if let Some(f) = &file {
                    if let Some((parent, _)) = target.rsplit_once('/') {
                        match c.mkdir(parent) { Ok(()) | Err(Error::Refused(code::UNSUPPORTED)) => {}, Err(e) => return Err(e) }
                    }
                    if let Err(e) = put(c, state, id, f, &target, false)? { with(state, id, |op| op.fail(&e)); return Ok(()); }
                }
                c.replace(unit, &target)?;
                set("name", s(&target));
                note(format!("{unit}: started again from {target}, unverified\n"));
            }
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
        // The other end answers, then restarts: the daemon says hello again once it is back.
        "reboot" => c.reboot()?,
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
        // A line for the other end's shell (800-LNK-0015): followed as a program is.
        "shell" => {
            let run = c.command(p.str("line").unwrap_or(""))?;
            let limit = p.num("timeout_seconds").map_or(RUN_TIMEOUT, |t| t as u64);
            with(state, id, |op| {
                (op.run, op.cursor, op.deadline) = (Some(run), 0, Some(now() + limit));
                op.result.set("run", Value::Num(run as f64));
            });
            return Ok(());
        }
        // Keys on the other end's machine (800-LNK-0032): each chord or character in one INPUT, pace_ms apart.
        "key" | "type" => {
            let mut keyboard = mindlink::keys::Keyboard::new();
            let groups = if kind == "type" { keyboard.text(p.str("text").unwrap_or("")) }
                         else { p.str("names").unwrap_or("").split_whitespace().map(|n| keyboard.chord(n)).collect() };
            let groups = match groups { Ok(g) => g, Err(e) => { with(state, id, |op| op.fail(&e)); return Ok(()); } };
            let pace = p.num("pace_ms").map_or(20, |v| v.max(0.0) as u64);
            let (mut sent, max) = (0usize, c.input_max());
            for (i, group) in groups.iter().enumerate() {
                if cancelled(state, id) { with(state, id, |op| { op.result.set("events", Value::Num(sent as f64)); op.finish("cancelled"); }); return Ok(()); }
                if i > 0 && pace > 0 { std::thread::sleep(Duration::from_millis(pace)); }
                for frame in group.chunks(max) {
                    let posted = match c.input(frame) {
                        Err(Error::Refused(code::DENIED)) => {
                            with(state, id, |op| op.fail("the other end takes no keys: MIND Core needs dev in link.conf and init's input capability for link (800-LNK-0032)"));
                            return Ok(());
                        }
                        r => r? as usize,
                    };
                    sent += posted;
                    if posted < frame.len() { with(state, id, |op| op.fail(&format!("the other end posted {posted} of {} events; {sent} in all", frame.len()))); return Ok(()); }
                }
            }
            set("events", Value::Num(sent as f64));
            note(format!("{sent} events posted, {} {}\n", groups.len(), if kind == "type" { "characters" } else { "chords" }));
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
