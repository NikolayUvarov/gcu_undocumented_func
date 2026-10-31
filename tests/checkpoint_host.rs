//! Host tests of checkpoints (libmind/src/checkpoint.rs, issue 306-STO-0009; MC-6.10, 6.11, 6.12): the manifest's
//! one encoding and its refusals, save and restore through the block store's own logic (blockstore/src/store.rs on a
//! simulated medium), fencing of a stale instance, contract and schema checks, names that disagree, effects that come
//! back pending, authorities checked against what an instance holds; messages between components, each writing only
//! its own checkpoint (313-STO-0030); a component moved to another store through a copy, active in one place only
//! (313-STO-0033).
#![allow(dead_code)]
#[path = "../libmind/src/sha256.rs"]
mod sha256;
#[path = "../libmind/src/cid.rs"]
mod cid;
#[path = "../libmind/src/dag.rs"]
mod dag;
#[path = "../libmind/src/tree.rs"]
mod tree;
#[path = "../libmind/src/journal.rs"]
mod journal;
#[path = "../blockstore/src/store.rs"]
mod store;
#[path = "../libmind/src/checkpoint.rs"]
mod checkpoint;
#[path = "../libmind/src/worktree.rs"]
mod worktree;
#[path = "../libmind/src/car.rs"]
mod car;

use car::{Named, Target};
use checkpoint::{read, rebind, release, released, restore, save, Applied, Error, Label, Manifest, Names, Outcome, Release, Restored, CLIENTS, MAX, OUTBOX};
use cid::{Cid, Codec};
use store::{hmac_sha256, Crypto, Device, Entry, Extent, Head, Pin, Update, BUFFER, LEASE_NS, SECTOR};

struct Memory { data: Vec<u8> }
impl Device for &mut Memory {
    fn sectors(&self) -> u64 { (self.data.len() / SECTOR) as u64 }
    fn writable(&self) -> bool { true }
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool { let at = lba as usize * SECTOR; out.copy_from_slice(&self.data[at..at + out.len()]); true }
    fn write(&mut self, lba: u64, data: &[u8]) -> bool { let at = lba as usize * SECTOR; self.data[at..at + data.len()].copy_from_slice(data); true }
    fn flush(&mut self) -> bool { true }
}

/// Records sealed by a tag alone (their bytes left plain): checkpoints need the store's logic, not its cipher, which
/// tests/blockstore_host.rs covers.
struct Tagged;
impl Crypto for Tagged {
    fn seal(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8]) -> [u8; 16] { hmac_sha256(key, &[nonce, aad, data])[..16].try_into().unwrap() }
    fn open(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8], tag: &[u8; 16]) -> bool { hmac_sha256(key, &[nonce, aad, data])[..16] == *tag }
    fn random(&self, _: &mut [u8]) -> bool { false }
}

struct Room { index: Vec<Entry>, heads: Vec<Head>, pins: Vec<Pin>, holes: Vec<Extent>, buffer: Box<[u8; BUFFER]>, scratch: Box<[u8; dag::CHUNK]> }
impl Room {
    fn new() -> Self { Room { index: vec![Entry::EMPTY; 64], heads: vec![Head::EMPTY; 16], pins: vec![Pin::EMPTY; 4], holes: vec![Extent::EMPTY; 64], buffer: Box::new([0; BUFFER]), scratch: Box::new([0; dag::CHUNK]) } }
}

/// The block store's logic as checkpoints see it, for one owner.
struct Local<'a>(store::Store<'a, &'a mut Memory>);
impl checkpoint::Store for Local<'_> {
    fn put(&mut self, data: &[u8]) -> Result<Cid, Error> { self.0.put(Codec::Raw, data).map_err(|_| Error::Store) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> { self.0.get(cid, out).map_err(|_| Error::Store) }
    fn snapshot(&mut self, names: [&str; 2]) -> Result<[(u64, Option<Cid>); 2], Error> {
        let mut out = [(0, None); 2];
        for (o, n) in out.iter_mut().zip(names) { *o = self.0.snapshot(n.as_bytes()).map_err(|_| Error::Store)?; }
        Ok(out)
    }
    fn commit(&mut self, names: [&str; 2], expected: u64, roots: [Cid; 2]) -> Result<u64, Error> {
        let updates = [0, 1].map(|k| Update { name: names[k].as_bytes(), expected, root: Some(roots[k]) });
        match self.0.commit(&updates, 7) { Ok(v) => Ok(v[0]), Err(store::Error::Conflict) => Err(Error::Fenced), Err(_) => Err(Error::Store) }
    }
}

/// Its blocks and names as copies see them (313-STO-0033), for the same owner.
impl dag::Blocks for Local<'_> {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, dag::Error> { dag::Blocks::put(&mut self.0, codec, data) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> { dag::Blocks::get(&mut self.0, cid, out) }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> { dag::Blocks::has(&mut self.0, cid) }
}
impl worktree::Names for Local<'_> {
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<Option<u64>, dag::Error> {
        match self.0.publish(name.as_bytes(), expected, root, 7) { Ok(v) => Ok(Some(v)), Err(store::Error::Conflict) => Ok(None), Err(_) => Err(dag::Error::Store) }
    }
    fn version(&mut self, name: &str) -> Result<u64, dag::Error> { self.0.snapshot(name.as_bytes()).map(|(v, _)| v).map_err(|_| dag::Error::Store) }
}
impl car::Store for Local<'_> {
    fn snapshot(&mut self, names: &[&str], heads: &mut [(u64, Option<Cid>)]) -> Result<(), dag::Error> {
        for (name, head) in names.iter().zip(heads.iter_mut()) { *head = self.0.snapshot(name.as_bytes()).map_err(|_| dag::Error::Store)?; }
        Ok(())
    }
    fn collect(&mut self) -> Result<u64, dag::Error> { self.0.collect().map(|c| c.blocks as u64).map_err(|_| dag::Error::Store) }
}

impl Target for Vec<u8> {
    fn write(&mut self, data: &[u8]) -> Result<(), car::Error> { self.extend_from_slice(data); Ok(()) }
}

fn mount<'a>(medium: &'a mut Memory, room: &'a mut Room) -> Local<'a> {
    Local(store::Store::mount(medium, &Tagged, None, &mut room.index, &mut room.heads, &mut room.pins, &mut room.holes, &mut room.buffer, &mut room.scratch, 0).unwrap())
}

// A state object of one block and a manifest for it, of contract "tally" 1, schema 1.
fn manifest(local: &mut Local, state: &[u8]) -> Manifest {
    let root = local.0.put(Codec::Raw, state).unwrap();
    let mut m = Manifest::new(Label::new("tally").unwrap(), 1, 1, root);
    m.add_resource("blockstore").unwrap();
    m.add_authority("blockstore:put,publish").unwrap();
    m.add_authority("files").unwrap();
    m
}

