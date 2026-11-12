//! Host tests of libmind/src/vector.rs (issue 312-STO-0047; MC-4.5, 4.6, 4.12, 4.13, 10.3): a vector index derived from
//! a journal of embeddings, through the block store's own logic (blockstore/src/store.rs on a simulated medium). An
//! embedding and a manifest are byte for byte what their schemas say, written here from the DAG-CBOR specification, and
//! an index written here from the layout has the root a build gives. An index built twice from a journal of seeded
//! embeddings by two models, and notes, has one root, also after a remount and a collection; every search answers what
//! the exact scan and a model answer, also once the index is stale, and says by how much; an index changed by hand is
//! refused by a search (a vector raised, one at an entry with none) or told by a check (one dropped); an index of
//! another journal, or of one that does not check, is refused; a query of another model or dimensions is refused, and an
//! embedding by the model of other dimensions has no index; past the work area nothing is published; an embedding
//! mentions its subject and model and retains neither.
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
#[path = "../libmind/src/vector.rs"]
mod vector;
#[path = "../blockstore/src/store.rs"]
mod store;
#[path = "../libmind/src/worktree.rs"]
mod worktree;
#[path = "../libmind/src/car.rs"]
mod car;

use cid::{Cid, Codec};
use dag::{Blocks, CHUNK};
use index::{Name, State, Verdict};
use journal::{append, Clock, Reading};
use std::collections::HashMap;
use store::{hmac_sha256, Crypto, Device, Extent, Head, Pin, Store, BUFFER, LEASE_NS, SECTOR};
use vector::{decode, decode_embedding, Built, Embedding, Error, Found, Indexed, Kept, Manifest, Near, Work, DIMS, MANIFEST_MAX, NEAREST, RECORD_MAX};

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
/// store's logic, not its cipher, which tests/blockstore_host.rs covers.
struct Tagged;
static TAGGED: Tagged = Tagged;
impl Crypto for Tagged {
    fn seal(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8]) -> [u8; 16] { hmac_sha256(key, &[nonce, aad, data])[..16].try_into().unwrap() }
    fn open(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8], tag: &[u8; 16]) -> bool { hmac_sha256(key, &[nonce, aad, data])[..16] == *tag }
    fn random(&self, _: &mut [u8]) -> bool { false }
}

struct Room { index: Vec<store::Entry>, heads: Vec<Head>, pins: Vec<Pin>, holes: Vec<Extent>, buffer: Box<[u8; BUFFER]>, scratch: Box<[u8; CHUNK]> }
impl Room {
    fn new() -> Self {
        Room { index: vec![store::Entry::EMPTY; store::slots_for(SECTORS as u64)], heads: vec![Head::EMPTY; 16], pins: vec![Pin::EMPTY; 4], holes: vec![Extent::EMPTY; 64], buffer: Box::new([0; BUFFER]), scratch: Box::new([0; CHUNK]) }
    }
}

type Local<'a> = Store<'a, &'a mut Memory>;

fn mount<'a>(medium: &'a mut Memory, room: &'a mut Room) -> Local<'a> {
    Store::mount(medium, &TAGGED, None, &mut room.index, &mut room.heads, &mut room.pins, &mut room.holes, &mut room.buffer, &mut room.scratch, 0).unwrap()
}

/// The badge the tests publish with (the shell's on the platform).
const OWNER: u16 = 7;
/// Entries a check may read: as many as any journal here holds.
const STEPS: u64 = 1 << 20;
const INDEX: &str = "vector/mind";

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

fn buffer() -> Box<[u8; CHUNK]> { Box::new([0; CHUNK]) }

/// A work area of `files` files and its three blocks.
struct Area { kept: Vec<Kept>, blocks: Box<[[u8; CHUNK]; 3]> }
impl Area {
    fn new(files: usize) -> Self { Area { kept: vec![Kept::EMPTY; files], blocks: Box::new([[0; CHUNK]; 3]) } }
    fn work(&mut self) -> Work<'_> { Work::new(&mut self.kept, &mut self.blocks) }
}
/// The work area `blocks` keeps.
fn area() -> Area { Area::new(vector::FILES) }

fn build(store: &mut Local, model: &Cid, dims: usize, a: &mut Area) -> Result<Built, Error> { vector::build(store, "mind", INDEX, model, dims, STEPS, &mut a.work()) }
fn state(store: &mut Local, a: &mut Area) -> Result<(Indexed, State), Error> { vector::state(store, "mind", INDEX, STEPS, &mut a.work()) }
fn verify(store: &mut Local, a: &mut Area) -> Result<Verdict, Error> { vector::verify(store, "mind", INDEX, STEPS, &mut a.work()).map(|(_, v)| v) }
fn nearest(store: &mut Local, model: &Cid, query: &[u8], k: usize, a: &mut Area) -> Result<(Vec<Near>, Found), Error> {
    let mut found = Vec::new();
    vector::nearest(store, "mind", INDEX, model, query, k, STEPS, &mut a.work(), |n| found.push(*n)).map(|f| (found, f))
}
fn scan(store: &mut Local, model: &Cid, query: &[u8], k: usize, a: &mut Area) -> Result<(Vec<Near>, u64), Error> {
    let mut found = Vec::new();
    vector::scan(store, "mind", model, query, k, STEPS, &mut a.work(), |n| found.push(*n)).map(|n| (found, n))
}

/// A seeded generator (xorshift64), so every run writes the same journals.
struct Seeded(u64);
impl Seeded {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn below(&mut self, n: usize) -> usize { (self.next() % n as u64) as usize }
    fn vector(&mut self, dims: usize) -> Vec<u8> { (0..dims).map(|_| self.next() as u8).collect() }
}

