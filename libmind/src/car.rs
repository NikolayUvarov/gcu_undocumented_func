//! Copies of named roots in a file (issue 309-STO-0031; MC-4.4, 4.8, 6.8): a CAR v1, the IPLD content archive, so other
//! tools read it. A header (a varint length, then DAG-CBOR `{"roots": [cid], "version": 1}`) whose one root is the
//! names node; then sections, each a varint length, a CID and its block: the names node first, then every block under
//! each name's root in the order `journal::walk` visits them (a journal's, or `tree::walk`'s for any other root). The
//! names node is `{"v": 1, "names": [[name, version, root], ...]}`, sorted by name. A copy is checked whole before
//! anything of it is put: the header and the names node only if exactly their encoding, every block against its CID,
//! and each root walked over the copy's own sections, so a copy cut short or missing a block is refused. An import puts
//! each block, a copy the store holds damaged replaced, and publishes each name by compare-and-swap, never over another
//! root. No heap and no system calls: tests/car_host.rs.
use crate::cid::{self, Cid, Codec};
use crate::dag::{self, Blocks, In, Out, Same, Sink, CHUNK, LINK};
use crate::journal;
use crate::sha256::{Sha256, DIGEST};
use crate::worktree::Names;

/// The CAR format's version.
pub const VERSION: u64 = 1;
/// The names node's schema version (`"v"`); another is refused (MC-4.13).
pub const SCHEMA: u64 = 1;
/// Names in a copy at most: as many as the store's `snapshot` reads at one point.
pub const NAMES: usize = 8;
/// Bytes of a name at most, as the store takes it.
pub const NAME_MAX: usize = 64;
/// Bytes of a header after its length: the map, "roots", one link, "version" and 1.
pub const HEADER: usize = 1 + 6 + 1 + LINK + 8 + 1;
/// Bytes of a section after its length at most: a CID and a block.
pub const SECTION_MAX: usize = cid::BYTES + CHUNK;
/// Bytes of the largest names node: the map, "v", "names", and NAMES names of NAME_MAX bytes.
pub const NODE_MAX: usize = 1 + 2 + 1 + 6 + 1 + NAMES * (1 + 2 + NAME_MAX + 9 + LINK);
// The `dag-cbor` sections a check remembers where it met, for the nodes a walk reads again.
const SEEN: usize = 32;
// A section's length and CID at most: what is read to find where its block starts.
const LEAD: usize = 9 + cid::BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The copy ends inside its header or a section.
    Truncated,
    /// A header, a names node or a varint that is not exactly its encoding (names unsorted or twice among it), or a
    /// section's length that cuts its CID.
    Format,
    /// A CAR of another version, or a names node of another schema version.
    Version,
    /// A section longer than a CID and a block (SECTION_MAX).
    TooLarge,
    /// A CID that is not a supported one: another version, content type or hash (MC-4.13).
    Cid(cid::Error),
    /// A block whose bytes do not match its CID.
    Corrupt,
    /// A block out of the schema or the shape its place under a root needs.
    Block,
    /// The copy ends where a walk under a root needs a block; to export, a name that does not exist or is removed.
    Missing,
    /// A section other than the next one under the names' roots: the names node not first, a block out of the walk's
    /// order, or a section after the last.
    Order,
    /// A name the store does not take or at version 0, none or more than NAMES of them, or one given twice to export.
    Name,
    /// The copy read for the import is not the one checked: nothing was published.
    Changed,
    /// The copy could not be read or written.
    File,
    /// The store refused or failed.
    Store(dag::Error),
}

/// Why a copy was refused, and where: the offset of the section it was refused at (0: its header).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused { pub error: Error, pub at: u64 }

