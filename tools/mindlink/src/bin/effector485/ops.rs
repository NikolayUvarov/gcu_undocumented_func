//! effector485's operations, as on Effector (docs/effector.md): each with an id, an idempotency key and a reason; queued,
//! run one at a time over the cable, read for their status and output, cancelled by id. They are kept in a state file,
//! so a restarted daemon finds its queue and follows the programs it had started. A lease (800-LNK-0021) holds the
//! machine for one session: the others' operations that change it wait, reads pass.
use crate::json::{obj, s, Value};
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// The statuses an operation ends in.
pub const FINAL: [&str; 5] = ["completed", "failed", "cancelled", "timeout", "lost"];
/// The output kept for each operation: its last bytes.
pub const OUTPUT_KEPT: usize = 256 * 1024;
/// A program's limit when the request names none, and the largest.
pub const RUN_TIMEOUT: u64 = 600;
pub const RUN_TIMEOUT_MAX: u64 = 86_400;
/// Ended operations kept in the state file; the oldest go first.
const ENDED_KEPT: usize = 500;

/// The kinds, and the fields each needs besides `kind`, `idempotency_key` and `reason`.
pub const KINDS: [(&str, &[&str]); 16] = [
    ("hello", &[]), ("put", &["file"]), ("get", &["name", "file"]), ("list", &[]), ("stat", &["name"]), ("mkdir", &["name"]),
    ("move", &["from", "to"]), ("delete", &["name"]), ("rmdir", &["name"]), ("run", &["name"]), ("install", &["file"]),
    ("uninstall", &["name"]), ("log", &[]), ("reboot", &[]), ("shell", &["line"]), ("replace", &["unit"]),
];
/// The kinds that only read the other end: they pass a lease that is not theirs.
pub const READS: [&str; 5] = ["hello", "log", "list", "stat", "get"];
/// A lease's longest term in minutes; it may be renewed without limit.
pub const LEASE_MINUTES_MAX: u64 = 60;

pub fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

pub struct Op {
    pub id: String,
    pub kind: String,
    /// The request as submitted.
    pub params: Value,
    pub key: String,
    pub reason: String,
    pub status: String,
    pub created: u64,
    pub started: Option<u64>,
    pub finished: Option<u64>,
    /// The last OUTPUT_KEPT bytes of what it printed or listed, and how many there were in all.
    pub output: Vec<u8>,
    pub output_bytes: u64,
    /// What it came to: exit_code, error, run, pid, size, sha256 and the like.
    pub result: Value,
    /// A cancel asked for and not yet carried out: its key and reason.
    pub cancel: Option<(String, String)>,
    /// A program it started: the other end's run, the output read so far, when it must end.
    pub run: Option<u32>,
    pub cursor: u64,
    pub deadline: Option<u64>,
    /// The lease it was sent under: while that lasts, it runs when other sessions' operations wait.
    pub session: Option<String>,
}

impl Op {
    pub fn ended(&self) -> bool { FINAL.contains(&self.status.as_str()) }

    pub fn print(&mut self, bytes: &[u8]) {
        self.output.extend_from_slice(bytes);
        self.output_bytes += bytes.len() as u64;
        let over = self.output.len().saturating_sub(OUTPUT_KEPT);
        self.output.drain(..over);
    }

    pub fn finish(&mut self, status: &str) {
        self.status = status.to_string();
        self.finished = Some(now());
        self.cancel = None;
    }

    pub fn fail(&mut self, error: &str) {
        self.result.set("error", s(error));
        self.finish("failed");
    }

    /// As the API answers it; `position` in the queue while queued (1 is next).
    pub fn json(&self, position: Option<usize>, full: bool) -> Value {
        let time = |t: Option<u64>| t.map_or(Value::Null, |t| Value::Num(t as f64));
        let mut result = self.result.clone();
        let text = String::from_utf8_lossy(&self.output);
        // The API answers the output's end; the state file keeps all of it.
        let shown = if full { &text[..] } else { tail(&text, 64 * 1024) };
        result.set("stdout", s(shown));
        result.set("output_bytes", Value::Num(self.output_bytes as f64));
        let mut v = obj(vec![
            ("id", s(&self.id)), ("kind", s(&self.kind)), ("status", s(&self.status)), ("idempotency_key", s(&self.key)),
            ("reason", s(&self.reason)), ("params", self.params.clone()), ("created_at", Value::Num(self.created as f64)),
            ("started_at", time(self.started)), ("finished_at", time(self.finished)), ("result", result),
        ]);
        if let Some(p) = position { v.set("queue_position", Value::Num(p as f64)); }
        if let Some((key, reason)) = &self.cancel { v.set("cancel", obj(vec![("idempotency_key", s(key)), ("reason", s(reason))])); }
        if let Some(session) = &self.session { v.set("reservation_session_id", s(session)); }
        if full {
            if let Some(run) = self.run { v.set("run", Value::Num(run as f64)); v.set("cursor", Value::Num(self.cursor as f64)); }
            if let Some(d) = self.deadline { v.set("deadline", Value::Num(d as f64)); }
        }
        v
    }

