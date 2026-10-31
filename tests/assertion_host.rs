//! Host tests of libmind/src/assertion.rs (issue 312-STO-0038; MC-4.5, 4.6, 4.12, 4.13): assertions about objects kept
//! in a journal, through the block store's own logic (blockstore/src/store.rs on a simulated medium). An assertion is
//! byte for byte what the reference DAG-CBOR library encodes, and the schema written here; every other encoding is
//! refused, another version by its number. `about` answers one subject's claims newest first, as a model of the rules
//! written here answers them, over seeded journals of several sources; a withdrawal by the same source withdraws, one by
//! another does not, and a retraction of anything but the source's own standing claim is refused with nothing appended.
//! A subject collected leaves its assertions, which `about` still answers; the store's walk never visits a mention. An
//! assertion changed is refused, and a journal that does not check answers nothing.
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
#[path = "../libmind/src/assertion.rs"]
mod assertion;
#[path = "../blockstore/src/store.rs"]
mod store;

use assertion::{decode, Added, Answered, Assertion, Claim, Error, Value, Work, MAX, TEXT};
use cid::{Cid, Codec};
use dag::{Blocks, CHUNK};
use journal::{Clock, Label, Reading, Refused};
use std::collections::{BTreeSet, HashMap};
use store::{hmac_sha256, Crypto, Device, Extent, Head, Pin, Store, BUFFER, HEADER, LEASE_NS, SECTOR};

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

/// Records sealed by a tag alone, their bytes left plain, so a test finds a block on the medium: assertions need the
/// store's logic, not its cipher, which tests/blockstore_host.rs covers.
struct Tagged;
static TAGGED: Tagged = Tagged;
impl Crypto for Tagged {
    fn seal(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8]) -> [u8; 16] { hmac_sha256(key, &[nonce, aad, data])[..16].try_into().unwrap() }
    fn open(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8], tag: &[u8; 16]) -> bool { hmac_sha256(key, &[nonce, aad, data])[..16] == *tag }
    fn random(&self, _out: &mut [u8]) -> bool { false }
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

/// The store as a journal sees it, for one owner.
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

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
fn buffer() -> Box<[u8; CHUNK]> { Box::new([0; CHUNK]) }
fn label(text: &str) -> Label { Label::new(text).unwrap() }

/// The clocks at the k-th append of a test: each later than the one before, in one boot.
fn reading(k: u64) -> Reading { Reading { monotonic: 1_000_000_000 + k * 1_000_000, calendar: 1_798_000_000 + k, clock: Clock::Chip } }

/// A work area of `claims` claims and withdrawals, and its two blocks.
struct Area { claims: Vec<Claim>, entry: Box<[u8; CHUNK]>, block: Box<[u8; CHUNK]> }
impl Area {
    fn new(claims: usize) -> Self { Area { claims: vec![Claim::EMPTY; claims], entry: buffer(), block: buffer() } }
    fn work(&mut self) -> Work<'_> { Work::new(&mut self.claims, &mut self.entry, &mut self.block) }
}
/// The work area `blocks` keeps.
fn area() -> Area { Area::new(256) }

/// A claim of text about `subject`, of type `ty`, interpretation 1.
fn text<'a>(subject: Cid, ty: &str, value: &'a str) -> Assertion<'a> { Assertion { subject, ty: label(ty), interp: 1, value: Some(Value::Text(value)), basis: None, retracts: None } }

fn encoded(a: &Assertion) -> Vec<u8> { let mut out = [0u8; MAX]; let n = a.encode(&mut out).unwrap(); out[..n].to_vec() }

/// What `about` answered: each claim's sequence, source, CID and bytes, newest first.
type Said = (u64, String, Cid, Vec<u8>);
fn about(store: &mut Local, journal: &str, subject: &Cid, steps: u64, area: &mut Area) -> (Result<Answered, Error>, Vec<Said>) {
    let mut heard = Vec::new();
    let r = assertion::about(store, journal, subject, steps, &mut area.work(), |c, a| heard.push((c.seq, c.source().to_owned(), c.cid(), encoded(a))));
    (r, heard)
}
fn answers(store: &mut Local, journal: &str, subject: &Cid) -> Vec<Said> {
    let (r, heard) = about(store, journal, subject, STEPS, &mut area());
    assert_eq!(r.map(|a| a.claims), Ok(heard.len() as u64));
    heard
}
fn version(store: &mut Local, name: &str) -> u64 { store.snapshot(name.as_bytes()).unwrap().0 }

