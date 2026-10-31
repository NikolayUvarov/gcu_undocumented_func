//! Host tests of libmind/src/index.rs (issue 312-STO-0035; MC-4.5, 4.6, 4.12, 4.13, 10.3): a text index derived from a
//! journal, through the block store's own logic (blockstore/src/store.rs on a simulated medium). A manifest and a
//! postings file are byte for byte what the reference DAG-CBOR library encodes, and an index written here from the
//! layout has the root a build gives; the tokenizer is the rule written here. An index built twice from 240 entries of
//! seeded words has one root, also after a remount and a collection; every search answers what the exact scan and a
//! model answer, also once the index is stale, and says by how much; an index of another journal, of one rewritten or
//! of one that does not check is refused; an index changed by hand is refused by a search (a posting added) or told by
//! a check (a posting dropped); a manifest of another version, kind or tokenizer is refused as such; a journal past the
//! work area, or with a shard over a block, has no index, nothing published.
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
#[path = "../libmind/src/index.rs"]
mod index;
#[path = "../blockstore/src/store.rs"]
mod store;
#[path = "../libmind/src/worktree.rs"]
mod worktree;
#[path = "../libmind/src/car.rs"]
mod car;

use cid::{Cid, Codec};
use dag::{Blocks, CHUNK};
use index::{decode, Built, Error, Found, Indexed, Manifest, Name, Posting, State, Term, Verdict, Work, MANIFEST_MAX};
use journal::{append, Appended, Clock, Reading, Refused};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use store::{hmac_sha256, Crypto, Device, Extent, Head, Pin, Store, BUFFER, LEASE_NS, SECTOR, VOLUME};

const SECTORS: usize = 16384;

/// A medium in memory.
struct Memory { data: Vec<u8> }
impl Memory { fn new() -> Self { Memory { data: vec![0; SECTORS * SECTOR] } } }
impl Device for &mut Memory {
    fn sectors(&self) -> u64 { (self.data.len() / SECTOR) as u64 }
    fn writable(&self) -> bool { true }
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool { let at = lba as usize * SECTOR; out.copy_from_slice(&self.data[at..at + out.len()]); true }
    fn write(&mut self, lba: u64, data: &[u8]) -> bool { let at = lba as usize * SECTOR; self.data[at..at + data.len()].copy_from_slice(data); true }
    fn flush(&mut self) -> bool { true }
}