#[test]
fn a_manifest_has_one_encoding_and_refuses_the_rest() {
    let mut m = Manifest::new(Label::new("tally").unwrap(), 1, 3, Cid::raw(b"state"));
    (m.epoch, m.sequence) = (5, 42);
    m.add_resource("rtc").unwrap();
    m.add_authority("files").unwrap();
    let id = m.begin("write:ram:out.txt").unwrap();
    assert!(m.settle(id, Outcome::Done) && !m.settle(id + 1, Outcome::Done));
    m.begin("beep").unwrap();
    let mut out = [0u8; MAX];
    let n = m.encode(&mut out);
    assert_eq!(Manifest::decode(&out[..n]), Ok(m));
    // Every shorter prefix, a trailing byte, another magic or format version, an unknown outcome, counts past the
    // format: refused, never read as something else.
    for len in 0..n { assert_eq!(Manifest::decode(&out[..len]), Err(Error::Format), "{len}"); }
    let mut long = out[..n].to_vec();
    long.push(0);
    assert_eq!(Manifest::decode(&long), Err(Error::Format));
    for (at, value) in [(0, b'X'), (n - 1 - 4 - 1, 3)] {
        let mut bad = out[..n].to_vec();
        bad[at] = value;
        assert_eq!(Manifest::decode(&bad), Err(Error::Format), "byte {at}");
    }
    // Another format version is refused by its number (313-STO-0026: format 1 is not read, nor converted).
    for version in [1u16, 3] {
        let mut bad = out[..n].to_vec();
        bad[8..10].copy_from_slice(&version.to_le_bytes());
        assert_eq!(Manifest::decode(&bad), Err(Error::Version(version)));
    }
    let counts = 8 + 2 + 1 + 5 + 2 + 2 + 8 + 8 + 36;
    for k in 0..5 {
        let mut bad = out[..n].to_vec();
        bad[counts + k] = 200;
        assert_eq!(Manifest::decode(&bad), Err(Error::Format));
    }
    // Labels are short and printable; the lists are bounded.
    assert_eq!(Label::new("with space"), None);
    assert_eq!(Label::new(&"x".repeat(33)), None);
    for k in 0..7 { m.add_authority(&format!("a{k}")).unwrap(); }
    assert_eq!(m.add_authority("one-more"), Err(Error::Invalid));
    assert!(Names::of("a/b").is_none() && Names::of("").is_none());
    let names = Names::of("tally").unwrap();
    assert_eq!((names.manifest(), names.state()), ("checkpoint/tally", "checkpoint/tally/state"));
}

#[test]
fn a_saved_checkpoint_is_restored_after_a_remount() {
    let mut medium = Memory { data: vec![0; 256 * SECTOR] };
    let names = Names::of("tally").unwrap();
    let mut buffer = [0u8; MAX];
    let saved;
    {
        let mut room = Room::new();
        let mut local = mount(&mut medium, &mut room);
        assert_eq!(restore(&mut local, &names, "tally", 1, 1, &mut buffer), Ok(Restored::Fresh { epoch: 0 }));
        let mut m = manifest(&mut local, b"a=1\n");
        m.sequence = 1;
        assert_eq!(save(&mut local, &names, &mut m, 0, &mut buffer), Ok(1));
        let mut next = manifest(&mut local, b"a=2\n");
        next.sequence = 2;
        assert_eq!(save(&mut local, &names, &mut next, 1, &mut buffer), Ok(2));
        saved = next;
    }
    let mut room = Room::new();
    let mut local = mount(&mut medium, &mut room);
    let Ok(Restored::Found(m)) = restore(&mut local, &names, "tally", 1, 1, &mut buffer) else { panic!() };
    assert_eq!((m, m.epoch, m.sequence), (saved, 2, 2));
    let mut state = [0u8; 16];
    let n = local.0.get(&m.state, &mut state).unwrap();
    assert_eq!(&state[..n], b"a=2\n");
}

#[test]
fn a_stale_instance_is_fenced() {
    // Two instances restore the same checkpoint; the first to save holds the component, the other's save is refused
    // whole and changes nothing (MC-6.12).
    let mut medium = Memory { data: vec![0; 256 * SECTOR] };
    let mut room = Room::new();
    let mut local = mount(&mut medium, &mut room);
    let names = Names::of("tally").unwrap();
    let mut buffer = [0u8; MAX];
    let mut first = manifest(&mut local, b"a=1\n");
    save(&mut local, &names, &mut first, 0, &mut buffer).unwrap();
    let (old, new) = (restore(&mut local, &names, "tally", 1, 1, &mut buffer).unwrap(), restore(&mut local, &names, "tally", 1, 1, &mut buffer).unwrap());
    let mut winner = manifest(&mut local, b"a=2\n");
    assert_eq!(save(&mut local, &names, &mut winner, new.epoch(), &mut buffer), Ok(2));
    let mut loser = manifest(&mut local, b"a=9\n");
    assert_eq!(save(&mut local, &names, &mut loser, old.epoch(), &mut buffer), Err(Error::Fenced));
    assert_eq!(restore(&mut local, &names, "tally", 1, 1, &mut buffer), Ok(Restored::Found(winner)));
}

#[test]
fn another_contract_schema_or_disagreeing_names_are_not_restored() {
    let mut medium = Memory { data: vec![0; 256 * SECTOR] };
    let mut room = Room::new();
    let mut local = mount(&mut medium, &mut room);
    let names = Names::of("tally").unwrap();
    let mut buffer = [0u8; MAX];
    let mut m = manifest(&mut local, b"a=1\n");
    save(&mut local, &names, &mut m, 0, &mut buffer).unwrap();
    // Refused by the version found (313-STO-0030: tally 2 and its schema 1 are named, not read).
    assert_eq!(restore(&mut local, &names, "tally", 2, 1, &mut buffer), Err(Error::Contract(1)));
    assert_eq!(restore(&mut local, &names, "other", 1, 1, &mut buffer), Err(Error::Contract(1)));
    assert_eq!(restore(&mut local, &names, "tally", 1, 2, &mut buffer), Err(Error::Schema(1)));
    // The state's name moved alone: the checkpoint no longer describes it.
    let elsewhere = local.0.put(Codec::Raw, b"a=5\n").unwrap();
    assert_eq!(local.0.publish(names.state().as_bytes(), 1, &elsewhere, 7), Ok(2));
    assert_eq!(restore(&mut local, &names, "tally", 1, 1, &mut buffer), Err(Error::Inconsistent));
    // A manifest block that is not one, with the versions agreeing again: refused as a format.
    let junk = local.0.put(Codec::Raw, b"not a manifest").unwrap();
    assert_eq!(local.0.publish(names.manifest().as_bytes(), 1, &junk, 7), Ok(2));
    assert_eq!(restore(&mut local, &names, "tally", 1, 1, &mut buffer), Err(Error::Format));
}

