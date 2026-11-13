//! Seeded fuzzing of what the SSH server's role parses (112-NET-0071), as 500-ASR-0001's harness runs the IDL decoders:
//! the plaintext version line, KEXINIT and KEX_ECDH_INIT, random and mutated; and, after a real key exchange with this
//! crate's client, the payloads a client sends (logins, channel messages, console keys, SFTP requests), random and
//! mutated. Never a panic; a console line only for a key with the console's right, the files only for one with theirs;
//! every path the files get is below their root. A run is evidence of the executions it made, not a proof (MC-12.2).
//! MIND_FUZZ_SEED and MIND_FUZZ_ITERATIONS override the fixed seed and the counts.
#![cfg(feature = "server")]
use sha2::Digest;
use ssh::server::{self, Attrs, Event, Files, Owner, Rights, FAILURE, NO_SUCH_FILE};
use ssh::transport::Transport;
use ssh::{Error, Io, Keys};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn below(&mut self, n: usize) -> usize { (self.next() % n.max(1) as u64) as usize }
}

fn env(name: &str, default: u64) -> u64 { std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default) }

// The server's random bytes: fixed, so a run repeats; the exchange's secrets need none here.
static COUNTER: AtomicU64 = AtomicU64::new(1);
fn random(out: &mut [u8]) -> bool {
    let mut x = COUNTER.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed) | 1;
    for b in out { x ^= x << 13; x ^= x >> 7; x ^= x << 17; *b = x as u8; }
    true
}

const PIECES: &[&[u8]] = &[b"\0\0\0\0", b"\xff\xff\xff\xff", b"\0\0\0\x01", b"\0\0\x80\0", b"ssh-ed25519", b"publickey", b"none", b"password",
    b"session", b"pty-req", b"shell", b"exec", b"subsystem", b"sftp", b"ssh-connection", b"ssh-userauth", b"/", b"..", b"/../..", b"\r", b"\n",
    b"\x03", b"\x04", b"\x7f", b"curve25519-sha256", b"kex-strict-c-v00@openssh.com", b"chacha20-poly1305@openssh.com"];

fn mutate(rng: &mut Rng, input: &mut Vec<u8>, max: usize) {
    for _ in 0..1 + rng.below(4) {
        let at = rng.below(input.len() + 1);
        match rng.below(6) {
            0 if !input.is_empty() => { let i = rng.below(input.len()); input[i] = rng.next() as u8; }
            1 if !input.is_empty() => { let end = (at + rng.below(8)).min(input.len()); input.drain(at.min(end)..end); }
            2 => { let piece = PIECES[rng.below(PIECES.len())]; input.splice(at..at, piece.iter().copied()); }
            3 => { let n = rng.below(16); input.splice(at..at, (0..n).map(|_| rng.next() as u8)); }
            4 if input.len() >= 4 => { let i = rng.below(input.len() - 3); input[i..i + 4].copy_from_slice(&(rng.below(70_000) as u32).to_be_bytes()); }
            _ => { let n = rng.below(input.len() + 1); let copy = input[..n].to_vec(); input.extend_from_slice(&copy); }
        }
    }
    input.truncate(max);
}

fn string(out: &mut Vec<u8>, s: &[u8]) { out.extend_from_slice(&(s.len() as u32).to_be_bytes()); out.extend_from_slice(s); }

// A plaintext packet (RFC 4253 section 6): padding to a multiple of 8, at least 4.
fn packet(payload: &[u8]) -> Vec<u8> {
    let pad = 4 + (8 - (payload.len() + 9) % 8) % 8;
    let mut out = Vec::new();
    out.extend_from_slice(&((payload.len() + pad + 1) as u32).to_be_bytes());
    out.push(pad as u8);
    out.extend_from_slice(payload);
    out.extend(std::iter::repeat(0).take(pad));
    out
}

fn kexinit() -> Vec<u8> {
    let mut p = vec![20u8];
    p.extend_from_slice(&[7; 16]);
    for list in [&b"curve25519-sha256,kex-strict-c-v00@openssh.com"[..], b"ssh-ed25519", b"chacha20-poly1305@openssh.com", b"chacha20-poly1305@openssh.com",
        b"hmac-sha2-256", b"hmac-sha2-256", b"none", b"none", b"", b""] { string(&mut p, list); }
    p.push(0);
    p.extend_from_slice(&[0; 4]);
    p
}