/// The two models of these tests, never put: a model is mentioned, never read.
fn model_a() -> Cid { Cid::of(Codec::Raw, b"a stand-in model of 200 dimensions") }
fn model_b() -> Cid { Cid::of(Codec::Raw, b"another model, of 8") }
const A: usize = 200;
const B: usize = 8;

/// What an entry holds: an embedding (its model, vector and subject), or a note.
#[derive(Clone)]
enum Held { Embedding(Cid, Vec<u8>, Cid), Note }

/// A journal `mind` and what this test knows it holds: entry k at k - 1.
struct Mind { rng: Seeded, cids: Vec<Cid>, held: Vec<Held> }
impl Mind {
    fn new(seed: u64) -> Self { Mind { rng: Seeded(seed), cids: Vec::new(), held: Vec::new() } }
    fn now(&self) -> Reading { let k = self.cids.len() as u64 + 1; Reading { monotonic: 1_000_000_000 + k * 1_000_000, calendar: 1_798_000_000 + k, clock: Clock::Chip } }
    /// Appends `n` seeded entries: about half embeddings by model A, some by model B, some copies of an earlier vector by
    /// A (a tie, which the newer wins), the rest notes.
    fn append(&mut self, store: &mut Local, n: usize) {
        for _ in 0..n {
            match self.rng.below(10) {
                0..=4 => { let v = self.rng.vector(A); self.embed(store, &model_a(), v) }
                5 | 6 => { let v = self.rng.vector(B); self.embed(store, &model_b(), v) }
                7 if self.held.iter().any(|h| matches!(h, Held::Embedding(m, ..) if *m == model_a())) => {
                    let earlier: Vec<Vec<u8>> = self.held.iter().filter_map(|h| match h { Held::Embedding(m, v, _) if *m == model_a() => Some(v.clone()), _ => None }).collect();
                    let v = earlier[self.rng.below(earlier.len())].clone();
                    self.embed(store, &model_a(), v)
                }
                _ => self.note(store),
            }
        }
    }
    fn embed(&mut self, store: &mut Local, model: &Cid, v: Vec<u8>) {
        let subject = Cid::of(Codec::Raw, format!("object {}", self.cids.len() + 1).as_bytes());
        let added = vector::add(store, "mind", self.now(), "test", &Embedding { subject, model: *model, vector: &v }, &mut buffer()).unwrap();
        assert_eq!(added.seq, self.cids.len() as u64 + 1);
        self.cids.push(added.entry);
        self.held.push(Held::Embedding(*model, v, subject));
    }
    fn note(&mut self, store: &mut Local) {
        let content = store.put(Codec::Raw, format!("a note {}", self.cids.len() + 1).as_bytes()).unwrap();
        let done = append(store, "mind", self.now(), "test", "note", &content, &mut buffer()).unwrap();
        self.cids.push(done.cid);
        self.held.push(Held::Note);
    }
    /// The embeddings by `model` up to `upto`, newest first: (sequence, vector).
    fn vectors(&self, model: &Cid, upto: u64) -> Vec<(u64, Vec<u8>)> {
        (1..=upto).rev().filter_map(|s| match &self.held[s as usize - 1] { Held::Embedding(m, v, _) if m == model => Some((s, v.clone())), _ => None }).collect()
    }
    /// The answer, written here: every embedding by `model` scored, highest first, the newest of equal scores first.
    fn answer(&self, model: &Cid, query: &[u8], k: usize) -> Vec<Near> {
        let mut all: Vec<Near> = self.vectors(model, self.cids.len() as u64).into_iter().map(|(seq, v)| {
            let score = v.iter().zip(query).map(|(&x, &y)| x as i8 as i32 * y as i8 as i32).sum();
            let Held::Embedding(_, _, subject) = self.held[seq as usize - 1] else { unreachable!() };
            Near { seq, score, subject }
        }).collect();
        all.sort_by_key(|n| (std::cmp::Reverse(n.score), std::cmp::Reverse(n.seq)));
        all.truncate(k);
        all
    }
    fn head(&self) -> Cid { *self.cids.last().unwrap() }
}

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
fn cbor_text(out: &mut Vec<u8>, text: &str) { cbor_head(out, 3, text.len() as u64); out.extend(text.as_bytes()); }
fn cbor_bytes(out: &mut Vec<u8>, bytes: &[u8]) { cbor_head(out, 2, bytes.len() as u64); out.extend(bytes); }

/// An embedding written here from its schema and the DAG-CBOR specification: `{"v", "model", "vector", "subject"}`, the
/// CIDs as byte strings, not links.
fn embedding_bytes(subject: &Cid, model: &Cid, vector: &[u8]) -> Vec<u8> {
    let mut out = vec![0xa4];
    cbor_text(&mut out, "v");
    out.push(1);
    cbor_text(&mut out, "model");
    cbor_bytes(&mut out, &model.to_bytes());
    cbor_text(&mut out, "vector");
    cbor_bytes(&mut out, vector);
    cbor_text(&mut out, "subject");
    cbor_bytes(&mut out, &subject.to_bytes());
    out
}

/// A manifest written here from its schema: `{"v", "seq", "dims", "head", "kind", "model", "metric", "entries",
/// "journal"}`, the head a link, the model a byte string; `kind`, `metric` and `v` as given, to write wrong ones.
fn manifest_with(v: u64, seq: u64, dims: u64, head: &Cid, kind: &str, model: &Cid, metric: &str, entries: u64, journal: &str) -> Vec<u8> {
    let mut out = vec![0xa9];
    cbor_text(&mut out, "v");
    cbor_head(&mut out, 0, v);
    cbor_text(&mut out, "seq");
    cbor_head(&mut out, 0, seq);
    cbor_text(&mut out, "dims");
    cbor_head(&mut out, 0, dims);
    cbor_text(&mut out, "head");
    out.extend([0xd8, 0x2a, 0x58, 0x25, 0x00]);
    out.extend(head.to_bytes());
    cbor_text(&mut out, "kind");
    cbor_text(&mut out, kind);
    cbor_text(&mut out, "model");
    cbor_bytes(&mut out, &model.to_bytes());
    cbor_text(&mut out, "metric");
    cbor_text(&mut out, metric);
    cbor_text(&mut out, "entries");
    cbor_head(&mut out, 0, entries);
    cbor_text(&mut out, "journal");
    cbor_text(&mut out, journal);
    out
}
fn manifest_bytes(seq: u64, dims: u64, head: &Cid, model: &Cid, entries: u64) -> Vec<u8> { manifest_with(1, seq, dims, head, "vector", model, "dot", entries, "mind") }