/// Where a copy is read from: a file, or memory.
pub trait Source {
    /// Copies the copy's bytes from `at` into `out`; returns how many, fewer only at its end.
    fn read_at(&mut self, at: u64, out: &mut [u8]) -> Result<usize, Error>;
}
impl<S: Source + ?Sized> Source for &mut S {
    fn read_at(&mut self, at: u64, out: &mut [u8]) -> Result<usize, Error> { (**self).read_at(at, out) }
}
impl Source for &[u8] {
    fn read_at(&mut self, at: u64, out: &mut [u8]) -> Result<usize, Error> {
        let from = at.min(self.len() as u64) as usize;
        let n = out.len().min(self.len() - from);
        out[..n].copy_from_slice(&self[from..from + n]);
        Ok(n)
    }
}

/// Where a copy is written: a file, or memory.
pub trait Target {
    fn write(&mut self, data: &[u8]) -> Result<(), Error>;
}
impl<T: Target + ?Sized> Target for &mut T {
    fn write(&mut self, data: &[u8]) -> Result<(), Error> { (**self).write(data) }
}

/// A block store as copies need it: its blocks, its names' compare-and-swap (`worktree::Names`), names read at one
/// point, and a collection.
pub trait Store: Blocks + Names {
    /// The current version and root of each of `names` (at most NAMES) into `heads`, read at one point: 0 and none for
    /// a name that does not exist, none for a removed one.
    fn snapshot(&mut self, names: &[&str], heads: &mut [(u64, Option<Cid>)]) -> Result<(), dag::Error>;
    /// Frees what nothing retains, a damaged copy a put replaced among it; returns the blocks freed.
    fn collect(&mut self) -> Result<u64, dag::Error>;
}

// An unsigned varint (multiformats) in its shortest form: its bytes and how many.
fn varint(mut value: u64) -> ([u8; 10], usize) {
    let (mut out, mut n) = ([0u8; 10], 0);
    loop {
        out[n] = value as u8 & 0x7f;
        value >>= 7;
        if value == 0 { return (out, n + 1); }
        out[n] |= 0x80;
        n += 1;
    }
}

// A varint read at the start of `bytes`, which the copy's end may cut short.
fn length(bytes: &[u8]) -> Result<(u64, usize), Error> {
    cid::varint(bytes).map_err(|e| if e == cid::Error::Truncated { Error::Truncated } else { Error::Format })
}

// CBOR the reader refused: not the encoding it must be.
fn format(_: dag::Error) -> Error { Error::Format }

// A header naming `root`, its keys in DAG-CBOR's order.
fn header(root: &Cid, o: &mut impl Sink) {
    o.head(5, 2);
    o.text("roots");
    o.head(4, 1);
    o.link(root);
    o.text("version");
    o.head(0, VERSION);
}

// The root of a header, only if it is exactly the encoding `header` writes, naming a `dag-cbor` block.
fn header_root(bytes: &[u8]) -> Result<Cid, Error> {
    let mut i = In { bytes, at: 0 };
    let keys = i.expect(5).map_err(format)?;
    // A CAR v2 begins with a header `{"version": 2}`.
    if keys == 1 {
        i.key("version").map_err(format)?;
        return Err(if i.expect(0).map_err(format)? != VERSION { Error::Version } else { Error::Format });
    }
    if keys != 2 { return Err(Error::Format); }
    i.key("roots").map_err(format)?;
    if i.expect(4).map_err(format)? != 1 { return Err(Error::Format); }
    let root = i.link().map_err(format)?;
    i.key("version").map_err(format)?;
    if i.expect(0).map_err(format)? != VERSION { return Err(Error::Version); }
    // Shortest forms: the encoding of what was read must be the bytes themselves.
    let mut same = Same { bytes, at: 0, equal: true };
    header(&root, &mut same);
    if !same.equal || same.at != bytes.len() || root.codec() != Codec::DagCbor { return Err(Error::Format); }
    Ok(root)
}

