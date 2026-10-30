//! The Effector gateway's decisions and effects (750-EFF-0007) against a fake machine: the badge, the deadline, the
//! policy, the audit before every answer (a failed audit write refuses the request, MC-10.6), files out in pieces with
//! their SHA-256, files in checked by size and SHA-256 before they are published; the screen within the policy's bound;
//! the telemetry counters for the session and the owner only; a developer's release streamed to the updater under the
//! policy's `update apply` and the updater's own `remote-update` check, then applied (751-EFF-0023).
#![allow(dead_code)]
extern crate alloc;
#[path = "../libmind/src/sha256.rs"] mod sha256;
#[path = "../libmind/src/effector/policy.rs"] mod policy;
#[path = "../effector_gw/src/audit.rs"] mod audit;
#[path = "../effector_gw/src/gateway.rs"] mod gateway;
use gateway::*;
use std::collections::HashMap;

#[derive(Default)]
struct Fake {
    now: u64,
    files: HashMap<String, Vec<u8>>,
    temporary: HashMap<String, Vec<u8>>,
    log_ring: Vec<u8>,
    policy: Option<Vec<u8>>,
    audit: Vec<String>,
    audit_fails: bool,
    controlled: Vec<(String, policy::Action)>,
    screen: Vec<u8>,
    screens: usize,
    reboots: usize,
    published_differs: bool,
    remote_update: bool,
    updater_denies: bool,
    begun: Vec<(u64, String, String)>,
    installed: Vec<u8>,
    staged: Option<Result<u64, Fail>>,
    commits: usize,
    applied: Vec<u8>,
    apply_refused: Option<Fail>,
}