    fn from_json(v: &Value) -> Option<Op> {
        let num = |k: &str| v.num(k).map(|n| n as u64);
        let mut result = v.get("result").cloned().unwrap_or(Value::Obj(Vec::new()));
        let stdout = result.str("stdout").unwrap_or("").as_bytes().to_vec();
        let bytes = result.num("output_bytes").unwrap_or(stdout.len() as f64) as u64;
        if let Value::Obj(fields) = &mut result { fields.retain(|(k, _)| k != "stdout" && k != "output_bytes"); }
        Some(Op {
            id: v.str("id")?.to_string(), kind: v.str("kind")?.to_string(), params: v.get("params")?.clone(),
            key: v.str("idempotency_key")?.to_string(), reason: v.str("reason").unwrap_or("").to_string(),
            status: v.str("status")?.to_string(), created: num("created_at").unwrap_or(0), started: num("started_at"),
            finished: num("finished_at"), output: stdout, output_bytes: bytes, result,
            cancel: v.get("cancel").and_then(|c| Some((c.str("idempotency_key")?.to_string(), c.str("reason")?.to_string()))),
            run: num("run").map(|r| r as u32), cursor: num("cursor").unwrap_or(0), deadline: num("deadline"),
            session: v.str("reservation_session_id").map(str::to_string),
        })
    }
}

/// An operation as a command, for the daemon's log: its kind and fields whole, but for the secret a shell line's
/// `tpm seal` takes, which stays out of the log as it does in MIND Core's (link/src/main.rs).
pub fn command(kind: &str, params: &Value) -> String {
    let mut out = kind.to_string();
    if let Value::Obj(fields) = params {
        for (name, value) in fields {
            let text = match value { Value::Str(v) => v.clone(), v => v.text() };
            let text = if kind == "shell" && name == "line" { sealed(&text) } else { text };
            out.push_str(&format!(" {name}={text}"));
        }
    }
    out
}

fn sealed(line: &str) -> String {
    let mut words = line.split_whitespace();
    match (words.next(), words.next()) {
        (Some(a), Some(b)) if a.eq_ignore_ascii_case("tpm") && b.eq_ignore_ascii_case("seal") => format!("{a} {b} [THE SECRET IS NOT LOGGED]"),
        _ => line.to_string(),
    }
}

/// The last `max` bytes of `text`, cut at a character's start.
pub fn tail(text: &str, max: usize) -> &str {
    let mut from = text.len().saturating_sub(max);
    while !text.is_char_boundary(from) { from += 1; }
    &text[from..]
}

/// The alphabet of a lease's session and PIN: no 0, O, 1 or I to misread.
const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// `n` characters of ALPHABET from SipHash under the keys std draws from the OS for each RandomState.
fn random(n: usize) -> String {
    static DRAWN: AtomicU64 = AtomicU64::new(0);
    let mut out = String::new();
    while out.len() < n {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(DRAWN.fetch_add(1, Ordering::Relaxed));
        h.write_u128(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        let mut v = h.finish();
        for _ in 0..12.min(n - out.len()) { out.push(ALPHABET[(v % 32) as usize] as char); v /= 32; }
    }
    out
}

/// Equal strings, compared in a time that does not depend on where they differ.
fn same(a: &str, b: &str) -> bool { a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0, |d, (x, y)| d | (x ^ y)) == 0 }

/// A time of day in UTC, HH:MM:SS.
pub fn utc(t: u64) -> String { format!("{:02}:{:02}:{:02}", t / 3600 % 24, t / 60 % 60, t % 60) }

/// A lease on the machine (800-LNK-0021), as on the Effector server's terminal reservations, but for two things: the
/// others' operations wait rather than being refused, and the lease has a reason everyone sees.
#[derive(Clone, Debug, PartialEq)]
pub struct Lease { pub session: String, pub pin: String, pub owner: String, pub reason: String, pub created: u64, pub expires: u64, pub minutes: u64, pub renewals: u64 }

