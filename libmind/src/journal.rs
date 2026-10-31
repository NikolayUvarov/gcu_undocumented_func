//! A journal, append-only, checked from its head (issue 312-STO-0034, docs/storage; MC-4.3, 4.5, 4.10, 4.12, 10.3).
//! Each entry is one DAG-CBOR block `{"v": 1, "seq": n, "boot": b, "kind": k, "prev": link, "clock": c, "source": s,
//! "content": link, "calendar": t, "monotonic": m}`, its keys in DAG-CBOR's order, linking the entry before it (none for
//! the first, at 1). An append publishes its entry under the journal's name by compare-and-swap from the version read,
//! so an entry's sequence is the name's version: a reader checks order and completeness from the head alone, and a head
//! published again later, a rewrite, is refused. The walk follows the links one after another, never nested, so a
//! journal is longer than `tree::DEPTH`; the block store retains one through it. No heap and no system calls:
//! tests/journal_host.rs.
use crate::cid::{Cid, Codec};
use crate::dag::{self, fetch, Blocks, In, Out, Same, Sink, CHUNK, LINK};
use crate::tree;

/// The entry schema's version (`"v"`); another is refused (MC-4.13).
pub const VERSION: u64 = 1;
/// Bytes of a label (a kind, a source) at most.
pub const LABEL: usize = 32;
/// Bytes of an entry at most: the map, and each key with its largest value.
pub const MAX: usize = 1 + (2 + 1) + (4 + 9) + (5 + 9) + (5 + 2 + LABEL) + (5 + LINK) + (6 + 1) + (7 + 2 + LABEL) + (8 + LINK) + (9 + 9) + (10 + 9);

/// A short printable text: 1 to LABEL bytes of `!` to `~`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label { len: u8, bytes: [u8; LABEL] }
impl Label {
    pub fn new(text: &str) -> Option<Label> {
        let b = text.as_bytes();
        if b.is_empty() || b.len() > LABEL || !b.iter().all(|c| (0x21..0x7f).contains(c)) { return None; }
        let mut l = Label { len: b.len() as u8, bytes: [0; LABEL] };
        l.bytes[..b.len()].copy_from_slice(b);
        Some(l)
    }
    pub fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("") }
}
impl core::fmt::Display for Label { fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { f.write_str(self.as_str()) } }

/// The clock the calendar time came from: none (the time is then 0), the clock chip, the network.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock { None, Chip, Network }
impl Clock {
    const fn code(self) -> u64 { match self { Clock::None => 0, Clock::Chip => 1, Clock::Network => 2 } }
    const fn from_code(code: u64) -> Option<Self> { match code { 0 => Some(Clock::None), 1 => Some(Clock::Chip), 2 => Some(Clock::Network), _ => None } }
}

/// When an entry was written: the boot, as the journal counts them from 1; the monotonic time in it, in nanoseconds;
/// and the calendar time, seconds since 1970-01-01 UTC, with its clock. The calendar time is the writer's claim, and a
/// check never orders by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Time { pub boot: u64, pub monotonic: u64, pub calendar: u64, pub clock: Clock }

/// An entry: its sequence from 1, its time, the program that wrote it, a kind, the content's CID, and the entry before.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry { pub seq: u64, pub time: Time, pub source: Label, pub kind: Label, pub content: Cid, pub prev: Option<Cid> }

impl Entry {
    // From 1, linking an entry (`dag-cbor`) unless the first; a calendar time only with a clock.
    fn valid(&self) -> bool {
        self.seq >= 1 && self.prev.is_some() == (self.seq > 1) && self.prev.is_none_or(|p| p.codec() == Codec::DagCbor)
            && (self.time.clock != Clock::None || self.time.calendar == 0)
    }

    /// Encodes the entry into `out` (MAX bytes at least); returns its length. One that makes no valid entry is Format.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, dag::Error> {
        if !self.valid() { return Err(dag::Error::Format); }
        let mut o = Out { bytes: out, at: 0 };
        encode_with(self, &mut o);
        Ok(o.at)
    }
}