// The section at `at`: its CID, where its block starts and its length, and where the next section starts; none at the
// end of the copy.
fn section<S: Source>(source: &mut S, at: u64) -> Result<Option<(Cid, u64, usize, u64)>, Error> {
    let mut lead = [0u8; LEAD];
    let got = source.read_at(at, &mut lead)?;
    if got == 0 { return Ok(None); }
    let (len, n) = length(&lead[..got.min(9)])?;
    if len > SECTION_MAX as u64 { return Err(Error::TooLarge); }
    let (read, want) = (&lead[n..got], (len as usize).min(cid::BYTES));
    let (cid, taken) = match Cid::read(&read[..want.min(read.len())]) {
        Ok(found) => found,
        // Cut by the copy's end, or by the section's own length.
        Err(cid::Error::Truncated) if read.len() < want => return Err(Error::Truncated),
        Err(cid::Error::Truncated) => return Err(Error::Format),
        Err(e) => return Err(Error::Cid(e)),
    };
    Ok(Some((cid, at + (n + taken) as u64, len as usize - taken, at + n as u64 + len)))
}

/// Reads a copy's sections in order, each checked against its CID, and keeps the SHA-256 of what it read.
pub struct Reader<S> { source: S, root: Cid, at: u64, hash: Sha256 }

impl<S: Source> Reader<S> {
    /// Reads and checks the header: exactly `{"roots": [cid], "version": 1}`, its one root a `dag-cbor` CID.
    pub fn open(mut source: S) -> Result<Self, Error> {
        let mut lead = [0u8; 9];
        let got = source.read_at(0, &mut lead)?;
        let (len, n) = length(&lead[..got])?;
        if len > HEADER as u64 { return Err(Error::Format); }
        let mut bytes = [0u8; HEADER];
        let head = &mut bytes[..len as usize];
        if source.read_at(n as u64, head)? < head.len() { return Err(Error::Truncated); }
        let root = header_root(head)?;
        let mut hash = Sha256::new();
        hash.update(&lead[..n]);
        hash.update(head);
        Ok(Reader { source, root, at: (n + head.len()) as u64, hash })
    }

    /// The root the header names: the names node's CID.
    pub fn root(&self) -> Cid { self.root }
    /// Where the next section starts; past the last, the copy's length.
    pub fn at(&self) -> u64 { self.at }
    /// The SHA-256 of the copy's bytes read so far.
    pub fn digest(&self) -> [u8; DIGEST] { self.hash.clone().finish() }

    /// The next section's CID and block, read into `block` and checked against the CID; none at the end of the copy.
    pub fn next(&mut self, block: &mut [u8; CHUNK]) -> Result<Option<(Cid, usize)>, Error> {
        let Some((cid, start, len, end)) = section(&mut self.source, self.at)? else { return Ok(None) };
        if self.source.read_at(start, &mut block[..len])? < len { return Err(Error::Truncated); }
        if !cid.matches(&block[..len]) { return Err(Error::Corrupt); }
        // Each read in its one encoding, so writing it again gives the bytes read.
        let (lead, n) = varint((cid::BYTES + len) as u64);
        self.hash.update(&lead[..n]);
        self.hash.update(&cid.to_bytes());
        self.hash.update(&block[..len]);
        self.at = end;
        Ok(Some((cid, len)))
    }
}

/// Writes a copy: its header, then its sections; keeps the SHA-256 of what it wrote.
pub struct Writer<T> { target: T, bytes: u64, hash: Sha256 }

impl<T: Target> Writer<T> {
    /// Writes the header naming `root`.
    pub fn new(target: T, root: &Cid) -> Result<Self, Error> {
        let mut w = Writer { target, bytes: 0, hash: Sha256::new() };
        let mut head = [0u8; HEADER];
        header(root, &mut Out { bytes: &mut head, at: 0 });
        let (lead, n) = varint(HEADER as u64);
        w.put(&lead[..n])?;
        w.put(&head)?;
        Ok(w)
    }

    /// Writes the section of the block `cid` names, at most CHUNK bytes.
    pub fn section(&mut self, cid: &Cid, block: &[u8]) -> Result<(), Error> {
        if block.len() > CHUNK { return Err(Error::TooLarge); }
        let (length, n) = varint((cid::BYTES + block.len()) as u64);
        let mut lead = [0u8; 10 + cid::BYTES];
        lead[..n].copy_from_slice(&length[..n]);
        lead[n..n + cid::BYTES].copy_from_slice(&cid.to_bytes());
        self.put(&lead[..n + cid::BYTES])?;
        self.put(block)
    }

