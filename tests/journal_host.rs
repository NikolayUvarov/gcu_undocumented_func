//! Host tests of libmind/src/journal.rs (issue 312-STO-0034; MC-4.3, 4.5, 4.10, 4.12, 10.3): a journal, append-only,
//! checked from its head, through the block store's own logic (blockstore/src/store.rs on a simulated medium). An
//! entry's bytes are those of the reference DAG-CBOR library and of an encoding written here from the schema, and every
//! other encoding is refused; 1000 entries check whole, also after a remount and a collection, and are retained and
//! charged whole; the check refuses an entry erased, an entry changed, an older head published again, a name removed
//! and published again, a monotonic time that goes back within one boot, and what a store that lies answers; of two
//! writers the second appends after reading the head again; a journal in a memory domain goes with the domain; an
//! append stopped after each of the store's calls, or at each write on the medium, leaves a journal that checks; the
//! store's walk follows a journal one entry after another, within its steps; a copy carries a journal whole.
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
#[path = "../libmind/src/worktree.rs"]
mod worktree;
#[path = "../libmind/src/car.rs"]
mod car;

use cid::{Cid, Codec};
use dag::CHUNK;
use journal::{append, check, decode, list, Appended, Checked, Clock, Entry, Error, Label, Reading, Refused, Time, LABEL, MAX};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use store::{hmac_sha256, Crypto, Device, Extent, Head, Pin, Store, BUFFER, LEASE_NS, SECTOR, VOLUME};

const SECTORS: usize = 16384;

/// A medium in memory. From write `stop` on (counted from the first), no write is made and no flush done; with `torn`,
/// that write makes its first sector alone.
struct Memory { data: Vec<u8>, writes: usize, stop: Option<usize>, torn: bool }
impl Memory {
    fn new() -> Self { Memory { data: vec![0; SECTORS * SECTOR], writes: 0, stop: None, torn: false } }
    fn stopped(&self) -> bool { self.stop.is_some_and(|s| self.writes >= s) }
}
impl Device for &mut Memory {
    fn sectors(&self) -> u64 { (self.data.len() / SECTOR) as u64 }
    fn writable(&self) -> bool { true }
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool { let at = lba as usize * SECTOR; out.copy_from_slice(&self.data[at..at + out.len()]); true }
    fn write(&mut self, lba: u64, data: &[u8]) -> bool {
        self.writes += 1;
        let at = lba as usize * SECTOR;
        if self.stopped() {
            if self.torn && self.stop == Some(self.writes) { self.data[at..at + SECTOR].copy_from_slice(&data[..SECTOR]); }
            return false;
        }
        self.data[at..at + data.len()].copy_from_slice(data);
        true
    }
    fn flush(&mut self) -> bool { !self.stopped() }
}

/// Records sealed by a tag alone, their bytes left plain, so a test finds an entry on the medium: a journal needs the
/// store's logic, not its cipher, which tests/blockstore_host.rs covers. `random` gives memory domains their keys.
struct Tagged { random: bool }
static TAGGED: Tagged = Tagged { random: false };
static RANDOM: Tagged = Tagged { random: true };
static NEXT: AtomicU64 = AtomicU64::new(1);
impl Crypto for Tagged {
    fn seal(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8]) -> [u8; 16] { hmac_sha256(key, &[nonce, aad, data])[..16].try_into().unwrap() }
    fn open(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8], tag: &[u8; 16]) -> bool { hmac_sha256(key, &[nonce, aad, data])[..16] == *tag }
    fn random(&self, out: &mut [u8]) -> bool {
        if !self.random { return false; }
        for chunk in out.chunks_mut(32) { chunk.copy_from_slice(&sha256::digest(&NEXT.fetch_add(1, Ordering::Relaxed).to_le_bytes())[..chunk.len()]); }
        true
    }
}

struct Room { index: Vec<store::Entry>, heads: Vec<Head>, pins: Vec<Pin>, holes: Vec<Extent>, buffer: Box<[u8; BUFFER]>, scratch: Box<[u8; CHUNK]> }
impl Room {
    fn new() -> Self { Room::with(store::slots_for(SECTORS as u64)) }
    fn with(slots: usize) -> Self {
        Room { index: vec![store::Entry::EMPTY; slots], heads: vec![Head::EMPTY; 16], pins: vec![Pin::EMPTY; 4], holes: vec![Extent::EMPTY; 64], buffer: Box::new([0; BUFFER]), scratch: Box::new([0; CHUNK]) }
    }
}

type Local<'a> = Store<'a, &'a mut Memory>;

fn mount<'a>(medium: &'a mut Memory, room: &'a mut Room) -> Local<'a> { mount_with(medium, room, &TAGGED) }
fn mount_with<'a>(medium: &'a mut Memory, room: &'a mut Room, crypto: &'static Tagged) -> Local<'a> {
    Store::mount(medium, crypto, None, &mut room.index, &mut room.heads, &mut room.pins, &mut room.holes, &mut room.buffer, &mut room.scratch, 0).unwrap()
}

/// The badge the tests publish with (the shell's on the platform).
const OWNER: u16 = 7;
/// Entries a check may read: as many as any journal here holds.
const STEPS: u64 = 1 << 20;

/// The store as a journal sees it, for one owner.
impl<D: Device> journal::Store for Store<'_, D> {
    fn put(&mut self, entry: &[u8]) -> Result<Cid, Error> { Store::put(self, Codec::DagCbor, entry).map_err(|_| Error::Store) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> {
        Store::get(self, cid, out).map_err(|e| match e { store::Error::NotFound => Error::Missing, store::Error::Corrupt => Error::Corrupt, _ => Error::Store })
    }
    fn head(&mut self, name: &str) -> Result<(u64, Option<Cid>), Error> { Store::snapshot(self, name.as_bytes()).map_err(|_| Error::Store) }
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<u64, Error> {
        Store::publish(self, name.as_bytes(), expected, root, OWNER).map_err(|e| match e { store::Error::Conflict => Error::Conflict, store::Error::Incomplete => Error::Missing, _ => Error::Store })
    }
}