/// Records sealed by a tag alone, their bytes left plain, so a test finds an entry on the medium: an index needs the
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
    fn new() -> Self {
        Room { index: vec![store::Entry::EMPTY; store::slots_for(SECTORS as u64)], heads: vec![Head::EMPTY; 16], pins: vec![Pin::EMPTY; 4], holes: vec![Extent::EMPTY; 64], buffer: Box::new([0; BUFFER]), scratch: Box::new([0; CHUNK]) }
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
const INDEX: &str = "index/diary";

/// The store as a journal and an index see it, for one owner.
impl<D: Device> journal::Store for Store<'_, D> {
    fn put(&mut self, entry: &[u8]) -> Result<Cid, journal::Error> { Store::put(self, Codec::DagCbor, entry).map_err(|_| journal::Error::Store) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, journal::Error> {
        Store::get(self, cid, out).map_err(|e| match e { store::Error::NotFound => journal::Error::Missing, store::Error::Corrupt => journal::Error::Corrupt, _ => journal::Error::Store })
    }
    fn head(&mut self, name: &str) -> Result<(u64, Option<Cid>), journal::Error> { Store::snapshot(self, name.as_bytes()).map_err(|_| journal::Error::Store) }
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<u64, journal::Error> {
        Store::publish(self, name.as_bytes(), expected, root, OWNER).map_err(|e| match e { store::Error::Conflict => journal::Error::Conflict, store::Error::Incomplete => journal::Error::Missing, _ => journal::Error::Store })
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

/// A work area of `terms` terms, `text` bytes of them and `postings` postings, and its three blocks.
struct Area { terms: Vec<Term>, text: Vec<u8>, postings: Vec<Posting>, blocks: Box<[[u8; CHUNK]; 3]> }
impl Area {
    fn new(terms: usize, text: usize, postings: usize) -> Self {
        Area { terms: vec![Term::EMPTY; terms], text: vec![0; text], postings: vec![Posting::EMPTY; postings], blocks: Box::new([[0; CHUNK]; 3]) }
    }
    fn work(&mut self) -> Work<'_> { Work::new(&mut self.terms, &mut self.text, &mut self.postings, &mut self.blocks) }
}
/// The work area `blocks` keeps.
fn area() -> Area { Area::new(4096, 64 * 1024, 32 * 1024) }

fn build(store: &mut Local, journal: &str, name: &str, area: &mut Area) -> Result<Built, Error> { index::build(store, journal, name, STEPS, &mut area.work()) }
fn state(store: &mut Local, journal: &str, name: &str, area: &mut Area) -> Result<(Indexed, State), Error> { index::state(store, journal, name, STEPS, &mut area.work()) }
fn verify(store: &mut Local, journal: &str, name: &str, area: &mut Area) -> Result<Verdict, Error> { index::verify(store, journal, name, STEPS, &mut area.work()).map(|(_, v)| v) }
fn search(store: &mut Local, journal: &str, name: &str, word: &str, area: &mut Area) -> Result<(Vec<u64>, Found), Error> {
    let mut found = Vec::new();
    index::search(store, journal, name, word, STEPS, &mut area.work(), |seq| found.push(seq)).map(|f| (found, f))
}
fn scan(store: &mut Local, journal: &str, word: &str, area: &mut Area) -> Result<Vec<u64>, Error> {
    let mut found = Vec::new();
    let n = index::scan(store, journal, word, STEPS, &mut area.work(), |seq| found.push(seq))?;
    assert_eq!(n as usize, found.len());
    Ok(found)
}

/// The terms of `text` as tokenizer version 1 makes them, written here from its rule: runs of ASCII letters and digits,
/// lowercased, cut into pieces of 32.
fn tokenized(text: &[u8]) -> Vec<Vec<u8>> {
    text.split(|c| !c.is_ascii_alphanumeric()).filter(|run| !run.is_empty()).flat_map(|run| run.chunks(32).map(|p| p.to_ascii_lowercase())).collect()
}

/// What a journal's contents hold: entry k's text at k - 1, none for a directory.
#[derive(Clone, Default)]
struct Model { contents: Vec<Option<Vec<u8>>> }
impl Model {
    fn terms(&self, seq: u64) -> BTreeSet<Vec<u8>> { self.contents[seq as usize - 1].as_deref().map(tokenized).unwrap_or_default().into_iter().collect() }
    /// Each term of the entries up to `upto`, with the sequences of those that hold it, ascending.
    fn postings(&self, upto: u64) -> BTreeMap<Vec<u8>, Vec<u64>> {
        let mut postings: BTreeMap<Vec<u8>, Vec<u64>> = BTreeMap::new();
        for seq in 1..=upto { for term in self.terms(seq) { postings.entry(term).or_default().push(seq); } }
        postings
    }
    /// The entries up to `upto` that hold a term.
    fn entries(&self, upto: u64) -> u64 { (1..=upto).filter(|&s| !self.terms(s).is_empty()).count() as u64 }
    /// The entries whose content holds every term of `word`, newest first.
    fn answer(&self, word: &str) -> Vec<u64> {
        let query: BTreeSet<Vec<u8>> = tokenized(word.as_bytes()).into_iter().collect();
        (1..=self.contents.len() as u64).rev().filter(|&s| query.is_subset(&self.terms(s))).collect()
    }
}

/// A seeded generator (xorshift64), so every run writes the same journals.
struct Seeded(u64);
impl Seeded {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn below(&mut self, n: usize) -> usize { (self.next() % n as u64) as usize }
}

/// A journal of seeded texts under a name: 120 words of 1 to 9 letters and digits, a run longer than a term, and words
/// with bytes beyond ASCII, which separate; each text 3 to 12 of them, some in capitals, between spaces, punctuation and
/// a no-break space.
struct Diary { name: &'static str, rng: Seeded, words: Vec<Vec<u8>>, cids: Vec<Cid>, model: Model }
impl Diary {
    fn new(name: &'static str, seed: u64) -> Self {
        let mut rng = Seeded(seed);
        let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789";
        let mut words: Vec<Vec<u8>> = (0..120).map(|_| { let len = 1 + rng.below(9); (0..len).map(|_| alphabet[rng.below(36)]).collect() }).collect();
        words.push(b"supercalifragilisticexpialidociousandmoreofit".to_vec());
        words.extend(["naïve", "Grüße", "café"].map(|w| w.as_bytes().to_vec()));
        Diary { name, rng, words, cids: Vec::new(), model: Model::default() }
    }
    fn text(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        for k in 0..3 + self.rng.below(10) {
            if k > 0 { out.extend_from_slice([&b" "[..], b", ", b"-", b"\n", b". ", "\u{a0}".as_bytes(), b"_"][self.rng.below(7)]); }
            let word = self.words[self.rng.below(self.words.len())].clone();
            match self.rng.below(5) {
                0 => out.extend(word.to_ascii_uppercase()),
                1 => { out.push(word[0].to_ascii_uppercase()); out.extend(&word[1..]) }
                _ => out.extend(word),
            }
        }
        out
    }
    /// Appends `n` entries of seeded texts.
    fn append(&mut self, store: &mut Local, n: usize) {
        for _ in 0..n { let text = self.text(); self.add(store, Some(text)); }
    }
    /// Appends one entry: a text, a file of one block or an object; or none, an empty directory.
    fn add(&mut self, store: &mut Local, text: Option<Vec<u8>>) -> Appended {
        let content = match &text {
            Some(t) if t.len() <= CHUNK => store.put(Codec::Raw, t).unwrap(),
            Some(t) => { let mut b = Box::new(dag::Builder::new()); b.write(store, t).unwrap(); b.finish(store).unwrap().0 }
            None => store.put(Codec::DagCbor, b"\xa2\x61v\x01\x67entries\x80").unwrap(),
        };
        self.add_content(store, &content, text)
    }
    fn add_content(&mut self, store: &mut Local, content: &Cid, text: Option<Vec<u8>>) -> Appended {
        let k = self.cids.len() as u64 + 1;
        let now = Reading { monotonic: 1_000_000_000 + k * 1_000_000, calendar: 1_798_000_000 + k, clock: Clock::Chip };
        let done = append(store, self.name, now, "test", "note", content, &mut buffer()).unwrap();
        assert_eq!(done.entry.seq, k);
        self.cids.push(done.cid);
        self.model.contents.push(text);
        done
    }
    fn head(&self) -> (u64, Cid) { (self.cids.len() as u64, *self.cids.last().unwrap()) }
}

/// A value of a manifest's field, written here as DAG-CBOR: a number, a text, a link.
#[derive(Clone, Copy)]
enum V<'a> { U(u64), T(&'a str), L(Cid) }

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
fn cbor_link(out: &mut Vec<u8>, cid: &Cid) { out.extend(unhex("d82a582500")); out.extend(cid.to_bytes()); }

/// A map of `fields` in the order given, written here from the DAG-CBOR specification, not by index.rs.
fn written(fields: &[(&str, V)]) -> Vec<u8> {
    let mut out = Vec::new();
    cbor_head(&mut out, 5, fields.len() as u64);
    for (key, value) in fields {
        cbor_head(&mut out, 3, key.len() as u64);
        out.extend(key.as_bytes());
        match value {
            V::U(n) => cbor_head(&mut out, 0, *n),
            V::T(t) => { cbor_head(&mut out, 3, t.len() as u64); out.extend(t.as_bytes()) }
            V::L(c) => cbor_link(&mut out, c),
        }
    }
    out
}

/// A manifest's fields in the schema's order.
fn fields(journal: &str, seq: u64, head: Cid, entries: u64, terms: u64, postings: u64) -> Vec<(&str, V<'_>)> {
    vec![("v", V::U(1)), ("seq", V::U(seq)), ("head", V::L(head)), ("kind", V::T("text")), ("terms", V::U(terms)), ("entries", V::U(entries)),
         ("journal", V::T(journal)), ("postings", V::U(postings)), ("tokenizer", V::U(1))]
}

/// A postings file written here from the layout: an array of the sequences.
fn postings_file(seqs: &[u64]) -> Vec<u8> {
    let mut out = Vec::new();
    cbor_head(&mut out, 4, seqs.len() as u64);
    for &s in seqs { cbor_head(&mut out, 0, s); }
    out
}

/// A directory node written here from `tree`'s schema: entries (name, kind, size, CID) in the order given.
fn directory(entries: &[(Vec<u8>, u64, u64, Cid)]) -> Vec<u8> {
    let mut out = Vec::new();
    cbor_head(&mut out, 5, 2);
    out.extend(b"\x61v\x01\x67entries");
    cbor_head(&mut out, 4, entries.len() as u64);
    for (name, kind, size, cid) in entries {
        out.push(0x84);
        cbor_head(&mut out, 3, name.len() as u64);
        out.extend(name);
        cbor_head(&mut out, 0, *kind);
        cbor_head(&mut out, 0, *size);
        cbor_link(&mut out, cid);
    }
    out
}

/// An index written here from the layout into the store, not by index.rs: the root holds `manifest` and `terms`, and
/// under `terms` each term's file (`files`) is at `<first byte>/<first two>/<term>`, every directory sorted by name.
fn layout(store: &mut Local, manifest: &[u8], files: &BTreeMap<Vec<u8>, Vec<u8>>) -> Cid {
    let mut tree: BTreeMap<Vec<u8>, BTreeMap<Vec<u8>, Vec<(Vec<u8>, u64, u64, Cid)>>> = BTreeMap::new();
    for (term, file) in files {
        let cid = store.put(Codec::Raw, file).unwrap();
        tree.entry(term[..1].to_vec()).or_default().entry(term[..term.len().min(2)].to_vec()).or_default().push((term.clone(), 0, file.len() as u64, cid));
    }
    let file = store.put(Codec::Raw, manifest).unwrap();
    let mut put = |entries: &[(Vec<u8>, u64, u64, Cid)]| store.put(Codec::DagCbor, &directory(entries)).unwrap();
    let firsts: Vec<_> = tree.iter().map(|(first, shards)| {
        let shards: Vec<_> = shards.iter().map(|(key, terms)| (key.clone(), 1, terms.len() as u64, put(terms))).collect();
        (first.clone(), 1, shards.len() as u64, put(&shards))
    }).collect();
    let terms = put(&firsts);
    put(&[(b"manifest".to_vec(), 0, manifest.len() as u64, file), (b"terms".to_vec(), 1, firsts.len() as u64, terms)])
}

/// The postings files of `postings`, as the layout has them.
fn files(postings: &BTreeMap<Vec<u8>, Vec<u64>>) -> BTreeMap<Vec<u8>, Vec<u8>> { postings.iter().map(|(t, s)| (t.clone(), postings_file(s))).collect() }

/// The index of `diary` up to `seq`, written here from the layout and the model; its root.
fn expected(store: &mut Local, diary: &Diary, seq: u64) -> Cid {
    let postings = diary.model.postings(seq);
    let count = postings.values().map(|s| s.len() as u64).sum();
    let manifest = written(&fields(diary.name, seq, diary.cids[seq as usize - 1], diary.model.entries(seq), postings.len() as u64, count));
    layout(store, &manifest, &files(&postings))
}

/// Publishes `root` under `name` after its current version.
fn publish(store: &mut Local, name: &str, root: &Cid) -> u64 {
    let version = store.snapshot(name.as_bytes()).unwrap().0;
    store.publish(name.as_bytes(), version, root, OWNER).unwrap()
}

#[test]
fn a_manifest_and_a_postings_file_are_the_reference_encodings() {
    // As the reference library dag-cbor 0.3.3 (with multiformats 0.3.1) encodes them, keys sorted by it.
    let head = Cid::of(Codec::DagCbor, b"an entry");
    let m = Manifest { journal: Name::new("diary").unwrap(), seq: 300, head, entries: 250, terms: 1000, postings: 70_000 };
    let mut out = [0u8; MANIFEST_MAX];
    let n = m.encode(&mut out).unwrap();
    assert_eq!(hex(&out[..n]), "a96176016373657119012c6468656164d82a582500017112209fb7f2ffa7e75ba5d5017c2b5b651a7d256016ca891c74ca01cb651d6781a6e9646b696e646474657874657465726d731903e867656e747269657318fa676a6f75726e616c65646961727968706f7374696e67731a0001117069746f6b656e697a657201");
    assert_eq!(Cid::raw(&out[..n]).to_string(), "bafkreibuellqqgdwpl2mh6mhatursg5g5mz32xp6euv2jkslwkn24fx7lu");
    assert_eq!(out[..n], written(&fields("diary", 300, head, 250, 1000, 70_000))[..]);
    assert_eq!(decode(&out[..n]), Ok(m));
    // The largest fits MANIFEST_MAX exactly.
    let long = "j".repeat(car::NAME_MAX);
    let max = Manifest { journal: Name::new(&long).unwrap(), seq: u64::MAX, head, entries: u64::MAX, terms: u64::MAX, postings: u64::MAX };
    assert_eq!(max.encode(&mut out), Ok(MANIFEST_MAX));
    assert_eq!(decode(&out), Ok(max));
    // The writers here against the reference: a postings file, and a small index whole, terms "a" (entry 1), "ab"
    // (entries 1 and 2) and "b7" (entry 2), of journal "notes" at sequence 2.
    let seqs = [1, 23, 24, 255, 256, 65_535, 65_536, (1 << 32) - 1];
    assert_eq!(hex(&postings_file(&seqs)), "880117181818ff19010019ffff1a000100001affffffff");
    assert_eq!(Cid::raw(&postings_file(&seqs)).to_string(), "bafkreigzh5xavzypnhwp7h2ntolm5pajifo7p63e4vlb54qpmznm2gnx6e");
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let small = written(&fields("notes", 2, head, 2, 3, 4));
    assert_eq!(hex(&small), "a961760163736571026468656164d82a582500017112209fb7f2ffa7e75ba5d5017c2b5b651a7d256016ca891c74ca01cb651d6781a6e9646b696e646474657874657465726d730367656e747269657302676a6f75726e616c656e6f74657368706f7374696e67730469746f6b656e697a657201");
    let postings = BTreeMap::from([(b"a".to_vec(), vec![1]), (b"ab".to_vec(), vec![1, 2]), (b"b7".to_vec(), vec![2])]);
    assert_eq!(layout(&mut store, &small, &files(&postings)).to_string(), "bafyreihijwwbwofecazf7xp6rclcebyscc7rlpt6wm2zayqoa4f4rubtiq");
    // A build of a journal holding those terms writes that layout: the root written here, its head the journal's.
    let mut notes = Diary::new("notes", 1);
    notes.add(&mut store, Some(b"A ab".to_vec()));
    notes.add(&mut store, Some(b"AB, b7!".to_vec()));
    assert_eq!(notes.model.postings(2), postings);
    let built = build(&mut store, "notes", "index/notes", &mut area()).unwrap();
    assert_eq!(built.index.manifest, Manifest { journal: Name::new("notes").unwrap(), seq: 2, head: notes.cids[1], entries: 2, terms: 3, postings: 4 });
    assert_eq!(built.index.root, expected(&mut store, &notes, 2));
}

#[test]
fn only_the_canonical_manifest_decodes_and_another_version_kind_or_tokenizer_is_refused_by_it() {
    let head = Cid::of(Codec::DagCbor, b"an entry");
    let f = fields("diary", 300, head, 250, 1000, 70_000);
    let bytes = written(&f);
    // Every byte changed to every other value: refused, or read as exactly what it encodes.
    let mut read = 0;
    for at in 0..bytes.len() {
        for value in 0..=255u8 {
            if value == bytes[at] { continue; }
            let mut changed = bytes.clone();
            changed[at] = value;
            if let Ok(other) = decode(&changed) {
                let mut out = [0u8; MANIFEST_MAX];
                let n = other.encode(&mut out).unwrap();
                assert_eq!(out[..n], changed[..], "byte {at} as {value:#x}");
                read += 1;
            }
        }
    }
    assert!(read > 0);
    for len in 0..bytes.len() { assert!(decode(&bytes[..len]).is_err(), "cut to {len}"); }
    let with = |key: &str, value: V| -> Vec<u8> { written(&f.iter().map(|&(k, v)| if k == key { (k, value) } else { (k, v) }).collect::<Vec<_>>()) };
    let without = |key: &str| -> Vec<u8> { written(&f.iter().filter(|(k, _)| *k != key).copied().collect::<Vec<_>>()) };
    let mut swapped = f.clone();
    swapped.swap(2, 3);
    let mut extra = f.clone();
    extra.push(("zzzzzzzzzz", V::U(0)));
    let mut longer = bytes.clone();
    let at = longer.windows(4).position(|w| w == b"\x63seq").unwrap() + 4;
    longer.splice(at..at + 3, [0x1a, 0, 0, 1, 0x2c]);
    for (case, bytes, error) in [
        ("version 2", with("v", V::U(2)), Error::Version(2)),
        ("version 0", with("v", V::U(0)), Error::Version(0)),
        ("another kind", with("kind", V::T("vector")), Error::Kind),
        ("tokenizer 2", with("tokenizer", V::U(2)), Error::Tokenizer(2)),
        ("tokenizer 0", with("tokenizer", V::U(0)), Error::Tokenizer(0)),
        ("keys out of order", written(&swapped), Error::Format),
        ("an extra key", written(&extra), Error::Format),
        ("no head", without("head"), Error::Format),
        ("no journal", without("journal"), Error::Format),
        ("a head of raw", with("head", V::L(Cid::raw(b"an entry"))), Error::Format),
        ("sequence 0", with("seq", V::U(0)), Error::Format),
        ("more entries than the sequence", with("entries", V::U(301)), Error::Format),
        ("a journal's name the store does not take", with("journal", V::T("a diary")), Error::Format),
        ("an empty journal's name", with("journal", V::T("")), Error::Format),
        ("a number written longer", longer, Error::Format),
    ] {
        assert_eq!(decode(&bytes), Err(error), "{case}");
    }
    // A manifest that makes none is not encoded.
    let m = decode(&bytes).unwrap();
    let mut out = [0u8; MANIFEST_MAX];
    for bad in [Manifest { seq: 0, ..m }, Manifest { head: Cid::raw(b"x"), ..m }, Manifest { entries: 301, ..m }] { assert_eq!(bad.encode(&mut out), Err(Error::Format)); }
    // Each refused as such by a search, a state and a check of an index whose manifest it is.
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut diary = Diary::new("diary", 3);
    diary.append(&mut store, 5);
    let mut a = area();
    build(&mut store, "diary", INDEX, &mut a).unwrap();
    let postings = diary.model.postings(5);
    let count = postings.values().map(|s| s.len() as u64).sum();
    let good = fields("diary", 5, diary.cids[4], diary.model.entries(5), postings.len() as u64, count);
    let word = std::str::from_utf8(postings.keys().next().unwrap()).unwrap().to_owned();
    for (key, value, error) in [("v", V::U(2), Error::Version(2)), ("kind", V::T("vector"), Error::Kind), ("tokenizer", V::U(2), Error::Tokenizer(2))] {
        let manifest = written(&good.iter().map(|&(k, v)| if k == key { (k, value) } else { (k, v) }).collect::<Vec<_>>());
        let root = layout(&mut store, &manifest, &files(&postings));
        publish(&mut store, INDEX, &root);
        assert_eq!(search(&mut store, "diary", INDEX, &word, &mut a).map(|r| r.0), Err(error), "{key}");
        assert_eq!(state(&mut store, "diary", INDEX, &mut a).map(|s| s.1), Err(error), "{key}");
        assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Err(error), "{key}");
    }
    // Built again, it is this module's.
    assert!(build(&mut store, "diary", INDEX, &mut a).unwrap().published);
    assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Ok(Verdict::Equal));
}