// The keys in DAG-CBOR's order: shorter first, then by their bytes.
fn encode_with(e: &Entry, o: &mut impl Sink) {
    o.head(5, if e.prev.is_some() { 10 } else { 9 });
    o.text("v");
    o.head(0, VERSION);
    o.text("seq");
    o.head(0, e.seq);
    o.text("boot");
    o.head(0, e.time.boot);
    o.text("kind");
    o.text(e.kind.as_str());
    if let Some(prev) = &e.prev {
        o.text("prev");
        o.link(prev);
    }
    o.text("clock");
    o.head(0, e.time.clock.code());
    o.text("source");
    o.text(e.source.as_str());
    o.text("content");
    o.link(&e.content);
    o.text("calendar");
    o.head(0, e.time.calendar);
    o.text("monotonic");
    o.head(0, e.time.monotonic);
}

fn label(i: &mut In) -> Result<Label, dag::Error> {
    let len = i.expect(3)? as usize;
    core::str::from_utf8(i.take(len)?).ok().and_then(Label::new).ok_or(dag::Error::Format)
}

/// Decodes an entry, only if it is exactly the canonical encoding of this schema.
pub fn decode(bytes: &[u8]) -> Result<Entry, dag::Error> {
    let mut i = In { bytes, at: 0 };
    let keys = i.expect(5)?;
    // "v" sorts first, so another schema version is told apart before anything else of it is read.
    i.key("v")?;
    if i.expect(0)? != VERSION { return Err(dag::Error::Version); }
    if keys != 9 && keys != 10 { return Err(dag::Error::Format); }
    i.key("seq")?;
    let seq = i.expect(0)?;
    i.key("boot")?;
    let boot = i.expect(0)?;
    i.key("kind")?;
    let kind = label(&mut i)?;
    let prev = if keys == 10 { i.key("prev")?; Some(i.link()?) } else { None };
    i.key("clock")?;
    let clock = Clock::from_code(i.expect(0)?).ok_or(dag::Error::Format)?;
    i.key("source")?;
    let source = label(&mut i)?;
    i.key("content")?;
    let content = i.link()?;
    i.key("calendar")?;
    let calendar = i.expect(0)?;
    i.key("monotonic")?;
    let monotonic = i.expect(0)?;
    let e = Entry { seq, time: Time { boot, monotonic, calendar, clock }, source, kind, content, prev };
    if i.at != bytes.len() || !e.valid() { return Err(dag::Error::Format); }
    // Shortest integer forms: the encoding of what was read must be the bytes themselves.
    let mut same = Same { bytes, at: 0, equal: true };
    encode_with(&e, &mut same);
    if !same.equal || same.at != bytes.len() { return Err(dag::Error::Format); }
    Ok(e)
}

/// Visits every block under `root`: a journal's entries from `root` down to the first, each followed by its content's
/// blocks (`tree::walk`); for any other root, the blocks `tree::walk` visits. The entries are followed one after
/// another, never nested, so `buffer` is the only block held whatever the journal's length. Each entry is checked
/// against its CID and the schema, and the one it links against its sequence (Shape). Returns the bytes of the contents'
/// files; TooLarge once it has taken `steps` steps (an entry visited, and the contents' steps).
pub fn walk<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK], steps: u64, visit: impl FnMut(&mut B, &Cid) -> Result<(), dag::Error>) -> Result<u64, dag::Error> {
    tour(blocks, root, buffer, true, steps, visit)
}

/// The bytes of the contents' files under `root`, as `walk` returns them, from the entries and each content's
/// directories and root alone (`tree::size`), within `steps` steps.
pub fn size<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK], steps: u64) -> Result<u64, dag::Error> {
    tour(blocks, root, buffer, false, steps, |_, _| Ok(()))
}

// The entry `cid` names, read and checked against its CID; none if it names no entry.
fn read<B: Blocks>(blocks: &mut B, cid: &Cid, buffer: &mut [u8; CHUNK]) -> Result<Option<Entry>, dag::Error> {
    if cid.codec() != Codec::DagCbor { return Ok(None); }
    let len = fetch(blocks, cid, buffer)?;
    Ok(decode(&buffer[..len]).ok())
}

