//! Directories as objects (issue 311-STO-0027; MC-4.2, 4.5, 4.13): a directory is one DAG-CBOR block of the form
//! `{"v": 1, "entries": [[name, kind, size, link], ...]}`, its entries sorted by the names' bytes, each naming a file (an
//! object of `dag`) or another directory by its CID. A walk goes through every block under a root, checking each
//! directory against its CID and the schema and each entry against the object it names. Two roots are compared by the
//! paths where they differ, a subtree of one CID on both sides never read (311-STO-0039). No heap and no system calls:
//! tests/tree_host.rs.
use crate::cid::{Cid, Codec};
use crate::dag::{self, fetch, Blocks, Error, In, Out, Same, Sink, CHUNK, LINK};

/// The directory schema's version (`"v"`); another is refused (MC-4.13).
pub const VERSION: u64 = 1;
/// Bytes of a name at most.
pub const NAME_MAX: usize = 255;
// The least bytes of an entry: its array, a name of one byte, kind, size and link.
const ENTRY_MIN: usize = 1 + 2 + 1 + 1 + LINK;
// Entries of a directory at most: as many of the least as fit a block.
const ENTRIES: usize = CHUNK / ENTRY_MIN;
/// Directories on a path down from a root at most, the root's included.
pub const DEPTH: usize = 32;
/// Bytes of a path below a root at most: DEPTH names and the `/` between them.
pub const PATH_MAX: usize = DEPTH * (NAME_MAX + 1);

/// What an entry names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An object of `dag`: a `raw` block, or a node over chunks.
    File,
    /// A directory node.
    Directory,
}

impl Kind {
    const fn code(self) -> u64 { match self { Kind::File => 0, Kind::Directory => 1 } }
    const fn from_code(code: u64) -> Option<Self> { match code { 0 => Some(Kind::File), 1 => Some(Kind::Directory), _ => None } }
}

/// An entry of a directory: a name, what it names, its size (a file's bytes, a directory's entries) and its CID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry<'a> { pub name: &'a [u8], pub kind: Kind, pub size: u64, pub cid: Cid }

/// Whether `name` may name an entry: 1 to NAME_MAX bytes of UTF-8 without `/` or NUL, neither `.` nor `..`.
pub fn valid_name(name: &[u8]) -> bool {
    (1..=NAME_MAX).contains(&name.len()) && core::str::from_utf8(name).is_ok() && !name.iter().any(|&b| b == b'/' || b == 0)
        && name != b"." && name != b".."
}

impl Entry<'_> {
    // A valid name; a directory linked as `dag-cbor`.
    fn valid(&self) -> bool { valid_name(self.name) && (self.kind == Kind::File || self.cid.codec() == Codec::DagCbor) }
    /// What the entry names, without its name.
    pub fn side(&self) -> Side { Side { kind: self.kind, size: self.size, cid: self.cid } }
}

/// One side of a change: what an entry names, its size and its CID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Side { pub kind: Kind, pub size: u64, pub cid: Cid }

/// A path where two trees differ: an entry only in the second, only in the first, or in both naming another object,
/// size or kind (first, then second).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change { Added(Side), Removed(Side), Changed(Side, Side) }

// Counts the bytes of an encoding, so a directory over a block is refused before anything is written.
struct Count(usize);
impl Sink for Count {
    fn push(&mut self, data: &[u8]) { self.0 += data.len(); }
}

/// Encodes a directory of `entries`, sorted by name with each name once, into `out`; returns its length. Entries that
/// make no valid directory are refused (Format), and so is a directory over a block (TooLarge).
pub fn encode(entries: &[Entry], out: &mut [u8; CHUNK]) -> Result<usize, Error> { encode_each(entries.len(), |k| entries[k], out) }

/// As `encode`, the `count` entries given one by one, so no array of them is built (`worktree` reads them from a node).
pub(crate) fn encode_each<'a>(count: usize, entry: impl Fn(usize) -> Entry<'a>, out: &mut [u8; CHUNK]) -> Result<usize, Error> {
    if !(0..count).all(|k| entry(k).valid()) || (1..count).any(|k| entry(k - 1).name >= entry(k).name) { return Err(Error::Format); }
    let mut n = Count(0);
    encode_with(count, &entry, &mut n);
    if n.0 > CHUNK { return Err(Error::TooLarge); }
    let mut o = Out { bytes: out, at: 0 };
    encode_with(count, &entry, &mut o);
    Ok(o.at)
}