struct HostKey(ed25519_dalek::SigningKey, Arc<AtomicU64>);
impl Keys for HostKey {
    fn public(&self) -> [u8; 32] { self.0.verifying_key().to_bytes() }
    fn sign(&mut self, data: &[u8]) -> Option<[u8; 64]> {
        use ed25519_dalek::Signer;
        assert_eq!(data.len(), 32, "the host key signs an exchange hash");
        self.1.fetch_add(1, Ordering::Relaxed);
        Some(self.0.sign(data).to_bytes())
    }
}

// Files in memory, refusing what would grow without bound; each path checked to be below the root.
#[derive(Default)]
struct Memory { files: HashMap<String, Vec<u8>>, dirs: HashSet<String>, open: HashMap<u32, String>, next: u32 }

fn below_root(path: &str) {
    assert!(path.starts_with('/'), "a path not from the root: {path:?}");
    if path != "/" { assert!(path[1..].split('/').all(|n| !n.is_empty() && n != "." && n != ".."), "a path not normal: {path:?}"); }
}

impl Files for Memory {
    fn open(&mut self, path: &str, write: bool, create: bool, truncate: bool) -> Result<u32, u32> {
        below_root(path);
        if !self.files.contains_key(path) { if !(write && create) || self.files.len() >= 32 { return Err(NO_SUCH_FILE); } self.files.insert(path.into(), Vec::new()); }
        if truncate && write { self.files.get_mut(path).unwrap().clear(); }
        if self.open.len() >= 16 { return Err(FAILURE); }
        self.next += 1;
        self.open.insert(self.next, path.into());
        Ok(self.next)
    }
    fn read(&mut self, handle: u32, offset: u64, out: &mut [u8]) -> Result<usize, u32> {
        let data = self.files.get(self.open.get(&handle).ok_or(FAILURE)?).ok_or(FAILURE)?;
        let start = (offset.min(data.len() as u64)) as usize;
        let n = out.len().min(data.len() - start);
        out[..n].copy_from_slice(&data[start..start + n]);
        Ok(n)
    }
    fn write(&mut self, handle: u32, offset: u64, data: &[u8]) -> Result<(), u32> {
        let file = self.files.get_mut(self.open.get(&handle).ok_or(FAILURE)?).ok_or(FAILURE)?;
        let end = offset.checked_add(data.len() as u64).filter(|&e| e <= 1 << 20).ok_or(FAILURE)? as usize;
        if file.len() < end { file.resize(end, 0); }
        file[offset as usize..end].copy_from_slice(data);
        Ok(())
    }
    fn close(&mut self, handle: u32) -> Result<(), u32> { self.open.remove(&handle).map(drop).ok_or(FAILURE) }
    fn stat(&mut self, path: &str) -> Result<Attrs, u32> {
        below_root(path);
        if path == "/" || self.dirs.contains(path) { return Ok(Attrs { size: 0, directory: true, modified: 0 }); }
        self.files.get(path).map(|f| Attrs { size: f.len() as u64, directory: false, modified: 0 }).ok_or(NO_SUCH_FILE)
    }
    fn fstat(&mut self, handle: u32) -> Result<Attrs, u32> { let path = self.open.get(&handle).ok_or(FAILURE)?.clone(); self.stat(&path) }
    fn list(&mut self, path: &str) -> Result<Vec<(String, Attrs)>, u32> {
        below_root(path);
        if path != "/" && !self.dirs.contains(path) { return Err(NO_SUCH_FILE); }
        Ok(self.files.iter().map(|(name, f)| (name.rsplit('/').next().unwrap().to_string(), Attrs { size: f.len() as u64, directory: false, modified: 0 })).collect())
    }
    fn remove(&mut self, path: &str) -> Result<(), u32> { below_root(path); self.files.remove(path).map(drop).ok_or(NO_SUCH_FILE) }
    fn mkdir(&mut self, path: &str) -> Result<(), u32> { below_root(path); if self.dirs.len() >= 32 { return Err(FAILURE); } self.dirs.insert(path.into()); Ok(()) }
    fn rmdir(&mut self, path: &str) -> Result<(), u32> { below_root(path); if self.dirs.remove(path) { Ok(()) } else { Err(NO_SUCH_FILE) } }
    fn rename(&mut self, from: &str, to: &str) -> Result<(), u32> {
        below_root(from);
        below_root(to);
        let data = self.files.remove(from).ok_or(NO_SUCH_FILE)?;
        self.files.insert(to.into(), data);
        Ok(())
    }
}

