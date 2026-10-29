// Programs' secrets (idl/keystore.wit 1.4, 000-NET-0045): a token, a password or an API key that one program alone
// reads. The loader asks for a client for each program it runs by its name and lends it in SLOT_SECRETS (000-KRN-0182);
// a client is answered only for its program, and only for the task the loader named or the first that called on it.
// Each program's secrets are one file, `system/keystore/secrets/<program>.kept`: MAGIC, a random nonce, then the plain
// record (secret_file.rs) sealed with ChaCha20-Poly1305 under a key derived from the device key and the program's name,
// which the authenticated data names too, so the file opens only under that device key and as that program's. Whoever
// has the disk without the device key reads nothing; where no TPM seals the device key, its file is on the same disk
// (docs/profile/threat-model.md). A boot service's secrets (1.5, 308-KRN-0183) are the same, by the badge init lent it,
// in `system/keystore/services/` and under a label that names a service: no program's name reaches them.
use crate::secret_file::{self, Kind, Refused, Secrets, wipe};
use alloc::vec::Vec;
use chacha20poly1305::aead::AeadInPlace;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce, Tag};
use mind::abi::{CAP_GRANT, CAP_WRITE, SLOT_SERVICE};
use mind::fs::{self, File};
use mind::idl::codec::{List, Text};
use mind::idl::keystore::Error;
use mind::ipc;
use mind::network::{BADGE_KEY_BLOCKSTORE, BADGE_KEY_LOADER};
use mind::util::FixedBuf;

const MAGIC: &[u8; 8] = b"MINDSEC1";
const NONCE: usize = 12;
const TAG: usize = 16;
const FILE_MAX: usize = MAGIC.len() + NONCE + secret_file::PLAIN_MAX + TAG;
const CLIENTS: usize = 64; // the programs' clients alive at once
const BADGE_FIRST: u16 = 0x100; // below: the named services' badges (mind::network::BADGE_KEY_*)
const UNUSED_MS: u64 = 60_000; // a client no task took up ends after a minute
// The boot services whose client init badges for their own secrets (1.5): the badge and the service's name.
const SERVICES: [(u16, &str); 1] = [(BADGE_KEY_BLOCKSTORE, "blockstore")];

type Program = Text<{ secret_file::PROGRAM_MAX }>;

/// Whose secrets a call is about: its kind and name.
#[derive(Clone)]
pub struct Owner { kind: Kind, name: Program }

impl core::fmt::Display for Owner {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self.kind { Kind::Program => f.write_str(self.name.as_str()), Kind::Service => write!(f, "SERVICE {}", self.name.as_str()) }
    }
}

struct Client { badge: u16, cap: usize, program: Option<Program>, user: Option<u64>, made_ms: u64, maker: u64 }

pub struct Clients { clients: Vec<Option<Client>>, next_badge: u16 }

impl Clients {
    pub fn new() -> Self { Self { clients: (0..CLIENTS).map(|_| None).collect(), next_badge: BADGE_FIRST } }

    fn index(&self, badge: u16) -> Option<usize> {
        if badge < BADGE_FIRST { return None; }
        self.clients.iter().position(|c| c.as_ref().is_some_and(|c| c.badge == badge))
    }

    fn end(&mut self, index: usize) {
        if let Some(client) = self.clients[index].take() {
            let _ = ipc::revoke(client.cap);
            let _ = ipc::drop_cap(client.cap);
        }
    }

    // Clients whose task has ended, or that no task took up, are revoked.
    fn sweep(&mut self) {
        let now = mind::time::uptime_ms() as u64;
        for index in 0..self.clients.len() {
            let over = self.clients[index].as_ref().is_some_and(|c| match c.user { Some(pid) => !mind::process::alive(pid), None => now.saturating_sub(c.made_ms) > UNUSED_MS });
            if over { self.end(index); }
        }
    }

    fn loader(badge: u16, pid: u64, what: &str) -> Result<(), Error> {
        if badge != BADGE_KEY_LOADER { mind::println!("[KEYSTORE] REFUSED {} FOR PID {}: NOT THE LOADER", what, pid); return Err(Error::Denied); }
        Ok(())
    }

    /// A new client for the loader: the handle to copy into the reply.
    pub fn client(&mut self, badge: u16, pid: u64) -> Result<usize, Error> {
        Self::loader(badge, pid, "A SECRETS CLIENT")?;
        self.sweep();
        let index = self.clients.iter().position(Option::is_none).ok_or_else(|| { mind::println!("[KEYSTORE] NO SECRETS CLIENT: {} LIVE", CLIENTS); Error::Budget })?;
        let badge = loop {
            let next = self.next_badge;
            self.next_badge = if next == u16::MAX { BADGE_FIRST } else { next + 1 };
            if self.index(next).is_none() { break next; }
        };
        let cap = ipc::mint_badged(SLOT_SERVICE, CAP_WRITE | CAP_GRANT, badge).map_err(|e| { mind::println!("[KEYSTORE] NO SECRETS CLIENT: {:?}", e); Error::Budget })?;
        self.clients[index] = Some(Client { badge, cap, program: None, user: None, made_ms: mind::time::uptime_ms() as u64, maker: pid });
        Ok(cap)
    }