fn tour<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK], whole: bool, steps: u64, mut visit: impl FnMut(&mut B, &Cid) -> Result<(), dag::Error>) -> Result<u64, dag::Error> {
    let Some(mut entry) = read(blocks, root, buffer)? else {
        return if whole { tree::walk(blocks, root, buffer, steps, visit) } else { tree::size(blocks, root, buffer, steps) };
    };
    let (mut cid, mut left, mut total) = (*root, steps, 0u64);
    loop {
        left = left.checked_sub(1).ok_or(dag::Error::TooLarge)?;
        visit(blocks, &cid)?;
        // A content is a file or a directory: another entry is not one (Format).
        let held = if whole { tree::walk_from(blocks, &entry.content, buffer, &mut left, &mut visit)? } else { tree::size_from(blocks, &entry.content, buffer, &mut left)? };
        total = total.checked_add(held).ok_or(dag::Error::TooLarge)?;
        let Some(prev) = entry.prev else { return Ok(total) };
        // Read now, the content's walk done with the buffer: the entry before, one less in sequence (the schema's
        // `prev` is only above 1).
        let before = read(blocks, &prev, buffer)?.ok_or(dag::Error::Shape)?;
        if before.seq != entry.seq - 1 { return Err(dag::Error::Shape); }
        (cid, entry) = (prev, before);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The store refused or failed; its own error is the caller's to keep.
    Store,
    /// No such name.
    NotFound,
    /// The name was removed at this version: its journal ended there, and none goes on under it.
    Removed(u64),
    /// Another writer published since the head was read: nothing was published; append again, after the new head.
    Conflict,
    /// A block of the journal the store does not hold.
    Missing,
    /// A block whose bytes do not match its CID.
    Corrupt,
    /// A block that is not an entry of this schema.
    Format,
    /// An entry of another schema version.
    Version,
    /// An entry of this sequence where another belongs: at the head, the name's version; below, one less each.
    Sequence(u64),
    /// A boot that goes back, or a monotonic time that goes back within one boot.
    Time,
    /// A kind or a source that is no label, or a calendar time without a clock.
    Invalid,
    /// More entries than the steps given.
    TooLarge,
}

/// The store as a journal needs it: blocks, and a name read and published by compare-and-swap.
pub trait Store {
    /// Stores an entry, a `dag-cbor` block; returns its CID.
    fn put(&mut self, entry: &[u8]) -> Result<Cid, Error>;
    /// Copies the block `cid` names into `out` and returns its length: Missing if the store holds none, Corrupt if what
    /// it holds is damaged.
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error>;
    /// The name's current version and root: 0 and none for no such name, a version and none for a removed one.
    fn head(&mut self, name: &str) -> Result<(u64, Option<Cid>), Error>;
    /// Makes `root` the name's next version if it is at `expected`, once every block under it is stored (Missing);
    /// Conflict, nothing published, if it is not.
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<u64, Error>;
}

// The entry `cid` names, read from the store and checked: the store is another process, and the reader does not trust it.
fn entry(store: &mut impl Store, cid: &Cid, buffer: &mut [u8; CHUNK]) -> Result<Entry, Error> {
    let len = store.get(cid, buffer)?;
    if !cid.matches(&buffer[..len]) { return Err(Error::Corrupt); }
    if cid.codec() != Codec::DagCbor { return Err(Error::Format); }
    decode(&buffer[..len]).map_err(|e| if e == dag::Error::Version { Error::Version } else { Error::Format })
}

/// The writer's reading of its clocks: the monotonic time, and the calendar time with the clock that gave it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reading { pub monotonic: u64, pub calendar: u64, pub clock: Clock }

/// What an append published: the entry, whose sequence is the name's new version, and its CID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Appended { pub entry: Entry, pub cid: Cid }

/// Appends an entry of `kind` from `source` for `content`, an object already stored, after the head of `name`, and
/// publishes it by compare-and-swap from the version read: its sequence is that version plus one. Its boot is the
/// head's, or the next when the head's monotonic time is past `now`'s (the clock started again); the first entry's
/// is 1. Conflict if another writer published since: nothing is published, and an append again reads the new head. A
/// head the check would refuse at once is refused (Sequence, Format, Version), and a removed name (Removed). `buffer`
/// holds a block.
pub fn append(store: &mut impl Store, name: &str, now: Reading, source: &str, kind: &str, content: &Cid, buffer: &mut [u8; CHUNK]) -> Result<Appended, Error> {
    let (source, kind) = (Label::new(source).ok_or(Error::Invalid)?, Label::new(kind).ok_or(Error::Invalid)?);
    let (version, root) = store.head(name)?;
    let (boot, prev) = match root {
        None if version > 0 => return Err(Error::Removed(version)),
        None => (1, None),
        Some(head) => {
            let e = entry(store, &head, buffer)?;
            if e.seq != version { return Err(Error::Sequence(e.seq)); }
            let boot = if e.time.monotonic > now.monotonic { e.time.boot.checked_add(1).ok_or(Error::Time)? } else { e.time.boot };
            (boot, Some(head))
        }
    };
    let time = Time { boot, monotonic: now.monotonic, calendar: now.calendar, clock: now.clock };
    let e = Entry { seq: version + 1, time, source, kind, content: *content, prev };
    let len = e.encode(&mut buffer[..]).map_err(|_| Error::Invalid)?;
    let cid = store.put(&buffer[..len])?;
    // The store's answer is checked, as `dag::Builder` checks it.
    if cid != Cid::of(Codec::DagCbor, &buffer[..len]) { return Err(Error::Corrupt); }
    store.publish(name, version, &cid)?;
    Ok(Appended { entry: e, cid })
}

/// What a check or a listing read: the name's version, the entries read and checked, and the sequences of the lowest
/// and the highest of them (with none read, the lowest is one past the version).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checked { pub version: u64, pub entries: u64, pub first: u64, pub last: u64 }

/// Why a check stopped, and where: the name's version, the sequence it was at, and the entries checked before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused { pub error: Error, pub version: u64, pub at: u64, pub checked: u64 }

/// Checks the journal under `name` from its head: the name's version and root read, then each entry down to the first
/// within `steps` entries (else TooLarge), each read and checked against its CID and the schema. The head's sequence must
/// be the version, and each entry below one less, down to 1; an entry's boot never past the boot of the entry after it,
/// nor within one boot its monotonic time. So a head published again at a later version, a rewrite, is refused, and so
/// is a journal whose name was removed and published again. The contents are not read. `buffer` holds a block.
pub fn check(store: &mut impl Store, name: &str, steps: u64, buffer: &mut [u8; CHUNK]) -> Result<Checked, Refused> {
    check_each(store, name, steps, buffer, |_, _, _| {})
}

/// As `check`, `each` hearing the store, each entry and its CID as it is read (an index's build and search, 312-STO-0035).
pub fn check_each<S: Store>(store: &mut S, name: &str, steps: u64, buffer: &mut [u8; CHUNK], each: impl FnMut(&mut S, &Entry, &Cid)) -> Result<Checked, Refused> {
    let done = down(store, name, steps, buffer, each)?;
    if done.first != 1 { return Err(Refused { error: Error::TooLarge, version: done.version, at: done.first - 1, checked: done.entries }); }
    Ok(done)
}

/// The last `most` entries of `name`, newest first, each checked as `check` checks it; `each` hears the store, each
/// entry and its CID. `buffer` holds a block.
pub fn list<S: Store>(store: &mut S, name: &str, most: u64, buffer: &mut [u8; CHUNK], each: impl FnMut(&mut S, &Entry, &Cid)) -> Result<Checked, Refused> {
    down(store, name, most, buffer, each)
}

// Reads at most `most` entries from the head down, each checked against the one read before it.
fn down<S: Store>(store: &mut S, name: &str, most: u64, buffer: &mut [u8; CHUNK], mut each: impl FnMut(&mut S, &Entry, &Cid)) -> Result<Checked, Refused> {
    let (version, root) = store.head(name).map_err(|error| Refused { error, version: 0, at: 0, checked: 0 })?;
    let Some(mut cid) = root else {
        let error = if version == 0 { Error::NotFound } else { Error::Removed(version) };
        return Err(Refused { error, version, at: version, checked: 0 });
    };
    let mut done = Checked { version, entries: 0, first: version + 1, last: version };
    let mut after: Option<Time> = None;
    while done.entries < most {
        let at = version - done.entries;
        let refused = |error| Refused { error, version, at, checked: done.entries };
        let e = entry(store, &cid, buffer).map_err(refused)?;
        if e.seq != at { return Err(refused(Error::Sequence(e.seq))); }
        if after.is_some_and(|t| e.time.boot > t.boot || (e.time.boot == t.boot && e.time.monotonic > t.monotonic)) { return Err(refused(Error::Time)); }
        each(store, &e, &cid);
        (done.entries, done.first) = (done.entries + 1, e.seq);
        // The first has none before it (the schema's), every other one has.
        let Some(prev) = e.prev else { break };
        (cid, after) = (prev, Some(e.time));
    }
    Ok(done)
}