// The entries come one by one, so the decoder's check builds no array of them. The keys are in DAG-CBOR's order.
fn encode_with<'a>(count: usize, entry: impl Fn(usize) -> Entry<'a>, o: &mut impl Sink) {
    o.head(5, 2);
    o.text("v");
    o.head(0, VERSION);
    o.text("entries");
    o.head(4, count as u64);
    for k in 0..count {
        let e = entry(k);
        o.head(4, 4);
        o.head(3, e.name.len() as u64);
        o.push(e.name);
        o.head(0, e.kind.code());
        o.head(0, e.size);
        o.link(&e.cid);
    }
}

// Reads an entry and checks its name and link.
fn entry<'a>(i: &mut In<'a>) -> Result<Entry<'a>, Error> {
    if i.expect(4)? != 4 { return Err(Error::Format); }
    let len = i.expect(3)? as usize;
    let name = i.take(len)?;
    let kind = Kind::from_code(i.expect(0)?).ok_or(Error::Format)?;
    let size = i.expect(0)?;
    let e = Entry { name, kind, size, cid: i.link()? };
    if !e.valid() { return Err(Error::Format); }
    Ok(e)
}

/// A decoded directory: its entries read from the node's own bytes when asked for, by where each one starts.
pub struct Dir<'a> { bytes: &'a [u8], count: usize, starts: [u16; ENTRIES] }

impl<'a> Dir<'a> {
    pub fn len(&self) -> usize { self.count }
    pub fn is_empty(&self) -> bool { self.count == 0 }
    pub fn entry(&self, i: usize) -> Entry<'a> { self.at(self.starts[i]) }
    /// The node's bytes.
    pub(crate) fn bytes(&self) -> &'a [u8] { self.bytes }

    /// The entry named `name`, by a binary search of the sorted names.
    pub fn find(&self, name: &[u8]) -> Option<Entry<'a>> {
        let i = self.starts[..self.count].binary_search_by(|&at| self.at(at).name.cmp(name)).ok()?;
        Some(self.entry(i))
    }

    // Every entry was read and checked by `decode`.
    fn at(&self, start: u16) -> Entry<'a> { entry(&mut In { bytes: self.bytes, at: start as usize }).unwrap() }
}

/// Decodes a directory, only if it is exactly the canonical encoding of this schema.
pub fn decode(bytes: &[u8]) -> Result<Dir<'_>, Error> {
    if bytes.len() > CHUNK { return Err(Error::TooLarge); }
    let mut i = In { bytes, at: 0 };
    let keys = i.expect(5)?;
    // "v" sorts first, so another schema version is told apart before anything else of it is read.
    i.key("v")?;
    if i.expect(0)? != VERSION { return Err(Error::Version); }
    if keys != 2 { return Err(Error::Format); }
    i.key("entries")?;
    let count = i.expect(4)? as usize;
    if count > ENTRIES { return Err(Error::Format); }
    let mut dir = Dir { bytes, count, starts: [0; ENTRIES] };
    let mut last: &[u8] = &[];
    for k in 0..count {
        dir.starts[k] = i.at as u16;
        let e = entry(&mut i)?;
        // Sorted by the names' bytes, each name once.
        if e.name <= last { return Err(Error::Format); }
        last = e.name;
    }
    if i.at != bytes.len() { return Err(Error::Format); }
    // Shortest integer forms: the encoding of what was read must be the bytes themselves.
    let mut same = Same { bytes, at: 0, equal: true };
    encode_with(count, |k| dir.entry(k), &mut same);
    if !same.equal || same.at != bytes.len() { return Err(Error::Format); }
    Ok(dir)
}

/// Visits every block under `root`, a file or a directory: each directory node (then read and checked against its CID
/// and the schema) and every block of each file (`dag::walk`). Each entry's size is checked against its object: a
/// file's bytes, a directory's entries. The directories on the way down are a stack of (CID, next entry), each read
/// again on the way back, so `buffer` is the only block held. Returns the bytes of the files under the root; TooLarge
/// once it has taken `steps` steps (an entry gone through, a block visited), so its work is bounded whatever the tree.
pub fn walk<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK], mut steps: u64, visit: impl FnMut(&mut B, &Cid) -> Result<(), Error>) -> Result<u64, Error> {
    walk_from(blocks, root, buffer, &mut steps, visit)
}

/// The bytes of the files under `root`, as `walk` returns them, from the directory nodes and each file's root alone,
/// within `steps` steps (an entry gone through, a directory visited).
pub fn size<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK], mut steps: u64) -> Result<u64, Error> {
    size_from(blocks, root, buffer, &mut steps)
}