#[test]
fn effects_begun_and_not_recorded_come_back_pending() {
    let mut medium = Memory { data: vec![0; 256 * SECTOR] };
    let mut room = Room::new();
    let mut local = mount(&mut medium, &mut room);
    let names = Names::of("tally").unwrap();
    let mut buffer = [0u8; MAX];
    // The intent is saved before the effect; the instance ends before it records the result.
    let mut m = manifest(&mut local, b"a=1\n");
    let done = m.begin("write:ram:a.txt").unwrap();
    m.settle(done, Outcome::Done);
    let open = m.begin("write:ram:b.txt").unwrap();
    save(&mut local, &names, &mut m, 0, &mut buffer).unwrap();
    let Ok(Restored::Found(mut back)) = restore(&mut local, &names, "tally", 1, 1, &mut buffer) else { panic!() };
    assert_eq!(back.pending().map(|e| (e.id, e.what.as_str())).collect::<Vec<_>>(), vec![(open, "write:ram:b.txt")]);
    // Reconciled by whoever knows the outcome, then saved: nothing is pending.
    assert!(back.settle(open, Outcome::Failed));
    save(&mut local, &names, &mut back, 1, &mut buffer).unwrap();
    let Ok(Restored::Found(again)) = restore(&mut local, &names, "tally", 1, 1, &mut buffer) else { panic!() };
    assert_eq!((again.pending().count(), again.effects().len()), (0, 2));
    // A full journal drops settled effects first and keeps every pending one.
    let mut full = again;
    for k in 0..30 { let id = full.begin("e").unwrap(); if k % 2 == 0 { full.settle(id, Outcome::Done); } }
    assert_eq!(full.effects().len(), checkpoint::EFFECTS);
    assert_eq!(full.pending().count(), checkpoint::EFFECTS - 1);
}

#[test]
fn authorities_are_checked_against_what_an_instance_holds() {
    let mut medium = Memory { data: vec![0; 256 * SECTOR] };
    let mut room = Room::new();
    let mut local = mount(&mut medium, &mut room);
    let m = manifest(&mut local, b"a=1\n");
    // The checkpoint names what the instance used; a new instance holding less is told what is missing.
    assert_eq!(rebind(&m, |_| true).count(), 0);
    assert_eq!(rebind(&m, |a| a != "files").map(|l| l.as_str()).collect::<Vec<_>>(), vec!["files"]);
    assert_eq!(m.resources().iter().map(|l| l.as_str()).collect::<Vec<_>>(), vec!["blockstore"]);
}


// The state of the model services below: one count, as text.
fn count_of(store: &mut impl checkpoint::Store, m: &Manifest) -> Result<u64, Error> {
    let mut b = [0u8; 32];
    let n = store.get(&m.state, &mut b)?;
    Ok(core::str::from_utf8(&b[..n]).unwrap().parse().unwrap())
}

// A component's checkpoint restored, or a new one of contract "model" 1 at count 0.
fn open(store: &mut impl checkpoint::Store, names: &Names, buffer: &mut [u8]) -> Result<(Manifest, u64, u64), Error> {
    match restore(store, names, "model", 1, 1, buffer)? {
        Restored::Found(m) => { let count = count_of(store, &m)?; Ok((m, m.epoch, count)) }
        Restored::Fresh { epoch } => Ok((Manifest::new(Label::new("model").unwrap(), 1, 1, store.put(b"0")?), epoch, 0)),
    }
}

// Puts the count as the state and saves the checkpoint after `from`.
fn keep(store: &mut impl checkpoint::Store, names: &Names, m: &mut Manifest, from: u64, count: u64, buffer: &mut [u8]) -> Result<u64, Error> {
    m.state = store.put(count.to_string().as_bytes())?;
    m.sequence += 1;
    save(store, names, m, from, buffer)
}

// The store with a stop: after `left` more calls, every call fails, as for an instance that ended there.
struct Stopping<'s, S> { store: &'s mut S, left: usize }
impl<S> Stopping<'_, S> {
    fn step(&mut self) -> Result<(), Error> { if self.left == 0 { return Err(Error::Store); } self.left -= 1; Ok(()) }
    fn dag(&mut self) -> Result<(), dag::Error> { self.step().map_err(|_| dag::Error::Store) }
}
impl<S: checkpoint::Store> checkpoint::Store for Stopping<'_, S> {
    fn put(&mut self, data: &[u8]) -> Result<Cid, Error> { self.step()?; self.store.put(data) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> { self.step()?; self.store.get(cid, out) }
    fn snapshot(&mut self, names: [&str; 2]) -> Result<[(u64, Option<Cid>); 2], Error> { self.step()?; self.store.snapshot(names) }
    fn commit(&mut self, names: [&str; 2], expected: u64, roots: [Cid; 2]) -> Result<u64, Error> { self.step()?; self.store.commit(names, expected, roots) }
}
impl<S: dag::Blocks> dag::Blocks for Stopping<'_, S> {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, dag::Error> { self.dag()?; self.store.put(codec, data) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> { self.dag()?; self.store.get(cid, out) }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> { self.dag()?; self.store.has(cid) }
}
impl<S: worktree::Names> worktree::Names for Stopping<'_, S> {
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<Option<u64>, dag::Error> { self.dag()?; self.store.publish(name, expected, root) }
    fn version(&mut self, name: &str) -> Result<u64, dag::Error> { self.dag()?; self.store.version(name) }
}
impl<S: car::Store> car::Store for Stopping<'_, S> {
    fn snapshot(&mut self, names: &[&str], heads: &mut [(u64, Option<Cid>)]) -> Result<(), dag::Error> { self.dag()?; self.store.snapshot(names, heads) }
    fn collect(&mut self) -> Result<u64, dag::Error> { self.dag()?; self.store.collect() }
}

// One request of a model service, in the protocol's order: restore, tell a repeat, apply in memory with the client's
// record, put the state, commit, answer. None: the instance ended before it answered, or the operation is stale.
fn serve(store: &mut impl checkpoint::Store, names: &Names, who: &str, op: u64, n: u64, buffer: &mut [u8]) -> Option<u64> {
    let (mut m, from, count) = open(store, names, buffer).ok()?;
    match m.applied(who, op) {
        Applied::Repeat(answer) => return Some(answer),
        Applied::Stale => return None,
        Applied::New => {}
    }
    m.record(who, op, count + n).ok()?;
    keep(store, names, &mut m, from, count + n, buffer).ok()?;
    Some(count + n)
}

#[test]
fn a_clients_operations_are_new_repeated_or_stale() {
    let mut m = Manifest::new(Label::new("model").unwrap(), 1, 1, Cid::raw(b"0"));
    assert_eq!(m.applied("shell", 1), Applied::New);
    m.record("shell", 1, 10).unwrap();
    m.sequence += 1;
    assert_eq!((m.applied("shell", 1), m.applied("shell", 2), m.applied("other", 1)), (Applied::Repeat(10), Applied::New, Applied::New));
    m.record("shell", 5, 50).unwrap();
    m.sequence += 1;
    assert_eq!((m.applied("shell", 4), m.applied("shell", 5)), (Applied::Stale, Applied::Repeat(50)));
    // An id not above the client's last, an id 0, a label that is not one: refused, nothing recorded.
    assert_eq!((m.record("shell", 5, 1), m.record("new", 0, 1), m.record("with space", 1, 1)), (Err(Error::Invalid), Err(Error::Invalid), Err(Error::Invalid)));
    // CLIENTS are kept; another drops the one applied longest ago, whose next operation is then taken as new.
    for k in 0..CLIENTS - 1 { m.record(&format!("c{k}"), 1, k as u64).unwrap(); m.sequence += 1; }
    assert_eq!(m.clients().len(), CLIENTS);
    m.record("late", 1, 99).unwrap();
    assert_eq!((m.clients().len(), m.applied("shell", 5), m.applied("late", 1)), (CLIENTS, Applied::New, Applied::Repeat(99)));
    // The table, the inbox and the outbox survive the encoding.
    m.inbox = 77;
    let s = m.send("peer", "hello").unwrap();
    m.send("peer", "again").unwrap();
    assert!(m.delivered(s) && !m.delivered(s));
    let mut out = [0u8; MAX];
    let n = m.encode(&mut out);
    assert_eq!(Manifest::decode(&out[..n]), Ok(m));
    // At most OUTBOX messages are owed: the next is refused until one is acknowledged.
    while m.outbox().len() < OUTBOX { m.send("peer", "more").unwrap(); }
    assert_eq!(m.send("peer", "full"), Err(Error::Invalid));
    let n = m.encode(&mut out);
    assert_eq!(Manifest::decode(&out[..n]), Ok(m));
}