/// The store as copies see it.
impl<D: Device> worktree::Names for Store<'_, D> {
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<Option<u64>, dag::Error> {
        match Store::publish(self, name.as_bytes(), expected, root, OWNER) { Ok(v) => Ok(Some(v)), Err(store::Error::Conflict) => Ok(None), Err(_) => Err(dag::Error::Store) }
    }
    fn version(&mut self, name: &str) -> Result<u64, dag::Error> { Store::snapshot(self, name.as_bytes()).map(|(v, _)| v).map_err(|_| dag::Error::Store) }
}
impl<D: Device> car::Store for Store<'_, D> {
    fn snapshot(&mut self, names: &[&str], heads: &mut [(u64, Option<Cid>)]) -> Result<(), dag::Error> {
        for (name, head) in names.iter().zip(heads.iter_mut()) { *head = Store::snapshot(self, name.as_bytes()).map_err(|_| dag::Error::Store)?; }
        Ok(())
    }
    fn collect(&mut self) -> Result<u64, dag::Error> { Store::collect(self).map(|c| c.blocks as u64).map_err(|_| dag::Error::Store) }
}
impl car::Target for Vec<u8> {
    fn write(&mut self, data: &[u8]) -> Result<(), car::Error> { self.extend_from_slice(data); Ok(()) }
}

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
fn unhex(text: &str) -> Vec<u8> { (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect() }
fn buffer() -> Box<[u8; CHUNK]> { Box::new([0; CHUNK]) }

/// The clocks read at step `k`: the monotonic time in ms, the calendar time from the clock chip.
fn reading(k: u64) -> Reading { Reading { monotonic: 1_000_000_000 + k * 1_000_000, calendar: 1_798_000_000 + k, clock: Clock::Chip } }
fn text(k: u64) -> Vec<u8> { format!("note {k}").into_bytes() }

/// Appends note `k` as test's, its content put first.
fn add(store: &mut Local, name: &str, k: u64, buffer: &mut [u8; CHUNK]) -> Appended {
    let content = store.put(Codec::Raw, &text(k)).unwrap();
    append(store, name, reading(k), "test", "note", &content, buffer).unwrap()
}

/// A journal of notes 1 to `n`; each entry's CID.
fn journal(store: &mut Local, name: &str, n: u64) -> Vec<Cid> {
    let mut b = buffer();
    (1..=n).map(|k| add(store, name, k, &mut b).cid).collect()
}

fn checked(version: u64) -> Checked { Checked { version, entries: version, first: 1, last: version } }

/// An entry made here, not by `append`: of test's, a note, from the clock chip.
fn entry(seq: u64, boot: u64, monotonic: u64, content: Cid, prev: Option<Cid>) -> Entry {
    Entry { seq, time: Time { boot, monotonic, calendar: 1_798_000_000, clock: Clock::Chip }, source: Label::new("test").unwrap(), kind: Label::new("note").unwrap(), content, prev }
}
fn encoded(e: &Entry) -> Vec<u8> {
    let mut out = [0u8; MAX];
    let n = e.encode(&mut out).unwrap();
    out[..n].to_vec()
}

/// An entry read back from the store.
fn read(store: &mut Local, cid: &Cid) -> Entry {
    let mut b = buffer();
    let n = store.get(cid, &mut b[..]).unwrap();
    decode(&b[..n]).unwrap()
}

/// A value of an entry's field, written here as DAG-CBOR: a number, a text, bytes, a link.
#[derive(Clone, Copy)]
enum V<'a> { U(u64), T(&'a str), B(&'a [u8]), L(Cid) }

fn cbor_head(out: &mut Vec<u8>, major: u8, value: u64) {
    let m = major << 5;
    match value {
        0..=23 => out.push(m | value as u8),
        24..=0xff => out.extend([m | 24, value as u8]),
        0x100..=0xffff => { out.push(m | 25); out.extend((value as u16).to_be_bytes()) }
        0x1_0000..=0xffff_ffff => { out.push(m | 26); out.extend((value as u32).to_be_bytes()) }
        _ => { out.push(m | 27); out.extend(value.to_be_bytes()) }
    }
}

/// A map of `fields` in the order given, written here from the DAG-CBOR specification, not by journal.rs, so that
/// entries the encoder refuses can be made too.
fn written(fields: &[(&str, V)]) -> Vec<u8> {
    let mut out = Vec::new();
    cbor_head(&mut out, 5, fields.len() as u64);
    for (key, value) in fields {
        cbor_head(&mut out, 3, key.len() as u64);
        out.extend(key.as_bytes());
        match value {
            V::U(n) => cbor_head(&mut out, 0, *n),
            V::T(t) => { cbor_head(&mut out, 3, t.len() as u64); out.extend(t.as_bytes()) }
            V::B(b) => { cbor_head(&mut out, 2, b.len() as u64); out.extend(*b) }
            V::L(c) => { out.extend(unhex("d82a582500")); out.extend(c.to_bytes()) }
        }
    }
    out
}

/// An entry's fields in the schema's order.
fn fields(e: &Entry) -> Vec<(&'static str, V<'_>)> {
    let clock = match e.time.clock { Clock::None => 0, Clock::Chip => 1, Clock::Network => 2 };
    let mut f = vec![("v", V::U(1)), ("seq", V::U(e.seq)), ("boot", V::U(e.time.boot)), ("kind", V::T(e.kind.as_str()))];
    if let Some(prev) = e.prev { f.push(("prev", V::L(prev))); }
    f.extend([("clock", V::U(clock)), ("source", V::T(e.source.as_str())), ("content", V::L(e.content)), ("calendar", V::U(e.time.calendar)), ("monotonic", V::U(e.time.monotonic))]);
    f
}

#[test]
fn an_entry_is_the_schemas_encoding_and_reads_back() {
    // Two entries as the reference library dag-cbor 0.3.3 (with multiformats 0.3.1) encodes them, keys sorted by it.
    let abc = Cid::raw(b"abc");
    let first = Entry { seq: 1, time: Time { boot: 1, monotonic: 5_000_000_000, calendar: 1_798_000_000, clock: Clock::Chip }, source: Label::new("blocks").unwrap(), kind: Label::new("note").unwrap(), content: abc, prev: None };
    assert_eq!(hex(&encoded(&first)), "a9617601637365710164626f6f7401646b696e64646e6f746565636c6f636b0166736f7572636566626c6f636b7367636f6e74656e74d82a58250001551220ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad6863616c656e6461721a6b2b4d80696d6f6e6f746f6e69631b000000012a05f200");
    let one = Cid::of(Codec::DagCbor, &encoded(&first));
    assert_eq!(one.to_string(), "bafyreieoxgtpwztgbcov5pnuveknbxlljmfmnubj52iagv3n6pn6ig6pli");
    let second = Entry { seq: 2, time: Time { boot: 1, monotonic: 6_000_000_000, calendar: 0, clock: Clock::None }, prev: Some(one), ..first };
    assert_eq!(hex(&encoded(&second)), "aa617601637365710264626f6f7401646b696e64646e6f74656470726576d82a582500017112208eb9a6fb6666089d5ebdb4a914d0dd6b4b0ac6d029ee9003576df3dbe41bcf5a65636c6f636b0066736f7572636566626c6f636b7367636f6e74656e74d82a58250001551220ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad6863616c656e64617200696d6f6e6f746f6e69631b0000000165a0bc00");
    assert_eq!(Cid::of(Codec::DagCbor, &encoded(&second)).to_string(), "bafyreicjxl533v7ewmzzvlc65onwjoka2pvx7qhalrwqgxbsj4omusrimm");
    for e in [first, second] { assert_eq!(decode(&encoded(&e)), Ok(e)); }
    // Numbers in every integer form, labels of one byte and of LABEL, every printable byte, each clock: the encoding
    // written here from the schema, and read back the same.
    let node = Cid::of(Codec::DagCbor, b"an entry");
    let long = "~".repeat(LABEL);
    let printable: String = (0x21u8..0x7f).map(char::from).collect();
    let numbers = [0, 1, 23, 24, 255, 256, 65_535, 65_536, (1 << 32) - 1, 1 << 32, u64::MAX];
    for (k, &n) in numbers.iter().enumerate() {
        let label = |at: usize| Label::new(&printable[at % 60..at % 60 + 1 + at % LABEL]).unwrap();
        let clock = [Clock::None, Clock::Chip, Clock::Network][k % 3];
        let seq = n.max(1);
        let e = Entry {
            seq, time: Time { boot: numbers[(k + 3) % numbers.len()], monotonic: n, calendar: if clock == Clock::None { 0 } else { numbers[(k + 5) % numbers.len()] }, clock },
            source: if k == 0 { Label::new(&long).unwrap() } else { label(k * 7) }, kind: if k == 1 { Label::new("x").unwrap() } else { label(k * 13) },
            content: if k % 2 == 0 { abc } else { node }, prev: (seq > 1).then_some(node),
        };
        let bytes = encoded(&e);
        assert!(bytes.len() <= MAX);
        assert_eq!(bytes, written(&fields(&e)), "{k}");
        assert_eq!(decode(&bytes), Ok(e), "{k}");
    }
    // The largest entry fits MAX exactly.
    let max = Entry { seq: u64::MAX, time: Time { boot: u64::MAX, monotonic: u64::MAX, calendar: u64::MAX, clock: Clock::Network }, source: Label::new(&long).unwrap(), kind: Label::new(&long).unwrap(), content: node, prev: Some(node) };
    assert_eq!(encoded(&max).len(), MAX);
}

#[test]
fn only_the_canonical_encoding_decodes() {
    let note = Cid::raw(b"note");
    let node = Cid::of(Codec::DagCbor, b"before");
    let e = Entry { seq: 300, time: Time { boot: 2, monotonic: 70_000, calendar: 1_798_000_000, clock: Clock::Chip }, source: Label::new("blocks").unwrap(), kind: Label::new("note").unwrap(), content: note, prev: Some(node) };
    let bytes = encoded(&e);
    // Every byte changed to every other value: refused, or read as exactly what it encodes.
    let mut read = 0;
    for at in 0..bytes.len() {
        for value in 0..=255u8 {
            if value == bytes[at] { continue; }
            let mut changed = bytes.clone();
            changed[at] = value;
            if let Ok(other) = decode(&changed) {
                assert_eq!(encoded(&other), changed, "byte {at} as {value:#x}");
                read += 1;
            }
        }
    }
    // A digest's bytes, a label's characters, a number's: some changes are other valid entries.
    assert!(read > 0);
    for len in 0..bytes.len() { assert!(decode(&bytes[..len]).is_err(), "cut to {len}"); }
    let mut long = bytes.clone();
    long.push(0);
    assert_eq!(decode(&long), Err(dag::Error::Format));
    let f = fields(&e);
    let with = |key: &str, value: V| -> Vec<u8> { written(&f.iter().map(|&(k, v)| if k == key { (k, value) } else { (k, v) }).collect::<Vec<_>>()) };
    let without = |key: &str| -> Vec<u8> { written(&f.iter().filter(|(k, _)| *k != key).copied().collect::<Vec<_>>()) };
    let mut swapped = f.clone();
    swapped.swap(2, 3);
    let mut extra = f.clone();
    extra.push(("zzz", V::U(0)));
    let mut longer_map = vec![0xb8, f.len() as u8];
    longer_map.extend(&bytes[1..]);
    let raw_prev = Cid::raw(b"before");
    for (case, bytes, error) in [
        ("another version", with("v", V::U(2)), dag::Error::Version),
        ("version 0", with("v", V::U(0)), dag::Error::Version),
        ("keys out of order", written(&swapped), dag::Error::Format),
        ("an extra key", written(&extra), dag::Error::Format),
        ("no source", without("source"), dag::Error::Format),
        ("no monotonic time", without("monotonic"), dag::Error::Format),
        ("a map head written longer", longer_map, dag::Error::Format),
        ("sequence 0", { let z = Entry { seq: 0, prev: None, ..e }; written(&fields(&z)) }, dag::Error::Format),
        ("sequence 1 with a link before", with("seq", V::U(1)), dag::Error::Format),
        ("sequence 2 without one", { let z = Entry { seq: 2, prev: None, ..e }; written(&fields(&z)) }, dag::Error::Format),
        ("an entry before linked as raw", with("prev", V::L(raw_prev)), dag::Error::Format),
        ("a clock of none of the three", with("clock", V::U(3)), dag::Error::Format),
        ("a calendar time without a clock", with("clock", V::U(0)), dag::Error::Format),
        ("an empty kind", with("kind", V::T("")), dag::Error::Format),
        ("a kind over LABEL", with("kind", V::T(&"k".repeat(LABEL + 1))), dag::Error::Format),
        ("a kind with a space", with("kind", V::T("a note")), dag::Error::Format),
        ("a source not ASCII", with("source", V::T("блоки")), dag::Error::Format),
        ("a kind as bytes", with("kind", V::B(b"note")), dag::Error::Format),
        ("a content as text", with("content", V::T("note")), dag::Error::Format),
        ("a negative boot", { let mut b = with("boot", V::U(2)); let at = b.windows(5).position(|w| w == b"\x64boot").unwrap() + 5; b[at] = 0x21; b }, dag::Error::Format),
    ] {
        assert_eq!(decode(&bytes), Err(error), "{case}");
    }
    // A link without tag 42, and a sequence written longer than its shortest form.
    let mut untagged = bytes.clone();
    let at = untagged.windows(8).position(|w| w == b"\x67content").unwrap() + 8;
    untagged.splice(at..at + 2, []);
    assert_eq!(decode(&untagged), Err(dag::Error::Format));
    let mut wider = bytes.clone();
    let at = wider.windows(4).position(|w| w == b"\x63seq").unwrap() + 4;
    wider.splice(at..at + 3, [0x1a, 0, 0, 1, 0x2c]);
    assert_eq!(decode(&wider), Err(dag::Error::Format));
    // The encoder refuses what decodes as no entry; a label is 1 to LABEL printable bytes.
    let mut out = [0u8; MAX];
    for bad in [Entry { seq: 0, prev: None, ..e }, Entry { prev: None, ..e }, Entry { seq: 1, ..e }, Entry { prev: Some(raw_prev), ..e },
                Entry { time: Time { clock: Clock::None, ..e.time }, ..e }] {
        assert_eq!(bad.encode(&mut out), Err(dag::Error::Format), "{bad:?}");
    }
    for bad in ["", "a b", "\u{7f}", "é", &"x".repeat(LABEL + 1)] { assert_eq!(Label::new(bad), None, "{bad:?}"); }
}

#[test]
fn a_thousand_entries_check_whole_after_a_remount_and_a_collection() {
    let mut medium = Memory::new();
    let mut b = buffer();
    let n = 1000;
    let cids;
    // The bytes of the contents of notes 1 to k; an owner is charged each version its name keeps, at its root.
    let bytes = |k: u64| (1..=k).map(|j| text(j).len() as u64).sum::<u64>();
    let charged = (n - 3..=n).map(bytes).sum::<u64>();
    {
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        cids = journal(&mut store, "diary", n);
        assert_eq!(check(&mut store, "diary", STEPS, &mut b), Ok(checked(n)));
        assert_eq!((store.stats().blocks, store.usage(OWNER).retained), (2 * n as u32, charged));
        // The last five, newest first, each with its content.
        let mut seen = Vec::new();
        let listed = list(&mut store, "diary", 5, &mut b, |s, e, cid| {
            let mut out = [0u8; 32];
            let len = s.get(&e.content, &mut out).unwrap();
            seen.push((e.seq, *cid, out[..len].to_vec(), e.time, e.prev));
        });
        assert_eq!(listed, Ok(Checked { version: n, entries: 5, first: n - 4, last: n }));
        for (k, (seq, cid, content, time, prev)) in seen.into_iter().enumerate() {
            let at = n - k as u64;
            let r = reading(at);
            assert_eq!((seq, cid, content, prev), (at, cids[at as usize - 1], text(at), Some(cids[at as usize - 2])));
            assert_eq!(time, Time { boot: 1, monotonic: r.monotonic, calendar: r.calendar, clock: Clock::Chip });
        }
        // The first entry, as the schema has it.
        let r = reading(1);
        let time = Time { boot: 1, monotonic: r.monotonic, calendar: r.calendar, clock: r.clock };
        assert_eq!(read(&mut store, &cids[0]), Entry { time, ..entry(1, 1, 0, Cid::raw(&text(1)), None) });
    }
    // A new instance mounts the medium: the journal checks whole, and its sizes are read again from its entries.
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Ok(checked(n)));
    assert_eq!(store.usage(OWNER).retained, charged);
    // Once every lease has ended a collection frees what nothing retains, a note never appended, and keeps the journal.
    let loose = store.put(Codec::Raw, b"never appended").unwrap();
    store.set_time(LEASE_NS + 1);
    assert_eq!(store.collect().unwrap().blocks, 1);
    assert!(!store.has(&loose) && cids.iter().all(|c| store.has(c)));
    assert_eq!(store.stats().blocks, 2 * n as u32);
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Ok(checked(n)));
    // And it goes on.
    assert_eq!(add(&mut store, "diary", n + 1, &mut b).entry.seq, n + 1);
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Ok(checked(n + 1)));
}

