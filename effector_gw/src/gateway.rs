//! The gateway's decisions and effects (750-EFF-0007), apart from IPC so the host can test them: each request of
//! `idl/effector.wit` is checked against its deadline and the owner's policy, audited before it is answered (a failed
//! audit write refuses it, MC-10.6), and done through `Machine`, the authorities `init` gives the service.
use alloc::{collections::VecDeque, string::String, vec, vec::Vec};
use crate::audit;
use crate::policy::{self, Ask, Deny, Policy};
use crate::sha256::Sha256;

pub const CHUNK: usize = 32768;
pub const AUDIT_KEPT: usize = 16;

/// Who asks: the session (`BADGE_EFFECTOR_SESSION`) or the shell for the owner (`BADGE_EFFECTOR_OWNER`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Badge { Session, Owner }

/// Why a request was not done; the IDL's `error`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fail { Denied, NotFound, Busy, Bounds, Digest, Exists, Expired, Audit, Io, Signature, Older, Malformed, Cancelled, Unsupported, NoPolicy }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceState { Running, Stopped, Quarantined, Absent }

/// Who asked and why, as the server gave it (the IDL's `origin`).
#[derive(Clone, Copy, Debug)]
pub struct Origin<'a> { pub command: &'a str, pub initiator: &'a str, pub reason: &'a str, pub truncated: bool, pub deadline_ms: u64 }

/// An open transfer out of the machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transfer { pub handle: u32, pub size: u64, pub sha256: [u8; 32], pub truncated: bool }

/// The machine's state for the heartbeat.
#[derive(Clone, Debug, Default)]
pub struct Status { pub uptime_ms: u64, pub memory_used: u64, pub memory_total: u64, pub services: Vec<(String, ServiceState)>, pub policy: bool, pub busy: bool }

/// The authorities `init` gives the gateway (750-KRN-0119); the service binds them, the host tests fake them.
pub trait Machine {
    fn uptime_ms(&mut self) -> u64;
    fn memory(&mut self) -> (u64, u64);
    fn services(&mut self) -> Vec<(String, ServiceState)>;
    fn control(&mut self, name: &str, action: policy::Action) -> Result<ServiceState, Fail>;
    /// A log source (`ring`, `boot`) read whole, at most `max` bytes: the bytes and whether more were cut.
    fn log(&mut self, source: &str, max: u64) -> Result<(Vec<u8>, bool), Fail>;
    /// A file's size; paths are in the server's form (`data/…`, `log:/…`), already allowed by the policy.
    fn size(&mut self, path: &str) -> Result<u64, Fail>;
    fn read(&mut self, path: &str, offset: u64, out: &mut [u8]) -> Result<usize, Fail>;
    fn exists(&mut self, path: &str) -> bool;
    /// Writes at `offset` of a temporary file beside `path` (created at offset 0).
    fn write_temporary(&mut self, path: &str, offset: u64, data: &[u8]) -> Result<(), Fail>;
    /// Flushes the temporary file and gives it `path`'s name; `replace` lets it take an existing file's place.
    fn publish(&mut self, path: &str, replace: bool) -> Result<(), Fail>;
    fn discard_temporary(&mut self, path: &str);
    /// One audit line, to the system log and the owner's audit file; false if it could not be written.
    fn audit(&mut self, line: &str) -> bool;
    /// The owner's policy file, read whole (at most `policy::FILE_MAX` + 1 bytes).
    fn policy_file(&mut self) -> Result<Vec<u8>, Fail>;
}

enum Out { Bytes(Vec<u8>), File(String) }
struct Outgoing { handle: u32, command: String, size: u64, source: Out }
struct Incoming { handle: u32, command: String, path: String, size: u64, sha256: [u8; 32], replace: bool, written: u64, hash: Sha256 }

pub struct Gateway {
    policy: Option<Policy>,
    policy_sha256: [u8; 32],
    policy_read_ms: u64,
    out: Option<Outgoing>,
    inc: Option<Incoming>,
    next_handle: u32,
    kept: VecDeque<String>,
}

impl Default for Gateway { fn default() -> Self { Self::new() } }

fn deny(d: Deny) -> Fail { match d { Deny::TooLarge => Fail::Bounds, Deny::Path => Fail::Malformed, _ => Fail::Denied } }

fn hex(bytes: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in bytes { s.push(b"0123456789abcdef"[(b >> 4) as usize] as char); s.push(b"0123456789abcdef"[(b & 15) as usize] as char); }
    s
}