#[test]
fn the_tokenizer_is_version_1() {
    let terms = |text: &[u8]| { let mut out = Vec::new(); index::terms(text, |t| out.push(t.to_vec())); out };
    let strings = |terms: Vec<Vec<u8>>| terms.into_iter().map(|t| String::from_utf8(t).unwrap()).collect::<Vec<_>>();
    assert_eq!(strings(terms(b"Hello, World! abc123 X_y-Z")), ["hello", "world", "abc123", "x", "y", "z"]);
    // Beyond ASCII separates: no term holds it.
    assert_eq!(strings(terms("naïve Grüße café ÉTÉ".as_bytes())), ["na", "ve", "gr", "e", "caf", "t"]);
    // A run longer than a term is cut into pieces of 32.
    let run = "A".repeat(70);
    assert_eq!(strings(terms(run.as_bytes())), ["a".repeat(32), "a".repeat(32), "a".repeat(6)]);
    assert_eq!(strings(terms("b".repeat(64).as_bytes())), ["b".repeat(32), "b".repeat(32)]);
    assert!(terms(b"").is_empty() && terms(b" .,;\n\t\x00\xff").is_empty());
    // Every byte, in runs of every length to 100: as the rule written here makes them.
    let mut rng = Seeded(7);
    for len in 0..2000 {
        let text: Vec<u8> = (0..len % 101 + len / 101).map(|_| match rng.below(4) { 0 => rng.below(256) as u8, 1 => b' ', _ => b"aZ09qQ"[rng.below(6)] }).collect();
        assert_eq!(terms(&text), tokenized(&text), "{text:?}");
    }
    for byte in 0..=255u8 { assert_eq!(terms(&[b'x', byte, b'y']), tokenized(&[b'x', byte, b'y']), "{byte:#x}"); }
}