impl Machine for Fake {
    fn uptime_ms(&mut self) -> u64 { self.now }
    fn memory(&mut self) -> (u64, u64) { (100, 1000) }
    fn services(&mut self) -> Vec<(String, ServiceState)> { vec![("sysmon".into(), ServiceState::Running), ("rtc".into(), ServiceState::Stopped)] }
    fn control(&mut self, name: &str, action: policy::Action) -> Result<ServiceState, Fail> { self.controlled.push((name.into(), action)); Ok(ServiceState::Running) }
    fn log(&mut self, _source: &str, max: u64) -> Result<(Vec<u8>, bool), Fail> {
        let cut = self.log_ring.len() as u64 > max;
        Ok((self.log_ring[self.log_ring.len() - (self.log_ring.len().min(max as usize))..].to_vec(), cut))
    }
    fn size(&mut self, path: &str) -> Result<u64, Fail> { self.files.get(path).map(|f| f.len() as u64).ok_or(Fail::NotFound) }
    fn read(&mut self, path: &str, offset: u64, out: &mut [u8]) -> Result<usize, Fail> {
        let f = self.files.get(path).ok_or(Fail::NotFound)?;
        let start = (offset as usize).min(f.len());
        let n = (f.len() - start).min(out.len());
        out[..n].copy_from_slice(&f[start..start + n]);
        Ok(n)
    }
    fn exists(&mut self, path: &str) -> bool { self.files.contains_key(path) }
    fn write_temporary(&mut self, path: &str, offset: u64, data: &[u8]) -> Result<(), Fail> {
        let t = self.temporary.entry(path.into()).or_default();
        if offset == 0 { t.clear(); }
        assert_eq!(t.len() as u64, offset, "writes come in order");
        t.extend_from_slice(data);
        Ok(())
    }
    fn publish(&mut self, path: &str, replace: bool) -> Result<(), Fail> {
        if !replace && self.files.contains_key(path) { return Err(Fail::Exists); }
        let t = self.temporary.remove(path).unwrap_or_default();
        self.files.insert(path.into(), t);
        Ok(())
    }
    fn discard_temporary(&mut self, path: &str) { self.temporary.remove(path); }
    fn read_published(&mut self, path: &str, offset: u64, out: &mut [u8]) -> Result<usize, Fail> {
        let n = self.read(path, offset, out)?;
        if self.published_differs && offset == 0 && n > 0 { out[0] ^= 1; }
        Ok(n)
    }
    fn audit(&mut self, line: &str) -> bool { if self.audit_fails { return false; } self.audit.push(line.into()); true }
    fn policy_file(&mut self) -> Result<Vec<u8>, Fail> { self.policy.clone().ok_or(Fail::NotFound) }
    fn screen(&mut self, max: u64) -> Result<Vec<u8>, Fail> {
        self.screens += 1;
        if self.screen.len() as u64 > max { Err(Fail::Bounds) } else { Ok(self.screen.clone()) }
    }
    fn load(&mut self) -> u32 { 150 }
    fn reboot(&mut self) -> Result<(), Fail> { self.reboots += 1; Ok(()) }
    fn remote_update(&mut self) -> bool { self.remote_update }
    // The updater: install-begin denied while remote-update is off (as it reads it), a second begin drops the first.
    fn install_begin(&mut self, size: u64, sha256: &str, command: &str) -> Result<u32, Fail> {
        if self.updater_denies { return Err(Fail::Denied); }
        self.begun.push((size, sha256.into(), command.into()));
        self.installed.clear();
        Ok(40 + self.begun.len() as u32)
    }
    fn install_write(&mut self, handle: u32, offset: u64, data: &[u8]) -> Result<(), Fail> {
        assert_eq!((handle, offset), (40 + self.begun.len() as u32, self.installed.len() as u64), "the updater's handle, in order");
        self.installed.extend_from_slice(data);
        Ok(())
    }
    fn install_commit(&mut self, handle: u32) -> Result<u64, Fail> {
        assert_eq!(handle, 40 + self.begun.len() as u32);
        self.commits += 1;
        let (size, sha, _) = self.begun.last().unwrap();
        if self.installed.len() as u64 != *size || &hex(&sha256::digest(&self.installed)) != sha { return Err(Fail::Digest); }
        self.staged.unwrap_or(Ok(6))
    }
    fn apply(&mut self, tries: u8) -> Result<(), Fail> { self.applied.push(tries); self.apply_refused.map_or(Ok(()), Err) }
    fn update_state(&mut self) -> Option<UpdateState> {
        Some(UpdateState { slot: b'A', trial: false, running: 5, recorded: b'A', confirmed: true, staged: if self.commits > 0 { 6 } else { 0 }, failure: String::new() })
    }
    fn sample(&mut self) -> Sample {
        Sample { cpus: 2, busy_ns: 5, idle_ns: 7, uptime_ms: self.now, memory_total: 1000, memory_free: 900,
                 services: vec![Load { name: "sysmon".into(), state: Some(ServiceState::Running), pid: 9, run_ns: 1, started_ns: 2, memory: 3 }],
                 volumes: vec![(String::new(), 100, 40)] }
    }
}

const POLICY: &str = "effector-policy 1\nagent on\nservice sysmon restart\nlog ring\ncollect data/reports/\ndeliver data/inbox/\nscreen on\nmax-bytes 100000\n";

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }

fn origin(command: &str) -> Origin<'_> { Origin { command, initiator: "operator", reason: "a test", truncated: false, deadline_ms: 10_000 } }

fn ready() -> (Gateway, Fake) {
    let mut m = Fake { now: 1000, policy: Some(POLICY.as_bytes().to_vec()), ..Fake::default() };
    let mut g = Gateway::new();
    let (version, _, rules, read_ms) = g.reload_policy(&mut m, Badge::Owner).unwrap();
    assert_eq!((version, rules, read_ms), (1, 7, 1000));
    (g, m)
}

#[test]
fn without_a_policy_everything_is_refused() {
    let mut m = Fake { now: 1000, ..Fake::default() };
    let mut g = Gateway::new();
    assert_eq!(g.service_control(&mut m, Badge::Session, &origin("c1"), "sysmon", policy::Action::Restart), Err(Fail::NoPolicy));
    assert!(m.controlled.is_empty());
    assert!(m.audit[0].contains(" c1 service-control?sysmon?restart no-policy refused "), "{:?}", m.audit);
    assert_eq!(g.reload_policy(&mut m, Badge::Owner).err(), Some(Fail::NotFound), "no policy file");
    m.policy = Some(b"effector-policy 1\nagent on\nunknown key\n".to_vec());
    assert_eq!(g.reload_policy(&mut m, Badge::Owner).err(), Some(Fail::Malformed));
    assert!(!g.policy_loaded());
}