/// One journal and the times its appends take, so every append's monotonic time is past the one before.
struct Writer { name: &'static str, k: u64 }
impl Writer {
    fn new(name: &'static str) -> Self { Writer { name, k: 0 } }
    fn now(&mut self) -> Reading { self.k += 1; reading(self.k) }
    fn add(&mut self, store: &mut Local, source: &str, a: &Assertion) -> Result<Added, Error> { let now = self.now(); assertion::add(store, self.name, now, source, a, &mut buffer()) }
    fn retract(&mut self, store: &mut Local, source: &str, target: &Cid) -> Result<Added, Error> {
        let now = self.now();
        assertion::retract(store, self.name, now, source, target, STEPS, &mut area().work())
    }
    /// An entry appended without this module: `bytes` put as a `raw` block, the content of an entry of `kind`.
    fn entry(&mut self, store: &mut Local, source: &str, kind: &str, bytes: &[u8]) -> journal::Appended {
        let content = store.put(Codec::Raw, bytes).unwrap();
        self.entry_of(store, source, kind, &content)
    }
    fn entry_of(&mut self, store: &mut Local, source: &str, kind: &str, content: &Cid) -> journal::Appended {
        let now = self.now();
        journal::append(store, self.name, now, source, kind, content, &mut buffer()).unwrap()
    }
}

/// A value of a field, written here as DAG-CBOR: a number, a text, a CID as a byte string, or as a link (tag 42).
#[derive(Clone, Copy)]
enum V<'a> { U(u64), T(&'a str), B(Cid), L(Cid), Raw(&'a [u8]) }

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

/// A map of `fields` in the order given, written here from the DAG-CBOR specification, not by assertion.rs.
fn written(fields: &[(&str, V)]) -> Vec<u8> {
    let mut out = Vec::new();
    cbor_head(&mut out, 5, fields.len() as u64);
    for (key, value) in fields {
        cbor_head(&mut out, 3, key.len() as u64);
        out.extend(key.as_bytes());
        match value {
            V::U(n) => cbor_head(&mut out, 0, *n),
            V::T(t) => { cbor_head(&mut out, 3, t.len() as u64); out.extend(t.as_bytes()) }
            V::B(c) => { cbor_head(&mut out, 2, 36); out.extend(c.to_bytes()) }
            V::L(c) => { out.extend([0xd8, 0x2a, 0x58, 0x25, 0x00]); out.extend(c.to_bytes()) }
            V::Raw(bytes) => out.extend(*bytes),
        }
    }
    out
}

/// An assertion's fields in the schema's order: `v`, `type`, `basis`, `value`, `interp`, `subject`, `retracts`.
fn fields<'a>(a: &Assertion<'a>) -> Vec<(&'static str, V<'a>)> {
    let mut f = vec![("v", V::U(1)), ("type", V::T(Box::leak(a.ty.as_str().to_owned().into_boxed_str())))];
    if let Some(b) = a.basis { f.push(("basis", V::B(b))); }
    match a.value { Some(Value::Text(t)) => f.push(("value", V::T(t))), Some(Value::Cid(c)) => f.push(("value", V::B(c))), None => {} }
    f.push(("interp", V::U(a.interp)));
    f.push(("subject", V::B(a.subject)));
    if let Some(r) = a.retracts { f.push(("retracts", V::B(r))); }
    f
}

#[test]
fn an_assertion_is_the_reference_encoding_and_mentions_by_bytes_not_links() {
    // As the reference library dag-cbor 0.3.3 (with multiformats 0.3.1) encodes them, keys sorted by it: CIDs given as
    // bytes, so byte strings, not links.
    let object = Cid::of(Codec::DagCbor, b"an object");
    let note = Cid::raw(b"a note");
    let a1 = text(object, "title", "A first title");
    let b1 = encoded(&a1);
    assert_eq!(hex(&b1), "a56176016474797065657469746c656576616c75656d41206669727374207469746c6566696e7465727001677375626a656374582401711220c44b27339f507e2e4c80dd9aa9d42ef740bc8b619e717f3aa45862de27874289");
    assert_eq!(Cid::raw(&b1).to_string(), "bafkreidmjd5jq6txgcvievgh34t3upnfioi4ggi7pawyvrgbkd7exinm7u");
    let a2 = Assertion { subject: note, ty: label("seealso"), interp: 2, value: Some(Value::Cid(Cid::raw(b"another note"))), basis: Some(Cid::of(Codec::DagCbor, b"a model")), retracts: None };
    let b2 = encoded(&a2);
    assert_eq!(hex(&b2), "a6617601647479706567736565616c736f65626173697358240171122097ff8a35544dcf40ad84047074439dd6aa3a0be469ea578a1b1c017d939ec61c6576616c7565582401551220ec6db24e4da311bf737ea6481d49881120c7d07e14a1e63b2254a344cb8c4d6866696e7465727002677375626a656374582401551220f63e34a034f19a24438f2d74b242bc96682abd843ecc4ea63fefed4006d860a3");
    assert_eq!(Cid::raw(&b2).to_string(), "bafkreidsylwoezaulvpxp7dcr6anlavg7oejofzr272kvtn2tvuef3mqzy");
    let a3 = Assertion { value: None, retracts: Some(Cid::raw(&b1)), ..a1 };
    let b3 = encoded(&a3);
    assert_eq!(hex(&b3), "a56176016474797065657469746c6566696e7465727001677375626a656374582401711220c44b27339f507e2e4c80dd9aa9d42ef740bc8b619e717f3aa45862de278742896872657472616374735824015512206c48fa987a7730aa8254c7df27ba3da54391c3191f782d8ac4c150fe4ba1acfd");
    assert_eq!(Cid::raw(&b3).to_string(), "bafkreiawuzfl3rvh3ajx2xulrqvikwp2gkwr4jgo3tv6jvy4nguh7wfrre");
    let long = "x".repeat(32);
    let a4 = Assertion { subject: note, ty: label(&long), interp: 300, value: None, basis: Some(Cid::raw(b"an index")), retracts: Some(Cid::raw(&b2)) };
    let b4 = encoded(&a4);
    assert_eq!(hex(&b4), "a661760164747970657820787878787878787878787878787878787878787878787878787878787878787865626173697358240155122013f28396e236ef752a2f8e7911c5ecc6750e3c9b96114475b260cc73cdd06d3a66696e7465727019012c677375626a656374582401551220f63e34a034f19a24438f2d74b242bc96682abd843ecc4ea63fefed4006d860a368726574726163747358240155122072c2ece264145d5f77fc628f80d582a6fb88971731d7f4aacdba9d6842ed90ce");
    assert_eq!(Cid::raw(&b4).to_string(), "bafkreibk7qktx47kacf4qkx6mkrxposglmygnvtaxree7yueljx3aimw3e");
    // The schema written here gives the same bytes, each reads back, and none holds a link (tag 42, `d8 2a`).
    for (a, bytes) in [(a1, &b1), (a2, &b2), (a3, &b3), (a4, &b4)] {
        assert_eq!(&written(&fields(&a)), bytes);
        assert_eq!(decode(bytes), Ok(a));
        assert!(!bytes.windows(2).any(|w| w == [0xd8, 0x2a]), "{}", hex(bytes));
    }
    // Numbers in every integer form, texts of 0, 23, 24, 255, 256 and TEXT bytes, labels of 1 and 32 bytes.
    for interp in [0, 23, 24, 255, 256, 65_535, 65_536, (1 << 32) - 1, 1 << 32, u64::MAX] {
        for len in [0, 23, 24, 255, 256, TEXT] {
            let t = "é".repeat(len / 2) + &"t".repeat(len % 2);
            for ty in ["t", long.as_str()] {
                let a = Assertion { interp, ty: label(ty), ..text(object, "x", &t) };
                let bytes = encoded(&a);
                assert_eq!(bytes, written(&fields(&a)), "{interp} {len}");
                assert_eq!(decode(&bytes), Ok(a));
            }
        }
    }
    // The largest fits MAX exactly.
    let t = "m".repeat(TEXT);
    let max = Assertion { subject: object, ty: label(&long), interp: u64::MAX, value: Some(Value::Text(&t)), basis: Some(note), retracts: None };
    let mut out = [0u8; MAX];
    assert_eq!(max.encode(&mut out), Ok(MAX));
    assert_eq!(decode(&out), Ok(max));
}

#[test]
fn only_the_canonical_encoding_decodes_and_another_version_is_refused_by_its_number() {
    let object = Cid::of(Codec::DagCbor, b"an object");
    let note = Cid::raw(b"a note");
    let claim = Assertion { subject: object, ty: label("tag"), interp: 2, value: Some(Value::Cid(note)), basis: Some(Cid::raw(b"a model")), retracts: None };
    let said = text(object, "title", "A title");
    let withdrawal = Assertion { value: None, retracts: Some(Cid::raw(&encoded(&said))), ..said };
    // Every byte changed to every other value: refused, or read as exactly what it encodes.
    for a in [claim, said, withdrawal] {
        let bytes = encoded(&a);
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
        assert!(read > 0);
        // Every truncation and a trailing byte are refused.
        for len in 0..bytes.len() { assert!(decode(&bytes[..len]).is_err(), "cut to {len}"); }
        for extra in [0u8, 0x01, 0xff] { assert_eq!(decode(&[&bytes[..], &[extra]].concat()), Err(Error::Format)); }
    }
    let f = fields(&said);
    let with = |key: &str, value: V| -> Vec<u8> { written(&f.iter().map(|&(k, v)| if k == key { (k, value) } else { (k, v) }).collect::<Vec<_>>()) };
    let without = |key: &str| -> Vec<u8> { written(&f.iter().filter(|(k, _)| *k != key).copied().collect::<Vec<_>>()) };
    let plus = |at: usize, key: &'static str, value: V<'static>| -> Vec<u8> { let mut g = f.clone(); g.insert(at, (key, value)); written(&g) };
    let mut swapped = f.clone();
    swapped.swap(1, 2);
    let mut longer = written(&f);
    let at = longer.windows(7).position(|w| w == b"\x66interp").unwrap() + 7;
    longer.splice(at..at + 1, [0x18, 0x01]);
    let mut text_longer = written(&f);
    let at = text_longer.windows(6).position(|w| w == b"\x65value").unwrap() + 6;
    text_longer.splice(at..at + 1, [0x78, 7]);
    let past = "p".repeat(TEXT + 1);
    let at_bound = "p".repeat(TEXT);
    let mut not_utf8 = with("value", V::T("A title"));
    let at = not_utf8.windows(7).position(|w| w == b"A title").unwrap();
    not_utf8[at + 1] = 0xff;
    let mut dag_pb = note.to_bytes().to_vec();
    dag_pb[1] = 0x70;
    let mut bytes_35 = vec![0x58, 35];
    bytes_35.extend(&note.to_bytes()[..35]);
    let mut as_dag_pb = vec![0x58, 36];
    as_dag_pb.extend(&dag_pb);
    let other_version = written(&[("v", V::U(2)), ("anything", V::U(0))]);
    let long_type = "y".repeat(33);
    for (case, bytes, error) in [
        ("version 2", with("v", V::U(2)), Error::Version(2)),
        ("version 0", with("v", V::U(0)), Error::Version(0)),
        ("another version, its keys others", other_version, Error::Version(2)),
        ("keys out of order", written(&swapped), Error::Format),
        ("an extra key", plus(5, "zzzzzzzzzz", V::U(0)), Error::Format),
        ("no type", without("type"), Error::Format),
        ("no interp", without("interp"), Error::Format),
        ("no subject", without("subject"), Error::Format),
        ("neither a value nor a withdrawal", without("value"), Error::Format),
        ("a value and a withdrawal", plus(5, "retracts", V::B(note)), Error::Format),
        ("a text past its bound", with("value", V::T(&past)), Error::Format),
        ("a text not UTF-8", not_utf8, Error::Format),
        ("a value that is a number", with("value", V::U(7)), Error::Format),
        ("an empty type", with("type", V::T("")), Error::Format),
        ("a type of 33 bytes", with("type", V::T(&long_type)), Error::Format),
        ("a type with a space", with("type", V::T("a type")), Error::Format),
        ("a type beyond ASCII", with("type", V::T("тип")), Error::Format),
        ("a type with a control byte", with("type", V::T("a\x7f")), Error::Format),
        ("a subject as a link", with("subject", V::L(object)), Error::Format),
        ("a subject of 35 bytes", with("subject", V::Raw(&bytes_35)), Error::Format),
        ("a subject of dag-pb", with("subject", V::Raw(&as_dag_pb)), Error::Format),
        ("a CID value as a link", with("value", V::L(note)), Error::Format),
        ("a basis as a link", plus(2, "basis", V::L(note)), Error::Format),
        ("an interp written longer", longer, Error::Format),
        ("a text's length written longer", text_longer, Error::Format),
    ] {
        assert_eq!(decode(&bytes), Err(error), "{case}");
    }
    assert_eq!(decode(&with("value", V::T(&at_bound))).map(|a| a.value), Ok(Some(Value::Text(&at_bound))));
    let w = fields(&withdrawal);
    let w_with = |key: &str, value: V| -> Vec<u8> { written(&w.iter().map(|&(k, v)| if k == key { (k, value) } else { (k, v) }).collect::<Vec<_>>()) };
    assert_eq!(decode(&w_with("retracts", V::B(object))), Err(Error::Format), "a withdrawal of a dag-cbor block");
    assert_eq!(decode(&w_with("retracts", V::L(note))), Err(Error::Format), "a withdrawal as a link");
    // What makes no assertion is not encoded: a text past its bound, a value and a withdrawal, neither, a withdrawal of
    // a `dag-cbor` block.
    let mut out = [0u8; MAX + 1];
    for bad in [text(object, "t", &past), Assertion { retracts: Some(note), ..said }, Assertion { value: None, ..said }, Assertion { retracts: Some(object), ..withdrawal }] {
        assert_eq!(bad.encode(&mut out), Err(Error::Invalid));
    }
}

/// A model of the rules, written here: what each entry of a journal is, and the claims `about` answers.
#[derive(Clone)]
struct Record { seq: u64, source: String, assertion: bool, subject: Cid, cid: Cid, retracts: Option<Cid>, bytes: Vec<u8> }
fn model(records: &[Record], subject: &Cid) -> Vec<Said> {
    records.iter().rev().filter(|r| r.assertion && r.subject == *subject && r.retracts.is_none())
        .filter(|r| !records.iter().any(|w| w.seq > r.seq && w.assertion && w.source == r.source && w.subject == *subject && w.retracts == Some(r.cid)))
        .map(|r| (r.seq, r.source.clone(), r.cid, r.bytes.clone())).collect()
}

/// A seeded generator (xorshift64), so every run writes the same journals.
struct Seeded(u64);
impl Seeded {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn below(&mut self, n: usize) -> usize { (self.next() % n as u64) as usize }
}

#[test]
fn about_answers_one_subjects_claims_newest_first_and_none_of_another() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let (x, y, z) = (Cid::raw(b"object x"), Cid::of(Codec::DagCbor, b"object y"), Cid::raw(b"object z"));
    let mut j = Writer::new("claims");
    let mut records = Vec::new();
    let said = |j: &mut Writer, store: &mut Local, records: &mut Vec<Record>, source: &str, a: Assertion| {
        let done = j.add(store, source, &a).unwrap();
        assert_eq!(done.seq, records.len() as u64 + 1);
        records.push(Record { seq: done.seq, source: source.into(), assertion: true, subject: a.subject, cid: done.cid, retracts: None, bytes: encoded(&a) });
    };
    let note = |j: &mut Writer, store: &mut Local, records: &mut Vec<Record>, kind: &str, bytes: &[u8]| {
        let done = j.entry(store, "a", kind, bytes);
        records.push(Record { seq: done.entry.seq, source: "a".into(), assertion: false, subject: x, cid: done.entry.content, retracts: None, bytes: bytes.to_vec() });
    };
    said(&mut j, &mut store, &mut records, "a", text(x, "title", "first"));
    note(&mut j, &mut store, &mut records, "note", b"a note between");
    said(&mut j, &mut store, &mut records, "b", text(x, "tag", "red"));
    said(&mut j, &mut store, &mut records, "a", text(y, "title", "other"));
    // An entry of another kind whose content is an assertion's block about x: not an assertion of the journal.
    note(&mut j, &mut store, &mut records, "note", &encoded(&text(x, "title", "not said")));
    said(&mut j, &mut store, &mut records, "a", Assertion { value: Some(Value::Cid(y)), basis: Some(z), ..text(x, "seealso", "") });
    // The same bytes said again by b, and by a: each its source's claim.
    said(&mut j, &mut store, &mut records, "b", text(x, "tag", "red"));
    said(&mut j, &mut store, &mut records, "a", text(x, "tag", "red"));
    let xs = answers(&mut store, "claims", &x);
    assert_eq!(xs.iter().map(|s| (s.0, s.1.as_str())).collect::<Vec<_>>(), [(8, "a"), (7, "b"), (6, "a"), (3, "b"), (1, "a")]);
    assert_eq!(xs, model(&records, &x));
    assert_eq!(xs[0].2, xs[1].2);
    assert_eq!(answers(&mut store, "claims", &y).iter().map(|s| s.0).collect::<Vec<_>>(), [4]);
    assert_eq!(answers(&mut store, "claims", &z), []);
    assert_eq!(about(&mut store, "claims", &x, STEPS, &mut area()).0, Ok(Answered { version: 8, claims: 5 }));
    // A journal of no assertion answers none; one not there is refused as its check refuses it.
    let mut notes = Writer::new("notes");
    notes.entry(&mut store, "a", "note", b"just a note");
    assert_eq!(answers(&mut store, "notes", &x), []);
    assert_eq!(about(&mut store, "nothing", &x, STEPS, &mut area()).0, Err(Error::Journal(Refused { error: journal::Error::NotFound, version: 0, at: 0, checked: 0 })));
}