#[test]
fn a_manifests_clients_and_outbox_have_one_encoding() {
    let mut m = Manifest::new(Label::new("model").unwrap(), 1, 1, Cid::raw(b"0"));
    m.sequence = 3;
    m.record("a", 2, 20).unwrap();
    m.record("b", 1, 10).unwrap();
    m.send("peer", "one").unwrap();
    m.send("peer", "two").unwrap();
    let mut out = [0u8; MAX];
    let n = m.encode(&mut out);
    // Each client: its label (1 + 1 bytes) and three u64; each message: its sequence and two labels ("peer", 3 letters).
    let (client, message) = (2 + 3 * 8, 8 + 5 + 4);
    let outbox = n - 2 * message;
    let clients = outbox - 2 * client;
    // A client twice, an operation 0, a client applied past the manifest's sequence; a message out of order or past the
    // last sequence given: none of them a manifest this format writes.
    let mut twice = out[..n].to_vec();
    twice[clients + client + 1] = b'a';
    let mut zero = out[..n].to_vec();
    zero[clients + 2..clients + 10].fill(0);
    let mut late = out[..n].to_vec();
    late[clients + 2 + 16..clients + 2 + 24].copy_from_slice(&4u64.to_le_bytes());
    let mut order = out[..n].to_vec();
    order[outbox + message..outbox + message + 8].copy_from_slice(&1u64.to_le_bytes());
    let mut past = out[..n].to_vec();
    past[outbox + message..outbox + message + 8].copy_from_slice(&3u64.to_le_bytes());
    for (what, bad) in [("twice", twice), ("zero", zero), ("late", late), ("order", order), ("past", past)] {
        assert_eq!(Manifest::decode(&bad), Err(Error::Format), "{what}");
    }
    assert_eq!(Manifest::decode(&out[..n]), Ok(m));
}

#[test]
fn a_service_stopped_anywhere_applies_each_operation_once() {
    // A client sends operations 1 to 5, adding 10 to 50, and sends each again until it is answered. The service stops
    // after each number of the store's calls in turn; then an answer is lost after the commit. Each operation is
    // applied once, and every answer to it is the count just after it.
    for stop in 0..7 {
        let mut medium = Memory { data: vec![0; 512 * SECTOR] };
        let mut room = Room::new();
        let mut local = mount(&mut medium, &mut room);
        let names = Names::of("model").unwrap();
        let mut buffer = [0u8; MAX];
        let mut total = 0;
        for op in 1..=5u64 {
            total += op * 10;
            let first = serve(&mut Stopping { store: &mut local, left: stop }, &names, "shell", op, op * 10, &mut buffer);
            if let Some(answer) = first { assert_eq!(answer, total, "stop {stop} op {op}"); }
            // Sent again: applied now if it was not, or answered with the first result.
            assert_eq!(serve(&mut local, &names, "shell", op, op * 10, &mut buffer), Some(total), "stop {stop} op {op}");
            assert_eq!(serve(&mut local, &names, "shell", op, op * 10, &mut buffer), Some(total), "stop {stop} op {op}: lost answer");
        }
        let (m, _, count) = open(&mut local, &names, &mut buffer).unwrap();
        assert_eq!((count, m.applied("shell", 5), m.applied("shell", 4)), (150, Applied::Repeat(150), Applied::Stale), "stop {stop}");
        // A stale operation is refused, and changes nothing.
        assert_eq!(serve(&mut local, &names, "shell", 3, 1000, &mut buffer), None);
        assert_eq!(open(&mut local, &names, &mut buffer).unwrap().2, 150);
    }
}

// Sends what `a` owes `b` and records each acknowledgement: `b` applies a message once, by `a`'s sequence, and
// acknowledges a repeat. Returns how many `a` still owes; Err: an instance stopped.
fn deliver(store: &mut impl checkpoint::Store, a: &Names, b: &Names, buffer: &mut [u8]) -> Result<usize, Error> {
    let (mut sender, from, count) = open(store, a, buffer)?;
    for message in sender.outbox().to_vec() {
        let (mut receiver, rfrom, rcount) = open(store, b, buffer)?;
        if receiver.applied("a", message.seq) == Applied::New {
            receiver.record("a", message.seq, rcount + 1)?;
            keep(store, b, &mut receiver, rfrom, rcount + 1, buffer)?;
        }
        sender.delivered(message.seq);
    }
    keep(store, a, &mut sender, from, count, buffer)?;
    Ok(sender.outbox().len())
}

#[test]
fn an_owed_message_is_sent_until_acknowledged_and_received_once() {
    for stop in 0..14 {
        let mut medium = Memory { data: vec![0; 512 * SECTOR] };
        let mut room = Room::new();
        let mut local = mount(&mut medium, &mut room);
        let (a, b) = (Names::of("a").unwrap(), Names::of("b").unwrap());
        let mut buffer = [0u8; MAX];
        // `a` owes three messages, saved with its state before any is sent.
        let (mut m, from, count) = open(&mut local, &a, &mut buffer).unwrap();
        for what in ["one", "two", "three"] { m.send("b", what).unwrap(); }
        keep(&mut local, &a, &mut m, from, count, &mut buffer).unwrap();
        // The first delivery stops anywhere; delivering again until nothing is owed receives each message once.
        let _ = deliver(&mut Stopping { store: &mut local, left: stop }, &a, &b, &mut buffer);
        while deliver(&mut local, &a, &b, &mut buffer).unwrap() > 0 {}
        assert_eq!(open(&mut local, &b, &mut buffer).unwrap().2, 3, "stop {stop}");
        let (sender, ..) = open(&mut local, &a, &mut buffer).unwrap();
        assert_eq!(sender.outbox().len(), 0);
        // The sequence goes on from the last given: a message after these is new to `b`.
        let mut sender = sender;
        assert_eq!(sender.send("b", "four"), Ok(4));
    }
}