/// A directory node written here from `tree`'s schema: entries (name, kind, size, CID) in the order given.
fn directory(entries: &[(Vec<u8>, u64, u64, Cid)]) -> Vec<u8> {
    let mut out = vec![0xa2, 0x61, b'v', 0x01, 0x67];
    out.extend(b"entries");
    cbor_head(&mut out, 4, entries.len() as u64);
    for (name, kind, size, cid) in entries {
        out.push(0x84);
        cbor_head(&mut out, 3, name.len() as u64);
        out.extend(name);
        cbor_head(&mut out, 0, *kind);
        cbor_head(&mut out, 0, *size);
        out.extend([0xd8, 0x2a, 0x58, 0x25, 0x00]);
        out.extend(cid.to_bytes());
    }
    out
}

/// The files of vectors as the layout has them: each vector its sequence (u32, little-endian) and components, newest
/// first, as many to a file as a block holds.
fn files_of(vectors: &[(u64, Vec<u8>)], dims: usize) -> Vec<Vec<u8>> {
    let per = CHUNK / (4 + dims);
    vectors.chunks(per).map(|c| c.iter().flat_map(|(s, v)| (*s as u32).to_le_bytes().into_iter().chain(v.iter().copied())).collect()).collect()
}

/// An index written here from the layout into the store, not by vector.rs: the root holds `manifest` and `vectors`, and
/// the files under `vectors` are named `0000`, `0001`, ... in their order.
fn layout(store: &mut Local, manifest: &[u8], files: &[Vec<u8>]) -> Cid {
    let entries: Vec<_> = files.iter().enumerate().map(|(k, f)| (format!("{k:04x}").into_bytes(), 0, f.len() as u64, store.put(Codec::Raw, f).unwrap())).collect();
    let vectors = store.put(Codec::DagCbor, &directory(&entries)).unwrap();
    let file = store.put(Codec::Raw, manifest).unwrap();
    store.put(Codec::DagCbor, &directory(&[(b"manifest".to_vec(), 0, manifest.len() as u64, file), (b"vectors".to_vec(), 1, files.len() as u64, vectors)])).unwrap()
}

/// The index of model A's embeddings in `mind` up to `seq`, written here from the layout; its root.
fn expected(store: &mut Local, mind: &Mind, seq: u64) -> Cid {
    let vectors = mind.vectors(&model_a(), seq);
    let manifest = manifest_bytes(seq, A as u64, &mind.cids[seq as usize - 1], &model_a(), vectors.len() as u64);
    layout(store, &manifest, &files_of(&vectors, A))
}

/// Publishes `root` under `name` after its current version.
fn publish(store: &mut Local, name: &str, root: &Cid) -> u64 {
    let version = store.snapshot(name.as_bytes()).unwrap().0;
    store.publish(name.as_bytes(), version, root, OWNER).unwrap()
}

#[test]
fn an_embedding_and_a_manifest_are_their_schemas_encodings() {
    let (subject, model) = (Cid::of(Codec::Raw, b"a photo of the sea"), model_a());
    let vector: Vec<u8> = (0..A).map(|k| (k as u8).wrapping_mul(37)).collect();
    let e = Embedding { subject, model, vector: &vector };
    let mut out = [0u8; RECORD_MAX];
    let len = e.encode(&mut out).unwrap();
    assert_eq!(out[..len], embedding_bytes(&subject, &model, &vector)[..]);
    assert_eq!(decode_embedding(&out[..len]), Ok(e));
    // The largest fits RECORD_MAX.
    let most = vec![0x80; DIMS];
    assert_eq!(Embedding { subject, model, vector: &most }.encode(&mut out).map(|n| n <= RECORD_MAX), Ok(true));
    let head = Cid::of(Codec::DagCbor, b"an entry");
    let m = Manifest { journal: Name::new("mind").unwrap(), seq: 300, head, model, dims: A as u64, entries: 150 };
    let mut out = [0u8; MANIFEST_MAX];
    let len = m.encode(&mut out).unwrap();
    assert_eq!(out[..len], manifest_bytes(300, A as u64, &head, &model, 150)[..]);
    assert_eq!(decode(&out[..len]), Ok(m));
    // The largest fits MANIFEST_MAX.
    let name = "n".repeat(car::NAME_MAX);
    let m = Manifest { journal: Name::new(&name).unwrap(), seq: u64::MAX, head, model, dims: DIMS as u64, entries: u64::MAX };
    assert_eq!(m.encode(&mut out).map(|n| n <= MANIFEST_MAX), Ok(true));
    // The metric, exact: the dot product of signed bytes.
    assert_eq!(vector::score(&[0x80, 0x7f, 3], &[0x80, 0x80, 0xff]), 16384 - 16256 - 3);
    assert_eq!(vector::score(&vec![0x80; DIMS], &vec![0x80; DIMS]), DIMS as i32 * 16384);
}

