//! Host tests of the SSH server's role (112-NET-0071) against OpenSSH's client: the server runs in a thread on
//! 127.0.0.1 with a test owner (one listed key, a console of three commands, a temporary directory as its files), and
//! `ssh`, `scp` and `sftp` talk to it with keys ssh-keygen makes. Checked: the host key as known_hosts names it; a
//! command run; a console with a terminal; an unlisted key, a password and a key without the console's right refused;
//! a file copied up and down whole with scp (SFTP), sftp's directory work, a path outside the root refused; each login,
//! refusal and write in the owner's log. Needs OpenSSH's client (`MIND_SSH`, default ssh) and ssh-keygen.
#![cfg(feature = "server")]
use ssh::server::{self, Attrs, Event, Files, Owner, Rights, NO_SUCH_FILE, PERMISSION_DENIED, FAILURE};
use ssh::{Error, Io, Keys};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 { out.push(if i <= chunk.len() { B64[(n >> (18 - 6 * i)) as usize & 63] as char } else { '=' }); }
    }
    out
}

fn unbase64(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut n, mut bits) = (0u32, 0);
    for c in text.bytes().take_while(|&c| c != b'=') {
        n = n << 6 | B64.iter().position(|&b| b == c).expect("base64") as u32;
        bits += 6;
        if bits >= 8 { bits -= 8; out.push((n >> bits) as u8); }
    }
    out
}

fn random(out: &mut [u8]) -> bool { std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(out)).is_ok() }

struct Tcp(TcpStream);

impl Io for Tcp {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> { self.0.write_all(data).map_err(|_| Error::Io) }
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        loop {
            match self.0.read(buffer) {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                other => return other.map_err(|_| Error::Io),
            }
        }
    }
}

struct HostKey(ed25519_dalek::SigningKey);

impl Keys for HostKey {
    fn public(&self) -> [u8; 32] { self.0.verifying_key().to_bytes() }
    fn sign(&mut self, data: &[u8]) -> Option<[u8; 64]> {
        use ed25519_dalek::Signer;
        // The host key signs the exchange hash alone: 32 bytes.
        assert_eq!(data.len(), 32, "the host key signs an exchange hash");
        Some(self.0.sign(data).to_bytes())
    }
}

// The owner's files: a directory on the host, below which every path is.
struct Dir { root: PathBuf, open: HashMap<u32, std::fs::File>, next: u32 }

fn host(e: std::io::Error) -> u32 { if e.kind() == std::io::ErrorKind::NotFound { NO_SUCH_FILE } else { FAILURE } }

impl Dir {
    fn at(&self, path: &str) -> PathBuf { self.root.join(path.trim_start_matches('/')) }
    fn attrs(m: &std::fs::Metadata) -> Attrs { Attrs { size: m.len(), directory: m.is_dir(), modified: 0 } }
}

impl Files for Dir {
    fn open(&mut self, path: &str, write: bool, create: bool, truncate: bool) -> Result<u32, u32> {
        let file = std::fs::OpenOptions::new().read(true).write(write).create(create && write).truncate(truncate && write).open(self.at(path)).map_err(host)?;
        self.next += 1;
        self.open.insert(self.next, file);
        Ok(self.next)
    }
    fn read(&mut self, handle: u32, offset: u64, out: &mut [u8]) -> Result<usize, u32> {
        use std::os::unix::fs::FileExt;
        self.open.get(&handle).ok_or(FAILURE)?.read_at(out, offset).map_err(host)
    }
    fn write(&mut self, handle: u32, offset: u64, data: &[u8]) -> Result<(), u32> {
        use std::os::unix::fs::FileExt;
        self.open.get(&handle).ok_or(FAILURE)?.write_all_at(data, offset).map_err(host)
    }
    fn close(&mut self, handle: u32) -> Result<(), u32> { self.open.remove(&handle).map(drop).ok_or(FAILURE) }
    fn stat(&mut self, path: &str) -> Result<Attrs, u32> { std::fs::metadata(self.at(path)).map(|m| Dir::attrs(&m)).map_err(host) }
    fn fstat(&mut self, handle: u32) -> Result<Attrs, u32> { self.open.get(&handle).ok_or(FAILURE)?.metadata().map(|m| Dir::attrs(&m)).map_err(host) }
    fn list(&mut self, path: &str) -> Result<Vec<(String, Attrs)>, u32> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(self.at(path)).map_err(host)? {
            let entry = entry.map_err(host)?;
            out.push((entry.file_name().to_string_lossy().into_owned(), Dir::attrs(&entry.metadata().map_err(host)?)));
        }
        Ok(out)
    }
    fn remove(&mut self, path: &str) -> Result<(), u32> { std::fs::remove_file(self.at(path)).map_err(host) }
    fn mkdir(&mut self, path: &str) -> Result<(), u32> { std::fs::create_dir(self.at(path)).map_err(host) }
    fn rmdir(&mut self, path: &str) -> Result<(), u32> { std::fs::remove_dir(self.at(path)).map_err(host) }
    fn rename(&mut self, from: &str, to: &str) -> Result<(), u32> { if to == "/" { return Err(PERMISSION_DENIED); } std::fs::rename(self.at(from), self.at(to)).map_err(host) }
}