/// As `walk`, its steps taken from `left`, which it leaves at those not taken: a journal's walk shares its own so.
pub fn walk_from<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK], left: &mut u64, visit: impl FnMut(&mut B, &Cid) -> Result<(), Error>) -> Result<u64, Error> {
    let cell = core::cell::Cell::new(*left);
    let walked = tour(blocks, root, buffer, true, &cell, visit);
    *left = cell.get();
    walked
}

/// As `size`, its steps taken from `left`, which it leaves at those not taken.
pub fn size_from<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK], left: &mut u64) -> Result<u64, Error> {
    let cell = core::cell::Cell::new(*left);
    let sized = tour(blocks, root, buffer, false, &cell, |_, _| Ok(()));
    *left = cell.get();
    sized
}

// The entries of the directory `cid` names; none for a file (a `raw` one is not read).
fn entries<B: Blocks>(blocks: &mut B, cid: &Cid, buffer: &mut [u8; CHUNK]) -> Result<Option<usize>, Error> {
    if cid.codec() != Codec::DagCbor { return Ok(None); }
    let len = fetch(blocks, cid, buffer)?;
    Ok(decode(&buffer[..len]).ok().map(|dir| dir.len()))
}

// A file's bytes: every block of it visited (`whole`), or its root alone read.
fn file<B: Blocks>(blocks: &mut B, cid: &Cid, buffer: &mut [u8; CHUNK], whole: bool, visit: &mut impl FnMut(&mut B, &Cid) -> Result<(), Error>) -> Result<u64, Error> {
    if whole { dag::walk(blocks, cid, buffer, visit) } else { dag::size(blocks, cid, buffer) }
}

fn tour<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK], whole: bool, left: &core::cell::Cell<u64>, mut visit: impl FnMut(&mut B, &Cid) -> Result<(), Error>) -> Result<u64, Error> {
    // Each entry and each block visited is a step, the walks of files' included.
    let step = || left.get().checked_sub(1).map(|n| left.set(n)).ok_or(Error::TooLarge);
    let mut visit = |b: &mut B, cid: &Cid| { step()?; visit(b, cid) };
    let Some(count) = entries(blocks, root, buffer)? else { return file(blocks, root, buffer, whole, &mut visit) };
    visit(blocks, root)?;
    // The directories from the root down: each one's CID, the entries its parent says it has, the next entry.
    let mut stack = [(*root, count as u64, 0usize); DEPTH];
    let (mut depth, mut total) = (1, 0u64);
    while depth > 0 {
        let (cid, count, next) = stack[depth - 1];
        // Read again each time: a file's walk takes the buffer.
        let len = fetch(blocks, &cid, buffer)?;
        let dir = match decode(&buffer[..len]) {
            // A file's node where a directory belongs.
            Err(Error::Format) if dag::decode(&buffer[..len]).is_ok() => return Err(Error::Shape),
            dir => dir?,
        };
        if dir.len() as u64 != count { return Err(Error::Shape); }
        if next == dir.len() { depth -= 1; continue; }
        step()?;
        stack[depth - 1].2 += 1;
        let Entry { kind, size, cid, .. } = dir.entry(next);
        match kind {
            Kind::File => {
                let held = match file(blocks, &cid, buffer, whole, &mut visit) {
                    // A directory where a file belongs.
                    Err(Error::Format) if matches!(entries(blocks, &cid, buffer), Ok(Some(_))) => Err(Error::Shape),
                    held => held,
                }?;
                if held != size { return Err(Error::Shape); }
                total = total.checked_add(size).ok_or(Error::TooLarge)?;
            }
            Kind::Directory => {
                if depth == DEPTH { return Err(Error::Shape); }
                visit(blocks, &cid)?;
                stack[depth] = (cid, size, 0);
                depth += 1;
            }
        }
    }
    Ok(total)
}

// The empty directory's encoding: the root of the empty tree.
const EMPTY: [u8; 13] = *b"\xa2\x61v\x01\x67entries\x80";

// A directory being compared: its CID on each side (none, the empty tree), the entries its parent says it has (none at
// the root), the next entry of each side, and the length of its path.
#[derive(Clone, Copy)]
struct Pair { cid: [Option<Cid>; 2], count: [Option<u64>; 2], next: [usize; 2], path: usize }

