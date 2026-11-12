#![no_std]
#![no_main]
// sshd (112-NET-0072): logins into a MIND machine with the keys its owner lists, and file copy over SFTP (OpenSSH's
// `scp` since 9.0, and `sftp`). The protocol is the `ssh` crate's server role (112-NET-0071). init starts it only when
// the owner's policy gives program `sshd` a flow, and gives it that flow (whose listening grant must name TCP 22),
// keystore's client badged for `ssh-host` (the device key signs each key exchange; it never leaves keystore, MC-11.9),
// and a writable scope of the owner's data/. Nothing else: no other files, no keys, no devices (MC-11.4).
// The keys: data/ssh/authorized_keys, read at each connection; each line names what its key may do (`console`,
// `files`). SFTP reaches data/ but data/ssh/. Nothing listens until the file lists a key. One session at a time; a
// login within 30 s, then 10 min of silence at most; 10 connections a minute from one address. The console's commands
// are the shell's, through its client in SLOT_SHELL, which init lends only once the shell takes a remote session's lines
// on their own rule (requests-APP.md): without it a key's console right is not used. Each login with its key's
// fingerprint, each refusal, each console command's word, each file written or removed and each session's end is one
// line in the system log; no session's data is (MC-10.3).
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use mind::abi::{BootInfo, SLOT_NETWORK, SLOT_SHELL, SLOT_VFS};
use mind::idl::keystore::{self, Purpose};
use mind::idl::{shell, socket};
use mind::ipc::Endpoint;
use ssh::keys::{self, Listed};
use ssh::server::{self, Attrs, Event, Files, Owner, Rights, FAILURE, NO_SUCH_FILE, PERMISSION_DENIED};

const FLOW: Endpoint = Endpoint(SLOT_NETWORK);
const KEYS: Endpoint = Endpoint(4);
const SHELL: Endpoint = Endpoint(SLOT_SHELL);
const ROOT: &str = "data";
const KEYS_FILE: &str = "data/ssh/authorized_keys";
const KEYS_FILE_MAX: usize = 64 * 1024;
const USER: &[u8] = b"mind";
const PORT: u16 = 22;
const BACKLOG: u8 = 4;
const LOGIN_MS: u64 = 30_000; // from the connection to a login
const IDLE_MS: u64 = 600_000; // the longest a session waits for its client
const PER_MINUTE: u32 = 10; // connections a minute from one address
const AGAIN_MS: usize = 10_000; // between looks for the keys file, and tries to listen
const CHUNK: usize = 4096; // the most one socket call moves
const WRITE_MAX: u64 = 1 << 30; // bytes one session may write
const OPEN_MAX: usize = 8; // files one session holds open
const ANSWER_MAX: usize = 7000; // shell.wit `run`'s answer
const LINE_MAX: usize = 240; // shell.wit `run`'s line

// When the connection being served started, and whether it logged in: its deadline is LOGIN_MS from the first until
// then, IDLE_MS from each wait after.
static CONNECTED: AtomicU64 = AtomicU64::new(0);
static LOGGED_IN: AtomicBool = AtomicBool::new(false);

fn now() -> u64 { mind::time::uptime_ms() as u64 }

fn dotted(address: u32) -> String { let [a, b, c, d] = address.to_be_bytes(); format!("{a}.{b}.{c}.{d}") }

fn random(out: &mut [u8]) -> bool { mind::random::fill(out) }

// A user name or a word as the log may show it: printable ASCII, at most 64 bytes.
fn printable(bytes: &[u8]) -> String { bytes.iter().take(64).map(|&b| if (0x21..0x7F).contains(&b) { b as char } else { '?' }).collect() }

/// The TCP connection being served.
struct Link(u32);

impl Link {
    fn deadline(waited_from: u64) -> u64 {
        if LOGGED_IN.load(Ordering::Relaxed) { waited_from + IDLE_MS } else { CONNECTED.load(Ordering::Relaxed) + LOGIN_MS }
    }
}