#[test]
fn only_canonical_embeddings_and_manifests_decode_and_another_version_kind_or_metric_is_refused_by_it() {
    let (subject, model) = (Cid::of(Codec::Raw, b"x"), model_b());
    let good = embedding_bytes(&subject, &model, &[1, 2, 3]);
    assert!(decode_embedding(&good).is_ok());
    let mut v2 = good.clone();
    v2[3] = 2;
    assert_eq!(decode_embedding(&v2), Err(Error::Version(2)));
    let mut after = good.clone();
    after.push(0);
    let mut longer = vec![0xa4, 0x61, b'v', 0x18, 0x01];
    longer.extend(&good[4..]);
    let mut link = vec![0xa4, 0x61, b'v', 0x01, 0x65];
    link.extend(b"model");
    link.extend([0xd8, 0x2a, 0x58, 0x25, 0x00]);
    link.extend(&good[4 + 6 + 2..]);
    let mut five = good.clone();
    five[0] = 0xa5;
    for (case, bytes) in [("no components", embedding_bytes(&subject, &model, &[])), ("past DIMS", embedding_bytes(&subject, &model, &vec![1; DIMS + 1])),
                          ("a byte after", after), ("a number written longer", longer), ("the model a link", link), ("five keys", five),
                          ("the keys out of order", { let mut b = vec![0xa4, 0x61, b'v', 0x01, 0x66]; b.extend(b"vector"); b.extend(&good[4..]); b })] {
        assert_eq!(decode_embedding(&bytes).map(|_| ()), Err(Error::Format), "{case}");
    }
    let mut out = [0u8; RECORD_MAX];
    assert_eq!(Embedding { subject, model, vector: &[] }.encode(&mut out), Err(Error::Invalid));
    assert_eq!(Embedding { subject, model, vector: &vec![0; DIMS + 1] }.encode(&mut out), Err(Error::Invalid));
    let head = Cid::of(Codec::DagCbor, b"an entry");
    assert!(decode(&manifest_bytes(5, 8, &head, &model, 3)).is_ok());
    assert_eq!(decode(&manifest_with(2, 5, 8, &head, "vector", &model, "dot", 3, "mind")), Err(Error::Version(2)));
    assert_eq!(decode(&manifest_with(1, 5, 8, &head, "text", &model, "dot", 3, "mind")), Err(Error::Kind));
    assert_eq!(decode(&manifest_with(1, 5, 8, &head, "vector", &model, "cosine", 3, "mind")), Err(Error::Metric));
    let raw = Cid::of(Codec::Raw, b"no entry");
    for (case, bytes) in [("no dimensions", manifest_bytes(5, 0, &head, &model, 3)), ("past DIMS", manifest_bytes(5, DIMS as u64 + 1, &head, &model, 3)),
                          ("more vectors than entries", manifest_bytes(5, 8, &head, &model, 6)), ("sequence 0", manifest_bytes(0, 8, &head, &model, 0)),
                          ("a head that is no entry", manifest_bytes(5, 8, &raw, &model, 3)), ("a journal's name the store does not take", manifest_with(1, 5, 8, &head, "vector", &model, "dot", 3, ""))] {
        assert_eq!(decode(&bytes), Err(Error::Format), "{case}");
    }
}

#[test]
fn an_index_is_built_twice_to_one_root_also_after_a_remount_and_a_collection() {
    let mut medium = Memory::new();
    let mut a = area();
    let mut mind = Mind::new(11);
    let root;
    {
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        mind.append(&mut store, 300);
        let built = build(&mut store, &model_a(), A, &mut a).unwrap();
        let m = built.index.manifest;
        let count = mind.vectors(&model_a(), 300).len() as u64;
        assert_eq!((m.journal.as_str(), m.seq, m.head, m.model, m.dims, m.entries), ("mind", 300, mind.head(), model_a(), A as u64, count));
        // More than a file holds (80 of 200 dimensions), and model B's not among them.
        assert!(count > 2 * 80 && mind.vectors(&model_b(), 300).len() > 30, "{count}");
        assert_eq!((built.index.version, built.published), (1, true));
        // The root of the index written here from the layout.
        root = built.index.root;
        assert_eq!(expected(&mut store, &mind, 300), root);
        // Built again: the same root, and nothing published again.
        assert_eq!(build(&mut store, &model_a(), A, &mut a), Ok(Built { index: built.index, published: false }));
        assert_eq!(store.snapshot(INDEX.as_bytes()), Ok((1, Some(root))));
        assert_eq!(state(&mut store, &mut a), Ok((built.index, State::Current)));
        assert_eq!(verify(&mut store, &mut a), Ok(Verdict::Equal));
    }
    // A new instance mounts the medium: the same root again.
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let again = build(&mut store, &model_a(), A, &mut a).unwrap();
    assert_eq!((again.index.root, again.index.version, again.published), (root, 1, false));
    // Once every lease has ended a collection frees what nothing retains and keeps the index, which its name retains.
    let loose = store.put(Codec::Raw, b"in no journal").unwrap();
    store.set_time(LEASE_NS + 1);
    assert_eq!(store.collect().unwrap().blocks, 1);
    assert!(!store.has(&loose));
    assert_eq!(verify(&mut store, &mut a), Ok(Verdict::Equal));
    let q = Seeded(5).vector(A);
    assert_eq!(nearest(&mut store, &model_a(), &q, 16, &mut a).unwrap().0, mind.answer(&model_a(), &q, 16));
    // An index of model B, under another name, beside it.
    let b = vector::build(&mut store, "mind", "vector/mind-b", &model_b(), B, STEPS, &mut a.work()).unwrap();
    assert_eq!(b.index.manifest.entries, mind.vectors(&model_b(), 300).len() as u64);
    let q = Seeded(6).vector(B);
    let mut found = Vec::new();
    vector::nearest(&mut store, "mind", "vector/mind-b", &model_b(), &q, 5, STEPS, &mut a.work(), |n| found.push(*n)).unwrap();
    assert_eq!(found, mind.answer(&model_b(), &q, 5));
}