fn unhex(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    if b.len() != 64 { return None; }
    let mut out = [0u8; 32];
    for i in 0..32 {
        let d = |c: u8| match c { b'0'..=b'9' => Some(c - b'0'), b'a'..=b'f' => Some(c - b'a' + 10), _ => None };
        out[i] = d(b[2 * i])? << 4 | d(b[2 * i + 1])?;
    }
    Some(out)
}

impl Gateway {
    pub fn new() -> Self {
        Self { policy: None, policy_sha256: [0; 32], policy_read_ms: 0, out: None, inc: None, next_handle: 1, kept: VecDeque::new() }
    }

    pub fn policy_loaded(&self) -> bool { self.policy.is_some() }

    /// Writes the audit line before the reply; a request whose line cannot be written is not done (MC-10.6).
    fn audited(&mut self, m: &mut dyn Machine, origin: Option<&Origin<'_>>, request: &str, decision: &str, result: &str) -> bool {
        let e = audit::Entry {
            uptime_ms: m.uptime_ms(), command: origin.map_or("", |o| o.command), request, decision, result,
            initiator: origin.map_or("owner", |o| o.initiator), reason: origin.map_or("", |o| o.reason),
        };
        let line = audit::format(&e);
        if !m.audit(line.as_str()) { return false; }
        if self.kept.len() == AUDIT_KEPT { self.kept.pop_front(); }
        self.kept.push_back(String::from(line.as_str()));
        true
    }

    /// The common path of a request from the session: its badge, its deadline, the policy's decision, the audit.
    /// `Ok(())` lets the caller do it; the caller audits the outcome when it differs from `allow`.
    fn admit(&mut self, m: &mut dyn Machine, badge: Badge, o: &Origin<'_>, request: &str, ask: Option<Ask<'_>>) -> Result<(), Fail> {
        let refusal = if badge != Badge::Session { Some(Fail::Denied) }
            else if o.deadline_ms <= m.uptime_ms() { Some(Fail::Expired) }
            else if self.policy.is_none() { Some(Fail::NoPolicy) }
            else { ask.and_then(|a| self.policy.as_ref().unwrap().decide(a).err()).map(deny) };
        let decision = match refusal { None => "allow", Some(f) => fail_name(f) };
        if !self.audited(m, Some(o), request, decision, if refusal.is_none() { "admitted" } else { "refused" }) { return Err(Fail::Audit); }
        refusal.map_or(Ok(()), Err)
    }

    fn handle(&mut self) -> u32 { let h = self.next_handle; self.next_handle = self.next_handle.wrapping_add(1).max(1); h }

    pub fn status(&mut self, m: &mut dyn Machine) -> Status {
        let (used, total) = m.memory();
        Status {
            uptime_ms: m.uptime_ms(), memory_used: used, memory_total: total,
            services: m.services(),
            policy: self.policy.is_some(), busy: self.out.is_some() || self.inc.is_some(),
        }
    }

    pub fn service_control(&mut self, m: &mut dyn Machine, badge: Badge, o: &Origin<'_>, name: &str, action: policy::Action) -> Result<ServiceState, Fail> {
        let word = match action { policy::Action::Start => "start", policy::Action::Stop => "stop", policy::Action::Restart => "restart" };
        let request = alloc::format!("service-control {} {}", name, word);
        self.admit(m, badge, o, &request, Some(Ask::Service(name, action)))?;
        let result = m.control(name, action);
        let _ = self.audited(m, Some(o), &request, "done", match &result { Ok(_) => "completed", Err(f) => fail_name(*f) });
        result
    }

    fn open(&mut self, o: &Origin<'_>, size: u64, truncated: bool, source: Out, sha256: [u8; 32]) -> Transfer {
        let handle = self.handle();
        self.out = Some(Outgoing { handle, command: String::from(o.command), size, source });
        Transfer { handle, size, sha256, truncated }
    }

    pub fn open_log(&mut self, m: &mut dyn Machine, badge: Badge, o: &Origin<'_>, source: &str) -> Result<Transfer, Fail> {
        let request = alloc::format!("open-log {}", source);
        if self.out.is_some() || self.inc.is_some() { self.audited(m, Some(o), &request, "busy", "refused"); return Err(Fail::Busy); }
        self.admit(m, badge, o, &request, Some(Ask::Log(source)))?;
        let max = self.policy.as_ref().map_or(0, |p| p.max_bytes());
        let (bytes, truncated) = match m.log(source, max) { Ok(r) => r, Err(f) => { let _ = self.audited(m, Some(o), &request, "done", fail_name(f)); return Err(f); } };
        let sha = crate::sha256::digest(&bytes);
        let size = bytes.len() as u64;
        Ok(self.open(o, size, truncated, Out::Bytes(bytes), sha))
    }