/// Erases the sector of the block `cid` where it lies on the medium. Records here are sealed by a tag alone: a block's
/// record is found in the clear by its header (`MIND-BLK`, the CID at 16), after its frame at the sector's start.
fn erase(medium: &mut Memory, cid: &Cid) {
    let key = cid.to_bytes();
    let at = (0..medium.data.len() - 84).find(|&at| &medium.data[at..at + 8] == b"MIND-BLK" && medium.data[at + 16..at + 52] == key).expect("the block on the medium");
    let sector = at / SECTOR * SECTOR;
    medium.data[sector..sector + SECTOR].fill(0);
}

#[test]
fn the_check_refuses_an_entry_erased() {
    let mut medium = Memory::new();
    let mut b = buffer();
    let cids = {
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        journal(&mut store, "diary", 10)
    };
    erase(&mut medium, &cids[3]);
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    // Six entries read whole from the head down, then the fourth is missing.
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Err(Refused { error: Error::Missing, version: 10, at: 4, checked: 6 }));
    // The store's walk finds it missing too: no entry is appended after it, and no collection frees anything.
    let content = store.put(Codec::Raw, b"after").unwrap();
    assert_eq!(append(&mut store, "diary", reading(11), "test", "note", &content, &mut b).map(|a| a.entry.seq), Err(Error::Missing));
    assert_eq!(store.snapshot(b"diary"), Ok((10, Some(cids[9]))));
    assert_eq!(store.collect(), Err(store::Error::Incomplete));
}