#[test]
fn an_index_is_built_twice_to_one_root_also_after_a_remount_and_a_collection() {
    let mut medium = Memory::new();
    let mut a = area();
    let mut diary = Diary::new("diary", 11);
    let root;
    {
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        diary.append(&mut store, 240);
        let built = build(&mut store, "diary", INDEX, &mut a).unwrap();
        let postings = diary.model.postings(240);
        let m = built.index.manifest;
        assert_eq!((m.journal.as_str(), m.seq, m.head, m.entries), ("diary", 240, diary.cids[239], diary.model.entries(240)));
        assert_eq!((m.terms, m.postings), (postings.len() as u64, postings.values().map(|s| s.len() as u64).sum()));
        assert!(m.terms > 100 && m.postings > 1000, "{m:?}");
        assert_eq!((built.index.version, built.published), (1, true));
        // The root of the index written here from the layout.
        root = built.index.root;
        assert_eq!(expected(&mut store, &diary, 240), root);
        // Built again: the same root, and nothing published again.
        assert_eq!(build(&mut store, "diary", INDEX, &mut a), Ok(Built { index: built.index, published: false }));
        assert_eq!(store.snapshot(INDEX.as_bytes()), Ok((1, Some(root))));
        assert_eq!(state(&mut store, "diary", INDEX, &mut a), Ok((built.index, State::Current)));
        assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Ok(Verdict::Equal));
    }
    // A new instance mounts the medium: the same root again.
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let again = build(&mut store, "diary", INDEX, &mut a).unwrap();
    assert_eq!((again.index.root, again.index.version, again.published), (root, 1, false));
    // Once every lease has ended a collection frees what nothing retains, a block put loose, and keeps every block of the
    // index, a tree its name retains: it checks equal, and a build gives its root.
    let loose = store.put(Codec::Raw, b"in no journal").unwrap();
    store.set_time(LEASE_NS + 1);
    assert_eq!(store.collect().unwrap().blocks, 1);
    assert!(!store.has(&loose));
    let mut held = 0;
    tree::walk(&mut store, &root, &mut buffer(), u64::MAX, |s, cid| { assert!(Blocks::has(s, cid).unwrap()); held += 1; Ok(()) }).unwrap();
    assert!(held > 200, "{held}");
    assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Ok(Verdict::Equal));
    assert_eq!(build(&mut store, "diary", INDEX, &mut a).map(|b| (b.index.root, b.published)), Ok((root, false)));
    // An index is charged to its owner as any tree is: the bytes of its files.
    assert!(store.usage(OWNER).retained > tree::size(&mut store, &root, &mut buffer(), u64::MAX).unwrap());
}