#[test]
fn every_search_answers_what_the_exact_scan_and_the_model_answer() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut a = area();
    let mut mind = Mind::new(23);
    mind.append(&mut store, 260);
    build(&mut store, &model_a(), A, &mut a).unwrap();
    let count = mind.vectors(&model_a(), 260).len() as u64;
    let mut rng = Seeded(99);
    // Seeded queries, the vectors the journal holds, and the extremes; every k.
    let mut queries: Vec<Vec<u8>> = (0..40).map(|_| rng.vector(A)).collect();
    queries.extend(mind.vectors(&model_a(), 260).into_iter().step_by(9).map(|(_, v)| v));
    queries.extend([vec![0x80; A], vec![0x7f; A], vec![0; A]]);
    let mut ties = 0;
    for (n, q) in queries.iter().enumerate() {
        let k = 1 + n % NEAREST;
        let (found, f) = nearest(&mut store, &model_a(), q, k, &mut a).unwrap();
        let answer = mind.answer(&model_a(), q, k);
        assert_eq!(found, answer, "query {n}");
        assert_eq!((f.state, f.vectors, f.nearest), (State::Current, count, k as u64));
        assert_eq!(scan(&mut store, &model_a(), q, k, &mut a), Ok((answer, count)), "query {n}");
        ties += found.windows(2).filter(|w| w[0].score == w[1].score).count();
    }
    // The zero query scores all alike: the newest win.
    assert!(ties >= NEAREST - 1, "{ties}");
    // No index under the name, or one removed.
    let q = rng.vector(A);
    assert_eq!(vector::nearest(&mut store, "mind", "vector/none", &model_a(), &q, 1, STEPS, &mut a.work(), |_| ()).map(|_| ()), Err(Error::NotFound));
    let version = store.snapshot(INDEX.as_bytes()).unwrap().0;
    assert_eq!(store.unpublish(INDEX.as_bytes(), version, OWNER), Ok(version + 1));
    assert_eq!(nearest(&mut store, &model_a(), &q, 1, &mut a).map(|_| ()), Err(Error::Removed(version + 1)));
    assert_eq!(verify(&mut store, &mut a), Err(Error::Removed(version + 1)));
}

#[test]
fn after_appends_the_index_is_told_stale_by_how_many_and_every_search_still_answers_the_scan() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut a = area();
    let mut mind = Mind::new(31);
    mind.append(&mut store, 200);
    let built = build(&mut store, &model_a(), A, &mut a).unwrap();
    // Twelve more, among them the best vector of every query below.
    mind.append(&mut store, 11);
    let best = vec![0x7f; A];
    mind.embed(&mut store, &model_a(), best.clone());
    assert_eq!(state(&mut store, &mut a), Ok((built.index, State::Stale(12))));
    let above = mind.vectors(&model_a(), 212).len() - mind.vectors(&model_a(), 200).len();
    let mut rng = Seeded(7);
    let mut from_above = 0;
    for n in 0..30 {
        let q: Vec<u8> = if n == 0 { best.clone() } else { rng.vector(A) };
        let (found, f) = nearest(&mut store, &model_a(), &q, 1 + n % NEAREST, &mut a).unwrap();
        assert_eq!((f.state, f.index, f.vectors), (State::Stale(12), built.index, built.index.manifest.entries + above as u64));
        assert_eq!(found, mind.answer(&model_a(), &q, 1 + n % NEAREST), "query {n}");
        assert_eq!(found, scan(&mut store, &model_a(), &q, 1 + n % NEAREST, &mut a).unwrap().0, "query {n}");
        from_above += found.iter().filter(|x| x.seq > 200).count();
    }
    assert!(from_above > 10, "{from_above}");
    assert_eq!(nearest(&mut store, &model_a(), &best, 1, &mut a).unwrap().0[0].seq, 212);
    // The check finds it equal to its rebuild from its own head, the journal grown by twelve.
    assert_eq!(verify(&mut store, &mut a), Ok(Verdict::Stale(12)));
    // Rebuilt: current, at the next version, its root the one written here.
    let rebuilt = build(&mut store, &model_a(), A, &mut a).unwrap();
    assert_eq!((rebuilt.index.version, rebuilt.published, rebuilt.index.manifest.seq), (2, true, 212));
    assert_eq!(rebuilt.index.root, expected(&mut store, &mind, 212));
    assert_eq!(state(&mut store, &mut a).map(|s| s.1), Ok(State::Current));
    assert_eq!(verify(&mut store, &mut a), Ok(Verdict::Equal));
}