#[test]
fn only_the_owner_reloads_the_policy_and_reads_the_audit() {
    let (mut g, mut m) = ready();
    assert_eq!(g.reload_policy(&mut m, Badge::Session).err(), Some(Fail::Denied));
    assert_eq!(g.audit_lines(Badge::Session, 4).err(), Some(Fail::Denied));
    assert!(g.audit_lines(Badge::Owner, 4).unwrap()[0].contains("reload-policy owner"));
    // A bad file replaces a good one with none: everything is refused until it is fixed.
    m.policy = Some(b"not a policy\n".to_vec());
    assert_eq!(g.reload_policy(&mut m, Badge::Owner).err(), Some(Fail::Malformed));
    assert_eq!(g.service_control(&mut m, Badge::Session, &origin("c2"), "sysmon", policy::Action::Restart), Err(Fail::NoPolicy));
}

#[test]
fn services_by_the_policy_the_badge_and_the_deadline() {
    let (mut g, mut m) = ready();
    assert_eq!(g.service_control(&mut m, Badge::Session, &origin("c1"), "sysmon", policy::Action::Restart), Ok(ServiceState::Running));
    assert_eq!(m.controlled, vec![("sysmon".to_owned(), policy::Action::Restart)]);
    assert_eq!(g.service_control(&mut m, Badge::Session, &origin("c2"), "sysmon", policy::Action::Stop), Err(Fail::Denied));
    assert_eq!(g.service_control(&mut m, Badge::Session, &origin("c3"), "shell", policy::Action::Restart), Err(Fail::Denied));
    assert_eq!(g.service_control(&mut m, Badge::Owner, &origin("c4"), "sysmon", policy::Action::Restart), Err(Fail::Denied), "actions come from the session");
    m.now = 20_000;
    assert_eq!(g.service_control(&mut m, Badge::Session, &origin("c5"), "sysmon", policy::Action::Restart), Err(Fail::Expired));
    assert_eq!(m.controlled.len(), 1, "nothing refused reached init");
    let lines = g.audit_lines(Badge::Owner, 16).unwrap();
    assert!(lines.iter().any(|l| l.contains(" c1 service-control?sysmon?restart done completed ")), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains(" c5 service-control?sysmon?restart expired refused ")), "{lines:?}");
}

#[test]
fn a_failed_audit_write_refuses_the_request() {
    let (mut g, mut m) = ready();
    m.audit_fails = true;
    assert_eq!(g.service_control(&mut m, Badge::Session, &origin("c1"), "sysmon", policy::Action::Restart), Err(Fail::Audit));
    assert!(m.controlled.is_empty(), "not done without its audit line (MC-10.6)");
}

#[test]
fn a_file_leaves_in_pieces_with_its_hash() {
    let (mut g, mut m) = ready();
    let body: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
    m.files.insert("data/reports/r.bin".into(), body.clone());
    let t = g.open_file(&mut m, Badge::Session, &origin("c1"), "data/reports/r.bin").unwrap();
    assert_eq!((t.size, t.sha256, t.truncated), (70_000, sha256::digest(&body), false));
    assert_eq!(g.open_file(&mut m, Badge::Session, &origin("c2"), "data/reports/r.bin").err(), Some(Fail::Busy), "one transfer at a time");
    let mut got = Vec::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = g.read(&mut m, Badge::Session, t.handle, got.len() as u64, &mut buf).unwrap();
        if n == 0 { break; }
        got.extend_from_slice(&buf[..n]);
    }
    assert_eq!(got, body);
    assert_eq!(g.read(&mut m, Badge::Session, t.handle + 1, 0, &mut buf).err(), Some(Fail::NotFound));
    // A file that changes after it was hashed is not sent on.
    m.files.get_mut("data/reports/r.bin").unwrap().push(0);
    assert_eq!(g.read(&mut m, Badge::Session, t.handle, 0, &mut buf).err(), Some(Fail::Io));
    g.close(&mut m, Badge::Session, t.handle).unwrap();
    assert_eq!(g.read(&mut m, Badge::Session, t.handle, 0, &mut buf).err(), Some(Fail::NotFound));
}