#[test]
fn every_search_answers_what_the_exact_scan_and_the_model_answer() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut a = area();
    let mut diary = Diary::new("diary", 23);
    diary.append(&mut store, 220);
    build(&mut store, "diary", INDEX, &mut a).unwrap();
    // Each term the journal holds, in its words' cases and with a byte beyond ASCII before it; words it does not hold;
    // words of several terms.
    let held: Vec<String> = diary.model.postings(220).keys().map(|t| String::from_utf8(t.clone()).unwrap()).collect();
    let mut words: Vec<String> = held.iter().flat_map(|t| [t.clone(), t.to_uppercase(), format!("é{t}")]).collect();
    words.extend(["absentword", "zzzzzzzzz", "q1w2e3r4", "supercalifragilisticexpialidociousandmoreofit", "naïve", "Supercalifragilisticexpialidocious"].map(String::from));
    words.extend(held.windows(2).step_by(7).map(|w| format!("{}-{}", w[0], w[1])));
    words.extend(held.windows(3).step_by(11).map(|w| format!("{}.{},{}", w[0], w[1], w[2])));
    let (mut answered, mut none) = (0, 0);
    for word in &words {
        let (found, f) = search(&mut store, "diary", INDEX, word, &mut a).unwrap();
        assert_eq!(f.state, State::Current, "{word}");
        assert_eq!(found, diary.model.answer(word), "{word}");
        assert_eq!(found, scan(&mut store, "diary", word, &mut a).unwrap(), "{word}");
        assert_eq!(f.entries, found.len() as u64);
        if found.is_empty() { none += 1 } else { answered += 1 }
    }
    assert!(answered > 300 && none >= 4, "{answered} {none}");
    // A word that makes no term, or more than WORD, is refused; WORD of them are one word.
    for word in ["", "!!!", "é", "a-b-c-d-e-f-g-h-i"] {
        assert_eq!(search(&mut store, "diary", INDEX, word, &mut a).map(|r| r.0), Err(Error::Word), "{word:?}");
        assert_eq!(scan(&mut store, "diary", word, &mut a), Err(Error::Word), "{word:?}");
    }
    let eight = held[..8].join("+");
    assert_eq!(search(&mut store, "diary", INDEX, &eight, &mut a).unwrap().0, diary.model.answer(&eight));
    // No index under the name, or one removed.
    assert_eq!(search(&mut store, "diary", "index/none", "x", &mut a).map(|r| r.0), Err(Error::NotFound));
    let version = store.snapshot(INDEX.as_bytes()).unwrap().0;
    assert_eq!(store.unpublish(INDEX.as_bytes(), version, OWNER), Ok(version + 1));
    assert_eq!(search(&mut store, "diary", INDEX, &held[0], &mut a).map(|r| r.0), Err(Error::Removed(version + 1)));
    assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Err(Error::Removed(version + 1)));
}

#[test]
fn after_appends_the_index_is_told_stale_by_how_many_and_every_search_still_answers_the_scan() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut a = area();
    let mut diary = Diary::new("diary", 31);
    diary.append(&mut store, 200);
    let built = build(&mut store, "diary", INDEX, &mut a).unwrap();
    // Seven more, one with a word no entry held before, one a directory, one an object of two chunks.
    diary.append(&mut store, 4);
    diary.add(&mut store, Some(b"a brandnewword here".to_vec()));
    diary.add(&mut store, None);
    let mut big = b"x ".repeat(9000);
    big.extend(b"brandnewword in a long one");
    diary.add(&mut store, Some(big));
    assert_eq!(state(&mut store, "diary", INDEX, &mut a), Ok((built.index, State::Stale(7))));
    let mut words: Vec<String> = diary.model.postings(207).keys().map(|t| String::from_utf8(t.clone()).unwrap()).collect();
    words.push("nowhere".into());
    let mut scanned = 0;
    for word in &words {
        let (found, f) = search(&mut store, "diary", INDEX, word, &mut a).unwrap();
        assert_eq!((f.state, f.index), (State::Stale(7), built.index), "{word}");
        assert_eq!(found, diary.model.answer(word), "{word}");
        assert_eq!(found, scan(&mut store, "diary", word, &mut a).unwrap(), "{word}");
        scanned += found.iter().filter(|&&s| s > 200).count();
    }
    assert!(scanned > 10, "{scanned}");
    assert_eq!(search(&mut store, "diary", INDEX, "BrandNewWord", &mut a).unwrap().0, [207, 205]);
    // The check finds it equal to its rebuild from its own head, the journal grown by seven.
    assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Ok(Verdict::Stale(7)));
    // Rebuilt: current, at the next version, its root the one written here.
    let rebuilt = build(&mut store, "diary", INDEX, &mut a).unwrap();
    assert_eq!((rebuilt.index.version, rebuilt.published, rebuilt.index.manifest.seq), (2, true, 207));
    assert_eq!(rebuilt.index.root, expected(&mut store, &diary, 207));
    assert_eq!(state(&mut store, "diary", INDEX, &mut a).map(|s| s.1), Ok(State::Current));
    assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Ok(Verdict::Equal));
    assert_eq!(search(&mut store, "diary", INDEX, "brandnewword", &mut a).map(|r| (r.0, r.1.state)), Ok((vec![207, 205], State::Current)));
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
fn an_index_of_another_journal_of_one_rewritten_or_of_one_that_does_not_check_is_refused() {
    let mut a = area();
    let (mut first, mut second, mut third) = (Memory::new(), Memory::new(), Memory::new());
    let (mut r1, mut r2, mut r3) = (Room::new(), Room::new(), Room::new());
    let mut store = mount(&mut first, &mut r1);
    let mut diary = Diary::new("diary", 41);
    diary.append(&mut store, 120);
    let mut other = Diary::new("other", 42);
    other.append(&mut store, 130);
    let built = build(&mut store, "diary", INDEX, &mut a).unwrap();
    // Asked of another journal: its manifest names this one.
    let word = std::str::from_utf8(diary.model.postings(120).keys().next().unwrap()).unwrap().to_owned();
    assert_eq!(search(&mut store, "other", INDEX, &word, &mut a).map(|r| r.0), Err(Error::Another));
    assert_eq!(state(&mut store, "other", INDEX, &mut a).map(|s| s.1), Err(Error::Another));
    assert_eq!(verify(&mut store, "other", INDEX, &mut a), Err(Error::Another));
    // The same entries under another name, each published at its own version: that journal checks whole, and still the
    // index is not answered for it, its manifest naming another. Its own index differs by that name alone.
    for (k, cid) in diary.cids.iter().enumerate() { assert_eq!(store.publish(b"copy", k as u64, cid, OWNER), Ok(k as u64 + 1)); }
    assert_eq!(journal::check(&mut store, "copy", STEPS, &mut buffer()).map(|c| c.version), Ok(120));
    assert_eq!(search(&mut store, "copy", INDEX, &word, &mut a).map(|r| r.0), Err(Error::Another));
    assert_eq!(state(&mut store, "copy", INDEX, &mut a).map(|s| s.1), Err(Error::Another));
    assert_eq!(verify(&mut store, "copy", INDEX, &mut a), Err(Error::Another));
    let copy = build(&mut store, "copy", "index/copy", &mut a).unwrap().index;
    assert_eq!((copy.manifest, copy.root == built.index.root), (Manifest { journal: Name::new("copy").unwrap(), ..built.index.manifest }, false));
    // Copied into stores whose journal of the same name is another: longer, its entry at the index's sequence another;
    // shorter.
    let (copy, _) = car::export(&mut store, &[INDEX], Vec::new(), 1 << 20, &mut buffer(), &mut buffer(), |_, _, _| {}).unwrap();
    for (medium, room, entries, error) in [(&mut second, &mut r2, 150, Error::Another), (&mut third, &mut r3, 60, Error::Shorter(60))] {
        let mut there = mount(medium, room);
        Diary::new("diary", 43).append(&mut there, entries);
        car::import(&mut there, &copy[..], &mut buffer(), &mut buffer(), |_, _| {}).unwrap();
        assert_eq!(search(&mut there, "diary", INDEX, &word, &mut a).map(|r| r.0), Err(error));
        assert_eq!(state(&mut there, "diary", INDEX, &mut a).map(|s| s.1), Err(error));
        assert_eq!(verify(&mut there, "diary", INDEX, &mut a), Err(error));
        // Its own index, built there, is current.
        build(&mut there, "diary", INDEX, &mut a).unwrap();
        assert_eq!(state(&mut there, "diary", INDEX, &mut a).map(|s| s.1), Ok(State::Current));
    }
    // A journal rewritten from an older point: its seventh entry published again as the head, which its check refuses;
    // so is every use of its index, and a build publishes nothing.
    assert_eq!(store.publish(b"diary", 120, &diary.cids[6], OWNER), Ok(121));
    let rewritten = Refused { error: journal::Error::Sequence(7), version: 121, at: 121, checked: 0 };
    assert_eq!(state(&mut store, "diary", INDEX, &mut a).map(|s| s.1), Err(Error::Journal(rewritten)));
    assert_eq!(search(&mut store, "diary", INDEX, &word, &mut a).map(|r| r.0), Err(Error::Journal(rewritten)));
    assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Err(Error::Journal(rewritten)));
    assert_eq!(scan(&mut store, "diary", &word, &mut a), Err(Error::Journal(rewritten)));
    assert_eq!(build(&mut store, "diary", INDEX, &mut a), Err(Error::Journal(rewritten)));
    assert_eq!(store.snapshot(INDEX.as_bytes()), Ok((1, Some(built.index.root))));
    // A journal read past the steps given is refused as its check refuses it.
    let short = Refused { error: journal::Error::TooLarge, version: 130, at: 30, checked: 100 };
    assert_eq!(index::build(&mut store, "other", "index/other", 100, &mut a.work()), Err(Error::Journal(short)));
    drop(store);
    // A journal that does not check: an entry's block erased on the medium.
    erase(&mut first, &other.cids[49]);
    let mut r5 = Room::new();
    let mut store = mount(&mut first, &mut r5);
    assert!(build(&mut store, "other", "index/other", &mut a).is_err());
    let missing = Refused { error: journal::Error::Missing, version: 130, at: 50, checked: 80 };
    let mut fresh = Memory::new();
    let mut r4 = Room::new();
    {
        // Its index, built in another store before the erasure, copied in.
        let mut before = mount(&mut fresh, &mut r4);
        let mut again = Diary::new("other", 42);
        again.append(&mut before, 130);
        assert_eq!(again.cids, other.cids);
        build(&mut before, "other", "index/other", &mut a).unwrap();
        let (copy, _) = car::export(&mut before, &["index/other"], Vec::new(), 1 << 20, &mut buffer(), &mut buffer(), |_, _, _| {}).unwrap();
        car::import(&mut store, &copy[..], &mut buffer(), &mut buffer(), |_, _| {}).unwrap();
    }
    let word = std::str::from_utf8(other.model.postings(130).keys().next().unwrap()).unwrap().to_owned();
    assert_eq!(build(&mut store, "other", "index/other", &mut a), Err(Error::Journal(missing)));
    assert_eq!(state(&mut store, "other", "index/other", &mut a).map(|s| s.1), Err(Error::Journal(missing)));
    assert_eq!(search(&mut store, "other", "index/other", &word, &mut a).map(|r| r.0), Err(Error::Journal(missing)));
    assert_eq!(verify(&mut store, "other", "index/other", &mut a), Err(Error::Journal(missing)));
    assert_eq!(scan(&mut store, "other", &word, &mut a), Err(Error::Journal(missing)));
    assert_eq!(store.snapshot(b"index/other").unwrap().0, 1);
}