#[test]
fn an_index_changed_by_hand_is_refused_by_a_search_or_told_by_a_check() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut a = area();
    let mut mind = Mind::new(53);
    mind.append(&mut store, 240);
    let built = build(&mut store, &model_a(), A, &mut a).unwrap();
    let vectors = mind.vectors(&model_a(), 240);
    let manifest = manifest_bytes(240, A as u64, &mind.head(), &model_a(), vectors.len() as u64);
    assert_eq!(layout(&mut store, &manifest, &files_of(&vectors, A)), built.index.root);
    let q = vec![0x7f; A];
    let changed = |store: &mut Local, manifest: &[u8], vectors: &[(u64, Vec<u8>)]| { let root = layout(store, manifest, &files_of(vectors, A)); publish(store, INDEX, &root); };
    // A vector raised past every other: a false answer, refused by the search, told by the check.
    let mut raised = vectors.clone();
    let low = raised.iter().position(|(_, v)| vector::score(&q, v) < 0).unwrap();
    raised[low].1 = vec![0x7f; A];
    changed(&mut store, &manifest, &raised);
    assert_eq!(nearest(&mut store, &model_a(), &q, 3, &mut a).map(|_| ()), Err(Error::False(raised[low].0)));
    assert_eq!(verify(&mut store, &mut a), Ok(Verdict::Differs));
    assert_eq!(state(&mut store, &mut a).map(|s| s.1), Ok(State::Current));
    // A vector at an entry with none (a note, or model B's): a false answer too.
    for other in [Held::Note, Held::Embedding(model_b(), vec![], model_b())] {
        let seq = (1..=240u64).find(|&s| match (&mind.held[s as usize - 1], &other) { (Held::Note, Held::Note) => true, (Held::Embedding(m, ..), Held::Embedding(o, ..)) => m == o, _ => false }).unwrap();
        let mut more = vectors.clone();
        let at = more.iter().position(|(s, _)| *s < seq).unwrap_or(more.len());
        more.insert(at, (seq, vec![0x7f; A]));
        let manifest = manifest_bytes(240, A as u64, &mind.head(), &model_a(), more.len() as u64);
        changed(&mut store, &manifest, &more);
        assert_eq!(nearest(&mut store, &model_a(), &q, 1, &mut a).map(|_| ()), Err(Error::False(seq)));
        assert_eq!(verify(&mut store, &mut a), Ok(Verdict::Differs));
    }
    // The best vector dropped: the search cannot tell, it answers the next; the check tells it.
    let answer = mind.answer(&model_a(), &q, 2);
    let dropped: Vec<_> = vectors.iter().filter(|(s, _)| *s != answer[0].seq).cloned().collect();
    let fewer = manifest_bytes(240, A as u64, &mind.head(), &model_a(), dropped.len() as u64);
    changed(&mut store, &fewer, &dropped);
    assert_eq!(nearest(&mut store, &model_a(), &q, 1, &mut a).unwrap().0, answer[1..]);
    assert_eq!(verify(&mut store, &mut a), Ok(Verdict::Differs));
    // A manifest naming another entry at its sequence, or another head: of another journal.
    for (seq, head) in [(240, mind.cids[238]), (239, mind.cids[239])] {
        let v = mind.vectors(&model_a(), seq);
        changed(&mut store, &manifest_bytes(seq, A as u64, &head, &model_a(), v.len() as u64), &v);
        assert_eq!(nearest(&mut store, &model_a(), &q, 1, &mut a).map(|_| ()), Err(Error::Another), "{seq}");
        assert_eq!(state(&mut store, &mut a).map(|s| s.1), Err(Error::Another), "{seq}");
        assert_eq!(verify(&mut store, &mut a), Err(Error::Another), "{seq}");
    }
    // Files not exactly the layout's: vectors out of order, twice, sequence 0 or past the index's; a file not full that is
    // not the last, one empty, a vector cut short; a count not the vectors'; a file misnamed.
    let swapped = { let mut v = vectors.clone(); v.swap(3, 4); v };
    let twice = { let mut v = vectors.clone(); v[4].0 = v[3].0; v };
    let zero = { let mut v = vectors.clone(); v.last_mut().unwrap().0 = 0; v };
    let past = { let mut v = vectors.clone(); v[0].0 = 241; v };
    for (case, v) in [("out of order", &swapped), ("twice", &twice), ("zero", &zero), ("past the sequence", &past)] {
        changed(&mut store, &manifest, v);
        assert_eq!(nearest(&mut store, &model_a(), &q, 1, &mut a).map(|_| ()), Err(Error::Format), "{case}");
    }
    let files = files_of(&vectors, A);
    assert!(files.len() >= 2);
    let short_first = { let mut f = files.clone(); let len = f[0].len() - (4 + A); f[0].truncate(len); f };
    let empty_last = { let mut f = files.clone(); f.push(Vec::new()); f };
    let cut = { let mut f = files.clone(); f.last_mut().unwrap().pop(); f };
    for (case, m, f) in [("a file not full before the last", &manifest, &short_first), ("an empty file", &manifest, &empty_last), ("a vector cut short", &manifest, &cut),
                         ("a count not the vectors'", &manifest_bytes(240, A as u64, &mind.head(), &model_a(), vectors.len() as u64 + 1), &files)] {
        let root = layout(&mut store, m, f);
        publish(&mut store, INDEX, &root);
        assert_eq!(nearest(&mut store, &model_a(), &q, 1, &mut a).map(|_| ()), Err(Error::Format), "{case}");
    }
    let entries: Vec<_> = files.iter().enumerate().map(|(k, f)| (format!("{:04x}", k + 1).into_bytes(), 0, f.len() as u64, store.put(Codec::Raw, f).unwrap())).collect();
    let misnamed = store.put(Codec::DagCbor, &directory(&entries)).unwrap();
    let mfile = store.put(Codec::Raw, &manifest).unwrap();
    let root = store.put(Codec::DagCbor, &directory(&[(b"manifest".to_vec(), 0, manifest.len() as u64, mfile), (b"vectors".to_vec(), 1, files.len() as u64, misnamed)])).unwrap();
    publish(&mut store, INDEX, &root);
    assert_eq!(nearest(&mut store, &model_a(), &q, 1, &mut a).map(|_| ()), Err(Error::Format));
    // A text index's manifest under the name: another kind. A file under it: no index at all.
    let text = manifest_with(1, 240, A as u64, &mind.head(), "text", &model_a(), "dot", vectors.len() as u64, "mind");
    changed(&mut store, &text, &vectors);
    assert_eq!(nearest(&mut store, &model_a(), &q, 1, &mut a).map(|_| ()), Err(Error::Kind));
    let note = store.put(Codec::Raw, b"just a note").unwrap();
    publish(&mut store, INDEX, &note);
    assert_eq!(state(&mut store, &mut a).map(|s| s.1), Err(Error::Format));
    // Built again: equal.
    assert_eq!(build(&mut store, &model_a(), A, &mut a).map(|b| b.index.root), Ok(built.index.root));
    assert_eq!(verify(&mut store, &mut a), Ok(Verdict::Equal));
}