#[test]
fn a_save_before_the_first_request_claims_the_component() {
    // Two instances restore the same checkpoint. The one that saves before taking a request holds the component; the
    // other is fenced at its own first save, so it never answers a request.
    let mut medium = Memory { data: vec![0; 256 * SECTOR] };
    let mut room = Room::new();
    let mut local = mount(&mut medium, &mut room);
    let names = Names::of("model").unwrap();
    let mut buffer = [0u8; MAX];
    assert_eq!(serve(&mut local, &names, "shell", 1, 5, &mut buffer), Some(5));
    let (mut old, old_from, _) = open(&mut local, &names, &mut buffer).unwrap();
    let (mut new, new_from, _) = open(&mut local, &names, &mut buffer).unwrap();
    let claimed = save(&mut local, &names, &mut new, new_from, &mut buffer).unwrap();
    assert_eq!((claimed, new.sequence), (new_from + 1, 1), "a claim changes no state");
    old.record("shell", 2, 6).unwrap();
    assert_eq!(keep(&mut local, &names, &mut old, old_from, 6, &mut buffer), Err(Error::Fenced));
    assert_eq!(serve(&mut local, &names, "shell", 2, 1, &mut buffer), Some(6));
}

#[test]
fn a_checkpoint_of_format_1_is_refused_by_its_number() {
    let mut medium = Memory { data: vec![0; 256 * SECTOR] };
    let mut room = Room::new();
    let mut local = mount(&mut medium, &mut room);
    let names = Names::of("tally").unwrap();
    let mut buffer = [0u8; MAX];
    let mut m = manifest(&mut local, b"a=1\n");
    save(&mut local, &names, &mut m, 0, &mut buffer).unwrap();
    // The same manifest written as format 1 would have it, under the same names.
    let mut old = [0u8; MAX];
    let n = m.encode(&mut old);
    old[8..10].copy_from_slice(&1u16.to_le_bytes());
    let block = local.0.put(Codec::Raw, &old[..n]).unwrap();
    let updates = [Update { name: names.manifest().as_bytes(), expected: 1, root: Some(block) }, Update { name: names.state().as_bytes(), expected: 1, root: Some(m.state) }];
    local.0.commit(&updates, 7).unwrap();
    assert_eq!(restore(&mut local, &names, "tally", 1, 1, &mut buffer), Err(Error::Version(1)));
}

// Messages between components (313-STO-0030): `a` owes `b` amounts, each its message's label. Each writes only its own
// checkpoint and reads the other's.

// The last checkpoint of component `of`, read and not restored.
fn peek(store: &mut impl checkpoint::Store, of: &Names, buffer: &mut [u8]) -> Result<Option<Manifest>, Error> {
    let mut state = [0u8; 32];
    Ok(read(store, of, "model", 1, 1, buffer, &mut state)?.map(|(m, _)| m))
}

// The sender's step: `a` restores, clears what `b`'s last checkpoint shows applied, moves `n` (if any) out of its count
// into a message owed to `b`, and saves. Err: the instance stopped.
fn send(store: &mut impl checkpoint::Store, a: &Names, b: &Names, n: u64, buffer: &mut [u8]) -> Result<(), Error> {
    let (mut m, from, count) = open(store, a, buffer)?;
    if let Some(receiver) = peek(store, b, buffer)? { m.acknowledge("a", "b", &receiver, |_| {}); }
    if n > 0 { m.send("b", &n.to_string())?; }
    keep(store, a, &mut m, from, count - n, buffer).map(|_| ())
}

// The receiver's step: `b` restores, reads `a`'s last checkpoint, applies what `a` owes it and it has not applied, and
// saves if it applied any. Returns how many; Err: the instance stopped.
fn receive(store: &mut impl checkpoint::Store, a: &Names, b: &Names, buffer: &mut [u8]) -> Result<usize, Error> {
    let (mut m, from, mut count) = open(store, b, buffer)?;
    let Some(sender) = peek(store, a, buffer)? else { return Ok(0) };
    let applied = m.receive("b", "a", &sender, |message| { count += message.what.as_str().parse::<u64>().unwrap(); Ok(count) })?;
    if applied > 0 { keep(store, b, &mut m, from, count, buffer)?; }
    Ok(applied)
}

// From the last checkpoints: `a`'s count, `b`'s, the amounts `a` owes `b` that `b` has not applied, and how many
// messages `a` still owes `b` that `b` applied.
fn ledger(store: &mut impl checkpoint::Store, a: &Names, b: &Names, buffer: &mut [u8]) -> (u64, u64, u64, usize) {
    let (sender, _, kept) = open(store, a, buffer).unwrap();
    let (receiver, _, got) = open(store, b, buffer).unwrap();
    let last = receiver.clients().iter().find(|c| c.who.as_str() == "a").map_or(0, |c| c.op);
    let owed = sender.owed("b").filter(|m| m.seq > last).map(|m| m.what.as_str().parse::<u64>().unwrap()).sum();
    (kept, got, owed, sender.owed("b").filter(|m| m.seq <= last).count())
}

#[derive(Clone, Copy, Debug)]
enum Step { Send(u64), Save, Receive }

#[test]
fn messages_between_components_are_applied_once_wherever_either_side_stops() {
    // `a` counts 100 and moves 1, 2, 4, 8 and 16 to `b`; `b` receives between, twice in a row too; `a` saves between,
    // clearing what `b` applied. Each step in turn stops after each number of the store's calls; the others run.
    use Step::*;
    let steps = [Send(1), Send(2), Receive, Receive, Save, Send(4), Receive, Send(8), Save, Receive, Send(16), Save];
    for (k, stopped) in steps.iter().enumerate() {
        for stop in 0..10 {
            let mut medium = Memory { data: vec![0; 1024 * SECTOR] };
            let mut room = Room::new();
            let mut local = mount(&mut medium, &mut room);
            let (a, b) = (Names::of("a").unwrap(), Names::of("b").unwrap());
            let mut buffer = [0u8; MAX];
            let (mut m, from, _) = open(&mut local, &a, &mut buffer).unwrap();
            keep(&mut local, &a, &mut m, from, 100, &mut buffer).unwrap();
            for (j, step) in steps.iter().enumerate() {
                let mut store = Stopping { store: &mut local, left: if j == k { stop } else { usize::MAX } };
                let done = match *step {
                    Send(n) => send(&mut store, &a, &b, n, &mut buffer),
                    Save => send(&mut store, &a, &b, 0, &mut buffer),
                    Receive => receive(&mut store, &a, &b, &mut buffer).map(|_| ()),
                };
                // Both counts and what is owed and not applied: the same total at every stop.
                let (kept, got, owed, applied) = ledger(&mut local, &a, &b, &mut buffer);
                assert_eq!(kept + got + owed, 100, "{stopped:?} stopped after {stop} calls; step {j}");
                // A sender's save leaves owed nothing its receiver applied.
                if done.is_ok() && !matches!(step, Receive) { assert_eq!(applied, 0, "{stopped:?} stopped after {stop} calls; step {j}"); }
            }
            // Received and saved once more: nothing is owed, and each message was applied once.
            receive(&mut local, &a, &b, &mut buffer).unwrap();
            send(&mut local, &a, &b, 0, &mut buffer).unwrap();
            let (kept, got, owed, applied) = ledger(&mut local, &a, &b, &mut buffer);
            assert_eq!((kept + got, owed, applied, open(&mut local, &a, &mut buffer).unwrap().0.outbox().len()), (100, 0, 0, 0), "{stopped:?} stopped after {stop} calls");
            assert!(got == 31 || matches!(stopped, Send(n) if got == 31 - n), "{stopped:?} stopped after {stop} calls: {got}");
        }
    }
}