/// The store, answering the CIDs of `swap` with other bytes, as a store that lies does.
struct Swapped<'s, 'a> { store: &'s mut Local<'a>, swap: HashMap<Cid, Vec<u8>> }
impl journal::Store for Swapped<'_, '_> {
    fn put(&mut self, entry: &[u8]) -> Result<Cid, Error> { journal::Store::put(self.store, entry) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> {
        match self.swap.get(cid) { Some(bytes) => { out[..bytes.len()].copy_from_slice(bytes); Ok(bytes.len()) } None => journal::Store::get(self.store, cid, out) }
    }
    fn head(&mut self, name: &str) -> Result<(u64, Option<Cid>), Error> { journal::Store::head(self.store, name) }
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<u64, Error> { journal::Store::publish(self.store, name, expected, root) }
}

#[test]
fn the_check_refuses_an_entry_changed() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut b = buffer();
    let cids = journal(&mut store, "diary", 6);
    // Another entry with the same links in place of the third: its bytes do not match the CID the fourth links.
    let third = read(&mut store, &cids[2]);
    let changed = Entry { kind: Label::new("changed").unwrap(), ..third };
    assert_eq!((changed.prev, changed.content), (third.prev, third.content));
    let mut lying = Swapped { store: &mut store, swap: HashMap::from([(cids[2], encoded(&changed))]) };
    assert_eq!(check(&mut lying, "diary", STEPS, &mut b), Err(Refused { error: Error::Corrupt, version: 6, at: 3, checked: 3 }));
    // Rewritten in the store: the changed third, then the fourth to the sixth again over it, each with its sequence, the
    // new head published from the version read. The store takes it; the check refuses its head's sequence.
    let mut prev = store.put(Codec::DagCbor, &encoded(&changed)).unwrap();
    for cid in &cids[3..] {
        let again = Entry { prev: Some(prev), ..read(&mut store, cid) };
        prev = store.put(Codec::DagCbor, &encoded(&again)).unwrap();
    }
    assert_eq!(store.publish(b"diary", 6, &prev, OWNER), Ok(7));
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Err(Refused { error: Error::Sequence(6), version: 7, at: 7, checked: 0 }));
}