#[test]
fn a_query_of_another_model_or_dimensions_is_refused_and_an_embedding_by_the_model_of_other_dimensions_has_no_index() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut a = area();
    let mut mind = Mind::new(61);
    mind.append(&mut store, 40);
    build(&mut store, &model_a(), A, &mut a).unwrap();
    let q = Seeded(3).vector(A);
    assert_eq!(nearest(&mut store, &model_b(), &q, 1, &mut a).map(|_| ()), Err(Error::Model));
    assert_eq!(nearest(&mut store, &model_a(), &q[..A - 1], 1, &mut a).map(|_| ()), Err(Error::Dims(0)));
    for k in [0, NEAREST + 1] {
        assert_eq!(nearest(&mut store, &model_a(), &q, k, &mut a).map(|_| ()), Err(Error::Count), "{k}");
        assert_eq!(scan(&mut store, &model_a(), &q, k, &mut a).map(|_| ()), Err(Error::Count), "{k}");
    }
    assert_eq!(scan(&mut store, &model_a(), &[], 1, &mut a).map(|_| ()), Err(Error::Dims(0)));
    assert_eq!(build(&mut store, &model_a(), 0, &mut a).map(|_| ()), Err(Error::Dims(0)));
    assert_eq!(build(&mut store, &model_a(), DIMS + 1, &mut a).map(|_| ()), Err(Error::Dims(0)));
    // No embedding, and no source, are refused with nothing appended.
    let subject = Cid::of(Codec::Raw, b"y");
    assert_eq!(vector::add(&mut store, "mind", mind.now(), "test", &Embedding { subject, model: model_a(), vector: &[] }, &mut buffer()).map(|_| ()), Err(Error::Invalid));
    assert_eq!(vector::add(&mut store, "mind", mind.now(), "", &Embedding { subject, model: model_a(), vector: &q }, &mut buffer()).map(|_| ()), Err(Error::Invalid));
    assert_eq!(store.snapshot(b"mind").unwrap().0, 40);
    // An embedding by the model of other dimensions above the index's head: the search, the scan and a build refuse it at
    // its sequence, nothing published.
    mind.embed(&mut store, &model_a(), vec![1; A - 1]);
    let version = store.snapshot(INDEX.as_bytes()).unwrap();
    assert_eq!(nearest(&mut store, &model_a(), &q, 1, &mut a).map(|_| ()), Err(Error::Dims(41)));
    assert_eq!(scan(&mut store, &model_a(), &q, 1, &mut a).map(|_| ()), Err(Error::Dims(41)));
    assert_eq!(build(&mut store, &model_a(), A, &mut a).map(|_| ()), Err(Error::Dims(41)));
    assert_eq!(store.snapshot(INDEX.as_bytes()).unwrap(), version);
    // Model B's index is not concerned: its embeddings are of B's dimensions.
    assert!(vector::build(&mut store, "mind", "vector/mind-b", &model_b(), B, STEPS, &mut a.work()).is_ok());
    // An entry of kind `embedding` whose content is no embedding: refused as its read is.
    let mut other = Mind::new(62);
    let note = store.put(Codec::Raw, b"not an embedding").unwrap();
    append(&mut store, "other", other.now(), "test", vector::ENTRY, &note, &mut buffer()).unwrap();
    other.cids.push(Cid::of(Codec::DagCbor, b"-"));
    assert_eq!(vector::build(&mut store, "other", "vector/other", &model_a(), A, STEPS, &mut a.work()).map(|_| ()), Err(Error::Format));
    let dir = store.put(Codec::DagCbor, b"\xa2\x61v\x01\x67entries\x80").unwrap();
    append(&mut store, "other", other.now(), "test", vector::ENTRY, &dir, &mut buffer()).unwrap();
    assert_eq!(vector::build(&mut store, "other", "vector/other", &model_a(), A, STEPS, &mut a.work()).map(|_| ()), Err(Error::Format));
    assert_eq!(store.snapshot(b"vector/other"), Ok((0, None)));
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
fn an_index_of_another_journal_or_of_one_that_does_not_check_is_refused() {
    let mut a = area();
    let mut medium = Memory::new();
    let mut mind = Mind::new(71);
    let q = Seeded(8).vector(A);
    let entry;
    {
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        mind.append(&mut store, 30);
        build(&mut store, &model_a(), A, &mut a).unwrap();
        // Asked of another journal, the index is refused; of a journal shorter than it, too.
        let mut other = Mind::new(72);
        for _ in 0..3 { let v = other.rng.vector(A); let subject = model_b(); vector::add(&mut store, "short", other.now(), "test", &Embedding { subject, model: model_a(), vector: &v }, &mut buffer()).unwrap(); other.cids.push(subject); }
        assert_eq!(vector::nearest(&mut store, "short", INDEX, &model_a(), &q, 1, STEPS, &mut a.work(), |_| ()).map(|_| ()), Err(Error::Another));
        let root = store.snapshot(INDEX.as_bytes()).unwrap().1.unwrap();
        publish(&mut store, "vector/short", &root);
        assert_eq!(vector::state(&mut store, "short", "vector/short", STEPS, &mut a.work()).map(|_| ()), Err(Error::Another));
        // An entry of the journal erased below: the journal does not check, and no index answers.
        entry = mind.cids[9];
    }
    erase(&mut medium, &entry);
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    assert!(matches!(nearest(&mut store, &model_a(), &q, 1, &mut a), Err(Error::Journal(_))));
    assert!(matches!(scan(&mut store, &model_a(), &q, 1, &mut a), Err(Error::Journal(_))));
    assert!(matches!(verify(&mut store, &mut a), Err(Error::Journal(_))));
    let version = store.snapshot(INDEX.as_bytes()).unwrap();
    assert!(matches!(build(&mut store, &model_a(), A, &mut a), Err(Error::Journal(_))));
    assert_eq!(store.snapshot(INDEX.as_bytes()).unwrap(), version);
}