impl ssh::Io for Link {
    fn send(&mut self, mut data: &[u8]) -> Result<(), ssh::Error> {
        let from = now();
        while !data.is_empty() {
            let taken = match socket::tcp_send(FLOW, self.0, &data[..data.len().min(CHUNK)]) { Ok(Ok(n)) => n as usize, Ok(Err(socket::Error::Again)) => 0, _ => return Err(ssh::Error::Io) };
            data = &data[taken.min(data.len())..];
            if taken == 0 {
                if now() > Self::deadline(from) { return Err(ssh::Error::Io); }
                mind::time::sleep(5);
            }
        }
        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, ssh::Error> {
        let (from, length) = (now(), buffer.len().min(CHUNK));
        let mut pause = 1;
        loop {
            match socket::tcp_receive(FLOW, self.0, length as u32, &mut buffer[..length]) {
                Ok(Ok(n)) if n > 0 => return Ok(n),
                Ok(Err(socket::Error::Closed)) => return Ok(0),
                Ok(Ok(_)) | Ok(Err(socket::Error::Again | socket::Error::Timeout)) => {
                    if now() > Self::deadline(from) { return Err(ssh::Error::Io); }
                    // A client that types: short waits; a quiet one, up to 50 ms between looks.
                    mind::time::sleep(pause);
                    pause = (pause * 2).min(50);
                }
                _ => return Err(ssh::Error::Io),
            }
        }
    }
}

impl Drop for Link { fn drop(&mut self) { let _ = socket::close(FLOW, self.0); } }

/// The device key as the host key: its public half, and keystore's signature of each exchange hash (`ssh-host`).
struct HostKey([u8; 32]);

impl ssh::Keys for HostKey {
    fn public(&self) -> [u8; 32] { self.0 }
    fn sign(&mut self, data: &[u8]) -> Option<[u8; 64]> {
        let mut signature = [0u8; 64];
        match keystore::sign(KEYS, Purpose::SshHost, data, &mut signature) { Ok(Ok(64)) => Some(signature), _ => None }
    }
}

fn file_error(error: mind::fs::Error) -> u32 {
    match error { mind::fs::Error::NotFound => NO_SUCH_FILE, mind::fs::Error::Denied | mind::fs::Error::ReadOnly => PERMISSION_DENIED, _ => FAILURE }
}

/// The owner's files a session reaches: data/ but data/ssh/, through init's scope.
struct Disk { open: Vec<(u32, mind::fs::File, bool)>, next: u32, written: u64 }

impl Disk {
    // A path below the files' root (server::normal's form) as mind::fs names it; the keys' directory is refused.
    fn local(path: &str) -> Result<String, u32> {
        if keys::under_dir(path) { return Err(PERMISSION_DENIED); }
        Ok(format!("{ROOT}{}", if path == "/" { "" } else { path }))
    }
    fn file(&mut self, handle: u32) -> Result<&mut (u32, mind::fs::File, bool), u32> { self.open.iter_mut().find(|(h, _, _)| *h == handle).ok_or(FAILURE) }
    fn attrs(m: &mind::fs::Metadata) -> Attrs { Attrs { size: m.size as u64, directory: m.is_dir, modified: 0 } }
}

impl Files for Disk {
    fn open(&mut self, path: &str, write: bool, create: bool, truncate: bool) -> Result<u32, u32> {
        let local = Self::local(path)?;
        if self.open.len() >= OPEN_MAX { return Err(FAILURE); }
        let mode = if write { mind::fs::MODE_WRITE | if create { mind::fs::MODE_CREATE } else { 0 } | if truncate { mind::fs::MODE_TRUNCATE } else { 0 } } else { 0 };
        let file = mind::fs::File::open_mode(&local, mode).map_err(file_error)?;
        self.next = self.next.wrapping_add(1) & 0x7FFF_FFFF;
        self.open.push((self.next, file, false));
        Ok(self.next)
    }
    fn read(&mut self, handle: u32, offset: u64, out: &mut [u8]) -> Result<usize, u32> {
        let (_, file, _) = self.file(handle)?;
        let Ok(offset) = usize::try_from(offset) else { return Ok(0) };
        if offset >= file.size() { return Ok(0); }
        file.read_at(offset, out).map_err(file_error)
    }
    fn write(&mut self, handle: u32, offset: u64, data: &[u8]) -> Result<(), u32> {
        if self.written + data.len() as u64 > WRITE_MAX { return Err(FAILURE); }
        self.written += data.len() as u64;
        let (_, file, written) = self.file(handle)?;
        let offset = usize::try_from(offset).ok().filter(|&o| o <= u32::MAX as usize).ok_or(FAILURE)?;
        *written = true;
        match file.write_at(offset, data) { Ok(n) if n == data.len() => Ok(()), Ok(_) => Err(FAILURE), Err(e) => Err(file_error(e)) }
    }
    fn close(&mut self, handle: u32) -> Result<(), u32> {
        let at = self.open.iter().position(|(h, _, _)| *h == handle).ok_or(FAILURE)?;
        let (_, file, written) = self.open.swap_remove(at);
        // A file written is flushed as it closes, so that what scp copied is on the medium when it says so.
        if written { file.flush().map_err(file_error)?; }
        Ok(())
    }
    fn stat(&mut self, path: &str) -> Result<Attrs, u32> { mind::fs::metadata(&Self::local(path)?).map(|m| Self::attrs(&m)).map_err(file_error) }
    fn fstat(&mut self, handle: u32) -> Result<Attrs, u32> { self.file(handle)?.1.metadata().map(|m| Self::attrs(&m)).map_err(file_error) }
    fn list(&mut self, path: &str) -> Result<Vec<(String, Attrs)>, u32> {
        let local = Self::local(path)?;
        let mut out = Vec::new();
        mind::fs::list(&local, |e| {
            let name = e.name_str();
            // The keys' directory is not shown at the root, as it is not reached.
            if name == "." || name == ".." || (path == "/" && keys::under_dir(name)) || out.len() >= 4096 { return; }
            out.push((String::from(name), Attrs { size: e.size as u64, directory: e.is_dir, modified: 0 }));
        }).map_err(file_error)?;
        Ok(out)
    }
    fn remove(&mut self, path: &str) -> Result<(), u32> { mind::fs::remove(&Self::local(path)?).map_err(file_error) }
    fn mkdir(&mut self, path: &str) -> Result<(), u32> { mind::fs::mkdir(&Self::local(path)?).map_err(file_error) }
    fn rmdir(&mut self, path: &str) -> Result<(), u32> { mind::fs::remove(&Self::local(path)?).map_err(file_error) }
    fn rename(&mut self, from: &str, to: &str) -> Result<(), u32> {
        if to == "/" { return Err(PERMISSION_DENIED); }
        mind::fs::rename(&Self::local(from)?, &Self::local(to)?).map_err(file_error)
    }
}

/// One connection's side of the owner: the keys read for it, its log lines, its files.
struct Session { keys: Vec<Listed>, peer: String, console: bool, files: Disk }

fn named(rights: Rights) -> &'static str {
    match (rights.console, rights.files) { (true, true) => "THE CONSOLE AND THE FILES", (true, false) => "THE CONSOLE", (false, true) => "THE FILES", _ => "NOTHING" }
}