impl Lease {
    /// As the API answers it: never with its PIN.
    pub fn json(&self, now: u64) -> Value {
        obj(vec![
            ("session_id", s(&self.session)), ("owner", s(&self.owner)), ("reason", s(&self.reason)),
            ("created_at", Value::Num(self.created as f64)), ("expires_at", Value::Num(self.expires as f64)),
            ("remaining_seconds", Value::Num(self.expires.saturating_sub(now) as f64)),
            ("duration_minutes", Value::Num(self.minutes as f64)), ("renewal_attempts", Value::Num(self.renewals as f64)),
        ])
    }

    fn from_json(v: &Value) -> Option<Lease> {
        let num = |k: &str| v.num(k).map(|n| n as u64);
        Some(Lease {
            session: v.str("session_id")?.to_string(), pin: v.str("pin")?.to_string(), owner: v.str("owner").unwrap_or("").to_string(),
            reason: v.str("reason").unwrap_or("").to_string(), created: num("created_at")?, expires: num("expires_at")?,
            minutes: num("duration_minutes").unwrap_or(LEASE_MINUTES_MAX), renewals: num("renewal_attempts").unwrap_or(0),
        })
    }

    pub fn holder(&self) -> &str { if self.owner.is_empty() { "a session" } else { &self.owner } }

    /// Why another session's operation waits, and why another lease is refused.
    pub fn waiting(&self) -> Value {
        let message = format!("the machine is reserved by {} ({}) until {} UTC ({})", self.holder(), self.reason, utc(self.expires), self.session);
        obj(vec![("code", s("terminal_reserved")), ("message", s(&message))])
    }
}

/// What the daemon knows of the cable's other end.
#[derive(Default)]
pub struct Link { pub port: String, pub online: bool, pub name: String, pub last_seen: u64, pub error: String }

/// `changes` counts the changes, so that the page's feeds see one without taking the lock.
/// `lease` is kept in reservation.json beside the operations.
pub struct State { pub ops: Vec<Op>, next: u64, file: PathBuf, pub link: Link, dirty: bool, pub changes: Arc<AtomicU64>, pub lease: Option<Lease> }

pub enum Submitted { New(usize), Again(usize) }