#[test]
fn seeded_journals_of_three_sources_answer_as_the_model_does() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let subjects: Vec<Cid> = (0..4).map(|k| Cid::raw(format!("subject {k}").as_bytes())).collect();
    let sources = ["a", "b", "c"];
    let values = ["red", "green", "blue"];
    let mut rng = Seeded(0x0038);
    let mut j = Writer::new("seeded");
    let mut records: Vec<Record> = Vec::new();
    let (mut withdrawn, mut refused) = (0, 0);
    for _ in 0..240 {
        let source = sources[rng.below(3)];
        let seq = records.len() as u64 + 1;
        match rng.below(10) {
            // A claim, often one said before, so a claim of the same bytes comes again after its withdrawal.
            0..=5 => {
                let subject = subjects[rng.below(4)];
                let v = values[rng.below(3)];
                let a = text(subject, ["tag", "title"][rng.below(2)], v);
                let done = j.add(&mut store, source, &a).unwrap();
                records.push(Record { seq, source: source.into(), assertion: true, subject, cid: done.cid, retracts: None, bytes: encoded(&a) });
            }
            // A retraction of an earlier claim of any source: only the source's own that stands is withdrawn.
            6..=8 if !records.is_empty() => {
                let target = records[rng.below(records.len())].clone();
                let own = records.iter().filter(|r| r.source == source && r.assertion && r.retracts.is_none() && r.cid == target.cid).last().cloned();
                let stands = own.as_ref().is_some_and(|o| !records.iter().any(|w| w.seq > o.seq && w.source == source && w.retracts == Some(o.cid)));
                let expected = match (&own, stands) {
                    _ if !target.assertion || target.retracts.is_some() => {
                        if records.iter().any(|r| r.assertion && r.retracts.is_none() && r.cid == target.cid && r.source != source) { Err(Error::Another) } else { Err(Error::NotFound) }
                    }
                    (Some(_), true) => Ok(()),
                    (Some(_), false) => Err(Error::Withdrawn),
                    (None, _) => Err(Error::Another),
                };
                let before = version(&mut store, "seeded");
                match j.retract(&mut store, source, &target.cid) {
                    Ok(done) => {
                        assert_eq!(expected, Ok(()));
                        let o = own.unwrap();
                        let w = Assertion { value: None, retracts: Some(o.cid), ..decode(&o.bytes).unwrap() };
                        assert_eq!((done.seq, done.cid), (seq, Cid::raw(&encoded(&w))));
                        records.push(Record { seq, source: source.into(), assertion: true, subject: o.subject, cid: done.cid, retracts: Some(o.cid), bytes: encoded(&w) });
                        withdrawn += 1;
                    }
                    Err(e) => { assert_eq!(Err(e), expected, "{seq}"); assert_eq!(version(&mut store, "seeded"), before); refused += 1; }
                }
            }
            // A withdrawal written without `retract`, of any claim, by any source: honoured only if its own.
            9 if !records.is_empty() => {
                let target = records[rng.below(records.len())].clone();
                if !target.assertion || target.retracts.is_some() { continue; }
                let w = Assertion { value: None, retracts: Some(target.cid), ..decode(&target.bytes).unwrap() };
                let done = j.entry(&mut store, source, "assertion", &encoded(&w));
                records.push(Record { seq, source: source.into(), assertion: true, subject: target.subject, cid: done.entry.content, retracts: Some(target.cid), bytes: encoded(&w) });
            }
            _ => {
                let done = j.entry(&mut store, source, "note", format!("a note at {seq}").as_bytes());
                records.push(Record { seq, source: source.into(), assertion: false, subject: subjects[0], cid: done.entry.content, retracts: None, bytes: vec![] });
            }
        }
        if records.len() as u64 % 40 == 0 {
            for s in &subjects { assert_eq!(answers(&mut store, "seeded", s), model(&records, s)); }
        }
    }
    let mut answered = 0;
    for s in &subjects {
        let found = answers(&mut store, "seeded", s);
        assert_eq!(found, model(&records, s));
        answered += found.len();
    }
    // Every kind of case was met.
    let others = records.iter().filter(|w| w.retracts.is_some() && records.iter().any(|r| Some(r.cid) == w.retracts && r.source != w.source)).count();
    assert!(answered > 40 && withdrawn > 10 && refused > 10 && others > 0, "{answered} {withdrawn} {refused} {others}");
}