#[test]
fn an_index_changed_by_hand_is_refused_by_a_search_or_told_by_a_check() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut a = area();
    let mut diary = Diary::new("diary", 53);
    diary.append(&mut store, 60);
    let built = build(&mut store, "diary", INDEX, &mut a).unwrap();
    let postings = diary.model.postings(60);
    let count: u64 = postings.values().map(|s| s.len() as u64).sum();
    let manifest = written(&fields("diary", 60, diary.cids[59], diary.model.entries(60), postings.len() as u64, count));
    assert_eq!(layout(&mut store, &manifest, &files(&postings)), built.index.root);
    // A term several entries hold, and an entry that does not hold it.
    let (term, seqs) = postings.iter().find(|(_, s)| s.len() >= 3).map(|(t, s)| (t.clone(), s.clone())).unwrap();
    let word = String::from_utf8(term.clone()).unwrap();
    let outside = (1..=60).find(|s| !seqs.contains(s)).unwrap();
    let other = postings.keys().find(|t| **t != term).map(|t| String::from_utf8(t.clone()).unwrap()).unwrap();
    let changed = |store: &mut Local, change: &dyn Fn(&mut BTreeMap<Vec<u8>, Vec<u8>>)| {
        let mut f = files(&postings);
        change(&mut f);
        let root = layout(store, &manifest, &f);
        publish(store, INDEX, &root);
    };
    // A posting added: a false answer, refused by the search, told by the check; the state reads no postings.
    let mut added = seqs.clone();
    added.push(outside);
    added.sort();
    changed(&mut store, &|f| { f.insert(term.clone(), postings_file(&added)); });
    assert_eq!(search(&mut store, "diary", INDEX, &word, &mut a).map(|r| r.0), Err(Error::False(outside)));
    assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Ok(Verdict::Differs));
    assert_eq!(state(&mut store, "diary", INDEX, &mut a).map(|s| s.1), Ok(State::Current));
    assert_eq!(search(&mut store, "diary", INDEX, &other, &mut a).unwrap().0, scan(&mut store, "diary", &other, &mut a).unwrap());
    // A term no entry holds, with a posting: a false answer too.
    changed(&mut store, &|f| { f.insert(b"nosuchterm".to_vec(), postings_file(&[outside])); });
    assert_eq!(search(&mut store, "diary", INDEX, "nosuchterm", &mut a).map(|r| r.0), Err(Error::False(outside)));
    assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Ok(Verdict::Differs));
    // A posting dropped: the search cannot tell, it answers less than the scan; the check tells it.
    let dropped: Vec<u64> = seqs[1..].to_vec();
    changed(&mut store, &|f| { f.insert(term.clone(), postings_file(&dropped)); });
    let found = search(&mut store, "diary", INDEX, &word, &mut a).unwrap().0;
    let all = scan(&mut store, "diary", &word, &mut a).unwrap();
    assert_eq!((found.len() + 1, &all[..all.len() - 1]), (all.len(), &found[..]));
    assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Ok(Verdict::Differs));
    // A term dropped whole: the same.
    changed(&mut store, &|f| { f.remove(&term); });
    assert_eq!(search(&mut store, "diary", INDEX, &word, &mut a).unwrap().0, Vec::<u64>::new());
    assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Ok(Verdict::Differs));
    // A manifest whose counts are not the postings': the check tells it, a search answers the scan.
    let miscounted = written(&fields("diary", 60, diary.cids[59], diary.model.entries(60), postings.len() as u64, count + 1));
    let root = layout(&mut store, &miscounted, &files(&postings));
    publish(&mut store, INDEX, &root);
    assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Ok(Verdict::Differs));
    assert_eq!(search(&mut store, "diary", INDEX, &word, &mut a).unwrap().0, all);
    // A manifest naming another entry at its sequence, or another head: of another journal.
    for (seq, head) in [(60, diary.cids[58]), (59, diary.cids[59])] {
        let p = diary.model.postings(seq);
        let wrong = written(&fields("diary", seq, head, diary.model.entries(seq), p.len() as u64, p.values().map(|s| s.len() as u64).sum()));
        let root = layout(&mut store, &wrong, &files(&p));
        publish(&mut store, INDEX, &root);
        assert_eq!(search(&mut store, "diary", INDEX, &word, &mut a).map(|r| r.0), Err(Error::Another), "{seq}");
        assert_eq!(state(&mut store, "diary", INDEX, &mut a).map(|s| s.1), Err(Error::Another), "{seq}");
        assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Err(Error::Another), "{seq}");
    }
    // Postings files not exactly the layout's: unsorted, twice, 0, past the index's sequence, none, a number written
    // longer, a byte after, a map: out of its layout.
    let mut longer = vec![0x81, 0x19];
    longer.extend((seqs[0] as u16).to_be_bytes());
    let mut after = postings_file(&seqs);
    after.push(0);
    let unsorted: Vec<u64> = seqs.iter().rev().copied().collect();
    for (case, file) in [("unsorted", postings_file(&unsorted)), ("twice", postings_file(&[seqs[0], seqs[0]])), ("zero", postings_file(&[0, seqs[0]])),
                         ("past the sequence", postings_file(&[seqs[0], 61])), ("none", postings_file(&[])), ("longer", longer), ("a byte after", after),
                         ("a map", written(&[("v", V::U(1))]))] {
        changed(&mut store, &|f| { f.insert(term.clone(), file.clone()); });
        assert_eq!(search(&mut store, "diary", INDEX, &word, &mut a).map(|r| r.0), Err(Error::Format), "{case}");
    }
    // A root without its manifest, or with an entry more, and a manifest that is a directory: out of the layout.
    let terms_dir = tree::decode(&{ let mut b = buffer(); let n = store.get(&built.index.root, &mut b[..]).unwrap(); b[..n].to_vec() }).unwrap().entry(1).cid;
    let file = store.put(Codec::Raw, &manifest).unwrap();
    let empty = store.put(Codec::DagCbor, b"\xa2\x61v\x01\x67entries\x80").unwrap();
    let count_of = |store: &mut Local, cid: &Cid| { let mut b = buffer(); let n = store.get(cid, &mut b[..]).unwrap(); tree::decode(&b[..n]).unwrap().len() as u64 };
    let nf = count_of(&mut store, &terms_dir);
    for (case, entries) in [
        ("no manifest", vec![(b"terms".to_vec(), 1, nf, terms_dir)]),
        ("an entry more", vec![(b"extra".to_vec(), 0, manifest.len() as u64, file), (b"manifest".to_vec(), 0, manifest.len() as u64, file), (b"terms".to_vec(), 1, nf, terms_dir)]),
        ("a manifest that is a directory", vec![(b"manifest".to_vec(), 1, 0, empty), (b"terms".to_vec(), 1, nf, terms_dir)]),
    ] {
        let root = store.put(Codec::DagCbor, &directory(&entries)).unwrap();
        publish(&mut store, INDEX, &root);
        assert_eq!(search(&mut store, "diary", INDEX, &word, &mut a).map(|r| r.0), Err(Error::Format), "{case}");
        assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Err(Error::Format), "{case}");
    }
    // A file published under the name, no index at all.
    let note = store.put(Codec::Raw, b"just a note").unwrap();
    publish(&mut store, INDEX, &note);
    assert_eq!(state(&mut store, "diary", INDEX, &mut a).map(|s| s.1), Err(Error::Format));
    // Built again: equal.
    assert_eq!(build(&mut store, "diary", INDEX, &mut a).map(|b| b.index.root), Ok(built.index.root));
    assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Ok(Verdict::Equal));
}