#[test]
fn an_older_head_published_again_is_refused() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut b = buffer();
    let cids = journal(&mut store, "diary", 10);
    // The seventh entry made the head again, as a rewrite from an older point would: whole below it, refused at its head.
    assert_eq!(store.publish(b"diary", 10, &cids[6], OWNER), Ok(11));
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Err(Refused { error: Error::Sequence(7), version: 11, at: 11, checked: 0 }));
    // Nothing is appended after it.
    let content = store.put(Codec::Raw, b"after").unwrap();
    assert_eq!(append(&mut store, "diary", reading(11), "test", "note", &content, &mut b), Err(Error::Sequence(7)));
    assert_eq!(store.snapshot(b"diary"), Ok((11, Some(cids[6]))));
    // The last head published again: it too is not at its version.
    assert_eq!(store.publish(b"diary", 11, &cids[9], OWNER), Ok(12));
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Err(Refused { error: Error::Sequence(10), version: 12, at: 12, checked: 0 }));
}

#[test]
fn a_name_removed_and_published_again_is_refused() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut b = buffer();
    let cids = journal(&mut store, "diary", 5);
    assert_eq!(store.unpublish(b"diary", 5, OWNER), Ok(6));
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Err(Refused { error: Error::Removed(6), version: 6, at: 6, checked: 0 }));
    let content = store.put(Codec::Raw, b"again").unwrap();
    assert_eq!(append(&mut store, "diary", reading(6), "test", "note", &content, &mut b), Err(Error::Removed(6)));
    // A new journal's first entry under the name: not at its version.
    let first = store.put(Codec::DagCbor, &encoded(&entry(1, 1, 5, content, None))).unwrap();
    assert_eq!(store.publish(b"diary", 6, &first, OWNER), Ok(7));
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Err(Refused { error: Error::Sequence(1), version: 7, at: 7, checked: 0 }));
    // The old head again: neither.
    assert_eq!(store.publish(b"diary", 7, &cids[4], OWNER), Ok(8));
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Err(Refused { error: Error::Sequence(5), version: 8, at: 8, checked: 0 }));
    // Another name is not touched: a journal checks only under its own.
    journal(&mut store, "other", 2);
    assert_eq!(check(&mut store, "other", STEPS, &mut b), Ok(checked(2)));
    assert_eq!(check(&mut store, "never", STEPS, &mut b), Err(Refused { error: Error::NotFound, version: 0, at: 0, checked: 0 }));
}

/// Publishes entries made here, one after another under `name` from version 0; their CIDs.
fn chain(store: &mut Local, name: &str, times: &[(u64, u64)]) -> Vec<Cid> {
    let content = store.put(Codec::Raw, b"timed").unwrap();
    let mut cids: Vec<Cid> = Vec::new();
    for (k, &(boot, monotonic)) in times.iter().enumerate() {
        let e = entry(k as u64 + 1, boot, monotonic, content, cids.last().copied());
        let cid = store.put(Codec::DagCbor, &encoded(&e)).unwrap();
        assert_eq!(store.publish(name.as_bytes(), k as u64, &cid, OWNER), Ok(k as u64 + 1));
        cids.push(cid);
    }
    cids
}

#[test]
fn a_monotonic_time_that_goes_back_within_one_boot_is_refused() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut b = buffer();
    // The store takes any times: they are the check's.
    chain(&mut store, "back", &[(1, 100), (1, 200), (1, 150), (1, 300)]);
    assert_eq!(check(&mut store, "back", STEPS, &mut b), Err(Refused { error: Error::Time, version: 4, at: 2, checked: 2 }));
    // A boot that goes back.
    chain(&mut store, "boots", &[(1, 100), (2, 50), (1, 500)]);
    assert_eq!(check(&mut store, "boots", STEPS, &mut b), Err(Refused { error: Error::Time, version: 3, at: 2, checked: 1 }));
    // A new boot starts its clock again; one time twice within a boot is not going back.
    chain(&mut store, "fine", &[(1, 100), (1, 100), (2, 5), (2, 7), (4, 1)]);
    assert_eq!(check(&mut store, "fine", STEPS, &mut b), Ok(checked(5)));
    // An append counts a new boot when the head's monotonic time is past its own, and the journal checks.
    let content = store.put(Codec::Raw, b"after a reset").unwrap();
    let now = |monotonic| Reading { monotonic, calendar: 0, clock: Clock::None };
    let after = append(&mut store, "fine", now(0), "test", "note", &content, &mut b).unwrap();
    assert_eq!((after.entry.seq, after.entry.time.boot), (6, 5));
    let same = append(&mut store, "fine", now(9), "test", "note", &content, &mut b).unwrap();
    assert_eq!((same.entry.seq, same.entry.time.boot, same.entry.time.calendar), (7, 5, 0));
    assert_eq!(check(&mut store, "fine", STEPS, &mut b), Ok(checked(7)));
    // A calendar time without a clock, and labels that are none, are refused before anything is put.
    let blocks = store.stats().blocks;
    assert_eq!(append(&mut store, "fine", Reading { monotonic: 10, calendar: 5, clock: Clock::None }, "test", "note", &content, &mut b), Err(Error::Invalid));
    assert_eq!(append(&mut store, "fine", now(10), "a test", "note", &content, &mut b), Err(Error::Invalid));
    assert_eq!(append(&mut store, "fine", now(10), "test", "", &content, &mut b), Err(Error::Invalid));
    assert_eq!(store.stats().blocks, blocks);
}