struct TestOwner { keys: Vec<([u8; 32], Rights)>, files: Dir, log: Arc<Mutex<Vec<String>>> }

impl Owner for TestOwner {
    fn rights(&mut self, user: &[u8], key: &[u8; 32]) -> Option<Rights> {
        if user != b"mind" { return None; }
        self.keys.iter().find(|(k, _)| k == key).map(|&(_, r)| r)
    }
    fn command(&mut self, line: &str, out: &mut dyn FnMut(&[u8])) -> bool {
        match line.split_once(' ').unwrap_or((line, "")) {
            ("echo", rest) => out(format!("{rest}\n").as_bytes()),
            ("ps", _) => out(b"PID NAME\n1 init\n"),
            ("exit", _) => return false,
            _ => out(b"ERROR: UNKNOWN COMMAND\n"),
        }
        true
    }
    fn prompt(&self) -> &str { "MIND> " }
    fn files(&mut self) -> &mut dyn Files { &mut self.files }
    fn event(&mut self, event: Event<'_>) {
        let line = match event {
            Event::Login { user, key } => format!("LOGIN {} SHA256:{}", String::from_utf8_lossy(user), base64(&key).trim_end_matches('=')),
            Event::Refused { user, why } => format!("REFUSED {} ({why})", String::from_utf8_lossy(user)),
            Event::Command { line } => format!("COMMAND {line}"),
            Event::Wrote { path } => format!("WROTE {path}"),
            Event::Removed { path } => format!("REMOVED {path}"),
            Event::End { why } => format!("END {why}"),
        };
        self.log.lock().unwrap().push(line);
    }
}

// The seed of the key this crate's own client logs in with.
const OUR_CLIENT: [u8; 32] = [3; 32];

struct ClientKey(ed25519_dalek::SigningKey);

impl Keys for ClientKey {
    fn public(&self) -> [u8; 32] { self.0.verifying_key().to_bytes() }
    fn sign(&mut self, data: &[u8]) -> Option<[u8; 64]> { use ed25519_dalek::Signer; Some(self.0.sign(data).to_bytes()) }
}

struct Fixture { dir: PathBuf, port: u16, log: Arc<Mutex<Vec<String>>>, files: PathBuf, host: [u8; 32] }

fn keygen(path: &Path) -> [u8; 32] {
    let made = Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-C", "test", "-f"]).arg(path).status().expect("ssh-keygen");
    assert!(made.success());
    let public = std::fs::read_to_string(path.with_extension("pub")).unwrap();
    let blob = unbase64(public.split_whitespace().nth(1).unwrap());
    blob[blob.len() - 32..].try_into().unwrap()
}