impl Owner for Session {
    fn rights(&mut self, user: &[u8], key: &[u8; 32]) -> Option<Rights> {
        if user != USER { return None; }
        let listed = self.keys.iter().find(|k| &k.key == key)?;
        // Without the shell's client a console right gives nothing yet.
        Some(Rights { console: listed.rights.console && self.console, files: listed.rights.files })
    }
    fn command(&mut self, line: &str, out: &mut dyn FnMut(&[u8])) -> bool {
        if matches!(line, "exit" | "logout") { return false; }
        if line.len() > LINE_MAX { out(b"refused: a line longer than 240 bytes\n"); return true; }
        let mut answer = alloc::vec![0u8; ANSWER_MAX];
        match shell::run(SHELL, line, &mut answer) {
            Ok(Ok(n)) => out(&answer[..n.min(ANSWER_MAX)]),
            Ok(Err(e)) => out(format!("the shell did not run it: {:?}\n", e).as_bytes()),
            Err(_) => out(b"the shell does not answer\n"),
        }
        true
    }
    fn prompt(&self) -> &str { "mind$ " }
    fn files(&mut self) -> &mut dyn Files { &mut self.files }
    fn event(&mut self, event: Event<'_>) {
        let peer = &self.peer;
        match event {
            Event::Login { user, key } => {
                LOGGED_IN.store(true, Ordering::Relaxed);
                let listed = self.keys.iter().find(|k| mind::sha256::digest(&ssh::login::key_blob(&k.key)) == key);
                let rights = listed.map_or(Rights { console: false, files: false }, |k| k.rights);
                let unused = if rights.console && !self.console { ", ITS CONSOLE RIGHT UNUSED: THE SHELL LENDS NO REMOTE CONSOLE YET" } else { "" };
                mind::println!("[SSHD] LOGIN AS {} FROM {} WITH {} ({}): {}{}", printable(user), peer, keys::fingerprint(&key),
                               listed.map_or("", |k| k.comment.as_str()), named(rights), unused);
            }
            Event::Refused { user, why } => mind::println!("[SSHD] A LOGIN AS {} FROM {} REFUSED: {}", printable(user), peer, why),
            Event::Command { line } => mind::println!("[SSHD] CONSOLE FROM {}: {}", peer, printable(line.split_whitespace().next().unwrap_or("").as_bytes()).to_ascii_uppercase()),
            Event::Wrote { path } => mind::println!("[SSHD] {} WROTE {}{}", peer, ROOT, path),
            Event::Removed { path } => mind::println!("[SSHD] {} REMOVED {}{}", peer, ROOT, path),
            Event::End { why } => mind::println!("[SSHD] SESSION FROM {} ENDED ({}): {} BYTES WRITTEN", peer, why, self.files.written),
        }
    }
}