/// The store, where another writer appends to `name` once this one has read the head, as it puts its entry.
struct Racing<'s, 'a> { store: &'s mut Local<'a>, name: &'static str, other: Option<Cid> }
impl journal::Store for Racing<'_, '_> {
    fn put(&mut self, entry: &[u8]) -> Result<Cid, Error> {
        if let Some(content) = self.other.take() { append(self.store, self.name, reading(50), "other", "note", &content, &mut buffer()).unwrap(); }
        journal::Store::put(self.store, entry)
    }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> { journal::Store::get(self.store, cid, out) }
    fn head(&mut self, name: &str) -> Result<(u64, Option<Cid>), Error> { journal::Store::head(self.store, name) }
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<u64, Error> { journal::Store::publish(self.store, name, expected, root) }
}

#[test]
fn of_two_writers_the_second_appends_after_reading_the_head_again() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut b = buffer();
    let cids = journal(&mut store, "diary", 3);
    let (mine, theirs) = (store.put(Codec::Raw, b"mine").unwrap(), store.put(Codec::Raw, b"theirs").unwrap());
    let blocks = store.stats().blocks;
    // Both read version 3; the other publishes first, and this writer is refused: nothing published.
    let mut racing = Racing { store: &mut store, name: "diary", other: Some(theirs) };
    assert_eq!(append(&mut racing, "diary", reading(51), "test", "note", &mine, &mut b), Err(Error::Conflict));
    let (version, head) = store.snapshot(b"diary").unwrap();
    let other = read(&mut store, &head.unwrap());
    assert_eq!((version, other.seq, other.source.as_str(), other.content, other.prev), (4, 4, "other", theirs, Some(cids[2])));
    // Appending again reads the new head and goes after it: no fork.
    let done = append(&mut store, "diary", reading(52), "test", "note", &mine, &mut b).unwrap();
    assert_eq!((done.entry.seq, done.entry.prev, done.entry.content), (5, head, mine));
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Ok(checked(5)));
    let mut sources = Vec::new();
    list(&mut store, "diary", 3, &mut b, |_, e, _| sources.push(e.source.as_str().to_owned())).unwrap();
    assert_eq!(sources, ["test", "other", "test"]);
    // The refused entry is retained by nothing: a collection frees it once its lease ends.
    assert_eq!(store.stats().blocks, blocks + 3);
    store.set_time(LEASE_NS + 1);
    assert_eq!(store.collect().unwrap().blocks, 1);
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Ok(checked(5)));
}

#[test]
fn a_journal_in_a_memory_domain_goes_with_the_domain() {
    let mut medium = Memory::new();
    let mut b = buffer();
    let domain;
    {
        let mut room = Room::new();
        let mut store = mount_with(&mut medium, &mut room, &RANDOM);
        journal(&mut store, "diary", 2);
        domain = store.domain_create(OWNER).unwrap();
        store.set_domain(domain).unwrap();
        journal(&mut store, "diary", 5);
        assert_eq!(check(&mut store, "diary", STEPS, &mut b), Ok(checked(5)));
        // The volume's journal of the same name is another one.
        store.set_domain(VOLUME).unwrap();
        assert_eq!(check(&mut store, "diary", STEPS, &mut b), Ok(checked(2)));
        assert_eq!(check(&mut store, "memo", STEPS, &mut b).map_err(|r| r.error), Err(Error::NotFound));
        store.set_domain(domain).unwrap();
        add(&mut store, "memo", 9, &mut b);
    }
    let mut room = Room::new();
    let mut store = mount_with(&mut medium, &mut room, &RANDOM);
    store.set_domain(domain).unwrap();
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Ok(checked(5)));
    assert_eq!(check(&mut store, "memo", STEPS, &mut b), Ok(checked(1)));
    // Forgotten: gone at once, its records then collected; the volume's journal whole.
    store.set_domain(VOLUME).unwrap();
    let blocks = store.stats().blocks;
    store.domain_forget(domain, OWNER).unwrap();
    assert_eq!(store.set_domain(domain), Err(store::Error::NotFound));
    assert_eq!(store.stats().blocks, blocks - 12);
    store.collect().unwrap();
    assert!(store.domains().next().is_none());
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Ok(checked(2)));
    drop(store);
    let mut room = Room::new();
    let mut store = mount_with(&mut medium, &mut room, &RANDOM);
    assert_eq!(store.set_domain(domain), Err(store::Error::NotFound));
    assert_eq!((store.stats().blocks, store.stats().corrupt), (4, 0));
    assert_eq!(check(&mut store, "diary", STEPS, &mut b), Ok(checked(2)));
}

/// The store with a stop: after `left` more calls, every call fails, as for a writer that ended there.
struct Stopping<'s, 'a> { store: &'s mut Local<'a>, left: usize }
impl Stopping<'_, '_> {
    fn step(&mut self) -> Result<(), Error> { if self.left == 0 { return Err(Error::Store); } self.left -= 1; Ok(()) }
    // The content's put, as the writer makes it before its append.
    fn content(&mut self, data: &[u8]) -> Result<Cid, Error> { self.step()?; self.store.put(Codec::Raw, data).map_err(|_| Error::Store) }
}
impl journal::Store for Stopping<'_, '_> {
    fn put(&mut self, entry: &[u8]) -> Result<Cid, Error> { self.step()?; journal::Store::put(self.store, entry) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> { self.step()?; journal::Store::get(self.store, cid, out) }
    fn head(&mut self, name: &str) -> Result<(u64, Option<Cid>), Error> { self.step()?; journal::Store::head(self.store, name) }
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<u64, Error> { self.step()?; journal::Store::publish(self.store, name, expected, root) }
}

#[test]
fn an_append_stopped_after_each_of_the_stores_calls_leaves_a_journal_that_checks() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut b = buffer();
    journal(&mut store, "diary", 3);
    // Stopped after 0, 1, ... calls: the content's put, the head, its entry, the entry's put, the publication.
    let mut calls = 0;
    loop {
        let mut stop = Stopping { store: &mut store, left: calls };
        let done = stop.content(b"stopped").and_then(|content| append(&mut stop, "diary", reading(9), "test", "note", &content, &mut b));
        let spare = stop.left;
        let version = store.snapshot(b"diary").unwrap().0;
        assert_eq!(check(&mut store, "diary", STEPS, &mut b), Ok(checked(version)), "after {calls} calls");
        match done {
            Err(_) => assert_eq!(version, 3, "after {calls} calls"),
            Ok(appended) => {
                assert_eq!((version, appended.entry.seq, spare), (4, 4, 0));
                assert_eq!(calls, 5);
                break;
            }
        }
        calls += 1;
    }
    assert_eq!(list(&mut store, "diary", 1, &mut b, |_, e, _| assert_eq!(e.content, Cid::raw(b"stopped"))).map(|c| c.entries), Ok(1));
}