#[test]
fn a_receiver_reading_the_same_outbox_again_applies_nothing() {
    let mut medium = Memory { data: vec![0; 512 * SECTOR] };
    let mut room = Room::new();
    let mut local = mount(&mut medium, &mut room);
    let (a, b) = (Names::of("a").unwrap(), Names::of("b").unwrap());
    let mut buffer = [0u8; MAX];
    let mut state = [0u8; 32];
    assert_eq!(read(&mut local, &a, "model", 1, 1, &mut buffer, &mut state), Ok(None));
    let (mut m, from, _) = open(&mut local, &a, &mut buffer).unwrap();
    keep(&mut local, &a, &mut m, from, 10, &mut buffer).unwrap();
    send(&mut local, &a, &b, 3, &mut buffer).unwrap();
    send(&mut local, &a, &b, 4, &mut buffer).unwrap();
    // A read gives the sender's manifest and state, checked as a restore checks them.
    let Ok(Some((sender, n))) = read(&mut local, &a, "model", 1, 1, &mut buffer, &mut state) else { panic!() };
    assert_eq!((&state[..n], sender.owed("b").count(), sender.epoch), (&b"3"[..], 2, 3));
    assert_eq!(read(&mut local, &a, "model", 2, 1, &mut buffer, &mut state), Err(Error::Contract(1)));
    let versions = |local: &mut Local| [a.manifest(), a.state(), b.manifest(), b.state()].map(|n| local.0.snapshot(n.as_bytes()).unwrap().0);
    assert_eq!(versions(&mut local), [3, 3, 0, 0]);
    // `b` applies both in one save, and writes only its own names; read again, nothing is applied or saved.
    assert_eq!(receive(&mut local, &a, &b, &mut buffer), Ok(2));
    assert_eq!(receive(&mut local, &a, &b, &mut buffer), Ok(0));
    assert_eq!((versions(&mut local), open(&mut local, &b, &mut buffer).unwrap().2), ([3, 3, 1, 1], 7));
    // Cleared at `a`'s next save; then only the next message is new to `b`.
    send(&mut local, &a, &b, 0, &mut buffer).unwrap();
    assert_eq!(open(&mut local, &a, &mut buffer).unwrap().0.outbox().len(), 0);
    assert_eq!(receive(&mut local, &a, &b, &mut buffer), Ok(0));
    send(&mut local, &a, &b, 2, &mut buffer).unwrap();
    assert_eq!((receive(&mut local, &a, &b, &mut buffer), receive(&mut local, &a, &b, &mut buffer)), (Ok(1), Ok(0)));
    let (receiver, _, count) = open(&mut local, &b, &mut buffer).unwrap();
    assert_eq!((count, receiver.applied("a", 3), open(&mut local, &a, &mut buffer).unwrap().2), (9, Applied::Repeat(9), 1));
}

#[test]
fn a_message_is_acknowledged_only_by_its_receiver_and_up_to_what_it_applied() {
    let model = || Manifest::new(Label::new("model").unwrap(), 1, 1, Cid::raw(b"0"));
    let mut a = model();
    for to in ["b", "c", "b", "c"] { a.send(to, "1").unwrap(); }
    // `b` applies its first message; the next is refused, and nothing after it is applied.
    let (mut b, mut seen) = (model(), vec![]);
    assert_eq!(b.receive("b", "a", &a, |m| { seen.push(m.seq); if m.seq == 1 { Ok(1) } else { Err(Error::Invalid) } }), Err(Error::Invalid));
    assert_eq!((seen, b.applied("a", 1), b.applied("a", 3)), (vec![1, 3], Applied::Repeat(1), Applied::New));
    // `c` applies its own two, in sequence order.
    let (mut c, mut order) = (model(), vec![]);
    assert_eq!(c.receive("c", "a", &a, |m| { order.push(m.seq); Ok(m.seq) }), Ok(2));
    assert_eq!(order, vec![2, 4]);
    // `c`'s table clears only what is owed to `c`, though `b`'s second is below what `c` applied; `b`'s clears only
    // up to what `b` applied.
    let mut cleared = vec![];
    assert_eq!(a.acknowledge("a", "c", &c, |m| cleared.push(m.seq)), 2);
    assert_eq!(a.acknowledge("a", "b", &b, |m| cleared.push(m.seq)), 1);
    assert_eq!((cleared, a.outbox().iter().map(|m| m.seq).collect::<Vec<_>>()), (vec![2, 4, 1], vec![3]));
    // A receiver with no record of `a`, or with another sender's: nothing.
    let mut other = model();
    other.record("x", 9, 0).unwrap();
    assert_eq!((a.acknowledge("a", "b", &model(), |_| {}), a.acknowledge("a", "b", &other, |_| {})), (0, 0));
    // Once `b` applies the rest it is cleared, once.
    assert_eq!(b.receive("b", "a", &a, |_| Ok(2)), Ok(1));
    assert_eq!((a.acknowledge("a", "b", &b, |_| {}), a.acknowledge("a", "b", &b, |_| {}), a.outbox().len()), (1, 0, 0));
}

// Moves through a copy (313-STO-0033): out of one store by a release and a copy, into another by an import and a restore.

// A component of the model with each part of a checkpoint: three operations of a client, an inbox, two messages owed
// and an effect pending. Its last checkpoint.
fn component(store: &mut impl checkpoint::Store, names: &Names, buffer: &mut [u8]) -> Manifest {
    for op in 1..=3 { serve(store, names, "shell", op, op * 10, buffer).unwrap(); }
    let (mut m, from, count) = open(store, names, buffer).unwrap();
    m.inbox = 41;
    m.send("peer", "one").unwrap();
    m.send("peer", "two").unwrap();
    m.begin("write:ram:x").unwrap();
    keep(store, names, &mut m, from, count, buffer).unwrap();
    open(store, names, buffer).unwrap().0
}

// Whether an instance restored in this store holds the component: a checkpoint found, to save from.
fn active(store: &mut impl checkpoint::Store, names: &Names, buffer: &mut [u8]) -> bool {
    matches!(restore(store, names, "model", 1, 1, buffer), Ok(Restored::Found(_)))
}

// A copy of the names at the roots given.
fn copy_of(store: &mut impl dag::Blocks, named: &[Named]) -> Result<Vec<u8>, car::Error> {
    let (mut chunk, mut block) = (Box::new([0; dag::CHUNK]), Box::new([0; dag::CHUNK]));
    car::write(store, named, Vec::new(), 1, &mut chunk, &mut block).map(|(copy, _)| copy)
}

// The move out: restore and release, or find the release made before; then the last checkpoint, which the names'
// history keeps, to a copy. Err: the instance stopped, or nothing to move.
fn move_out<S: checkpoint::Store + dag::Blocks>(store: &mut S, names: &Names, buffer: &mut [u8]) -> Result<Vec<u8>, Error> {
    let r = match restore(store, names, "model", 1, 1, buffer) {
        Ok(Restored::Found(m)) => release(store, names, m.epoch, buffer)?.0,
        Ok(Restored::Fresh { .. }) => return Err(Error::Invalid),
        Err(Error::Released(_)) => released(store, names, buffer)?,
        Err(e) => return Err(e),
    };
    let named = [Named { name: names.manifest(), version: r.epoch, root: r.manifest }, Named { name: names.state(), version: r.epoch, root: r.state }];
    copy_of(store, &named).map_err(|_| Error::Store)
}