impl State {
    /// The state kept in `dir`: queued operations wait again; a program being followed is followed again; anything
    /// else that was running when the daemon stopped is lost.
    pub fn load(dir: &Path, port: &str) -> Result<State, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let file = dir.join("operations.json");
        let mut ops = Vec::new();
        if file.exists() {
            let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let Value::Arr(items) = crate::json::parse(&text).map_err(|e| format!("{}: {e}", file.display()))? else { return Err(format!("{}: not a list", file.display())) };
            ops = items.iter().filter_map(Op::from_json).collect();
        }
        for op in ops.iter_mut().filter(|o| o.status == "running" && o.run.is_none()) {
            op.fail("the daemon stopped while this ran; whether it finished on the other end is not known");
            op.status = "lost".into();
        }
        let next = ops.iter().filter_map(|o| o.id.rsplit('-').next()?.parse::<u64>().ok()).max().unwrap_or(0) + 1;
        // A lease outlives a restart; one whose time is up ends at the first look.
        let leased = dir.join("reservation.json");
        let lease = if leased.exists() {
            let text = std::fs::read_to_string(&leased).map_err(|e| format!("{}: {e}", leased.display()))?;
            let v = crate::json::parse(&text).map_err(|e| format!("{}: {e}", leased.display()))?;
            Some(Lease::from_json(&v).ok_or_else(|| format!("{}: not a reservation", leased.display()))?)
        } else { None };
        Ok(State { ops, next, file, link: Link { port: port.to_string(), ..Link::default() }, dirty: false, changes: Arc::default(), lease })
    }

    /// Writes the state file when something changed: a new file renamed over the old, so a crash leaves one whole.
    pub fn save(&mut self) {
        if !self.dirty { return; }
        let text = Value::Arr(self.ops.iter().map(|o| o.json(None, true)).collect()).text();
        let part = self.file.with_extension("json.part");
        if std::fs::write(&part, text).and_then(|_| std::fs::rename(&part, &self.file)).is_ok() { self.dirty = false; }
        else { eprintln!("effector485: {} could not be written", self.file.display()); }
    }

    /// The oldest ended operations dropped beyond ENDED_KEPT. It moves every later index, so only the worker calls it,
    /// between operations, where no index is held: a route that pruned inside `save` read past the end and poisoned
    /// the state's lock (800-LNK-0022).
    pub fn prune(&mut self) {
        let ended: Vec<usize> = (0..self.ops.len()).filter(|&i| self.ops[i].ended()).collect();
        if ended.len() <= ENDED_KEPT { return; }
        let drop: std::collections::HashSet<usize> = ended[..ended.len() - ENDED_KEPT].iter().copied().collect();
        let mut i = 0;
        self.ops.retain(|_| { i += 1; !drop.contains(&(i - 1)) });
        self.touch();
    }

    pub fn touch(&mut self) { self.dirty = true; self.changed(); }

    /// A change the state file does not keep: the cable's other end came or went.
    pub fn changed(&self) { self.changes.fetch_add(1, Ordering::Release); }

    pub fn position(&self, index: usize) -> Option<usize> {
        (self.ops[index].status == "queued").then(|| self.ops[..=index].iter().filter(|o| o.status == "queued").count())
    }

    pub fn find(&self, id: &str) -> Option<usize> { self.ops.iter().position(|o| o.id == id) }

    /// An operation as the API answers it: its place in the queue, and why it waits while a lease holds it back.
    pub fn view(&self, index: usize, now: u64) -> Value {
        let mut v = self.ops[index].json(self.position(index), false);
        if let Some(w) = self.waiting(&self.ops[index], now) { v.set("waiting", w); }
        v
    }

    /// A new operation from a request, or the one its idempotency key already made; `session` is the lease the request
    /// holds.
    pub fn submit(&mut self, request: &Value, session: Option<String>) -> Result<Submitted, String> {
        let kind = request.str("kind").ok_or("\"kind\" is missing")?;
        let key = request.str("idempotency_key").filter(|k| !k.is_empty()).ok_or("\"idempotency_key\" is missing")?;
        let reason = request.str("reason").filter(|r| !r.is_empty()).ok_or("\"reason\" is missing")?;
        if let Some(i) = self.ops.iter().position(|o| o.key == key) {
            if self.ops[i].kind != kind { return Err(format!("the idempotency key {key} was used for a {} operation", self.ops[i].kind)); }
            return Ok(Submitted::Again(i));
        }
        let needs = KINDS.iter().find(|(k, _)| *k == kind).ok_or_else(|| format!("no kind {kind:?}: {}", KINDS.map(|k| k.0).join(", ")))?.1;
        for field in needs { if request.str(field).is_none_or(str::is_empty) { return Err(format!("a {kind} needs \"{field}\"")); } }
        for field in ["file"] {
            if let Some(f) = request.str(field) { if !Path::new(f).is_absolute() { return Err(format!("\"{field}\" must be an absolute path on this machine")); } }
        }
        if let Some(t) = request.num("timeout_seconds") { if t < 1.0 || t > RUN_TIMEOUT_MAX as f64 { return Err(format!("\"timeout_seconds\" is 1 to {RUN_TIMEOUT_MAX}")); } }
        let mut params = request.clone();
        if let Value::Obj(fields) = &mut params { fields.retain(|(k, _)| !matches!(k.as_str(), "kind" | "idempotency_key" | "reason")); }
        let id = format!("e485-{:x}-{}", now(), self.next);
        self.next += 1;
        self.ops.push(Op {
            id, kind: kind.into(), params, key: key.into(), reason: reason.into(), status: "queued".into(), created: now(),
            started: None, finished: None, output: Vec::new(), output_bytes: 0, result: Value::Obj(Vec::new()), cancel: None,
            run: None, cursor: 0, deadline: None, session,
        });
        self.touch();
        Ok(Submitted::New(self.ops.len() - 1))
    }

    /// Asks operation `id` to be cancelled: a queued one is at once, a running one by the worker.
    pub fn cancel(&mut self, id: &str, key: &str, reason: &str) -> Result<usize, (u16, String)> {
        let i = self.find(id).ok_or((404, format!("no operation {id}")))?;
        let op = &mut self.ops[i];
        if op.ended() { return Ok(i); }
        if op.status == "queued" {
            op.result.set("cancel_reason", s(reason));
            op.finish("cancelled");
        } else {
            op.cancel = Some((key.to_string(), reason.to_string()));
        }
        self.touch();
        Ok(i)
    }

    pub fn queued(&self) -> usize { self.ops.iter().filter(|o| o.status == "queued").count() }
    pub fn running(&self) -> usize { self.ops.iter().filter(|o| o.status == "running").count() }

    /// The lease, while its time lasts.
    pub fn lease_at(&self, now: u64) -> Option<&Lease> { self.lease.as_ref().filter(|l| l.expires > now) }

    /// Whether `op` may start now: no lease, the holder's own, or a read.
    pub fn may_run(&self, op: &Op, now: u64) -> bool {
        self.lease_at(now).is_none_or(|l| READS.contains(&op.kind.as_str()) || op.session.as_deref() == Some(l.session.as_str()))
    }

    /// Why a queued operation waits, when a lease that is not its own holds it back.
    pub fn waiting(&self, op: &Op, now: u64) -> Option<Value> {
        if op.status != "queued" || self.may_run(op, now) { return None; }
        self.lease_at(now).map(Lease::waiting)
    }

    /// A new lease of `minutes`, or the one held already.
    pub fn reserve(&mut self, minutes: u64, reason: &str, owner: &str, now: u64) -> Result<Lease, Lease> {
        self.expire(now);
        if let Some(l) = &self.lease { return Err(l.clone()); }
        let l = Lease { session: format!("R-{}", random(8)), pin: random(12), owner: owner.into(), reason: reason.into(), created: now,
                        expires: now + minutes * 60, minutes, renewals: 0 };
        eprintln!("effector485: the reservation {} of {} began, until {} UTC: {}", l.session, l.holder(), utc(l.expires), l.reason);
        self.lease = Some(l.clone());
        self.lease_changed();
        Ok(l)
    }

    /// Whether the lease `session` is held, with `pin`.
    pub fn holds(&self, session: &str, pin: &str, now: u64) -> Result<(), String> {
        match self.lease_at(now) {
            Some(l) if l.session == session && same(&l.pin, pin) => Ok(()),
            Some(l) if l.session == session => Err(format!("the PIN of the reservation {session} is wrong")),
            _ => Err(format!("the reservation {session} is not held (expired or released)")),
        }
    }

    /// The lease made to last `minutes` (its own term when none) from now.
    pub fn renew(&mut self, session: &str, pin: &str, minutes: Option<u64>, now: u64) -> Result<Lease, String> {
        self.holds(session, pin, now)?;
        let l = self.lease.as_mut().unwrap();
        l.minutes = minutes.unwrap_or(l.minutes);
        (l.expires, l.renewals) = (now + l.minutes * 60, l.renewals + 1);
        let l = l.clone();
        self.lease_changed();
        Ok(l)
    }

    pub fn release(&mut self, session: &str, pin: &str, now: u64) -> Result<Lease, String> {
        self.holds(session, pin, now)?;
        Ok(self.end("released").unwrap())
    }

    /// Ends a lease whose time is up.
    pub fn expire(&mut self, now: u64) {
        if self.lease.as_ref().is_some_and(|l| l.expires <= now) { self.end("expired"); }
    }

    fn end(&mut self, how: &str) -> Option<Lease> {
        let l = self.lease.take()?;
        eprintln!("effector485: the reservation {} of {} ended: {how}", l.session, l.holder());
        self.lease_changed();
        Some(l)
    }

    /// Writes reservation.json as save() writes the operations, or removes it with the lease; the feeds see the change.
    fn lease_changed(&mut self) {
        let file = self.file.with_file_name("reservation.json");
        let written = match &self.lease {
            Some(l) => {
                let mut v = l.json(l.created);
                if let Value::Obj(fields) = &mut v { fields.retain(|(k, _)| k != "remaining_seconds"); }
                v.set("pin", s(&l.pin));
                let part = file.with_extension("json.part");
                std::fs::write(&part, v.text()).and_then(|_| std::fs::rename(&part, &file))
            }
            None => std::fs::remove_file(&file).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) }),
        };
        if written.is_err() { eprintln!("effector485: {} could not be written", file.display()); }
        self.changed();
    }
}