#[test]
fn an_append_stopped_at_any_write_on_the_medium_leaves_the_old_head_or_the_new() {
    // Three entries; then an append on a medium that stops at its write `k` (not made, or `torn`: its first sector
    // alone). A new instance mounts the medium: the journal checks whole at version 3 or 4.
    let run = |stop: Option<(usize, bool)>| -> (usize, Result<Appended, Error>, Memory) {
        let mut medium = Memory::new();
        let (done, writes);
        {
            let mut room = Room::new();
            let mut store = mount(&mut medium, &mut room);
            let mut b = buffer();
            journal(&mut store, "diary", 3);
            let start = store.device().writes;
            if let Some((k, torn)) = stop { (store.device().stop, store.device().torn) = (Some(start + k), torn); }
            done = store.put(Codec::Raw, b"the fourth").map_err(|_| Error::Store)
                .and_then(|content| append(&mut store, "diary", reading(4), "test", "note", &content, &mut b));
            writes = store.device().writes - start;
        }
        medium.stop = None;
        (writes, done, medium)
    };
    let mounted = |mut medium: Memory| {
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let version = check(&mut store, "diary", STEPS, &mut buffer()).unwrap().version;
        assert_eq!(store.stats().corrupt, 0);
        version
    };
    // Not stopped: the content, the entry and the name's record.
    let (writes, done, medium) = run(None);
    assert_eq!((writes, done.map(|a| a.entry.seq), mounted(medium)), (3, Ok(4), 4));
    let mut newer = 0;
    for k in 1..=writes {
        for torn in [false, true] {
            let (_, done, medium) = run(Some((k, torn)));
            match mounted(medium) {
                3 => assert!(done.is_err(), "write {k}, torn {torn}"),
                4 => newer += 1,
                v => panic!("version {v} at write {k}, torn {torn}"),
            }
        }
    }
    // Only the name's record, written whole in its one sector before the medium stopped, publishes the entry.
    assert_eq!(newer, 1);
}

#[test]
fn the_stores_walk_follows_a_journal_one_entry_after_another_within_its_steps() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut b = buffer();
    // Longer than tree::DEPTH: the walk visits each entry, then its content, from the head down.
    let n = 3 * tree::DEPTH as u64;
    let cids = journal(&mut store, "diary", n);
    let mut visited = Vec::new();
    let total = journal::walk(&mut store, &cids[n as usize - 1], &mut b, 2 * n, |_, cid| { visited.push(*cid); Ok(()) }).unwrap();
    let expected: Vec<Cid> = (1..=n).rev().flat_map(|k| [cids[k as usize - 1], Cid::raw(&text(k))]).collect();
    assert_eq!((visited, total), (expected, (1..=n).map(|k| text(k).len() as u64).sum()));
    // Two steps an entry: one fewer stops it. Sizing takes one an entry, and the content's root.
    assert_eq!(journal::walk(&mut store, &cids[n as usize - 1], &mut b, 2 * n - 1, |_, _| Ok(())), Err(dag::Error::TooLarge));
    assert_eq!(journal::size(&mut store, &cids[n as usize - 1], &mut b, n), Ok(total));
    assert_eq!(journal::size(&mut store, &cids[n as usize - 1], &mut b, n - 1), Err(dag::Error::TooLarge));
    // Any other root walks as tree::walk walks it.
    let note = store.put(Codec::Raw, b"a file").unwrap();
    let mut seen = Vec::new();
    assert_eq!(journal::walk(&mut store, &note, &mut b, 1, |_, cid| { seen.push(*cid); Ok(()) }), Ok(6));
    assert_eq!(seen, [note]);
    // Out of shape, so not published (invalid): a link to an entry not one less in sequence, to a directory, a content
    // that is an entry.
    let content = Cid::raw(&text(1));
    let gap = store.put(Codec::DagCbor, &encoded(&entry(3, 1, 9, content, Some(cids[0])))).unwrap();
    let dir = store.put(Codec::DagCbor, b"\xa2\x61v\x01\x67entries\x80").unwrap();
    let to_dir = store.put(Codec::DagCbor, &encoded(&entry(2, 1, 9, content, Some(dir)))).unwrap();
    let nested = store.put(Codec::DagCbor, &encoded(&entry(1, 1, 9, cids[0], None))).unwrap();
    for (case, root) in [("a gap", gap), ("a directory before", to_dir), ("an entry as content", nested)] {
        assert_eq!(store.publish(b"bad", 0, &root, OWNER), Err(store::Error::Invalid), "{case}");
    }
    // An entry of another schema version is no block the store takes.
    let mut v2 = encoded(&entry(1, 1, 9, content, None));
    v2[3] = 2;
    assert_eq!(store.put(Codec::DagCbor, &v2), Err(store::Error::Invalid));
    // A content never stored: nothing published.
    let ghost = Cid::raw(b"never stored");
    assert_eq!(append(&mut store, "diary", reading(n + 1), "test", "note", &ghost, &mut b), Err(Error::Missing));
    assert_eq!(store.snapshot(b"diary").unwrap().0, n);
}

#[test]
fn a_journal_whose_walk_takes_more_steps_than_the_store_gives_is_refused() {
    // 64 slots: 56 blocks, so a walk takes at most 4 * 56 + 64 = 288 steps. Each entry's content is a directory of 100
    // files, one block named 100 times: about 202 steps an entry, so the first entry is published and the second not.
    let mut medium = Memory::new();
    let mut room = Room::with(64);
    let mut store = mount(&mut medium, &mut room);
    let mut b = buffer();
    let file = store.put(Codec::Raw, b"x").unwrap();
    let names: Vec<String> = (0..100).map(|k| format!("f{k:03}")).collect();
    let entries: Vec<tree::Entry> = names.iter().map(|name| tree::Entry { name: name.as_bytes(), kind: tree::Kind::File, size: 1, cid: file }).collect();
    let len = tree::encode(&entries, &mut b).unwrap();
    let dir = store.put(Codec::DagCbor, &b[..len]).unwrap();
    assert_eq!(append(&mut store, "big", reading(1), "test", "dir", &dir, &mut b).map(|a| a.entry.seq), Ok(1));
    assert_eq!(append(&mut store, "big", reading(2), "test", "dir", &dir, &mut b), Err(Error::Store));
    assert_eq!(store.snapshot(b"big").unwrap().0, 1);
    assert_eq!(check(&mut store, "big", STEPS, &mut b), Ok(checked(1)));
}