// The move in: the copy imported, then restored.
fn move_in<S: checkpoint::Store + car::Store>(store: &mut S, copy: &[u8], names: &Names, buffer: &mut [u8]) -> Result<Restored, Error> {
    let (mut chunk, mut block) = (Box::new([0; dag::CHUNK]), Box::new([0; dag::CHUNK]));
    car::import(store, copy, &mut chunk, &mut block, |_, _| {}).map_err(|_| Error::Store)?;
    restore(store, names, "model", 1, 1, buffer)
}

// `m` as saved at `epoch`.
fn at(mut m: Manifest, epoch: u64) -> Manifest { m.epoch = epoch; m }

// The last checkpoint a release names, read from the store's blocks: its manifest and its count.
fn last_of(store: &mut impl checkpoint::Store, r: &Release) -> (Manifest, u64) {
    let mut b = [0u8; MAX];
    let n = store.get(&r.manifest, &mut b).unwrap();
    let m = Manifest::decode(&b[..n]).unwrap();
    assert_eq!(m.state, r.state);
    (m, count_of(store, &m).unwrap())
}

#[test]
fn a_move_stopped_anywhere_leaves_the_component_active_in_one_place_and_lands_whole() {
    // The move runs with one store stopping after each number of its calls in turn, until one it does not reach, the
    // other whole; then again, whole. After the stop the component is active at the source or at the destination, never
    // both; released at the source, its last checkpoint is still there, and written again it is the same copy. It lands
    // with the sequence, clients, inbox, outbox, effects and state the source last saved, and its next save is the next
    // epoch in its new store.
    let names = Names::of("model").unwrap();
    for stopped in ["source", "destination"] {
        for stop in 0.. {
            let (mut a, mut b) = (Memory { data: vec![0; 512 * SECTOR] }, Memory { data: vec![0; 512 * SECTOR] });
            let (mut ra, mut rb) = (Room::new(), Room::new());
            let (mut source, mut destination) = (mount(&mut a, &mut ra), mount(&mut b, &mut rb));
            let mut buffer = [0u8; MAX];
            let last = component(&mut source, &names, &mut buffer);
            let count = count_of(&mut source, &last).unwrap();
            let left = |which| if which == stopped { stop } else { usize::MAX };
            let mut out = Stopping { store: &mut source, left: left("source") };
            let first = move_out(&mut out, &names, &mut buffer);
            let mut unused = out.left;
            if let Ok(copy) = &first {
                let mut into = Stopping { store: &mut destination, left: left("destination") };
                let _ = move_in(&mut into, copy, &names, &mut buffer);
                if stopped == "destination" { unused = into.left; }
            }
            let (here, there) = (active(&mut source, &names, &mut buffer), active(&mut destination, &names, &mut buffer));
            assert!(!(here && there), "{stopped} stopped after {stop} calls: active in both");
            if !here {
                assert_eq!(restore(&mut source, &names, "model", 1, 1, &mut buffer), Err(Error::Released(last.epoch + 1)), "{stopped} {stop}");
                let r = released(&mut source, &names, &mut buffer).unwrap();
                assert_eq!((r.epoch, last_of(&mut source, &r)), (last.epoch, (last, count)), "{stopped} {stop}");
            }
            let again = move_out(&mut source, &names, &mut buffer).unwrap();
            if let Ok(copy) = first { assert_eq!(copy, again, "{stopped} {stop}"); }
            let Ok(Restored::Found(landed)) = move_in(&mut destination, &again, &names, &mut buffer) else { panic!("{stopped} {stop}") };
            assert_eq!((landed.epoch, at(landed, last.epoch)), (1, last), "{stopped} {stop}");
            assert_eq!((count_of(&mut destination, &landed), landed.outbox().len(), landed.pending().count(), landed.inbox), (Ok(count), 2, 1, 41));
            // A repeat is answered from the record; a new operation is applied, saved at the next epoch here.
            assert_eq!(serve(&mut destination, &names, "shell", 3, 30, &mut buffer), Some(60));
            assert_eq!(serve(&mut destination, &names, "shell", 4, 40, &mut buffer), Some(100));
            let (m, from, _) = open(&mut destination, &names, &mut buffer).unwrap();
            assert_eq!((from, m.sequence, m.applied("shell", 4)), (2, last.sequence + 1, Applied::Repeat(100)));
            assert!(!active(&mut source, &names, &mut buffer));
            // A stop the move did not reach: every point of it was cut.
            if unused > 0 { assert!(there && stop > 5, "{stopped} {stop}"); break; }
        }
    }
}

#[test]
fn a_released_component_is_refused_at_the_source_and_an_older_instance_fenced() {
    let mut medium = Memory { data: vec![0; 512 * SECTOR] };
    let mut room = Room::new();
    let mut local = mount(&mut medium, &mut room);
    let names = Names::of("model").unwrap();
    let mut buffer = [0u8; MAX];
    let last = component(&mut local, &names, &mut buffer);
    // An instance restored before the release. A release from an epoch since saved past is fenced and releases nothing.
    let (mut old, old_from, count) = open(&mut local, &names, &mut buffer).unwrap();
    assert_eq!(release(&mut local, &names, last.epoch - 1, &mut buffer), Err(Error::Fenced));
    assert!(active(&mut local, &names, &mut buffer));
    let copy = move_out(&mut local, &names, &mut buffer).unwrap();
    // A new instance is refused by its restore, by the release's epoch, never started fresh; a reader too.
    let refused = Error::Released(last.epoch + 1);
    assert_eq!(restore(&mut local, &names, "model", 1, 1, &mut buffer), Err(refused));
    assert_eq!(serve(&mut local, &names, "shell", 9, 1, &mut buffer), None);
    let mut state = [0u8; 32];
    assert_eq!(read(&mut local, &names, "model", 1, 1, &mut buffer, &mut state), Err(refused));
    assert_eq!(release(&mut local, &names, last.epoch + 1, &mut buffer), Err(refused));
    // The instance restored before is fenced at its save, which changes nothing.
    old.record("shell", 9, count + 1).unwrap();
    assert_eq!(keep(&mut local, &names, &mut old, old_from, count + 1, &mut buffer), Err(Error::Fenced));
    assert_eq!(restore(&mut local, &names, "model", 1, 1, &mut buffer), Err(refused));
    // Its copy imported back here finds both names at the release, another root: left as they are, still released.
    let mut conflicts = 0;
    let (mut chunk, mut block) = (Box::new([0; dag::CHUNK]), Box::new([0; dag::CHUNK]));
    car::import(&mut local, &copy[..], &mut chunk, &mut block, |_, o| conflicts += matches!(o, car::Outcome::Conflict { .. }) as usize).unwrap();
    assert_eq!((conflicts, restore(&mut local, &names, "model", 1, 1, &mut buffer)), (2, Err(refused)));
    // The release names the last checkpoint, the version before it in the names' history, which retains it: after a
    // collection with every lease ended, the copy written again is the same.
    let r = released(&mut local, &names, &mut buffer).unwrap();
    let (version, root) = local.0.snapshot(names.manifest().as_bytes()).unwrap();
    for (name, at) in [(names.manifest(), r.manifest), (names.state(), r.state)] {
        let kept: Vec<_> = local.0.history(name.as_bytes()).unwrap().iter().map(|v| (v.version, v.root())).collect();
        assert_eq!(kept[..2], [(version, root), (last.epoch, Some(at))], "{name}");
    }
    local.0.set_time(10 * LEASE_NS);
    assert!(local.0.collect().unwrap().blocks > 0);
    assert_eq!(last_of(&mut local, &r), (last, count));
    assert_eq!(move_out(&mut local, &names, &mut buffer), Ok(copy));
    // The release record has one encoding; another format is refused by its number; it is not a manifest.
    let mut out = [0u8; MAX];
    let n = r.encode(&mut out);
    assert_eq!((Release::decode(&out[..n]), checkpoint::Store::get(&mut local, &root.unwrap(), &mut buffer)), (Ok(r), Ok(n)));
    assert_eq!(&buffer[..n], &out[..n]);
    for len in 0..n { assert_eq!(Release::decode(&out[..len]), Err(Error::Format), "{len}"); }
    let mut long = out[..n].to_vec();
    long.push(0);
    assert_eq!(Release::decode(&long), Err(Error::Format));
    let mut other = out[..n].to_vec();
    other[8..10].copy_from_slice(&3u16.to_le_bytes());
    assert_eq!(Release::decode(&other), Err(Error::Version(3)));
    assert_eq!(Manifest::decode(&out[..n]), Err(Error::Format));
}

