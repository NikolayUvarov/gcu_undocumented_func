#![no_std]
#![no_main]
// tally: named counters kept across instances by checkpoints in the block store (306-STO-0009, the pilot of
// docs/storage/checkpoints.md, contract "tally" 1). Each run is a new instance: it restores the last checkpoint,
// applies one request and saves the next, fenced if another instance saved first (MC-6.12). Effects (writing a file)
// are journaled before they begin and come back pending if the instance ends first (Appendix B.4). A console program:
// it asks the shell for the store's client and the user's files, and holds nothing a checkpoint could hand back.
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_BLOCKSTORE, SLOT_FILE};
use mind::checkpoint::{self, rebind, restore, save, Error, Label, Manifest, Names, Outcome, Restored, MAX};
use mind::cid::Cid;
use mind::fs::File;
use mind::idl::blockstore::{self, Update};
use mind::idl::codec::{List, Text};
use mind::ipc::Endpoint;

mind::request!(REQUEST_CONSOLE | REQUEST_FILES | REQUEST_BLOCKSTORE);

const STORE: Endpoint = Endpoint(SLOT_BLOCKSTORE);
const CONTRACT: &str = "tally";
const VERSION: u16 = 1;
/// The state's schema: lines `key=count`, sorted by key.
const SCHEMA: u16 = 1;
const KEYS: usize = 32;
const KEY: usize = 16;

static mut BUFFER: [u8; MAX] = [0; MAX];
static mut STATE: [u8; KEYS * (KEY + 22)] = [0; KEYS * (KEY + 22)];

/// The store's client as checkpoints need it.
struct Remote;
impl checkpoint::Store for Remote {
    fn put(&mut self, data: &[u8]) -> Result<Cid, Error> {
        let mut out = [0u8; 36];
        match blockstore::put(STORE, blockstore::Codec::Raw, data, &mut out) { Ok(Ok(n)) => Cid::from_bytes(&out[..n]).map_err(|_| Error::Store), _ => Err(Error::Store) }
    }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> {
        match blockstore::get(STORE, &cid.to_bytes(), out) { Ok(Ok(n)) => Ok(n), _ => Err(Error::Store) }
    }
    fn snapshot(&mut self, names: [&str; 2]) -> Result<[(u64, Option<Cid>); 2], Error> {
        let texts = names.map(|n| Text::new(n).unwrap_or_default());
        let Ok(Ok(heads)) = blockstore::snapshot(STORE, &texts) else { return Err(Error::Store) };
        let mut out = [(0, None); 2];
        for (o, h) in out.iter_mut().zip(heads.as_slice()) { *o = (h.version, Cid::from_bytes(h.root.as_slice()).ok()); }
        Ok(out)
    }
    fn commit(&mut self, names: [&str; 2], expected: u64, roots: [Cid; 2]) -> Result<u64, Error> {
        let updates = [0, 1].map(|k| Update { name: Text::new(names[k]).unwrap_or_default(), expected, root: List::from_slice(&roots[k].to_bytes()).unwrap_or_default() });
        match blockstore::commit(STORE, &updates) {
            Ok(Ok(v)) => v.as_slice().first().copied().ok_or(Error::Store),
            Ok(Err(blockstore::Error::Conflict)) => Err(Error::Fenced),
            _ => Err(Error::Store),
        }
    }
}

/// The counters: the state of contract "tally" 1.
struct Counters { keys: [Label; KEYS], counts: [u64; KEYS], len: usize }
impl Counters {
    fn parse(text: &[u8]) -> Option<Counters> {
        let mut c = Counters { keys: [Label::EMPTY; KEYS], counts: [0; KEYS], len: 0 };
        for line in core::str::from_utf8(text).ok()?.lines() {
            let (key, count) = line.split_once('=')?;
            if c.len == KEYS || key.len() > KEY { return None; }
            (c.keys[c.len], c.counts[c.len]) = (Label::new(key)?, count.parse().ok()?);
            c.len += 1;
        }
        Some(c)
    }
    fn add(&mut self, key: &str, n: u64) -> Option<u64> {
        let label = Label::new(key).filter(|_| key.len() <= KEY && !key.contains('='))?;
        let at = match self.keys[..self.len].iter().position(|k| k.as_str() >= key) {
            Some(i) if self.keys[i] == label => i,
            found => {
                if self.len == KEYS { return None; }
                let i = found.unwrap_or(self.len);
                self.keys.copy_within(i..self.len, i + 1);
                self.counts.copy_within(i..self.len, i + 1);
                (self.keys[i], self.counts[i]) = (label, 0);
                self.len += 1;
                i
            }
        };
        self.counts[at] = self.counts[at].checked_add(n)?;
        Some(self.counts[at])
    }
    fn encode(&self, out: &mut [u8]) -> usize {
        let mut at = 0;
        for k in 0..self.len {
            let mut line = [0u8; KEY + 22];
            let n = format_line(&mut line, self.keys[k].as_str(), self.counts[k]);
            out[at..at + n].copy_from_slice(&line[..n]);
            at += n;
        }
        at
    }
}