    /// The target, the bytes written and their SHA-256.
    pub fn done(self) -> (T, u64, [u8; DIGEST]) { (self.target, self.bytes, self.hash.finish()) }

    fn put(&mut self, data: &[u8]) -> Result<(), Error> {
        self.target.write(data)?;
        self.hash.update(data);
        self.bytes += data.len() as u64;
        Ok(())
    }
}

/// A name in a copy: the version it was at in the store it was exported from, and its root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Named<'a> { pub name: &'a str, pub version: u64, pub root: Cid }

/// Whether `name` is one the store takes: 1 to NAME_MAX bytes of `A-Z a-z 0-9 . _ / -`.
pub fn valid_name(name: &[u8]) -> bool {
    (1..=NAME_MAX).contains(&name.len()) && name.iter().all(|&c| c.is_ascii_alphanumeric() || b"._/-".contains(&c))
}

// A name the store takes, at a version it published.
fn valid(n: &Named) -> bool { valid_name(n.name.as_bytes()) && n.version >= 1 }

/// Encodes the names node of `names` into `out` and returns its length: 1 to NAMES of them, sorted by name with each
/// once, each a name the store takes at a version from 1 (else Name).
pub fn encode_names(names: &[Named], out: &mut [u8; NODE_MAX]) -> Result<usize, Error> { encode_each(names.len(), |k| names[k], out) }

fn encode_each<'a>(count: usize, named: impl Fn(usize) -> Named<'a>, out: &mut [u8; NODE_MAX]) -> Result<usize, Error> {
    if !(1..=NAMES).contains(&count) || !(0..count).all(|k| valid(&named(k))) || (1..count).any(|k| named(k - 1).name >= named(k).name) {
        return Err(Error::Name);
    }
    let mut o = Out { bytes: out, at: 0 };
    names_with(count, named, &mut o);
    Ok(o.at)
}

// The names come one by one, so the decoder's check builds no array of them. The keys are in DAG-CBOR's order.
fn names_with<'a>(count: usize, named: impl Fn(usize) -> Named<'a>, o: &mut impl Sink) {
    o.head(5, 2);
    o.text("v");
    o.head(0, SCHEMA);
    o.text("names");
    o.head(4, count as u64);
    for k in 0..count {
        let n = named(k);
        o.head(4, 3);
        o.text(n.name);
        o.head(0, n.version);
        o.link(&n.root);
    }
}

// Reads a name, its version and its root, and checks them.
fn named<'a>(i: &mut In<'a>) -> Result<Named<'a>, Error> {
    if i.expect(4).map_err(format)? != 3 { return Err(Error::Format); }
    let len = i.expect(3).map_err(format)? as usize;
    let name = core::str::from_utf8(i.take(len).map_err(format)?).map_err(|_| Error::Name)?;
    let version = i.expect(0).map_err(format)?;
    let n = Named { name, version, root: i.link().map_err(format)? };
    if !valid(&n) { return Err(Error::Name); }
    Ok(n)
}

/// A decoded names node: its names read from its bytes when asked for, in the order of their bytes.
pub struct Roots<'a> { bytes: &'a [u8], count: usize, starts: [u16; NAMES] }

impl<'a> Roots<'a> {
    pub fn len(&self) -> usize { self.count }
    pub fn is_empty(&self) -> bool { self.count == 0 }
    // Every name was read and checked by `decode_names`.
    pub fn get(&self, i: usize) -> Named<'a> { named(&mut In { bytes: self.bytes, at: self.starts[i] as usize }).unwrap() }
}

