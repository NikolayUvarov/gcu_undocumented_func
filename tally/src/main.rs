#![no_std]
#![no_main]
// tally: named counters kept across instances by checkpoints in the block store (306-STO-0009, the pilot of
// docs/storage/checkpoints.md, contract "tally" 3). Each run is a new instance of one component, `tally`, or
// `tally.<name>` run `as <name>`: it restores the last checkpoint, applies one request and saves the next, fenced if
// another instance saved first (MC-6.12). Effects (writing a file) are journaled before they begin and come back pending
// if the instance ends first (Appendix B.4). An addition sent with an operation ID is applied once: sent again, it is
// answered with its first result (313-STO-0026). A component moves an amount to another as a message it owes: the
// receiver reads the sender's checkpoint and applies it once, the sender clears it once the receiver's checkpoint shows
// it applied, and each writes only its own checkpoint (313-STO-0030). A console program: it asks the shell for the
// store's client and the user's files, and holds nothing a checkpoint could hand back.
use core::fmt::Write;
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_BLOCKSTORE, SLOT_FILE};
use mind::checkpoint::{self, read, rebind, restore, save, Applied, Error, Label, Manifest, Message, Names, Outcome, Restored, FORMAT, LABEL, MAX, OUTBOX};
use mind::cid::Cid;
use mind::fs::File;
use mind::idl::blockstore::{self, Update};
use mind::idl::codec::{List, Text};
use mind::ipc::Endpoint;

mind::request!(REQUEST_CONSOLE | REQUEST_FILES | REQUEST_BLOCKSTORE);

const STORE: Endpoint = Endpoint(SLOT_BLOCKSTORE);
const CONTRACT: &str = "tally";
const VERSION: u16 = 3;
/// The state's schema: lines `key=count`, sorted by key, then `<seq> <n>` for each message owed, in the outbox's order
/// (its key and receiver are the manifest's).
const SCHEMA: u16 = 2;
const KEYS: usize = 32;
const KEY: usize = 16;
/// Bytes of a state at most: a line for each key, then one for each message owed.
const STATE_MAX: usize = KEYS * (KEY + 22) + OUTBOX * 42;

static mut BUFFER: [u8; MAX] = [0; MAX];
static mut STATE: [u8; STATE_MAX] = [0; STATE_MAX];

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
            Ok(Ok(done)) => done.versions.as_slice().first().copied().ok_or(Error::Store),
            Ok(Err(blockstore::Error::Conflict)) => Err(Error::Fenced),
            _ => Err(Error::Store),
        }
    }
}

// Formats into a fixed buffer.
struct Writer<'b> { out: &'b mut [u8], len: usize }
impl Write for Writer<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        self.out.get_mut(self.len..end).ok_or(core::fmt::Error)?.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// The counters and the amounts owed: the state of contract "tally" 3, schema 2.
struct Counters { keys: [Label; KEYS], counts: [u64; KEYS], len: usize, owed: [(u64, u64); OUTBOX], owing: usize }
impl Counters {
    const EMPTY: Counters = Counters { keys: [Label::EMPTY; KEYS], counts: [0; KEYS], len: 0, owed: [(0, 0); OUTBOX], owing: 0 };
    // The amounts are those of `outbox`'s messages, a line each in its order, after the counters.
    fn parse(text: &[u8], outbox: &[Message]) -> Option<Counters> {
        let mut c = Counters::EMPTY;
        for line in core::str::from_utf8(text).ok()?.lines() {
            if let Some((seq, n)) = line.split_once(' ') {
                let seq = seq.parse().ok()?;
                if outbox.get(c.owing)?.seq != seq { return None; }
                c.owed[c.owing] = (seq, n.parse().ok()?);
                c.owing += 1;
                continue;
            }
            let (key, count) = line.split_once('=')?;
            if c.owing > 0 || c.len == KEYS || key.len() > KEY { return None; }
            (c.keys[c.len], c.counts[c.len]) = (Label::new(key)?, count.parse().ok()?);
            c.len += 1;
        }
        (c.owing == outbox.len()).then_some(c)
    }
    fn count(&self, key: &str) -> u64 { self.keys[..self.len].iter().position(|k| k.as_str() == key).map_or(0, |i| self.counts[i]) }
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
    // Moves `n` out of `key`, which holds it, into message `seq`.
    fn owe(&mut self, key: &str, n: u64, seq: u64) {
        if let Some(i) = self.keys[..self.len].iter().position(|k| k.as_str() == key) { self.counts[i] -= n; }
        self.owed[self.owing] = (seq, n);
        self.owing += 1;
    }
    fn amount(&self, seq: u64) -> Option<u64> { self.owed[..self.owing].iter().find(|o| o.0 == seq).map(|o| o.1) }
    // Message `seq` is owed no more.
    fn forget(&mut self, seq: u64) {
        let Some(k) = self.owed[..self.owing].iter().position(|o| o.0 == seq) else { return };
        self.owed.copy_within(k + 1..self.owing, k);
        self.owing -= 1;
    }
    fn encode(&self, out: &mut [u8]) -> usize {
        let mut w = Writer { out, len: 0 };
        for k in 0..self.len { let _ = writeln!(w, "{}={}", self.keys[k], self.counts[k]); }
        for (seq, n) in &self.owed[..self.owing] { let _ = writeln!(w, "{} {}", seq, n); }
        w.len
    }
}