fn format_line(out: &mut [u8], key: &str, count: u64) -> usize {
    let mut digits = [0u8; 20];
    let (mut n, mut v) = (0, count);
    loop { digits[n] = b'0' + (v % 10) as u8; n += 1; v /= 10; if v == 0 { break; } }
    out[..key.len()].copy_from_slice(key.as_bytes());
    out[key.len()] = b'=';
    for k in 0..n { out[key.len() + 1 + k] = digits[n - 1 - k]; }
    out[key.len() + 1 + n] = b'\n';
    key.len() + n + 2
}

// The authorities this contract uses, as the checkpoint records them; held now if the capability is there.
fn holds(authority: &str) -> bool {
    let slot = match authority { "blockstore" => SLOT_BLOCKSTORE, "files" => SLOT_FILE, _ => return false };
    mind::dev::cap_info(slot).0 == CAP_KIND_ENDPOINT
}

/// The instance: what it restored, and the counters at that point.
struct Instance { from: u64, manifest: Manifest, counters: Counters }

fn open(names: &Names, buffer: &mut [u8]) -> Option<Instance> {
    let restored = match restore(&mut Remote, names, CONTRACT, VERSION, SCHEMA, buffer) {
        Ok(r) => r,
        Err(e) => { mind::println!("tally: restore: {:?}", e); return None }
    };
    let (manifest, counters) = match restored {
        Restored::Fresh { .. } => {
            let mut m = Manifest::new(Label::new(CONTRACT)?, VERSION, SCHEMA, Cid::raw(b""));
            m.add_resource("blockstore").ok()?;
            m.add_authority("blockstore").ok()?;
            m.add_authority("files").ok()?;
            (m, Counters { keys: [Label::EMPTY; KEYS], counts: [0; KEYS], len: 0 })
        }
        Restored::Found(m) => {
            // The state's block is checked against its CID by the store; then against the schema here.
            let n = match checkpoint::Store::get(&mut Remote, &m.state, buffer) { Ok(n) => n, Err(e) => { mind::println!("tally: state: {:?}", e); return None } };
            let Some(c) = Counters::parse(&buffer[..n]) else { mind::println!("tally: state: not of schema {}", SCHEMA); return None };
            (m, c)
        }
    };
    Some(Instance { from: restored.epoch(), manifest, counters })
}

// Stores the counters and saves the checkpoint after the one this instance restored.
fn commit(names: &Names, i: &mut Instance, buffer: &mut [u8], state: &mut [u8]) -> bool {
    let n = i.counters.encode(state);
    match checkpoint::Store::put(&mut Remote, &state[..n]) {
        Ok(root) => i.manifest.state = root,
        Err(e) => { mind::println!("tally: state: {:?}", e); return false }
    }
    i.manifest.sequence += 1;
    match save(&mut Remote, names, &mut i.manifest, i.from, buffer) {
        Ok(epoch) => { i.from = epoch; true }
        Err(Error::Fenced) => { mind::println!("tally: FENCED: epoch {} is no longer current; nothing saved", i.from); false }
        Err(e) => { mind::println!("tally: save: {:?}", e); false }
    }
}