#[test]
fn a_retraction_by_the_same_source_withdraws_and_by_another_does_not() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let x = Cid::raw(b"object x");
    let mut j = Writer::new("claims");
    let seqs = |found: Vec<Said>| found.iter().map(|s| (s.0, s.1.clone())).collect::<Vec<_>>();
    let a = j.add(&mut store, "a", &text(x, "title", "one")).unwrap();
    let b = j.add(&mut store, "a", &text(x, "title", "two")).unwrap();
    let c = j.add(&mut store, "b", &text(x, "title", "three")).unwrap();
    assert_eq!(seqs(answers(&mut store, "claims", &x)), [(3, "b".into()), (2, "a".into()), (1, "a".into())]);
    // a withdraws its first: the withdrawal is the claim's subject, type and interpretation and names it.
    let w = j.retract(&mut store, "a", &a.cid).unwrap();
    let expected = Assertion { value: None, retracts: Some(a.cid), ..text(x, "title", "") };
    assert_eq!((w.seq, w.cid), (4, Cid::raw(&encoded(&expected))));
    assert_eq!(assertion::read(&mut store, &w.cid, &mut buffer()), Ok(expected));
    assert_eq!(seqs(answers(&mut store, "claims", &x)), [(3, "b".into()), (2, "a".into())]);
    // b's withdrawal of a's second, written without `retract`: b's own record, which withdraws nothing of a's.
    let by_b = Assertion { value: None, retracts: Some(b.cid), ..text(x, "title", "") };
    j.entry(&mut store, "b", "assertion", &encoded(&by_b));
    assert_eq!(seqs(answers(&mut store, "claims", &x)), [(3, "b".into()), (2, "a".into())]);
    // Refused, nothing appended: another source's claim, one withdrawn already, a withdrawal, an entry, a block of no
    // assertion, a CID not held, and a withdrawal given to `add`.
    let note = j.entry(&mut store, "a", "note", b"a note");
    assert_eq!(version(&mut store, "claims"), 6);
    for (source, target, error) in [("b", b.cid, Error::Another), ("c", c.cid, Error::Another), ("a", c.cid, Error::Another), ("a", a.cid, Error::Withdrawn),
                                    ("a", w.cid, Error::NotFound), ("a", note.cid, Error::NotFound), ("a", note.entry.content, Error::NotFound),
                                    ("a", Cid::raw(b"never put"), Error::NotFound)] {
        assert_eq!(j.retract(&mut store, source, &target), Err(error), "{source} {target}");
        assert_eq!(version(&mut store, "claims"), 6);
    }
    assert_eq!(j.add(&mut store, "a", &expected), Err(Error::Invalid));
    assert_eq!(j.retract(&mut store, "a source", &b.cid), Err(Error::Invalid));
    assert_eq!(j.add(&mut store, "", &text(x, "title", "no source")), Err(Error::Invalid));
    assert_eq!(version(&mut store, "claims"), 6);
    // The first said again: a withdrawal withdraws only what came before it, so it stands, and is withdrawn again.
    let again = j.add(&mut store, "a", &text(x, "title", "one")).unwrap();
    assert_eq!((again.seq, again.cid), (7, a.cid));
    assert_eq!(seqs(answers(&mut store, "claims", &x)), [(7, "a".into()), (3, "b".into()), (2, "a".into())]);
    assert_eq!(j.retract(&mut store, "a", &a.cid).map(|w| (w.seq, w.cid)), Ok((8, Cid::raw(&encoded(&expected)))));
    assert_eq!(seqs(answers(&mut store, "claims", &x)), [(3, "b".into()), (2, "a".into())]);
    // Each source withdraws its own.
    j.retract(&mut store, "a", &b.cid).unwrap();
    j.retract(&mut store, "b", &c.cid).unwrap();
    assert_eq!(answers(&mut store, "claims", &x), []);
    assert_eq!(journal::check(&mut store, "claims", STEPS, &mut buffer()).map(|c| c.version), Ok(10));
}