#[test]
fn a_journal_past_the_work_area_has_no_index_and_nothing_is_published() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut mind = Mind::new(81);
    // Two files of vectors of 1024 dimensions (15 to a file).
    for _ in 0..16 { let v = mind.rng.vector(DIMS); mind.embed(&mut store, &model_a(), v); }
    let mut small = Area::new(1);
    assert_eq!(build(&mut store, &model_a(), DIMS, &mut small).map(|_| ()), Err(Error::Full));
    assert_eq!(store.snapshot(INDEX.as_bytes()), Ok((0, None)));
    let mut a = Area::new(2);
    let built = build(&mut store, &model_a(), DIMS, &mut a).unwrap();
    assert_eq!(built.index.manifest.entries, 16);
    assert_eq!(verify(&mut store, &mut a), Ok(Verdict::Equal));
    assert_eq!(verify(&mut store, &mut small), Err(Error::Full));
    let q = mind.rng.vector(DIMS);
    assert_eq!(nearest(&mut store, &model_a(), &q, NEAREST, &mut small).unwrap().0, mind.answer(&model_a(), &q, NEAREST));
}

#[test]
fn an_embedding_mentions_its_subject_and_model_and_retains_neither() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut a = area();
    let mut mind = Mind::new(91);
    // The subject and the model are put, then nothing retains them but the embedding's mention.
    let subject = store.put(Codec::Raw, b"a photo of the sea").unwrap();
    let model = store.put(Codec::Raw, b"a model's weights, in short").unwrap();
    let v = mind.rng.vector(B);
    let added = vector::add(&mut store, "mind", mind.now(), "test", &Embedding { subject, model, vector: &v }, &mut buffer()).unwrap();
    assert_eq!(added.cid, Cid::of(Codec::Raw, &embedding_bytes(&subject, &model, &v)));
    mind.cids.push(added.entry);
    vector::build(&mut store, "mind", INDEX, &model, B, STEPS, &mut a.work()).unwrap();
    store.set_time(LEASE_NS + 1);
    assert_eq!(store.collect().unwrap().blocks, 2);
    assert!(!store.has(&subject) && !store.has(&model) && store.has(&added.cid));
    let mut found = Vec::new();
    let f = vector::nearest(&mut store, "mind", INDEX, &model, &v, 1, STEPS, &mut a.work(), |n| found.push(*n)).unwrap();
    assert_eq!((found, f.state), (vec![Near { seq: 1, score: vector::score(&v, &v), subject }], State::Current));
}

/// A store that answers a block with other bytes than it was asked for.
struct Lying<'s, 'a> { store: &'s mut Local<'a>, swap: HashMap<Cid, Vec<u8>> }
impl journal::Store for Lying<'_, '_> {
    fn put(&mut self, entry: &[u8]) -> Result<Cid, journal::Error> { journal::Store::put(self.store, entry) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, journal::Error> { journal::Store::get(self.store, cid, out) }
    fn head(&mut self, name: &str) -> Result<(u64, Option<Cid>), journal::Error> { journal::Store::head(self.store, name) }
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<u64, journal::Error> { journal::Store::publish(self.store, name, expected, root) }
}
impl Blocks for Lying<'_, '_> {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, dag::Error> { Blocks::put(self.store, codec, data) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> {
        match self.swap.get(cid) { Some(bytes) => { out[..bytes.len()].copy_from_slice(bytes); Ok(bytes.len()) } None => Blocks::get(self.store, cid, out) }
    }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> { Blocks::has(self.store, cid) }
}

#[test]
fn a_store_that_lies_is_not_believed() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let mut a = area();
    let mut mind = Mind::new(97);
    mind.append(&mut store, 30);
    let built = build(&mut store, &model_a(), A, &mut a).unwrap();
    let q = vec![0x7f; A];
    let answer = mind.answer(&model_a(), &q, 1);
    // The best embedding answered with another vector: its CID does not match.
    let Held::Embedding(_, _, subject) = mind.held[answer[0].seq as usize - 1] else { unreachable!() };
    let true_cid = Cid::of(Codec::Raw, &embedding_bytes(&subject, &model_a(), &mind.vectors(&model_a(), 30).into_iter().find(|(s, _)| *s == answer[0].seq).unwrap().1));
    let mut lying = Lying { store: &mut store, swap: HashMap::from([(true_cid, embedding_bytes(&subject, &model_a(), &vec![0; A]))]) };
    let r = vector::nearest(&mut lying, "mind", INDEX, &model_a(), &q, 1, STEPS, &mut a.work(), |_| ());
    assert_eq!(r.map(|_| ()), Err(Error::Corrupt));
    // A file of the index answered with other bytes: its CID does not match either.
    let root = built.index.root;
    let mut b = buffer();
    let n = Blocks::get(&mut store, &root, &mut b[..]).unwrap();
    let vectors = tree::decode(&b[..n]).unwrap().entry(1).cid;
    let n = Blocks::get(&mut store, &vectors, &mut b[..]).unwrap();
    let first = tree::decode(&b[..n]).unwrap().entry(0).cid;
    let n = Blocks::get(&mut store, &first, &mut b[..]).unwrap();
    let mut wrong = b[..n].to_vec();
    wrong[4] ^= 1;
    let mut lying = Lying { store: &mut store, swap: HashMap::from([(first, wrong)]) };
    let r = vector::nearest(&mut lying, "mind", INDEX, &model_a(), &q, 1, STEPS, &mut a.work(), |_| ());
    assert_eq!(r.map(|_| ()), Err(Error::Corrupt));
}