    pub fn open_file(&mut self, m: &mut dyn Machine, badge: Badge, o: &Origin<'_>, path: &str) -> Result<Transfer, Fail> {
        let request = alloc::format!("open-file {}", path);
        if self.out.is_some() || self.inc.is_some() { self.audited(m, Some(o), &request, "busy", "refused"); return Err(Fail::Busy); }
        // The size is not known until the path is allowed: decide the path first, the size after.
        self.admit(m, badge, o, &request, Some(Ask::Collect(path, 0)))?;
        let size = match m.size(path) { Ok(s) => s, Err(f) => { let _ = self.audited(m, Some(o), &request, "done", fail_name(f)); return Err(f); } };
        if let Err(d) = self.policy.as_ref().unwrap().decide(Ask::Collect(path, size)) {
            let _ = self.audited(m, Some(o), &request, "too-large", "refused");
            return Err(deny(d));
        }
        let mut hash = Sha256::new();
        let mut buf = vec![0u8; CHUNK];
        let mut at = 0u64;
        while at < size {
            let n = match m.read(path, at, &mut buf) { Ok(0) => break, Ok(n) => n, Err(f) => { let _ = self.audited(m, Some(o), &request, "done", fail_name(f)); return Err(f); } };
            hash.update(&buf[..n]);
            at += n as u64;
        }
        if at != size { let _ = self.audited(m, Some(o), &request, "done", "io"); return Err(Fail::Io); }
        Ok(self.open(o, size, false, Out::File(String::from(path)), hash.finish()))
    }

    /// Up to `CHUNK` bytes of the open transfer from `offset`; an empty answer is its end. A file that changed size
    /// since it was opened is an `io` failure: what leaves must be what was hashed.
    pub fn read(&mut self, m: &mut dyn Machine, badge: Badge, handle: u32, offset: u64, out: &mut [u8]) -> Result<usize, Fail> {
        if badge != Badge::Session { return Err(Fail::Denied); }
        let Some(t) = self.out.as_ref().filter(|t| t.handle == handle) else { return Err(Fail::NotFound) };
        if offset > t.size { return Err(Fail::Bounds); }
        let want = ((t.size - offset) as usize).min(out.len()).min(CHUNK);
        match &t.source {
            Out::Bytes(b) => { out[..want].copy_from_slice(&b[offset as usize..offset as usize + want]); Ok(want) }
            Out::File(p) => {
                let p = p.clone();
                if m.size(&p)? != t.size { return Err(Fail::Io); }
                let n = m.read(&p, offset, &mut out[..want])?;
                if n != want { return Err(Fail::Io); }
                Ok(n)
            }
        }
    }

    pub fn close(&mut self, m: &mut dyn Machine, badge: Badge, handle: u32) -> Result<(), Fail> {
        if badge != Badge::Session { return Err(Fail::Denied); }
        if self.out.as_ref().is_some_and(|t| t.handle == handle) { self.out = None; return Ok(()); }
        if let Some(i) = self.inc.take() {
            if i.handle == handle { m.discard_temporary(&i.path); return Ok(()); }
            self.inc = Some(i);
        }
        Err(Fail::NotFound)
    }

    pub fn deliver_begin(&mut self, m: &mut dyn Machine, badge: Badge, o: &Origin<'_>, path: &str, size: u64, sha256: &str, replace: bool) -> Result<u32, Fail> {
        let request = alloc::format!("deliver {} {} bytes", path, size);
        if self.out.is_some() || self.inc.is_some() { self.audited(m, Some(o), &request, "busy", "refused"); return Err(Fail::Busy); }
        let Some(sha) = unhex(sha256) else { self.audited(m, Some(o), &request, "malformed", "refused"); return Err(Fail::Malformed) };
        self.admit(m, badge, o, &request, Some(Ask::Deliver(path, size)))?;
        if !replace && m.exists(path) { let _ = self.audited(m, Some(o), &request, "done", "exists"); return Err(Fail::Exists); }
        let handle = self.handle();
        self.inc = Some(Incoming { handle, command: String::from(o.command), path: String::from(path), size, sha256: sha, replace, written: 0, hash: Sha256::new() });
        Ok(handle)
    }

    /// The next bytes, in order and without gaps; more than announced is `bounds`.
    pub fn write(&mut self, m: &mut dyn Machine, badge: Badge, handle: u32, offset: u64, data: &[u8]) -> Result<(), Fail> {
        if badge != Badge::Session { return Err(Fail::Denied); }
        let Some(i) = self.inc.as_mut().filter(|i| i.handle == handle) else { return Err(Fail::NotFound) };
        if offset != i.written { return Err(Fail::Bounds); }
        if data.len() > CHUNK || i.written + data.len() as u64 > i.size { return Err(Fail::Bounds); }
        m.write_temporary(&i.path, offset, data)?;
        i.hash.update(data);
        i.written += data.len() as u64;
        Ok(())
    }