    /// The loader names the program a client of its own reaches, once.
    pub fn name(&mut self, badge: u16, pid: u64, client: u16, program: &str) -> Result<(), Error> {
        Self::loader(badge, pid, "A SECRETS CLIENT'S NAME")?;
        if !secret_file::program_ok(program.as_bytes()) { return Err(Error::Invalid); }
        let index = self.index(client).ok_or(Error::Invalid)?;
        let entry = self.clients[index].as_mut().ok_or(Error::Invalid)?;
        if entry.maker != pid || entry.program.is_some() { return Err(Error::Denied); }
        let name = Program::new(program).ok_or(Error::Invalid)?;
        mind::println!("[KEYSTORE] SECRETS CLIENT {:#x} FOR {}", client, program);
        entry.program = Some(name);
        Ok(())
    }

    /// The loader names the task it started with a client of its own.
    pub fn adopt(&mut self, badge: u16, pid: u64, client: u16, task: u64) -> Result<(), Error> {
        Self::loader(badge, pid, "A SECRETS CLIENT'S TASK")?;
        let index = self.index(client).ok_or(Error::Invalid)?;
        match self.clients[index].as_mut() {
            Some(c) if c.maker == pid && c.user.is_none_or(|user| user == task) => { c.user = Some(task); Ok(()) }
            _ => Err(Error::Denied),
        }
    }

    /// Whose secrets a call is about: a boot service's by its badge, else the program of the secrets client; the first
    /// task to call on a client not yet adopted takes it.
    pub fn owner(&mut self, badge: u16, pid: u64) -> Result<Owner, Error> {
        if let Some(&(_, service)) = SERVICES.iter().find(|&&(b, _)| b == badge) {
            return Program::new(service).map(|name| Owner { kind: Kind::Service, name }).ok_or(Error::Invalid);
        }
        let index = self.index(badge).ok_or_else(|| { mind::println!("[KEYSTORE] REFUSED SECRETS FOR PID {}: NOT A SECRETS CLIENT", pid); Error::Denied })?;
        let client = self.clients[index].as_mut().ok_or(Error::Denied)?;
        let Some(program) = client.program.clone() else { mind::println!("[KEYSTORE] REFUSED SECRETS FOR PID {}: THE CLIENT NAMES NO PROGRAM", pid); return Err(Error::Denied) };
        match client.user {
            None => client.user = Some(pid),
            Some(user) if user != pid => { mind::println!("[KEYSTORE] REFUSED SECRETS OF {} FOR PID {}: ANOTHER TASK'S CLIENT", program.as_str(), pid); return Err(Error::Denied); }
            _ => {}
        }
        Ok(Owner { kind: Kind::Program, name: program })
    }
}

fn path(owner: &Owner, suffix: &str) -> FixedBuf<64> {
    let mut path = FixedBuf::new();
    let _ = core::fmt::Write::write_fmt(&mut path, format_args!("{}/{}{}", owner.kind.dir(), owner.name.as_str(), suffix));
    path
}

// The key of `owner`'s file under the device key whose seed is `seed`.
fn key(seed: &[u8; 32], owner: &Owner) -> [u8; 32] {
    let mut material = owner.kind.key_material(seed, owner.name.as_str());
    let key = mind::sha256::digest(&material);
    wipe(&mut material);
    key
}

fn aad(owner: &Owner) -> Vec<u8> { [MAGIC.as_slice(), owner.name.as_str().as_bytes()].concat() }

/// `owner`'s secrets: none when it keeps no file; `no-key` when its file does not open (another device key, or damaged).
pub fn load(seed: &[u8; 32], owner: &Owner) -> Result<Secrets, Error> {
    // The file a write left before it replaced the kept one (`.new`) holds whole when the kept one is gone (`store`).
    let mut data = alloc::vec![0u8; FILE_MAX + 1];
    let read = match File::open(path(owner, ".kept").as_str()) {
        Err(fs::Error::NotFound) => File::open(path(owner, ".new").as_str()),
        other => other,
    }.and_then(|file| file.read_at(0, &mut data));
    let n = match read {
        Ok(n) => n,
        Err(fs::Error::NotFound) => return Ok(Secrets::default()),
        Err(error) => { mind::println!("[KEYSTORE] SECRETS OF {} UNREADABLE: {:?}", owner, error); return Err(Error::NoKey); }
    };
    let opened = (|| {
        if n < MAGIC.len() + NONCE + TAG || n > FILE_MAX || &data[..MAGIC.len()] != MAGIC { return Err("DAMAGED"); }
        let (nonce, rest) = data[MAGIC.len()..n].split_at_mut(NONCE);
        let (sealed, tag) = rest.split_at_mut(rest.len() - TAG);
        let mut key = key(seed, owner);
        let cipher = ChaCha20Poly1305::new(&key.into());
        wipe(&mut key);
        cipher.decrypt_in_place_detached(Nonce::from_slice(nonce), &aad(owner), sealed, Tag::from_slice(tag)).map_err(|_| "DO NOT OPEN UNDER THIS DEVICE KEY AND NAME")?;
        Secrets::decode(sealed).map_err(|_| "DAMAGED")
    })();
    wipe(&mut data);
    opened.map_err(|why| { mind::println!("[KEYSTORE] SECRETS OF {} {}: NOT READ", owner, why); Error::NoKey })
}