#[test]
fn files_the_policy_does_not_let_leave() {
    let (mut g, mut m) = ready();
    m.files.insert("data/effector/token".into(), b"secret".to_vec());
    m.files.insert("data/other.txt".into(), b"x".to_vec());
    m.files.insert("data/reports/big.bin".into(), vec![0; 100_001]);
    assert_eq!(g.open_file(&mut m, Badge::Session, &origin("c1"), "data/effector/token").err(), Some(Fail::Denied));
    assert_eq!(g.open_file(&mut m, Badge::Session, &origin("c2"), "data/other.txt").err(), Some(Fail::Denied));
    assert_eq!(g.open_file(&mut m, Badge::Session, &origin("c3"), "data/reports/../effector/token").err(), Some(Fail::Malformed));
    assert_eq!(g.open_file(&mut m, Badge::Session, &origin("c4"), "data/reports/big.bin").err(), Some(Fail::Bounds));
    assert_eq!(g.open_file(&mut m, Badge::Session, &origin("c5"), "data/reports/missing").err(), Some(Fail::NotFound));
    assert!(!m.audit.iter().any(|l| l.contains("secret")));
}

#[test]
fn the_log_ring_leaves_cut_to_the_bound() {
    let (mut g, mut m) = ready();
    m.log_ring = vec![b'x'; 150_000];
    let t = g.open_log(&mut m, Badge::Session, &origin("c1"), "ring").unwrap();
    assert_eq!((t.size, t.truncated), (100_000, true));
    g.close(&mut m, Badge::Session, t.handle).unwrap();
    assert_eq!(g.open_log(&mut m, Badge::Session, &origin("c2"), "boot").err(), Some(Fail::Denied));
}

#[test]
fn a_delivered_file_is_checked_before_it_is_published() {
    let (mut g, mut m) = ready();
    let body = b"the delivered file\n".repeat(3000);
    let sha: String = sha256::digest(&body).iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(g.deliver_begin(&mut m, Badge::Session, &origin("c1"), "data/inbox/a.bin", body.len() as u64, "zz", false).err(), Some(Fail::Malformed));
    assert_eq!(g.deliver_begin(&mut m, Badge::Session, &origin("c2"), "data/reports/a.bin", body.len() as u64, &sha, false).err(), Some(Fail::Denied));
    let h = g.deliver_begin(&mut m, Badge::Session, &origin("c3"), "data/inbox/a.bin", body.len() as u64, &sha, false).unwrap();
    assert_eq!(g.write(&mut m, Badge::Session, h, 5, &body[..5]).err(), Some(Fail::Bounds), "bytes in order");
    for (i, piece) in body.chunks(CHUNK).enumerate() { g.write(&mut m, Badge::Session, h, (i * CHUNK) as u64, piece).unwrap(); }
    assert_eq!(g.write(&mut m, Badge::Session, h, body.len() as u64, b"more").err(), Some(Fail::Bounds), "no more than announced");
    let delivered = g.deliver_commit(&mut m, Badge::Session, h).unwrap();
    assert_eq!(m.files["data/inbox/a.bin"], body);
    // The answer is the file as read back under its name (750-EFF-0021), its digest's head in the audit.
    assert_eq!(delivered, Delivered { path: "data/inbox/a.bin".into(), size: body.len() as u64, sha256: sha256::digest(&body) });
    assert!(m.audit.iter().any(|l| l.contains(&format!(" c3 deliver?data/inbox/a.bin?{}?bytes commit completed?{} ", body.len(), &sha[..16]))), "{:?}", m.audit);
    // The same name again without replacing: refused before a byte is taken.
    assert_eq!(g.deliver_begin(&mut m, Badge::Session, &origin("c4"), "data/inbox/a.bin", 4, &sha, false).err(), Some(Fail::Exists));
    // Bytes that do not match the announced hash are discarded, never published.
    let h = g.deliver_begin(&mut m, Badge::Session, &origin("c5"), "data/inbox/b.bin", 4, &sha, false).unwrap();
    g.write(&mut m, Badge::Session, h, 0, b"evil").unwrap();
    assert_eq!(g.deliver_commit(&mut m, Badge::Session, h).err(), Some(Fail::Digest));
    assert!(!m.files.contains_key("data/inbox/b.bin") && m.temporary.is_empty());
}

