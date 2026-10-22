//! effector485's operations, as on Effector (docs/effector.md): each with an id, an idempotency key and a reason; queued,
//! run one at a time over the cable, read for their status and output, cancelled by id. They are kept in a state file,
//! so a restarted daemon finds its queue and follows the programs it had started.
use crate::json::{obj, s, Value};
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
        })
    }
}

/// The last `max` bytes of `text`, cut at a character's start.
pub fn tail(text: &str, max: usize) -> &str {
    let mut from = text.len().saturating_sub(max);
    while !text.is_char_boundary(from) { from += 1; }
    &text[from..]
}

/// What the daemon knows of the cable's other end.
#[derive(Default)]
pub struct Link { pub port: String, pub online: bool, pub name: String, pub last_seen: u64, pub error: String }

/// `changes` counts the changes, so that the page's feeds see one without taking the lock.
pub struct State { pub ops: Vec<Op>, next: u64, file: PathBuf, pub link: Link, dirty: bool, pub changes: Arc<AtomicU64> }

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
        Ok(State { ops, next, file, link: Link { port: port.to_string(), ..Link::default() }, dirty: false, changes: Arc::default() })
    }

    /// Writes the state file when something changed: a new file renamed over the old, so a crash leaves one whole.
    pub fn save(&mut self) {
        if !self.dirty { return; }
        let ended: Vec<usize> = (0..self.ops.len()).filter(|&i| self.ops[i].ended()).collect();
        if ended.len() > ENDED_KEPT {
            let drop: std::collections::HashSet<usize> = ended[..ended.len() - ENDED_KEPT].iter().copied().collect();
            let mut i = 0;
            self.ops.retain(|_| { i += 1; !drop.contains(&(i - 1)) });
        }
        let text = Value::Arr(self.ops.iter().map(|o| o.json(None, true)).collect()).text();
        let part = self.file.with_extension("json.part");
        if std::fs::write(&part, text).and_then(|_| std::fs::rename(&part, &self.file)).is_ok() { self.dirty = false; }
        else { eprintln!("effector485: {} could not be written", self.file.display()); }
    }

    pub fn touch(&mut self) { self.dirty = true; self.changed(); }

    /// A change the state file does not keep: the cable's other end came or went.
    pub fn changed(&self) { self.changes.fetch_add(1, Ordering::Release); }

    pub fn position(&self, index: usize) -> Option<usize> {
        (self.ops[index].status == "queued").then(|| self.ops[..=index].iter().filter(|o| o.status == "queued").count())
    }

    pub fn find(&self, id: &str) -> Option<usize> { self.ops.iter().position(|o| o.id == id) }

    /// A new operation from a request, or the one its idempotency key already made.
    pub fn submit(&mut self, request: &Value) -> Result<Submitted, String> {
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
            run: None, cursor: 0, deadline: None,
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
}