#[derive(Default)]
struct Counts { logins: u64, commands: u64, files: u64 }

struct FuzzOwner { keys: Vec<([u8; 32], Rights)>, fingerprints: Vec<([u8; 32], Rights)>, rights: Option<Rights>, files: Memory, counts: Arc<Mutex<Counts>> }

impl FuzzOwner {
    fn new(keys: Vec<([u8; 32], Rights)>, counts: Arc<Mutex<Counts>>) -> Self {
        let fingerprints = keys.iter().map(|(k, r)| (sha2::Sha256::digest(ssh::login::key_blob(k)).into(), *r)).collect();
        FuzzOwner { keys, fingerprints, rights: None, files: Memory::default(), counts }
    }
}

impl Owner for FuzzOwner {
    fn rights(&mut self, user: &[u8], key: &[u8; 32]) -> Option<Rights> {
        if user != b"mind" { return None; }
        self.keys.iter().find(|(k, _)| k == key).map(|&(_, r)| r)
    }
    fn command(&mut self, line: &str, out: &mut dyn FnMut(&[u8])) -> bool {
        assert!(self.rights.is_some_and(|r| r.console), "a console line without the console's right: {line:?}");
        assert!(line.len() <= server::LINE_MAX);
        self.counts.lock().unwrap().commands += 1;
        out(line.as_bytes());
        out(b"\n");
        line != "exit"
    }
    fn prompt(&self) -> &str { "mind> " }
    fn files(&mut self) -> &mut dyn Files {
        assert!(self.rights.is_some_and(|r| r.files), "the files reached without their right");
        self.counts.lock().unwrap().files += 1;
        &mut self.files
    }
    fn event(&mut self, event: Event<'_>) {
        if let Event::Login { user, key } = event {
            assert_eq!(user, b"mind");
            let rights = self.fingerprints.iter().find(|(f, _)| *f == key).map(|&(_, r)| r);
            assert!(rights.is_some(), "a login with a key not listed");
            assert!(self.rights.is_none(), "a second login");
            self.rights = rights;
            self.counts.lock().unwrap().logins += 1;
        }
    }
}

// One end of an in-memory connection.
struct Pipe { to: Option<Sender<Vec<u8>>>, from: Receiver<Vec<u8>>, input: Vec<u8> }

impl Io for Pipe {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        // The other end gone: its bytes are dropped, as a closed socket's are, and the read after says so.
        if let Some(to) = &self.to { if to.send(data.to_vec()).is_err() { self.to = None; } }
        Ok(())
    }
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        if self.input.is_empty() { match self.from.recv() { Ok(data) => self.input = data, Err(_) => return Ok(0) } }
        let n = buffer.len().min(self.input.len());
        buffer[..n].copy_from_slice(&self.input[..n]);
        self.input.drain(..n);
        Ok(n)
    }
}

fn pipe() -> (Pipe, Pipe) {
    let (a_to, b_from) = channel();
    let (b_to, a_from) = channel();
    (Pipe { to: Some(a_to), from: a_from, input: Vec::new() }, Pipe { to: Some(b_to), from: b_from, input: Vec::new() })
}

fn host() -> ed25519_dalek::SigningKey { ed25519_dalek::SigningKey::from_bytes(&[0x42; 32]) }

const CONSOLE: Rights = Rights { console: true, files: false };
const FILES: Rights = Rights { console: false, files: true };

fn listed() -> (Vec<ed25519_dalek::SigningKey>, Vec<([u8; 32], Rights)>) {
    let keys: Vec<ed25519_dalek::SigningKey> = (1..=3u8).map(|i| ed25519_dalek::SigningKey::from_bytes(&[i; 32])).collect();
    let rights = vec![(keys[0].verifying_key().to_bytes(), CONSOLE), (keys[1].verifying_key().to_bytes(), FILES)];
    (keys, rights)
}

// Runs the server on `io` in a thread; its panic is the test's.
fn spawn(io: Pipe, keys: Vec<([u8; 32], Rights)>, counts: Arc<Mutex<Counts>>, signed: Arc<AtomicU64>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut owner = FuzzOwner::new(keys, counts);
        let _ = server::serve(io, &mut HostKey(host(), signed), &mut owner, random);
    })
}