#[test]
fn a_delivered_file_that_reads_back_otherwise_is_a_digest_failure() {
    let (mut g, mut m) = ready();
    m.published_differs = true;
    let body = b"read back with a bit flipped\n".repeat(10);
    let sha: String = sha256::digest(&body).iter().map(|b| format!("{b:02x}")).collect();
    let h = g.deliver_begin(&mut m, Badge::Session, &origin("c1"), "data/inbox/c.bin", body.len() as u64, &sha, false).unwrap();
    g.write(&mut m, Badge::Session, h, 0, &body).unwrap();
    assert_eq!(g.deliver_commit(&mut m, Badge::Session, h).err(), Some(Fail::Digest));
    assert!(m.files.contains_key("data/inbox/c.bin"), "published before the read-back, so it stays");
    assert!(m.audit.last().unwrap().contains(" c1 deliver?data/inbox/c.bin?290?bytes commit digest "), "{:?}", m.audit);
}

#[test]
fn the_screen_leaves_within_the_policy() {
    let (mut g, mut m) = ready();
    m.screen = b"\x89PNG a picture".to_vec();
    let t = g.open_screen(&mut m, Badge::Session, &origin("c1")).unwrap();
    assert_eq!((t.size, t.sha256, t.truncated), (14, sha256::digest(&m.screen), false));
    let mut buf = [0u8; 64];
    assert_eq!(g.read(&mut m, Badge::Session, t.handle, 0, &mut buf).unwrap(), 14);
    assert_eq!(&buf[..14], &m.screen[..]);
    assert_eq!(g.open_screen(&mut m, Badge::Session, &origin("c2")).err(), Some(Fail::Busy), "one transfer at a time");
    g.close(&mut m, Badge::Session, t.handle).unwrap();
    assert!(m.audit.iter().any(|l| l.contains(" c1 open-screen done 14?bytes ")), "{:?}", m.audit);
    // Larger than the policy's max-bytes (100000): refused, audited.
    m.screen = vec![0; 100_001];
    assert_eq!(g.open_screen(&mut m, Badge::Session, &origin("c3")).err(), Some(Fail::Bounds));
    assert!(m.audit.last().unwrap().contains(" c3 open-screen done bounds "), "{:?}", m.audit);
    // Without `screen on` the screen is never captured.
    m.policy = Some(POLICY.replace("screen on\n", "").into_bytes());
    g.reload_policy(&mut m, Badge::Owner).unwrap();
    let captured = m.screens;
    assert_eq!(g.open_screen(&mut m, Badge::Session, &origin("c4")).err(), Some(Fail::Denied));
    assert_eq!(g.open_screen(&mut m, Badge::Other, &origin("c5")).err(), Some(Fail::Denied));
    assert_eq!(m.screens, captured);
}

#[test]
fn a_reboot_only_under_reboot_on_after_its_answer() {
    let (mut g, mut m) = ready();
    assert_eq!(g.reboot(&mut m, Badge::Session, &origin("c1"), 10_000), Err(Fail::Denied), "the policy has no `reboot on`");
    assert!(m.audit.last().unwrap().contains(" c1 reboot?in?10000?ms denied refused "), "{:?}", m.audit);
    m.policy = Some(format!("{}reboot on\n", POLICY).into_bytes());
    g.reload_policy(&mut m, Badge::Owner).unwrap();
    assert_eq!(g.reboot(&mut m, Badge::Other, &origin("c2"), 10_000), Err(Fail::Denied));
    assert_eq!(g.reboot(&mut m, Badge::Session, &origin("c3"), 600_001), Err(Fail::Bounds));
    assert_eq!(g.reboot(&mut m, Badge::Session, &origin("c4"), 10_000), Ok(10_000));
    assert_eq!(m.reboots, 0, "admitted, not yet asked of init: the answer goes first");
    g.restart(&mut m, "c4").unwrap();
    assert_eq!(m.reboots, 1);
    assert!(m.audit.last().unwrap().contains(" c4 reboot done restarting "), "{:?}", m.audit);
}