/// The owner's keys, as the file holds them now; its lines left out are logged when the file changes.
struct Keyring { seen: Option<[u8; 32]> }

impl Keyring {
    fn read(&mut self) -> Vec<Listed> {
        let mut buffer = alloc::vec![0u8; KEYS_FILE_MAX];
        let text = match mind::fs::File::open(KEYS_FILE).and_then(|mut f| f.read(&mut buffer)) {
            Ok(n) => core::str::from_utf8(&buffer[..n]).unwrap_or(""),
            Err(_) => "",
        };
        let (listed, left) = keys::parse(text);
        let digest = mind::sha256::digest(text.as_bytes());
        if self.seen != Some(digest) {
            self.seen = Some(digest);
            mind::println!("[SSHD] {}: {} KEYS LISTED", KEYS_FILE, listed.len());
            for (line, why) in left { mind::println!("[SSHD] {} LINE {} LEFT OUT: {}", KEYS_FILE, line, why); }
        }
        listed
    }
}

// Connections a minute by address: the minute's start and its count.
struct Rate(Vec<(u32, u64, u32)>);

impl Rate {
    fn allow(&mut self, address: u32) -> bool {
        let t = now();
        self.0.retain(|&(_, from, _)| t - from < 60_000);
        match self.0.iter_mut().find(|(a, _, _)| *a == address) {
            Some(entry) => { entry.2 += 1; entry.2 <= PER_MINUTE }
            None => { if self.0.len() >= 64 { self.0.remove(0); } self.0.push((address, t, 1)); true }
        }
    }
}