// The component `tally.<name>`, if that is a component's name.
fn component(name: &str) -> Option<Label> {
    let mut bytes = [0u8; LABEL];
    let mut w = Writer { out: &mut bytes, len: 0 };
    write!(w, "{}.{}", CONTRACT, name).ok()?;
    let len = w.len;
    let full = core::str::from_utf8(&bytes[..len]).ok()?;
    Names::of(full).and(Label::new(full))
}

// A component as the user names it: `b` for `tally.b`.
fn short(component: &Label) -> &str { component.as_str().strip_prefix("tally.").unwrap_or(component.as_str()) }

// The authorities this contract uses, as the checkpoint records them; held now if the capability is there.
fn holds(authority: &str) -> bool {
    let slot = match authority { "blockstore" => SLOT_BLOCKSTORE, "files" => SLOT_FILE, _ => return false };
    mind::dev::cap_info(slot).0 == CAP_KIND_ENDPOINT
}

/// The instance: its component, what it restored, the counters at that point, and the messages its receivers
/// acknowledged since its last save.
struct Instance { me: Label, from: u64, manifest: Manifest, counters: Counters, acked: [Message; OUTBOX], acks: usize }

fn open(me: Label, names: &Names, buffer: &mut [u8]) -> Option<Instance> {
    let restored = match restore(&mut Remote, names, CONTRACT, VERSION, SCHEMA, buffer) {
        Ok(r) => r,
        Err(Error::Version(found)) => { mind::println!("tally: a checkpoint of format {}; this build reads format {} and converts none", found, FORMAT); return None }
        Err(Error::Contract(found)) => { mind::println!("tally: a checkpoint of contract version {}; this build follows tally {} and converts none", found, VERSION); return None }
        Err(Error::Schema(found)) => { mind::println!("tally: a state of schema {}; this build reads schema {} and converts none", found, SCHEMA); return None }
        Err(e) => { mind::println!("tally: restore: {:?}", e); return None }
    };
    let (manifest, counters) = match restored {
        Restored::Fresh { .. } => {
            let mut m = Manifest::new(Label::new(CONTRACT)?, VERSION, SCHEMA, Cid::raw(b""));
            m.add_resource("blockstore").ok()?;
            m.add_authority("blockstore").ok()?;
            m.add_authority("files").ok()?;
            (m, Counters::EMPTY)
        }
        Restored::Found(m) => {
            // The state's block is checked against its CID by the store; then against the schema here.
            let n = match checkpoint::Store::get(&mut Remote, &m.state, buffer) { Ok(n) => n, Err(e) => { mind::println!("tally: state: {:?}", e); return None } };
            let Some(c) = Counters::parse(&buffer[..n], m.outbox()) else { mind::println!("tally: state: not of schema {}", SCHEMA); return None };
            (m, c)
        }
    };
    Some(Instance { me, from: restored.epoch(), manifest, counters, acked: [Message::EMPTY; OUTBOX], acks: 0 })
}