#[cfg(test)]
mod tests {
    use super::{command, utc, Op, State, Submitted, ALPHABET};
    use crate::json::parse;

    #[test]
    fn a_lease_holds_back_others_and_ends_at_its_time() {
        let dir = std::env::temp_dir().join(format!("effector485-lease-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let mut st = State::load(&dir, "test").unwrap();
        let t = 1_000_000;
        let l = st.reserve(5, "test: a slot update", "EFF", t).unwrap();
        assert!(l.session.starts_with("R-") && l.session.len() == 10 && l.pin.len() == 12, "{l:?}");
        assert!(l.session[2..].bytes().chain(l.pin.bytes()).all(|b| ALPHABET.contains(&b)), "{l:?}");
        assert_eq!(st.reserve(5, "test: another", "DRV", t + 1).unwrap_err(), l);
        let held = l.session.clone();
        // reservation.json brings it back whole, PIN and all.
        assert_eq!(State::load(&dir, "test").unwrap().lease, Some(l.clone()));

        let mut op = |kind: &str, key: &str, session: Option<String>| {
            let request = parse(&format!(r#"{{"kind":"{kind}","name":"x","idempotency_key":"{key}","reason":"test"}}"#)).unwrap();
            match st.submit(&request, session).unwrap() { Submitted::New(i) => i, Submitted::Again(_) => panic!("{key} again") }
        };
        let (theirs, read, mine) = (op("mkdir", "k1", None), op("log", "k2", None), op("mkdir", "k3", Some(l.session.clone())));
        assert!(!st.may_run(&st.ops[theirs], t + 1) && st.may_run(&st.ops[read], t + 1) && st.may_run(&st.ops[mine], t + 1));
        let waiting = st.view(theirs, t + 1);
        let waiting = waiting.get("waiting").unwrap();
        assert_eq!(waiting.str("code"), Some("terminal_reserved"));
        assert_eq!(waiting.str("message"), Some(&*format!("the machine is reserved by EFF (test: a slot update) until 13:51:40 UTC ({})", l.session)));
        assert!(st.view(read, t + 1).get("waiting").is_none() && st.view(mine, t + 1).get("waiting").is_none());
        assert!(st.holds(&l.session, "WRONGWRONGWR", t + 1).unwrap_err().contains("PIN"));
        assert!(st.holds("R-AAAAAAAA", &l.pin, t + 1).unwrap_err().contains("not held"));

        // A renewal counts from now; the lease ends at its time, and its file with it.
        let r = st.renew(&l.session, &l.pin, Some(1), t + 200).unwrap();
        assert_eq!((r.expires, r.minutes, r.renewals), (t + 260, 1, 1));
        st.expire(t + 259);
        assert!(st.lease.is_some() && !st.may_run(&st.ops[theirs], t + 259));
        assert!(st.may_run(&st.ops[theirs], t + 260) && st.renew(&l.session, &l.pin, None, t + 260).is_err());
        st.expire(t + 260);
        assert!(st.lease.is_none() && !dir.join("reservation.json").exists());
        assert_eq!(State::load(&dir, "test").unwrap().lease, None);

        let l = st.reserve(1, "test: released", "", t + 300).unwrap();
        assert!(st.view(theirs, t + 300).get("waiting").unwrap().str("message").unwrap().contains("reserved by a session (test: released)"));
        assert!(st.release(&l.session, "WRONGWRONGWR", t + 301).is_err() && st.lease.is_some());
        assert_eq!(st.release(&l.session, &l.pin, t + 301).unwrap(), l);
        assert!(st.lease.is_none() && !dir.join("reservation.json").exists());

        // An operation keeps its lease in the state file; one written before leases loads without one.
        assert_eq!(Op::from_json(&st.ops[mine].json(None, true)).unwrap().session, Some(held));
        let old = parse(r#"{"id":"e485-1-1","kind":"hello","params":{},"idempotency_key":"k","status":"completed"}"#).unwrap();
        assert_eq!(Op::from_json(&old).unwrap().session, None);
        assert_eq!(utc(t), "13:46:40");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_command_whole_but_a_seal() {
        let c = |kind, json| command(kind, &parse(json).unwrap());
        assert_eq!(c("shell", r#"{"line":"netpolicy add netcheck 10.0.2.9 icmp"}"#), "shell line=netpolicy add netcheck 10.0.2.9 icmp");
        assert_eq!(c("shell", r#"{"line":"TPM  seal hunter2 more"}"#), "shell line=TPM seal [THE SECRET IS NOT LOGGED]");
        assert_eq!(c("run", r#"{"name":"netcheck","args":"ping:1.2.3.4","timeout_seconds":30}"#), "run name=netcheck args=ping:1.2.3.4 timeout_seconds=30");
        assert_eq!(c("replace", r#"{"unit":"rtc","back":true}"#), "replace unit=rtc back=true");
    }
}