/// The CIDs the store's walk visits from a journal's head.
fn visited(store: &mut Local, name: &str) -> BTreeSet<Cid> {
    let head = store.snapshot(name.as_bytes()).unwrap().1.unwrap();
    let mut seen = BTreeSet::new();
    journal::walk(store, &head, &mut buffer(), STEPS, |_, cid| { seen.insert(*cid); Ok(()) }).unwrap();
    seen
}

#[test]
fn a_subject_collected_leaves_its_assertions_and_about_still_answers_them() {
    let mut medium = Memory::new();
    let (subject, chunks, basis, thumb, said);
    {
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        // An object of three chunks under a name, a model under another, and a thumbnail put loose.
        let mut builder = Box::new(dag::Builder::new());
        let bytes: Vec<u8> = (0..2 * CHUNK + 100).map(|i| ((i * 31 + 7) % 251) as u8).collect();
        builder.write(&mut store, &bytes).unwrap();
        subject = builder.finish(&mut store).unwrap().0;
        let mut b = buffer();
        chunks = { let mut c = Vec::new(); dag::walk(&mut store, &subject, &mut b, |_, cid| { c.push(*cid); Ok(()) }).unwrap(); c };
        assert_eq!(chunks.len(), 4);
        store.publish(b"photo", 0, &subject, OWNER).unwrap();
        basis = store.put(Codec::Raw, b"a model").unwrap();
        store.publish(b"model", 0, &basis, OWNER).unwrap();
        thumb = store.put(Codec::Raw, b"a thumbnail").unwrap();
        let mut j = Writer::new("claims");
        let first = j.add(&mut store, "a", &text(subject, "title", "a photo")).unwrap();
        let second = j.add(&mut store, "a", &Assertion { value: Some(Value::Cid(thumb)), basis: Some(basis), ..text(subject, "thumbnail", "") }).unwrap();
        said = [second, first];
        // The store's walk of the journal visits each entry and assertion, and nothing an assertion mentions.
        let seen = visited(&mut store, "claims");
        assert_eq!(seen, BTreeSet::from([first.entry, first.cid, second.entry, second.cid]));
        // The names removed and every lease ended: a collection frees the object, the model and the thumbnail.
        store.unpublish(b"photo", 1, OWNER).unwrap();
        store.unpublish(b"model", 1, OWNER).unwrap();
        store.set_time(LEASE_NS + 1);
        assert_eq!(store.collect().unwrap().blocks, 6);
        for cid in chunks.iter().chain([&basis, &thumb]) { assert!(!store.has(cid), "{cid}"); }
        for d in &said { assert!(store.has(&d.cid) && store.has(&d.entry)); }
        let found = answers(&mut store, "claims", &subject);
        assert_eq!(found.iter().map(|s| (s.0, s.2)).collect::<Vec<_>>(), [(2, said[0].cid), (1, said[1].cid)]);
        let mut b = buffer();
        let a = assertion::read(&mut store, &said[0].cid, &mut b).unwrap();
        assert_eq!((a.subject, a.value, a.basis), (subject, Some(Value::Cid(thumb)), Some(basis)));
    }
    // A new instance mounts the medium: the same answers, the object still gone.
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    assert!(!store.has(&subject));
    assert_eq!(answers(&mut store, "claims", &subject).iter().map(|s| s.2).collect::<Vec<_>>(), [said[0].cid, said[1].cid]);
    // The owner is charged the assertions' bytes, as any journal's contents, and not the subject's.
    let charged = store.usage(OWNER).retained;
    assert!(charged < CHUNK as u64, "{charged}");
}