#[test]
fn the_mark_follows_the_sessions_reports_and_the_policy() {
    let (mut g, mut m) = ready();
    assert_eq!(g.mark(m.now), Mark::Unconnected, "no report yet");
    g.connection(&mut m, Badge::Session, true, false);
    assert_eq!(g.mark(m.now), Mark::Unconnected, "connected, not activated");
    g.connection(&mut m, Badge::Session, true, true);
    assert_eq!(g.mark(m.now), Mark::Changing, "the policy delivers and restarts sysmon");
    assert_eq!(g.mark(m.now + CONNECTION_MS), Mark::Unconnected, "no report for 3 s: the session stopped or hung");
    g.connection(&mut m, Badge::Other, false, false);
    assert_eq!(g.mark(m.now), Mark::Changing, "another client's report is ignored");
    m.policy = Some(b"effector-policy 1\nagent on\ncollect data/reports/\nscreen on\nlog ring\nupdate check\n".to_vec());
    g.reload_policy(&mut m, Badge::Owner).unwrap();
    assert_eq!(g.mark(m.now), Mark::Reading, "reading only: files, the screen, the log, an update's check");
    g.connection(&mut m, Badge::Session, false, true);
    assert_eq!(g.mark(m.now), Mark::Unconnected);
}

#[test]
fn the_sample_is_for_the_session_and_the_owner() {
    let (mut g, mut m) = ready();
    let s = g.sample(&mut m, Badge::Session);
    assert_eq!((s.cpus, s.memory_free, s.services.len(), s.volumes.len()), (2, 900, 1, 1));
    assert_eq!(g.sample(&mut m, Badge::Owner), s);
    assert_eq!(g.sample(&mut m, Badge::Other), Sample::default());
    assert_eq!((g.load(&mut m, Badge::Session), g.load(&mut m, Badge::Owner), g.load(&mut m, Badge::Other)), (150, 150, 0));
}

#[test]
fn not_yet_cancel_and_status() {
    let (mut g, mut m) = ready();
    assert_eq!(g.not_yet(&mut m, Badge::Session, &origin("c2"), "update-check", policy::Ask::Update(policy::Update::Check)), Fail::Denied);
    m.files.insert("data/reports/r.bin".into(), vec![1; 10]);
    let t = g.open_file(&mut m, Badge::Session, &origin("c3"), "data/reports/r.bin").unwrap();
    assert!(g.status(&mut m).busy);
    g.cancel(&mut m, Badge::Session, "c3").unwrap();
    assert_eq!(g.cancel(&mut m, Badge::Session, "c3").err(), Some(Fail::NotFound));
    let mut buf = [0u8; 16];
    assert_eq!(g.read(&mut m, Badge::Session, t.handle, 0, &mut buf).err(), Some(Fail::NotFound));
    let s = g.status(&mut m);
    assert!(s.policy && !s.busy && s.services.len() == 2 && s.memory_total == 1000);
}

#[test]
fn a_client_with_another_badge_gets_nothing() {
    let (mut g, mut m) = ready();
    m.files.insert("data/reports/r.bin".into(), vec![1; 10]);
    assert_eq!(g.open_file(&mut m, Badge::Other, &origin("c1"), "data/reports/r.bin").err(), Some(Fail::Denied));
    assert_eq!(g.open_screen(&mut m, Badge::Other, &origin("c2")).err(), Some(Fail::Denied));
    assert_eq!(g.reload_policy(&mut m, Badge::Other).err(), Some(Fail::Denied));
    assert_eq!(g.audit_lines(Badge::Other, 4).err(), Some(Fail::Denied));
    assert_eq!(g.cancel(&mut m, Badge::Other, "c1").err(), Some(Fail::Denied));
}