#[test]
fn a_copy_carries_a_journal_whole_and_a_store_it_comes_into_checks_it_at_its_own_versions() {
    let (mut buffer, mut block) = (buffer(), buffer());
    let mut first = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut first, &mut room);
    let cids = journal(&mut store, "diary", 20);
    let (copy, exported) = car::export(&mut store, &["diary"], Vec::new(), 1 << 20, &mut buffer, &mut block, |_, _, _| {}).unwrap();
    // Every entry and every content, in the walk's order, the copy's own check reading them all.
    assert_eq!(exported.blocks, 40);
    assert_eq!(car::check(&copy[..], &mut buffer, &mut block, |_| {}).map(|c| c.blocks), Ok(40));
    // A copy cut short of the first entry's content is refused.
    let cut = &copy[..copy.len() - text(1).len()];
    assert!(car::check(cut, &mut buffer, &mut block, |_| {}).is_err());
    // Into an empty store: the name at its version 1 there, every block whole, and the journal's head at 20: the check
    // refuses it as it refuses a head published again.
    let mut second = Memory::new();
    let mut room2 = Room::new();
    let mut other = mount(&mut second, &mut room2);
    let mut outcomes = Vec::new();
    car::import(&mut other, &copy[..], &mut buffer, &mut block, |_, outcome| outcomes.push(outcome)).unwrap();
    assert_eq!(outcomes, [car::Outcome::Published(1)]);
    assert!(cids.iter().all(|c| other.has(c)));
    assert_eq!(check(&mut other, "diary", STEPS, &mut buffer), Err(Refused { error: Error::Sequence(20), version: 1, at: 1, checked: 0 }));
    // Where it was written, it checks whole.
    assert_eq!(check(&mut store, "diary", STEPS, &mut buffer), Ok(checked(20)));
}

/// A store in memory that checks nothing, as one that lies may answer.
#[derive(Default)]
struct Loose { blocks: HashMap<Cid, Vec<u8>>, names: HashMap<String, (u64, Option<Cid>)> }
impl journal::Store for Loose {
    fn put(&mut self, entry: &[u8]) -> Result<Cid, Error> { let cid = Cid::of(Codec::DagCbor, entry); self.blocks.insert(cid, entry.to_vec()); Ok(cid) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> {
        let data = self.blocks.get(cid).ok_or(Error::Missing)?;
        out[..data.len()].copy_from_slice(data);
        Ok(data.len())
    }
    fn head(&mut self, name: &str) -> Result<(u64, Option<Cid>), Error> { Ok(self.names.get(name).copied().unwrap_or((0, None))) }
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<u64, Error> {
        if self.head(name)?.0 != expected { return Err(Error::Conflict); }
        self.names.insert(name.to_owned(), (expected + 1, Some(*root)));
        Ok(expected + 1)
    }
}
impl Loose {
    // Puts `bytes` under `codec` and makes them the head of `name` at `version`.
    fn head_at(&mut self, name: &str, version: u64, codec: Codec, bytes: &[u8]) -> Cid {
        let cid = Cid::of(codec, bytes);
        self.blocks.insert(cid, bytes.to_vec());
        self.names.insert(name.to_owned(), (version, Some(cid)));
        cid
    }
}

#[test]
fn the_check_trusts_no_store() {
    let mut loose = Loose::default();
    let mut b = buffer();
    let content = Cid::raw(b"c");
    let one = journal::Store::put(&mut loose, &encoded(&entry(1, 1, 1, content, None))).unwrap();
    let two = journal::Store::put(&mut loose, &encoded(&entry(2, 1, 2, content, Some(one)))).unwrap();
    // A gap: the third links the first.
    loose.head_at("gap", 3, Codec::DagCbor, &encoded(&entry(3, 1, 3, content, Some(one))));
    assert_eq!(check(&mut loose, "gap", STEPS, &mut b), Err(Refused { error: Error::Sequence(1), version: 3, at: 2, checked: 1 }));
    // An entry of another version below the head.
    let mut v2 = encoded(&entry(1, 1, 1, content, None));
    v2[3] = 2;
    let older = Cid::of(Codec::DagCbor, &v2);
    loose.blocks.insert(older, v2);
    loose.head_at("version", 2, Codec::DagCbor, &encoded(&entry(2, 1, 2, content, Some(older))));
    assert_eq!(check(&mut loose, "version", STEPS, &mut b), Err(Refused { error: Error::Version, version: 2, at: 1, checked: 1 }));
    // An entry's bytes under a raw CID, and a directory, are no entries.
    loose.head_at("raw", 1, Codec::Raw, &encoded(&entry(1, 1, 1, content, None)));
    assert_eq!(check(&mut loose, "raw", STEPS, &mut b).map_err(|r| r.error), Err(Error::Format));
    loose.head_at("dir", 1, Codec::DagCbor, b"\xa2\x61v\x01\x67entries\x80");
    assert_eq!(check(&mut loose, "dir", STEPS, &mut b).map_err(|r| r.error), Err(Error::Format));
    // Bytes that are not the block's.
    let three = loose.head_at("lies", 3, Codec::DagCbor, &encoded(&entry(3, 1, 3, content, Some(two))));
    loose.blocks.insert(two, encoded(&entry(2, 1, 7, content, Some(one))));
    assert_eq!(check(&mut loose, "lies", STEPS, &mut b), Err(Refused { error: Error::Corrupt, version: 3, at: 2, checked: 1 }));
    loose.blocks.insert(two, encoded(&entry(2, 1, 2, content, Some(one))));
    // Within its steps, or refused; a listing reads as many as asked.
    assert_eq!(check(&mut loose, "lies", 3, &mut b), Ok(checked(3)));
    assert_eq!(check(&mut loose, "lies", 2, &mut b), Err(Refused { error: Error::TooLarge, version: 3, at: 1, checked: 2 }));
    assert_eq!(check(&mut loose, "lies", 0, &mut b), Err(Refused { error: Error::TooLarge, version: 3, at: 3, checked: 0 }));
    let mut seen = Vec::new();
    assert_eq!(list(&mut loose, "lies", 2, &mut b, |_, e, cid| seen.push((e.seq, *cid))), Ok(Checked { version: 3, entries: 2, first: 2, last: 3 }));
    assert_eq!(seen, [(3, three), (2, two)]);
    // What the store keeps is never read past the first entry: a listing of more stops there.
    assert_eq!(list(&mut loose, "lies", 10, &mut b, |_, _, _| {}), Ok(checked(3)));
}