// Clears the messages each receiver's last checkpoint shows applied, to be printed once saved. A receiver's checkpoint
// is read, never written.
fn acknowledge(i: &mut Instance, buffer: &mut [u8], state: &mut [u8]) {
    let (mut to, mut n) = ([Label::EMPTY; OUTBOX], 0);
    for m in i.manifest.outbox() { if !to[..n].contains(&m.to) { to[n] = m.to; n += 1; } }
    for receiver in &to[..n] {
        let Some(names) = Names::of(receiver.as_str()) else { continue };
        match read(&mut Remote, &names, CONTRACT, VERSION, SCHEMA, buffer, state) {
            Ok(Some((r, _))) => {
                let (counters, acked, acks) = (&mut i.counters, &mut i.acked, &mut i.acks);
                i.manifest.acknowledge(i.me.as_str(), receiver.as_str(), &r, |m| {
                    counters.forget(m.seq);
                    if let Some(slot) = acked.get_mut(*acks) { *slot = *m; *acks += 1; }
                });
            }
            Ok(None) => {}
            Err(e) => mind::println!("tally: {}: {:?}: what it applied is not known", short(receiver), e),
        }
    }
}

// Stores the counters and saves the checkpoint after the one this instance restored, what its receivers acknowledged
// cleared first.
fn commit(names: &Names, i: &mut Instance, buffer: &mut [u8], state: &mut [u8]) -> bool {
    acknowledge(i, buffer, state);
    let n = i.counters.encode(state);
    match checkpoint::Store::put(&mut Remote, &state[..n]) {
        Ok(root) => i.manifest.state = root,
        Err(e) => { mind::println!("tally: state: {:?}", e); return false }
    }
    i.manifest.sequence += 1;
    match save(&mut Remote, names, &mut i.manifest, i.from, buffer) {
        Ok(epoch) => {
            i.from = epoch;
            for m in &i.acked[..i.acks] { mind::println!("ACKNOWLEDGED {} {} BY {}", short(&i.me), m.seq, short(&m.to)); }
            i.acks = 0;
            true
        }
        Err(Error::Fenced) => { mind::println!("tally: FENCED: epoch {} is no longer current; nothing saved", i.from); false }
        Err(e) => { mind::println!("tally: save: {:?}", e); false }
    }
}

fn show(i: &Instance) {
    mind::println!("TALLY EPOCH {} SEQUENCE {}", i.manifest.epoch, i.manifest.sequence);
    for k in 0..i.counters.len { mind::println!("{} = {}", i.counters.keys[k], i.counters.counts[k]); }
    for e in i.manifest.pending() { mind::println!("EFFECT {} {} PENDING: RECONCILE BEFORE TRYING AGAIN", e.id, e.what); }
    for m in i.manifest.outbox() { mind::println!("OWED {} {} TO {} {}", m.what, i.counters.amount(m.seq).unwrap_or(0), short(&m.to), m.seq); }
}

// Adds `n` (a number, 1 if none) to `key`. With an operation (`client`, `id`) it is applied once: a repeat is answered
// with its first result and an older one refused.
fn add(names: &Names, i: &mut Instance, buffer: &mut [u8], state: &mut [u8], key: &str, n: Option<&str>, op: Option<(&str, &str)>) {
    let Ok(n) = n.map_or(Ok(1), |n| n.parse::<u64>()) else { mind::println!("tally: not a count"); return };
    let op = match op.map(|(client, id)| (client, id.parse::<u64>())) {
        None => None,
        // A component's operations are the messages it owes, never a request's.
        Some((client, _)) if client.starts_with("tally.") => { mind::println!("tally: {} is a component: its operations are its messages", client); return }
        Some((client, Ok(id))) if id > 0 => Some((client, id)),
        Some(_) => { mind::println!("tally: an operation ID is a number from 1"); return }
    };
    if let Some((client, id)) = op {
        match i.manifest.applied(client, id) {
            Applied::Repeat(total) => { mind::println!("REPEATED {} {}: ANSWERED {}, NOT APPLIED AGAIN", client, id, total); return }
            Applied::Stale => { mind::println!("tally: operation {} of {} is older than its last: refused", id, client); return }
            Applied::New => {}
        }
    }
    let Some(total) = i.counters.add(key, n) else { mind::println!("tally: add: a key of at most {} printable bytes, at most {} keys", KEY, KEYS); return };
    if let Some((client, id)) = op {
        if i.manifest.record(client, id, total).is_err() { mind::println!("tally: a client of at most {} printable bytes", checkpoint::LABEL); return; }
    }
    if commit(names, i, buffer, state) { mind::println!("SAVED EPOCH {} SEQUENCE {}: {} = {}", i.from, i.manifest.sequence, key, total); }
}