#[test]
fn the_key_exchange_on_random_and_mutated_input() {
    let mut rng = Rng(env("MIND_FUZZ_SEED", 0x6d69_6e64_7373_6831) | 1);
    let iterations = env("MIND_FUZZ_ITERATIONS", 3000);
    let mut q_c = [9u8; 32];
    q_c[0] = 0x55;
    let mut init = vec![30u8];
    string(&mut init, &q_c);
    let mut seed = b"SSH-2.0-OpenSSH_9.6\r\n".to_vec();
    for p in [packet(&kexinit()), packet(&init), packet(&[21])] { seed.extend_from_slice(&p); }
    let signed = Arc::new(AtomicU64::new(0));
    let counts = Arc::new(Mutex::new(Counts::default()));
    let keys = listed().1;
    for i in 0..iterations {
        let mut input = if i % 8 == 0 { (0..rng.below(400)).map(|_| rng.next() as u8).collect() } else { seed.clone() };
        if i % 16 != 1 { mutate(&mut rng, &mut input, 4096); }
        let (mut client, server) = pipe();
        // The client's bytes in pieces of random sizes, then its end closed.
        let mut at = 0;
        while at < input.len() { let n = 1 + rng.below(300); client.send(&input[at..(at + n).min(input.len())]).unwrap(); at += n; }
        drop(client.to.take());
        let server = spawn(server, keys.clone(), counts.clone(), signed.clone());
        if server.join().is_err() { panic!("the server panicked on input {i} of seed {:#x}", env("MIND_FUZZ_SEED", 0x6d69_6e64_7373_6831)); }
    }
    let signed = signed.load(Ordering::Relaxed);
    // The run reached past the version line and the lists: the host key signed exchanges.
    assert!(signed > iterations / 20, "signed {signed} of {iterations}");
    println!("ssh server fuzz, key exchange: {iterations} inputs, {signed} exchanges signed");
}

// The payloads of a client that logs in as `user` with `key` and opens a session for `what`.
fn script(session: &[u8; 32], key: &ed25519_dalek::SigningKey, what: usize) -> Vec<Vec<u8>> {
    use ed25519_dalek::Signer;
    let public = key.verifying_key().to_bytes();
    let mut out = Vec::new();
    let mut service = vec![5u8];
    string(&mut service, b"ssh-userauth");
    out.push(service);
    let mut none = vec![50u8];
    for s in [&b"mind"[..], b"ssh-connection", b"none"] { string(&mut none, s); }
    out.push(none);
    let mut login = vec![50u8];
    for s in [&b"mind"[..], b"ssh-connection", b"publickey"] { string(&mut login, s); }
    login.push(1);
    string(&mut login, b"ssh-ed25519");
    string(&mut login, &ssh::login::key_blob(&public));
    let mut sig = Vec::new();
    string(&mut sig, b"ssh-ed25519");
    string(&mut sig, &key.sign(&ssh::login::signed_data(session, b"mind", &public)).to_bytes());
    string(&mut login, &sig);
    out.push(login);
    let mut open = vec![90u8];
    string(&mut open, b"session");
    open.extend_from_slice(&0u32.to_be_bytes());
    open.extend_from_slice(&(if what == 2 { 64u32 } else { 1 << 20 }).to_be_bytes());
    open.extend_from_slice(&32768u32.to_be_bytes());
    out.push(open);
    let request = |kind: &[u8], extra: &[u8]| { let mut r = vec![98u8, 0, 0, 0, 0]; string(&mut r, kind); r.push(1); r.extend_from_slice(extra); r };
    let data = |bytes: &[u8]| { let mut d = vec![94u8, 0, 0, 0, 0]; string(&mut d, bytes); d };
    match what {
        0 | 2 => {
            out.push(request(b"pty-req", &[]));
            out.push(request(b"shell", &[]));
            out.push(data(b"echo hello\r\nps\n\x7f\x7fx\x03ls\r"));
        }
        1 => {
            let mut sub = Vec::new();
            string(&mut sub, b"sftp");
            out.push(request(b"subsystem", &sub));
            let sftp = |kind: u8, body: &[u8]| { let mut m = vec![kind]; m.extend_from_slice(&7u32.to_be_bytes()); m.extend_from_slice(body); let mut f = Vec::new(); string(&mut f, &m); f };
            out.push(data(&[0, 0, 0, 5, 1, 0, 0, 0, 3]));
            let mut open = Vec::new();
            string(&mut open, b"/a/../b");
            open.extend_from_slice(&(2u32 | 8 | 0x10).to_be_bytes());
            open.extend_from_slice(&0u32.to_be_bytes());
            out.push(data(&sftp(3, &open)));
            let mut write = Vec::new();
            string(&mut write, &1u32.to_be_bytes());
            write.extend_from_slice(&0u64.to_be_bytes());
            string(&mut write, b"hello");
            out.push(data(&sftp(6, &write)));
            let mut read = Vec::new();
            string(&mut read, &1u32.to_be_bytes());
            read.extend_from_slice(&0u64.to_be_bytes());
            read.extend_from_slice(&4096u32.to_be_bytes());
            out.push(data(&sftp(5, &read)));
            let mut dir = Vec::new();
            string(&mut dir, b"/");
            out.push(data(&sftp(11, &dir)));
            let mut handle = Vec::new();
            string(&mut handle, &0x8000_0000u32.to_be_bytes());
            out.push(data(&sftp(12, &handle)));
            let mut path = Vec::new();
            string(&mut path, b"../../etc");
            out.push(data(&sftp(16, &path)));
            out.push(data(&sftp(17, &path)));
            let mut rename = Vec::new();
            string(&mut rename, b"/b");
            string(&mut rename, b"c/./d");
            out.push(data(&sftp(18, &rename)));
        }
        _ => {
            let mut command = Vec::new();
            string(&mut command, b"echo one");
            out.push(request(b"exec", &command));
        }
    }
    let mut adjust = vec![93u8, 0, 0, 0, 0];
    adjust.extend_from_slice(&4096u32.to_be_bytes());
    out.push(adjust);
    out.push(vec![96, 0, 0, 0, 0]);
    out.push(vec![97, 0, 0, 0, 0]);
    out
}