/// Decodes a names node, only if it is exactly the canonical encoding of this schema.
pub fn decode_names(bytes: &[u8]) -> Result<Roots<'_>, Error> {
    if bytes.len() > NODE_MAX { return Err(Error::Format); }
    let mut i = In { bytes, at: 0 };
    let keys = i.expect(5).map_err(format)?;
    // "v" sorts first, so another schema version is told apart before anything else of it is read.
    i.key("v").map_err(format)?;
    if i.expect(0).map_err(format)? != SCHEMA { return Err(Error::Version); }
    if keys != 2 { return Err(Error::Format); }
    i.key("names").map_err(format)?;
    let count = i.expect(4).map_err(format)?;
    if !(1..=NAMES as u64).contains(&count) { return Err(Error::Name); }
    let mut roots = Roots { bytes, count: count as usize, starts: [0; NAMES] };
    let mut last = "";
    for k in 0..roots.count {
        roots.starts[k] = i.at as u16;
        let n = named(&mut i)?;
        // Sorted by the names' bytes, each name once.
        if k > 0 && n.name <= last { return Err(Error::Format); }
        last = n.name;
    }
    if i.at != bytes.len() { return Err(Error::Format); }
    // Shortest forms: the encoding of what was read must be the bytes themselves.
    let mut same = Same { bytes, at: 0, equal: true };
    names_with(roots.count, |k| roots.get(k), &mut same);
    if !same.equal || same.at != bytes.len() { return Err(Error::Format); }
    Ok(roots)
}

// A copy's blocks as a walk under a root reads them. Each block the walk visits must be the copy's next section, read
// whole and checked; one it reads is found among the sections met: the last, the last SEEN `dag-cbor` ones (a walk reads
// its directories and nodes again), the next (a walk reads its root first), else looked for from the first.
struct Sections<'r, S> {
    reader: &'r mut Reader<S>, first: u64, block: &'r mut [u8; CHUNK], visited: u64, failed: Option<Error>,
    last: ([u8; cid::BYTES], u64), seen: [([u8; cid::BYTES], u64, u64); SEEN], clock: u64,
}

impl<S: Source> Sections<'_, S> {
    // The walk visits `cid`: the next section must be its block.
    fn visit(&mut self, cid: &Cid) -> Result<(), dag::Error> {
        let at = self.reader.at;
        match self.reader.next(&mut *self.block) {
            Ok(Some((next, _))) if next == *cid => {
                self.visited += 1;
                self.last = (next.to_bytes(), at);
                if next.codec() == Codec::DagCbor { self.remember(next.to_bytes(), at); }
                Ok(())
            }
            Ok(Some(_)) => self.fail(Error::Order),
            Ok(None) => self.fail(Error::Missing),
            Err(e) => self.fail(e),
        }
    }

    // A refusal of the copy's, kept: the walk hears only that its store failed.
    fn fail<T>(&mut self, e: Error) -> Result<T, dag::Error> {
        self.failed = Some(e);
        Err(dag::Error::Store)
    }

    // The section at `at` holds `key`; the oldest one remembered gives way.
    fn remember(&mut self, key: [u8; cid::BYTES], at: u64) {
        self.clock += 1;
        let slot = self.seen.iter().position(|s| s.0 == key).unwrap_or_else(|| (0..SEEN).min_by_key(|&k| self.seen[k].2).unwrap());
        self.seen[slot] = (key, at, self.clock);
    }

    // Where the section of `cid` starts, among those met and the next.
    fn find(&mut self, cid: &Cid) -> Result<Option<u64>, Error> {
        let key = cid.to_bytes();
        if self.last.0 == key { return Ok(Some(self.last.1)); }
        if let Some(&(_, at, _)) = self.seen.iter().find(|s| s.0 == key) {
            self.remember(key, at);
            return Ok(Some(at));
        }
        let next = self.reader.at;
        if section(&mut self.reader.source, next)?.is_some_and(|s| s.0 == *cid) { return Ok(Some(next)); }
        let mut at = self.first;
        while at < next {
            let Some((found, _, _, after)) = section(&mut self.reader.source, at)? else { break };
            if found == *cid {
                if found.codec() == Codec::DagCbor { self.remember(key, at); }
                return Ok(Some(at));
            }
            at = after;
        }
        Ok(None)
    }

    fn read(&mut self, cid: &Cid, out: &mut [u8]) -> Result<Option<usize>, Error> {
        let Some(at) = self.find(cid)? else { return Ok(None) };
        let Some((_, start, len, _)) = section(&mut self.reader.source, at)? else { return Ok(None) };
        let out = out.get_mut(..len).ok_or(Error::TooLarge)?;
        if self.reader.source.read_at(start, out)? < len { return Err(Error::Truncated); }
        Ok(Some(len))
    }
}