fn public_key() -> [u8; 32] {
    let mut said = false;
    loop {
        if let Ok(Ok(key)) = keystore::public_key(KEYS) { if let Ok(key) = <[u8; 32]>::try_from(key.as_slice()) { return key; } }
        // No key yet: keystore waits for the owner's passphrase, or has no random source.
        if !core::mem::replace(&mut said, true) { mind::println!("[SSHD] NO DEVICE KEY FROM KEYSTORE YET: NOTHING LISTENS; ASKED AGAIN EVERY {} S", AGAIN_MS / 1000); }
        mind::time::sleep(AGAIN_MS);
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("sshd — the SSH server: logins with the keys data/ssh/authorized_keys lists, file copy over SFTP into data/, the host key in keystore.");
    if mind::dev::cap_info(SLOT_VFS).0 != mind::abi::CAP_KIND_ENDPOINT || !mind::random::available() {
        mind::println!("[SSHD] NO SCOPE OF {}/ OR NO {}: NOTHING LISTENS", ROOT, mind::random::SOURCE);
        loop { mind::time::sleep(usize::MAX / 2); }
    }
    mind::fs::use_scope(Endpoint(SLOT_VFS), ROOT);
    let mut host = HostKey(public_key());
    let console = mind::dev::cap_info(SLOT_SHELL).0 == mind::abi::CAP_KIND_ENDPOINT;
    mind::println!("[SSHD] HOST KEY: THE DEVICE KEY, {}", keys::fingerprint(&mind::sha256::digest(&ssh::login::key_blob(&host.0))));
    let mut keyring = Keyring { seen: None };
    let mut said = false;
    while keyring.read().is_empty() {
        if !core::mem::replace(&mut said, true) { mind::println!("[SSHD] NO KEY IN {}: NOTHING LISTENS ON TCP {}; LOOKED FOR AGAIN EVERY {} S", KEYS_FILE, PORT, AGAIN_MS / 1000); }
        mind::time::sleep(AGAIN_MS);
    }
    let listener = loop {
        match socket::tcp_listen(FLOW, PORT, BACKLOG) {
            Ok(Ok(listener)) => break listener,
            // Denied: the policy's listening grant does not name the port; tried again, as the owner may change the policy.
            other => mind::println!("[SSHD] TCP {} NOT LISTENED ON: {:?}; AGAIN IN {} S", PORT, other, AGAIN_MS / 1000),
        }
        mind::time::sleep(AGAIN_MS);
    };
    mind::println!("[SSHD] LISTENING ON TCP {}: ONE SESSION AT A TIME, A LOGIN WITHIN {} S, THE CONSOLE {}", PORT, LOGIN_MS / 1000,
                   if console { "THROUGH THE SHELL" } else { "NOT LENT (THE FILES ALONE)" });
    let mut rate = Rate(Vec::new());
    loop {
        let accepted = match socket::tcp_accept(FLOW, listener) {
            Ok(Ok(accepted)) => accepted,
            Ok(Err(socket::Error::Again)) => { mind::time::sleep(50); continue; }
            other => { mind::println!("[SSHD] ACCEPT FAILED: {:?}", other.map(|r| r.map(|a| a.socket))); mind::time::sleep(1000); continue; }
        };
        let peer = format!("{}:{}", dotted(accepted.address), accepted.port);
        let link = Link(accepted.socket);
        if !rate.allow(accepted.address) {
            mind::println!("[SSHD] {} REFUSED: MORE THAN {} CONNECTIONS A MINUTE FROM ITS ADDRESS", peer, PER_MINUTE);
            continue;
        }
        CONNECTED.store(now(), Ordering::Relaxed);
        LOGGED_IN.store(false, Ordering::Relaxed);
        let mut session = Session { keys: keyring.read(), peer, console, files: Disk { open: Vec::new(), next: 0, written: 0 } };
        if let Err(error) = server::serve(link, &mut host, &mut session, random) {
            mind::println!("[SSHD] SESSION FROM {}: {:?}", session.peer, error);
        }
    }
}