    /// Checks size and SHA-256, then publishes. Either way the transfer ends.
    pub fn deliver_commit(&mut self, m: &mut dyn Machine, badge: Badge, handle: u32) -> Result<(), Fail> {
        if badge != Badge::Session { return Err(Fail::Denied); }
        let Some(i) = self.inc.take() else { return Err(Fail::NotFound) };
        if i.handle != handle { self.inc = Some(i); return Err(Fail::NotFound); }
        let request = alloc::format!("deliver {} {} bytes", i.path, i.size);
        let o = Origin { command: &i.command, initiator: "", reason: "", truncated: false, deadline_ms: u64::MAX };
        let result = if i.written != i.size || i.hash.finish() != i.sha256 { Err(Fail::Digest) } else { m.publish(&i.path, i.replace) };
        if result.is_err() { m.discard_temporary(&i.path); }
        if !self.audited(m, Some(&o), &request, "commit", match &result { Ok(()) => "completed", Err(f) => fail_name(*f) }) && result.is_ok() {
            // Published but not audited: the file stays, the request is reported as the audit failure it is.
            return Err(Fail::Audit);
        }
        result
    }

    /// Requests the policy may allow but this build cannot do yet: audited, then `unsupported` (or the denial).
    pub fn not_yet(&mut self, m: &mut dyn Machine, badge: Badge, o: &Origin<'_>, request: &str, ask: Ask<'_>) -> Fail {
        match self.admit(m, badge, o, request, Some(ask)) {
            Err(f) => f,
            Ok(()) => { let _ = self.audited(m, Some(o), request, "done", "unsupported"); Fail::Unsupported }
        }
    }

    /// Cancels the open transfer of that command; anything that already ended is `not-found`.
    pub fn cancel(&mut self, m: &mut dyn Machine, badge: Badge, command: &str) -> Result<(), Fail> {
        if badge != Badge::Session { return Err(Fail::Denied); }
        if self.out.as_ref().is_some_and(|t| t.command == command) { self.out = None; }
        else if self.inc.as_ref().is_some_and(|i| i.command == command) { let i = self.inc.take().unwrap(); m.discard_temporary(&i.path); }
        else { return Err(Fail::NotFound); }
        let o = Origin { command, initiator: "", reason: "", truncated: false, deadline_ms: u64::MAX };
        if self.audited(m, Some(&o), "cancel", "allow", "cancelled") { Ok(()) } else { Err(Fail::Audit) }
    }

    /// The owner only: reads the policy again; one that does not parse leaves none loaded.
    pub fn reload_policy(&mut self, m: &mut dyn Machine, badge: Badge) -> Result<(u16, [u8; 32], u16, u64), Fail> {
        if badge != Badge::Owner { return Err(Fail::Denied); }
        let now = m.uptime_ms();
        let parsed = m.policy_file().and_then(|b| Policy::parse(&b).map(|p| (p, crate::sha256::digest(&b))).map_err(|_| Fail::Malformed));
        let result = match parsed {
            Ok((p, sha)) => { self.policy_sha256 = sha; self.policy = Some(p); self.policy_read_ms = now; Ok((policy::VERSION, sha, p.rules(), now)) }
            Err(f) => { self.policy = None; Err(f) }
        };
        let text = match &result { Ok(_) => alloc::format!("policy {}", hex(&self.policy_sha256)), Err(f) => alloc::format!("policy refused: {}", fail_name(*f)) };
        if !self.audited(m, None, "reload-policy", "owner", &text) { return Err(Fail::Audit); }
        result
    }

    /// The owner only: the last audit lines, newest last.
    pub fn audit_lines(&self, badge: Badge, count: usize) -> Result<Vec<String>, Fail> {
        if badge != Badge::Owner { return Err(Fail::Denied); }
        let skip = self.kept.len().saturating_sub(count.min(AUDIT_KEPT));
        Ok(self.kept.iter().skip(skip).cloned().collect())
    }
}

pub fn fail_name(f: Fail) -> &'static str {
    match f {
        Fail::Denied => "denied", Fail::NotFound => "not-found", Fail::Busy => "busy", Fail::Bounds => "bounds", Fail::Digest => "digest",
        Fail::Exists => "exists", Fail::Expired => "expired", Fail::Audit => "audit", Fail::Io => "io", Fail::Signature => "signature",
        Fail::Older => "older", Fail::Malformed => "malformed", Fail::Cancelled => "cancelled", Fail::Unsupported => "unsupported", Fail::NoPolicy => "no-policy",
    }
}