// What the walk reads is checked against its CID by the walk itself.
impl<S: Source> Blocks for Sections<'_, S> {
    fn put(&mut self, _: Codec, _: &[u8]) -> Result<Cid, dag::Error> { Err(dag::Error::Store) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> {
        match self.read(cid, out) { Ok(Some(len)) => Ok(len), Ok(None) => Err(dag::Error::NotFound), Err(e) => self.fail(e) }
    }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> {
        match self.find(cid) { Ok(found) => Ok(found.is_some()), Err(e) => self.fail(e) }
    }
}

/// What a check found: the names, the blocks under their roots (a block met twice counted twice), the copy's bytes and
/// their SHA-256.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checked { pub names: usize, pub blocks: u64, pub bytes: u64, pub digest: [u8; DIGEST] }

// Checks a whole copy, as `check` does, its names node kept in `node`; the node's length and what was found.
fn verify<S: Source>(source: S, buffer: &mut [u8; CHUNK], block: &mut [u8; CHUNK], node: &mut [u8; NODE_MAX]) -> Result<(usize, Checked), Refused> {
    let mut reader = Reader::open(source).map_err(|error| Refused { error, at: 0 })?;
    let at = reader.at;
    let refused = |error| Refused { error, at };
    let len = match reader.next(block).map_err(refused)? {
        Some((cid, _)) if cid != reader.root => return Err(refused(Error::Order)),
        Some((_, len)) if len > NODE_MAX => return Err(refused(Error::Format)),
        Some((_, len)) => len,
        None => return Err(refused(Error::Missing)),
    };
    node[..len].copy_from_slice(&block[..len]);
    let roots = decode_names(&node[..len]).map_err(refused)?;
    let first = reader.at;
    let mut sections = Sections {
        reader: &mut reader, first, block: &mut *block, visited: 0, failed: None,
        last: ([0; cid::BYTES], 0), seen: [([0; cid::BYTES], 0, 0); SEEN], clock: 0,
    };
    for k in 0..roots.len() {
        // Every step takes a section or fails, so the copy bounds the walk.
        if let Err(e) = journal::walk(&mut sections, &roots.get(k).root, buffer, u64::MAX, |s, cid| s.visit(cid)) {
            let error = sections.failed.take().unwrap_or(match e { dag::Error::NotFound => Error::Missing, dag::Error::Corrupt => Error::Corrupt, _ => Error::Block });
            return Err(Refused { error, at: sections.reader.at });
        }
    }
    let blocks = sections.visited;
    let at = reader.at;
    match reader.next(block) {
        Ok(None) => {}
        Ok(Some(_)) => return Err(Refused { error: Error::Order, at }),
        Err(error) => return Err(Refused { error, at }),
    }
    Ok((len, Checked { names: roots.len(), blocks, bytes: reader.at, digest: reader.digest() }))
}

/// Reads and checks a whole copy, putting nothing: the header and the names node, each exactly its encoding; each name's
/// root walked (`journal::walk`) over the copy's sections, each the next one the walk visits, read whole and checked against
/// its CID, and each node against its schema and place; then the copy's end. `each` hears each name. `buffer` and `block`
/// hold a block each.
pub fn check<S: Source>(source: S, buffer: &mut [u8; CHUNK], block: &mut [u8; CHUNK], mut each: impl FnMut(Named)) -> Result<Checked, Refused> {
    let mut node = [0u8; NODE_MAX];
    let (len, checked) = verify(source, buffer, block, &mut node)?;
    let roots = decode_names(&node[..len]).map_err(|error| Refused { error, at: 0 })?;
    for k in 0..roots.len() { each(roots.get(k)); }
    Ok(checked)
}