// The directory `cid` names, read into `buffer` and checked against its CID, the schema and `count`; the empty tree for
// none. A read is a step.
fn side<'b, B: Blocks>(blocks: &mut B, cid: Option<&Cid>, count: Option<u64>, buffer: &'b mut [u8; CHUNK], left: &mut u64) -> Result<Dir<'b>, Error> {
    let Some(cid) = cid else { return decode(&EMPTY) };
    *left = left.checked_sub(1).ok_or(Error::TooLarge)?;
    if cid.codec() != Codec::DagCbor { return Err(Error::Shape); }
    let len = fetch(blocks, cid, buffer)?;
    let dir = match decode(&buffer[..len]) {
        // A file's node where a directory belongs.
        Err(Error::Format) if dag::decode(&buffer[..len]).is_ok() => return Err(Error::Shape),
        dir => dir?,
    };
    if count.is_some_and(|count| count != dir.len() as u64) { return Err(Error::Shape); }
    Ok(dir)
}

/// Compares the trees under `a` and `b`, each a directory or none (the empty tree), and visits each path where they
/// differ, below the roots, in the order of the names' bytes, depth first. An entry the same on both sides (kind, size
/// and CID) is not read; a directory on one side only is one change, its entries not listed; a directory on both sides
/// whose entries differ is gone into. Each node read is checked against its CID, the schema and its parent's entry.
/// `path` holds the path visited (PATH_MAX bytes hold every path): a longer one, or more than `left` steps (an entry
/// compared, a node read), is TooLarge; `left` is left at the steps not taken. Returns the changes visited; a visit's
/// error ends it.
pub fn diff<B: Blocks>(blocks: &mut B, a: Option<&Cid>, b: Option<&Cid>, buffers: [&mut [u8; CHUNK]; 2], path: &mut [u8], left: &mut u64, mut visit: impl FnMut(&[u8], Change) -> Result<(), Error>) -> Result<u64, Error> {
    if a == b { return Ok(0); }
    let mut stack = [Pair { cid: [a.copied(), b.copied()], count: [None, None], next: [0, 0], path: 0 }; DEPTH];
    let (mut depth, mut changes) = (1, 0u64);
    let [first, second] = buffers;
    while depth > 0 {
        let pair = stack[depth - 1];
        // Read again after each directory gone into, as `walk` reads its own: the buffers hold one pair at a time.
        let one = side(blocks, pair.cid[0].as_ref(), pair.count[0], first, left)?;
        let two = side(blocks, pair.cid[1].as_ref(), pair.count[1], second, left)?;
        let [mut i, mut j] = pair.next;
        let mut down = None;
        while down.is_none() && (i < one.len() || j < two.len()) {
            *left = left.checked_sub(1).ok_or(Error::TooLarge)?;
            let x = (i < one.len()).then(|| one.entry(i));
            let y = (j < two.len()).then(|| two.entry(j));
            let (name, change) = match (x, y) {
                (Some(x), Some(y)) if x.name == y.name => {
                    (i, j) = (i + 1, j + 1);
                    if x.side() == y.side() { continue; }
                    // Two directories: gone into, so an entry of one CID but another count is read and refused.
                    if x.kind == Kind::Directory && y.kind == Kind::Directory {
                        down = Some([x, y]);
                        (x.name, None)
                    } else {
                        (x.name, Some(Change::Changed(x.side(), y.side())))
                    }
                }
                (Some(x), Some(y)) if y.name < x.name => { j += 1; (y.name, Some(Change::Added(y.side()))) }
                (Some(x), _) => { i += 1; (x.name, Some(Change::Removed(x.side()))) }
                (None, Some(y)) => { j += 1; (y.name, Some(Change::Added(y.side()))) }
                (None, None) => unreachable!(),
            };
            // The path: the directory's own, a `/` below the root, and the name.
            let at = pair.path + usize::from(pair.path > 0);
            let end = at + name.len();
            if end > path.len() { return Err(Error::TooLarge); }
            if pair.path > 0 { path[pair.path] = b'/'; }
            path[at..end].copy_from_slice(name);
            match (change, down) {
                (Some(change), _) => { visit(&path[..end], change)?; changes += 1; }
                (None, Some([x, y])) => {
                    if depth == DEPTH { return Err(Error::Shape); }
                    stack[depth - 1].next = [i, j];
                    stack[depth] = Pair { cid: [Some(x.cid), Some(y.cid)], count: [Some(x.size), Some(y.size)], next: [0, 0], path: end };
                }
                (None, None) => unreachable!(),
            }
        }
        if down.is_some() { depth += 1; } else { depth -= 1; }
    }
    Ok(changes)
}