fn show(i: &Instance) {
    mind::println!("TALLY EPOCH {} SEQUENCE {}", i.manifest.epoch, i.manifest.sequence);
    for k in 0..i.counters.len { mind::println!("{} = {}", i.counters.keys[k], i.counters.counts[k]); }
    for e in i.manifest.pending() { mind::println!("EFFECT {} {} PENDING: RECONCILE BEFORE TRYING AGAIN", e.id, e.what); }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("tally — named counters kept by checkpoints in the block store (contract tally 1).\nUsage: tally [show] | add <key> [n] | hold <seconds> <key> | effect <file> [crash] | reconcile <id> done|failed");
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT { mind::fs::use_endpoint(Endpoint(SLOT_FILE)); }
    if !holds("blockstore") { mind::println!("tally: no client of the block store: nothing to restore from"); return; }
    let (buffer, state) = unsafe { (&mut *core::ptr::addr_of_mut!(BUFFER), &mut *core::ptr::addr_of_mut!(STATE)) };
    let names = Names::of(CONTRACT).unwrap();
    let Some(mut i) = open(&names, buffer) else { return };
    // The rights the checkpoint names come from this instance's grants, never from the checkpoint (MC-6.11).
    let mut files = true;
    for missing in rebind(&i.manifest, holds) {
        mind::println!("tally: AUTHORITY {} NOT HELD NOW", missing);
        files &= missing.as_str() != "files";
    }
    let mut words = mind::process::args_str().split_whitespace();
    match (words.next(), words.next(), words.next(), words.next()) {
        (None | Some("show"), None, ..) => show(&i),
        (Some("add"), Some(key), n, None) => {
            let Ok(n) = n.map_or(Ok(1), |n| n.parse::<u64>()) else { mind::println!("tally: not a count"); return };
            let Some(total) = i.counters.add(key, n) else { mind::println!("tally: add: a key of at most {} printable bytes, at most {} keys", KEY, KEYS); return };
            if commit(&names, &mut i, buffer, state) { mind::println!("SAVED EPOCH {} SEQUENCE {}: {} = {}", i.from, i.manifest.sequence, key, total); }
        }
        (Some("hold"), Some(seconds), Some(key), None) => {
            // An instance that restored, then stalls: if another saved meanwhile, its save is fenced.
            let Ok(seconds) = seconds.parse::<usize>() else { mind::println!("tally: not a number of seconds"); return };
            mind::println!("HOLDING EPOCH {}", i.from);
            mind::time::sleep(seconds * 1000);
            let Some(total) = i.counters.add(key, 1) else { return };
            if commit(&names, &mut i, buffer, state) { mind::println!("SAVED EPOCH {} SEQUENCE {}: {} = {}", i.from, i.manifest.sequence, key, total); }
        }
        (Some("effect"), Some(file), crash, None) => {
            if !files { mind::println!("tally: effect refused: this instance does not hold the files authority"); return; }
            if let Some(e) = i.manifest.pending().next() { mind::println!("tally: effect refused: effect {} is pending; reconcile it first", e.id); return; }
            let mut what = [0u8; 32];
            let (prefix, path) = (b"write:ram:", file.as_bytes());
            if prefix.len() + path.len() > what.len() { mind::println!("tally: a shorter file name"); return; }
            what[..prefix.len()].copy_from_slice(prefix);
            what[prefix.len()..prefix.len() + path.len()].copy_from_slice(path);
            let what = core::str::from_utf8(&what[..prefix.len() + path.len()]).unwrap_or("");
            // The intent is durable before the effect begins.
            let Ok(id) = i.manifest.begin(what) else { mind::println!("tally: the effect journal is full of pending effects"); return };
            if !commit(&names, &mut i, buffer, state) { return; }
            let written = File::create(&what[6..]).and_then(|mut f| { f.write(b"done by tally\n")?; f.flush() }).is_ok();
            if crash == Some("crash") { mind::println!("EFFECT {} BEGUN; ENDING BEFORE ITS OUTCOME IS RECORDED", id); return; }
            i.manifest.settle(id, if written { Outcome::Done } else { Outcome::Failed });
            if commit(&names, &mut i, buffer, state) { mind::println!("EFFECT {} {} SAVED EPOCH {}", id, if written { "DONE" } else { "FAILED" }, i.from); }
        }
        (Some("reconcile"), Some(id), Some(outcome), None) => {
            let outcome = match outcome { "done" => Outcome::Done, "failed" => Outcome::Failed, _ => { mind::println!("tally: reconcile <id> done|failed"); return } };
            let Ok(id) = id.parse::<u64>() else { mind::println!("tally: not an effect id"); return };
            if !i.manifest.pending().any(|e| e.id == id) { mind::println!("tally: effect {} is not pending", id); return; }
            i.manifest.settle(id, outcome);
            if commit(&names, &mut i, buffer, state) { mind::println!("RECONCILED {} SAVED EPOCH {}", id, i.from); }
        }
        _ => mind::println!("Usage: tally [show] | add <key> [n] | hold <seconds> <key> | effect <file> [crash] | reconcile <id> done|failed"),
    }
}