/// What became of a name in an import.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Published at the copy's root, from the version it was at (0: a new name; a removal's): its new version.
    Published(u64),
    /// At the copy's root already, at this version: nothing published.
    Same(u64),
    /// At another root, left as it is (a conflict, never overwritten): its version and root.
    Conflict { version: u64, root: Option<Cid> },
}

/// What an import did: the copy checked; its blocks the store had not, had damaged (repaired) and had whole; and, after
/// a repair, the collection that freed the damaged copies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Imported { pub checked: Checked, pub stored: u64, pub repaired: u64, pub held: u64, pub collected: Option<Result<u64, dag::Error>> }

/// Imports a copy into `store`. It is checked whole first (`check`), and nothing is put if any of it is refused. Then
/// each block is put, read from the store first: a damaged copy leaves the store's index, so the put stores it again.
/// The copy must read the same the second time (Changed), or nothing is published. Then each name, read at one point:
/// one at the copy's root is left (Same); one with no root (new, or removed) is published at the copy's root by
/// compare-and-swap from the version it is at; one at another root is left as it is (Conflict). After a repair, a
/// collection frees the damaged copies. `each` hears each name and what became of it.
pub fn import<S: Store, R: Source>(store: &mut S, mut source: R, buffer: &mut [u8; CHUNK], block: &mut [u8; CHUNK], mut each: impl FnMut(Named, Outcome)) -> Result<Imported, Refused> {
    let mut node = [0u8; NODE_MAX];
    let (len, checked) = verify(&mut source, buffer, block, &mut node)?;
    let mut reader = Reader::open(&mut source).map_err(|error| Refused { error, at: 0 })?;
    let (mut stored, mut repaired, mut held, mut first) = (0, 0, 0, true);
    loop {
        let at = reader.at;
        let refused = |error| Refused { error, at };
        let Some((cid, len)) = reader.next(block).map_err(refused)? else { break };
        // The names node is the copy's, not a block the store takes.
        if core::mem::take(&mut first) { continue; }
        match store.get(&cid, buffer) {
            Ok(n) if cid.matches(&buffer[..n]) => held += 1,
            Ok(_) => return Err(refused(Error::Store(dag::Error::Corrupt))),
            Err(dag::Error::NotFound) => stored += 1,
            Err(dag::Error::Corrupt) => repaired += 1,
            Err(e) => return Err(refused(Error::Store(e))),
        }
        match store.put(cid.codec(), &block[..len]) {
            Ok(put) if put == cid => {}
            Ok(_) => return Err(refused(Error::Store(dag::Error::Corrupt))),
            Err(e) => return Err(refused(Error::Store(e))),
        }
    }
    let end = |error| Refused { error, at: reader.at };
    if reader.at != checked.bytes || reader.digest() != checked.digest { return Err(end(Error::Changed)); }
    let roots = decode_names(&node[..len]).map_err(end)?;
    let (mut names, mut heads) = ([""; NAMES], [(0, None); NAMES]);
    for (k, name) in names.iter_mut().enumerate().take(roots.len()) { *name = roots.get(k).name; }
    store.snapshot(&names[..roots.len()], &mut heads[..roots.len()]).map_err(|e| end(Error::Store(e)))?;
    for (k, &head) in heads.iter().enumerate().take(roots.len()) {
        let n = roots.get(k);
        let mut head = head;
        let outcome = loop {
            match head {
                (version, Some(root)) if root == n.root => break Outcome::Same(version),
                (version, Some(root)) => break Outcome::Conflict { version, root: Some(root) },
                (version, None) => match store.publish(n.name, version, &n.root) {
                    Ok(Some(new)) => break Outcome::Published(new),
                    // Another writer published since the names were read: what it left is read again, not published over.
                    Ok(None) => {
                        let mut now = [(0, None)];
                        store.snapshot(&[n.name], &mut now).map_err(|e| end(Error::Store(e)))?;
                        if now[0].1.is_none() { break Outcome::Conflict { version: now[0].0, root: None }; }
                        head = now[0];
                    }
                    Err(e) => return Err(end(Error::Store(e))),
                },
            }
        };
        each(n, outcome);
    }
    let collected = if repaired > 0 { Some(store.collect()) } else { None };
    Ok(Imported { checked, stored, repaired, held, collected })
}