#[test]
fn the_payloads_after_the_key_exchange() {
    let mut rng = Rng(env("MIND_FUZZ_SEED", 0x6d69_6e64_7373_6832) | 1);
    let iterations = env("MIND_FUZZ_ITERATIONS", 3000) / 3;
    let pin: [u8; 32] = sha2::Sha256::digest(ssh::login::key_blob(&host().verifying_key().to_bytes())).into();
    let (keys, rights) = listed();
    let signed = Arc::new(AtomicU64::new(0));
    let counts = Arc::new(Mutex::new(Counts::default()));
    let mut client_random = |b: &mut [u8]| random(b);
    for i in 0..iterations {
        let (client, server) = pipe();
        let server = spawn(server, rights.clone(), counts.clone(), signed.clone());
        let mut t = Transport::connect(client, &pin, &mut client_random).expect("the key exchange with this crate's client");
        let key = &keys[rng.below(keys.len())];
        let mut payloads = script(&t.session_id, key, rng.below(4));
        // Each payload kept, mutated, dropped or followed by a random one; some runs keep the whole script.
        if i % 8 != 0 {
            let mut fuzzed = Vec::new();
            for mut p in payloads {
                match rng.below(8) {
                    0 => continue,
                    1 | 2 => mutate(&mut rng, &mut p, 2048),
                    3 => fuzzed.push((0..rng.below(64)).map(|_| rng.next() as u8).collect()),
                    _ => {}
                }
                if !p.is_empty() { fuzzed.push(p); }
            }
            payloads = fuzzed;
        }
        for p in &payloads { if t.send(p, &mut client_random).is_err() { break; } }
        drop(t);
        if server.join().is_err() { panic!("the server panicked on run {i} of seed {:#x}", env("MIND_FUZZ_SEED", 0x6d69_6e64_7373_6832)); }
    }
    let c = counts.lock().unwrap();
    // The run reached past the logins into the console and the files.
    assert!(c.logins > iterations / 8 && c.commands > 0 && c.files > 0, "logins {}, console lines {}, file calls {}", c.logins, c.commands, c.files);
    println!("ssh server fuzz, payloads: {iterations} connections, {} logins, {} console lines, {} file calls", c.logins, c.commands, c.files);
}