/// Where the block `cid` lies on the medium: its record's header (`MIND-BLK`, the CID at 16) is in the clear, as records
/// here are sealed by a tag alone, and its bytes follow it.
fn record(medium: &Memory, cid: &Cid) -> usize {
    let key = cid.to_bytes();
    (0..medium.data.len() - HEADER).find(|&at| &medium.data[at..at + 8] == b"MIND-BLK" && medium.data[at + 16..at + 52] == key).expect("the block on the medium")
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

#[test]
fn an_assertion_changed_is_refused_and_a_journal_that_does_not_check_answers_nothing() {
    let mut medium = Memory::new();
    let x = Cid::raw(b"object x");
    let (said, entries);
    {
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        let mut j = Writer::new("claims");
        let mut s = Vec::new();
        let mut e = Vec::new();
        for k in 0..6 {
            let done = j.add(&mut store, "a", &text(x, "tag", &format!("tag {k}"))).unwrap();
            s.push(done.cid);
            e.push(done.entry);
        }
        (said, entries) = (s, e);
        // Answered by a store that lies, with the bytes of another assertion about the same subject: not believed.
        let other = encoded(&text(x, "tag", "tag 0"));
        let mut lying = Lying { store: &mut store, swap: HashMap::from([(said[3], other.clone())]) };
        let mut heard = 0;
        assert_eq!(assertion::about(&mut lying, "claims", &x, STEPS, &mut area().work(), |_, _| heard += 1), Err(Error::Corrupt));
        assert_eq!(heard, 0);
        assert_eq!(assertion::retract(&mut lying, "claims", reading(100), "a", &said[5], STEPS, &mut area().work()), Err(Error::Corrupt));
        assert_eq!(assertion::read(&mut lying, &said[3], &mut buffer()), Err(Error::Corrupt));
        assert_eq!(version(&mut store, "claims"), 6);
        // Read past its steps, the journal is refused as its check refuses it, and nothing is answered.
        let (r, heard) = about(&mut store, "claims", &x, 4, &mut area());
        assert_eq!((r, heard.len()), (Err(Error::Journal(Refused { error: journal::Error::TooLarge, version: 6, at: 2, checked: 4 })), 0));
        assert_eq!(answers(&mut store, "claims", &x).len(), 6);
        // A rewrite from an older point, its fourth entry published again as the head: refused, nothing answered.
        store.publish(b"rewritten", 0, &entries[0], OWNER).unwrap();
        for (k, cid) in entries.iter().enumerate().skip(1) { store.publish(b"rewritten", k as u64, cid, OWNER).unwrap(); }
        assert_eq!(answers(&mut store, "rewritten", &x).len(), 6);
        store.publish(b"rewritten", 6, &entries[3], OWNER).unwrap();
        let (r, heard) = about(&mut store, "rewritten", &x, STEPS, &mut area());
        assert_eq!((r, heard.len()), (Err(Error::Journal(Refused { error: journal::Error::Sequence(4), version: 7, at: 7, checked: 0 })), 0));
        let mut j = Writer { name: "rewritten", k: 100 };
        assert!(matches!(j.retract(&mut store, "a", &said[5]), Err(Error::Journal(_))));
        assert_eq!(version(&mut store, "rewritten"), 7);
    }
    // An assertion's block changed on the medium, a byte of its text: the mount finds its record does not open and
    // counts it corrupt, so the store holds it no more, and `about` is refused (`missing`), nothing answered.
    let at = record(&medium, &said[4]) + HEADER;
    let k = (0..MAX).find(|&k| medium.data[at + k..at + k + 5] == *b"tag 4").unwrap();
    medium.data[at + k] = b'T';
    {
        let mut room = Room::new();
        let mut store = mount(&mut medium, &mut room);
        assert_eq!(store.stats().corrupt, 1);
        let (r, heard) = about(&mut store, "claims", &x, STEPS, &mut area());
        assert_eq!((r, heard.len()), (Err(Error::Missing), 0));
        // Every assertion of the journal is read, so the answer about another subject is refused too.
        assert_eq!(about(&mut store, "claims", &Cid::raw(b"another object"), STEPS, &mut area()).0, Err(Error::Missing));
    }
    // An entry's block erased on the medium: the journal does not check, and nothing is answered.
    let entry = record(&medium, &entries[1]);
    let sector = entry / SECTOR * SECTOR;
    medium.data[sector..sector + SECTOR].fill(0);
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let (r, heard) = about(&mut store, "claims", &x, STEPS, &mut area());
    assert_eq!((r, heard.len()), (Err(Error::Journal(Refused { error: journal::Error::Missing, version: 6, at: 2, checked: 4 })), 0));
}

#[test]
fn an_entry_of_kind_assertion_that_holds_none_is_refused_and_one_of_another_kind_is_skipped() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let x = Cid::raw(b"object x");
    let said = encoded(&text(x, "tag", "red"));
    let v2 = { let mut b = said.clone(); b[3] = 2; b };
    assert_eq!(decode(&v2), Err(Error::Version(2)));
    let directory = store.put(Codec::DagCbor, b"\xa2\x61v\x01\x67entries\x80").unwrap();
    let big = { let mut b = Box::new(dag::Builder::new()); b.write(&mut store, &vec![7u8; CHUNK + 1]).unwrap(); b.finish(&mut store).unwrap().0 };
    for (name, kind, content, error) in [("text", "assertion", store.put(Codec::Raw, b"just text").unwrap(), Some(Error::Format)),
                                         ("v2", "assertion", store.put(Codec::Raw, &v2).unwrap(), Some(Error::Version(2))),
                                         ("directory", "assertion", directory, Some(Error::Format)),
                                         ("object", "assertion", big, Some(Error::Format)),
                                         ("other", "notes", store.put(Codec::Raw, &said).unwrap(), None),
                                         ("cased", "Assertion", store.put(Codec::Raw, &said).unwrap(), None)] {
        let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
        let mut j = Writer::new(name);
        j.add(&mut store, "a", &text(x, "tag", "before")).unwrap();
        j.entry_of(&mut store, "a", kind, &content);
        j.add(&mut store, "a", &text(x, "tag", "after")).unwrap();
        let (r, heard) = about(&mut store, name, &x, STEPS, &mut area());
        match error {
            Some(e) => {
                assert_eq!((r, heard.len()), (Err(e), 0), "{name}");
                assert_eq!(j.retract(&mut store, "a", &Cid::raw(&encoded(&text(x, "tag", "after")))), Err(e), "{name}");
            }
            None => assert_eq!(heard.iter().map(|s| s.0).collect::<Vec<_>>(), [3, 1], "{name}"),
        }
    }
}

