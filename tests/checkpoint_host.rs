//! Host tests of checkpoints (libmind/src/checkpoint.rs, issue 306-STO-0009; MC-6.10, 6.11, 6.12): the manifest's
//! one encoding and its refusals, save and restore through the block store's own logic (blockstore/src/store.rs on a
//! simulated medium), fencing of a stale instance, contract and schema checks, names that disagree, effects that come
//! back pending, authorities checked against what an instance holds.
#![allow(dead_code)]
#[path = "../libmind/src/sha256.rs"]
mod sha256;
#[path = "../libmind/src/cid.rs"]
mod cid;
#[path = "../libmind/src/dag.rs"]
mod dag;
#[path = "../libmind/src/tree.rs"]
mod tree;
#[path = "../blockstore/src/store.rs"]
mod store;
#[path = "../libmind/src/checkpoint.rs"]
mod checkpoint;

use checkpoint::{rebind, restore, save, Applied, Error, Label, Manifest, Names, Outcome, Restored, CLIENTS, MAX, OUTBOX};
use cid::{Cid, Codec};
use store::{hmac_sha256, Crypto, Device, Entry, Extent, Head, Pin, Update, BUFFER, SECTOR};

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
    assert_eq!(restore(&mut local, &names, "tally", 2, 1, &mut buffer), Err(Error::Contract));
    assert_eq!(restore(&mut local, &names, "other", 1, 1, &mut buffer), Err(Error::Contract));
    assert_eq!(restore(&mut local, &names, "tally", 1, 2, &mut buffer), Err(Error::Schema));
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
impl<S: checkpoint::Store> Stopping<'_, S> {
    fn step(&mut self) -> Result<(), Error> { if self.left == 0 { return Err(Error::Store); } self.left -= 1; Ok(()) }
}
impl<S: checkpoint::Store> checkpoint::Store for Stopping<'_, S> {
    fn put(&mut self, data: &[u8]) -> Result<Cid, Error> { self.step()?; self.store.put(data) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> { self.step()?; self.store.get(cid, out) }
    fn snapshot(&mut self, names: [&str; 2]) -> Result<[(u64, Option<Cid>); 2], Error> { self.step()?; self.store.snapshot(names) }
    fn commit(&mut self, names: [&str; 2], expected: u64, roots: [Cid; 2]) -> Result<u64, Error> { self.step()?; self.store.commit(names, expected, roots) }
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