// A server with two listed keys (the console and files, the files alone) and one key that is not listed.
fn start(tag: &str) -> Fixture {
    let dir = std::env::temp_dir().join(format!("mind-sshd-test-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let files = dir.join("files");
    std::fs::create_dir_all(&files).unwrap();
    let both = keygen(&dir.join("both"));
    let only_files = keygen(&dir.join("files-only"));
    let _ = keygen(&dir.join("stranger"));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let log = Arc::new(Mutex::new(Vec::new()));
    let seed = [7u8; 32];
    let host = ed25519_dalek::SigningKey::from_bytes(&seed).verifying_key().to_bytes();
    let (thread_log, root) = (log.clone(), files.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let ours = ed25519_dalek::SigningKey::from_bytes(&OUR_CLIENT).verifying_key().to_bytes();
            let mut owner = TestOwner { keys: vec![(both, Rights { console: true, files: true }), (only_files, Rights { console: false, files: true }), (ours, Rights { console: false, files: true })],
                                        files: Dir { root: root.clone(), open: HashMap::new(), next: 0 }, log: thread_log.clone() };
            let _ = server::serve(Tcp(stream), &mut HostKey(ed25519_dalek::SigningKey::from_bytes(&seed)), &mut owner, random);
        }
    });
    // known_hosts names the host key as the server's own: StrictHostKeyChecking=yes then proves its signature.
    let blob = ssh::login::key_blob(&host);
    std::fs::write(dir.join("known_hosts"), format!("[127.0.0.1]:{port} ssh-ed25519 {}\n", base64(&blob))).unwrap();
    Fixture { dir, port, log, files, host }
}

impl Fixture {
    fn options(&self, key: &str) -> Vec<String> {
        ["-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes", "-o", "IdentitiesOnly=yes", "-o", "LogLevel=ERROR", "-o", "ConnectTimeout=20"]
            .iter().map(|s| s.to_string())
            .chain(["-o".into(), format!("UserKnownHostsFile={}", self.dir.join("known_hosts").display()), "-i".into(), self.dir.join(key).display().to_string()])
            .collect()
    }
    fn ssh(&self, key: &str, extra: &[&str], stdin: &[u8]) -> Output {
        let mut child = Command::new(std::env::var("MIND_SSH").unwrap_or("ssh".into())).args(self.options(key)).args(["-p", &self.port.to_string()]).args(extra)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("ssh");
        child.stdin.take().unwrap().write_all(stdin).unwrap();
        child.wait_with_output().unwrap()
    }
    fn scp(&self, key: &str, from: &str, to: &str) -> Output {
        Command::new("scp").args(self.options(key)).args(["-P", &self.port.to_string(), from, to]).output().expect("scp")
    }
    fn log(&self) -> String { self.log.lock().unwrap().join("\n") }
}

fn text(o: &Output) -> String { format!("status {:?}\nout: {}\nerr: {}", o.status.code(), String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)) }

