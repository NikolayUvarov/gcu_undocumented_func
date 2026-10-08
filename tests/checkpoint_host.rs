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
#[path = "../blockstore/src/store.rs"]
mod store;
#[path = "../libmind/src/checkpoint.rs"]
mod checkpoint;

use checkpoint::{rebind, restore, save, Error, Label, Manifest, Names, Outcome, Restored, MAX};
use cid::{Cid, Codec};
use store::{Device, Entry, Extent, Head, Pin, Update, BUFFER, SECTOR};

struct Memory { data: Vec<u8> }
impl Device for &mut Memory {
    fn sectors(&self) -> u64 { (self.data.len() / SECTOR) as u64 }
    fn writable(&self) -> bool { true }
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool { let at = lba as usize * SECTOR; out.copy_from_slice(&self.data[at..at + out.len()]); true }
    fn write(&mut self, lba: u64, data: &[u8]) -> bool { let at = lba as usize * SECTOR; self.data[at..at + data.len()].copy_from_slice(data); true }
    fn flush(&mut self) -> bool { true }
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
    Local(store::Store::mount(medium, &mut room.index, &mut room.heads, &mut room.pins, &mut room.holes, &mut room.buffer, &mut room.scratch, 0).unwrap())
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
    for (at, value) in [(0, b'X'), (8, 2), (n - 1 - 4 - 1, 3)] {
        let mut bad = out[..n].to_vec();
        bad[at] = value;
        assert_eq!(Manifest::decode(&bad), Err(Error::Format), "byte {at}");
    }
    let counts = 8 + 2 + 1 + 5 + 2 + 2 + 8 + 8 + 36;
    for k in 0..3 {
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