const UPDATING: &str = "effector-policy 1\nagent on\ndeliver data/inbox/\nupdate apply\nmax-bytes 100000\n";

fn updating() -> (Gateway, Fake) {
    let mut m = Fake { now: 1000, policy: Some(UPDATING.as_bytes().to_vec()), remote_update: true, ..Fake::default() };
    let mut g = Gateway::new();
    g.reload_policy(&mut m, Badge::Owner).unwrap();
    (g, m)
}

#[test]
fn an_update_without_update_apply_in_the_policy_reaches_no_updater() {
    let (mut g, mut m) = ready();
    m.remote_update = true;
    let sha = hex(&sha256::digest(b"bundle"));
    assert_eq!(g.update_begin(&mut m, Badge::Session, &origin("u1"), 6, &sha), Err(Fail::Denied), "the policy has no `update` line");
    assert!(m.audit.last().unwrap().contains(" u1 update-begin?6?bytes denied refused "), "{:?}", m.audit);
    m.policy = Some(format!("{}update fetch\n", POLICY).into_bytes());
    g.reload_policy(&mut m, Badge::Owner).unwrap();
    assert_eq!(g.update_begin(&mut m, Badge::Session, &origin("u2"), 6, &sha), Err(Fail::Denied), "`fetch` is below `apply`");
    assert_eq!(g.update_apply(&mut m, Badge::Session, &origin("u3"), 1), Err(Fail::Denied));
    assert!(m.begun.is_empty() && m.applied.is_empty(), "nothing asked of the updater");
}

#[test]
fn with_remote_update_off_nothing_reaches_the_updater() {
    let (mut g, mut m) = updating();
    m.remote_update = false;
    let sha = hex(&sha256::digest(b"bundle"));
    assert_eq!(g.update_begin(&mut m, Badge::Session, &origin("u1"), 6, &sha), Err(Fail::Denied), "remote-update is off");
    assert_eq!(g.update_apply(&mut m, Badge::Session, &origin("u0"), 1), Err(Fail::Denied));
    assert!(m.begun.is_empty() && m.applied.is_empty(), "nothing asked of the updater");
    let lines = g.audit_lines(Badge::Owner, 16).unwrap();
    assert!(lines.iter().any(|l| l.contains(" u1 update-begin?6?bytes allow admitted ")), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains(" u1 update-begin?6?bytes remote-update off ")), "{lines:?}");
    assert!(lines.last().unwrap().contains(" u0 update-apply?1?tries remote-update off "), "{lines:?}");
    // The updater asks init itself: its refusal, should the word go between, is audited the same way.
    m.remote_update = true;
    m.updater_denies = true;
    assert_eq!(g.update_begin(&mut m, Badge::Session, &origin("u1"), 6, &sha), Err(Fail::Denied));
    assert!(g.audit_lines(Badge::Owner, 1).unwrap()[0].contains(" u1 update-begin?6?bytes updater denied "), "{:?}", m.audit);
    assert!(!g.status(&mut m).busy, "no transfer open");
    assert_eq!(g.write(&mut m, Badge::Session, 1, 0, b"bundle"), Err(Fail::NotFound));
    assert_eq!(g.update_begin(&mut m, Badge::Session, &origin("u2"), 6, "not hex"), Err(Fail::Malformed));
    assert_eq!(g.update_begin(&mut m, Badge::Owner, &origin("u3"), 6, &sha), Err(Fail::Denied), "actions come from the session");
}