#[test]
fn a_moved_in_checkpoint_is_restored_and_saved_and_its_names_still_checked() {
    let names = Names::of("model").unwrap();
    let mut buffer = [0u8; MAX];
    let medium = || Memory { data: vec![0; 512 * SECTOR] };
    let (mut a, mut b, mut c, mut d, mut e) = (medium(), medium(), medium(), medium(), medium());
    let (mut ra, mut rb, mut rc, mut rd, mut re) = (Room::new(), Room::new(), Room::new(), Room::new(), Room::new());
    let (mut source, mut destination) = (mount(&mut a, &mut ra), mount(&mut b, &mut rb));
    let last = component(&mut source, &names, &mut buffer);
    let count = count_of(&mut source, &last).unwrap();
    let copy = move_out(&mut source, &names, &mut buffer).unwrap();
    // Into a store that never held its names: they come in at version 1, its epoch here, though it recorded 4.
    let Ok(Restored::Found(landed)) = move_in(&mut destination, &copy, &names, &mut buffer) else { panic!() };
    assert_eq!((landed.epoch, at(landed, last.epoch), count_of(&mut destination, &landed)), (1, last, Ok(count)));
    // Saved again here from that epoch; an instance restored before that save is fenced.
    let (mut stale, stale_from, _) = open(&mut destination, &names, &mut buffer).unwrap();
    assert_eq!(serve(&mut destination, &names, "shell", 4, 40, &mut buffer), Some(100));
    assert_eq!(open(&mut destination, &names, &mut buffer).unwrap().1, 2);
    assert_eq!(keep(&mut destination, &names, &mut stale, stale_from, 0, &mut buffer), Err(Error::Fenced));
    // The state's name moved alone: refused. The manifest's name moved along to the same manifest: its state and the
    // state's name still disagree, refused.
    let (version, manifest) = destination.0.snapshot(names.manifest().as_bytes()).unwrap();
    let elsewhere = destination.0.put(Codec::Raw, b"5").unwrap();
    assert_eq!(destination.0.publish(names.state().as_bytes(), version, &elsewhere, 7), Ok(version + 1));
    assert_eq!(restore(&mut destination, &names, "model", 1, 1, &mut buffer), Err(Error::Inconsistent));
    assert_eq!(destination.0.publish(names.manifest().as_bytes(), version, &manifest.unwrap(), 7), Ok(version + 1));
    assert_eq!(restore(&mut destination, &names, "model", 1, 1, &mut buffer), Err(Error::Inconsistent));
    // A copy of the manifest's name alone, or with the state's name at another root, is not restored where it lands.
    let r = released(&mut source, &names, &mut buffer).unwrap();
    let other = source.0.put(Codec::Raw, b"5").unwrap();
    let manifest = Named { name: names.manifest(), version: r.epoch, root: r.manifest };
    let alone = copy_of(&mut source, &[manifest]).unwrap();
    let moved = copy_of(&mut source, &[manifest, Named { name: names.state(), version: r.epoch, root: other }]).unwrap();
    assert_eq!(move_in(&mut mount(&mut c, &mut rc), &alone, &names, &mut buffer), Err(Error::Inconsistent));
    assert_eq!(move_in(&mut mount(&mut d, &mut rd), &moved, &names, &mut buffer), Err(Error::Inconsistent));
    // Into a store that held its names, removed there at version 5: they come in at 6, not their first version, and it
    // is refused (not provided).
    let mut held = mount(&mut e, &mut re);
    for op in 1..=4 { serve(&mut held, &names, "other", op, 1, &mut buffer).unwrap(); }
    let removal = [names.manifest(), names.state()].map(|n| Update { name: n.as_bytes(), expected: 4, root: None });
    held.0.commit(&removal, 7).unwrap();
    assert_eq!(restore(&mut held, &names, "model", 1, 1, &mut buffer), Ok(Restored::Fresh { epoch: 5 }));
    assert_eq!(move_in(&mut held, &copy, &names, &mut buffer), Err(Error::Inconsistent));
    assert_eq!(held.0.snapshot(names.manifest().as_bytes()), Ok((6, Some(r.manifest))));
}

#[test]
fn an_older_checkpoint_published_again_by_hand_at_a_later_version_is_refused() {
    // Four saves; then the manifest and state of epoch 2, which the names' history keeps, published again over both
    // names at version 5. Its epoch is not their version, and they are not at their first: not restored, nothing saved.
    let mut medium = Memory { data: vec![0; 512 * SECTOR] };
    let mut room = Room::new();
    let mut local = mount(&mut medium, &mut room);
    let names = Names::of("model").unwrap();
    let mut buffer = [0u8; MAX];
    for op in 1..=4 { serve(&mut local, &names, "shell", op, op, &mut buffer).unwrap(); }
    let at_2 = |local: &Local, name: &str| local.0.history(name.as_bytes()).unwrap().iter().find(|v| v.version == 2).and_then(|v| v.root()).unwrap();
    let older = [names.manifest(), names.state()].map(|n| Update { name: n.as_bytes(), expected: 4, root: Some(at_2(&local, n)) });
    assert_eq!(local.0.commit(&older, 7).unwrap()[..2], [5, 5]);
    assert_eq!(restore(&mut local, &names, "model", 1, 1, &mut buffer), Err(Error::Inconsistent));
    assert_eq!(serve(&mut local, &names, "shell", 5, 5, &mut buffer), None);
    assert_eq!(local.0.snapshot(names.manifest().as_bytes()).unwrap().0, 5);
}