#[test]
fn a_journal_past_the_work_area_or_with_a_shard_over_a_block_has_no_index_and_nothing_is_published() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut diary = Diary::new("diary", 61);
    diary.append(&mut store, 40);
    let postings = diary.model.postings(40);
    let (terms, text, count) = (postings.len(), postings.keys().map(|t| t.len()).sum::<usize>(), postings.values().map(|s| s.len()).sum::<usize>());
    let blocks = store.stats().blocks;
    // One short of each bound: refused, nothing put, nothing published.
    for (t, x, p) in [(terms - 1, text, count), (terms, text - 1, count), (terms, text, count - 1), (0, 0, 0)] {
        assert_eq!(build(&mut store, "diary", INDEX, &mut Area::new(t, x, p)), Err(Error::Full), "{t} {x} {p}");
        assert_eq!((store.snapshot(INDEX.as_bytes()), store.stats().blocks), (Ok((0, None)), blocks));
    }
    // At the bounds exactly: built, as in the area `blocks` keeps.
    let mut exact = Area::new(terms, text, count);
    let built = build(&mut store, "diary", INDEX, &mut exact).unwrap();
    assert_eq!(built.index.root, expected(&mut store, &diary, 40));
    assert_eq!(verify(&mut store, "diary", INDEX, &mut exact), Ok(Verdict::Equal));
    // A check past the area is refused as a build is; a search's answers past it too.
    assert_eq!(verify(&mut store, "diary", INDEX, &mut Area::new(terms - 1, text, count)), Err(Error::Full));
    let (term, seqs) = postings.iter().max_by_key(|(_, s)| s.len()).unwrap();
    let word = String::from_utf8(term.clone()).unwrap();
    assert_eq!(search(&mut store, "diary", INDEX, &word, &mut Area::new(0, 0, seqs.len() - 1)).map(|r| r.0), Err(Error::Full));
    assert_eq!(search(&mut store, "diary", INDEX, &word, &mut Area::new(0, 0, seqs.len())).unwrap().0, diary.model.answer(&word));
    // 400 terms sharing their first two bytes: a shard over a block, refused; nothing published.
    let mut wide = Diary::new("wide", 62);
    let text: Vec<u8> = (0..400).map(|k| format!("zz{k:03}")).collect::<Vec<_>>().join(" ").into_bytes();
    wide.add(&mut store, Some(text));
    assert_eq!(build(&mut store, "wide", "index/wide", &mut area()), Err(Error::TooLarge));
    assert_eq!(store.snapshot(b"index/wide"), Ok((0, None)));
    // 300 are one shard of a block.
    let mut fits = Diary::new("fits", 63);
    fits.add(&mut store, Some((0..300).map(|k| format!("zz{k:03}")).collect::<Vec<_>>().join(" ").into_bytes()));
    let built = build(&mut store, "fits", "index/fits", &mut area()).unwrap();
    assert_eq!(built.index.root, expected(&mut store, &fits, 1));
    assert_eq!(search(&mut store, "fits", "index/fits", "ZZ299", &mut area()).unwrap().0, [1]);
}

#[test]
fn contents_of_several_chunks_are_read_whole_and_a_directory_holds_no_term() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut a = area();
    let mut diary = Diary::new("diary", 71);
    // A word across the first chunk's end, and one at the end of the third.
    let mut big = b"x ".repeat(CHUNK / 2 - 2);
    big.extend(b"boundary");
    big.extend(b" y".repeat(CHUNK / 2));
    big.extend(b" lastword");
    assert!(big.len() > 2 * CHUNK);
    diary.add(&mut store, Some(big));
    // A directory of a file whose text is "insidefile": not read.
    let file = store.put(Codec::Raw, b"insidefile").unwrap();
    let entries = [tree::Entry { name: b"named", kind: tree::Kind::File, size: 10, cid: file }];
    let mut b = buffer();
    let len = tree::encode(&entries, &mut b).unwrap();
    let dir = store.put(Codec::DagCbor, &b[..len]).unwrap();
    diary.add_content(&mut store, &dir, None);
    diary.add(&mut store, Some(b"Boundary, small".to_vec()));
    let built = build(&mut store, "diary", INDEX, &mut a).unwrap();
    assert_eq!(built.index.root, expected(&mut store, &diary, 3));
    assert_eq!((built.index.manifest.entries, built.index.manifest.terms), (2, 5));
    for (word, found) in [("boundary", vec![3, 1]), ("lastword", vec![1]), ("insidefile", vec![]), ("named", vec![]), ("y", vec![1])] {
        assert_eq!(search(&mut store, "diary", INDEX, word, &mut a).unwrap().0, found, "{word}");
        assert_eq!(scan(&mut store, "diary", word, &mut a).unwrap(), found, "{word}");
    }
}