#[test]
fn a_release_is_streamed_to_the_updater_staged_and_applied() {
    let (mut g, mut m) = updating();
    let bundle: Vec<u8> = (0..70_000u32).map(|i| (i * 13 % 251) as u8).collect();
    let sha = hex(&sha256::digest(&bundle));
    let h = g.update_begin(&mut m, Badge::Session, &origin("u1"), bundle.len() as u64, &sha).unwrap();
    assert_eq!(m.begun, vec![(bundle.len() as u64, sha.clone(), "u1".to_owned())], "the command named for the updater's log");
    assert!(g.status(&mut m).busy);
    assert_eq!(g.update_apply(&mut m, Badge::Session, &origin("u2"), 1), Err(Fail::Busy), "not while a bundle comes");
    assert_eq!(g.deliver_commit(&mut m, Badge::Session, h).err(), Some(Fail::NotFound), "not a delivery");
    for (i, piece) in bundle.chunks(CHUNK).enumerate() {
        if i == 1 { assert_eq!(g.write(&mut m, Badge::Session, h, CHUNK as u64 + 1, piece), Err(Fail::Bounds), "no gap"); }
        assert_eq!(g.write(&mut m, Badge::Session, h, (i * CHUNK) as u64, piece), Ok(()));
    }
    assert_eq!(g.write(&mut m, Badge::Session, h, 0, b"again"), Err(Fail::Bounds), "in order");
    assert_eq!(m.installed, bundle, "every byte went to the updater, none to a file");
    assert!(m.temporary.is_empty() && m.files.is_empty());
    assert_eq!(g.update_commit(&mut m, Badge::Session, h), Ok(6));
    assert_eq!(g.update_commit(&mut m, Badge::Session, h), Err(Fail::NotFound), "the transfer ended");
    assert!(m.audit.last().unwrap().contains(&format!(" u1 update-commit?{}?bytes commit staged?6 ", bundle.len())), "{:?}", m.audit);
    assert_eq!(g.update_apply(&mut m, Badge::Session, &origin("u3"), 0), Err(Fail::Bounds));
    assert_eq!(g.update_apply(&mut m, Badge::Session, &origin("u4"), 3), Ok(()));
    assert_eq!(m.applied, vec![3]);
    assert!(m.audit.last().unwrap().contains(" u4 update-apply?3?tries updater asked "), "audited before the restart: {:?}", m.audit);
    m.apply_refused = Some(Fail::Expired);
    assert_eq!(g.update_apply(&mut m, Badge::Session, &origin("u5"), 1), Err(Fail::Expired));
    assert!(m.audit.last().unwrap().contains(" u5 update-apply?1?tries done expired "), "{:?}", m.audit);
    let state = g.update_state(&mut m, Badge::Session);
    assert_eq!((state.slot, state.running, state.staged, state.confirmed), (b'A', 5, 6, true));
    assert_eq!(g.update_state(&mut m, Badge::Owner), state);
    assert_eq!(g.update_state(&mut m, Badge::Other), UpdateState::default());
}

#[test]
fn a_short_or_changed_bundle_is_not_staged() {
    let (mut g, mut m) = updating();
    let bundle = vec![7u8; 1000];
    let sha = hex(&sha256::digest(&bundle));
    let h = g.update_begin(&mut m, Badge::Session, &origin("u1"), 1000, &sha).unwrap();
    g.write(&mut m, Badge::Session, h, 0, &bundle[..600]).unwrap();
    assert_eq!(g.write(&mut m, Badge::Session, h, 600, &[0; 401]), Err(Fail::Bounds), "past the size begun");
    assert_eq!(g.update_commit(&mut m, Badge::Session, h), Err(Fail::Digest), "short: not handed to the updater");
    assert_eq!(m.commits, 0);
    let h = g.update_begin(&mut m, Badge::Session, &origin("u2"), 1000, &sha).unwrap();
    let mut changed = bundle.clone();
    changed[999] ^= 1;
    g.write(&mut m, Badge::Session, h, 0, &changed).unwrap();
    assert_eq!(g.update_commit(&mut m, Badge::Session, h), Err(Fail::Digest), "the updater's own check");
    assert!(m.audit.last().unwrap().contains(" u2 update-commit?1000?bytes commit digest "), "{:?}", m.audit);
    // Closed or cancelled, a bundle under way ends; the updater drops it at the next begin.
    let h = g.update_begin(&mut m, Badge::Session, &origin("u3"), 1000, &sha).unwrap();
    g.close(&mut m, Badge::Session, h).unwrap();
    assert!(!g.status(&mut m).busy);
    g.update_begin(&mut m, Badge::Session, &origin("u4"), 1000, &sha).unwrap();
    g.cancel(&mut m, Badge::Session, "u4").unwrap();
    assert!(!g.status(&mut m).busy);
}
