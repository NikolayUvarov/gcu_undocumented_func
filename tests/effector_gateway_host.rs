//! The Effector gateway's decisions and effects (750-EFF-0007) against a fake machine: the badge, the deadline, the
//! policy, the audit before every answer (a failed audit write refuses the request, MC-10.6), files out in pieces with
//! their SHA-256, files in checked by size and SHA-256 before they are published.
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
    fn audit(&mut self, line: &str) -> bool { if self.audit_fails { return false; } self.audit.push(line.into()); true }
    fn policy_file(&mut self) -> Result<Vec<u8>, Fail> { self.policy.clone().ok_or(Fail::NotFound) }
}

const POLICY: &str = "effector-policy 1\nagent on\nservice sysmon restart\nlog ring\ncollect data/reports/\ndeliver data/inbox/\nscreen on\nmax-bytes 100000\n";

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
    g.deliver_commit(&mut m, Badge::Session, h).unwrap();
    assert_eq!(m.files["data/inbox/a.bin"], body);
    // The same name again without replacing: refused before a byte is taken.
    assert_eq!(g.deliver_begin(&mut m, Badge::Session, &origin("c4"), "data/inbox/a.bin", 4, &sha, false).err(), Some(Fail::Exists));
    // Bytes that do not match the announced hash are discarded, never published.
    let h = g.deliver_begin(&mut m, Badge::Session, &origin("c5"), "data/inbox/b.bin", 4, &sha, false).unwrap();
    g.write(&mut m, Badge::Session, h, 0, b"evil").unwrap();
    assert_eq!(g.deliver_commit(&mut m, Badge::Session, h).err(), Some(Fail::Digest));
    assert!(!m.files.contains_key("data/inbox/b.bin") && m.temporary.is_empty());
}

#[test]
fn not_yet_cancel_and_status() {
    let (mut g, mut m) = ready();
    assert_eq!(g.not_yet(&mut m, Badge::Session, &origin("c1"), "open-screen", policy::Ask::Screen), Fail::Unsupported, "allowed, not built yet");
    assert_eq!(g.not_yet(&mut m, Badge::Session, &origin("c2"), "reboot", policy::Ask::Reboot), Fail::Denied);
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
    assert_eq!(g.not_yet(&mut m, Badge::Other, &origin("c2"), "open-screen", policy::Ask::Screen), Fail::Denied);
    assert_eq!(g.reload_policy(&mut m, Badge::Other).err(), Some(Fail::Denied));
    assert_eq!(g.audit_lines(Badge::Other, 4).err(), Some(Fail::Denied));
    assert_eq!(g.cancel(&mut m, Badge::Other, "c1").err(), Some(Fail::Denied));
}