/// Writes `owner`'s secrets whole: the new file first, then in place of the kept one; none kept, no file.
pub fn store(seed: &[u8; 32], owner: &Owner, secrets: &Secrets) -> Result<(), Error> {
    let (kept, new) = (path(owner, ".kept"), path(owner, ".new"));
    let result = (|| -> Result<(), &'static str> {
        fs::mkdir(owner.kind.dir()).map_err(|_| "NO DIRECTORY")?;
        if secrets.is_empty() {
            let _ = fs::remove(new.as_str());
            return match fs::remove(kept.as_str()) { Ok(()) | Err(fs::Error::NotFound) => Ok(()), Err(_) => Err("NOT REMOVED") };
        }
        let mut nonce = [0u8; NONCE];
        if !mind::random::fill(&mut nonce) { return Err(mind::random::SOURCE); }
        let mut data = Vec::with_capacity(FILE_MAX);
        data.extend_from_slice(MAGIC);
        data.extend_from_slice(&nonce);
        let mut plain = secrets.encode();
        let mut key = key(seed, owner);
        let cipher = ChaCha20Poly1305::new(&key.into());
        wipe(&mut key);
        let tag = cipher.encrypt_in_place_detached(Nonce::from_slice(&nonce), &aad(owner), &mut plain).map_err(|_| "NOT SEALED")?;
        data.extend_from_slice(&plain);
        data.extend_from_slice(&tag);
        let written = File::create(new.as_str()).and_then(|mut file| { file.write_at(0, &data)?; file.flush() }).map_err(|_| "NOT WRITTEN");
        written?;
        match fs::remove(kept.as_str()) { Ok(()) | Err(fs::Error::NotFound) => {} Err(_) => return Err("THE OLD FILE NOT REMOVED") }
        fs::rename(new.as_str(), kept.as_str()).map_err(|_| "NOT RENAMED")
    })();
    result.map_err(|why| { mind::println!("[KEYSTORE] SECRETS OF {} NOT WRITTEN: {}", owner, why); Error::NoKey })
}

fn refused(error: Refused) -> Error { match error { Refused::Name | Refused::Value | Refused::Damaged => Error::Invalid, Refused::Full => Error::Budget } }

pub fn put(seed: &[u8; 32], owner: &Owner, pid: u64, name: &str, value: &[u8]) -> Result<(), Error> {
    let mut secrets = load(seed, owner)?;
    secrets.put(name.as_bytes(), value).map_err(|error| {
        mind::println!("[KEYSTORE] REFUSED SECRET {} OF {} FOR PID {}: {:?}", if secret_file::name_ok(name.as_bytes()) { name } else { "(A NAME REFUSED)" }, owner, pid, error);
        refused(error)
    })?;
    store(seed, owner, &secrets)?;
    mind::println!("[KEYSTORE] SECRET {} OF {} KEPT FOR PID {} ({} KEPT)", name, owner, pid, secrets.names().count());
    Ok(())
}

/// The value of `name`, which the caller wipes once it is sent.
pub fn get(seed: &[u8; 32], owner: &Owner, pid: u64, name: &str) -> Result<Vec<u8>, Error> {
    let secrets = load(seed, owner)?;
    let Some(value) = secrets.get(name.as_bytes()) else {
        if secret_file::name_ok(name.as_bytes()) { mind::println!("[KEYSTORE] SECRET {} OF {}: NONE KEPT (PID {})", name, owner, pid); }
        return Err(Error::Missing);
    };
    mind::println!("[KEYSTORE] SECRET {} OF {} READ BY PID {}", name, owner, pid);
    Ok(value.to_vec())
}

pub fn delete(seed: &[u8; 32], owner: &Owner, pid: u64, name: &str) -> Result<bool, Error> {
    let mut secrets = load(seed, owner)?;
    if !secrets.delete(name.as_bytes()) { return Ok(false); }
    store(seed, owner, &secrets)?;
    mind::println!("[KEYSTORE] SECRET {} OF {} FORGOTTEN FOR PID {}", name, owner, pid);
    Ok(true)
}

pub fn list(seed: &[u8; 32], owner: &Owner, out: &mut List<Text<32>, 16>) -> Result<(), Error> {
    for name in load(seed, owner)?.names() {
        out.push(Text::new(core::str::from_utf8(name).unwrap_or("")).unwrap_or_default());
    }
    Ok(())
}