/// What an export wrote: the names, the blocks under their roots (a block met twice written twice), the copy's bytes
/// and their SHA-256.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exported { pub names: usize, pub blocks: u64, pub bytes: u64, pub digest: [u8; DIGEST] }

/// Writes a copy of `names` to `target`: their current roots read at one point (`each` hears each name, sorted, with
/// its version and root), the names node, then every block under each root as `journal::walk` visits it within `steps`,
/// each read from the store and checked against its CID. Nothing is written for names the store does not take, more
/// than NAMES or one twice (Name), or a name with no root (Missing).
pub fn export<S: Store, T: Target>(store: &mut S, names: &[&str], target: T, steps: u64, buffer: &mut [u8; CHUNK], block: &mut [u8; CHUNK],
                                   mut each: impl FnMut(&str, u64, Option<Cid>)) -> Result<(T, Exported), Error> {
    let count = names.len();
    if !(1..=NAMES).contains(&count) || !names.iter().all(|n| valid_name(n.as_bytes())) || (1..count).any(|k| names[..k].contains(&names[k])) {
        return Err(Error::Name);
    }
    let mut heads = [(0, None); NAMES];
    store.snapshot(names, &mut heads[..count]).map_err(Error::Store)?;
    // Sorted by name, as the names node lists them.
    let mut order = [0usize; NAMES];
    for (k, o) in order.iter_mut().enumerate() { *o = k; }
    order[..count].sort_unstable_by_key(|&k| names[k]);
    for &k in &order[..count] { each(names[k], heads[k].0, heads[k].1); }
    if heads[..count].iter().any(|h| h.1.is_none()) { return Err(Error::Missing); }
    let named = |i: usize| Named { name: names[order[i]], version: heads[order[i]].0, root: heads[order[i]].1.unwrap() };
    write_each(store, count, named, target, steps, buffer, block)
}

/// Writes a copy of `names` at the roots given, as `export` writes the current ones: a version a name's history keeps,
/// for one. 1 to NAMES of them, sorted by name with each once, each a name the store takes at a version from 1 (else
/// Name).
pub fn write<B: Blocks, T: Target>(blocks: &mut B, names: &[Named], target: T, steps: u64, buffer: &mut [u8; CHUNK], block: &mut [u8; CHUNK]) -> Result<(T, Exported), Error> {
    write_each(blocks, names.len(), |k| names[k], target, steps, buffer, block)
}

fn write_each<'a, B: Blocks, T: Target>(store: &mut B, count: usize, named: impl Fn(usize) -> Named<'a>, target: T, steps: u64, buffer: &mut [u8; CHUNK],
                                        block: &mut [u8; CHUNK]) -> Result<(T, Exported), Error> {
    let mut node = [0u8; NODE_MAX];
    let len = encode_each(count, &named, &mut node)?;
    let root = Cid::of(Codec::DagCbor, &node[..len]);
    let mut writer = Writer::new(target, &root)?;
    writer.section(&root, &node[..len])?;
    let (mut blocks, mut failed) = (0, None);
    for i in 0..count {
        let walked = journal::walk(store, &named(i).root, buffer, steps, |s, cid| {
            let len = s.get(cid, &mut block[..])?;
            if !cid.matches(&block[..len]) { return Err(dag::Error::Corrupt); }
            writer.section(cid, &block[..len]).map_err(|e| { failed = Some(e); dag::Error::Store })?;
            blocks += 1;
            Ok(())
        });
        if let Err(e) = walked { return Err(failed.unwrap_or(Error::Store(e))); }
    }
    let (target, bytes, digest) = writer.done();
    Ok((target, Exported { names: count, blocks, bytes, digest }))
}