#[test]
fn a_command_and_a_console() {
    let f = start("console");
    assert_eq!(f.host, ed25519_dalek::SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes());
    let o = f.ssh("both", &["mind@127.0.0.1", "echo", "hello", "mind"], b"");
    assert!(o.status.success() && o.stdout == b"hello mind\n", "{}\n{}", text(&o), f.log());
    let o = f.ssh("both", &["mind@127.0.0.1", "ps"], b"");
    assert_eq!(String::from_utf8_lossy(&o.stdout), "PID NAME\n1 init\n", "{}", text(&o));
    // A console with a terminal: each key echoed, lines CR LF, `exit` ends it.
    let o = f.ssh("both", &["-tt", "mind@127.0.0.1"], b"echo one\nnosuch\nexit\n");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("MIND> echo one\r\none\r\nMIND> nosuch\r\nERROR: UNKNOWN COMMAND\r\nMIND> exit\r\n"), "{}", text(&o));
    let log = f.log();
    assert!(log.contains("LOGIN mind SHA256:") && log.contains("COMMAND echo hello mind") && log.contains("COMMAND nosuch") && log.contains("END closed"), "{log}");
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn refusals() {
    let f = start("refusals");
    let o = f.ssh("stranger", &["mind@127.0.0.1", "ps"], b"");
    assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("Permission denied (publickey)"), "{}", text(&o));
    // A client that would use a password: the server names publickey alone, so none is sent.
    let o = f.ssh("both", &["-o", "PubkeyAuthentication=no", "-o", "PreferredAuthentications=password,keyboard-interactive", "mind@127.0.0.1", "ps"], b"");
    assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("Permission denied (publickey)"), "{}", text(&o));
    let o = f.ssh("both", &["root@127.0.0.1", "ps"], b"");
    assert!(!o.status.success(), "{}", text(&o));
    // The key with the files' right alone gets no console.
    let o = f.ssh("files-only", &["mind@127.0.0.1", "ps"], b"");
    assert!(!o.status.success() && o.stdout.is_empty(), "{}", text(&o));
    // A server whose host key known_hosts does not name is refused by the client.
    std::fs::write(f.dir.join("known_hosts"), format!("[127.0.0.1]:{} ssh-ed25519 {}\n", f.port, base64(&ssh::login::key_blob(&[9; 32])))).unwrap();
    let o = f.ssh("both", &["mind@127.0.0.1", "ps"], b"");
    assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("HOST IDENTIFICATION HAS CHANGED"), "{}", text(&o));
    let log = f.log();
    assert!(log.contains("REFUSED mind (a key not listed)") && log.contains("REFUSED root (a key not listed)"), "{log}");
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn files_with_scp_and_sftp() {
    let f = start("files");
    let local = f.dir.join("blob.bin");
    let data: Vec<u8> = (0..300_000u32).map(|i| (i * 31 % 251) as u8).collect();
    std::fs::write(&local, &data).unwrap();
    let remote = format!("mind@127.0.0.1:/blob.bin");
    let o = f.scp("files-only", local.to_str().unwrap(), &remote);
    assert!(o.status.success(), "{}\n{}", text(&o), f.log());
    assert_eq!(std::fs::read(f.files.join("blob.bin")).unwrap(), data, "written whole");
    let back = f.dir.join("back.bin");
    let o = f.scp("files-only", &remote, back.to_str().unwrap());
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(std::fs::read(&back).unwrap(), data, "read back whole");
    // Outside the root: refused, nothing written above it.
    let o = f.scp("files-only", local.to_str().unwrap(), "mind@127.0.0.1:/../outside.bin");
    assert!(!o.status.success(), "{}", text(&o));
    assert!(!f.dir.join("outside.bin").exists());
    // sftp's directory work.
    let batch = f.dir.join("batch");
    std::fs::write(&batch, "mkdir sub\nput blob.bin sub/b2.bin\nls -l sub\nrename sub/b2.bin sub/b3.bin\nget sub/b3.bin\nrm sub/b3.bin\nrmdir sub\n").unwrap();
    let o = Command::new("sftp").args(f.options("files-only")).args(["-P", &f.port.to_string(), "-b"]).arg(&batch).arg("mind@127.0.0.1").current_dir(&f.dir).output().unwrap();
    assert!(o.status.success() && String::from_utf8_lossy(&o.stdout).contains("b2.bin"), "{}", text(&o));
    assert_eq!(std::fs::read(f.dir.join("b3.bin")).unwrap(), data);
    assert!(!f.files.join("sub").exists());
    let log = f.log();
    assert!(log.contains("WROTE /blob.bin") && log.contains("WROTE /sub/b2.bin") && log.contains("REMOVED /sub/b3.bin") && log.contains("REMOVED /sub"), "{log}");
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn this_crates_own_client() {
    // The client of 351-NET-0026 against the server: the host key pinned, a file read over SFTP.
    let f = start("own");
    let data: Vec<u8> = (0..70_000u32).map(|i| (i % 253) as u8).collect();
    std::fs::write(f.files.join("own.bin"), &data).unwrap();
    let pin: [u8; 32] = { use sha2::Digest; sha2::Sha256::digest(ssh::login::key_blob(&f.host)).into() };
    let stream = TcpStream::connect(("127.0.0.1", f.port)).unwrap();
    let mut key = ClientKey(ed25519_dalek::SigningKey::from_bytes(&OUR_CLIENT));
    let mut session = ssh::connect(Tcp(stream), "mind", &pin, &mut key, random).expect("logged in");
    let file = session.open("/own.bin").unwrap();
    assert_eq!(session.size(&file).unwrap(), Some(data.len() as u64));
    let mut got = Vec::new();
    let mut buffer = vec![0u8; ssh::session::READ_MAX];
    loop {
        let n = session.read(&file, got.len() as u64, &mut buffer).unwrap();
        if n == 0 { break; }
        got.extend_from_slice(&buffer[..n]);
    }
    assert_eq!(got, data);
    session.close(file).unwrap();
    assert!(matches!(session.open("/missing.bin"), Err(Error::Status(2))));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn paths_stay_below_the_root() {
    assert_eq!(server::normal("/a/./b//c").as_deref(), Some("/a/b/c"));
    assert_eq!(server::normal("a/b/../c").as_deref(), Some("/a/c"));
    assert_eq!(server::normal(".").as_deref(), Some("/"));
    assert_eq!(server::normal("/a/../.."), None);
    assert_eq!(server::normal("/a\u{1}"), None);
}