#[test]
fn an_index_lives_in_its_journals_memory_domain_and_goes_with_it() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount_with(&mut medium, &mut room, &RANDOM);
    let mut a = area();
    let mut volume = Diary::new("diary", 81);
    volume.append(&mut store, 5);
    let domain = store.domain_create(OWNER).unwrap();
    store.set_domain(domain).unwrap();
    let mut inside = Diary::new("diary", 82);
    inside.append(&mut store, 7);
    let built = build(&mut store, "diary", INDEX, &mut a).unwrap();
    assert_eq!(built.index.root, expected(&mut store, &inside, 7));
    let word = std::str::from_utf8(inside.model.postings(7).keys().next().unwrap()).unwrap().to_owned();
    assert_eq!(search(&mut store, "diary", INDEX, &word, &mut a).unwrap().0, inside.model.answer(&word));
    // The volume's journal of the same name has no index: the domain's is not seen from it.
    store.set_domain(VOLUME).unwrap();
    assert_eq!(search(&mut store, "diary", INDEX, &word, &mut a).map(|r| r.0), Err(Error::NotFound));
    // Forgotten: the domain's index goes with it, and the volume's journal is indexed on its own.
    store.domain_forget(domain, OWNER).unwrap();
    assert_eq!(store.set_domain(domain), Err(store::Error::NotFound));
    store.collect().unwrap();
    assert_eq!(build(&mut store, "diary", INDEX, &mut a).unwrap().index.root, expected(&mut store, &volume, 5));
}

/// The store, where another writer publishes under `name` once this one has put its first block.
struct Racing<'s, 'a> { store: &'s mut Local<'a>, name: &'static str, other: Option<Cid> }
impl journal::Store for Racing<'_, '_> {
    fn put(&mut self, entry: &[u8]) -> Result<Cid, journal::Error> { journal::Store::put(self.store, entry) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, journal::Error> { journal::Store::get(self.store, cid, out) }
    fn head(&mut self, name: &str) -> Result<(u64, Option<Cid>), journal::Error> { journal::Store::head(self.store, name) }
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<u64, journal::Error> { journal::Store::publish(self.store, name, expected, root) }
}
impl Blocks for Racing<'_, '_> {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, dag::Error> {
        if let Some(root) = self.other.take() { self.store.publish(self.name.as_bytes(), 0, &root, OWNER).unwrap(); }
        Blocks::put(self.store, codec, data)
    }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> { Blocks::get(self.store, cid, out) }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> { Blocks::has(self.store, cid) }
}

#[test]
fn of_two_builders_the_second_publishes_nothing() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut a = area();
    let mut diary = Diary::new("diary", 91);
    diary.append(&mut store, 12);
    let theirs = store.put(Codec::Raw, b"another writer's").unwrap();
    let mut racing = Racing { store: &mut store, name: INDEX, other: Some(theirs) };
    assert_eq!(index::build(&mut racing, "diary", INDEX, STEPS, &mut a.work()), Err(Error::Conflict));
    assert_eq!(store.snapshot(INDEX.as_bytes()), Ok((1, Some(theirs))));
    // Built again after it: published at the next version.
    let built = build(&mut store, "diary", INDEX, &mut a).unwrap();
    assert_eq!((built.index.version, built.published, built.index.root), (2, true, expected(&mut store, &diary, 12)));
    assert_eq!(state(&mut store, "diary", INDEX, &mut a).map(|s| s.1), Ok(State::Current));
}

/// The terms a journal's head names: the head as the manifest gives it, read again from the store.
fn head_of(store: &mut Local, name: &str) -> (u64, Cid) { let (v, root) = store.snapshot(name.as_bytes()).unwrap(); (v, root.unwrap()) }

#[test]
fn a_journal_with_no_term_has_an_index_of_none() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut a = area();
    let mut diary = Diary::new("diary", 97);
    diary.add(&mut store, Some(b"... !!! ---".to_vec()));
    diary.add(&mut store, None);
    let built = build(&mut store, "diary", INDEX, &mut a).unwrap();
    let m = built.index.manifest;
    assert_eq!((m.seq, m.head, m.entries, m.terms, m.postings), (2, head_of(&mut store, "diary").1, 0, 0, 0));
    assert_eq!(built.index.root, expected(&mut store, &diary, 2));
    assert_eq!(search(&mut store, "diary", INDEX, "word", &mut a).unwrap().0, Vec::<u64>::new());
    assert_eq!(verify(&mut store, "diary", INDEX, &mut a), Ok(Verdict::Equal));
    // Unknown journals have none.
    assert_eq!(build(&mut store, "nothing", "index/nothing", &mut a).map(|b| b.published), Err(Error::Journal(Refused { error: journal::Error::NotFound, version: 0, at: 0, checked: 0 })));
    assert_eq!(build(&mut store, "a diary", "index/x", &mut a).map(|b| b.published), Err(Error::Name));
}

#[test]
fn a_store_that_lies_is_not_believed() {
    // Contents and index blocks are read through `dag`'s checks: bytes not the block's are corrupt.
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut a = area();
    let mut diary = Diary::new("diary", 99);
    diary.append(&mut store, 10);
    build(&mut store, "diary", INDEX, &mut a).unwrap();
    let word = std::str::from_utf8(diary.model.postings(10).keys().next().unwrap()).unwrap().to_owned();
    let content = { let mut b = buffer(); let n = store.get(&diary.cids[9], &mut b[..]).unwrap(); journal::decode(&b[..n]).unwrap().content };
    let mut lying = Lying { store: &mut store, swap: HashMap::from([(content, b"other bytes".to_vec())]) };
    assert_eq!(index::scan(&mut lying, "diary", &word, STEPS, &mut a.work(), |_| {}), Err(Error::Corrupt));
    assert_eq!(index::build(&mut lying, "diary", INDEX, STEPS, &mut a.work()), Err(Error::Corrupt));
    let root = store.snapshot(INDEX.as_bytes()).unwrap().1.unwrap();
    let mut lying = Lying { store: &mut store, swap: HashMap::from([(root, b"other bytes".to_vec())]) };
    assert_eq!(index::state(&mut lying, "diary", INDEX, STEPS, &mut a.work()).map(|s| s.1), Err(Error::Corrupt));
}

/// The store, answering the CIDs of `swap` with other bytes, as a store that lies does.
struct Lying<'s, 'a> { store: &'s mut Local<'a>, swap: HashMap<Cid, Vec<u8>> }
impl Lying<'_, '_> {
    fn answer(&mut self, cid: &Cid, out: &mut [u8]) -> Option<usize> { self.swap.get(cid).map(|b| { out[..b.len()].copy_from_slice(b); b.len() }) }
}
impl journal::Store for Lying<'_, '_> {
    fn put(&mut self, entry: &[u8]) -> Result<Cid, journal::Error> { journal::Store::put(self.store, entry) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, journal::Error> { match self.answer(cid, out) { Some(n) => Ok(n), None => journal::Store::get(self.store, cid, out) } }
    fn head(&mut self, name: &str) -> Result<(u64, Option<Cid>), journal::Error> { journal::Store::head(self.store, name) }
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<u64, journal::Error> { journal::Store::publish(self.store, name, expected, root) }
}
impl Blocks for Lying<'_, '_> {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, dag::Error> { Blocks::put(self.store, codec, data) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> { match self.answer(cid, out) { Some(n) => Ok(n), None => Blocks::get(self.store, cid, out) } }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> { Blocks::has(self.store, cid) }
}