// Moves `n` of `key` into a message owed to component `to`: the count drops at once, and the message holds the amount
// until the receiver's checkpoint shows it applied.
fn send(names: &Names, i: &mut Instance, buffer: &mut [u8], state: &mut [u8], key: &str, n: &str, to: &str) {
    let Ok(n @ 1..) = n.parse::<u64>() else { mind::println!("tally: a count from 1"); return };
    let Some(receiver) = component(to) else { mind::println!("tally: {} is not a component's name", to); return };
    let held = i.counters.count(key);
    if held < n { mind::println!("tally: {} holds {} {}: {} refused", short(&i.me), held, key, n); return; }
    // What the receivers applied frees its slot first.
    acknowledge(i, buffer, state);
    let Ok(seq) = i.manifest.send(receiver.as_str(), key) else { mind::println!("tally: {} messages are owed: none more until their receivers apply them", OUTBOX); return };
    i.counters.owe(key, n, seq);
    if commit(names, i, buffer, state) {
        mind::println!("OWED {} {} TO {} {}", key, n, to, seq);
        mind::println!("SAVED EPOCH {} SEQUENCE {}: {} = {}", i.from, i.manifest.sequence, key, i.counters.count(key));
    }
}

// Applies what component `from` owes this one and this one has not applied, in sequence order and in one save; a
// message applied before is not applied again. The sender's checkpoint is read, never written.
fn receive(names: &Names, i: &mut Instance, buffer: &mut [u8], state: &mut [u8], from: &str) {
    let Some(sender) = component(from) else { mind::println!("tally: {} is not a component's name", from); return };
    let Some(sender_names) = Names::of(sender.as_str()) else { return };
    let (owing, amounts) = match read(&mut Remote, &sender_names, CONTRACT, VERSION, SCHEMA, buffer, state) {
        Ok(Some((m, n))) => match Counters::parse(&state[..n], m.outbox()) {
            Some(c) => (m, c),
            None => { mind::println!("tally: {}: state: not of schema {}", from, SCHEMA); return }
        },
        Ok(None) => { mind::println!("NOTHING OWED BY {}", from); return }
        Err(e) => { mind::println!("tally: {}: {:?}", from, e); return }
    };
    let before = i.manifest.clients().iter().find(|c| c.who == sender).map_or(0, |c| c.op);
    let counters = &mut i.counters;
    let applied = i.manifest.receive(i.me.as_str(), sender.as_str(), &owing, |m| {
        let n = amounts.amount(m.seq).ok_or(Error::Invalid)?;
        counters.add(m.what.as_str(), n).ok_or(Error::Invalid)
    });
    match applied {
        Ok(0) => {}
        Ok(_) => if !commit(names, i, buffer, state) { return },
        Err(_) => { mind::println!("tally: receive: at most {} keys, each count below 2^64: nothing applied", KEYS); return }
    }
    if owing.owed(i.me.as_str()).next().is_none() { mind::println!("NOTHING OWED BY {}", from); }
    for m in owing.owed(i.me.as_str()) {
        let n = amounts.amount(m.seq).unwrap_or(0);
        if m.seq > before { mind::println!("ADDED {} {} FROM {} {}", m.what, n, from, m.seq); }
        else { mind::println!("REPEATED {} {} FROM {} {}: NOT APPLIED AGAIN", m.what, n, from, m.seq); }
    }
    if let Ok(1..) = applied { mind::println!("SAVED EPOCH {} SEQUENCE {}", i.from, i.manifest.sequence); }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("tally — named counters kept by checkpoints in the block store (contract tally 3). As <name> it is the component tally.<name>, which owes amounts to others and receives what they owe it.\nUsage: tally [as <name>] [show] | add <key> [n] [op <client> <id>] | hold <seconds> <key> | effect <file> [crash] | reconcile <id> done|failed\n       tally as <name> send <key> <n> to <name> | receive <name>");
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT { mind::fs::use_endpoint(Endpoint(SLOT_FILE)); }
    if !holds("blockstore") { mind::println!("tally: no client of the block store: nothing to restore from"); return; }
    let (buffer, state) = unsafe { (&mut *core::ptr::addr_of_mut!(BUFFER), &mut *core::ptr::addr_of_mut!(STATE)) };
    let (mut all, mut count) = ([""; 9], 0);
    for w in mind::process::args_str().split_whitespace() {
        if let Some(slot) = all.get_mut(count) { *slot = w; }
        count += 1;
    }
    let words = all.get(..count).unwrap_or(&["?"]);
    let (named, words) = match words { ["as", name, rest @ ..] => (Some(*name), rest), _ => (None, words) };
    let me = match named.map(component) {
        None => Label::new(CONTRACT).unwrap(),
        Some(Some(me)) => me,
        Some(None) => { mind::println!("tally: a component's name: at most {} of A-Z a-z 0-9 . _ -", LABEL - CONTRACT.len() - 1); return }
    };
    let names = Names::of(me.as_str()).unwrap();
    let Some(mut i) = open(me, &names, buffer) else { return };
    // The rights the checkpoint names come from this instance's grants, never from the checkpoint (MC-6.11).
    let mut files = true;
    for missing in rebind(&i.manifest, holds) {
        mind::println!("tally: AUTHORITY {} NOT HELD NOW", missing);
        files &= missing.as_str() != "files";
    }
    match (named.is_some(), words) {
        (_, [] | ["show"]) => show(&i),
        (_, ["add", key]) => add(&names, &mut i, buffer, state, key, None, None),
        (_, ["add", key, n]) => add(&names, &mut i, buffer, state, key, Some(n), None),
        (_, ["add", key, "op", client, id]) => add(&names, &mut i, buffer, state, key, None, Some((client, id))),
        (_, ["add", key, n, "op", client, id]) => add(&names, &mut i, buffer, state, key, Some(n), Some((client, id))),
        (true, ["send", key, n, "to", to]) => send(&names, &mut i, buffer, state, key, n, to),
        (true, ["receive", from]) => receive(&names, &mut i, buffer, state, from),
        (_, ["hold", seconds, key]) => {
            // An instance that restored, then stalls: if another saved meanwhile, its save is fenced.
            let Ok(seconds) = seconds.parse::<usize>() else { mind::println!("tally: not a number of seconds"); return };
            mind::println!("HOLDING EPOCH {}", i.from);
            mind::time::sleep(seconds * 1000);
            let Some(total) = i.counters.add(key, 1) else { return };
            if commit(&names, &mut i, buffer, state) { mind::println!("SAVED EPOCH {} SEQUENCE {}: {} = {}", i.from, i.manifest.sequence, key, total); }
        }
        (_, ["effect", file, crash @ ..]) if crash.len() <= 1 && crash.iter().all(|c| *c == "crash") => {
            let crash = crash.first().copied();
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
        (_, ["reconcile", id, outcome]) => {
            let outcome = match *outcome { "done" => Outcome::Done, "failed" => Outcome::Failed, _ => { mind::println!("tally: reconcile <id> done|failed"); return } };
            let Ok(id) = id.parse::<u64>() else { mind::println!("tally: not an effect id"); return };
            if !i.manifest.pending().any(|e| e.id == id) { mind::println!("tally: effect {} is not pending", id); return; }
            i.manifest.settle(id, outcome);
            if commit(&names, &mut i, buffer, state) { mind::println!("RECONCILED {} SAVED EPOCH {}", id, i.from); }
        }
        _ => mind::println!("Usage: tally [as <name>] [show] | add <key> [n] [op <client> <id>] | hold <seconds> <key> | effect <file> [crash] | reconcile <id> done|failed | as <name> send <key> <n> to <name> | as <name> receive <name>"),
    }
}