#[test]
fn about_is_bounded_by_its_work_area() {
    let mut medium = Memory::new();
    let mut room = Room::new();
    let mut store = mount(&mut medium, &mut room);
    let (x, y) = (Cid::raw(b"object x"), Cid::raw(b"object y"));
    let mut j = Writer::new("claims");
    let mut cids = Vec::new();
    for k in 0..5 { cids.push(j.add(&mut store, "a", &text(x, "tag", &format!("tag {k}"))).unwrap().cid); }
    for k in 0..20 { j.add(&mut store, "a", &text(y, "tag", &format!("tag {k}"))).unwrap(); }
    j.retract(&mut store, "a", &cids[1]).unwrap();
    j.retract(&mut store, "a", &cids[3]).unwrap();
    // Three claims stand and two withdrawals are kept: five places; another subject's take none.
    let (r, heard) = about(&mut store, "claims", &x, STEPS, &mut Area::new(4));
    assert_eq!((r, heard.len()), (Err(Error::Full), 0));
    let (r, heard) = about(&mut store, "claims", &x, STEPS, &mut Area::new(5));
    assert_eq!((r, heard.iter().map(|s| s.0).collect::<Vec<_>>()), (Ok(Answered { version: 27, claims: 3 }), vec![5, 3, 1]));
    assert_eq!(about(&mut store, "claims", &x, STEPS, &mut Area::new(0)).0, Err(Error::Full));
    assert_eq!(about(&mut store, "claims", &Cid::raw(b"none"), STEPS, &mut Area::new(0)).0, Ok(Answered { version: 27, claims: 0 }));
}
